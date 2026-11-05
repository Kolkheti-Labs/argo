//! Deploy an embedded guest to the sequencer the wallet config points at.
//! Prints `PROGRAM_ID=<hex>` and `TX=<hash>` for the shell harness.
//!
//!   LEE_WALLET_HOME_DIR=<home> argo_deploy argo_lending

use lee::program_deployment_transaction::{Message, ProgramDeploymentTransaction};
use lez_common::transaction::LeeTransaction;
use sequencer_service_rpc::RpcClient as _;
use wallet::WalletCore;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let name = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("usage: argo_deploy <guest>"))?;
    let guest =
        harness_runner::guest(&name).ok_or_else(|| anyhow::anyhow!("unknown guest {name}"))?;
    let wallet = WalletCore::from_env().await?;

    let tx = ProgramDeploymentTransaction::new(Message::new(guest.elf.to_vec()));
    let tx_hash = wallet
        .helm_owned()
        .send_transaction(LeeTransaction::ProgramDeployment(tx))
        .await
        .map_err(|e| anyhow::anyhow!("send_transaction failed: {e}"))?;

    println!("GUEST={}", guest.name);
    println!(
        "PROGRAM_ID={}",
        harness_runner::format_program_id(&guest.id)
    );
    println!("TX={tx_hash}");
    Ok(())
}
