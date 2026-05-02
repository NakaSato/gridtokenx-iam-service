//! Persistence layer for the IAM service.
//! 
//! This crate provides concrete implementations of the core traits,
//! including database repositories (PostgreSQL), caching (Redis),
//! event publishing (Redis Streams), and email notifications (SMTP).

/// Redis-backed caching service.
pub mod cache;
/// Redis-backed event bus for domain events.
pub mod event_bus;
/// SMTP-based email notification service.
pub mod email;
/// SQLx-based PostgreSQL repositories.
pub mod repository;

pub use cache::CacheService;
pub use event_bus::EventBus;
pub use email::EmailService;
pub use repository::*;
