//! Argo lending program types: the instruction set, account layouts, market and
//! oracle ids, and PDA seeds. Shared by the guest, the program logic, the
//! harness and (from M5) the client core.
//!
//! M1 changes to the reviewed M0 spec are listed in `docs/m1/decisions.md`
//! (cited here as D1..D16).

use borsh::{BorshDeserialize, BorshSerialize};
use lee_core::account::AccountId;
use lee_core::program::{PdaSeed, ProgramId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub mod external;

/// Instruction set. Encoded with risc0 serde by the client and decoded by the
/// SPEL dispatcher; variant and field names must match the guest handlers.
///
/// Amount convention (Morpho Blue): for `supply`, `withdraw`, `borrow` and
/// `repay` exactly one of `assets` and `shares` is non-zero.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    /// Claim the singleton `Config` PDA. Accounts: `config`, `admin` (signer).
    Initialize,
    /// Admin: add an LLTV (WAD-scaled, `< WAD`) to the enabled set.
    /// Accounts: `config`, `admin` (signer).
    EnableLltv { lltv: u128 },
    /// Admin: add an IRM program id to the enabled set.
    /// Accounts: `config`, `admin` (signer).
    EnableIrm { irm: ProgramId },
    /// Permissionless. Claims Market, both vaults and the dead-deposit Position,
    /// and moves `DEAD_DEPOSIT` loan units from the creator (D11).
    /// Accounts: `config`, `market`, `loan_vault`, `collateral_vault`,
    /// `dead_position`, `loan_definition`, `collateral_definition`,
    /// `price_account`, `clock`, `creator_loan_holding` (authorised).
    CreateMarket {
        params: MarketParams,
        oracle: OracleDescriptor,
    },
    /// Accounts: `market`, `position`, `loan_vault`, `payer_holding`
    /// (authorised), `clock`.
    Supply {
        assets: u128,
        shares: u128,
        on_behalf: AccountId,
    },
    /// Accounts: `market`, `position`, `loan_vault`, `receiver_holding`,
    /// `caller` (authorised, `== on_behalf` in M1), `clock`.
    Withdraw {
        assets: u128,
        shares: u128,
        on_behalf: AccountId,
    },
    /// Accounts: `market`, `position`, `collateral_vault`, `payer_holding`
    /// (authorised).
    SupplyCollateral { assets: u128, on_behalf: AccountId },
    /// Accounts: `market`, `position`, `collateral_vault`,
    /// `receiver_holding`, `caller` (authorised, `== on_behalf` in M1),
    /// `price_account`, `clock`.
    WithdrawCollateral { assets: u128, on_behalf: AccountId },
    /// Accounts: `market`, `position`, `loan_vault`, `receiver_holding`,
    /// `caller` (authorised, `== on_behalf` in M1), `price_account`, `clock`.
    Borrow {
        assets: u128,
        shares: u128,
        on_behalf: AccountId,
    },
    /// Accounts: `market`, `position`, `loan_vault`, `payer_holding`
    /// (authorised), `clock`.
    Repay {
        assets: u128,
        shares: u128,
        on_behalf: AccountId,
    },
}

/// Singleton configuration account.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Admin authority (RFP-001 pattern is wired in M2).
    pub admin: AccountId,
    /// Protocol fee recipient; `None` until set.
    pub fee_recipient: Option<AccountId>,
    /// Enabled LLTV values (WAD-scaled), at most `MAX_ENABLED_LLTV`.
    pub enabled_lltv: Vec<u128>,
    /// Enabled IRM program ids, at most `MAX_ENABLED_IRM`.
    pub enabled_irm: Vec<ProgramId>,
}

/// The five parameters that identify a market (RFP-008 F3). The market id is
/// `SHA-256(MARKET_DOMAIN ‖ borsh(MarketParams))`, a 158-byte preimage.
#[derive(
    Serialize, Deserialize, BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq, Eq,
)]
pub struct MarketParams {
    /// Loan token definition account id.
    pub loan_token: AccountId,
    /// Collateral token definition account id.
    pub collateral_token: AccountId,
    /// `oracle_digest(&OracleDescriptor)` (D2).
    pub oracle: [u8; 32],
    /// IRM program id (D9).
    pub irm: ProgramId,
    /// Liquidation LTV, WAD-scaled.
    pub lltv: u128,
}

/// What a market's `oracle` parameter commits to (D2). Shown in full by the GUI
/// and CLI so a lender can see which feed and staleness bound a market uses.
#[derive(
    Serialize, Deserialize, BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq, Eq,
)]
pub struct OracleDescriptor {
    /// Descriptor format version; `ORACLE_DESCRIPTOR_VERSION` in M1.
    pub version: u8,
    /// Program that must own the price account.
    pub oracle_program: ProgramId,
    /// The exact `OraclePriceAccount` the market reads.
    pub price_account: AccountId,
    /// Maximum price age in milliseconds, in `[MIN_MAX_AGE_MS, MAX_MAX_AGE_MS]` (D5).
    pub max_age_ms: u64,
}

/// Per-market state.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
pub struct Market {
    /// The market's identifying parameters.
    pub params: MarketParams,
    /// The preimage of `params.oracle`.
    pub oracle: OracleDescriptor,
    /// Total supplied loan assets, including accrued interest.
    pub total_supply_assets: u128,
    /// Total supply shares.
    pub total_supply_shares: u128,
    /// Total borrowed loan assets, including accrued interest.
    pub total_borrow_assets: u128,
    /// Total borrow shares.
    pub total_borrow_shares: u128,
    /// Last accrual time, Unix milliseconds from `CLOCK_01` (D8).
    pub last_update_ms: u64,
    /// Protocol fee fraction, WAD-scaled; zero in M1 (D10).
    pub fee: u128,
    /// Fee shares accrued but not yet claimed; zero in M1 (D10).
    pub pending_fee_shares: u128,
    /// AdaptiveCurveIRM state, WAD per second; zero until M2.
    pub rate_at_target: u128,
}

/// A user's position in one market.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct Position {
    /// Supply shares.
    pub supply_shares: u128,
    /// Borrow shares.
    pub borrow_shares: u128,
    /// Collateral, in raw collateral-token units.
    pub collateral: u128,
}

/// The only account that may sign `Initialize` and become admin (D14). The
/// image is immutable, so this is fixed at build time. M1 value: the id of the
/// public test key `[1; 32]` used by the tests and the localnet harness. It must
/// be replaced with a real key (or the RFP-001 nominate/accept flow, M2)
/// before any public deployment.
pub const ADMIN_GENESIS: AccountId = AccountId::new([
    148, 179, 206, 253, 199, 51, 82, 86, 232, 2, 152, 122, 80, 243, 54, 207, 237, 112, 83, 153, 44,
    59, 204, 49, 128, 84, 160, 227, 216, 149, 97, 102,
]);

/// Domain prefix of the market-id preimage (14 raw ASCII bytes, no length prefix).
pub const MARKET_DOMAIN: &[u8] = b"argo/market/v1";
/// Domain prefix of the oracle-digest preimage.
pub const ORACLE_DOMAIN: &[u8] = b"argo/oracle/v1";
/// The only descriptor version M1 accepts.
pub const ORACLE_DESCRIPTOR_VERSION: u8 = 1;
/// Smallest allowed `max_age_ms` (D5): a sanity bound of about one testnet-legacy
/// block interval. At this value a `CLOCK_01`-stamped price is fresh only in the
/// block it was written; usable values follow the rule in `docs/m1/decisions.md`.
pub const MIN_MAX_AGE_MS: u64 = 60_000;
/// Largest allowed `max_age_ms`: one day (D5).
pub const MAX_MAX_AGE_MS: u64 = 86_400_000;
/// How far ahead of `CLOCK_01` a price timestamp may be (D5a). Kanon accepts
/// RedStone packages dated up to 180 s ahead of the clock (Kanon ADR 18); a later
/// timestamp is rejected, so a future-dated price cannot stay fresh long past
/// `max_age_ms`.
pub const MAX_FUTURE_SKEW_MS: u64 = 180_000;
/// Loan units the market creator deposits into a Position nobody can withdraw (D11).
pub const DEAD_DEPOSIT: u128 = 1_000;
/// Bound on `Config::enabled_lltv`.
pub const MAX_ENABLED_LLTV: usize = 32;
/// Bound on `Config::enabled_irm`.
pub const MAX_ENABLED_IRM: usize = 8;

fn sha256(parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p);
    }
    h.finalize().into()
}

/// `SHA-256(ORACLE_DOMAIN ‖ borsh(descriptor))` (D2).
#[must_use]
pub fn oracle_digest(descriptor: &OracleDescriptor) -> [u8; 32] {
    let enc = borsh::to_vec(descriptor).expect("OracleDescriptor serialises");
    sha256(&[ORACLE_DOMAIN, &enc])
}

/// `SHA-256(MARKET_DOMAIN ‖ borsh(params))` (spec §4).
#[must_use]
pub fn market_id(params: &MarketParams) -> [u8; 32] {
    let enc = borsh::to_vec(params).expect("MarketParams serialises");
    sha256(&[MARKET_DOMAIN, &enc])
}

#[allow(
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    reason = "i < s.len() && i < 32 is the loop condition, so both indexes and the increment are in bounds"
)]
const fn padded(s: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    let mut i = 0;
    while i < s.len() && i < 32 {
        out[i] = s[i];
        i += 1;
    }
    out
}

/// Seed of the `Config` PDA (zero-padded, not hashed, as M0 ships it).
pub const CONFIG_SEED: PdaSeed = PdaSeed::new(padded(b"config"));

/// Seed of `Market(market_id)`: `SHA-256("market" ‖ market_id)`.
#[must_use]
pub fn market_seed(market_id: &[u8; 32]) -> PdaSeed {
    PdaSeed::new(sha256(&[b"market", market_id]))
}

/// Seed of `Position(market_id, owner)`.
#[must_use]
pub fn position_seed(market_id: &[u8; 32], owner: &AccountId) -> PdaSeed {
    PdaSeed::new(sha256(&[b"position", market_id, owner.value()]))
}

/// Seed of `LoanVault(market_id)`.
#[must_use]
pub fn loan_vault_seed(market_id: &[u8; 32]) -> PdaSeed {
    PdaSeed::new(sha256(&[b"loan-vault", market_id]))
}

/// Seed of `CollateralVault(market_id)`.
#[must_use]
pub fn collateral_vault_seed(market_id: &[u8; 32]) -> PdaSeed {
    PdaSeed::new(sha256(&[b"coll-vault", market_id]))
}

/// Owner of the dead-deposit Position: an id derived from the market that no
/// key and no program can authorise (D11).
#[must_use]
pub fn dead_owner(market_id: &[u8; 32]) -> AccountId {
    AccountId::new(sha256(&[b"argo/dead-owner", market_id]))
}

/// Derive the `Config` PDA id.
#[must_use]
pub fn config_id(program_id: &ProgramId) -> AccountId {
    AccountId::for_public_pda(program_id, &CONFIG_SEED)
}

/// Derive the `Market` PDA id.
#[must_use]
pub fn market_account_id(program_id: &ProgramId, market_id: &[u8; 32]) -> AccountId {
    AccountId::for_public_pda(program_id, &market_seed(market_id))
}

/// Derive the `Position` PDA id.
#[must_use]
pub fn position_id(program_id: &ProgramId, market_id: &[u8; 32], owner: &AccountId) -> AccountId {
    AccountId::for_public_pda(program_id, &position_seed(market_id, owner))
}

/// Derive the `LoanVault` PDA id.
#[must_use]
pub fn loan_vault_id(program_id: &ProgramId, market_id: &[u8; 32]) -> AccountId {
    AccountId::for_public_pda(program_id, &loan_vault_seed(market_id))
}

/// Derive the `CollateralVault` PDA id.
#[must_use]
pub fn collateral_vault_id(program_id: &ProgramId, market_id: &[u8; 32]) -> AccountId {
    AccountId::for_public_pda(program_id, &collateral_vault_seed(market_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> MarketParams {
        MarketParams {
            loan_token: AccountId::new([1; 32]),
            collateral_token: AccountId::new([2; 32]),
            oracle: [3; 32],
            irm: [4; 8],
            lltv: 860_000_000_000_000_000,
        }
    }

    #[test]
    fn market_id_preimage_is_158_bytes() {
        let enc = borsh::to_vec(&params()).expect("serialises");
        assert_eq!(MARKET_DOMAIN.len().checked_add(enc.len()), Some(158));
    }

    #[test]
    fn market_id_changes_with_every_param() {
        let base = market_id(&params());
        let mut p = params();
        p.lltv = 1;
        assert_ne!(market_id(&p), base);
        let mut p = params();
        p.oracle = [9; 32];
        assert_ne!(market_id(&p), base);
        let mut p = params();
        p.irm = [5; 8];
        assert_ne!(market_id(&p), base);
        let mut p = params();
        p.loan_token = AccountId::new([6; 32]);
        assert_ne!(market_id(&p), base);
        let mut p = params();
        p.collateral_token = AccountId::new([6; 32]);
        assert_ne!(market_id(&p), base);
    }

    #[test]
    fn oracle_digest_binds_account_and_max_age() {
        let d = OracleDescriptor {
            version: ORACLE_DESCRIPTOR_VERSION,
            oracle_program: [7; 8],
            price_account: AccountId::new([8; 32]),
            max_age_ms: 60_000,
        };
        let base = oracle_digest(&d);
        assert_ne!(
            oracle_digest(&OracleDescriptor {
                max_age_ms: 60_001,
                ..d
            }),
            base
        );
        assert_ne!(
            oracle_digest(&OracleDescriptor {
                price_account: AccountId::new([9; 32]),
                ..d
            }),
            base
        );
        assert_ne!(
            oracle_digest(&OracleDescriptor {
                oracle_program: [6; 8],
                ..d
            }),
            base
        );
    }

    #[test]
    fn seeds_are_domain_separated() {
        let m = [1u8; 32];
        assert_ne!(loan_vault_seed(&m), collateral_vault_seed(&m));
        assert_ne!(market_seed(&m), loan_vault_seed(&m));
    }
}
