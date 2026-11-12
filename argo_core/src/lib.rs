//! Argo core: pure, deterministic math shared by the guest, the client core,
//! and the formal-verification harness. `no_std`, no allocation, no I/O.
//!
//! Constants, widening mul/div, virtual-share conversions and the health check.
//! Account layouts and instructions live in `argo_lending_core`.
#![no_std]
#![deny(missing_docs)]

/// Fixed-point scale for rates, LLTV, and fee fractions.
pub const WAD: u128 = 1_000_000_000_000_000_000;
/// Virtual shares added to every share/asset conversion (inflation resistance).
pub const VIRTUAL_SHARES: u128 = 1_000_000;
/// Virtual assets added to every share/asset conversion.
pub const VIRTUAL_ASSETS: u128 = 1;
/// Fractional bits of an oracle price: Q64.64, raw loan-token units per raw
/// collateral-token unit (`docs/m1/decisions.md` D3).
pub const PRICE_FRACTIONAL_BITS: u32 = 64;
/// `2^PRICE_FRACTIONAL_BITS`, the divisor that turns `amount × price` into loan units.
pub const PRICE_ONE: u128 = 1 << PRICE_FRACTIONAL_BITS;
/// Maximum protocol fee fraction of accrued interest (25%).
pub const MAX_FEE: u128 = WAD / 4;
/// Liquidation incentive cursor (beta = 0.3).
pub const LIQUIDATION_CURSOR: u128 = 300_000_000_000_000_000;
/// Maximum liquidation incentive factor (1.15).
pub const MAX_LIQUIDATION_INCENTIVE_FACTOR: u128 = 1_150_000_000_000_000_000;

pub mod health;
pub mod math;
pub mod shares;
