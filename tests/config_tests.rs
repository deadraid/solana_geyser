use std::fs::File;
use std::io::Write;

use solana_geyser::config::{read_config, Config};

#[test]
fn read_config_valid_yaml() {
    // Arrange
    let yaml = r#"
geyser:
  url: "https://grpc.ny.shyft.to"
  api_key: "dummy_api_key"
solana:
  network: "devnet"
  sender_keypair_path: "/path/to/sender.json"
  recipient_wallet: "11111111111111111111111111111111"
  amount: 0.5
"#;
    let mut file_path = std::env::temp_dir();
    file_path.push("cfg.yaml");
    let mut file = File::create(&file_path).expect("failed create temp file");
    file.write_all(yaml.as_bytes()).unwrap();

    // Act
    let cfg: Config = read_config(&file_path).expect("should parse");

    // Assert
    assert_eq!(cfg.geyser.url, "https://grpc.ny.shyft.to");
    assert_eq!(cfg.geyser.api_key, "dummy_api_key");
    assert_eq!(cfg.solana.network, "devnet");
    assert_eq!(cfg.solana.amount, 0.5);
}
