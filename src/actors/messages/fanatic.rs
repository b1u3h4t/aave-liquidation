use crate::{actors::Executor, contracts};
use actix::prelude::*;
use alloy::primitives::Address;
use alloy::providers::Provider;

#[derive(Message, Debug, Clone)]
#[rtype(result = "()")]
pub struct UpdateReservePrice {
    pub reserve: Address,
    pub new_price: f64,
}

#[derive(Message, Debug, Clone)]
#[rtype(result = "()")]
pub struct UpdateReserveUser {
    pub reserve: Address,
    pub user_addr: Address,
}

#[derive(Message)]
#[rtype(result = "eyre::Result<()>")]
pub struct DoSmthWithLiquidationCall(pub contracts::aave_v3::PoolContract::LiquidationCall);

#[derive(Message)]
#[rtype(result = "eyre::Result<()>")]
pub struct SuccessfulLiquidation {
    pub user_addr: Address,
}

#[derive(Message)]
#[rtype(result = "eyre::Result<()>")]
pub struct FailedLiquidation {
    pub user_addr: Address,
}

#[derive(Message, Debug, Clone)]
#[rtype(result = "()")]
pub struct SendExecutorAddr<P: Provider + Unpin + Clone + 'static>(pub Addr<Executor<P>>);
