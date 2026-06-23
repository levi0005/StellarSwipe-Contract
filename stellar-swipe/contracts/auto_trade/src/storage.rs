#![allow(dead_code)]
use soroban_sdk::{contracttype, symbol_short, Address, Env};

use crate::auth::{AuthConfig, AuthKey};

#[contracttype]
#[derive(Clone)]
pub struct Signal {
    pub signal_id: u64,
    pub price: i128,
    pub expiry: u64,
    pub base_asset: u32,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RateLimitInfo {
    pub user: Address,
    pub is_limited: bool,
    pub expires_at: u64,
}

#[contracttype]
pub enum DataKey {
    Trades(Address, u64),
    Signal(u64),
    RateLimitInfo(Address),
}

/// Get a signal by ID
pub fn get_signal(env: &Env, id: u64) -> Option<Signal> {
    env.storage().persistent().get(&DataKey::Signal(id))
}

/// Set a signal
pub fn set_signal(env: &Env, id: u64, signal: &Signal) {
    env.storage().persistent().set(&DataKey::Signal(id), signal);
}

/// Test helper: auth plus max temporary SDEX balance.
pub fn authorize_user(env: &Env, user: &Address) {
    authorize_user_with_limits(env, user, i128::MAX / 4, 30);
    env.storage()
        .temporary()
        .set(&(user.clone(), symbol_short!("balance")), &i128::MAX);
}

/// Authorize a user with explicit limits.
pub fn authorize_user_with_limits(
    env: &Env,
    user: &Address,
    max_trade_amount: i128,
    duration_days: u32,
) {
    let config = AuthConfig {
        authorized: true,
        max_trade_amount,
        expires_at: env.ledger().timestamp() + (duration_days as u64 * 86400),
        granted_at: env.ledger().timestamp(),
    };
    env.storage()
        .persistent()
        .set(&AuthKey::Authorization(user.clone()), &config);
    env.storage()
        .temporary()
        .set(&(user.clone(), symbol_short!("balance")), &i128::MAX);
}

pub fn revoke_user_authorization(env: &Env, user: &Address) {
    env.storage()
        .persistent()
        .remove(&AuthKey::Authorization(user.clone()));
}

/// Get the stored rate-limit info for a user, if any.
pub fn get_rate_limit_info(env: &Env, user: &Address) -> Option<RateLimitInfo> {
    env.storage()
        .persistent()
        .get(&DataKey::RateLimitInfo(user.clone()))
}

/// Persist rate-limit info for a user.
pub fn set_rate_limit_info(env: &Env, user: &Address, info: &RateLimitInfo) {
    env.storage()
        .persistent()
        .set(&DataKey::RateLimitInfo(user.clone()), info);
}

/// Whether a user is currently rate limited (flag set and not yet expired).
pub fn is_rate_limited(env: &Env, user: &Address) -> bool {
    match get_rate_limit_info(env, user) {
        Some(info) => info.is_limited && env.ledger().timestamp() < info.expires_at,
        None => false,
    }
}
