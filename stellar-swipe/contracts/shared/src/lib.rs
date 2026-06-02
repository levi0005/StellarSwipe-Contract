#![no_std]

pub mod auth;
pub mod cross_contract;
pub mod events;
pub mod math;
pub mod version;

pub use cross_contract::{CrossContractError, CrossContractMessage, CrossContractMessageReceiverClient, CrossContractVersionClient, MessageStatus, MAX_MESSAGE_SIZE};
