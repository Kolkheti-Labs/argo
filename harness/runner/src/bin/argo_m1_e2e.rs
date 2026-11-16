//! M1 done-gate run against the standalone sequencer the wallet config points
//! at: deploy the pinned lez-programs v2.0.1 token and twap_oracle and the
//! Argo guest, create two tokens and a price feed, then run all six core
//! operations, checking balances and Market and Position accounting. Includes an
//! over-LLTV borrow and an unhealthy collateral withdrawal that must not land.
//! Prints `ARGO M1 E2E GREEN` on success.
//!
//!   LEE_WALLET_HOME_DIR=<home> ARGO_SEQ_LOG=<sequencer log> argo_m1_e2e

use std::time::Duration;

use argo_lending_core::external::{CLOCK_01_ACCOUNT_ID, TOKEN_PROGRAM_ID};
use argo_lending_core::{
    collateral_vault_id, config_id, dead_owner, loan_vault_id, market_account_id, market_id,
    oracle_digest, position_id, Instruction, Market, MarketParams, OracleDescriptor, Position,
    DEAD_DEPOSIT, ORACLE_DESCRIPTOR_VERSION,
};
use lee::program_deployment_transaction::{Message as DeployMessage, ProgramDeploymentTransaction};
use lee::public_transaction::{Message, WitnessSet};
use lee::{PrivateKey, PublicKey, PublicTransaction};
use lee_core::account::{Account, AccountId};
use lee_core::program::ProgramId;
use lez_common::transaction::LeeTransaction;
use sequencer_service_rpc::RpcClient as _;
use wallet::WalletCore;

const TOKEN_BIN: &[u8] = include_bytes!("../../../../vendor/lez-programs-v2.0.1/token.bin");
const TWAP_BIN: &[u8] = include_bytes!("../../../../vendor/lez-programs-v2.0.1/twap_oracle.bin");
const LLTV: u128 = 860_000_000_000_000_000;
const IRM: ProgramId = [7; 8];
const SUPPLY: u128 = 10_000_000;
/// One hour: the e2e feed is created once and never republished.
const MAX_AGE_MS: u64 = 3_600_000;
const WINDOW_MS: u64 = 86_400_000;

/// Mirror of `twap_oracle_core::Instruction` at lez-programs v2.0.1, up to the
/// variant this run sends (that crate pins `ruint =1.17.0`, which the
/// workspace lock cannot take). Variant order matches upstream.
#[derive(serde::Serialize)]
enum TwapInstruction {
    _CreatePriceObservations {
        initial_tick: i32,
        window_duration: u64,
    },
    CreateOraclePriceAccount {
        base_asset: AccountId,
        quote_asset: AccountId,
        initial_price: u128,
        window_duration: u64,
    },
}

/// `twap_oracle_core::compute_oracle_price_account_pda` at v2.0.1.
fn twap_price_account(twap: &ProgramId, source: &AccountId, window_ms: u64) -> AccountId {
    use sha2::Digest as _;
    let mut h = sha2::Sha256::new();
    h.update(source.value());
    h.update(window_ms.to_le_bytes());
    h.update(b"ORACLE_PRICE_ACCOUNT");
    let seed = lee_core::program::PdaSeed::new(h.finalize().into());
    AccountId::for_public_pda(twap, &seed)
}

struct Run {
    wallet: WalletCore,
    argo: ProgramId,
}

fn key(b: u8) -> PrivateKey {
    PrivateKey::try_new([b; 32]).expect("valid key")
}
fn id_of(k: &PrivateKey) -> AccountId {
    AccountId::from(&PublicKey::new_from_private_key(k))
}

impl Run {
    async fn send(&self, tx: LeeTransaction) -> anyhow::Result<String> {
        let h = self
            .wallet
            .helm_owned()
            .send_transaction(tx)
            .await
            .map_err(|e| anyhow::anyhow!("send_transaction failed: {e}"))?;
        Ok(h.to_string())
    }

    /// Wait until `tx` is in a block. A rejected transaction is dropped from the
    /// block with no receipt, so absence after `secs` means it failed.
    async fn landed(&self, tx: &str, secs: u64) -> anyhow::Result<bool> {
        for _ in 0..secs {
            let hash = tx.parse().map_err(|e| anyhow::anyhow!("bad hash: {e:?}"))?;
            if self
                .wallet
                .helm_owned()
                .get_transaction(hash)
                .await
                .map_err(|e| anyhow::anyhow!("get_transaction failed: {e}"))?
                .is_some()
            {
                return Ok(true);
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        Ok(false)
    }

    async fn deploy(&self, name: &str, bytes: &[u8]) -> anyhow::Result<()> {
        let tx = ProgramDeploymentTransaction::new(DeployMessage::new(bytes.to_vec()));
        let h = self.send(LeeTransaction::ProgramDeployment(tx)).await?;
        anyhow::ensure!(self.landed(&h, 60).await?, "deploy {name} did not land");
        println!("deployed {name}");
        Ok(())
    }

    async fn public(
        &self,
        program: ProgramId,
        accounts: Vec<AccountId>,
        signers: &[&PrivateKey],
        ix: impl serde::Serialize,
    ) -> anyhow::Result<String> {
        let nonces = self
            .wallet
            .helm_owned()
            .get_accounts_nonces(signers.iter().map(|k| id_of(k)).collect())
            .await
            .map_err(|e| anyhow::anyhow!("get_accounts_nonces failed: {e}"))?;
        let msg = Message::try_new(program, accounts, nonces, ix)?;
        let ws = WitnessSet::for_message(&msg, signers);
        self.send(LeeTransaction::Public(PublicTransaction::new(msg, ws)))
            .await
    }

    /// Submit and require the transaction to land.
    async fn ok(
        &self,
        what: &str,
        program: ProgramId,
        accounts: Vec<AccountId>,
        signers: &[&PrivateKey],
        ix: impl serde::Serialize,
    ) -> anyhow::Result<()> {
        let h = self.public(program, accounts, signers, ix).await?;
        anyhow::ensure!(self.landed(&h, 60).await?, "{what}: tx {h} did not land");
        println!("ok   {what}");
        Ok(())
    }

    async fn account(&self, id: AccountId) -> anyhow::Result<Account> {
        self.wallet
            .helm_owned()
            .get_account(id)
            .await
            .map_err(|e| anyhow::anyhow!("get_account failed: {e}"))
    }

    async fn balance(&self, id: AccountId) -> anyhow::Result<u128> {
        let h: lp_token_core::TokenHolding =
            borsh::from_slice(self.account(id).await?.data.as_ref())?;
        match h {
            lp_token_core::TokenHolding::Fungible { balance, .. } => Ok(balance),
            lp_token_core::TokenHolding::NftMaster { .. }
            | lp_token_core::TokenHolding::NftPrintedCopy { .. } => anyhow::bail!("not fungible"),
        }
    }

    async fn market(&self, id: AccountId) -> anyhow::Result<Market> {
        Ok(borsh::from_slice(self.account(id).await?.data.as_ref())?)
    }

    async fn position(&self, id: AccountId) -> anyhow::Result<Position> {
        Ok(borsh::from_slice(self.account(id).await?.data.as_ref())?)
    }
}

/// True when the sequencer log shows `tx` rejected with `reason`. The sequencer
/// logs `Transaction with hash <h> failed execution check with error:
/// <pretty-printed error>, skipping it` across several lines.
fn rejected_in_log(tx: &str, reason: &str) -> anyhow::Result<bool> {
    let path =
        std::env::var("ARGO_SEQ_LOG").map_err(|_| anyhow::anyhow!("ARGO_SEQ_LOG not set"))?;
    let log = std::fs::read_to_string(&path)?;
    let header = format!("Transaction with hash {tx} failed execution check");
    let block = log
        .split_once(header.as_str())
        .and_then(|(_, rest)| rest.split_once("skipping it").map(|(b, _)| b.to_owned()))
        .unwrap_or_default();
    Ok(block.contains(reason))
}

fn check(cond: bool, what: &str) -> anyhow::Result<()> {
    anyhow::ensure!(cond, "check failed: {what}");
    println!("     {what}");
    Ok(())
}

#[tokio::main]
#[allow(clippy::too_many_lines, reason = "one linear scenario")]
async fn main() -> anyhow::Result<()> {
    let run = Run {
        wallet: WalletCore::from_env().await?,
        argo: argo_lending_methods::ARGO_LENDING_ID,
    };
    let twap_id: ProgramId = lee::program::Program::new(std::borrow::Cow::Borrowed(TWAP_BIN))?.id();
    let token_id = lee::program::Program::new(std::borrow::Cow::Borrowed(TOKEN_BIN))?.id();
    anyhow::ensure!(
        token_id == TOKEN_PROGRAM_ID,
        "vendored token binary is not the pinned program"
    );

    run.deploy("lez-programs v2.0.1 token", TOKEN_BIN).await?;
    run.deploy("lez-programs v2.0.1 twap_oracle", TWAP_BIN)
        .await?;
    run.deploy("argo_lending", argo_lending_methods::ARGO_LENDING_ELF)
        .await?;

    // Actors: one account holds one token.
    let (admin, alice, alice2, bob, bob_loan, bob_coll2) =
        (key(1), key(2), key(6), key(3), key(4), key(5));
    let (loan_def_k, coll_def_k, source_k) = (key(11), key(12), key(20));
    let (loan_def, coll_def) = (id_of(&loan_def_k), id_of(&coll_def_k));

    // Tokens: the full supply goes to the first holding.
    for (def_k, holder, name) in [(&loan_def_k, &alice, "LOAN"), (&coll_def_k, &bob, "COLL")] {
        run.ok(
            &format!("token {name}: NewFungibleDefinition"),
            TOKEN_PROGRAM_ID,
            vec![id_of(def_k), id_of(holder)],
            &[def_k, holder],
            lp_token_core::Instruction::NewFungibleDefinition {
                name: name.to_owned(),
                total_supply: SUPPLY,
                mint_authority: None,
            },
        )
        .await?;
    }
    for (def, holder, what) in [
        (loan_def, &alice2, "alice2 LOAN holding"),
        (loan_def, &bob_loan, "bob LOAN holding"),
        (coll_def, &bob_coll2, "bob second COLL holding"),
    ] {
        run.ok(
            &format!("InitializeAccount {what}"),
            TOKEN_PROGRAM_ID,
            vec![def, id_of(holder)],
            &[holder],
            lp_token_core::Instruction::InitializeAccount,
        )
        .await?;
    }

    // Price feed: a twap_oracle-owned OraclePriceAccount, collateral in loan, 1:1.
    let price_account = twap_price_account(&twap_id, &id_of(&source_k), WINDOW_MS);
    run.ok(
        "twap_oracle CreateOraclePriceAccount COLL/LOAN = 1.0",
        twap_id,
        vec![price_account, id_of(&source_k), CLOCK_01_ACCOUNT_ID],
        &[&source_k],
        TwapInstruction::CreateOraclePriceAccount {
            base_asset: coll_def,
            quote_asset: loan_def,
            initial_price: argo_core::PRICE_ONE,
            window_duration: WINDOW_MS,
        },
    )
    .await?;

    // Config.
    let cfg = config_id(&run.argo);
    run.ok(
        "Initialize",
        run.argo,
        vec![cfg, id_of(&admin)],
        &[&admin],
        Instruction::Initialize,
    )
    .await?;
    run.ok(
        "EnableLltv 0.86",
        run.argo,
        vec![cfg, id_of(&admin)],
        &[&admin],
        Instruction::EnableLltv { lltv: LLTV },
    )
    .await?;
    run.ok(
        "EnableIrm",
        run.argo,
        vec![cfg, id_of(&admin)],
        &[&admin],
        Instruction::EnableIrm { irm: IRM },
    )
    .await?;

    // Market.
    let oracle = OracleDescriptor {
        version: ORACLE_DESCRIPTOR_VERSION,
        oracle_program: twap_id,
        price_account,
        max_age_ms: MAX_AGE_MS,
    };
    let params = MarketParams {
        loan_token: loan_def,
        collateral_token: coll_def,
        oracle: oracle_digest(&oracle),
        irm: IRM,
        lltv: LLTV,
    };
    let mid = market_id(&params);
    let market = market_account_id(&run.argo, &mid);
    let loan_vault = loan_vault_id(&run.argo, &mid);
    let coll_vault = collateral_vault_id(&run.argo, &mid);
    let pos = |owner: &AccountId| position_id(&run.argo, &mid, owner);
    let (a, b) = (id_of(&alice), id_of(&bob));

    run.ok(
        "CreateMarket",
        run.argo,
        vec![
            cfg,
            market,
            loan_vault,
            coll_vault,
            pos(&dead_owner(&mid)),
            loan_def,
            coll_def,
            price_account,
            CLOCK_01_ACCOUNT_ID,
            a,
        ],
        &[&alice],
        Instruction::CreateMarket { params, oracle },
    )
    .await?;
    check(
        run.balance(loan_vault).await? == DEAD_DEPOSIT,
        "loan vault holds the dead deposit",
    )?;
    check(
        run.market(market).await?.total_supply_assets == DEAD_DEPOSIT,
        "market supply = dead deposit",
    )?;

    // 1. supply
    run.ok(
        "Supply 1_000_000 (alice)",
        run.argo,
        vec![market, pos(&a), loan_vault, a, CLOCK_01_ACCOUNT_ID],
        &[&alice],
        Instruction::Supply {
            assets: 1_000_000,
            shares: 0,
            on_behalf: a,
        },
    )
    .await?;
    check(
        Some(run.balance(loan_vault).await?) == DEAD_DEPOSIT.checked_add(1_000_000),
        "loan vault +1_000_000",
    )?;
    check(
        Some(run.balance(a).await?)
            == SUPPLY
                .checked_sub(DEAD_DEPOSIT)
                .and_then(|x| x.checked_sub(1_000_000)),
        "alice paid the dead deposit and 1_000_000",
    )?;
    check(
        run.position(pos(&a)).await?.supply_shares > 0,
        "alice has supply shares",
    )?;

    // 2. supply_collateral
    run.ok(
        "SupplyCollateral 2_000_000 (bob)",
        run.argo,
        vec![market, pos(&b), coll_vault, b],
        &[&bob],
        Instruction::SupplyCollateral {
            assets: 2_000_000,
            on_behalf: b,
        },
    )
    .await?;
    check(
        run.position(pos(&b)).await?.collateral == 2_000_000,
        "bob collateral = 2_000_000",
    )?;
    check(
        Some(run.balance(b).await?) == SUPPLY.checked_sub(2_000_000),
        "bob paid 2_000_000 COLL",
    )?;
    check(
        run.balance(coll_vault).await? == 2_000_000,
        "collateral vault = 2_000_000",
    )?;

    // 3. borrow
    let borrow_accounts = |receiver: AccountId| {
        vec![
            market,
            pos(&b),
            loan_vault,
            receiver,
            b,
            price_account,
            CLOCK_01_ACCOUNT_ID,
        ]
    };
    run.ok(
        "Borrow 500_000 (bob -> bob_loan)",
        run.argo,
        borrow_accounts(id_of(&bob_loan)),
        &[&bob],
        Instruction::Borrow {
            assets: 500_000,
            shares: 0,
            on_behalf: b,
        },
    )
    .await?;
    check(
        run.balance(id_of(&bob_loan)).await? == 500_000,
        "bob_loan received 500_000",
    )?;
    check(
        Some(run.balance(loan_vault).await?) == DEAD_DEPOSIT.checked_add(500_000),
        "loan vault paid out 500_000",
    )?;
    let mb = run.market(market).await?;
    check(
        run.position(pos(&b)).await?.borrow_shares == mb.total_borrow_shares
            && mb.total_borrow_assets == 500_000,
        "bob holds all borrow shares; market debt 500_000",
    )?;

    // Health check on chain. Add supply so liquidity is not what stops the next
    // borrow: bob's borrowing power is 2_000_000 × 0.86 = 1_720_000 and he owes
    // 500_000, so 1_220_001 more is one unit past LLTV.
    run.ok(
        "Supply 5_000_000 more (alice) for headroom",
        run.argo,
        vec![market, pos(&a), loan_vault, a, CLOCK_01_ACCOUNT_ID],
        &[&alice],
        Instruction::Supply {
            assets: 5_000_000,
            shares: 0,
            on_behalf: a,
        },
    )
    .await?;
    let before = run.market(market).await?;
    let h = run
        .public(
            run.argo,
            borrow_accounts(id_of(&bob_loan)),
            &[&bob],
            Instruction::Borrow {
                assets: 1_220_001,
                shares: 0,
                on_behalf: b,
            },
        )
        .await?;
    check(
        !run.landed(&h, 15).await?,
        "over-LLTV borrow (1_720_001 total) is dropped",
    )?;
    // Not just slow: the sequencer rejected it for the health check.
    check(
        rejected_in_log(&h, "Guest panicked: position would be unhealthy")?,
        "sequencer log: that tx failed with 'position would be unhealthy'",
    )?;
    check(
        run.market(market).await? == before,
        "market unchanged after the dropped borrow",
    )?;

    // 4. repay
    let vault_before_repay = run.balance(loan_vault).await?;
    run.ok(
        "Repay 200_000 (bob_loan for bob)",
        run.argo,
        vec![
            market,
            pos(&b),
            loan_vault,
            id_of(&bob_loan),
            CLOCK_01_ACCOUNT_ID,
        ],
        &[&bob_loan],
        Instruction::Repay {
            assets: 200_000,
            shares: 0,
            on_behalf: b,
        },
    )
    .await?;
    check(
        run.balance(id_of(&bob_loan)).await? == 300_000,
        "bob_loan paid 200_000",
    )?;
    check(
        Some(run.balance(loan_vault).await?) == vault_before_repay.checked_add(200_000),
        "loan vault received 200_000",
    )?;
    let mr = run.market(market).await?;
    check(mr.total_borrow_assets == 300_000, "market debt = 300_000")?;
    check(
        run.position(pos(&b)).await?.borrow_shares == mr.total_borrow_shares
            && mr.total_borrow_shares < mb.total_borrow_shares,
        "bob's borrow shares burned and match the market",
    )?;

    // Health check on withdraw_collateral, on chain: with 300_000 owed, leaving
    // 300_000 collateral (power 258_000) is unhealthy.
    let before = run.market(market).await?;
    let h = run
        .public(
            run.argo,
            vec![
                market,
                pos(&b),
                coll_vault,
                id_of(&bob_coll2),
                b,
                price_account,
                CLOCK_01_ACCOUNT_ID,
            ],
            &[&bob],
            Instruction::WithdrawCollateral {
                assets: 1_700_000,
                on_behalf: b,
            },
        )
        .await?;
    check(
        !run.landed(&h, 15).await?,
        "unhealthy collateral withdrawal is dropped",
    )?;
    check(
        rejected_in_log(&h, "Guest panicked: position would be unhealthy")?,
        "sequencer log: that tx failed with 'position would be unhealthy'",
    )?;
    check(run.market(market).await? == before, "market unchanged")?;
    check(
        run.position(pos(&b)).await?.collateral == 2_000_000,
        "bob collateral unchanged",
    )?;

    // 5. withdraw_collateral
    run.ok(
        "WithdrawCollateral 1_000_000 (bob -> bob_coll2)",
        run.argo,
        vec![
            market,
            pos(&b),
            coll_vault,
            id_of(&bob_coll2),
            b,
            price_account,
            CLOCK_01_ACCOUNT_ID,
        ],
        &[&bob],
        Instruction::WithdrawCollateral {
            assets: 1_000_000,
            on_behalf: b,
        },
    )
    .await?;
    check(
        run.balance(id_of(&bob_coll2)).await? == 1_000_000,
        "bob_coll2 received 1_000_000",
    )?;
    check(
        run.position(pos(&b)).await?.collateral == 1_000_000,
        "bob collateral = 1_000_000",
    )?;

    // 6. withdraw
    let a_shares = run.position(pos(&a)).await?.supply_shares;
    let vault_before_withdraw = run.balance(loan_vault).await?;
    let supply_before_withdraw = run.market(market).await?.total_supply_assets;
    run.ok(
        "Withdraw 100_000 (alice -> alice2)",
        run.argo,
        vec![
            market,
            pos(&a),
            loan_vault,
            id_of(&alice2),
            a,
            CLOCK_01_ACCOUNT_ID,
        ],
        &[&alice],
        Instruction::Withdraw {
            assets: 100_000,
            shares: 0,
            on_behalf: a,
        },
    )
    .await?;
    check(
        run.balance(id_of(&alice2)).await? == 100_000,
        "alice2 received 100_000",
    )?;
    check(
        Some(run.balance(loan_vault).await?) == vault_before_withdraw.checked_sub(100_000),
        "loan vault paid 100_000",
    )?;
    check(
        run.position(pos(&a)).await?.supply_shares < a_shares,
        "alice's supply shares burned",
    )?;

    let m = run.market(market).await?;
    check(
        Some(m.total_supply_assets) == supply_before_withdraw.checked_sub(100_000),
        "market supply fell by 100_000",
    )?;
    check(
        Some(run.balance(loan_vault).await?)
            == m.total_supply_assets.checked_sub(m.total_borrow_assets),
        "loan vault balance = supply − borrow",
    )?;
    check(
        run.balance(coll_vault).await? == 1_000_000,
        "collateral vault = 1_000_000",
    )?;
    println!(
        "=== ARGO M1 E2E GREEN (argo {}) ===",
        harness_runner::format_program_id(&run.argo)
    );
    Ok(())
}
