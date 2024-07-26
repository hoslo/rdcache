pub mod client;

pub mod error;

pub use client::*;
pub use error::{Error, Result};

mod script;
