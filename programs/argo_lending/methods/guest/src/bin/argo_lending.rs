#![cfg_attr(not(test), no_main)]

use nssa_core::account::AccountWithMetadata;
use spel_framework::context::ProgramContext;
use spel_framework::prelude::*;

#[cfg(not(test))]
risc0_zkvm::guest::entry!(main);

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
}
