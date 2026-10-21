//! Argo lending program logic (M0 placeholder: `initialize` only).

use argo_lending_core::{Config, CONFIG_SEED};
use lee_core::account::{Account, AccountWithMetadata, Data};
use lee_core::program::{AccountPostState, ChainedCall, Claim, ProgramId};

/// Output shape shared by every handler.
pub type Output = (Vec<AccountPostState>, Vec<ChainedCall>);

/// Claim the `Config` singleton; the signing `admin` account becomes the admin
/// authority. M2 replaces this with the RFP-001 admin-authority library.
pub fn initialize(
    config: AccountWithMetadata,
    admin: AccountWithMetadata,
    self_program_id: ProgramId,
) -> Output {
    assert!(admin.is_authorized, "admin must sign Initialize");
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
    let cfg = Config {
        admin: admin.account_id,
        fee_recipient: None,
        enabled_lltv: vec![],
        enabled_irm: vec![],
    };
    let mut post = config.account;
    post.data =
        Data::try_from(borsh::to_vec(&cfg).expect("Config serialises")).expect("Config fits");
    (
        vec![
            AccountPostState::new_claimed(post, Claim::Pda(CONFIG_SEED)),
            AccountPostState::new_claimed_if_default(admin.account, Claim::Authorized),
        ],
        vec![],
    )
}
