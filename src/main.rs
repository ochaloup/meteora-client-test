use anchor_lang;
use mercurial_vault::state::{LockedProfitTracker, Vault, LOCKED_PROFIT_DEGRADATION_DENOMINATOR};
use solana_client::rpc_client::RpcClient;
use solana_sdk::account::ReadableAccount;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use spl_associated_token_account::get_associated_token_address;
use std::convert::TryInto;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

/**
 * This is a test to work with Meteora SDK to find out about a user wallet what is his share of the vault.
 * We use mSOL Meteora vault: 8p1VKP45hhqq5iZG5fNGoi7ucme8nFLeChoDWNy7rWFm
 * On API to get list of pools: https://app.meteora.ag/amm/pools, to get list of vaults: https://merv2-api.mercurial.finance/vault_info
 * The vault SDK is here: https://github.com/mercurial-finance/vault-sdk
 *
 * Calculation of the user share is based on the vault SDK typescript client at README: https://github.com/mercurial-finance/vault-sdk/tree/main/ts-client
 * ```ignore
 * const userShare = await vaultImpl.getUserBalance(mockWallet.publicKey);
 * const unlockedAmount = await vaultImpl.getWithdrawableAmount()
 * const lpSupply = await vaultImpl.getVaultSupply();
 * // To convert user's LP balance into underlying token amount
 * const underlyingShare = helper.getAmountByShare(userShare, unlockedAmount, lpSupply)
 * ```
 *
 * We can check what are token accounts of particular LP mint (in our case of the mSOL vault) by using API:
 * 21bR3D4QR4GzopVco44PVMBXwHFpSYrbrdeNwdKk7umb
 * we can use RCP call
 * ```ignore
 * RPC_URL="..."
 * MINT="21bR3D4QR4GzopVco44PVMBXwHFpSYrbrdeNwdKk7umb"
 * curl "$RPC_URL" -X POST -H "Content-Type: application/json" -d '
 *   {
 *     "jsonrpc": "2.0",
 *     "id": 1,
 *     "method": "getProgramAccounts",
 *     "params": [
 *       "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
 *       {
 *         "encoding": "jsonParsed",
 *         "filters": [
 *           {
 *             "dataSize": 165
 *           },
 *           {
 *             "memcmp": {
 *               "offset": 0,
 *               "bytes": "'"${MINT}"'"
 *             }
 *           }
 *         ]
 *       }
 *     ]
 * }'
 * ```
 */

const RPC_URL: &str = "https://api.mainnet-beta.solana.com";

fn main() {
    let meteora_msol_vault: Pubkey =
        Pubkey::from_str("8p1VKP45hhqq5iZG5fNGoi7ucme8nFLeChoDWNy7rWFm").unwrap();
    let connection = RpcClient::new_with_commitment(RPC_URL, CommitmentConfig::confirmed());
    let vault_account = connection.get_account(&meteora_msol_vault).unwrap();
    let vault: Vault =
        anchor_lang::AccountDeserialize::try_deserialize(&mut vault_account.data()).unwrap();

    // current time
    let now = SystemTime::now();
    let since_the_epoch = now.duration_since(UNIX_EPOCH).unwrap();
    let curr_ts = since_the_epoch.as_secs();

    // calculateWithdrawableAmount
    let Vault {
        locked_profit_tracker:
            LockedProfitTracker {
                last_report,
                locked_profit_degradation,
                last_updated_locked_profit,
            },
        total_amount: vault_total_amount,
        ..
    } = vault;

    let withdrawable_amount: u64;
    let duration = curr_ts.checked_sub(last_report).unwrap() as u128;
    let locked_fund_ratio = duration
        .checked_mul(locked_profit_degradation as u128)
        .unwrap();
    if locked_fund_ratio.gt(&LOCKED_PROFIT_DEGRADATION_DENOMINATOR) {
        withdrawable_amount = vault_total_amount;
    } else {
        let locked_profit = (last_updated_locked_profit as u128)
            .checked_mul(
                LOCKED_PROFIT_DEGRADATION_DENOMINATOR
                    .checked_sub(locked_fund_ratio as u128)
                    .unwrap(),
            )
            .unwrap()
            .checked_div(LOCKED_PROFIT_DEGRADATION_DENOMINATOR)
            .unwrap();
        withdrawable_amount = vault_total_amount
            .checked_sub(locked_profit.try_into().unwrap())
            .unwrap();
    }

    println!(
        "Token mint: {}, lp mint: {}",
        vault.token_mint, vault.lp_mint
    );
    let total_lp_supply = connection
        .get_token_supply(&vault.lp_mint)
        .unwrap()
        .amount
        .parse()
        .unwrap();

    // just expecting ATA is a user ATA, when there is the incentives program in play then ATA is calculated differently
    let expected_pk = "AWuNkfDa5o7sGaDTB6JBYV371CCQXnDAXPNwgxJxjShY";
    let wallet = Pubkey::from_str("5ZTBXQRpKa7TUVeYgC8tXVEFiMaTe77nR1aJcBRdF1Vz").unwrap();
    let user_ata = get_associated_token_address(&wallet, &vault.lp_mint);
    println!(
        "User ata account: {}, expected account Pubkey is: {}",
        user_ata, expected_pk
    );
    let user_share: u64 = connection
        .get_token_account_balance(&user_ata)
        .unwrap()
        .amount
        .parse()
        .unwrap();
    println!(
        "user share: {}, total supply: {}, withdrawable amount: {}",
        user_share, total_lp_supply, withdrawable_amount
    );

    let underlaying_share = if total_lp_supply == 0 {
        0
    } else {
        user_share
            .checked_mul(withdrawable_amount)
            .unwrap()
            .checked_div(total_lp_supply)
            .unwrap()
    };

    println!(
        "Hello vault! {}, user {} has got {} mSOL lamports",
        meteora_msol_vault, wallet, underlaying_share
    );
}
