use anyhow::{anyhow, Result};
use solana_client::nonblocking::rpc_client::RpcClient as AsyncRpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{read_keypair_file, Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use std::str::FromStr;
use std::sync::Arc;
use tracing::info;

/// Constant from Solana SDK
const LAMPORTS_PER_SOL: u64 = 1_000_000_000;

#[cfg(feature = "sync")]
use solana_client::rpc_client::RpcClient;

pub fn rpc_url(network: &str) -> &str {
    match network {
        "mainnet-beta" => "https://api.mainnet-beta.solana.com",
        "testnet" => "https://api.testnet.solana.com",
        _ => "https://api.devnet.solana.com", // default
    }
}

pub fn load_keypair(path: &str) -> Result<Keypair> {
    read_keypair_file(path).map_err(|e| anyhow!("failed to read keypair '{}': {e}", path))
}

pub fn parse_pubkey(s: &str) -> Result<Pubkey> {
    Pubkey::from_str(s).map_err(|e| anyhow!("invalid pubkey '{s}': {e}"))
}

pub fn sol_to_lamports(sol: f64) -> Result<u64> {
    if sol < 0.0 {
        return Err(anyhow!("amount cannot be negative"));
    }
    let lamports_f = sol * LAMPORTS_PER_SOL as f64;
    // round‑away floating quirks and check range
    let lamports = lamports_f.round() as u128;
    if lamports > u64::MAX as u128 {
        return Err(anyhow!("amount too large"));
    }
    Ok(lamports as u64)
}

pub async fn transfer_once_async(
    client: Arc<AsyncRpcClient>,
    sender: Arc<Keypair>,
    recipient: Pubkey,
    lamports: u64,
) -> Result<String> {
    let ix = system_instruction::transfer(&sender.pubkey(), &recipient, lamports);
    let blockhash = client.get_latest_blockhash().await?;
    let tx =
        Transaction::new_signed_with_payer(&[ix], Some(&sender.pubkey()), &[&*sender], blockhash);
    let sig = client.send_and_confirm_transaction(&tx).await?;
    info!("transaction confirmed: {sig}");
    Ok(sig.to_string())
}

#[cfg(feature = "sync")]
/// Send a synchronous transfer using the blocking `RpcClient`. Executed in a dedicated
/// blocking thread so that the async runtime is not stalled. Enable via
/// `--features sync`.
pub async fn transfer_once(
    client: std::sync::Arc<RpcClient>,
    sender: std::sync::Arc<Keypair>,
    recipient: Pubkey,
    lamports: u64,
) -> Result<String> {
    let client = Arc::clone(&client);
    let sender = Arc::clone(&sender);
    tokio::task::spawn_blocking(move || {
        let ix = system_instruction::transfer(&sender.pubkey(), &recipient, lamports);
        let blockhash = client.get_latest_blockhash()?;
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&sender.pubkey()),
            &[&*sender],
            blockhash,
        );
        client
            .send_and_confirm_transaction(&tx)
            .map_err(|e| anyhow!(e))
    })
    .await
    .expect("task panicked")
    .map(|sig| {
        info!("transaction confirmed: {sig}");
        sig.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use solana_sdk::signature::Keypair;

    #[test]
    fn sol_to_lamports_valid() {
        // Arrange
        let sol = 1.5_f64;
        let expected_lamports = 1_500_000_000_u64;

        // Act
        let result = sol_to_lamports(sol).unwrap();

        // Assert
        assert_eq!(result, expected_lamports);
    }

    #[test]
    fn sol_to_lamports_negative() {
        // Arrange
        let sol = -0.1_f64;

        // Act
        let result = sol_to_lamports(sol);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn sol_to_lamports_overflow() {
        // Arrange
        let sol = (u64::MAX as f64 / LAMPORTS_PER_SOL as f64) + 1.0;

        // Act
        let result = sol_to_lamports(sol);

        // Assert
        assert!(result.is_err());
    }

    #[test]
    fn rpc_url_mapping() {
        // Arrange is implicit – using known network strings

        // Act & Assert
        assert_eq!(rpc_url("devnet"), "https://api.devnet.solana.com");
        assert_eq!(rpc_url("testnet"), "https://api.testnet.solana.com");
        assert_eq!(
            rpc_url("mainnet-beta"),
            "https://api.mainnet-beta.solana.com"
        );
    }

    #[test]
    fn parse_pubkey_valid() {
        // Arrange
        let kp = Keypair::new();
        let pubkey_str = kp.pubkey().to_string();

        // Act
        let parsed = parse_pubkey(&pubkey_str).unwrap();

        // Assert
        assert_eq!(parsed, kp.pubkey());
    }

    #[test]
    fn parse_pubkey_invalid() {
        // Arrange
        let bad_str = "invalid_pubkey";

        // Act
        let result = parse_pubkey(bad_str);

        // Assert
        assert!(result.is_err());
    }
}
