#![cfg_attr(not(test), no_main)]

use argo_lending_core::{MarketParams, OracleDescriptor};
use nssa_core::account::{AccountId, AccountWithMetadata};
use nssa_core::program::ProgramId;
use spel_framework::context::ProgramContext;
use spel_framework::prelude::*;

#[cfg(not(test))]
risc0_zkvm::guest::entry!(main);

/// Thin dispatcher: every handler forwards to `argo_lending_program`, which
/// holds the logic and is tested on the host. Account order here is the
/// instruction's account order (see `argo_lending_core::Instruction`).
#[lez_program(instruction = "argo_lending_core::Instruction")]
mod argo_lending {
    #[allow(unused_imports)]
    use super::*;

    #[instruction]
    pub fn initialize(
        ctx: ProgramContext,
        #[account(mut)] config: AccountWithMetadata,
        #[account(mut, signer)] admin: AccountWithMetadata,
    ) -> SpelResult {
        let (posts, calls) = argo_lending_program::initialize(config, admin, ctx.self_program_id);
        Ok(spel_framework::SpelOutput::execute(posts, calls))
    }

    #[instruction]
    pub fn enable_lltv(
        ctx: ProgramContext,
        #[account(mut)] config: AccountWithMetadata,
        #[account(signer)] admin: AccountWithMetadata,
        lltv: u128,
    ) -> SpelResult {
        let (posts, calls) = argo_lending_program::enable_lltv(config, admin, lltv, ctx.self_program_id);
        Ok(spel_framework::SpelOutput::execute(posts, calls))
    }

    #[instruction]
    pub fn enable_irm(
        ctx: ProgramContext,
        #[account(mut)] config: AccountWithMetadata,
        #[account(signer)] admin: AccountWithMetadata,
        irm: ProgramId,
    ) -> SpelResult {
        let (posts, calls) = argo_lending_program::enable_irm(config, admin, irm, ctx.self_program_id);
        Ok(spel_framework::SpelOutput::execute(posts, calls))
    }

    #[instruction]
    pub fn create_market(
        ctx: ProgramContext,
        config: AccountWithMetadata,
        #[account(mut)] market: AccountWithMetadata,
        #[account(mut)] loan_vault: AccountWithMetadata,
        #[account(mut)] collateral_vault: AccountWithMetadata,
        #[account(mut)] dead_position: AccountWithMetadata,
        loan_definition: AccountWithMetadata,
        collateral_definition: AccountWithMetadata,
        price_account: AccountWithMetadata,
        clock: AccountWithMetadata,
        #[account(mut, signer)] creator_loan_holding: AccountWithMetadata,
        params: MarketParams,
        oracle: OracleDescriptor,
    ) -> SpelResult {
        let (posts, calls) = argo_lending_program::create_market(
            config,
            market,
            loan_vault,
            collateral_vault,
            dead_position,
            loan_definition,
            collateral_definition,
            price_account,
            clock,
            creator_loan_holding,
            params,
            oracle,
            ctx.self_program_id,
        );
        Ok(spel_framework::SpelOutput::execute(posts, calls))
    }

    #[instruction]
    pub fn supply(
        ctx: ProgramContext,
        #[account(mut)] market: AccountWithMetadata,
        #[account(mut)] position: AccountWithMetadata,
        #[account(mut)] loan_vault: AccountWithMetadata,
        #[account(mut, signer)] payer_holding: AccountWithMetadata,
        clock: AccountWithMetadata,
        assets: u128,
        shares: u128,
        on_behalf: AccountId,
    ) -> SpelResult {
        let (posts, calls) = argo_lending_program::supply(
            market, position, loan_vault, payer_holding, clock, assets, shares, on_behalf, ctx.self_program_id,
        );
        Ok(spel_framework::SpelOutput::execute(posts, calls))
    }

    #[instruction]
    pub fn withdraw(
        ctx: ProgramContext,
        #[account(mut)] market: AccountWithMetadata,
        #[account(mut)] position: AccountWithMetadata,
        #[account(mut)] loan_vault: AccountWithMetadata,
        #[account(mut)] receiver_holding: AccountWithMetadata,
        #[account(signer)] caller: AccountWithMetadata,
        clock: AccountWithMetadata,
        assets: u128,
        shares: u128,
        on_behalf: AccountId,
    ) -> SpelResult {
        let (posts, calls) = argo_lending_program::withdraw(
            market,
            position,
            loan_vault,
            receiver_holding,
            caller,
            clock,
            assets,
            shares,
            on_behalf,
            ctx.self_program_id,
        );
        Ok(spel_framework::SpelOutput::execute(posts, calls))
    }

    #[instruction]
    pub fn supply_collateral(
        ctx: ProgramContext,
        market: AccountWithMetadata,
        #[account(mut)] position: AccountWithMetadata,
        #[account(mut)] collateral_vault: AccountWithMetadata,
        #[account(mut, signer)] payer_holding: AccountWithMetadata,
        assets: u128,
        on_behalf: AccountId,
    ) -> SpelResult {
        let (posts, calls) = argo_lending_program::supply_collateral(
            market, position, collateral_vault, payer_holding, assets, on_behalf, ctx.self_program_id,
        );
        Ok(spel_framework::SpelOutput::execute(posts, calls))
    }

    #[instruction]
    pub fn withdraw_collateral(
        ctx: ProgramContext,
        #[account(mut)] market: AccountWithMetadata,
        #[account(mut)] position: AccountWithMetadata,
        #[account(mut)] collateral_vault: AccountWithMetadata,
        #[account(mut)] receiver_holding: AccountWithMetadata,
        #[account(signer)] caller: AccountWithMetadata,
        price_account: AccountWithMetadata,
        clock: AccountWithMetadata,
        assets: u128,
        on_behalf: AccountId,
    ) -> SpelResult {
        let (posts, calls) = argo_lending_program::withdraw_collateral(
            market,
            position,
            collateral_vault,
            receiver_holding,
            caller,
            price_account,
            clock,
            assets,
            on_behalf,
            ctx.self_program_id,
        );
        Ok(spel_framework::SpelOutput::execute(posts, calls))
    }

    #[instruction]
    pub fn borrow(
        ctx: ProgramContext,
        #[account(mut)] market: AccountWithMetadata,
        #[account(mut)] position: AccountWithMetadata,
        #[account(mut)] loan_vault: AccountWithMetadata,
        #[account(mut)] receiver_holding: AccountWithMetadata,
        #[account(signer)] caller: AccountWithMetadata,
        price_account: AccountWithMetadata,
        clock: AccountWithMetadata,
        assets: u128,
        shares: u128,
        on_behalf: AccountId,
    ) -> SpelResult {
        let (posts, calls) = argo_lending_program::borrow(
            market,
            position,
            loan_vault,
            receiver_holding,
            caller,
            price_account,
            clock,
            assets,
            shares,
            on_behalf,
            ctx.self_program_id,
        );
        Ok(spel_framework::SpelOutput::execute(posts, calls))
    }

    #[instruction]
    pub fn repay(
        ctx: ProgramContext,
        #[account(mut)] market: AccountWithMetadata,
        #[account(mut)] position: AccountWithMetadata,
        #[account(mut)] loan_vault: AccountWithMetadata,
        #[account(mut, signer)] payer_holding: AccountWithMetadata,
        clock: AccountWithMetadata,
        assets: u128,
        shares: u128,
        on_behalf: AccountId,
    ) -> SpelResult {
        let (posts, calls) = argo_lending_program::repay(
            market, position, loan_vault, payer_holding, clock, assets, shares, on_behalf, ctx.self_program_id,
        );
        Ok(spel_framework::SpelOutput::execute(posts, calls))
    }
}
