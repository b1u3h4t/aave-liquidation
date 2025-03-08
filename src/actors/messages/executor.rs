use actix::prelude::*;
use alloy::{primitives::Address, providers::Provider};

use crate::actors::Fanatic;

#[derive(Message, Debug, Clone)]
#[rtype(result = "eyre::Result<()>")]
pub struct LiquidationRequest {
    pub user_address: Address,
    pub network: String,
    pub protocol: String,
}
