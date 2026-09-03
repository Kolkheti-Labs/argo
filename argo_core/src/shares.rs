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
mod tests {
    use super::*;

    #[test]
    fn empty_market_first_deposit_is_not_100_percent() {
        // 1 wei into an empty market yields VIRTUAL_SHARES-scaled shares, so a
        // later donation cannot make the first depositor own the market.
        let s = to_shares_down(1, 0, 0);
        assert_eq!(s, Some(VIRTUAL_SHARES));
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
