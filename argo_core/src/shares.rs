//! Virtual-share conversions (Morpho Blue semantics): round down for supply,
//! up for borrow.

use crate::math::{mul_div_down, mul_div_up};
use crate::{VIRTUAL_ASSETS, VIRTUAL_SHARES};

/// Shares minted for `assets`, rounding down.
pub fn to_shares_down(assets: u128, total_assets: u128, total_shares: u128) -> Option<u128> {
    mul_div_down(
        assets,
        total_shares.checked_add(VIRTUAL_SHARES)?,
        total_assets.checked_add(VIRTUAL_ASSETS)?,
    )
}

/// Assets for `shares`, rounding down.
pub fn to_assets_down(shares: u128, total_assets: u128, total_shares: u128) -> Option<u128> {
    mul_div_down(
        shares,
        total_assets.checked_add(VIRTUAL_ASSETS)?,
        total_shares.checked_add(VIRTUAL_SHARES)?,
    )
}

/// Shares for `assets`, rounding up.
pub fn to_shares_up(assets: u128, total_assets: u128, total_shares: u128) -> Option<u128> {
    mul_div_up(
        assets,
        total_shares.checked_add(VIRTUAL_SHARES)?,
        total_assets.checked_add(VIRTUAL_ASSETS)?,
    )
}

/// Assets for `shares`, rounding up.
pub fn to_assets_up(shares: u128, total_assets: u128, total_shares: u128) -> Option<u128> {
    mul_div_up(
        shares,
        total_assets.checked_add(VIRTUAL_ASSETS)?,
        total_shares.checked_add(VIRTUAL_SHARES)?,
    )
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    reason = "test inputs are bounded far below u128::MAX"
)]
mod tests {
    use super::*;

    #[test]
    fn empty_market_first_deposit_is_not_100_percent() {
        // 1 wei into an empty market yields VIRTUAL_SHARES-scaled shares, so a
        // later donation cannot make the first depositor own the market.
        let s = to_shares_down(1, 0, 0);
        assert_eq!(s, Some(VIRTUAL_SHARES));
    }

    use proptest::prelude::*;

    proptest! {
        /// Up and down differ by at most one, and up is never below down.
        #[test]
        fn up_is_down_or_down_plus_one(a in 0u128..1u128 << 96, ta in 0u128..1u128 << 96, ts in 0u128..1u128 << 96) {
            let (sd, su) = (to_shares_down(a, ta, ts), to_shares_up(a, ta, ts));
            if let (Some(d), Some(u)) = (sd, su) {
                prop_assert!(u == d || u == d + 1);
            }
            let (ad, au) = (to_assets_down(a, ta, ts), to_assets_up(a, ta, ts));
            if let (Some(d), Some(u)) = (ad, au) {
                prop_assert!(u == d || u == d + 1);
            }
        }

        /// Borrowing `a` mints shares rounded up; repaying those shares costs at
        /// least `a`. Supplying `a` mints shares rounded down; redeeming them pays
        /// at most `a`.
        #[test]
        fn round_trips_favour_the_protocol(a in 1u128..1u128 << 80, ta in 0u128..1u128 << 80, ts in 0u128..1u128 << 100) {
            if let Some(s) = to_shares_up(a, ta, ts) {
                let (ta2, ts2) = (ta + a, ts + s);
                if let Some(cost) = to_assets_up(s, ta2, ts2) {
                    prop_assert!(cost >= a);
                }
            }
            if let Some(s) = to_shares_down(a, ta, ts) {
                let (ta2, ts2) = (ta + a, ts + s);
                if let Some(paid) = to_assets_down(s, ta2, ts2) {
                    prop_assert!(paid <= a);
                }
            }
        }
    }

    #[test]
    fn non_exact_ratio_pins_floor_and_ceil() {
        // 1 asset into (1_000, 1e9 + 1): 1 × 1_001_000_001 / 1_001.
        assert_eq!(to_shares_down(1, 1_000, 1_000_000_001), Some(1_000_000));
        assert_eq!(to_shares_up(1, 1_000, 1_000_000_001), Some(1_000_001));
        // 1 share of (1_001, 1_000_000_001): 1_002 / 1_001_000_001.
        assert_eq!(to_assets_down(1, 1_001, 1_000_000_001), Some(0));
        assert_eq!(to_assets_up(1, 1_001, 1_000_000_001), Some(1));
    }

    #[test]
    fn round_trip_never_gains() {
        let (ta, ts) = (1_000_000_000u128, 999_000_000u128);
        for a in [1u128, 7, 1_000, 123_456_789] {
            let s = to_shares_down(a, ta, ts).expect("fits");
            let back = to_assets_down(s, ta, ts).expect("fits");
            assert!(back <= a);
        }
    }
}
