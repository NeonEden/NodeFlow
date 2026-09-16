
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use reqwest::{Client, header::{AUTHORIZATION, CONTENT_TYPE}};
use crate::cerebro::AppState;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TokenInfo {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: i64, // epoch seconds
}

impl TokenInfo {
    pub fn is_valid(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        now < self.expires_at - 60 // 1 min safety
    }
}

pub struct GraphClient {
    client: Client,
    token: TokenInfo,
    tenant: String,
}

impl GraphClient {
    pub fn new(token: TokenInfo) -> Self {
        Self {
            client: Client::new(),
            token,
            tenant: "common".to_string(),
        }
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.token.access_token)
    }

    async fn request(&self, method: reqwest::Method, url: &str, body: Option<serde_json::Value>) -> Result<serde_json::Value, String> {
        let mut req = self.client.request(method, url)
            .header(AUTHORIZATION, self.auth_header());
        if let Some(b) = body {
            req = req.header(CONTENT_TYPE, "application/json").json(&b);
        }
        let resp = req.send().await.map_err(|e| e.to_string())?;
        let status = resp.status();
        let txt = resp.text().await.map_err(|e| e.to_string())?;
        serde_json::from_str(&txt).map_err(|e| format!("parse error {}: {}", status, e))
    }

    // List children of a folder (by item id or root)
    pub async fn list_children(&self, item_id: Option<&str>) -> Result<serde_json::Value, String> {
        let base = "https://graph.microsoft.com/v1.0/me/drive";
        let url = match item_id {
            Some(id) => format!("{}/items/{}/children", base, id),
            None => format!("{}/root/children", base),
        };
        self.request(reqwest::Method::GET, &url, None).await
    }

    // Download content of a file (by item id)
    pub async fn download(&self, item_id: &str) -> Result<Vec<u8>, String> {
        let url = format!("https://graph.microsoft.com/v1.0/me/drive/items/{}/content", item_id);
        let resp = self.client.get(&url)
            .header(AUTHORIZATION, self.auth_header())
            .send().await.map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("download error {}", resp.status()));
        }
        resp.bytes().await.map(|b| b.to_vec()).map_err(|e| e.to_string())
    }

    // Upload (create or replace) a file under a parent folder
    pub async fn upload(&self, parent_id: &str, name: &str, content: &[u8]) -> Result<serde_json::Value, String> {
        let url = format!("https://graph.microsoft.com/v1.0/me/drive/items/{}/children/{}:/content", parent_id, name);
        let resp = self.client.put(&url)
            .header(AUTHORIZATION, self.auth_header())
            .header(CONTENT_TYPE, "application/octet-stream")
            .body(content.to_vec())
            .send().await.map_err(|e| e.to_string())?;
        let txt = resp.text().await.map_err(|e| e.to_string())?;
        serde_json::from_str(&txt).map_err(|e| format!("upload parse error: {}", e))
    }

    // Delta query – incremental changes. `token` optional previous delta link.
    pub async fn delta(&self, delta_link: Option<&str>) -> Result<serde_json::Value, String> {
        let base = "https://graph.microsoft.com/v1.0/me/drive/root/delta";
        let url = delta_link.unwrap_or(&base);
        self.request(reqwest::Method::GET, url, None).await
    }
}
