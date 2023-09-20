use anchor_lang;
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
// use anchor_lang::{account, AnchorSerialize, AnchorDeserialize};
// use mercurial_vault::context::VaultBumps;
// use mercurial_vault::state::{LockedProfitTracker, MAX_STRATEGY};
use mercurial_vault::state::{LockedProfitTracker, Vault, LOCKED_PROFIT_DEGRADATION_DENOMINATOR};
use solana_sdk::account::ReadableAccount;
use spl_associated_token_account::get_associated_token_address;
use std::convert::TryInto;

const RPC_URL: &str = "https://api.mainnet-beta.solana.com";

// #[account]
// #[derive(Default, Debug, AnchorDeserialize)]
// pub struct Vault {
//     pub enabled: u8,
//     pub bumps: VaultBumps,
//
//     pub total_amount: u64,
//
//     pub token_vault: Pubkey,
//     pub fee_vault: Pubkey,
//     pub token_mint: Pubkey,
//
//     pub lp_mint: Pubkey,
//     pub strategies: [Pubkey; MAX_STRATEGY],
//
//     pub base: Pubkey,
//     pub admin: Pubkey,
//     pub operator: Pubkey, // person to send crank
//     pub locked_profit_tracker: LockedProfitTracker,
// }

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
