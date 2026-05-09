//! Public library surface for Fence.
//!
//! The module root intentionally stays thin. Product behavior lives in focused
//! modules and is re-exported here for the CLI and future integrations.

mod constants;
mod model;
mod repository;
mod sentinel;

pub use model::*;
pub use repository::*;
pub use sentinel::*;
