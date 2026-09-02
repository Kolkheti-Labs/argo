//! Argo lending program types. M0 ships only the `Config` account and the
//! `Initialize` instruction so the SPEL pipeline (guest → IDL → deploy → read
//! back) is exercised end to end. Market instructions land in M1.

use borsh::{BorshDeserialize, BorshSerialize};
use lee_core::account::AccountId;
use lee_core::program::{PdaSeed, ProgramId};
use serde::{Deserialize, Serialize};

/// Instruction set (M0 subset).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Instruction {
    /// Claim the singleton `Config` PDA. Callable once. Accounts: `config` (the
    /// PDA), `admin` (signer; becomes the admin authority).
    Initialize,
}

/// Singleton configuration account.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Admin authority (RFP-001 pattern is wired in M2).
    pub admin: AccountId,
    /// Protocol fee recipient; `None` until set.
    pub fee_recipient: Option<AccountId>,
    /// Enabled LLTV values (WAD-scaled).
    pub enabled_lltv: Vec<u128>,
    /// Enabled IRM program ids.
    pub enabled_irm: Vec<ProgramId>,
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

/// Seed of the `Config` PDA.
pub const CONFIG_SEED: PdaSeed = PdaSeed::new(padded(b"config"));

/// Derive the `Config` PDA id.
#[must_use]
pub fn config_id(program_id: &ProgramId) -> AccountId {
    AccountId::for_public_pda(program_id, &CONFIG_SEED)
}
