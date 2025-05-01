# Solana Geyser Auto-Transfer CLI

**Automate SOL payouts triggered by new finalized blocks on the Solana network.**

This tool connects to the public Yellowstone Geyser gRPC endpoint (`grpc.ny.shyft.to`), listens for `Finalized` block updates and—optionally at a given cadence—pushes a SOL transfer from your sender wallet to a predefined recipient.

_Repository URL_: <https://github.com/deadraid/solana_geyser>

---

## Features

| Feature                             | Description                                                                                        |
| ----------------------------------- | -------------------------------------------------------------------------------------------------- |
| **Async runtime**                   | Fully non-blocking: gRPC stream (Yellowstone) + RPC client (`solana-client::nonblocking`).         |
| **Rate control**                    | `--every N` — send on each N-th finalized block; `--concurrency K` — limit simultaneous transfers. |
| **Automatic reconnect & ping/pong** | Keeps the gRPC stream healthy and transparently reconnects.                                        |
| **Config-driven**                   | Single `config.yaml` with Geyser credentials, network, keypair paths and amount.                   |

> **Security notice** The sender keypair is read from a local file; protect it as you would any private key.

---

## Quick start

### Prerequisites

- Rust ≥ 1.70 (`rustup.rs`)
- Solana CLI (`solana-install init`)

### 1. Clone & build

```bash
git clone https://github.com/deadraid/solana_geyser.git
cd solana_geyser
cargo build --release
```

### 2. Configure

Create keypairs (skip if you already have them):

```bash
solana-keygen new -o sender_keypair.json
solana-keygen new -o recipient_keypair.json # or use an existing address
```

Edit `config.yaml`:

```yaml
geyser:
  url: "https://grpc.ny.shyft.to"
  api_key: "YOUR-SHYFT-TOKEN"

solana:
  network: "devnet" # devnet | testnet | mainnet-beta
  sender_keypair_path: "sender_keypair.json"
  recipient_wallet: "RECIPIENT-PUBKEY"
  amount: 0.01 # SOL to send each time
```

Fund the sender (devnet example):

```bash
solana airdrop 2 $(solana-keygen pubkey sender_keypair.json) --url https://api.devnet.solana.com
```

### 3. Run

```bash
./target/release/solana_geyser subscribe \
    --every 1         # every block (default)
    --concurrency 4   # max 4 simultaneous in-flight txs
```

Optional flags:

```bash
-c, --config <FILE>   # custom yaml path (default: ./config.yaml)
--every <N>           # send on every N-th block (default: 1)
--concurrency <K>     # limit parallel txs (default: 4)
```

Logs include block slot, hash and resulting transaction signature.

---

## How it works

1. Create a bidirectional gRPC subscription via Yellowstone SDK, requesting only block-level events.
2. Wait for each _finalized_ block update. Every `N`-th event is selected according to `--every`.
3. Transfer task is spawned under a `tokio::Semaphore` (back-pressure). Uses `solana-client::nonblocking` to sign & submit in one round-trip.
4. gRPC Ping/Pong keeps the stream alive; on error the client auto-reconnects after 5 s.

---

## Advanced

### Enable synchronous RPC (fallback)

The async path covers 99 % cases. If you need the legacy blocking client, enable the **sync** feature flag:

```toml
[features]
sync = []
```

and build with `cargo build --release --features sync`. See `solana.rs::transfer_once` for the implementation (disabled by default).

---

---

## License

MIT License
