pub mod parser;
pub mod requirements;
pub mod install;
pub mod state;
pub mod ipc;

pub use parser::schema::InstallerManifest;
pub use state::{InstallerState, Page};