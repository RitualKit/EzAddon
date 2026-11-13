use crate::curseforge::KeyCheck;
use serde::{Serialize, Serializer};

/// Every error a command can return. It is serialized to the frontend as
/// `{ kind, message, url?, check? }` so the UI can react to specific cases.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("No CurseForge API key is set. Add one in Settings.")]
    NoApiKey,
    /// CurseForge answered 401 or 403. Keeps what it said so the user (and
    /// support) can see why, instead of a generic "invalid key".
    #[error("{}", rejected_message(*status, endpoint, detail))]
    Rejected { status: u16, endpoint: String, detail: String },
    #[error("CurseForge is rate-limiting requests (HTTP 429). Wait a minute and try again.")]
    RateLimited,
    /// The key check before saving found a problem; carries both probe results.
    #[error("{}", .0.hint)]
    KeyCheckFailed(Box<KeyCheck>),
    #[error("The WoW Forever AddOns folder is not set. Choose it in Settings.")]
    NoAddonsPath,
    #[error("This addon has no WoW Forever version on CurseForge.")]
    NoForeverFile,
    #[error("The author of {name} does not allow downloads from third-party apps. Download it from the CurseForge website instead.")]
    DistributionDisabled { name: String, url: String },
    #[error("CurseForge request failed: {0}")]
    Api(String),
    #[error("Network error: {0}")]
    Network(String),
    #[error("The downloaded file is not a valid addon archive: {0}")]
    BadArchive(String),
    #[error("File error: {0}")]
    Io(String),
    #[error("{0}")]
    Other(String),
}

fn rejected_message(status: u16, endpoint: &str, detail: &str) -> String {
    let detail = if detail.is_empty() { String::new() } else { format!(" ({detail})") };
    if status == 401 {
        format!(
            "CurseForge didn't recognize this API key (HTTP 401 on {endpoint}){detail}. \
             Copy it again from console.curseforge.com, or generate a new one."
        )
    } else {
        format!(
            "CurseForge refused access (HTTP {status} on {endpoint}){detail}. The key may be \
             revoked, not active yet, or not allowed for World of Warcraft. Try generating a new \
             key in the console, then use Settings → Test key."
        )
    }
}

impl AppError {
    fn kind(&self) -> &'static str {
        match self {
            AppError::NoApiKey => "noApiKey",
            AppError::Rejected { .. } => "invalidApiKey",
            AppError::RateLimited => "rateLimited",
            AppError::KeyCheckFailed(_) => "keyCheckFailed",
            AppError::NoAddonsPath => "noAddonsPath",
            AppError::NoForeverFile => "noForeverFile",
            AppError::DistributionDisabled { .. } => "distributionDisabled",
            AppError::Api(_) => "api",
            AppError::Network(_) => "network",
            AppError::BadArchive(_) => "badArchive",
            AppError::Io(_) => "io",
            AppError::Other(_) => "other",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("AppError", 4)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("message", &self.to_string())?;
        let url = match self {
            AppError::DistributionDisabled { url, .. } => Some(url.as_str()),
            _ => None,
        };
        st.serialize_field("url", &url)?;
        let check = match self {
            AppError::KeyCheckFailed(c) => Some(c.as_ref()),
            _ => None,
        };
        st.serialize_field("check", &check)?;
        st.end()
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Network(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Other(format!("Invalid data: {e}"))
    }
}

impl From<zip::result::ZipError> for AppError {
    fn from(e: zip::result::ZipError) -> Self {
        AppError::BadArchive(e.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
