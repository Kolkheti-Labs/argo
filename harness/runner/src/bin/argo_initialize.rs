//! Submit `argo_lending::Initialize`: claims the Config PDA with the genesis
//! admin (`argo_lending_core::ADMIN_GENESIS`, the public test key `[1; 32]` in M1)
//! signing as admin.
//! Prints `CONFIG_ID=<id>`, `ADMIN_ID=<id>` and `TX=<hash>`.
//!
//!   LEE_WALLET_HOME_DIR=<home> argo_initialize <program-id-hex>

use lee::public_transaction::{Message, WitnessSet};
use lee::PublicTransaction;
use lez_common::transaction::LeeTransaction;
use sequencer_service_rpc::RpcClient as _;
use wallet::WalletCore;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pid_hex = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("usage: argo_initialize <program-id-hex>"))?;
    let program_id = harness_runner::parse_program_id(&pid_hex)?;
    let wallet = WalletCore::from_env().await?;
    let admin_key = lee::PrivateKey::try_new([1; 32])?;
    let admin_id =
        lee_core::account::AccountId::from(&lee::PublicKey::new_from_private_key(&admin_key));
    anyhow::ensure!(
        admin_id == argo_lending_core::ADMIN_GENESIS,
        "test key is not ADMIN_GENESIS"
    );

    let config_id = argo_lending_core::config_id(&program_id);
    let nonces = wallet
        .helm_owned()
        .get_accounts_nonces(vec![admin_id])
        .await
        .map_err(|e| anyhow::anyhow!("get_accounts_nonces failed: {e}"))?;
    let message = Message::try_new(
        program_id,
        vec![config_id, admin_id],
        nonces,
        argo_lending_core::Instruction::Initialize,
    )?;
    let witness_set = WitnessSet::for_message(&message, &[&admin_key]);
    let tx = PublicTransaction::new(message, witness_set);

    let tx_hash = wallet
        .helm_owned()
        .send_transaction(LeeTransaction::Public(tx))
        .await
        .map_err(|e| anyhow::anyhow!("send_transaction failed: {e}"))?;

    println!("CONFIG_ID={config_id}");
    println!("ADMIN_ID={admin_id}");
    println!("TX={tx_hash}");
    Ok(())
}
