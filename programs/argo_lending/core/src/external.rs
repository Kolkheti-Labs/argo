//! Types of the accounts and programs Argo reads or calls but does not own,
//! copied (D1, D3, D13) so the guest does not depend on lez-programs crates,
//! whose `twap_oracle_core` pins `ruint =1.17.0`. Tests compare the token types
//! and instruction indices with the originals; the oracle and clock layouts are
//! exercised by the sequencer e2e.
//!
//! - Token: logos-blockchain/lez-programs tag `v2.0.1`, `programs/token/core/src/lib.rs`
//!   (MIT). Deployed on testnet-legacy with image id `14b3bc6c…` (lez-programs
//!   `DEPLOYMENTS.md` on `main`; that file is not in the v2.0.1 tag).
//! - Oracle: the same tag, `programs/twap_oracle/core/src/lib.rs`, `OraclePriceAccount`.
//! - Clock: LEZ `v0.2.4`, `lez/programs/clock/core/src/lib.rs`.
//!
//! Only the variants Argo encodes are vendored. Borsh and risc0-serde encode an
//! enum variant by its index, so each vendored enum keeps the upstream order up
//! to the last variant Argo uses, with placeholders for the rest.

use borsh::{BorshDeserialize, BorshSerialize};
use lee_core::account::AccountId;
use lee_core::program::ProgramId;
use serde::{Deserialize, Serialize};

/// lez-programs v2.0.1 token program, image id
/// `14b3bc6cc129d0359f9595ba0f60a13e4a8df08ffe8eb1a83251c47abecc67a8`
/// as little-endian `u32` words. Argo's token calls target only this id (D1).
pub const TOKEN_PROGRAM_ID: ProgramId = [
    1_824_305_940,
    902_834_625,
    3_130_365_343,
    1_050_763_279,
    2_414_906_698,
    2_830_208_766,
    2_059_686_194,
    2_825_374_910,
];

/// The token program's instructions, in upstream order. Only `Transfer` and
/// `InitializeAccount` are ever sent by Argo; the two placeholders keep
/// `InitializeAccount` at index 3.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum TokenInstruction {
    /// Index 0. Accounts: sender holding (authorised), recipient holding.
    Transfer { amount_to_transfer: u128 },
    /// Index 1, never sent by Argo.
    NewFungibleDefinitionPlaceholder,
    /// Index 2, never sent by Argo.
    NewDefinitionWithMetadataPlaceholder,
    /// Index 3. Accounts: definition, new holding (uninitialised, authorised).
    InitializeAccount,
}

/// `token_core::TokenDefinition` at lez-programs v2.0.1.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
pub enum TokenDefinition {
    /// A fungible token.
    Fungible {
        /// Display name.
        name: String,
        /// Current supply.
        total_supply: u128,
        /// Optional metadata account.
        metadata_id: Option<AccountId>,
        /// Mint authority; `None` means fixed supply.
        authority: Option<AccountId>,
    },
    /// A non-fungible token.
    NonFungible {
        /// Display name.
        name: String,
        /// Copies that can still be printed.
        printable_supply: u128,
        /// Metadata account.
        metadata_id: AccountId,
    },
}

/// `token_core::TokenHolding` at lez-programs v2.0.1.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
pub enum TokenHolding {
    /// A fungible balance.
    Fungible {
        /// The token definition this holding belongs to.
        definition_id: AccountId,
        /// Balance in raw units.
        balance: u128,
    },
    /// Master copy of an NFT.
    NftMaster {
        /// The token definition.
        definition_id: AccountId,
        /// Copies left to print, minus one.
        print_balance: u128,
    },
    /// A printed NFT copy.
    NftPrintedCopy {
        /// The token definition.
        definition_id: AccountId,
        /// Whether the holder owns it.
        owned: bool,
    },
}

/// `twap_oracle_core::OraclePriceAccount` at lez-programs v2.0.1 (136 bytes, no
/// discriminator). `price` is Q64.64: `quote_asset` raw units per one raw unit
/// of `base_asset` (D3). Decode it with [`OraclePriceAccount::decode_prefix`].
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
pub struct OraclePriceAccount {
    /// The priced asset (Argo: the collateral token definition).
    pub base_asset: AccountId,
    /// The asset `price` is denominated in (Argo: the loan token definition).
    pub quote_asset: AccountId,
    /// Q64.64 price.
    pub price: u128,
    /// Observation time, Unix milliseconds.
    pub timestamp: u64,
    /// The account that wrote this price.
    pub source_id: AccountId,
    /// Source confidence interval, or zero. Argo does not read it.
    pub confidence_interval: u128,
}

impl OraclePriceAccount {
    /// Decode the known fields and ignore any trailing bytes, so a field appended
    /// upstream (RFP-019 append-friendly) does not break an immutable market (D6).
    #[must_use]
    pub fn decode_prefix(mut data: &[u8]) -> Option<Self> {
        Self::deserialize(&mut data).ok()
    }
}

/// `CLOCK_01`, rewritten by the sequencer every block (LEZ v0.2.4).
pub const CLOCK_01_ACCOUNT_ID: AccountId = AccountId::new(*b"/LEZ/ClockProgramAccount/0000001");

/// `clock_core::ClockAccountData` at LEZ v0.2.4.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockAccountData {
    /// Block the timestamp was taken in.
    pub block_id: u64,
    /// Unix milliseconds.
    pub timestamp: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_program_id_matches_published_hex() {
        let mut bytes = Vec::with_capacity(32);
        for w in TOKEN_PROGRAM_ID {
            bytes.extend_from_slice(&w.to_le_bytes());
        }
        assert_eq!(
            hex::encode(bytes),
            "14b3bc6cc129d0359f9595ba0f60a13e4a8df08ffe8eb1a83251c47abecc67a8"
        );
    }

    #[test]
    fn token_account_types_match_v2_0_1() {
        let def = TokenDefinition::Fungible {
            name: "X".to_owned(),
            total_supply: 5,
            metadata_id: None,
            authority: Some(AccountId::new([4; 32])),
        };
        let theirs = lp_token_core::TokenDefinition::Fungible {
            name: "X".to_owned(),
            total_supply: 5,
            metadata_id: None,
            authority: Some(AccountId::new([4; 32])),
        };
        assert_eq!(
            borsh::to_vec(&def).expect("ok"),
            borsh::to_vec(&theirs).expect("ok")
        );
        let h = TokenHolding::Fungible {
            definition_id: AccountId::new([2; 32]),
            balance: 9,
        };
        let theirs = lp_token_core::TokenHolding::Fungible {
            definition_id: AccountId::new([2; 32]),
            balance: 9,
        };
        assert_eq!(
            borsh::to_vec(&h).expect("ok"),
            borsh::to_vec(&theirs).expect("ok")
        );
    }

    #[test]
    fn price_account_is_136_bytes_and_tolerates_trailing_bytes() {
        let p = OraclePriceAccount {
            base_asset: AccountId::new([1; 32]),
            quote_asset: AccountId::new([2; 32]),
            price: 1u128 << 64,
            timestamp: 5,
            source_id: AccountId::new([3; 32]),
            confidence_interval: 0,
        };
        let mut enc = borsh::to_vec(&p).expect("serialises");
        assert_eq!(enc.len(), 136);
        enc.extend_from_slice(&[0xAB; 24]);
        assert_eq!(OraclePriceAccount::decode_prefix(&enc), Some(p));
        assert!(borsh::from_slice::<OraclePriceAccount>(&enc).is_err());
    }

    #[test]
    fn token_instruction_indices_match_upstream() {
        let t = risc0_zkvm::serde::to_vec(&TokenInstruction::Transfer {
            amount_to_transfer: 7,
        })
        .expect("serialises");
        let i =
            risc0_zkvm::serde::to_vec(&TokenInstruction::InitializeAccount).expect("serialises");
        assert_eq!(t.first(), Some(&0));
        assert_eq!(i.first(), Some(&3));
        // Same bytes as the pinned lez-programs v2.0.1 token's own enum.
        let lt = risc0_zkvm::serde::to_vec(&lp_token_core::Instruction::Transfer {
            amount_to_transfer: 7,
        })
        .expect("serialises");
        let li = risc0_zkvm::serde::to_vec(&lp_token_core::Instruction::InitializeAccount)
            .expect("serialises");
        assert_eq!(t, lt);
        assert_eq!(i, li);
        // And as the v0.2.4 builtin's, whose order lez-programs kept.
        let bt = risc0_zkvm::serde::to_vec(&token_core::Instruction::Transfer {
            amount_to_transfer: 7,
        })
        .expect("serialises");
        let bi = risc0_zkvm::serde::to_vec(&token_core::Instruction::InitializeAccount)
            .expect("serialises");
        assert_eq!(t, bt);
        assert_eq!(i, bi);
    }
}
