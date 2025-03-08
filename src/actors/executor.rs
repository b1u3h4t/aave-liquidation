use crate::contracts;
use crate::utils::{find_most_liquid_uniswap_pool, health_factor, user_liquidation_data};
use actix::prelude::*;
use alloy::{
    primitives::{Address, Uint},
    providers::Provider,
};

use tracing::{error, info, warn};

use super::messages::executor::LiquidationRequest;
use super::messages::fanatic::SendExecutorAddr;
use super::Database;
use super::Fanatic;
use crate::{
    actors::messages::{
        database,
        fanatic::{FailedLiquidation, SuccessfulLiquidation},
    },
    configs::ExecutorConfig,
};

// executes the liquidations by triggering the liquidator contract
#[derive(Debug, Clone)]
pub struct Executor<P: Provider + Unpin + Clone + 'static> {
    pub provider: P,

    pub db_addr: Addr<Database>,
    pub fanatic_addr: Addr<Fanatic<P>>,
    pub provider_addr: Address,

    // deployed liquidator bot
    pub bot_contract:
        contracts::liquidator::LiquidatoorContract::LiquidatoorContractInstance<(), P>,

    // aave_v3
    pub pool_contract: contracts::aave_v3::PoolContract::PoolContractInstance<(), P>,
    pub datap_contract:
        contracts::aave_v3::DataProviderContract::DataProviderContractInstance<(), P>,

    // uniswap_v3
    pub factory_contract: contracts::uniswap_v3::FactoryContract::FactoryContractInstance<(), P>,
    pub quoter_contract: contracts::uniswap_v3::QuoterContract::QuoterContractInstance<(), P>,
}

impl<P: Provider + Unpin + Clone + 'static> Actor for Executor<P> {
    type Context = Context<Self>;

    // notify `fanatic` of our address
    fn started(&mut self, ctx: &mut Self::Context) {
        self.fanatic_addr.do_send(SendExecutorAddr(ctx.address()));
    }
}

impl<P: Provider + Unpin + Clone + 'static> Executor<P> {
    pub async fn new(config: ExecutorConfig<P>) -> eyre::Result<Executor<P>> {
        let target_network = config.target.split('-').next().unwrap();
        let uniswap_target = format!("{}-uniswap_v3", target_network);

        let aave_contracts = config
            .db_addr
            .send(database::GetProtocolContracts(config.target))
            .await??;
        let uniswap_contracts = config
            .db_addr
            .send(database::GetProtocolContracts(uniswap_target))
            .await??;

        let (
            bot_contract,
            factory_contract,
            quoter_contract,
            pool_contract,
            datap_contract,
            provider_addr,
        ) = match (
            uniswap_contracts.get("UniswapV3Factory"),
            uniswap_contracts.get("QuoterV2"),
            aave_contracts.get("Pool"),
            aave_contracts.get("UiPoolDataProviderV3"),
            aave_contracts.get("PoolAddressesProvider"),
        ) {
            (
                Some(factory_addr),
                Some(quoter_addr),
                Some(pool_addr),
                Some(datap_addr),
                Some(provider_addr),
            ) => (
                contracts::liquidator::LiquidatoorContract::new(
                    config.bot_addr,
                    config.provider.clone(),
                ),
                contracts::uniswap_v3::FactoryContract::new(*factory_addr, config.provider.clone()),
                contracts::uniswap_v3::QuoterContract::new(*quoter_addr, config.provider.clone()),
                contracts::aave_v3::PoolContract::new(*pool_addr, config.provider.clone()),
                contracts::aave_v3::DataProviderContract::new(*datap_addr, config.provider.clone()),
                *provider_addr,
            ),
            _ => return Err(eyre::eyre!("Missing required contract addresses")),
        };

        Ok(Executor {
            provider: config.provider,

            db_addr: config.db_addr,
            fanatic_addr: config.fanatic_addr,
            provider_addr,

            bot_contract,
            pool_contract,
            datap_contract,
            factory_contract,
            quoter_contract,
        })
    }
}

impl<P: Provider + Unpin + Clone + 'static> Handler<LiquidationRequest> for Executor<P> {
    type Result = ResponseFuture<eyre::Result<()>>;

    fn handle(&mut self, msg: LiquidationRequest, _ctx: &mut Self::Context) -> Self::Result {
        let db_addr = self.db_addr.clone();
        let fanatic_addr = self.fanatic_addr.clone();
        let provider = self.provider.clone();
        let provider_addr = self.provider_addr;
        let bot_contract = self.bot_contract.clone();
        let pool_contract = self.pool_contract.clone();
        let datap_contract = self.datap_contract.clone();
        let factory_contract = self.factory_contract.clone();

        let fut = async move {
            let health_factor = health_factor(&pool_contract, msg.user_address).await;

            if let Some(hf) = health_factor {
                info!(user = ?msg.user_address, health_factor = ?hf, "user is undercollateralized");

                let indices = db_addr
                    .send(database::GetReservesLiquidityIndices(format!(
                        "{}-{}",
                        msg.network, msg.protocol
                    )))
                    .await??;

                let (debt_asset, collateral_asset, debt_to_cover) = user_liquidation_data(
                    &datap_contract,
                    provider_addr,
                    msg.user_address,
                    &indices,
                )
                .await?;

                let (_pool_addr, fee) = find_most_liquid_uniswap_pool(
                    &provider,
                    &factory_contract,
                    collateral_asset,
                    debt_asset,
                )
                .await?;

                match bot_contract
                    .liquidatoor(
                        debt_asset,
                        collateral_asset,
                        msg.user_address,
                        debt_to_cover,
                        Uint::from(fee),
                    )
                    .call()
                    .await
                {
                    Ok(_) => {
                        fanatic_addr
                            .send(SuccessfulLiquidation {
                                user_addr: msg.user_address,
                            })
                            .await?;
                    }
                    Err(e) => {
                        fanatic_addr
                            .send(FailedLiquidation {
                                user_addr: msg.user_address,
                            })
                            .await?;
                        error!("Liquidation failed: {}", e);
                    }
                }
            }

            Ok(())
        };

        Box::pin(fut)
    }
}
