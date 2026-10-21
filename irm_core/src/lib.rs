//! AdaptiveCurveIRM constants. The rate function itself is M2 work; M0 only
//! fixes the constants so spike S-D can link a realistically-shaped crate.
#![no_std]
#![deny(missing_docs)]

use argo_core::WAD;

/// Seconds per year used by the curve.
pub const SECONDS_PER_YEAR: u128 = 365 * 24 * 60 * 60;
/// Target utilisation (90%).
pub const TARGET_UTILIZATION: u128 = 900_000_000_000_000_000;
/// Curve steepness (4).
pub const CURVE_STEEPNESS: u128 = 4 * WAD;
/// Adjustment speed: 50 per year, expressed per second in WAD.
pub const ADJUSTMENT_SPEED: u128 = 50 * WAD / SECONDS_PER_YEAR;
/// Initial rate at target: 4% per year, per second in WAD.
pub const INITIAL_RATE_AT_TARGET: u128 = 4 * WAD / 100 / SECONDS_PER_YEAR;
/// Minimum rate at target: 0.1% per year, per second in WAD.
pub const MIN_RATE_AT_TARGET: u128 = WAD / 1000 / SECONDS_PER_YEAR;
/// Maximum rate at target: 200% per year, per second in WAD.
pub const MAX_RATE_AT_TARGET: u128 = 2 * WAD / SECONDS_PER_YEAR;

/// Utilisation = borrow / supply in WAD; zero supply yields zero.
pub fn utilization(total_borrow_assets: u128, total_supply_assets: u128) -> Option<u128> {
    if total_supply_assets == 0 {
        return Some(0);
    }
    argo_core::math::w_div_down(total_borrow_assets, total_supply_assets)
}

#[cfg(test)]
#[allow(
    clippy::assertions_on_constants,
    reason = "the point is to pin the constant ordering"
)]
mod tests {
    use super::*;

    #[test]
    fn bounds_are_ordered() {
        assert!(MIN_RATE_AT_TARGET < INITIAL_RATE_AT_TARGET);
        assert!(INITIAL_RATE_AT_TARGET < MAX_RATE_AT_TARGET);
    }

    #[test]
    fn utilization_edges() {
        assert_eq!(utilization(0, 0), Some(0));
        assert_eq!(utilization(50, 100), Some(WAD / 2));
        assert_eq!(utilization(100, 100), Some(WAD));
    }
}
