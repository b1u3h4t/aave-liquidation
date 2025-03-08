use std::sync::Arc;

use actix::Addr;
use alloy::{primitives::Address, providers::Provider};
use sqlx::PgPool;

use crate::actors::{Database, Fanatic, Follower};

#[derive(Debug, Clone)]
pub struct Config {
    pub ws_url: String,
    pub database_url: String,
    pub target: String,
    pub account_pubkey: Address,
    pub account_privkey: String,
    pub bot_addr: Address,
}

#[derive(Debug, Clone)]
pub struct FollowerConfig<P: Provider + Unpin + Clone + 'static> {
    pub provider: P,
    pub db_addr: Addr<Database>,
    pub target: String,
}

#[derive(Debug, Clone)]
pub struct ExecutorConfig<P: Provider + Unpin + Clone + 'static> {
    pub provider: P,
    pub db_addr: Addr<Database>,
    pub fanatic_addr: Addr<Fanatic<P>>,
    pub bot_addr: Address,
    pub target: String,
}

#[derive(Debug, Clone)]
pub struct FanaticConfig<P: Provider + Unpin + Clone + 'static> {
    pub provider: P,
    pub db_addr: Addr<Database>,
    pub follower_addr: Addr<Follower<P>>,
    pub target: String,
}

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub pool: Arc<PgPool>,
}
