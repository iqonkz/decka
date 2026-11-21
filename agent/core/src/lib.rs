pub mod actions;
pub mod agent;
pub mod apk_update;
pub mod ai;
pub mod audio;
pub mod auth;
pub mod collectors;
pub mod config;
pub mod diag;
pub mod dpapi;
pub mod icons;
pub mod installed;
#[cfg(windows)]
pub mod imaging;
pub mod logging;
pub mod media_control;
pub mod model;
pub mod netpolicy;
pub mod obs;
pub mod paths;
pub mod proto;
pub mod qr;
pub mod ratelimit;
pub mod safety;
pub mod secure;
pub mod server;
#[cfg(windows)]
pub mod sta;
pub mod state;
pub mod strip;
pub mod battery;
pub mod mail;
pub mod winnotif;
pub mod statuses;
pub mod templates;
pub mod tray_text;
pub mod twitch;
pub mod updates;
pub mod usb;
pub mod validate;
pub mod watch;
pub mod weather;
pub mod youtube;

pub use agent::{Agent, AgentOptions, AgentStatus, SaveResult, StartError};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PROTOCOL_VERSION: u32 = 2;
