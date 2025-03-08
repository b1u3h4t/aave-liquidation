use std::sync::Arc;

use actix::prelude::*;
use alloy::{
    network::EthereumWallet,
    providers::{ProviderBuilder, WsConnect},
    signers::local::PrivateKeySigner,
};
use sqlx::postgres::PgPoolOptions;
use tokio::signal::unix::{signal, SignalKind};

use crate::{
    actors::{Database, Executor, Fanatic, Follower},
    configs::{Config, DatabaseConfig, ExecutorConfig, FanaticConfig, FollowerConfig},
};

#[derive(Message)]
#[rtype(result = "eyre::Result<()>")]
pub struct Shutdown;

pub async fn run(config: Config) {
    let mut interrupt =
        signal(SignalKind::interrupt()).expect("Unable to initialise interrupt signal handler");
    let mut terminate =
        signal(SignalKind::terminate()).expect("Unable to initialise termination signal handler");

    /* Init any clients, db conns etc. */
    let (provider_with_wallet, db_pool) = {
        (
            ProviderBuilder::new()
                .wallet(EthereumWallet::from(
                    config.account_privkey.parse::<PrivateKeySigner>().unwrap(),
                ))
                .on_ws(WsConnect::new(config.ws_url))
                .await
                .expect("Unable to initialise provider with wallet"),
            Arc::new(
                PgPoolOptions::new()
                    .connect(&config.database_url)
                    .await
                    .expect("Unable to establish database connection"),
            ),
        )
    };

    /* Spin up the database actor */
    let db_addr = Database::new(DatabaseConfig {
        pool: db_pool.clone(),
    })
    .await
    .start();

    /* Spin up the follower actor */
    let follower_addr = Follower::new(FollowerConfig {
        provider: provider_with_wallet.clone(),
        db_addr: db_addr.clone(),
        target: config.target.clone(),
    })
    .await
    .expect("Unable to initialise follower actor")
    .start();

    /* Spin up the fanatic actor */
    let fanatic_addr = Fanatic::new(FanaticConfig {
        provider: provider_with_wallet.clone(),
        db_addr: db_addr.clone(),
        follower_addr: follower_addr.clone(),
        target: config.target.clone(),
    })
    .await
    .expect("Unable to initialise Fanatic actor")
    .init()
    .await
    .expect("Unable to initialise Fanatic actor")
    .start();

    /* Spin up the alpha executor actor */
    let _ = Executor::new(ExecutorConfig {
        provider: provider_with_wallet.clone(),
        db_addr: db_addr.clone(),
        fanatic_addr: fanatic_addr.clone(),
        bot_addr: config.bot_addr,
        target: config.target,
    })
    .await
    .expect("Unable to initialise Executor actor")
    .start();

    tokio::select! {
        _ = interrupt.recv() => {
            fanatic_addr.send(Shutdown).await.unwrap().unwrap();
            System::current().stop();
        }
        _ = terminate.recv() => {
            fanatic_addr.send(Shutdown).await.unwrap().unwrap();
            System::current().stop();
        }
    }
}
