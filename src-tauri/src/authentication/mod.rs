//! Accounts. Microsoft: official device-code flow -> Xbox Live -> XSTS -> Minecraft Services.
//! SAYNAY never sees a password. Refresh tokens live in the OS keychain (one entry per account),
//! access tokens stay in memory and are never sent to the UI or written to logs.
//! Offline: a local test account with no credentials, clearly labelled and unable to join online-mode servers.

use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

use crate::{db::Db, error::*, logger, state::AppState};

const SCOPE: &str = "XboxLive.signin offline_access";
const MS: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
const KEY_SERVICE: &str = "saynay-launcher";

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    /// The player UUID (no dashes). Unique per account.
    pub id: String,
    /// "microsoft" or "offline"
    pub kind: String,
    pub name: String,
    pub uuid: String,
    pub created_at: i64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct McSession {
    pub name: String,
    pub uuid: String,
    pub kind: String,
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

fn entry(uuid: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(KEY_SERVICE, &format!("microsoft-refresh-{uuid}")).map_err(|e| AppError::Msg(format!("keychain unavailable: {e}")))
}
pub fn store_refresh(uuid: &str, token: &str) -> Result<()> {
    entry(uuid)?.set_password(token).map_err(|e| AppError::Msg(format!("could not save login to the keychain: {e}")))
}
pub fn load_refresh(uuid: &str) -> Option<String> {
    entry(uuid).ok()?.get_password().ok()
}
pub fn clear_refresh(uuid: &str) {
    if let Ok(e) = entry(uuid) {
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

async fn refresh_token(http: &reqwest::Client, cid: &str, uuid: &str, refresh: &str) -> Result<TokenSet> {
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
            clear_refresh(uuid);
            logger::info("Microsoft refresh token was rejected; the saved login was removed.");
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
        kind: "microsoft".into(),
        access_token: access,
        expires_at: now() + expires,
    })
}

// ---------- accounts ----------

pub fn active_id(db: &Db) -> Option<String> {
    db.get_setting("active_account").ok().flatten().filter(|s| !s.is_empty())
}

pub fn active_account(db: &Db) -> Result<Option<Account>> {
    let id = active_id(db);
    let all = db.list_accounts()?;
    Ok(id.and_then(|i| all.into_iter().find(|a| a.id == i)))
}

pub fn set_active(db: &Db, id: &str) -> Result<()> {
    if !db.list_accounts()?.iter().any(|a| a.id == id) {
        return fail("That account does not exist.");
    }
    db.set_setting("active_account", id)
}

/// Java's UUID.nameUUIDFromBytes("OfflinePlayer:<name>"), the same id a vanilla offline-mode server derives.
pub fn offline_uuid(name: &str) -> String {
    let mut h: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
    h[6] = (h[6] & 0x0f) | 0x30;
    h[8] = (h[8] & 0x3f) | 0x80;
    hex::encode(h)
}

pub fn add_offline(db: &Db, name: &str) -> Result<Account> {
    let name = name.trim();
    if !(3..=16).contains(&name.len()) || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return fail("An offline username needs 3 to 16 letters, numbers or underscores.");
    }
    let uuid = offline_uuid(name);
    if db.list_accounts()?.iter().any(|a| a.id == uuid) {
        return fail("This offline account already exists.");
    }
    let a = Account { id: uuid.clone(), kind: "offline".into(), name: name.into(), uuid, created_at: now() };
    db.save_account(&a)?;
    if active_id(db).is_none() {
        db.set_setting("active_account", &a.id)?;
    }
    logger::info(&format!("Added offline test account {name}"));
    Ok(a)
}

pub async fn remove_account(app: &AppHandle, id: &str) -> Result<()> {
    let state = app.state::<AppState>();
    let all = state.db.list_accounts()?;
    let Some(a) = all.iter().find(|a| a.id == id) else { return fail("That account does not exist.") };
    if a.kind == "microsoft" {
        clear_refresh(&a.uuid);
    }
    state.db.delete_account(id)?;
    if active_id(&state.db).as_deref() == Some(id) {
        let next = all.iter().find(|a| a.id != id).map(|a| a.id.clone()).unwrap_or_default();
        state.db.set_setting("active_account", &next)?;
    }
    *state.session.lock().await = None;
    logger::info(&format!("Removed account {}", a.name));
    Ok(())
}

// ---------- public flow used by commands ----------

pub async fn complete_device_login(app: &AppHandle, cid: &str, dc: &DeviceCode) -> Result<Account> {
    let state = app.state::<AppState>();
    let t = poll_token(&state.http, cid, dc).await?;
    let s = xbox_chain(&state.http, &t.access).await?;
    if !t.refresh.is_empty() {
        store_refresh(&s.uuid, &t.refresh)?;
    }
    let existing = state.db.list_accounts()?.into_iter().find(|a| a.id == s.uuid);
    let acc = Account { id: s.uuid.clone(), kind: "microsoft".into(), name: s.name.clone(), uuid: s.uuid.clone(), created_at: existing.map(|a| a.created_at).unwrap_or_else(now) };
    state.db.save_account(&acc)?;
    state.db.set_setting("active_account", &acc.id)?;
    *state.session.lock().await = Some(s);
    logger::info(&format!("Signed in Microsoft account {}", acc.name));
    Ok(acc)
}

/// A session for the active account. Offline accounts need no network. Microsoft accounts refresh silently.
pub async fn ensure_session(app: &AppHandle) -> Result<McSession> {
    let state = app.state::<AppState>();
    let Some(acc) = active_account(&state.db)? else {
        return fail("Add an account first: sign in with Microsoft, or add an offline test account.");
    };
    if acc.kind == "offline" {
        return Ok(McSession { name: acc.name, uuid: acc.uuid, kind: "offline".into(), access_token: "0".into(), expires_at: i64::MAX });
    }
    {
        let g = state.session.lock().await;
        if let Some(s) = g.as_ref() {
            if s.uuid == acc.uuid && s.expires_at > now() + 300 {
                return Ok(s.clone());
            }
        }
    }
    let refresh = load_refresh(&acc.uuid).ok_or_else(|| AppError::Msg("This account's saved login is gone. Sign in with Microsoft again.".into()))?;
    let cid = client_id(&state.db)?;
    let t = refresh_token(&state.http, &cid, &acc.uuid, &refresh).await?;
    if !t.refresh.is_empty() {
        store_refresh(&acc.uuid, &t.refresh)?;
    }
    let s = xbox_chain(&state.http, &t.access).await?;
    *state.session.lock().await = Some(s.clone());
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_matches_java() {
        // UUID.nameUUIDFromBytes("OfflinePlayer:Notch") = b50ad385-829d-3141-a216-7e7d7539ba7f
        assert_eq!(offline_uuid("Notch"), "b50ad385829d3141a2167e7d7539ba7f");
    }
}
