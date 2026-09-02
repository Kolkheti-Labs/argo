//! Submit `argo_lending::Initialize` (claims the Config PDA, no signer).
//! Prints `CONFIG_ID=<id>` and `TX=<hash>`.
//!
//!   LEE_WALLET_HOME_DIR=<home> argo_initialize <program-id-hex>

use lee::public_transaction::{Message, WitnessSet};
use lee::PublicTransaction;
use lez_common::transaction::LeeTransaction;
use sequencer_service_rpc::RpcClient as _;
use wallet::WalletCore;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pid_hex = std::env::args().nth(1).ok_or_else(|| anyhow::anyhow!("usage: argo_initialize <program-id-hex>"))?;
    let program_id = harness_runner::parse_program_id(&pid_hex)?;
    let wallet = WalletCore::from_env().await?;

    let config_id = argo_lending_core::config_id(&program_id);
    // Admin is a placeholder for M0: the Config PDA itself. M2 wires RFP-001.
    let instruction = argo_lending_core::Instruction::Initialize { admin: config_id };
    let message = Message::try_new(program_id, vec![config_id], vec![], instruction)?;
    let witness_set = WitnessSet::for_message(&message, &[]);
    let tx = PublicTransaction::new(message, witness_set);

    let tx_hash = wallet
        .helm_owned()
        .send_transaction(LeeTransaction::Public(tx))
        .await
        .map_err(|e| anyhow::anyhow!("send_transaction failed: {e}"))?;

    println!("CONFIG_ID={config_id}");
    println!("TX={tx_hash}");
    Ok(())
}
