//! Launcher log (logs/launcher.log). Secrets are scrubbed before anything is written.

use std::io::Write;

use crate::paths;

const MAX_BYTES: u64 = 1_500_000;

/// Removes values that must never reach a log: the value after --accessToken / --session and
/// anything that looks like a bearer token or a refresh token.
pub fn redact(s: &str) -> String {
    let mut out = s.to_string();
    for key in ["Bearer", "bearer", "Authorization", "accessToken", "refresh_token", "access_token"] {
        while let Some(i) = out.find(key) {
            let after = &out[i + key.len()..];
            let skip = after.len() - after.trim_start_matches(|c: char| matches!(c, '"' | ':' | '=' | ' ')).len();
            let tok = &after[skip..];
            let end = tok.find(|c: char| c.is_whitespace() || matches!(c, '"' | ',' | '}')).unwrap_or(tok.len());
            out.replace_range(i..i + key.len() + skip + end, "[redacted]");
        }
    }
    out
}

pub fn redact_args(args: &[String]) -> String {
    let mut v: Vec<String> = Vec::new();
    let mut hide = false;
    for a in args {
        if hide {
            v.push("[redacted]".into());
            hide = false;
        } else {
            hide = matches!(a.as_str(), "--accessToken" | "--session" | "--xuid" | "--clientId");
            v.push(a.clone());
        }
    }
    v.join(" ")
}

pub fn log(level: &str, msg: &str) {
    let dir = paths::logs();
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join("launcher.log");
    if std::fs::metadata(&file).map(|m| m.len() > MAX_BYTES).unwrap_or(false) {
        let _ = std::fs::rename(&file, dir.join("launcher.old.log"));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(file) {
        let _ = writeln!(f, "{} [{}] {}", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S"), level, redact(msg));
    }
}

pub fn info(msg: &str) {
    log("INFO", msg)
}
pub fn error(msg: &str) {
    log("ERROR", msg)
}

pub fn tail(lines: usize) -> String {
    let text = std::fs::read_to_string(paths::logs().join("launcher.log")).unwrap_or_default();
    let all: Vec<&str> = text.lines().collect();
    all[all.len().saturating_sub(lines)..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hides_tokens() {
        let a = vec!["--username".into(), "Steve".into(), "--accessToken".into(), "secret123".into(), "--uuid".into(), "abc".into()];
        let r = redact_args(&a);
        assert!(!r.contains("secret123") && r.contains("Steve"));
        assert!(!redact("{\"access_token\":\"eyJabc\",\"x\":1}").contains("eyJabc"));
    }
}
