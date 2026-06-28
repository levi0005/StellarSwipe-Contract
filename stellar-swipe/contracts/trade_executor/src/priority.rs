//! Priority lanes for high-stake vs standard followers (Issue #682).
//!
//! During `batch_execute`, copy-trades are processed in priority-tier order so
//! that followers with larger stake or longer tenure enjoy preferential execution
//! during high-congestion periods. A fairness fallback prevents starvation of
//! lower-priority followers.

use soroban_sdk::{contracterror, contracttype, Address, Env, Vec};

use crate::StorageKey;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Default minimum stake (in XLM stroops) to qualify for the HighStake tier.
pub const DEFAULT_HIGH_STAKE_MIN: i128 = 1_000_000_000; // 1,000 XLM

/// Default minimum account age (seconds) to qualify for the HighTenure tier.
pub const DEFAULT_HIGH_TENURE_MIN_SECS: u64 = 90 * 24 * 60 * 60; // 90 days

/// Default number of consecutive priority-only batches before the fairness
/// fallback forces inclusion of standard-follower trades.
pub const DEFAULT_FAIRNESS_BATCH_WINDOW: u32 = 3;

// ── Priority tier ─────────────────────────────────────────────────────────────

/// Ordered priority tiers for follower copy trades.
/// Higher ordinal = higher priority.
#[contracttype]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u32)]
pub enum FollowerPriorityTier {
    /// Standard follower — default tier.
    Standard = 0,
    /// Follower with a tenure/age above threshold.
    HighTenure = 1,
    /// Follower with a stake above threshold.
    HighStake = 2,
}

// ── Configuration ─────────────────────────────────────────────────────────────

/// Priority-lane configuration stored in instance storage.
#[contracttype]
#[derive(Clone, Debug)]
pub struct PriorityConfig {
    /// Minimum stake (in smallest token unit) to qualify for HighStake tier.
    pub high_stake_min: i128,
    /// Minimum account age (seconds since first activity) for HighTenure tier.
    pub high_tenure_min_secs: u64,
    /// Consecutive priority-only batches allowed before forcing fairness fallback.
    pub fairness_batch_window: u32,
}

impl Default for PriorityConfig {
    fn default() -> Self {
        Self {
            high_stake_min: DEFAULT_HIGH_STAKE_MIN,

// ── Error ─────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum PriorityError {
    ConfigNotSet = 1,
}

// ── Storage helpers ───────────────────────────────────────────────────────────

/// Read the stored priority config, or return the default if not yet set.
pub fn get_priority_config(env: &Env) -> PriorityConfig {
    env.storage()
        .instance()
        .get(&StorageKey::PriorityConfig)
        .unwrap_or_default()
}

/// Set the priority config (admin only).
pub fn set_priority_config(env: &Env, config: &PriorityConfig) {
    env.storage()
        .instance()
        .set(&StorageKey::PriorityConfig, config);

// ── Tier determination ────────────────────────────────────────────────────────

/// Determine the priority tier for a follower based on the current config.
///
/// Checks the user's stake against the high_stake_min threshold and their
/// account age (via the configured portfolio contract) against the tenure threshold.
/// Returns the highest applicable tier.
pub fn get_follower_priority_tier(
    env: &Env,
    user: &Address,
    portfolio: Option<&Address>,
) -> FollowerPriorityTier {
    let config = get_priority_config(env);

    // Check stake via portfolio contract
    let stake = if let Some(portfolio_addr) = portfolio {
        get_follower_stake(env, user, portfolio_addr)
    } else {
        0
    };

    if stake >= config.high_stake_min {
        return FollowerPriorityTier::HighStake;
    }

    // Check tenure / account age
    if let Some(portfolio_addr) = portfolio {
        let account_age = get_follower_account_age(env, user, portfolio_addr);
        if account_age >= config.high_tenure_min_secs {
            return FollowerPriorityTier::HighTenure;
        }
    }

    FollowerPriorityTier::Standard
}

/// Attempt to read a follower's stake from the portfolio contract via
/// `get_stake(user) -> i128`. Returns 0 if the portfolio doesn't expose it.
fn get_follower_stake(env: &Env, user: &Address, portfolio: &Address) -> i128 {
    let sym = soroban_sdk::Symbol::new(env, "get_stake");
    let mut args = soroban_sdk::Vec::<soroban_sdk::Val>::new(env);
    args.push_back(user.clone().into_val(env));
    env.try_invoke_contract::<i128, soroban_sdk::Error>(portfolio, &sym, args)
        .ok()
        .flatten()
        .unwrap_or(0)
}

/// Attempt to read a follower's account age from the portfolio contract via
/// `get_account_age(user) -> u64`. Returns 0 if the portfolio doesn't expose it.
fn get_follower_account_age(env: &Env, user: &Address, portfolio: &Address) -> u64 {
    let sym = soroban_sdk::Symbol::new(env, "get_account_age");
    let mut args = soroban_sdk::Vec::<soroban_sdk::Val>::new(env);
    args.push_back(user.clone().into_val(env));
    env.try_invoke_contract::<u64, soroban_sdk::Error>(portfolio, &sym, args)
        .ok()
        .flatten()
        .unwrap_or(0)
}

// ── Batch sorting ─────────────────────────────────────────────────────────────

/// Represents a trade input paired with its computed priority tier for sorting.
#[derive(Clone, Debug)]
struct PrioritizedTrade {
    tier: FollowerPriorityTier,
    user: Address,
    token: Address,
    amount: i128,
}

/// Sort a batch of trades by priority tier (highest first), then return them in
/// that order. Trades with the same tier retain their relative order (stable sort).
///
/// If the fairness fallback is active (consecutive priority-only batches >=
/// `fairness_batch_window`), standard-follower trades are interleaved so they
/// are not skipped.
pub fn sort_trades_by_priority(
    env: &Env,
    trades: Vec<crate::BatchTradeInput>,
    portfolio: Option<&Address>,
) -> (Vec<crate::BatchTradeInput>, u32) {
    let config = get_priority_config(env);
    let mut counter = get_priority_batch_counter(env);

    // Classify each trade
    let mut scored: Vec<PrioritizedTrade> = Vec::new();
    for i in 0..trades.len() {
        let t = trades.get(i).unwrap();
        let tier = get_follower_priority_tier(env, &t.user, portfolio);
        scored.push(PrioritizedTrade {
            tier,
            user: t.user,
            token: t.token,
            amount: t.amount,
        });
    }

    // Sort by tier descending (higher priority first), stable
    scored.sort_by(|a, b| b.tier.cmp(&a.tier));

    // Check if all trades in this batch are priority
    let all_priority = scored
        .iter()
        .all(|t| t.tier > FollowerPriorityTier::Standard);

    if all_priority {
        counter += 1;
    } else {
        counter = 0; // standard trades reset the counter
    }

    // If fairness fallback is triggered, re-interleave standard trades
    if counter >= config.fairness_batch_window {
        let mut priority_count = 0u32;
        let mut standard_idx = None;
        for (i, t) in scored.iter().enumerate() {
            if t.tier > FollowerPriorityTier::Standard {
                priority_count += 1;
            } else if standard_idx.is_none() {
                standard_idx = Some(i);
            }
        }

        // If there are standard trades, swap one into an earlier position
        if let Some(idx) = standard_idx {
            if idx > 0 && priority_count > 0 {
                let insert_at = (priority_count.saturating_sub(1)).min(idx as u32) as usize;
                let standard_trade = scored.remove(idx);
                scored.insert(insert_at, standard_trade);
            }
        }

        // Reset counter after applying the fallback
        counter = 0;
    }

    set_priority_batch_counter(env, counter);

    // Convert back to BatchTradeInput
    let mut result: Vec<crate::BatchTradeInput> = Vec::new(env);
    for pt in scored {
        result.push_back(crate::BatchTradeInput {
            user: pt.user,
            token: pt.token,
            amount: pt.amount,

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BatchTradeInput;
    use soroban_sdk::{
        contract, contractimpl, contracttype,
        testutils::Address as _,
        Address, Env,
    };

    #[contract]
    struct MockPortfolio;

    #[contracttype]
    #[derive(Clone)]
    enum MockKey {
        Stake(Address),
        AccountAge(Address),
    }

    #[contractimpl]
    impl MockPortfolio {
        pub fn get_stake(env: Env, user: Address) -> i128 {
            env.storage()
                .instance()
                .get(&MockKey::Stake(user))
                .unwrap_or(0)
        }

        pub fn get_account_age(env: Env, user: Address) -> u64 {
            env.storage()
                .instance()
                .get(&MockKey::AccountAge(user))
                .unwrap_or(0)
        }

        pub fn set_stake(env: Env, user: Address, amount: i128) {
            env.storage()
                .instance()
                .set(&MockKey::Stake(user), &amount);
        }

        pub fn set_account_age(env: Env, user: Address, age: u64) {
            env.storage()
                .instance()
                .set(&MockKey::AccountAge(user), &age);
        }
    }

    #[contract]
    struct TestContract;

    fn setup_env() -> (Env, Address) {
        let env = Env::default();
        let cid = env.register(TestContract, ());
        (env, cid)
    }

    fn make_trade_input(env: &Env, user: Address, amount: i128) -> BatchTradeInput {
        let token = Address::generate(env);
        BatchTradeInput { user, token, amount }
    }

    // --- Priority config ---

    #[test]
    fn default_config_is_reasonable() {
        let config = PriorityConfig::default();
        assert_eq!(config.high_stake_min, DEFAULT_HIGH_STAKE_MIN);
        assert_eq!(config.high_tenure_min_secs, DEFAULT_HIGH_TENURE_MIN_SECS);
        assert_eq!(config.fairness_batch_window, DEFAULT_FAIRNESS_BATCH_WINDOW);
    }

    #[test]
    fn set_and_get_config_roundtrip() {
        let (env, cid) = setup_env();
        env.as_contract(&cid, || {
            let config = PriorityConfig {
                high_stake_min: 500_000_000,
                high_tenure_min_secs: 30 * 24 * 60 * 60,
                fairness_batch_window: 5,
            };
            set_priority_config(&env, &config);
            let stored = get_priority_config(&env);
            assert_eq!(stored.high_stake_min, 500_000_000);
            assert_eq!(stored.high_tenure_min_secs, 30 * 24 * 60 * 60);
            assert_eq!(stored.fairness_batch_window, 5);
        });
    }

    // --- Tier determination ---

    #[test]
    fn default_tier_is_standard() {
        let (env, cid) = setup_env();
        let user = Address::generate(&env);
        env.as_contract(&cid, || {
            set_priority_config(&env, &PriorityConfig::default());
            let tier = get_follower_priority_tier(&env, &user, None);
            assert_eq!(tier, FollowerPriorityTier::Standard);
        });
    }

    #[test]
    fn high_stake_tier_above_threshold() {
        let (env, cid) = setup_env();
        let user = Address::generate(&env);
        let portfolio_id = env.register(MockPortfolio, ());
        let portfolio = MockPortfolioClient::new(&env, &portfolio_id);
        portfolio.set_stake(&user, &DEFAULT_HIGH_STAKE_MIN);

        env.as_contract(&cid, || {
            set_priority_config(&env, &PriorityConfig::default());
            let tier = get_follower_priority_tier(&env, &user, Some(&portfolio_id));
    // --- Batch sorting ---

    #[test]
    fn batch_sorted_by_priority() {
        let (env, cid) = setup_env();
        let portfolio_id = env.register(MockPortfolio, ());
        let portfolio = MockPortfolioClient::new(&env, &portfolio_id);
        let high_stake_user = Address::generate(&env);
        let standard_user = Address::generate(&env);
        portfolio.set_stake(&high_stake_user, &DEFAULT_HIGH_STAKE_MIN);

        let mut trades: Vec<BatchTradeInput> = Vec::new(&env);
        trades.push_back(make_trade_input(&env, standard_user.clone(), 1000));
        trades.push_back(make_trade_input(&env, high_stake_user.clone(), 2000));

        env.as_contract(&cid, || {
            set_priority_config(&env, &PriorityConfig::default());
            let (sorted, _) = sort_trades_by_priority(&env, trades, Some(&portfolio_id));
            assert_eq!(sorted.len(), 2);
            assert_eq!(sorted.get(0).unwrap().user, high_stake_user);
            assert_eq!(sorted.get(1).unwrap().user, standard_user);
        });
    }

    #[test]
    fn standard_users_keep_relative_order() {
        let (env, cid) = setup_env();
        let portfolio_id = env.register(MockPortfolio, ());
        let portfolio = MockPortfolioClient::new(&env, &portfolio_id);
        let high_user = Address::generate(&env);
        let std_user1 = Address::generate(&env);
        let std_user2 = Address::generate(&env);
        portfolio.set_stake(&high_user, &DEFAULT_HIGH_STAKE_MIN);

        let mut trades: Vec<BatchTradeInput> = Vec::new(&env);
        trades.push_back(make_trade_input(&env, std_user1.clone(), 100));
        trades.push_back(make_trade_input(&env, high_user.clone(), 200));
        trades.push_back(make_trade_input(&env, std_user2.clone(), 300));

        env.as_contract(&cid, || {
            set_priority_config(&env, &PriorityConfig::default());
            let (sorted, _) = sort_trades_by_priority(&env, trades, Some(&portfolio_id));
            assert_eq!(sorted.len(), 3);
            assert_eq!(sorted.get(0).unwrap().user, high_user);
            assert_eq!(sorted.get(1).unwrap().user, std_user1);
            assert_eq!(sorted.get(2).unwrap().user, std_user2);
        });
    }

    // --- Fairness fallback ---

    #[test]
    fn batch_counter_resets_with_standard_trades() {
        let (env, cid) = setup_env();
        let std_user = Address::generate(&env);
        env.as_contract(&cid, || {
            set_priority_config(&env, &PriorityConfig::default());
            set_priority_batch_counter(&env, 1);
            let mut trades: Vec<BatchTradeInput> = Vec::new(&env);
            trades.push_back(make_trade_input(&env, std_user.clone(), 100));
            let (_, new_counter) = sort_trades_by_priority(&env, trades, None);
            assert_eq!(new_counter, 0);
        });
    }

    #[test]
    fn batch_counter_increments_with_all_priority() {
        let (env, cid) = setup_env();
        let portfolio_id = env.register(MockPortfolio, ());
        let portfolio = MockPortfolioClient::new(&env, &portfolio_id);
        let high_user = Address::generate(&env);
        portfolio.set_stake(&high_user, &DEFAULT_HIGH_STAKE_MIN);

        env.as_contract(&cid, || {
            set_priority_config(&env, &PriorityConfig::default());
            set_priority_batch_counter(&env, 0);
            let mut trades: Vec<BatchTradeInput> = Vec::new(&env);
            trades.push_back(make_trade_input(&env, high_user.clone(), 100));
            let (_, new_counter) = sort_trades_by_priority(&env, trades, Some(&portfolio_id));
            assert_eq!(new_counter, 1);
        });
    }

    #[test]
    fn no_portfolio_falls_back_to_standard() {
        let (env, cid) = setup_env();
        let user = Address::generate(&env);
        env.as_contract(&cid, || {
            set_priority_config(&env, &PriorityConfig::default());
            let tier = get_follower_priority_tier(&env, &user, None);
            assert_eq!(tier, FollowerPriorityTier::Standard);
        });
    }
}

            assert_eq!(tier, FollowerPriorityTier::HighStake);
        });
    }

    #[test]
    fn high_tenure_tier_above_threshold() {
        let (env, cid) = setup_env();
        let user = Address::generate(&env);
        let portfolio_id = env.register(MockPortfolio, ());
        let portfolio = MockPortfolioClient::new(&env, &portfolio_id);
        portfolio.set_account_age(&user, &DEFAULT_HIGH_TENURE_MIN_SECS);

        env.as_contract(&cid, || {
            set_priority_config(&env, &PriorityConfig::default());
            let tier = get_follower_priority_tier(&env, &user, Some(&portfolio_id));
            assert_eq!(tier, FollowerPriorityTier::HighTenure);
        });
    }

    #[test]
    fn high_stake_trumps_high_tenure() {
        let (env, cid) = setup_env();
        let user = Address::generate(&env);
        let portfolio_id = env.register(MockPortfolio, ());
        let portfolio = MockPortfolioClient::new(&env, &portfolio_id);
        portfolio.set_stake(&user, &DEFAULT_HIGH_STAKE_MIN);
        portfolio.set_account_age(&user, &DEFAULT_HIGH_TENURE_MIN_SECS);

        env.as_contract(&cid, || {
            set_priority_config(&env, &PriorityConfig::default());
            let tier = get_follower_priority_tier(&env, &user, Some(&portfolio_id));
            assert_eq!(tier, FollowerPriorityTier::HighStake);
        });
    }
}

        });
    }

    (result, counter)
}


}

/// Read the consecutive priority-only batch counter.
pub fn get_priority_batch_counter(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&StorageKey::PriorityBatchCounter)
        .unwrap_or(0)
}

/// Set the consecutive priority-only batch counter.
pub fn set_priority_batch_counter(env: &Env, count: u32) {
    env.storage()
        .instance()
        .set(&StorageKey::PriorityBatchCounter, &count);
}

            high_tenure_min_secs: DEFAULT_HIGH_TENURE_MIN_SECS,
            fairness_batch_window: DEFAULT_FAIRNESS_BATCH_WINDOW,
        }
    }
}
