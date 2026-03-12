use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub type Claims = Map<String, Value>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthIdentity {
    pub provider: String,
    pub subject: String,
    pub issuer: Option<String>,
    pub audiences: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub issued_at: Option<DateTime<Utc>>,
    pub not_before: Option<DateTime<Utc>>,
    pub session_id: Option<String>,
    pub claims: Claims,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CallbackResult {
    pub identity: AuthIdentity,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub set_cookie_headers: Vec<String>,
}

pub fn timestamp_to_utc(timestamp: i64) -> Option<DateTime<Utc>> {
    DateTime::from_timestamp(timestamp, 0)
}
