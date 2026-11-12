//! Admin operations M1 needs so markets can be created. The bounded-admin
//! design (RFP-001 library, fee, fee recipient) is M2.

use crate::{read_config, same, write, Output};
use argo_core::WAD;
use argo_lending_core::{MAX_ENABLED_IRM, MAX_ENABLED_LLTV};
use lee_core::account::AccountWithMetadata;
use lee_core::program::{AccountPostState, ProgramId};

fn assert_admin(config: &argo_lending_core::Config, admin: &AccountWithMetadata) {
    assert!(admin.is_authorized, "admin must sign");
    assert_eq!(admin.account_id, config.admin, "not the admin");
    assert_ne!(
        admin.account.program_owner,
        lee_core::program::DEFAULT_PROGRAM_ID,
        "admin must be a program-owned account"
    );
}

/// Add `lltv` (WAD-scaled, `< WAD`) to the enabled set. Enabling twice fails.
pub fn enable_lltv(
    config: AccountWithMetadata,
    admin: AccountWithMetadata,
    lltv: u128,
    self_program_id: ProgramId,
) -> Output {
    crate::assert_runtime_checked(&[&config, &admin]);
    let mut cfg = read_config(&config, &self_program_id);
    assert_admin(&cfg, &admin);
    assert!(lltv < WAD, "lltv must be below 1");
    assert!(!cfg.enabled_lltv.contains(&lltv), "lltv already enabled");
    assert!(cfg.enabled_lltv.len() < MAX_ENABLED_LLTV, "too many lltvs");
    cfg.enabled_lltv.push(lltv);
    (
        vec![
            AccountPostState::new(write(config.account, &cfg)),
            same(&admin),
        ],
        vec![],
    )
}

/// Add an IRM program id to the enabled set. Enabling twice fails.
pub fn enable_irm(
    config: AccountWithMetadata,
    admin: AccountWithMetadata,
    irm: ProgramId,
    self_program_id: ProgramId,
) -> Output {
    crate::assert_runtime_checked(&[&config, &admin]);
    let mut cfg = read_config(&config, &self_program_id);
    assert_admin(&cfg, &admin);
    assert!(!cfg.enabled_irm.contains(&irm), "irm already enabled");
    assert!(cfg.enabled_irm.len() < MAX_ENABLED_IRM, "too many irms");
    cfg.enabled_irm.push(irm);
    (
        vec![
            AccountPostState::new(write(config.account, &cfg)),
            same(&admin),
        ],
        vec![],
    )
}
