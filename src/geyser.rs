use crate::{config::Config, solana};
use anyhow::Result;
use futures::stream::StreamExt;
use futures::SinkExt;
use solana_client::nonblocking::rpc_client::RpcClient as AsyncRpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::signer::Signer;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tracing::{error, info};
use yellowstone_grpc_client::GeyserGrpcClient;
use yellowstone_grpc_proto::{
    prelude::{
        subscribe_update::UpdateOneof, CommitmentLevel, SubscribeRequest,
        SubscribeRequestFilterBlocks,
    },
    tonic::transport::ClientTlsConfig,
};

/// Connects, subscribes to new finalized blocks using Yellowstone, and sends a SOL transfer for each.
pub async fn subscribe_and_transfer(cfg: Config, every: u64, concurrency: usize) -> Result<()> {
    // Setup Solana client details once
    let rpc_url = solana::rpc_url(&cfg.solana.network).to_string();
    let rpc = AsyncRpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());
    let rpc = Arc::new(rpc);
    let sender = solana::load_keypair(&cfg.solana.sender_keypair_path)?;
    let sender = Arc::new(sender);
    let recipient = solana::parse_pubkey(&cfg.solana.recipient_wallet)?;
    let lamports = solana::sol_to_lamports(cfg.solana.amount)?;

    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));
    let mut block_counter: u64 = 0;

    loop {
        info!("connecting to geyser => {}", cfg.geyser.url);

        let mut client = match GeyserGrpcClient::build_from_shared(cfg.geyser.url.clone())?
            .x_token(Some(cfg.geyser.api_key.clone()))?
            .connect_timeout(Duration::from_secs(10))
            // Set a timeout for receiving messages to detect stalls
            .timeout(Duration::from_secs(60))
            .tls_config(
                ClientTlsConfig::new()
                    .domain_name("grpc.ny.shyft.to") // Required for TLS certificate validation
                    .with_native_roots(),
            )?
            .connect()
            .await
        {
            Ok(client) => client,
            Err(e) => {
                error!(?e, "connection failed, retrying in 5s");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        let mut blocks_map = HashMap::new();
        blocks_map.insert(
            "block_updates".to_string(), // Client-defined subscription name
            SubscribeRequestFilterBlocks {
                account_include: vec![],
                // Minimize data transfer: only need notification of new blocks
                include_transactions: Some(false),
                include_accounts: Some(false),
                include_entries: Some(false),
            },
        );

        let subscribe_request = SubscribeRequest {
            blocks: blocks_map,
            commitment: Some(CommitmentLevel::Finalized as i32), // Ensure we react to finalized blocks
            accounts: HashMap::new(),
            slots: HashMap::new(),
            transactions: HashMap::new(),
            transactions_status: HashMap::new(),
            blocks_meta: HashMap::new(),
            entry: HashMap::new(),
            accounts_data_slice: vec![],
            ping: None,
            from_slot: None,
        };

        info!("(re)subscribing to block stream...");
        let (mut subscribe_tx, mut stream) =
            match client.subscribe_with_request(Some(subscribe_request)).await {
                Ok((tx, stream)) => (tx, stream),
                Err(e) => {
                    error!(?e, "subscribe failed, retrying in 5s");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
            };

        info!(
            sender = %sender.pubkey(),
            recipient = %recipient,
            lamports,
            "ready – waiting for finalized blocks"
        );

        loop {
            match stream.next().await {
                Some(Ok(message)) => {
                    match message.update_oneof {
                        Some(UpdateOneof::Block(block_update)) => {
                            block_counter += 1;
                            if block_counter % every != 0 {
                                continue;
                            }
                            info!(
                                slot = block_update.slot,
                                block_hash = %bs58::encode(block_update.blockhash.as_bytes()).into_string(),
                                "new finalized block detected – sending transfer"
                            );

                            let rpc_clone = Arc::clone(&rpc);
                            let sender_clone = Arc::clone(&sender);
                            let sem_clone = Arc::clone(&semaphore);

                            // Execute the async RPC call in a separate task with concurrency control
                            // to avoid stalling the main gRPC message loop.
                            tokio::spawn(async move {
                                // try_acquire_owned avoids await and skip if busy
                                let permit = match sem_clone.clone().try_acquire_owned() {
                                    Ok(p) => p,
                                    Err(_) => {
                                        info!("transfer backlog full, skipping block");
                                        return;
                                    }
                                };

                                if let Err(e) = solana::transfer_once_async(
                                    rpc_clone,
                                    sender_clone,
                                    recipient,
                                    lamports,
                                )
                                .await
                                {
                                    error!(?e, "transfer failed");
                                }
                                drop(permit);
                            });
                        }
                        Some(UpdateOneof::Ping(_)) => {
                            info!("received ping, sending pong");
                            // Respond to server Ping to keep the connection healthy
                            if let Err(e) = subscribe_tx
                                .send(SubscribeRequest {
                                    ping: Some(
                                        yellowstone_grpc_proto::prelude::SubscribeRequestPing {
                                            id: 1,
                                        },
                                    ),
                                    ..Default::default()
                                })
                                .await
                            {
                                error!(?e, "failed to send pong");
                            }
                        }
                        Some(UpdateOneof::Pong(_)) => {
                            info!("received pong");
                        }
                        _ => {}
                    }
                }
                Some(Err(e)) => {
                    error!(?e, "error receiving stream update, reconnecting");
                    break;
                }
                None => {
                    info!("stream closed by server, reconnecting");
                    break;
                }
            }
        }

        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
