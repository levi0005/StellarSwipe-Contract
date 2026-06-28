#![no_std]

pub mod access_control;
/// Asset metadata registry (Issue #700).
pub mod asset_registry;

pub mod auth;
#[allow(deprecated)]
pub mod cross_contract;
pub mod errors;
/// Canonical event-topic constants (issue #585).
pub mod event_topics;
#[allow(deprecated)]
pub mod events;
/// Shared double-initialization guard (issue #584).
pub mod initializable;
/// Minimum-liquidity threshold guard for pooled-fund withdrawals (issue #591).
pub mod liquidity_pool;
/// Decimal-precision scaling helpers (Issue #562).
pub mod math;
/// Shared emergency-pause state and guard (Issue #561).
pub mod pausable;
#[allow(deprecated)]
pub mod version;

pub use cross_contract::{
    CrossContractError, CrossContractMessage, CrossContractMessageReceiverClient,
    CrossContractVersionClient, MessageStatus, MAX_MESSAGE_SIZE,
};
pub use errors::{ErrorCategory, RecoveryStrategy};
pub use pausable::{is_paused, require_not_paused, set_paused, PausableKey};
pub use version::{ContractKind, VersionError};
