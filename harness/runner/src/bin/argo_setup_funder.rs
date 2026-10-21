//! Import the genesis-funded public account (testnet_initial_state public
//! account 0) into the wallet at `LEE_WALLET_HOME_DIR`, so the harness can
//! pay from it on a standalone sequencer whose genesis is the testnet initial
//! state. Prints `FUNDER_ID=<id>`.

use wallet::WalletCore;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut wallet = WalletCore::from_env().await?;
    let funder = testnet_initial_state::initial_pub_accounts_private_keys()
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("genesis has no public accounts"))?;
    let funder_id = funder.account_id;
    wallet
        .storage_mut()
        .key_chain_mut()
        .add_imported_public_account(funder.pub_sign_key);
    wallet.store_persistent_data()?;
    println!("FUNDER_ID={funder_id}");
    Ok(())
}
