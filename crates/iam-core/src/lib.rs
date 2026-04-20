//! Core primitives, domain models, and traits for the GridTokenX IAM service.
//!
//! This crate contains the shared logic and definitions that are used by all other crates
//! in the IAM workspace, following a "Sync Core" design pattern.

pub mod error;
pub mod domain;
pub mod traits;
pub mod config;

pub use error::ApiError;
pub use traits::*;
