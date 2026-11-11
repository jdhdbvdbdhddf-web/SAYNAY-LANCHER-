use serde::Serialize;

use crate::error::*;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub available: bool,
    pub url: String,
    pub notes: String,
}

fn parts(v: &str) -> Vec<u64> {
    v.trim_start_matches('v').split(|c: char| !c.is_ascii_digit()).filter_map(|s| s.parse().ok()).collect()
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    let (a, b) = (parts(latest), parts(current));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (a.get(i).copied().unwrap_or(0), b.get(i).copied().unwrap_or(0));
        if x != y {
            return x > y;
        }
    }
    false
}

/// Checks the latest GitHub release. Installing an update is a manual download from the release page
/// until a signing key is configured for in-app updates (see docs/ARCHITECTURE.md).
pub async fn check(http: &reqwest::Client, repo: &str) -> Result<UpdateInfo> {
    let ok = repo.split('/').count() == 2
        && repo.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'));
    if !ok {
        return fail("Set the update repository in Settings as owner/name, for example my-org/saynay-launcher.");
    }
    let v = crate::net::get_json(http, &format!("https://api.github.com/repos/{repo}/releases/latest")).await?;
    let latest = v["tag_name"].as_str().unwrap_or("").trim_start_matches('v').to_string();
    let current = env!("CARGO_PKG_VERSION").to_string();
    Ok(UpdateInfo {
        available: !latest.is_empty() && is_newer(&latest, &current),
        latest,
        current,
        url: v["html_url"].as_str().unwrap_or("").to_string(),
        notes: v["body"].as_str().unwrap_or("").chars().take(1200).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compares() {
        assert!(is_newer("0.2.0", "0.1.9"));
        assert!(is_newer("v1.0", "0.9.9"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.1", "0.1.1"));
    }
}
