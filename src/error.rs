//! Plugin error type.

use thiserror::Error;

/// Errors raised by the MediaFire plugin.
#[derive(Debug, Error)]
pub enum PluginError {
    #[error("MediaFire JSON parse error: {0}")]
    ParseJson(String),

    #[error("JSON error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("MediaFire HTTP returned status {status}: {message}")]
    HttpStatus { status: u16, message: String },

    #[error("host function response invalid: {0}")]
    HostResponse(String),

    #[error("URL is not a recognised MediaFire resource: {0}")]
    UnsupportedUrl(String),

    #[error("MediaFire file is offline or removed: {0}")]
    Offline(String),

    #[error("no direct download link found in MediaFire page (file may be private or password-protected)")]
    NoDirectLink,
}
