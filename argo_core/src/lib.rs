//! Argo core: pure, deterministic math shared by the guest, the client core,
//! and the formal-verification harness. `no_std`, no allocation, no I/O.
//!
//! M0 ships constants and the share-conversion primitives only; instruction
//! and account layouts land in M1 once the state-layout spec is accepted.
#![no_std]
#![deny(missing_docs)]

/// Fixed-point scale for rates, LLTV, and fee fractions.
pub const WAD: u128 = 1_000_000_000_000_000_000;
/// Virtual shares added to every share/asset conversion (inflation resistance).
pub const VIRTUAL_SHARES: u128 = 1_000_000;
/// Virtual assets added to every share/asset conversion.
pub const VIRTUAL_ASSETS: u128 = 1;
/// Oracle price scale: price of one collateral unit in loan units, times 1e36.
pub const ORACLE_PRICE_SCALE: u128 = 1_000_000_000_000_000_000_000_000_000_000_000_000;
/// Maximum protocol fee fraction of accrued interest (25%).
pub const MAX_FEE: u128 = WAD / 4;
/// Liquidation incentive cursor (beta = 0.3).
pub const LIQUIDATION_CURSOR: u128 = 300_000_000_000_000_000;
/// Maximum liquidation incentive factor (1.15).
pub const MAX_LIQUIDATION_INCENTIVE_FACTOR: u128 = 1_150_000_000_000_000_000;

pub mod math;
pub mod shares;
