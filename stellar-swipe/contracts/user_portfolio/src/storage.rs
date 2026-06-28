use soroban_sdk::{contracttype, Address};

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Initialized,
    Admin,
    Oracle,
    OracleAssetPair,
    NextPositionId,
    Position(u64),
    /// V1: mixed open+closed list (preserved for migration reads).
    UserPositions(Address),
    UserOpenPositions(Address),
    UserClosedPositions(Address),
    /// Registered TradeExecutor contract allowed to call `close_position_keeper`.
    TradeExecutor,
    /// Per-user KYC verification flag (bool). No PII stored — boolean only.
    KycVerified(Address),
    /// Global KYC-required mode (bool). When true, only KYC-verified users can trade.
    KycRequiredMode,
    /// Per-user geographic restriction flag (bool). Restricted users cannot trade.
    Restricted(Address),
    /// Per-user current streak (consecutive profitable closes)
    CurrentStreak(Address),
    /// Per-user best streak observed
    BestStreak(Address),
    /// Migration: marks a user as already migrated from V1 to V2 layout.
    MigratedUser(Address),
    /// Migration: queue of users pending V1→V2 migration.
    MigrationQueue,
    /// Per-user notification preferences (Issue #430).
    NotificationPrefs(Address),
    /// Per-user achievement list (Issue #432).
    UserAchievements(Address),
    /// Anchor deposit destination address by token.
    AnchorDepositAddress(Address),
    // Badge-related keys used by badges.rs
    UserBadges(Address),
    UserClosedTradeCount(Address),
    UserProfitStreak(Address),
    LeaderboardRank(Address),
    EarlyAdopterCap,
    TotalUsersFirstOpen,
    /// Per-user trading style profile for personalized signal recommendations.
    TradingStyle(Address),
    /// Configured SignalRegistry contract address used by recommendation queries.
    SignalRegistry,
    /// Per-user signal watchlist (Issue: signal watchlist).
    Watchlist(Address),
    UserOnboardingStatus(Address),
    UserOnboardingMilestone(Address),
    /// Per-user custom string tags on positions (Issue #703).
    /// Maps (user, position_id) -> tag string.
    PositionTag(Address, u64),
    /// Per-user map of tag -> Vec<position_id> for reverse lookup (Issue #703).
    /// Maps (user, tag_string_hash) -> Vec<position_id>.
    /// Tags are bounded to a reasonable length to prevent spam.
    UserPositionsByTag(Address, soroban_sdk::String),
    /// Ordered list of snapshot timestamps for a user (issue #685).
    UserSnapshotTimestamps(Address),
    /// Portfolio value recorded at a specific timestamp for a user (issue #685).
    /// Maps (user, timestamp) -> total portfolio value (i128).
    PortfolioSnapshotEntry(Address, u64),
}
