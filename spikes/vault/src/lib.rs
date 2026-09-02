//! Pure, host-testable logic of the spike vault program. Every function takes
//! the pre-states it was given and returns `(post_states, chained_calls)`;
//! the guest is a thin dispatcher (see `methods/guest/src/bin/spike_vault.rs`).
//!
//! Panics are the program's error channel: a panic anywhere in a chain fails
//! the whole transaction and the sequencer drops it from the block.

use lee_core::account::{Account, AccountWithMetadata, Data};
use lee_core::program::{AccountPostState, ChainedCall, Claim, ProgramId};
use spike_vault_core::{Instruction, VaultState, STATE_SEED, VAULT_SEED};
use token_core::TokenHolding;

/// Output shape shared by every handler.
pub type Output = (Vec<AccountPostState>, Vec<ChainedCall>);

fn read_state(state: &AccountWithMetadata) -> VaultState {
    borsh::from_slice(state.account.data.as_ref()).expect("state PDA must hold a VaultState")
}

fn write_state(mut account: Account, st: &VaultState) -> Account {
    account.data = Data::try_from(borsh::to_vec(st).expect("VaultState serialises"))
        .expect("VaultState fits DATA_MAX_LENGTH");
    account
}

/// `Init`: claim the state PDA under our seed and ask the token program to
/// initialise the vault as a fungible holding, authorised by our vault seed.
pub fn init(
    state: AccountWithMetadata,
    vault: AccountWithMetadata,
    token_definition: AccountWithMetadata,
    self_program_id: ProgramId,
) -> Output {
    assert_eq!(state.account, Account::default(), "state must be uninitialised");
    assert_eq!(vault.account, Account::default(), "vault must be uninitialised");
    assert_eq!(state.account_id, spike_vault_core::state_id(&self_program_id), "state id mismatch");
    assert_eq!(vault.account_id, spike_vault_core::vault_id(&self_program_id), "vault id mismatch");
    let token_program_id = token_definition.account.program_owner;

    let st = VaultState { ops: 0, internal_hits: 0, vault: vault.account_id, pad: Vec::new() };
    let post_states = vec![
        AccountPostState::new_claimed(write_state(state.account, &st), Claim::Pda(STATE_SEED)),
        AccountPostState::new(vault.account.clone()),
        AccountPostState::new(token_definition.account.clone()),
    ];

    let mut vault_authorized = vault;
    vault_authorized.is_authorized = true;
    let init_call = ChainedCall::new(
        token_program_id,
        vec![token_definition, vault_authorized],
        &token_core::Instruction::InitializeAccount,
    )
    .with_pda_seeds(vec![VAULT_SEED]);

    (post_states, vec![init_call])
}

/// `PayIn`: bump `ops`, then chain a user-authorised transfer into the vault.
pub fn pay_in(
    state: AccountWithMetadata,
    user_holding: AccountWithMetadata,
    vault: AccountWithMetadata,
    amount: u128,
) -> Output {
    assert!(user_holding.is_authorized, "payer must be authorised");
    let mut st = read_state(&state);
    assert_eq!(st.vault, vault.account_id, "vault id does not match state");
    st.ops = st.ops.checked_add(1).expect("ops overflow");
    let token_program_id = vault.account.program_owner;

    let post_states = vec![
        AccountPostState::new(write_state(state.account, &st)),
        AccountPostState::new(user_holding.account.clone()),
        AccountPostState::new(vault.account.clone()),
    ];
    let transfer = ChainedCall::new(
        token_program_id,
        vec![user_holding, vault],
        &token_core::Instruction::Transfer { amount_to_transfer: amount },
    );
    (post_states, vec![transfer])
}

/// `PayOut`: bump `ops`, chain a PDA-authorised transfer out of the vault, then
/// chain `Internal { must_fail }` back into ourselves. With `then_fail = true`
/// the last leg panics; S-A asserts `ops` and the vault balance are untouched.
///
/// `seed` is normally `VAULT_SEED`; the wrong-seed negative test passes
/// `WRONG_SEED` and expects the runtime to reject the chain.
pub fn pay_out(
    state: AccountWithMetadata,
    vault: AccountWithMetadata,
    recipient: AccountWithMetadata,
    amount: u128,
    then_fail: bool,
    seed: lee_core::program::PdaSeed,
    self_program_id: ProgramId,
) -> Output {
    let mut st = read_state(&state);
    assert_eq!(st.vault, vault.account_id, "vault id does not match state");
    st.ops = st.ops.checked_add(1).expect("ops overflow");
    let token_program_id = vault.account.program_owner;
    let state_post = write_state(state.account.clone(), &st);

    let post_states = vec![
        AccountPostState::new(state_post.clone()),
        AccountPostState::new(vault.account.clone()),
        AccountPostState::new(recipient.account.clone()),
    ];

    let mut vault_authorized = vault;
    vault_authorized.is_authorized = true;
    let transfer = ChainedCall::new(
        token_program_id,
        vec![vault_authorized, recipient],
        &token_core::Instruction::Transfer { amount_to_transfer: amount },
    )
    .with_pda_seeds(vec![seed]);

    // The self-call's pre-state must equal the state as it will be after this
    // execution's post-state is applied (the caller predicts the callee's view).
    let state_for_internal = AccountWithMetadata {
        account: state_post,
        is_authorized: false,
        account_id: state.account_id,
    };
    let internal = ChainedCall::new(
        self_program_id,
        vec![state_for_internal],
        &Instruction::Internal { must_fail: then_fail },
    );

    (post_states, vec![transfer, internal])
}

/// `Fanout`: chain `n` copies of `Internal { must_fail: false }`. Every copy
/// sees the same predicted state, so `n` copies would each bump
/// `internal_hits` from the same base; that is fine for a length test but the
/// pre-state of copy k must reflect copies 1..k-1 having run. We therefore
/// predict `internal_hits + k` for copy k.
pub fn fanout(state: AccountWithMetadata, n: u32, self_program_id: ProgramId) -> Output {
    let base = read_state(&state);
    let mut calls = Vec::new();
    for k in 0..n {
        let mut predicted = base.clone();
        predicted.internal_hits = predicted
            .internal_hits
            .checked_add(u64::from(k))
            .expect("hits overflow");
        let pre = AccountWithMetadata {
            account: write_state(state.account.clone(), &predicted),
            is_authorized: false,
            account_id: state.account_id,
        };
        calls.push(ChainedCall::new(
            self_program_id,
            vec![pre],
            &Instruction::Internal { must_fail: false },
        ));
    }
    (vec![AccountPostState::new(state.account)], calls)
}

/// `Stress`: S-D compute probe. Each iteration does the arithmetic an Argo
/// accrual + health check + liquidation derivation does (widening mul_div in
/// both rounding modes, share conversions, utilisation), over values that
/// depend on the loop counter so nothing folds away.
pub fn stress(state: AccountWithMetadata, iters: u32, pad: u32) -> Output {
    let mut st = read_state(&state);
    let mut acc: u128 = 1_000_000_000_000u128.wrapping_add(u128::from(st.ops));
    for i in 0..iters {
        let k = u128::from(i).wrapping_add(1);
        let ta = acc.wrapping_add(k.wrapping_mul(7));
        let ts = acc.wrapping_add(k.wrapping_mul(5));
        let s1 = argo_core::shares::to_shares_up(k.wrapping_mul(1_000), ta, ts).unwrap_or(1);
        let a1 = argo_core::shares::to_assets_down(s1, ta, ts).unwrap_or(1);
        let u = irm_core::utilization(a1, ta).unwrap_or(0);
        let hv = argo_core::math::mul_div_down(a1, argo_core::WAD, u.wrapping_add(1)).unwrap_or(1);
        let lif = argo_core::math::w_div_up(argo_core::WAD, hv.wrapping_add(argo_core::WAD)).unwrap_or(1);
        acc = acc.wrapping_add(s1 ^ a1 ^ u ^ hv ^ lif) & (u128::MAX >> 8);
    }
    st.ops = st.ops.checked_add(1).expect("ops overflow");
    // Fold the accumulator into the pad so the loop is observable.
    let n = usize::try_from(pad).expect("pad fits usize");
    st.pad = (0..n).map(|j| (acc.wrapping_add(j as u128) & 0xff) as u8).collect();
    (vec![AccountPostState::new(write_state(state.account, &st))], vec![])
}

/// `Internal`: reachable only from a chained self-call.
pub fn internal(
    state: AccountWithMetadata,
    must_fail: bool,
    caller_program_id: ProgramId,
    self_program_id: ProgramId,
) -> Output {
    assert_eq!(caller_program_id, self_program_id, "Internal must be called by self");
    assert!(!must_fail, "Internal asked to fail");
    let mut st = read_state(&state);
    st.internal_hits = st.internal_hits.checked_add(1).expect("hits overflow");
    (vec![AccountPostState::new(write_state(state.account, &st))], vec![])
}

/// Decode the vault's fungible balance from a token holding account.
#[must_use]
pub fn holding_balance(account: &Account) -> Option<u128> {
    match TokenHolding::try_from(&account.data).ok()? {
        TokenHolding::Fungible { balance, .. } => Some(balance),
        _ => None,
    }
}
