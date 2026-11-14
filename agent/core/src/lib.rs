pub mod config;
pub mod model;
pub mod paths;
pub mod proto;
pub mod state;
pub mod validate;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PROTOCOL_VERSION: u32 = 1;
