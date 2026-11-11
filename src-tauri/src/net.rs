//! Small HTTPS helpers: every metadata request goes through here so failures read the same everywhere.

use crate::error::*;

pub fn ensure_https(url: &str) -> Result<()> {
    if !url.starts_with("https://") || url.contains(char::is_whitespace) {
        return fail(format!("Only https:// addresses are allowed: {url}"));
    }
    Ok(())
}

fn status_error(url: &str, status: reqwest::StatusCode, retry_after: Option<&str>) -> AppError {
    match status.as_u16() {
        429 | 403 if retry_after.is_some() || status.as_u16() == 429 => AppError::Msg(format!(
            "The server is rate limiting requests ({status}). Wait {} and try again: {url}",
            retry_after.map(|s| format!("{s} seconds")).unwrap_or_else(|| "a minute".into())
        )),
        404 => AppError::Msg(format!("Not found (404): {url}")),
        s if s >= 500 => AppError::Msg(format!("The server is having problems ({status}): {url}")),
        _ => AppError::Msg(format!("Request failed ({status}): {url}")),
    }
}

pub async fn get_text(http: &reqwest::Client, url: &str) -> Result<String> {
    ensure_https(url)?;
    let resp = http.get(url).send().await.map_err(|e| AppError::Msg(format!("Could not reach {url}: {e}")))?;
    let status = resp.status();
    if !status.is_success() {
        let ra = resp.headers().get("retry-after").and_then(|v| v.to_str().ok()).map(String::from);
        return Err(status_error(url, status, ra.as_deref()));
    }
    Ok(resp.text().await?)
}

pub async fn get_json(http: &reqwest::Client, url: &str) -> Result<serde_json::Value> {
    let text = get_text(http, url).await?;
    serde_json::from_str(&text).map_err(|_| AppError::Msg(format!("The server answered with unexpected data: {url}")))
}
