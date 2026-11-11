use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf};

use crate::{error::*, paths};

// ---------- version JSON model (the parts a launcher needs) ----------

#[derive(Deserialize, Clone, Debug, Default)]
pub struct OsRule {
    pub name: Option<String>,
    pub arch: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Rule {
    pub action: String,
    #[serde(default)]
    pub os: Option<OsRule>,
    #[serde(default)]
    pub features: Option<HashMap<String, bool>>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum Arg {
    Plain(String),
    Cond { rules: Vec<Rule>, value: OneOrMany },
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Arguments {
    #[serde(default)]
    pub game: Vec<Arg>,
    #[serde(default)]
    pub jvm: Vec<Arg>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Artifact {
    pub path: Option<String>,
    pub sha1: Option<String>,
    pub url: String,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct LibDownloads {
    pub artifact: Option<Artifact>,
    pub classifiers: Option<HashMap<String, Artifact>>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Extract {
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Library {
    pub name: String,
    #[serde(default)]
    pub downloads: Option<LibDownloads>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub rules: Option<Vec<Rule>>,
    #[serde(default)]
    pub natives: Option<HashMap<String, String>>,
    #[serde(default)]
    pub extract: Option<Extract>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct AssetIndexRef {
    pub id: String,
    pub sha1: String,
    pub url: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct ClientDownload {
    pub sha1: String,
    pub url: String,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Downloads {
    pub client: Option<ClientDownload>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct JavaVersionReq {
    #[serde(rename = "majorVersion")]
    pub major_version: u32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct VersionJson {
    pub id: String,
    #[serde(rename = "inheritsFrom", default)]
    pub inherits_from: Option<String>,
    #[serde(rename = "mainClass")]
    pub main_class: String,
    #[serde(rename = "minecraftArguments", default)]
    pub minecraft_arguments: Option<String>,
    #[serde(default)]
    pub arguments: Option<Arguments>,
    #[serde(rename = "assetIndex", default)]
    pub asset_index: Option<AssetIndexRef>,
    #[serde(default)]
    pub downloads: Option<Downloads>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(rename = "javaVersion", default)]
    pub java_version: Option<JavaVersionReq>,
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    /// Old Forge profiles name the vanilla jar to use instead of inheriting.
    #[serde(default)]
    pub jar: Option<String>,
}

pub struct Resolved {
    pub v: VersionJson,
    /// The id whose client jar goes on the classpath (the vanilla parent for Fabric).
    pub jar_id: String,
}

// ---------- rules ----------

pub fn os_name() -> &'static str {
    if cfg!(windows) { "windows" } else if cfg!(target_os = "macos") { "osx" } else { "linux" }
}

fn arch_matches(a: &str) -> bool {
    match a {
        "x86" => cfg!(target_arch = "x86"),
        "arm64" | "aarch64" => cfg!(target_arch = "aarch64"),
        "x86_64" | "amd64" | "x64" => cfg!(target_arch = "x86_64"),
        _ => true,
    }
}

fn rule_matches(r: &Rule, feats: &HashMap<&str, bool>) -> bool {
    if let Some(os) = &r.os {
        if let Some(n) = &os.name {
            if n != os_name() {
                return false;
            }
        }
        if let Some(a) = &os.arch {
            if !arch_matches(a) {
                return false;
            }
        }
    }
    if let Some(f) = &r.features {
        for (k, v) in f {
            if feats.get(k.as_str()).copied().unwrap_or(false) != *v {
                return false;
            }
        }
    }
    true
}

/// Mojang rule semantics: no rules = allowed; otherwise the last matching rule decides.
pub fn rules_allow(rules: &[Rule], feats: &HashMap<&str, bool>) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for r in rules {
        if rule_matches(r, feats) {
            allowed = r.action == "allow";
        }
    }
    allowed
}

// ---------- maven coordinates ----------

pub struct Coord {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub classifier: Option<String>,
    pub ext: String,
}

pub fn parse_coord(name: &str) -> Option<Coord> {
    let (n, ext) = match name.split_once('@') {
        Some((n, e)) => (n, e.to_string()),
        None => (name, "jar".to_string()),
    };
    let p: Vec<&str> = n.split(':').collect();
    if p.len() < 3 {
        return None;
    }
    Some(Coord {
        group: p[0].into(),
        artifact: p[1].into(),
        version: p[2].into(),
        classifier: p.get(3).map(|s| s.to_string()),
        ext,
    })
}

pub fn maven_path(c: &Coord) -> String {
    let cls = c.classifier.as_ref().map(|x| format!("-{x}")).unwrap_or_default();
    format!("{}/{}/{}/{}-{}{}.{}", c.group.replace('.', "/"), c.artifact, c.version, c.artifact, c.version, cls, c.ext)
}

// ---------- library planning ----------

/// Old profiles still point at http:// and the retired files.minecraftforge.net maven path.
pub fn normalize_url(u: &str) -> String {
    let u = u.replace("http://files.minecraftforge.net/maven/", "https://maven.minecraftforge.net/").replace("https://files.minecraftforge.net/maven/", "https://maven.minecraftforge.net/");
    if let Some(rest) = u.strip_prefix("http://") {
        format!("https://{rest}")
    } else {
        u
    }
}

#[derive(Clone, Debug)]
pub struct LibFile {
    pub key: String,
    /// Relative path under the shared libraries folder.
    pub path: String,
    pub url: String,
    pub sha1: Option<String>,
    pub on_classpath: bool,
    pub extract_natives: bool,
    pub exclude: Vec<String>,
}

pub fn features(has_resolution: bool) -> HashMap<&'static str, bool> {
    let mut m = HashMap::new();
    m.insert("has_custom_resolution", has_resolution);
    m
}

pub fn plan_libraries(v: &VersionJson) -> Vec<LibFile> {
    let feats = features(true);
    let mut out: Vec<LibFile> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let arch_tag = if cfg!(target_pointer_width = "64") { "64" } else { "32" };

    let mut push = |f: LibFile, out: &mut Vec<LibFile>| {
        if seen.insert(f.key.clone()) {
            out.push(f);
        }
    };

    for lib in &v.libraries {
        if let Some(r) = &lib.rules {
            if !rules_allow(r, &feats) {
                continue;
            }
        }
        let coord = parse_coord(&lib.name);
        let base_key = coord.as_ref().map(|c| format!("{}:{}", c.group, c.artifact)).unwrap_or_else(|| lib.name.clone());
        let exclude = lib.extract.as_ref().map(|e| e.exclude.clone()).unwrap_or_default();

        // Pre-1.19 style natives: a classifier jar that is extracted, never put on the classpath.
        if let Some(nat) = &lib.natives {
            if let Some(tpl) = nat.get(os_name()) {
                let cls = tpl.replace("${arch}", arch_tag);
                if let Some(a) = lib.downloads.as_ref().and_then(|d| d.classifiers.as_ref()).and_then(|c| c.get(&cls)) {
                    let path = a.path.clone().unwrap_or_else(|| {
                        coord
                            .as_ref()
                            .map(|c| maven_path(&Coord { group: c.group.clone(), artifact: c.artifact.clone(), version: c.version.clone(), classifier: Some(cls.clone()), ext: "jar".into() }))
                            .unwrap_or_default()
                    });
                    push(
                        LibFile { key: format!("{base_key}:{cls}"), path, url: normalize_url(&a.url), sha1: a.sha1.clone(), on_classpath: false, extract_natives: true, exclude: exclude.clone() },
                        &mut out,
                    );
                }
            }
        }

        // Main artifact (Mojang libraries, Fabric libraries from a maven URL).
        let classifier = coord.as_ref().and_then(|c| c.classifier.clone());
        let key = match &classifier {
            Some(c) => format!("{base_key}:{c}"),
            None => base_key.clone(),
        };
        let is_native_jar = classifier.as_deref().map_or(false, |c| c.starts_with("natives-"));
        if let Some(a) = lib.downloads.as_ref().and_then(|d| d.artifact.as_ref()) {
            let path = a.path.clone().or_else(|| coord.as_ref().map(maven_path)).unwrap_or_default();
            push(
                LibFile { key, path, url: normalize_url(&a.url), sha1: a.sha1.clone(), on_classpath: true, extract_natives: is_native_jar, exclude },
                &mut out,
            );
        } else if lib.natives.is_none() {
            if let Some(c) = &coord {
                let path = maven_path(c);
                let base = lib.url.clone().unwrap_or_else(|| "https://libraries.minecraft.net/".into());
                let url = normalize_url(&format!("{}{}", if base.ends_with('/') { base } else { format!("{base}/") }, path));
                push(LibFile { key, path, url, sha1: None, on_classpath: true, extract_natives: is_native_jar, exclude }, &mut out);
            }
        }
    }
    out
}

// ---------- loading and inheritance ----------

pub fn json_path(id: &str) -> PathBuf {
    paths::versions().join(id).join(format!("{id}.json"))
}

pub fn load(id: &str) -> Result<VersionJson> {
    let p = json_path(id);
    let bytes = std::fs::read(&p).map_err(|_| AppError::Msg(format!("Version {id} is not installed")))?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// Merge a child profile (Fabric) onto its parent. Child libraries come first so they win on conflicts.
pub fn resolve(id: &str) -> Result<Resolved> {
    let child = load(id)?;
    let Some(parent_id) = child.inherits_from.clone() else {
        let jar_id = child.jar.clone().filter(|j| !j.is_empty()).unwrap_or_else(|| id.to_string());
        return Ok(Resolved { jar_id, v: child });
    };
    let parent = resolve(&parent_id)?;
    let mut m = parent.v;
    m.id = child.id;
    m.main_class = child.main_class;
    let mut libs = child.libraries;
    libs.extend(m.libraries);
    m.libraries = libs;
    if let Some(ca) = child.arguments {
        let ma = m.arguments.get_or_insert_with(Default::default);
        ma.game.extend(ca.game);
        ma.jvm.extend(ca.jvm);
    }
    if child.minecraft_arguments.is_some() {
        m.minecraft_arguments = child.minecraft_arguments;
    }
    Ok(Resolved { jar_id: parent.jar_id, v: m })
}

pub fn client_jar(jar_id: &str) -> PathBuf {
    paths::versions().join(jar_id).join(format!("{jar_id}.jar"))
}

pub fn natives_dir(jar_id: &str) -> PathBuf {
    paths::versions().join(jar_id).join("natives")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(action: &str, os: Option<&str>) -> Rule {
        Rule { action: action.into(), os: os.map(|n| OsRule { name: Some(n.into()), arch: None }), features: None }
    }

    #[test]
    fn rules_follow_last_match() {
        let f = features(true);
        assert!(rules_allow(&[], &f));
        assert!(rules_allow(&[rule("allow", None)], &f));
        assert!(!rules_allow(&[rule("allow", None), rule("disallow", Some(os_name()))], &f));
        assert!(!rules_allow(&[rule("allow", Some("some-other-os"))], &f));
    }

    #[test]
    fn maven_paths() {
        let c = parse_coord("net.fabricmc:fabric-loader:0.16.5").unwrap();
        assert_eq!(maven_path(&c), "net/fabricmc/fabric-loader/0.16.5/fabric-loader-0.16.5.jar");
        let n = parse_coord("org.lwjgl:lwjgl:3.3.3:natives-windows").unwrap();
        assert_eq!(maven_path(&n), "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar");
    }
}
