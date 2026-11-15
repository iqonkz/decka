pub mod actions;
pub mod agent;
pub mod auth;
pub mod collectors;
pub mod config;
pub mod icons;
pub mod installed;
#[cfg(windows)]
pub mod imaging;
pub mod logging;
pub mod media_control;
pub mod model;
pub mod paths;
pub mod proto;
pub mod qr;
pub mod ratelimit;
pub mod server;
#[cfg(windows)]
pub mod sta;
pub mod state;
pub mod statuses;
pub mod templates;
pub mod tray_text;
pub mod usb;
pub mod validate;
pub mod watch;

pub use agent::{Agent, AgentOptions, AgentStatus, SaveResult, StartError};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PROTOCOL_VERSION: u32 = 1;
