#![no_std]

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
#[allow(deprecated)]
pub mod version;

pub use cross_contract::{
    CrossContractError, CrossContractMessage, CrossContractMessageReceiverClient,
    CrossContractVersionClient, MessageStatus, MAX_MESSAGE_SIZE,
};
pub use errors::{ErrorCategory, RecoveryStrategy};
pub use math::{normalize_amount, scale_down, scale_up};
pub use version::{ContractKind, VersionError};
