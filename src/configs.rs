use std::sync::Arc;

use actix::Addr;
use alloy::{primitives::Address, providers::Provider};
use sqlx::PgPool;

use crate::actors::{Bum, Database, Fanatic};

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
pub struct BumConfig<P: Provider + Unpin + Clone + 'static> {
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
    pub bum_addr: Addr<Bum<P>>,
    pub target: String,
}

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub pool: Arc<PgPool>,
}
