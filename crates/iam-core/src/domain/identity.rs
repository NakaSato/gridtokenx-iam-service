/// `ServiceRole`/`ServiceClaims` — the RBAC identity carried on requests.
pub mod auth;
pub mod roles;
pub mod models;
/// Domain events published to the IAM event bus (registration, verification, etc).
pub mod events;
/// API-key domain types (the hashed/keyed credential, not the gRPC service).
pub mod keys;

pub use auth::*;
pub use roles::*;
pub use models::*;
pub use events::*;
pub use keys::*;
