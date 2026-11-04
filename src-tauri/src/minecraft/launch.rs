use std::{collections::HashMap, path::PathBuf};
use tauri::{AppHandle, Manager};

use crate::{
    authentication::{self, McSession},
    error::*,
    java,
    minecraft::{
        version::{self, Arg, OneOrMany},
        Instance,
    },
    paths, performance,
    state::AppState,
    system::process::PreparedLaunch,
};

fn subst(s: &str, vars: &HashMap<&str, String>) -> String {
    let mut o = s.to_string();
    for (k, v) in vars {
        o = o.replace(&format!("${{{k}}}"), v);
    }
    o
}

fn expand(args: &[Arg], feats: &HashMap<&str, bool>, vars: &HashMap<&str, String>) -> Vec<String> {
    let mut out = Vec::new();
    for a in args {
        match a {
            Arg::Plain(s) => out.push(subst(s, vars)),
            Arg::Cond { rules, value } => {
                if version::rules_allow(rules, feats) {
                    match value {
                        OneOrMany::One(s) => out.push(subst(s, vars)),
                        OneOrMany::Many(v) => out.extend(v.iter().map(|s| subst(s, vars))),
                    }
                }
            }
        }
    }
    out
}

/// Build the full java argument list for an installed instance.
pub fn build(inst: &Instance, sess: &McSession, jvm_flags: &[String], extra: &[String]) -> Result<Vec<String>> {
    let r = version::resolve(&inst.version_id)?;
    let libs = version::plan_libraries(&r.v);

    let mut cp: Vec<String> = Vec::new();
    for l in libs.iter().filter(|l| l.on_classpath) {
        let p = paths::libraries().join(&l.path);
        if !p.is_file() {
            return fail(format!("Missing library {} - repair the instance on the Versions page.", l.path));
        }
        cp.push(p.to_string_lossy().into_owned());
    }
    let jar = version::client_jar(&r.jar_id);
    if !jar.is_file() {
        return fail("The Minecraft client jar is missing - repair the instance on the Versions page.");
    }
    cp.push(jar.to_string_lossy().into_owned());
    let classpath = cp.join(if cfg!(windows) { ";" } else { ":" });
    let natives = version::natives_dir(&r.jar_id);
    let assets_root = paths::assets();
    let s = &inst.settings;

    let mut vars: HashMap<&str, String> = HashMap::new();
    vars.insert("auth_player_name", sess.name.clone());
    vars.insert("auth_uuid", sess.uuid.clone());
    vars.insert("auth_access_token", sess.access_token.clone());
    vars.insert("auth_session", sess.access_token.clone());
    vars.insert("clientid", String::new());
    vars.insert("auth_xuid", String::new());
    vars.insert("user_type", "msa".into());
    vars.insert("user_properties", "{}".into());
    vars.insert("version_name", inst.version_id.clone());
    vars.insert("version_type", r.v.kind.clone().unwrap_or_else(|| "release".into()));
    vars.insert("game_directory", inst.game_dir.clone());
    vars.insert("assets_root", assets_root.to_string_lossy().into_owned());
    vars.insert("game_assets", assets_root.join("virtual").join("legacy").to_string_lossy().into_owned());
    vars.insert("assets_index_name", r.v.asset_index.as_ref().map(|a| a.id.clone()).unwrap_or_default());
    vars.insert("natives_directory", natives.to_string_lossy().into_owned());
    vars.insert("library_directory", paths::libraries().to_string_lossy().into_owned());
    vars.insert("classpath_separator", if cfg!(windows) { ";".into() } else { ":".into() });
    vars.insert("classpath", classpath.clone());
    vars.insert("launcher_name", "SAYNAY".into());
    vars.insert("launcher_version", env!("CARGO_PKG_VERSION").into());
    vars.insert("resolution_width", s.width.to_string());
    vars.insert("resolution_height", s.height.to_string());

    let feats = version::features(true);
    let ram = s.ram_mb.max(512);
    let mut args: Vec<String> = vec![format!("-Xms{}M", ram.min(1024)), format!("-Xmx{ram}M")];
    args.extend(jvm_flags.iter().cloned());
    args.push("-Dlog4j2.formatMsgNoLookups=true".into()); // Log4Shell mitigation for 1.7 - 1.18
    if let Some(a) = &r.v.arguments {
        args.extend(expand(&a.jvm, &feats, &vars));
    }
    if !args.iter().any(|a| a.starts_with("-Djava.library.path")) {
        args.push(format!("-Djava.library.path={}", natives.display()));
    }
    if !args.iter().any(|a| a == "-cp" || a == "-classpath") {
        args.push("-cp".into());
        args.push(classpath);
    }
    args.extend(extra.iter().cloned());
    args.push(r.v.main_class.clone());

    match (&r.v.arguments, &r.v.minecraft_arguments) {
        (Some(a), _) => args.extend(expand(&a.game, &feats, &vars)),
        (None, Some(m)) => {
            for t in m.split_whitespace() {
                args.push(subst(t, &vars));
            }
            args.push("--width".into());
            args.push(s.width.to_string());
            args.push("--height".into());
            args.push(s.height.to_string());
        }
        _ => return fail("This version has no launch arguments"),
    }
    Ok(args)
}

/// Validate everything and produce a launch plan: login, Java, flags, arguments.
pub async fn prepare(app: &AppHandle, inst: &Instance, extra: Vec<String>) -> Result<PreparedLaunch> {
    if !inst.installed {
        return fail("This instance is not installed yet. Install it on the Versions page.");
    }
    let state = app.state::<AppState>();
    let session = authentication::ensure_session(app).await?;
    let r = version::resolve(&inst.version_id)?;
    let major = r.v.java_version.as_ref().map(|j| j.major_version).unwrap_or(8);

    let java_path = match inst.java_path.as_deref().filter(|p| !p.is_empty()) {
        Some(p) => {
            let pb = PathBuf::from(p);
            let probe_path = pb.clone();
            let info = tokio::task::spawn_blocking(move || java::probe(&probe_path))
                .await
                .map_err(|e| AppError::Msg(e.to_string()))?
                .ok_or_else(|| AppError::Msg("The selected Java runtime cannot start. Pick another one on the Performance page.".into()))?;
            if java::pick(major, &[info.clone()]).is_none() {
                return fail(format!(
                    "Minecraft {} needs Java {major}{}, but the selected runtime is Java {}.",
                    inst.mc_version,
                    if major <= 8 { " (exactly 8)" } else { " or newer" },
                    info.major
                ));
            }
            pb
        }
        None => PathBuf::from(java::ensure(app, &state.http, major).await?.path),
    };

    let threads = state.hardware().threads;
    let flags: Vec<String> = if inst.settings.custom_jvm_args.trim().is_empty() {
        performance::jvm_flags(&inst.profile, major, threads)
    } else {
        inst.settings.custom_jvm_args.split_whitespace().map(String::from).collect()
    };
    let args = build(inst, &session, &flags, &extra)?;
    std::fs::create_dir_all(inst.game_path())?;
    Ok(PreparedLaunch { instance_id: inst.id.clone(), java: java_path, args, game_dir: inst.game_path() })
}
