use crate::contracts;
use crate::utils::norm;
use actix::prelude::*;
use alloy::{primitives::Address, providers::Provider, rpc::types::Filter, sol_types::SolEvent};
use futures_util::StreamExt;
use tracing::{error, info, warn};

use crate::{
    actors::{
        messages::{
            database,
            fanatic::{DoSmthWithLiquidationCall, UpdateReservePrice, UpdateReserveUser},
            follower::{SendFanaticAddr, StartListeningForEvents, StartListeningForOraclePrices},
        },
        Database, Fanatic,
    },
    configs::FollowerConfig,
    consts::RAY,
};

#[derive(Debug, Clone)]
pub struct Follower<P: Provider + Unpin + Clone + 'static> {
    provider: P,
    filter: Filter,

    provider_addr: Address,
    db_addr: Addr<Database>,
    fanatic_addr: Option<Addr<Fanatic<P>>>,

    pool_contract: contracts::aave_v3::PoolContract::PoolContractInstance<(), P>,
    datap_contract: contracts::aave_v3::DataProviderContract::DataProviderContractInstance<(), P>,

    target: String,
}

impl<P: Provider + Unpin + Clone + 'static> Actor for Follower<P> {
    type Context = Context<Self>;

    fn started(&mut self, _: &mut Self::Context) {
        info!("[started] Follower");
    }
}

impl<P: Provider + Unpin + Clone + 'static> Handler<SendFanaticAddr<P>> for Follower<P> {
    type Result = ();

    fn handle(&mut self, msg: SendFanaticAddr<P>, _: &mut Context<Self>) -> Self::Result {
        self.fanatic_addr = Some(msg.0);
    }
}

impl<P: Provider + Unpin + Clone + 'static> Handler<StartListeningForOraclePrices> for Follower<P> {
    type Result = ();

    fn handle(
        &mut self,
        _: StartListeningForOraclePrices,
        ctx: &mut Context<Self>,
    ) -> Self::Result {
        self.listen_oracle_prices(ctx);
    }
}

impl<P: Provider + Unpin + Clone + 'static> Handler<StartListeningForEvents> for Follower<P> {
    type Result = ();

    fn handle(&mut self, _: StartListeningForEvents, ctx: &mut Context<Self>) -> Self::Result {
        self.listen_events(ctx);
    }
}

impl<P: Provider + Unpin + Clone + 'static> Follower<P> {
    pub async fn new(config: FollowerConfig<P>) -> eyre::Result<Self> {
        let contracts = config
            .db_addr
            .send(database::GetProtocolContracts(config.target.clone()))
            .await??;

        match (
            contracts.get("Pool"),
            contracts.get("PoolAddressesProvider"),
            contracts.get("UiPoolDataProviderV3"),
        ) {
            (Some(pool_addr), Some(provider_addr), Some(datap_addr)) => {
                let filter = Filter::new().address(*pool_addr).events(vec![
                    contracts::aave_v3::PoolContract::LiquidationCall::SIGNATURE,
                    contracts::aave_v3::PoolContract::Supply::SIGNATURE,
                    contracts::aave_v3::PoolContract::Borrow::SIGNATURE,
                    contracts::aave_v3::PoolContract::Repay::SIGNATURE,
                    contracts::aave_v3::PoolContract::Withdraw::SIGNATURE,
                    contracts::aave_v3::PoolContract::ReserveDataUpdated::SIGNATURE,
                ]);

                let pool_contract =
                    contracts::aave_v3::PoolContract::new(*pool_addr, config.provider.clone());
                let datap_contract = contracts::aave_v3::DataProviderContract::new(
                    *datap_addr,
                    config.provider.clone(),
                );

                Ok(Self {
                    provider: config.provider,
                    filter,
                    db_addr: config.db_addr,
                    fanatic_addr: None,
                    provider_addr: *provider_addr,
                    pool_contract,
                    datap_contract,
                    target: config.target.clone(),
                })
            }
            _ => {
                error!("Missing required contract addresses in database");
                Err(eyre::eyre!("Required contract addresses not found"))
            }
        }
    }

    fn listen_oracle_prices(&self, ctx: &mut Context<Self>) {
        let provider = self.provider.clone();
        let db_addr = self.db_addr.clone();
        let fanatic_addr = self.fanatic_addr.clone();
        let target = self.target.clone();

        let fut = async move {
            // Get the list of aggregator addresses to monitor.
            let aggregators = db_addr
                .send(database::GetAggregatorMapping(target.clone()))
                .await
                .unwrap();
            info!(
                ?aggregators,
                "listening for oracle price events from aggregators [{}] [aggregator => reserve]",
                aggregators.keys().len()
            );

            // Build a filter for events coming from the aggregator addresses.
            // In this example we filter for logs matching the AnswerUpdated event signature
            let filter = Filter::new()
                .address(aggregators.keys().cloned().collect::<Vec<Address>>())
                .events(vec![
                    contracts::chainlink::EACAggregatorProxyContract::AnswerUpdated::SIGNATURE,
                ]);

            let sub = provider.subscribe_logs(&filter).await.unwrap();
            let mut stream = sub.into_stream();

            while let Some(log) = stream.next().await {
                if let Ok(event) =
                    contracts::chainlink::EACAggregatorProxyContract::AnswerUpdated::decode_log(
                        &log.inner, true,
                    )
                {
                    let price = oracle_price(&provider, event.address).await;
                    info!(aggregator=?event.address, ?price, "new price from aggregator");

                    // Update the DB reserve price, and send a message to calculate the affected
                    // users' health_factor.
                    // TODO: perhaps this should be done in the `fanatic` actor
                    //
                    // get the reserve from the aggregator => reserve HashMap
                    if let Some(reserve) = aggregators.get(&event.address) {
                        db_addr
                            .send(database::UpdateOraclePrice {
                                reserve: *reserve,
                                target: target.clone(),
                                price,
                            })
                            .await
                            .unwrap()
                            .unwrap();

                        fanatic_addr
                            .clone()
                            .expect("no fanatic_addr found")
                            .send(UpdateReservePrice {
                                reserve: *reserve,
                                new_price: price,
                            })
                            .await
                            .unwrap();
                    }
                }
            }
        };

        ctx.spawn(fut.into_actor(self));
    }

    /// Listen for realtime action happening in the lending pools
    /// ..and act accordingly
    fn listen_events(&self, ctx: &mut Context<Self>) {
        let provider = self.provider.clone();
        let filter = self.filter.clone();

        let db_addr = self.db_addr.clone();
        let fanatic_addr = self.fanatic_addr.clone();

        let fut = async move {
            let sub = provider.subscribe_logs(&filter).await.unwrap();
            let mut stream = sub.into_stream();

            while let Some(log) = stream.next().await {
                let signature = log.topic0().unwrap();

                match signature {
                    hash if *hash
                        == contracts::aave_v3::PoolContract::LiquidationCall::SIGNATURE_HASH =>
                    {
                        if let Ok(event) =
                            contracts::aave_v3::PoolContract::LiquidationCall::decode_log(
                                &log.inner, true,
                            )
                        {
                            info!(?event.user, "liquidation_event_handler");
                            fanatic_addr
                                .clone()
                                .expect("no fanatic_addr found")
                                .send(DoSmthWithLiquidationCall(event.data))
                                .await
                                .unwrap()
                                .unwrap();
                        }
                    }
                    hash if *hash == contracts::aave_v3::PoolContract::Supply::SIGNATURE_HASH => {
                        if let Ok(event) =
                            contracts::aave_v3::PoolContract::Supply::decode_log(&log.inner, true)
                        {
                            info!(reserve = ?event.reserve, user = ?event.user, amount = ?event.amount, "supply_event_handler");
                            fanatic_addr
                                .clone()
                                .expect("no fanatic_addr found")
                                .send(UpdateReserveUser {
                                    reserve: event.reserve,
                                    user_addr: event.user,
                                })
                                .await
                                .unwrap();
                        }
                    }
                    hash if *hash == contracts::aave_v3::PoolContract::Borrow::SIGNATURE_HASH => {
                        if let Ok(event) =
                            contracts::aave_v3::PoolContract::Borrow::decode_log(&log.inner, true)
                        {
                            info!(reserve = ?event.reserve, user = ?event.user, amount = ?event.amount, "borrow_event_handler");
                            fanatic_addr
                                .clone()
                                .expect("no fanatic_addr found")
                                .send(UpdateReserveUser {
                                    reserve: event.reserve,
                                    user_addr: event.user,
                                })
                                .await
                                .unwrap();
                        }
                    }
                    hash if *hash == contracts::aave_v3::PoolContract::Repay::SIGNATURE_HASH => {
                        if let Ok(event) =
                            contracts::aave_v3::PoolContract::Repay::decode_log(&log.inner, true)
                        {
                            info!(reserve = ?event.reserve, user = ?event.user, amount = ?event.amount, "repay_event_handler");
                            fanatic_addr
                                .clone()
                                .expect("no fanatic_addr found")
                                .send(UpdateReserveUser {
                                    reserve: event.reserve,
                                    user_addr: event.user,
                                })
                                .await
                                .unwrap();
                        }
                    }
                    hash if *hash == contracts::aave_v3::PoolContract::Withdraw::SIGNATURE_HASH => {
                        if let Ok(event) =
                            contracts::aave_v3::PoolContract::Withdraw::decode_log(&log.inner, true)
                        {
                            info!(reserve = ?event.reserve, user = ?event.user, amount = ?event.amount, "withdraw_event_handler");
                            fanatic_addr
                                .clone()
                                .expect("no fanatic_addr found")
                                .send(UpdateReserveUser {
                                    reserve: event.reserve,
                                    user_addr: event.user,
                                })
                                .await
                                .unwrap();
                        }
                    }
                    hash if *hash
                        == contracts::aave_v3::PoolContract::ReserveDataUpdated::SIGNATURE_HASH =>
                    {
                        if let Ok(event) =
                            contracts::aave_v3::PoolContract::ReserveDataUpdated::decode_log(
                                &log.inner, true,
                            )
                        {
                            info!(reserve = ?event.reserve, liq_rate = ?event.liquidityRate,
                                liq_index = ?event.liquidityIndex, stable_borrow_rate = ?event.stableBorrowRate,
                                variable_borrow_rate = ?event.variableBorrowRate, "reserve_update_event_handler");
                            match db_addr
                                .send(database::UpsertReservesStats(vec![
                                    database::UpsertReserveStats {
                                        reserve: event.reserve.to_string(),
                                        liquidity_rate: norm(
                                            event.liquidityRate,
                                            Some(100.0 / RAY),
                                        )
                                        .unwrap(),
                                        variable_borrow_rate: norm(
                                            event.variableBorrowRate,
                                            Some(100.0 / RAY),
                                        )
                                        .unwrap(),
                                        liquidity_index: norm(
                                            event.liquidityIndex,
                                            Some(1.0 / RAY),
                                        )
                                        .unwrap(),
                                        variable_borrow_index: norm(
                                            event.variableBorrowIndex,
                                            Some(1.0 / RAY),
                                        )
                                        .unwrap(),
                                    },
                                ]))
                                .await
                            {
                                Ok(Ok(_)) => (),
                                Ok(Err(e)) => {
                                    error!(?event.reserve, error = ?e, "Failed to update reserve data")
                                }
                                Err(e) => {
                                    error!(?event.reserve, error = ?e, "Failed to send reserve update")
                                }
                            }
                        }
                    }
                    _ => warn!(?signature, ?log, "unknown event"),
                }
            }
        };

        ctx.spawn(fut.into_actor(self));
    }
}

pub async fn oracle_price<P: Provider + Clone>(provider: P, addr: Address) -> f64 {
    let contract = contracts::chainlink::OffchainAggregatorContract::new(addr, &provider);

    let latest_answer = contract
        .latestAnswer()
        .call()
        .await
        .unwrap()
        ._0
        .to_string()
        .parse::<f64>()
        .unwrap();

    // Try to get decimals using CLRatePriceCapAdapter, if it fails, use CLSynchronicityPriceAdapterPegToBase.
    let decimals = match contract.decimals().call().await {
        Ok(resp) => resp._0,
        Err(e) => {
            warn!(error = ?e, "Failed to get decimals from CLRatePriceCapAdapter, trying CLSynchronicityPriceAdapterPegToBase");
            let synch_adapter =
                contracts::chainlink::CLSynchronicityPriceAdapterPegToBaseContract::new(
                    addr, &provider,
                );
            synch_adapter.DECIMALS().call().await.unwrap()._0
        }
    };

    latest_answer / 10_f64.powi(decimals as i32)
}
