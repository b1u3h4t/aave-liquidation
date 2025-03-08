mod actors;
mod args;
mod configs;
mod consts;
mod contracts;
mod database;
mod run;
mod utils;

use clap::Parser;
use secrecy::ExposeSecret;
use tracing::{debug, info};

use crate::{args::Args, configs::Config, run::run};

pub async fn run_migrations(config: &Config) {
    sqlx::migrate!("./migrations")
        .run(&sqlx::PgPool::connect(&config.database_url).await.unwrap())
        .await
        .unwrap();
}

#[actix_rt::main]
async fn main() {
    tracing_subscriber::fmt().init();

    let args = Args::parse();
    info!(?args);

    let config = Config {
        ws_url: args.ws_url.expose_secret().into(),
        database_url: args.database_url.expose_secret().into(),
        account_pubkey: args.account_pubkey,
        account_privkey: args.account_privkey.expose_secret().into(),
        bot_addr: args.bot_addr,
        target: format!("{}-{}", args.network, args.protocol),
    };
    debug!(?config);

    run_migrations(&config).await;
    run(config).await
}
