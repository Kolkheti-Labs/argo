//! Argo lending program logic. Every handler takes the pre-states it was given
//! and returns `(post_states, chained_calls)`, one post-state per input in
//! input order; the guest is a thin SPEL dispatcher.
//!
//! Panics are the program's error channel: a panic fails the whole
//! transaction, chained calls included (M0 spike S-A).

use argo_lending_core::external::{ClockAccountData, OraclePriceAccount, CLOCK_01_ACCOUNT_ID};
use argo_lending_core::{Config, Market, MarketParams, OracleDescriptor, Position, CONFIG_SEED};
use lee_core::account::{Account, AccountWithMetadata, Data};
use lee_core::program::{
    AccountPostState, ChainedCall, Claim, PdaSeed, ProgramId, DEFAULT_PROGRAM_ID,
};

mod admin;
mod market;
mod ops;
pub mod token;

pub use admin::{enable_irm, enable_lltv};
pub use market::create_market;
pub use ops::{borrow, repay, supply, supply_collateral, withdraw, withdraw_collateral};

/// Output shape shared by every handler.
pub type Output = (Vec<AccountPostState>, Vec<ChainedCall>);

/// Claim the `Config` singleton; the signing `admin` account becomes the admin
/// authority. M2 replaces this with the RFP-001 admin-authority library.
pub fn initialize(
    config: AccountWithMetadata,
    admin: AccountWithMetadata,
    self_program_id: ProgramId,
) -> Output {
    assert_runtime_checked(&[&config]);
    assert!(admin.is_authorized, "admin must sign Initialize");
    // Deployment has no signer, so without this anyone could initialise first
    // and take admin (D14).
    assert_eq!(
        admin.account_id,
        argo_lending_core::ADMIN_GENESIS,
        "only the genesis admin may initialize"
    );
    assert_eq!(
        config.account,
        Account::default(),
        "config already initialised"
    );
    assert_eq!(
        config.account_id,
        argo_lending_core::config_id(&self_program_id),
        "config id mismatch"
    );
    // A fresh admin account is claimed so it is never dropped from a later
    // output (D15); a program-owned one is left as it is. An unowned account
    // that has already signed something cannot be claimed or checked.
    let admin_post = if admin.account == Account::default() {
        AccountPostState::new_claimed(admin.account.clone(), Claim::Authorized)
    } else {
        assert_ne!(
            admin.account.program_owner, DEFAULT_PROGRAM_ID,
            "admin account must be fresh or program-owned"
        );
        AccountPostState::new(admin.account.clone())
    };
    let cfg = Config {
        admin: admin.account_id,
        fee_recipient: None,
        enabled_lltv: vec![],
        enabled_irm: vec![],
    };
    (
        vec![
            AccountPostState::new_claimed(write(config.account, &cfg), Claim::Pda(CONFIG_SEED)),
            admin_post,
        ],
        vec![],
    )
}

// ---------------------------------------------------------------- helpers ----

/// Every input must be one the runtime checks (D15).
///
/// SPEL's generated `main` (rev 1ef0500) drops from the output each input that
/// is owned by `DEFAULT_PROGRAM_ID`, is not the default account and is not
/// claimed. LEZ v0.2.4 checks state equality and authorisation only on the
/// pre-states a program outputs (`validated_state_diff/mod.rs:143-173`). So a
/// calling program could hand Argo such an input with any content and any
/// `is_authorized`, and nothing would check it. Refuse them all.
fn assert_runtime_checked(inputs: &[&AccountWithMetadata]) {
    for acc in inputs {
        assert!(
            acc.account.program_owner != DEFAULT_PROGRAM_ID || acc.account == Account::default(),
            "input {} is unowned and not default, so the runtime cannot check it",
            acc.account_id
        );
    }
}

/// Decode a Borsh account body, panicking with `what` on failure.
fn read<T: borsh::BorshDeserialize>(acc: &AccountWithMetadata, what: &str) -> T {
    borsh::from_slice(acc.account.data.as_ref())
        .unwrap_or_else(|_| panic!("{what}: bad account data"))
}

/// Replace an account's data with `value`'s Borsh encoding.
fn write<T: borsh::BorshSerialize>(mut account: Account, value: &T) -> Account {
    let bytes = borsh::to_vec(value).expect("account state serialises");
    account.data = Data::try_from(bytes).expect("account state fits DATA_MAX_LENGTH");
    account
}

/// The Config account, checked by id and owner.
fn read_config(config: &AccountWithMetadata, self_program_id: &ProgramId) -> Config {
    assert_eq!(
        config.account_id,
        argo_lending_core::config_id(self_program_id),
        "config id mismatch"
    );
    assert_eq!(
        config.account.program_owner, *self_program_id,
        "config not owned by argo"
    );
    read(config, "config")
}

/// A Market account, checked by owner and by its id being derived from the
/// params it stores. Returns the state and its market id.
fn read_market(market: &AccountWithMetadata, self_program_id: &ProgramId) -> (Market, [u8; 32]) {
    assert_eq!(
        market.account.program_owner, *self_program_id,
        "market not owned by argo"
    );
    let m: Market = read(market, "market");
    let id = argo_lending_core::market_id(&m.params);
    assert_eq!(
        market.account_id,
        argo_lending_core::market_account_id(self_program_id, &id),
        "market id mismatch"
    );
    (m, id)
}

/// A Position for `(market_id, owner)`: decoded if Argo owns it, default if the
/// account is still unclaimed. The bool is `true` when it must be claimed.
fn read_position(
    position: &AccountWithMetadata,
    self_program_id: &ProgramId,
    market_id: &[u8; 32],
    owner: &lee_core::account::AccountId,
) -> (Position, bool) {
    assert_eq!(
        position.account_id,
        argo_lending_core::position_id(self_program_id, market_id, owner),
        "position id mismatch"
    );
    if position.account == Account::default() {
        (Position::default(), true)
    } else {
        assert_eq!(
            position.account.program_owner, *self_program_id,
            "position not owned by argo"
        );
        (read(position, "position"), false)
    }
}

/// Post-state for a Position, claiming it on first use.
fn position_post(
    position: AccountWithMetadata,
    state: &Position,
    needs_claim: bool,
    seed: PdaSeed,
) -> AccountPostState {
    let acc = write(position.account, state);
    if needs_claim {
        AccountPostState::new_claimed(acc, Claim::Pda(seed))
    } else {
        AccountPostState::new(acc)
    }
}

/// Current time from `CLOCK_01`, Unix milliseconds (D8, D13). The only place
/// Argo reads time, so a caller-passed `now` can replace it later.
fn now_ms(clock: &AccountWithMetadata) -> u64 {
    assert_eq!(
        clock.account_id, CLOCK_01_ACCOUNT_ID,
        "clock must be CLOCK_01"
    );
    let c: ClockAccountData = read(clock, "clock");
    assert_ne!(c.timestamp, 0, "clock not started");
    c.timestamp
}

/// The market's price: the exact account its descriptor names, owned by the
/// descriptor's program, quoting collateral in loan units, non-zero and at
/// most `max_age_ms` old (D2–D6).
fn read_price(
    price_account: &AccountWithMetadata,
    oracle: &OracleDescriptor,
    params: &MarketParams,
    now: u64,
) -> u128 {
    let p = check_price_account(price_account, oracle, params);
    assert_ne!(p.price, 0, "oracle price is zero");
    assert!(
        p.timestamp <= now.saturating_add(argo_lending_core::MAX_FUTURE_SKEW_MS),
        "oracle price is from the future"
    );
    let age = now.saturating_sub(p.timestamp);
    assert!(age <= oracle.max_age_ms, "oracle price is stale");
    p.price
}

/// Identity, owner and pair checks shared by `create_market` and price reads.
fn check_price_account(
    price_account: &AccountWithMetadata,
    oracle: &OracleDescriptor,
    params: &MarketParams,
) -> OraclePriceAccount {
    assert_eq!(
        price_account.account_id, oracle.price_account,
        "wrong price account"
    );
    assert_eq!(
        price_account.account.program_owner, oracle.oracle_program,
        "price account not owned by the oracle program"
    );
    let p = OraclePriceAccount::decode_prefix(price_account.account.data.as_ref())
        .expect("price account does not decode");
    assert_eq!(
        p.base_asset, params.collateral_token,
        "oracle base is not the collateral token"
    );
    assert_eq!(
        p.quote_asset, params.loan_token,
        "oracle quote is not the loan token"
    );
    p
}

/// Accrual scaffolding (D9). M1 has no rate model, so interest is zero and
/// only `last_update_ms` moves; M2 computes interest from `irm_core` over
/// `elapsed_ms` here. Time never moves backwards: a clock earlier than
/// `last_update_ms` accrues nothing and leaves it unchanged.
fn accrue(market: &mut Market, now: u64) {
    let elapsed_ms = now.saturating_sub(market.last_update_ms);
    if elapsed_ms == 0 {
        return;
    }
    market.last_update_ms = now;
}

/// Exactly one of `assets` and `shares` must be non-zero (Morpho's `exactlyOneZero`).
fn assert_exactly_one(assets: u128, shares: u128) {
    assert!(
        (assets == 0) != (shares == 0),
        "exactly one of assets and shares must be non-zero"
    );
}

/// Unchanged post-state for a read-only input.
fn same(acc: &AccountWithMetadata) -> AccountPostState {
    AccountPostState::new(acc.account.clone())
}

/// D15: inputs a calling program could forge because SPEL would drop them from
/// the output (unowned, not default, unclaimed). These call the handlers the way
/// a wrapper program's chained call would, with forged pre-states.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, reason = "tests")]
mod forged_input_tests {
    use super::*;
    use lee_core::account::{AccountId, Nonce};

    const ARGO: ProgramId = [42; 8];

    /// Unowned but not default: what a wrapper fabricates, or what a plain key
    /// becomes after its first signature (nonce 1).
    fn forged(id: [u8; 32], authorized: bool) -> AccountWithMetadata {
        AccountWithMetadata {
            account: Account {
                nonce: Nonce(1),
                ..Account::default()
            },
            is_authorized: authorized,
            account_id: AccountId::new(id),
        }
    }

    fn owned(id: [u8; 32]) -> AccountWithMetadata {
        AccountWithMetadata {
            account: Account {
                program_owner: ARGO,
                ..Account::default()
            },
            is_authorized: false,
            account_id: AccountId::new(id),
        }
    }

    #[test]
    #[should_panic(expected = "is unowned and not default, so the runtime cannot check it")]
    fn forged_caller_is_refused_in_withdraw() {
        let victim = AccountId::new([7; 32]);
        let _ = ops::withdraw(
            owned([1; 32]),
            owned([2; 32]),
            owned([3; 32]),
            owned([4; 32]),
            forged([7; 32], true),
            owned([5; 32]),
            1,
            0,
            victim,
            ARGO,
        );
    }

    #[test]
    #[should_panic(expected = "is unowned and not default, so the runtime cannot check it")]
    fn forged_payer_is_refused_in_supply_collateral() {
        let _ = ops::supply_collateral(
            owned([1; 32]),
            owned([2; 32]),
            owned([3; 32]),
            forged([8; 32], true),
            1,
            AccountId::new([8; 32]),
            ARGO,
        );
    }

    #[test]
    #[should_panic(expected = "is unowned and not default, so the runtime cannot check it")]
    fn forged_clock_is_refused_in_borrow() {
        let mut clock = forged(*CLOCK_01_ACCOUNT_ID.value(), false);
        clock.account.data = Data::try_from(
            borsh::to_vec(&ClockAccountData {
                block_id: 1,
                timestamp: 1,
            })
            .unwrap(),
        )
        .unwrap();
        let _ = ops::borrow(
            owned([1; 32]),
            owned([2; 32]),
            owned([3; 32]),
            owned([4; 32]),
            owned([6; 32]),
            owned([5; 32]),
            clock,
            1,
            0,
            AccountId::new([6; 32]),
            ARGO,
        );
    }

    #[test]
    #[should_panic(expected = "is unowned and not default, so the runtime cannot check it")]
    fn forged_admin_is_refused_in_enable_irm() {
        let _ = admin::enable_irm(owned([1; 32]), forged([9; 32], true), [3; 8], ARGO);
    }

    #[test]
    #[should_panic(expected = "admin account must be fresh or program-owned")]
    fn unowned_used_admin_cannot_initialize() {
        let admin = forged(*argo_lending_core::ADMIN_GENESIS.value(), true);
        let config = AccountWithMetadata {
            account: Account::default(),
            is_authorized: false,
            account_id: argo_lending_core::config_id(&ARGO),
        };
        let _ = initialize(config, admin, ARGO);
    }
}
