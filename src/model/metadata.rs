use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct AppMetadata {
    pub name: String,
    pub description: String,

    pub current_version_android: i64,
    pub last_supported_version_android: i64,
    pub under_maintenance: bool,

    pub developer: String,
    pub developer_email: String,
    pub developer_phone_number: Option<String>,

    pub emoji_pack_version: i64,

    pub terms_of_service: String,
    pub privacy_policy: String,
    pub community_guideline: String,

    pub created_at: i64,
    pub created_by: String,
    pub updated_at: Option<i64>,
    pub updated_by: Option<String>
}