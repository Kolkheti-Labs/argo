//! Argo lending program logic (M0 placeholder: `initialize` only).

use argo_lending_core::{Config, CONFIG_SEED};
use lee_core::account::{Account, AccountId, AccountWithMetadata, Data};
use lee_core::program::{AccountPostState, ChainedCall, Claim, ProgramId};

/// Output shape shared by every handler.
pub type Output = (Vec<AccountPostState>, Vec<ChainedCall>);

/// Claim the `Config` singleton.
pub fn initialize(config: AccountWithMetadata, admin: AccountId, self_program_id: ProgramId) -> Output {
    assert_eq!(config.account, Account::default(), "config already initialised");
    assert_eq!(config.account_id, argo_lending_core::config_id(&self_program_id), "config id mismatch");
    let cfg = Config { admin, fee_recipient: None, enabled_lltv: vec![], enabled_irm: vec![] };
    let mut post = config.account;
    post.data = Data::try_from(borsh::to_vec(&cfg).expect("Config serialises")).expect("Config fits");
    (vec![AccountPostState::new_claimed(post, Claim::Pda(CONFIG_SEED))], vec![])
}
