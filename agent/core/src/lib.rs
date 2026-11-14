pub mod actions;
pub mod agent;
pub mod auth;
pub mod collectors;
pub mod config;
pub mod model;
pub mod paths;
pub mod proto;
pub mod qr;
pub mod ratelimit;
pub mod server;
pub mod state;
pub mod statuses;
pub mod validate;
pub mod watch;

pub use agent::{Agent, AgentOptions, AgentStatus, SaveResult, StartError};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PROTOCOL_VERSION: u32 = 1;
