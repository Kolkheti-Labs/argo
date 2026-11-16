//! Token adapter (D1). Every chained token call Argo emits is built here, and
//! always targets the pinned `TOKEN_PROGRAM_ID`, never an id taken from an
//! input account (spec §2, "Token program").

use argo_lending_core::external::{
    TokenDefinition, TokenHolding, TokenInstruction, TOKEN_PROGRAM_ID,
};
use lee_core::account::{Account, AccountId, AccountWithMetadata, Data};
use lee_core::program::{ChainedCall, PdaSeed};

/// Assert `definition` is a fungible definition owned by the pinned token program.
pub fn assert_fungible_definition(definition: &AccountWithMetadata) {
    assert_eq!(
        definition.account.program_owner, TOKEN_PROGRAM_ID,
        "token definition not owned by the pinned token program"
    );
    let d: TokenDefinition = borsh::from_slice(definition.account.data.as_ref())
        .unwrap_or_else(|_| panic!("token definition does not decode"));
    assert!(
        matches!(d, TokenDefinition::Fungible { .. }),
        "token definition is not fungible"
    );
}

/// Assert `holding` is a fungible holding of `definition_id` owned by the pinned
/// token program. Used for vaults, so a vault is never mistaken for another token.
pub fn assert_holding_of(holding: &AccountWithMetadata, definition_id: &AccountId) {
    assert_eq!(
        holding.account.program_owner, TOKEN_PROGRAM_ID,
        "holding not owned by the pinned token program"
    );
    let h: TokenHolding = borsh::from_slice(holding.account.data.as_ref())
        .unwrap_or_else(|_| panic!("token holding does not decode"));
    match h {
        TokenHolding::Fungible {
            definition_id: d, ..
        } => {
            assert_eq!(&d, definition_id, "holding is for another token");
        }
        TokenHolding::NftMaster { .. } | TokenHolding::NftPrintedCopy { .. } => {
            panic!("holding is not fungible")
        }
    }
}

/// `InitializeAccount` for a vault PDA, authorised by Argo's `seed`.
pub fn initialize_vault(
    definition: AccountWithMetadata,
    mut vault: AccountWithMetadata,
    seed: PdaSeed,
) -> ChainedCall {
    vault.is_authorized = true;
    ChainedCall::new(
        TOKEN_PROGRAM_ID,
        vec![definition, vault],
        &TokenInstruction::InitializeAccount,
    )
    .with_pda_seeds(vec![seed])
}

/// The state a vault holds right after `initialize_vault`, so a later chained
/// call in the same transaction can name it as its pre-state.
#[must_use]
pub fn initialized_vault(vault_id: AccountId, definition_id: AccountId) -> AccountWithMetadata {
    let holding = TokenHolding::Fungible {
        definition_id,
        balance: 0,
    };
    let account = Account {
        program_owner: TOKEN_PROGRAM_ID,
        data: Data::try_from(borsh::to_vec(&holding).expect("holding serialises"))
            .expect("holding fits"),
        ..Account::default()
    };
    AccountWithMetadata {
        account,
        is_authorized: false,
        account_id: vault_id,
    }
}

/// Transfer from a user holding the transaction authorised.
pub fn transfer_from_user(
    from: AccountWithMetadata,
    to: AccountWithMetadata,
    amount: u128,
) -> ChainedCall {
    assert!(from.is_authorized, "payer holding must be authorised");
    ChainedCall::new(
        TOKEN_PROGRAM_ID,
        vec![from, to],
        &TokenInstruction::Transfer {
            amount_to_transfer: amount,
        },
    )
}

/// Transfer out of an Argo vault, authorised by Argo's `seed`.
pub fn transfer_from_vault(
    mut vault: AccountWithMetadata,
    to: AccountWithMetadata,
    amount: u128,
    seed: PdaSeed,
) -> ChainedCall {
    vault.is_authorized = true;
    ChainedCall::new(
        TOKEN_PROGRAM_ID,
        vec![vault, to],
        &TokenInstruction::Transfer {
            amount_to_transfer: amount,
        },
    )
    .with_pda_seeds(vec![seed])
}
