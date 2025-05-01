use anyhow::Result;
use serde::Deserialize;
use std::{fs::File, io::Read, path::Path};

#[derive(Debug, Deserialize)]
pub struct Config {
    pub geyser: GeyserConfig,
    pub solana: SolanaConfig,
}

#[derive(Debug, Deserialize)]
pub struct GeyserConfig {
    pub url: String,
    pub api_key: String,
}

#[derive(Debug, Deserialize)]
pub struct SolanaConfig {
    pub network: String, // devnet | testnet | mainnet-beta
    pub sender_keypair_path: String,
    pub recipient_wallet: String,
    pub amount: f64, // SOL amount per transfer
}

pub fn read_config<P: AsRef<Path>>(path: P) -> Result<Config> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    Ok(serde_yaml::from_str(&contents)?)
}
