//! Types shared by the S-A/S-B/S-D spike program `spike_vault`, its guest, and
//! the host-side runner. This program exists only to observe runtime rules;
//! it is not part of Argo's on-chain surface.

use borsh::{BorshDeserialize, BorshSerialize};
use lee_core::account::AccountId;
use lee_core::program::{PdaSeed, ProgramId};
use serde::{Deserialize, Serialize};

/// Instruction set of the spike program. Encoded with risc0 serde.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    /// Claim the state PDA and, via a chained token `InitializeAccount`,
    /// the vault PDA as a token holding owned by the token program.
    Init,
    /// Anyone credits the vault: chained token `Transfer` user → vault.
    PayIn { amount: u128 },
    /// Program debits the vault under PDA seed: chained `Transfer` vault →
    /// recipient, then a chained self-call `Internal { must_fail }`.
    /// `wrong_seed` authorises the debit with a seed that derives no account
    /// (negative test A4).
    PayOut {
        amount: u128,
        then_fail: bool,
        wrong_seed: bool,
    },
    /// Chain `n` self-calls to `Internal { must_fail: false }` (chain-length
    /// test A6; the runtime allows at most 10 chained executions).
    Fanout { n: u32 },
    /// S-D: run `iters` rounds of Argo-shaped share/health math and resize the
    /// state's `pad` to `pad` bytes. No chained calls.
    Stress { iters: u32, pad: u32 },
    /// Internal entrypoint: only reachable through a chained call from self.
    Internal { must_fail: bool },
}

/// State PDA contents.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct VaultState {
    /// Incremented by every `PayIn` / `PayOut` before the chained calls run.
    /// S-A checks it is NOT incremented when a later leg fails.
    pub ops: u64,
    /// Incremented by `Internal`. Distinguishes "state leg ran" from "chain ran".
    pub internal_hits: u64,
    /// The vault account this state governs.
    pub vault: AccountId,
    /// Ballast for the S-D account-size sweep.
    pub pad: Vec<u8>,
}

const fn padded(s: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    let mut i = 0;
    while i < s.len() && i < 32 {
        out[i] = s[i];
        i += 1;
    }
    out
}

/// Seed of the state PDA.
pub const STATE_SEED: PdaSeed = PdaSeed::new(padded(b"argo-spike/state"));
/// Seed of the vault PDA.
pub const VAULT_SEED: PdaSeed = PdaSeed::new(padded(b"argo-spike/vault"));
/// A seed no account was derived from, for the wrong-seed negative test.
pub const WRONG_SEED: PdaSeed = PdaSeed::new(padded(b"argo-spike/wrong"));

/// Derive the state PDA id for a deployed program id.
#[must_use]
pub fn state_id(program_id: &ProgramId) -> AccountId {
    AccountId::for_public_pda(program_id, &STATE_SEED)
}

/// Derive the vault PDA id for a deployed program id.
#[must_use]
pub fn vault_id(program_id: &ProgramId) -> AccountId {
    AccountId::for_public_pda(program_id, &VAULT_SEED)
}
