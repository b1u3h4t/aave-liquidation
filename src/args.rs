use alloy::primitives::Address;
use clap::Parser;
use secrecy::SecretString;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(long, env = "WS_URL", default_value = "ws://localhost:8545")]
    pub ws_url: SecretString,

    #[arg(
        long,
        env = "DATABASE_URL",
        default_value = "postgres://user:password@localhost:5432/liquidator"
    )]
    pub database_url: SecretString,

    #[arg(
        long,
        env = "NETWORK",
        help = "The network ID to connect to (e.g., ethereum, polygon, zksync)"
    )]
    pub network: String,

    #[arg(
        long,
        env = "PROTOCOL",
        help = "The protocol ID (e.g., aave_v3, spark (spark's an aave fork))"
    )]
    pub protocol: String,

    #[arg(long, env = "ACCOUNT_PUBKEY")]
    pub account_pubkey: Address,

    #[arg(long, env = "ACCOUNT_PRIVKEY")]
    pub account_privkey: SecretString,

    #[arg(long, env = "BOT_ADDR")]
    pub bot_addr: Address,
}
