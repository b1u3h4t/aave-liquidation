use std::{collections::HashMap, sync::Arc};

use crate::actors::messages::executor::LiquidationRequest;
use crate::actors::Database;
use crate::contracts;
use crate::utils::{health_factor, norm, user_positions};
use actix::prelude::*;
use alloy::{primitives::Address, providers::Provider};
use sqlx::types::time::OffsetDateTime;
use tokio::{sync::Mutex, time::Duration};
use tracing::{error, info, warn};

use super::messages::fanatic::SendExecutorAddr;
use super::Executor;
use super::{
    follower::oracle_price,
    messages::fanatic::{FailedLiquidation, SuccessfulLiquidation},
};
use crate::{
    actors::{
        messages::{
            database,
            fanatic::{DoSmthWithLiquidationCall, UpdateReservePrice, UpdateReserveUser},
            follower::{SendFanaticAddr, StartListeningForEvents, StartListeningForOraclePrices},
        },
        Bum,
    },
    configs::FanaticConfig,
    consts::RAY,
    run::Shutdown,
};

#[derive(Debug, Clone)]
pub struct Fanatic<P: Provider + Unpin + Clone + 'static> {
    provider: P,

    db_addr: Addr<Database>,
    bum_addr: Addr<Bum<P>>,
    executor_addr: Option<Addr<Executor<P>>>,

    pool_contract: contracts::aave_v3::PoolContract::PoolContractInstance<(), P>,
    datap_contract: contracts::aave_v3::DataProviderContract::DataProviderContractInstance<(), P>,
    addressp_contract:
        contracts::aave_v3::AddressProviderContract::AddressProviderContractInstance<(), P>,

    users: Arc<Mutex<HashMap<Address, database::UserData>>>,
    reserves: Arc<Mutex<HashMap<Address, database::ReserveData>>>,

    target: String,
    protocol_details_id: i32,
}

impl<P: Provider + Unpin + Clone + 'static> Actor for Fanatic<P> {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let addr = ctx.address();
        let bum_addr = self.bum_addr.clone();

        let fut = async move {
            bum_addr.send(SendFanaticAddr(addr)).await.unwrap();
            bum_addr.send(StartListeningForOraclePrices).await.unwrap();
            bum_addr.send(StartListeningForEvents).await.unwrap();
        };

        ctx.spawn(fut.into_actor(self));

        ctx.run_interval(Duration::from_secs(60), |actor, ctx| {
            let users = actor.users.clone();
            let db_addr = actor.db_addr.clone();
            let datap_contract = actor.datap_contract.clone();
            let addressp_address = *actor.addressp_contract.address();
            let protocol_details_id = actor.protocol_details_id;
            let target = actor.target.clone();

            let fut = async move {
                if let Err(e) = update_recent_users(
                    &users,
                    &db_addr,
                    protocol_details_id,
                    &datap_contract,
                    &addressp_address,
                    target,
                )
                .await
                {
                    error!("Failed to update recent users: {}", e);
                }
            };

            ctx.spawn(fut.into_actor(actor));
        });
    }
}

impl<P: Provider + Unpin + Clone + 'static> Handler<SendExecutorAddr<P>> for Fanatic<P> {
    type Result = ();

    fn handle(&mut self, msg: SendExecutorAddr<P>, _: &mut Context<Self>) -> Self::Result {
        self.executor_addr = Some(msg.0);
    }
}

async fn update_recent_users<P: Provider + Clone>(
    users: &Arc<Mutex<HashMap<Address, database::UserData>>>,
    db_addr: &Addr<Database>,
    protocol_details_id: i32,
    datap_contract: &contracts::aave_v3::DataProviderContract::DataProviderContractInstance<(), P>,
    addressp_address: &Address,
    target: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let secs = 70;
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let indices = db_addr
        .send(database::GetReservesLiquidityIndices(target))
        .await??;

    let users_guard = users.lock().await;
    let total_users = users_guard.len();
    info!("Starting update for {} total users", total_users);

    let mut recent_updates = 0;
    for (user, data) in users_guard.iter() {
        if now - data.last_update < secs {
            let user_positions =
                user_positions(datap_contract, addressp_address, user, &indices).await?;
            recent_updates += 1;

            info!(
                "Updating user {} with HF {} (timestamp: {})",
                user, data.health_factor, data.last_update
            );
            db_addr
                .send(database::UpsertUserData {
                    address: user.to_string(),
                    health_factor: data.health_factor,
                    protocol_details_id,
                    positions: user_positions,
                })
                .await??;
        }
    }

    info!(
        "Updated {}/{} users with recent data (<{}s old)",
        recent_updates, total_users, secs
    );
    Ok(())
}

impl<P: Provider + Unpin + Clone + 'static> Fanatic<P> {
    pub async fn new(config: FanaticConfig<P>) -> eyre::Result<Fanatic<P>> {
        let protocol_details_id = config
            .db_addr
            .send(database::GetProtocolDetailsId(config.target.clone()))
            .await??;
        let contracts_addr = config
            .db_addr
            .send(database::GetProtocolContracts(config.target.clone()))
            .await??;

        let (pool_contract, datap_contract, addressp_contract) = match (
            contracts_addr.get("Pool"),
            contracts_addr.get("UiPoolDataProviderV3"),
            contracts_addr.get("PoolAddressesProvider"),
        ) {
            (Some(pool_addr), Some(datap_addr), Some(provider_addr)) => (
                contracts::aave_v3::PoolContract::new(*pool_addr, config.provider.clone()),
                contracts::aave_v3::DataProviderContract::new(*datap_addr, config.provider.clone()),
                contracts::aave_v3::AddressProviderContract::new(
                    *provider_addr,
                    config.provider.clone(),
                ),
            ),
            _ => return Err(eyre::eyre!("Missing required contract addresses")),
        };

        let (users, prices) = config
            .db_addr
            .send(database::GetReservesUsers(config.target.clone()))
            .await??;

        info!("Reserves Users: {:?}", users);
        info!("Reserves Prices: {:#?}", prices);

        Ok(Fanatic {
            provider: config.provider,
            db_addr: config.db_addr,
            bum_addr: config.bum_addr,
            executor_addr: None,
            pool_contract,
            datap_contract,
            addressp_contract,
            users: Arc::new(Mutex::new(users)),
            reserves: Arc::new(Mutex::new(prices)),
            target: config.target.clone(),
            protocol_details_id,
        })
    }

    pub async fn init(self) -> eyre::Result<Self> {
        self._init_contracts().await?;
        self._init_reserves().await?;

        Ok(self)
    }

    // TODO
    async fn _init_contracts(&self) -> eyre::Result<()> {
        Ok(())
    }

    async fn _init_reserves(&self) -> eyre::Result<()> {
        let reserves = self
            .db_addr
            .send(database::GetReserves(self.target.clone()))
            .await??;

        if reserves.is_empty() {
            info!("No reserves found");
            let reserves = self
                .datap_contract
                .getReservesData(*self.addressp_contract.address())
                .call()
                .await?;

            info!("reserves data: {:#?}", reserves);
            let mut init_reserves = Vec::new();
            for reserve in reserves._0.iter() {
                let aggregator_addr = self.aggregator_addr(&reserve.priceOracle).await.ok();
                info!("aggregator_addr: {:#?}", aggregator_addr);

                let price = oracle_price(&self.provider, reserve.priceOracle).await;
                info!("price: {:?}", price);

                init_reserves.push(database::UpsertReserve {
                    symbol: reserve.symbol.to_string(),
                    name: reserve.name.to_string(),
                    decimals: reserve.decimals.to::<u32>() as i32,
                    address: reserve.underlyingAsset.to_string(),
                    protocol_details_id: self.protocol_details_id,
                    liquidation_threshold: norm(
                        reserve.reserveLiquidationThreshold,
                        Some(10.0_f64.powf(-2.0)),
                    )?,
                    // https://aave.com/docs/developers/smart-contracts/pool-configurator#only-risk-or-pool-admins-methods-configurereserveascollateral
                    // All the values are expressed in bps. A value of 10000 results in 100.00%.
                    // The liquidationBonus is always above 100%.
                    // A value of 105% means the liquidator will receive a 5% bonus.
                    liquidation_bonus: if (norm(reserve.reserveLiquidationBonus, None)? - 10_000.0)
                        / 100.0
                        > 0.0
                    {
                        (norm(reserve.reserveLiquidationBonus, None)? - 10_000.0) / 100.0
                    } else {
                        0.0
                    },
                    flashloan_enabled: reserve.flashLoanEnabled,
                    // You can choose to store the aggregator address or the original oracle address.
                    // Here we store the original oracle address (as a string) for reference.
                    oracle_addr: reserve.priceOracle.to_string(),
                    aggregator_addr: aggregator_addr.map(|v| v.to_string()),
                    price_usd: price,
                    stats: database::UpsertReserveStats {
                        reserve: reserve.underlyingAsset.to_string(),
                        liquidity_rate: norm(reserve.liquidityRate, Some(100.0 / RAY))?,
                        variable_borrow_rate: norm(reserve.variableBorrowRate, Some(100.0 / RAY))?,
                        liquidity_index: norm(reserve.liquidityIndex, Some(1.0 / RAY))?,
                        variable_borrow_index: norm(reserve.variableBorrowIndex, Some(1.0 / RAY))?,
                    },
                });
            }

            let (network, _) = self.target.split_once('-').unwrap();
            self.db_addr
                .send(database::UpsertReserves {
                    network_id: network.to_string(),
                    reserves: init_reserves,
                })
                .await??;
        }

        Ok(())
    }

    pub async fn aggregator_addr(&self, oracle_addr: &Address) -> eyre::Result<Address> {
        // 1. Attempt using the CLRatePriceCapAdapter first.
        let adapter =
            contracts::chainlink::CLRatePriceCapAdapterContract::new(*oracle_addr, &self.provider);
        if let Ok(resp) = adapter.BASE_TO_USD_AGGREGATOR().call().await {
            return Ok(resp._0);
        }

        // 2. Next, try using CLSynchronicityPriceAdapterPegToBase.
        let synch_adapter = contracts::chainlink::CLSynchronicityPriceAdapterPegToBaseContract::new(
            *oracle_addr,
            &self.provider,
        );
        if let Ok(resp) = synch_adapter.ASSET_TO_PEG().call().await {
            // Use the returned address to initialize EACAggregatorProxy and call aggregator().
            let proxy =
                contracts::chainlink::EACAggregatorProxyContract::new(resp._0, &self.provider);
            if let Ok(proxy_resp) = proxy.aggregator().call().await {
                return Ok(proxy_resp._0);
            }
        }

        // 3. Next, try using EACAggregatorProxy directly on the original oracle address.
        let proxy =
            contracts::chainlink::EACAggregatorProxyContract::new(*oracle_addr, &self.provider);
        if let Ok(res) = proxy.aggregator().call().await {
            return Ok(res._0);
        }

        // 4. As fallback, try PriceCapAdapterStable's ASSET_TO_USD_AGGREGATOR.
        let stable_adapter =
            contracts::chainlink::PriceCapAdapterStableContract::new(*oracle_addr, &self.provider);
        if let Ok(resp_stable) = stable_adapter.ASSET_TO_USD_AGGREGATOR().call().await {
            let new_proxy = contracts::chainlink::EACAggregatorProxyContract::new(
                resp_stable._0,
                &self.provider,
            );
            let res_new = new_proxy.aggregator().call().await?;
            return Ok(res_new._0);
        }

        warn!(
            "couldn't acquire aggregator address for reserve with oracle {}",
            oracle_addr
        );
        Err(eyre::eyre!("fetch_base_to_usd_addr: all calls failed"))
    }
}

impl<P: Provider + Unpin + Clone + 'static> Handler<UpdateReservePrice> for Fanatic<P> {
    type Result = ResponseFuture<()>;

    fn handle(&mut self, msg: UpdateReservePrice, _ctx: &mut Self::Context) -> Self::Result {
        // TODO: we have to recalculate the new users' health factors
        // we'll initially do a brute force calculation, i.e calculate the new health factors for all users
        let executor_addr = self.executor_addr.clone().unwrap();
        let reserve_addr = msg.reserve;
        let new_price = msg.new_price;

        let target = self.target.clone();
        let pool_contract = self.pool_contract.clone();

        let reserves = self.reserves.clone();
        let users = self.users.clone();

        let fut = async move {
            // update the reserve price
            {
                let mut reserves = reserves.lock().await;
                if let Some(reserve_data) = reserves.get_mut(&reserve_addr) {
                    info!(
                        ?reserve_addr,
                        old_price = reserve_data.price,
                        new_price,
                        "update_reserve_price"
                    );
                    reserve_data.price = new_price;
                } else {
                    warn!(?reserve_addr, "Unable to find reserve price");
                }
            }

            // TODO: tons of possibilities for improvements. this won't do that well in the long run
            // when we manage tons and tons of users.
            let reserves = reserves.lock().await;
            let reserve = reserves.get(&reserve_addr);
            if let Some(reserve_data) = reserve {
                let users_lock = users.lock().await;
                let high_prio = reserve_data
                    .users
                    .iter()
                    .filter(|user| {
                        users_lock.get(*user).is_some()
                            && users_lock.get(*user).unwrap().health_factor < 1.05
                    })
                    .collect::<Vec<&Address>>();

                for user in high_prio {
                    let hf = match health_factor(&pool_contract, *user).await {
                        Some(hf) => hf,
                        None => return,
                    };

                    if hf < 1.0 {
                        let (network, protocol) = target.split_once('-').unwrap();
                        let (network, protocol) = (network.to_string(), protocol.to_string());

                        let payload = LiquidationRequest {
                            user_address: *user,
                            network: network,
                            protocol: protocol,
                        };

                        let result = executor_addr.send(payload).await;
                        match result {
                            Ok(_) => info!("sent liquidation request for user {}", user),
                            Err(e) => error!(
                                "failed to send liquidation request for user {}: {}",
                                user, e
                            ),
                        }

                        let mut users = users.lock().await;
                        users.insert(
                            *user,
                            database::UserData {
                                health_factor: hf,
                                last_update: OffsetDateTime::now_utc().unix_timestamp(),
                            },
                        );
                    }
                }

                for user in &reserve_data.users {
                    let hf = match health_factor(&pool_contract, *user).await {
                        Some(hf) => hf,
                        None => return,
                    };

                    if hf < 1.0 {
                        let (network, protocol) = target.split_once('-').unwrap();
                        let (network, protocol) = (network.to_string(), protocol.to_string());

                        let payload = LiquidationRequest {
                            user_address: *user,
                            network: network,
                            protocol: protocol,
                        };

                        let result = executor_addr.send(payload).await;
                        match result {
                            Ok(_) => info!("sent liquidation request for user {}", user),
                            Err(e) => error!(
                                "failed to send liquidation request for user {}: {}",
                                user, e
                            ),
                        }
                    }

                    if hf < 100.0 {
                        let mut users = users.lock().await;
                        users.insert(
                            *user,
                            database::UserData {
                                health_factor: hf,
                                last_update: OffsetDateTime::now_utc().unix_timestamp(),
                            },
                        );
                    }
                }
            } else {
                info!("No users found in reserve");
            }
        };

        Box::pin(fut)
    }
}

impl<P: Provider + Unpin + Clone + 'static> Handler<UpdateReserveUser> for Fanatic<P> {
    type Result = ResponseFuture<()>;

    fn handle(&mut self, msg: UpdateReserveUser, _ctx: &mut Self::Context) -> Self::Result {
        let reserve_addr = msg.reserve;
        let user = msg.user_addr;
        let executor_addr = self.executor_addr.clone().unwrap();

        let target = self.target.clone();

        let pool_contract = self.pool_contract.clone();

        let reserves = self.reserves.clone();
        let users = self.users.clone();

        let fut = async move {
            let hf = match health_factor(&pool_contract, user).await {
                Some(hf) => hf,
                None => return,
            };

            {
                let users = users.lock().await;
                let user_data = users
                    .get(&user)
                    .cloned()
                    .unwrap_or(database::UserData::default());
                info!(
                    "user={} | last updated at {} | health factor changed from {} to {}",
                    user, user_data.last_update, user_data.health_factor, hf
                );
            }

            if hf < 1.0 {
                let (network, protocol) = target.split_once('-').unwrap();
                let (network, protocol) = (network.to_string(), protocol.to_string());

                let payload = LiquidationRequest {
                    user_address: user,
                    network: network,
                    protocol: protocol,
                };

                let result = executor_addr.send(payload).await;
                match result {
                    Ok(_) => info!("sent liquidation request for user {} with HF {}", user, hf),
                    Err(e) => error!(
                        "failed to send liquidation request for user {}: {}",
                        user, e
                    ),
                }
            }

            // sanity check
            if hf < 100.0 {
                let mut reserves = reserves.lock().await;
                if let Some(reserve) = reserves.get_mut(&reserve_addr) {
                    reserve.users.insert(user);
                }

                let mut users = users.lock().await;
                users.insert(
                    user,
                    database::UserData {
                        health_factor: hf,
                        last_update: OffsetDateTime::now_utc().unix_timestamp(),
                    },
                );
            }
        };

        Box::pin(fut)
    }
}

impl<P: Provider + Unpin + Clone + 'static> Handler<DoSmthWithLiquidationCall> for Fanatic<P> {
    type Result = ResponseFuture<eyre::Result<()>>;

    fn handle(&mut self, msg: DoSmthWithLiquidationCall, _: &mut Self::Context) -> Self::Result {
        let protocol_details_id = self.protocol_details_id;
        let db_addr = self.db_addr.clone();

        let fut = async move {
            db_addr
                .send(database::InsertLiquidationCall {
                    call: msg.0,
                    protocol_details_id,
                })
                .await??;

            Ok(())
        };

        Box::pin(fut)
    }
}

impl<P: Provider + Unpin + Clone + 'static> Handler<Shutdown> for Fanatic<P> {
    type Result = ResponseFuture<eyre::Result<()>>;

    fn handle(&mut self, _: Shutdown, _: &mut Self::Context) -> Self::Result {
        let users = self.users.clone();
        let db_addr = self.db_addr.clone();
        let datap_contract = self.datap_contract.clone();
        let addressp_address = *self.addressp_contract.address();
        let protocol_details_id = self.protocol_details_id;
        let target = self.target.clone();

        let fut = async move {
            if let Err(e) = update_recent_users(
                &users,
                &db_addr,
                protocol_details_id,
                &datap_contract,
                &addressp_address,
                target,
            )
            .await
            {
                error!("Failed to update recent users: {}", e);
            } else {
                info!("[shutdown] upsert all users positions & health factors..done");
            }

            Ok(())
        };

        Box::pin(fut)
    }
}

impl<P: Provider + Unpin + Clone + 'static> Handler<SuccessfulLiquidation> for Fanatic<P> {
    type Result = ResponseActFuture<Self, eyre::Result<()>>;

    fn handle(&mut self, msg: SuccessfulLiquidation, _: &mut Context<Self>) -> Self::Result {
        let users = self.users.clone();

        Box::pin(
            async move {
                let mut users_guard = users.lock().await;
                if let Some(user_data) = users_guard.get_mut(&msg.user_addr) {
                    user_data.health_factor = -2.0; // Liquidated
                    user_data.last_update = OffsetDateTime::now_utc().unix_timestamp();
                }
                Ok(())
            }
            .into_actor(self),
        )
    }
}

impl<P: Provider + Unpin + Clone + 'static> Handler<FailedLiquidation> for Fanatic<P> {
    type Result = ResponseActFuture<Self, eyre::Result<()>>;

    fn handle(&mut self, msg: FailedLiquidation, _: &mut Context<Self>) -> Self::Result {
        let users = self.users.clone();

        Box::pin(
            async move {
                let mut users_guard = users.lock().await;
                if let Some(user_data) = users_guard.get_mut(&msg.user_addr) {
                    user_data.last_update = OffsetDateTime::now_utc().unix_timestamp();
                }
                Ok(())
            }
            .into_actor(self),
        )
    }
}
