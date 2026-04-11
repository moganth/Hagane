/// IPC message protocol between the WebView2 JS frontend and the Rust engine.
/// JS → Rust: `window.chrome.webview.postMessage(JSON.stringify(msg))`
/// Rust → JS: webview.evaluate_script(&format!("window.__engineEvent({})", json))
use serde::{Deserialize, Serialize};

// ── Messages FROM JavaScript to Rust ─────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InboundMessage {
    /// UI is ready — send initial state
    Ready,

    /// User clicked Next
    Next,

    /// User clicked Back
    Back,

    /// User clicked Cancel
    Cancel,

    /// User accepted/declined the license
    LicenseAccepted { accepted: bool },

    /// User changed install directory
    SetInstallDir { path: String },

    /// User toggled a component
    SetComponent { id: String, selected: bool },

    /// User changed user info fields
    SetUserInfo { name: String, organization: String, serial_key: String },

    /// User changed finish-page toggles
    SetFinishOptions { launch_app: bool, create_desktop_shortcut: bool },

    /// Open an external URL in the system browser
    OpenUrl { url: String },

    /// User clicked Browse for install dir (engine opens native folder picker)
    BrowseInstallDir,

    /// Request current state snapshot (for page re-renders)
    GetState,
}

// ── Events FROM Rust to JavaScript ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum OutboundEvent {
    /// Full state snapshot (sent on Ready and after every state change)
    StateUpdate {
        state: serde_json::Value,
    },

    /// Navigate the WebView to a different page HTML
    Navigate {
        page: String,
        html: String,
    },

    /// Install progress tick
    Progress {
        current: usize,
        total: usize,
        percent: u8,
        label: String,
    },

    /// Log line during installation
    LogLine {
        text: String,
    },

    /// Install finished
    InstallComplete {
        success: bool,
        error: Option<String>,
    },

    /// Requirement check results available
    RequirementsResult {
        results: Vec<crate::requirements::CheckResult>,
        all_passed: bool,
    },

    /// Result of BrowseInstallDir — returns selected path or null if cancelled
    BrowseResult {
        path: Option<String>,
    },

    /// Show a native error dialog
    Error {
        title: String,
        message: String,
    },
}

impl OutboundEvent {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| r#"{"event":"error","message":"serialize failed"}"#.into())
    }

    /// Wrap the event as a JS call: window.__engineEvent(<json>)
    pub fn to_js_call(&self) -> String {
        format!("window.__engineEvent({})", self.to_json())
    }
}

/// Parse an inbound raw JSON string from the WebView postMessage.
pub fn parse_inbound(raw: &str) -> Result<InboundMessage, serde_json::Error> {
    serde_json::from_str(raw)
}