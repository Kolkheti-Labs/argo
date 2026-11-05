//! Read the Config PDA over JSON-RPC and print its decoded fields.
//!   LEE_WALLET_HOME_DIR=<home> argo_read_config <config-account-id>

use std::str::FromStr as _;

use lee_core::account::AccountId;
use sequencer_service_rpc::RpcClient as _;
use wallet::WalletCore;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let id = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("usage: argo_read_config <account-id>"))?;
    let id = AccountId::from_str(&id).map_err(|e| anyhow::anyhow!("bad account id: {e:?}"))?;
    let wallet = WalletCore::from_env().await?;
    let account = wallet
        .helm_owned()
        .get_account(id)
        .await
        .map_err(|e| anyhow::anyhow!("get_account failed: {e}"))?;
    let cfg: argo_lending_core::Config = borsh::from_slice(account.data.as_ref())?;
    println!("ADMIN={}", cfg.admin);
    println!(
        "FEE_RECIPIENT={:?}",
        cfg.fee_recipient.map(|a| a.to_string())
    );
    println!("ENABLED_LLTV={:?}", cfg.enabled_lltv);
    println!("ENABLED_IRM={}", cfg.enabled_irm.len());
    Ok(())
}
