//! Official Microsoft sign-in: OAuth device-code flow -> Xbox Live -> XSTS -> Minecraft Services.
//! SAYNAY never sees a password. The refresh token lives in the OS keychain; access tokens stay in memory
//! and are never sent to the UI.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

use crate::{db::Db, error::*, state::AppState};

const SCOPE: &str = "XboxLive.signin offline_access";
const MS: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
const KEY_SERVICE: &str = "saynay-launcher";
const KEY_USER: &str = "microsoft-refresh-token";

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct McSession {
    pub name: String,
    pub uuid: String,
    #[serde(skip)]
    pub access_token: String,
    #[serde(skip)]
    pub expires_at: i64,
}

#[derive(Deserialize, Clone, Debug)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    #[serde(default = "five")]
    pub interval: u64,
}
fn five() -> u64 {
    5
}

struct TokenSet {
    access: String,
    refresh: String,
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// The Azure app (client) ID must be your own registration, approved by Mojang for Minecraft Services.
pub fn client_id(db: &Db) -> Result<String> {
    if let Some(v) = db.get_setting("ms_client_id")? {
        if !v.trim().is_empty() {
            return Ok(v.trim().to_string());
        }
    }
    if let Some(v) = option_env!("SAYNAY_MS_CLIENT_ID") {
        if !v.is_empty() {
            return Ok(v.to_string());
        }
    }
    fail("No Microsoft client ID is configured. Add your approved Azure app client ID in Settings.")
}

// ---------- keychain ----------

fn entry() -> Result<keyring::Entry> {
    keyring::Entry::new(KEY_SERVICE, KEY_USER).map_err(|e| AppError::Msg(format!("keychain unavailable: {e}")))
}
pub fn store_refresh(token: &str) -> Result<()> {
    entry()?.set_password(token).map_err(|e| AppError::Msg(format!("could not save login to the keychain: {e}")))
}
pub fn load_refresh() -> Option<String> {
    entry().ok()?.get_password().ok()
}
pub fn clear_refresh() {
    if let Ok(e) = entry() {
        let _ = e.delete_credential();
    }
}

// ---------- protocol ----------

fn explain(url: &str, status: u16, text: &str) -> AppError {
    if text.contains("Invalid app registration") {
        return AppError::Msg("Minecraft Services rejected this app registration. The Azure client ID must be approved by Mojang (aka.ms/mce-reviewappid) before it can sign players in.".into());
    }
    if let Ok(v) = serde_json::from_str::<Value>(text) {
        let m = match v["XErr"].as_u64() {
            Some(2148916233) => Some("This Microsoft account has no Xbox profile. Sign in once at xbox.com to create one."),
            Some(2148916235) => Some("Xbox Live is not available in your country or region."),
            Some(2148916238) => Some("This is a child account. Add it to a Microsoft family group to play."),
            _ => None,
        };
        if let Some(m) = m {
            return AppError::Msg(m.into());
        }
    }
    AppError::Msg(format!("Sign-in step failed with status {status} ({url})"))
}

async fn post_json(http: &reqwest::Client, url: &str, body: Value) -> Result<Value> {
    let resp = http.post(url).header("Accept", "application/json").json(&body).send().await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        return Err(explain(url, status.as_u16(), &text));
    }
    Ok(serde_json::from_str(&text)?)
}

pub async fn request_device_code(http: &reqwest::Client, cid: &str) -> Result<DeviceCode> {
    let resp = http.post(format!("{MS}/devicecode")).form(&[("client_id", cid), ("scope", SCOPE)]).send().await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        return fail(format!("Microsoft rejected the sign-in request. Check the client ID in Settings and that public client flows are enabled. ({status})"));
    }
    Ok(serde_json::from_str(&text)?)
}

async fn poll_token(http: &reqwest::Client, cid: &str, dc: &DeviceCode) -> Result<TokenSet> {
    let deadline = Instant::now() + Duration::from_secs(dc.expires_in);
    let mut interval = dc.interval.max(1);
    loop {
        tokio::time::sleep(Duration::from_secs(interval)).await;
        if Instant::now() > deadline {
            return fail("The sign-in code expired. Start again.");
        }
        let r: Value = http
            .post(format!("{MS}/token"))
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", cid),
                ("device_code", dc.device_code.as_str()),
            ])
            .send()
            .await?
            .json()
            .await?;
        if let Some(t) = r["access_token"].as_str() {
            return Ok(TokenSet { access: t.into(), refresh: r["refresh_token"].as_str().unwrap_or("").into() });
        }
        match r["error"].as_str() {
            Some("authorization_pending") => {}
            Some("slow_down") => interval += 5,
            Some("authorization_declined") => return fail("Sign-in was declined."),
            Some("expired_token") => return fail("The sign-in code expired. Start again."),
            Some(other) => return fail(format!("Microsoft sign-in failed: {other}")),
            None => return fail("Unexpected response from Microsoft."),
        }
    }
}

async fn refresh_token(http: &reqwest::Client, cid: &str, refresh: &str) -> Result<TokenSet> {
    let r: Value = http
        .post(format!("{MS}/token"))
        .form(&[("grant_type", "refresh_token"), ("client_id", cid), ("refresh_token", refresh), ("scope", SCOPE)])
        .send()
        .await?
        .json()
        .await?;
    match r["access_token"].as_str() {
        Some(t) => Ok(TokenSet { access: t.into(), refresh: r["refresh_token"].as_str().unwrap_or("").into() }),
        None => {
            clear_refresh();
            fail("Your Microsoft login expired. Sign in again.")
        }
    }
}

async fn xbox_chain(http: &reqwest::Client, ms_access: &str) -> Result<McSession> {
    let xbl = post_json(
        http,
        "https://user.auth.xboxlive.com/user/authenticate",
        json!({
            "Properties": { "AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={ms_access}") },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }),
    )
    .await?;
    let xbl_token = xbl["Token"].as_str().ok_or("Xbox Live returned no token")?;
    let uhs = xbl["DisplayClaims"]["xui"][0]["uhs"].as_str().ok_or("Xbox Live returned no user hash")?;

    let xsts = post_json(
        http,
        "https://xsts.auth.xboxlive.com/xsts/authorize",
        json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }),
    )
    .await?;
    let xsts_token = xsts["Token"].as_str().ok_or("XSTS returned no token")?;

    let mc = post_json(
        http,
        "https://api.minecraftservices.com/authentication/login_with_xbox",
        json!({ "identityToken": format!("XBL3.0 x={uhs};{xsts_token}") }),
    )
    .await?;
    let access = mc["access_token"].as_str().ok_or("Minecraft Services returned no token")?.to_string();
    let expires = mc["expires_in"].as_i64().unwrap_or(86_400);

    let prof = http.get("https://api.minecraftservices.com/minecraft/profile").bearer_auth(&access).send().await?;
    if prof.status().as_u16() == 404 {
        return fail("This Microsoft account does not own Minecraft: Java Edition.");
    }
    let prof: Value = prof.error_for_status()?.json().await?;
    Ok(McSession {
        name: prof["name"].as_str().ok_or("Profile has no name")?.to_string(),
        uuid: prof["id"].as_str().ok_or("Profile has no id")?.to_string(),
        access_token: access,
        expires_at: now() + expires,
    })
}

// ---------- public flow used by commands ----------

pub async fn complete_device_login(app: &AppHandle, cid: &str, dc: &DeviceCode) -> Result<McSession> {
    let state = app.state::<AppState>();
    let t = poll_token(&state.http, cid, dc).await?;
    if !t.refresh.is_empty() {
        store_refresh(&t.refresh)?;
    }
    let s = xbox_chain(&state.http, &t.access).await?;
    *state.session.lock().await = Some(s.clone());
    Ok(s)
}

/// A valid Minecraft session, refreshed silently from the keychain when needed.
pub async fn ensure_session(app: &AppHandle) -> Result<McSession> {
    let state = app.state::<AppState>();
    {
        let g = state.session.lock().await;
        if let Some(s) = g.as_ref() {
            if s.expires_at > now() + 300 {
                return Ok(s.clone());
            }
        }
    }
    let refresh = load_refresh().ok_or_else(|| AppError::Msg("Sign in with your Microsoft account first.".into()))?;
    let cid = client_id(&state.db)?;
    let t = refresh_token(&state.http, &cid, &refresh).await?;
    if !t.refresh.is_empty() {
        store_refresh(&t.refresh)?;
    }
    let s = xbox_chain(&state.http, &t.access).await?;
    *state.session.lock().await = Some(s.clone());
    Ok(s)
}
