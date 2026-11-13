use serde::{Serialize, Serializer};

/// Every error a command can return. It is serialized to the frontend as
/// `{ kind, message, url? }` so the UI can react to specific cases.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("No CurseForge API key is set. Add one in Settings.")]
    NoApiKey,
    #[error("CurseForge rejected the API key. Check it in Settings.")]
    InvalidApiKey,
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

impl AppError {
    fn kind(&self) -> &'static str {
        match self {
            AppError::NoApiKey => "noApiKey",
            AppError::InvalidApiKey => "invalidApiKey",
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
        let mut st = s.serialize_struct("AppError", 3)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("message", &self.to_string())?;
        let url = match self {
            AppError::DistributionDisabled { url, .. } => Some(url.as_str()),
            _ => None,
        };
        st.serialize_field("url", &url)?;
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
