// Design lints that would need a different error type across the crate (an `axum::Response` as the error of a guard, `()` for
// a failed decryption, a boxed callback in a static): the behaviour is deliberate, so they are allowed here, once.
#![allow(clippy::result_large_err, clippy::result_unit_err, clippy::type_complexity)]

pub mod actions;
pub mod agent;
pub mod apk_update;
pub mod assistant;
pub mod ai;
pub mod audio;
pub mod auth;
pub mod collectors;
pub mod config;
pub mod diag;
pub mod dpapi;
pub mod icons;
pub mod portowner;
pub mod installed;
#[cfg(windows)]
pub mod imaging;
pub mod logging;
pub mod lyrics;
pub mod glass;
pub mod island;
pub mod lyrics_overlay;
pub mod media_control;
pub mod model;
pub mod netpolicy;
pub mod notes;
pub mod obs;
pub mod overlay;
pub mod paths;
pub mod proto;
pub mod qr;
pub mod ratelimit;
pub mod safety;
pub mod secure;
pub mod server;
pub mod shot;
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
pub mod voice;
pub mod watch;
pub mod weather;
pub mod wintheme;
pub mod webhook;
pub mod youtube;

pub use agent::{Agent, AgentOptions, AgentStatus, SaveResult, StartError};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PROTOCOL_VERSION: u32 = 2;
