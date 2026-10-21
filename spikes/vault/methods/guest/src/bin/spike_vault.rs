#![cfg_attr(not(test), no_main)]

use nssa_core::account::AccountWithMetadata;
use spel_framework::context::ProgramContext;
use spel_framework::prelude::*;

#[cfg(not(test))]
risc0_zkvm::guest::entry!(main);

#[lez_program(instruction = "spike_vault_core::Instruction")]
mod spike_vault {
    #[allow(unused_imports)]
    use super::*;

    #[instruction]
    pub fn init(
        ctx: ProgramContext,
        #[account(mut)] state: AccountWithMetadata,
        #[account(mut)] vault: AccountWithMetadata,
        token_definition: AccountWithMetadata,
    ) -> SpelResult {
        let (posts, calls) =
            spike_vault_program::init(state, vault, token_definition, ctx.self_program_id);
        Ok(spel_framework::SpelOutput::execute(posts, calls))
    }

    #[instruction]
    pub fn pay_in(
        #[account(mut)] state: AccountWithMetadata,
        #[account(mut, signer)] user_holding: AccountWithMetadata,
        #[account(mut)] vault: AccountWithMetadata,
        amount: u128,
    ) -> SpelResult {
        let (posts, calls) = spike_vault_program::pay_in(state, user_holding, vault, amount);
        Ok(spel_framework::SpelOutput::execute(posts, calls))
    }

    #[instruction]
    pub fn pay_out(
        ctx: ProgramContext,
        #[account(mut)] state: AccountWithMetadata,
        #[account(mut)] vault: AccountWithMetadata,
        #[account(mut)] recipient: AccountWithMetadata,
        amount: u128,
        then_fail: bool,
        wrong_seed: bool,
    ) -> SpelResult {
        let seed = if wrong_seed { spike_vault_core::WRONG_SEED } else { spike_vault_core::VAULT_SEED };
        let (posts, calls) = spike_vault_program::pay_out(
            state,
            vault,
            recipient,
            amount,
            then_fail,
            seed,
            ctx.self_program_id,
        );
        Ok(spel_framework::SpelOutput::execute(posts, calls))
    }

    #[instruction]
    pub fn fanout(
        ctx: ProgramContext,
        #[account(mut)] state: AccountWithMetadata,
        n: u32,
    ) -> SpelResult {
        let (posts, calls) = spike_vault_program::fanout(state, n, ctx.self_program_id);
        Ok(spel_framework::SpelOutput::execute(posts, calls))
    }

    #[instruction]
    pub fn stress(
        #[account(mut)] state: AccountWithMetadata,
        iters: u32,
        pad: u32,
    ) -> SpelResult {
        let (posts, calls) = spike_vault_program::stress(state, iters, pad);
        Ok(spel_framework::SpelOutput::execute(posts, calls))
    }

    #[instruction]
    pub fn internal(
        ctx: ProgramContext,
        #[account(mut)] state: AccountWithMetadata,
        must_fail: bool,
    ) -> SpelResult {
        let (posts, calls) = spike_vault_program::internal(
            state,
            must_fail,
            ctx.caller_program_id,
            ctx.self_program_id,
        );
        Ok(spel_framework::SpelOutput::execute(posts, calls))
    }
}
