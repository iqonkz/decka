pub mod actions;
pub mod agent;
pub mod config;
pub mod model;
pub mod paths;
pub mod proto;
pub mod server;
pub mod state;
pub mod validate;

pub use agent::{Agent, AgentOptions, AgentStatus, StartError};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PROTOCOL_VERSION: u32 = 1;
