use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

use actix::prelude::*;
use alloy::primitives::Address;
pub use handlers::*;
use sqlx::{types::time::PrimitiveDateTime, FromRow, PgPool, Row};

use crate::actors::Database;

#[derive(Clone, Debug)]
pub struct UserData {
    pub health_factor: f64,
    pub last_update: i64, // UTC EPOCH timestamp
}

impl Default for UserData {
    fn default() -> Self {
        UserData {
            health_factor: -1.0,
            last_update: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ReserveData {
    pub users: HashSet<Address>,
    pub price: f64,
}

#[derive(Message)]
#[rtype(result = "Result<(), sqlx::Error>")]
pub struct UpsertReserve {
    pub symbol: String,
    pub name: String,
    pub decimals: i32,
    pub address: String,
    pub protocol_details_id: i32,
    pub liquidation_threshold: f64,
    pub liquidation_bonus: f64,
    pub flashloan_enabled: bool,
    pub oracle_addr: String,
    pub aggregator_addr: Option<String>,
    pub stats: UpsertReserveStats,
    pub price_usd: f64,
}

#[derive(Message)]
#[rtype(result = "Result<(), sqlx::Error>")]
pub struct UpsertReserveStats {
    pub reserve: String,
    pub liquidity_rate: f64,
    pub variable_borrow_rate: f64,
    pub liquidity_index: f64,
    pub variable_borrow_index: f64,
}

pub mod handlers {
    use crate::contracts;

    use super::*;

    #[derive(Message)]
    #[rtype(result = "Result<HashMap<String, Address>, sqlx::Error>")]
    pub struct GetProtocolContracts(pub String);
    impl Handler<GetProtocolContracts> for Database {
        type Result = ResponseFuture<Result<HashMap<String, Address>, sqlx::Error>>;

        fn handle(&mut self, msg: GetProtocolContracts, _: &mut Self::Context) -> Self::Result {
            let pool = self.pool.clone();
            let (network, protocol) = msg.0.split_once('-').unwrap();
            let (network, protocol) = (network.to_string(), protocol.to_string());

            let fut = async move { get_protocol_contracts(&pool, &network, &protocol).await };

            Box::pin(fut)
        }
    }

    #[derive(Message)]
    #[rtype(result = "Result<i32, sqlx::Error>")]
    pub struct GetProtocolDetailsId(pub String);
    impl Handler<GetProtocolDetailsId> for Database {
        type Result = ResponseFuture<Result<i32, sqlx::Error>>;

        fn handle(&mut self, msg: GetProtocolDetailsId, _: &mut Self::Context) -> Self::Result {
            let pool = self.pool.clone();
            let (network, protocol) = msg.0.split_once('-').unwrap();
            let (network, protocol) = (network.to_string(), protocol.to_string());

            let fut = async move { get_protocol_details_id(&pool, &network, &protocol).await };

            Box::pin(fut)
        }
    }

    #[derive(Message)]
    #[rtype(result = "Result<Option<i64>, sqlx::Error>")]
    pub struct GetProtocolDeployBlock(pub String);
    impl Handler<GetProtocolDeployBlock> for Database {
        type Result = ResponseFuture<Result<Option<i64>, sqlx::Error>>;

        fn handle(&mut self, msg: GetProtocolDeployBlock, _: &mut Self::Context) -> Self::Result {
            let pool = self.pool.clone();
            let (network, protocol) = msg.0.split_once('-').unwrap();
            let (network, protocol) = (network.to_string(), protocol.to_string());

            let fut = async move { get_protocol_deploy_block(&pool, &network, &protocol).await };

            Box::pin(fut)
        }
    }

    #[derive(Message)]
    #[rtype(result = "HashMap<Address, Address>")]
    pub struct GetAggregatorMapping(pub String);
    impl Handler<GetAggregatorMapping> for Database {
        type Result = ResponseFuture<HashMap<Address, Address>>;

        fn handle(&mut self, msg: GetAggregatorMapping, _ctx: &mut Self::Context) -> Self::Result {
            let pool = self.pool.clone();
            let fut = async move { get_aggregator_mapping(&pool, msg.0).await };

            Box::pin(fut)
        }
    }

    #[derive(Message)]
    #[rtype(result = "Result<Vec<Reserve>, sqlx::Error>")]
    pub struct GetReserves(pub String);
    impl Handler<GetReserves> for Database {
        type Result = ResponseFuture<Result<Vec<Reserve>, sqlx::Error>>;

        fn handle(&mut self, msg: GetReserves, _: &mut Self::Context) -> Self::Result {
            let pool = self.pool.clone();
            let (network, protocol) = msg.0.split_once('-').unwrap();
            let (network, protocol) = (network.to_string(), protocol.to_string());
            let fut = async move { get_reserves(&pool, &network, &protocol).await };

            Box::pin(fut)
        }
    }

    #[derive(Message)]
    #[rtype(
        result = "Result<(HashMap<Address, UserData>, HashMap<Address, ReserveData>), sqlx::Error>"
    )]
    pub struct GetReservesUsers(pub String);
    impl Handler<GetReservesUsers> for Database {
        type Result = ResponseFuture<
            Result<(HashMap<Address, UserData>, HashMap<Address, ReserveData>), sqlx::Error>,
        >;

        fn handle(&mut self, msg: GetReservesUsers, _: &mut Self::Context) -> Self::Result {
            let pool = self.pool.clone();
            let (network, protocol) = msg.0.split_once('-').unwrap();
            let (network, protocol) = (network.to_string(), protocol.to_string());

            let fut = async move { get_reserves_users(&pool, &network, &protocol).await };

            Box::pin(fut)
        }
    }

    #[derive(Message)]
    #[rtype(result = "Result<HashMap<String, (f64, f64)>, sqlx::Error>")]
    pub struct GetReservesLiquidityIndices(pub String);
    impl Handler<GetReservesLiquidityIndices> for Database {
        type Result = ResponseFuture<Result<HashMap<String, (f64, f64)>, sqlx::Error>>;

        fn handle(
            &mut self,
            msg: GetReservesLiquidityIndices,
            _: &mut Self::Context,
        ) -> Self::Result {
            let pool = self.pool.clone();
            let (network, protocol) = msg.0.split_once('-').unwrap();
            let (network, protocol) = (network.to_string(), protocol.to_string());

            let fut =
                async move { get_reserves_liquidity_indices(&pool, &network, &protocol).await };

            Box::pin(fut)
        }
    }

    #[derive(Message)]
    #[rtype(result = "Result<(), sqlx::Error>")]
    pub struct UpdateOraclePrice {
        pub target: String,
        pub reserve: Address,
        pub price: f64,
    }
    impl Handler<UpdateOraclePrice> for Database {
        type Result = ResponseFuture<Result<(), sqlx::Error>>;

        fn handle(&mut self, msg: UpdateOraclePrice, _ctx: &mut Self::Context) -> Self::Result {
            let pool = self.pool.clone();
            let (network, protocol) = msg.target.split_once('-').unwrap();
            let (network, protocol) = (network.to_string(), protocol.to_string());

            let fut = async move {
                update_oracle_price(
                    &pool,
                    msg.price,
                    &msg.reserve.to_string(),
                    &network,
                    &protocol,
                )
                .await
            };

            Box::pin(fut)
        }
    }

    #[derive(Message)]
    #[rtype(result = "Result<(), sqlx::Error>")]
    pub struct UpsertReserves {
        pub network_id: String,
        pub reserves: Vec<UpsertReserve>,
    }
    impl Handler<UpsertReserves> for Database {
        type Result = ResponseFuture<Result<(), sqlx::Error>>;

        fn handle(&mut self, msg: UpsertReserves, _: &mut Self::Context) -> Self::Result {
            let pool = self.pool.clone();
            let fut = async move { upsert_reserves(&pool, &msg.network_id, msg.reserves).await };

            Box::pin(fut)
        }
    }

    #[derive(Message)]
    #[rtype(result = "Result<(), sqlx::Error>")]
    pub struct UpsertReservesStats(pub Vec<UpsertReserveStats>);
    impl Handler<UpsertReservesStats> for Database {
        type Result = ResponseFuture<Result<(), sqlx::Error>>;

        fn handle(&mut self, msg: UpsertReservesStats, _: &mut Self::Context) -> Self::Result {
            let pool = self.pool.clone();
            let fut = async move { upsert_reserves_stats(&pool, msg.0).await };

            Box::pin(fut)
        }
    }

    #[derive(Message)]
    #[rtype(result = "Result<(), sqlx::Error>")]
    pub struct UpsertUserData {
        pub address: String,
        pub protocol_details_id: i32,
        pub health_factor: f64,
        pub positions: Vec<(Address, f64, f64)>, // (token_address, supply amount, borrow amount)
    }
    impl Handler<UpsertUserData> for Database {
        type Result = ResponseFuture<Result<(), sqlx::Error>>;

        fn handle(&mut self, msg: UpsertUserData, _: &mut Self::Context) -> Self::Result {
            let pool = self.pool.clone();
            let fut = async move {
                upsert_user_data(
                    &pool,
                    &msg.address,
                    msg.protocol_details_id,
                    msg.positions,
                    msg.health_factor,
                )
                .await
            };

            Box::pin(fut)
        }
    }

    #[derive(Message)]
    #[rtype(result = "Result<(), sqlx::Error>")]
    pub struct UpsertUsersStats {
        pub users: HashMap<Address, f64>,
        pub protocol_details_id: i32,
    }
    impl Handler<UpsertUsersStats> for Database {
        type Result = ResponseFuture<Result<(), sqlx::Error>>;

        fn handle(&mut self, msg: UpsertUsersStats, _: &mut Self::Context) -> Self::Result {
            let pool = self.pool.clone();
            let fut =
                async move { upsert_users_stats(&pool, msg.protocol_details_id, msg.users).await };

            Box::pin(fut)
        }
    }

    #[derive(Message)]
    #[rtype(result = "Result<(), sqlx::Error>")]
    pub struct InsertLiquidationCall {
        pub call: contracts::aave_v3::PoolContract::LiquidationCall,
        pub protocol_details_id: i32,
    }
    impl Handler<InsertLiquidationCall> for Database {
        type Result = ResponseFuture<Result<(), sqlx::Error>>;

        fn handle(&mut self, msg: InsertLiquidationCall, _: &mut Self::Context) -> Self::Result {
            let pool = self.pool.clone();
            let fut = async move {
                insert_liquidation_call(
                    &pool,
                    msg.protocol_details_id,
                    &msg.call.user.to_string(),
                    &msg.call.collateralAsset.to_string(),
                    &msg.call.debtAsset.to_string(),
                )
                .await
            };

            Box::pin(fut)
        }
    }
}

pub async fn get_protocol_contracts(
    pool: &PgPool,
    network: &str,
    protocol: &str,
) -> Result<HashMap<String, Address>, sqlx::Error> {
    const QUERY: &str = r#"
                SELECT pc.name, pc.address
                FROM protocols_details pd
                JOIN protocols_contracts pc ON pd.id = pc.protocol_details_id
                WHERE pd.network_id = $1 AND pd.protocol_id = $2
            "#;

    let rows = sqlx::query(QUERY)
        .bind(network)
        .bind(protocol)
        .fetch_all(pool)
        .await?;
    let mapping: HashMap<String, Address> = rows
        .into_iter()
        .map(|row| {
            (
                row.get::<String, _>("name"),
                Address::from_str(&row.get::<String, _>("address")).unwrap(),
            )
        })
        .collect();
    Ok(mapping)
}

pub async fn get_protocol_details_id(
    pool: &PgPool,
    network: &str,
    protocol: &str,
) -> Result<i32, sqlx::Error> {
    const QUERY: &str =
        "SELECT pd.id FROM protocols_details pd WHERE pd.network_id = $1 AND pd.protocol_id = $2";
    let row = sqlx::query(QUERY)
        .bind(network)
        .bind(protocol)
        .fetch_one(pool)
        .await?;
    let id = row.get::<i32, _>("id");
    Ok(id)
}

pub async fn get_protocol_deploy_block(
    pool: &PgPool,
    network: &str,
    protocol: &str,
) -> Result<Option<i64>, sqlx::Error> {
    const QUERY: &str = r#"
            SELECT deployed_block
            FROM protocols_details
            WHERE network_id = $1 AND protocol_id = $2
            LIMIT 1"#;

    let row = sqlx::query(QUERY)
        .bind(network)
        .bind(protocol)
        .fetch_optional(pool)
        .await?;

    Ok(row.map(|r| r.get("deployed_block")))
}

pub async fn get_aggregator_mapping(pool: &PgPool, target: String) -> HashMap<Address, Address> {
    const QUERY: &str = r#"
            SELECT ar.aggregator_addr, ar.reserve
            FROM aavev3_reserves ar
            JOIN protocols_details pd ON pd.id = ar.protocol_details_id
            WHERE ar.aggregator_addr IS NOT NULL AND ar.reserve IS NOT NULL AND pd.network_id = $1 AND pd.protocol_id = $2"#;

    let (network, protocol) = target.split_once('-').unwrap();
    let (network, protocol) = (network.to_string(), protocol.to_string());

    let r = sqlx::query(QUERY)
        .bind(network)
        .bind(protocol)
        .fetch_all(pool)
        .await
        .unwrap();
    let mapping: HashMap<Address, Address> = r
        .into_iter()
        .map(|row| {
            (
                row.get::<String, _>("aggregator_addr"),
                row.get::<String, _>("reserve"),
            )
        })
        .map(|(aggregator_addr, reserve)| {
            (
                Address::from_str(&aggregator_addr).unwrap(),
                Address::from_str(&reserve).unwrap(),
            )
        })
        .collect();

    mapping
}

#[derive(Clone, Debug, FromRow)]
pub struct Reserve {
    pub reserve: Address,
    pub protocol_details_id: i32,
    pub liquidation_threshold: f64,
    pub liquidation_bonus: f64,
    pub flashloan_enabled: bool,
    pub oracle_addr: Address,
    pub aggregator_addr: Option<Address>,
    pub decimals: i32,

    #[sqlx(flatten)]
    pub stats: ReserveStats,
}

#[derive(Clone, Debug, FromRow)]
pub struct ReserveStats {
    pub liquidity_index: f64,
    pub liquidity_rate: f64,
    pub variable_borrow_rate: f64,
    pub variable_borrow_index: f64,
    pub price_usd: f64,
    pub updated_at: PrimitiveDateTime,
}

pub async fn update_oracle_price(
    pool: &PgPool,
    price: f64,
    reserve: &str,
    network: &str,
    protocol: &str,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
        UPDATE aavev3_reserves_stats ars
        SET price_usd = $1, updated_at = NOW()
        FROM aavev3_reserves ar
        JOIN protocols_details pd ON ar.protocol_details_id = pd.id
        WHERE ars.reserve = ar.reserve
          AND ar.reserve = $2
          AND pd.network_id = $3
          AND pd.protocol_id = $4
    "#;
    sqlx::query(QUERY)
        .bind(price)
        .bind(reserve)
        .bind(network)
        .bind(protocol)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn upsert_reserves_stats(
    pool: &PgPool,
    reserves: Vec<UpsertReserveStats>,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
        INSERT INTO aavev3_reserves_stats (reserve, liquidity_rate, variable_borrow_rate, liquidity_index, variable_borrow_index)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (reserve) DO UPDATE SET
            liquidity_rate = $2,
            variable_borrow_rate = $3,
            liquidity_index = $4,
            variable_borrow_index = $5,
            updated_at = NOW()
    "#;
    let mut tx = pool.begin().await?;
    for reserve in reserves {
        sqlx::query(QUERY)
            .bind(&reserve.reserve)
            .bind(reserve.liquidity_rate)
            .bind(reserve.variable_borrow_rate)
            .bind(reserve.liquidity_index)
            .bind(reserve.variable_borrow_index)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn get_reserves_liquidity_indices(
    pool: &PgPool,
    network: &str,
    protocol: &str,
) -> Result<HashMap<String, (f64, f64)>, sqlx::Error> {
    const QUERY: &str = r#"
        SELECT rs.reserve, rs.liquidity_index, rs.variable_borrow_index
        FROM aavev3_reserves_stats rs
        JOIN aavev3_reserves r ON rs.reserve = r.reserve
        JOIN protocols_details pd ON r.protocol_details_id = pd.id
        WHERE pd.network_id = $1 AND pd.protocol_id = $2
    "#;
    let rows = sqlx::query(QUERY)
        .bind(network)
        .bind(protocol)
        .fetch_all(pool)
        .await?;
    let mapping: HashMap<String, (f64, f64)> = rows
        .into_iter()
        .map(|row| {
            (
                row.get::<String, _>("reserve"),
                (
                    row.get::<f64, _>("liquidity_index"),
                    row.get::<f64, _>("variable_borrow_index"),
                ),
            )
        })
        .collect();
    Ok(mapping)
}

pub async fn upsert_reserves(
    pool: &PgPool,
    network_id: &str,
    reserves: Vec<UpsertReserve>,
) -> Result<(), sqlx::Error> {
    const INSERT_ERC20: &str = r#"
        INSERT INTO erc20 (symbol, name)
        VALUES ($1, $2)
        ON CONFLICT DO NOTHING
        RETURNING id
    "#;
    const UPSERT_ERC20_DETAILS: &str = r#"
        INSERT INTO erc20_details (address, decimals, erc20_id, network_id)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (address) DO UPDATE SET
            decimals = $2,
            erc20_id = $3,
            network_id = $4
    "#;
    const UPSERT_RESERVES: &str = r#"
        INSERT INTO aavev3_reserves (
            reserve, protocol_details_id, liquidation_threshold, liquidation_bonus,
            flashloan_enabled, oracle_addr, aggregator_addr
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        ON CONFLICT (reserve) DO UPDATE SET
            liquidation_threshold = $3,
            liquidation_bonus = $4,
            flashloan_enabled = $5,
            oracle_addr = $6,
            aggregator_addr = $7
    "#;
    const UPSERT_STATS: &str = r#"
        INSERT INTO aavev3_reserves_stats (
            reserve, liquidity_rate, variable_borrow_rate, liquidity_index, variable_borrow_index, price_usd
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (reserve) DO UPDATE SET
            liquidity_rate = $2,
            variable_borrow_rate = $3,
            liquidity_index = $4,
            variable_borrow_index = $5,
            price_usd = $6,
            updated_at = NOW()
    "#;

    let mut tx = pool.begin().await?;
    for reserve in reserves {
        let erc20_row = sqlx::query(INSERT_ERC20)
            .bind(&reserve.symbol)
            .bind(&reserve.name)
            .fetch_one(&mut *tx)
            .await?;
        let erc20_id: i32 = erc20_row.get("id");

        sqlx::query(UPSERT_ERC20_DETAILS)
            .bind(&reserve.address)
            .bind(reserve.decimals)
            .bind(erc20_id)
            .bind(network_id)
            .execute(&mut *tx)
            .await?;

        sqlx::query(UPSERT_RESERVES)
            .bind(&reserve.address)
            .bind(reserve.protocol_details_id)
            .bind(reserve.liquidation_threshold)
            .bind(reserve.liquidation_bonus)
            .bind(reserve.flashloan_enabled)
            .bind(&reserve.oracle_addr)
            .bind(&reserve.aggregator_addr)
            .execute(&mut *tx)
            .await?;

        sqlx::query(UPSERT_STATS)
            .bind(&reserve.stats.reserve)
            .bind(reserve.stats.liquidity_rate)
            .bind(reserve.stats.variable_borrow_rate)
            .bind(reserve.stats.liquidity_index)
            .bind(reserve.stats.variable_borrow_index)
            .bind(reserve.price_usd)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn upsert_user_data(
    pool: &PgPool,
    address: &str,
    protocol_details_id: i32,
    positions: Vec<(Address, f64, f64)>,
    health_factor: f64,
) -> Result<(), sqlx::Error> {
    const UPSERT_USERS: &str = r#"
        INSERT INTO aavev3_users (address, protocol_details_id)
        VALUES ($1, $2)
        ON CONFLICT (address) DO UPDATE SET protocol_details_id = $2
    "#;
    const DELETE_POSITIONS: &str = r#"
        DELETE FROM aavev3_positions
        WHERE user_address = $1 AND reserve != ANY($2::text[])
    "#;
    const UPSERT_POSITIONS: &str = r#"
        INSERT INTO aavev3_positions (user_address, reserve, supply_amount, borrow_amount)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (user_address, reserve)
        DO UPDATE SET
            supply_amount = $3,
            borrow_amount = $4,
            updated_at = NOW()
    "#;
    const UPSERT_STATS: &str = r#"
        INSERT INTO aavev3_users_stats (user_address, health_factor)
        VALUES ($1, $2)
        ON CONFLICT (user_address)
        DO UPDATE SET health_factor = $2, updated_at = NOW()
    "#;

    let mut tx = pool.begin().await?;
    sqlx::query(UPSERT_USERS)
        .bind(address)
        .bind(protocol_details_id)
        .execute(&mut *tx)
        .await?;

    let reserves: Vec<String> = positions
        .iter()
        .map(|(r, _, _)| r.to_string().clone())
        .collect();
    sqlx::query(DELETE_POSITIONS)
        .bind(address)
        .bind(&reserves)
        .execute(&mut *tx)
        .await?;

    for (token_address, supply_amount, borrow_amount) in positions {
        sqlx::query(UPSERT_POSITIONS)
            .bind(address)
            .bind(token_address.to_string())
            .bind(supply_amount)
            .bind(borrow_amount)
            .execute(&mut *tx)
            .await?;
    }

    sqlx::query(UPSERT_STATS)
        .bind(address)
        .bind(health_factor)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn get_reserves(
    pool: &PgPool,
    network: &str,
    protocol: &str,
) -> Result<Vec<Reserve>, sqlx::Error> {
    const QUERY: &str = r#"
        SELECT
            ar.reserve,
            ar.protocol_details_id,
            ar.liquidation_threshold,
            ar.liquidation_bonus,
            ar.flashloan_enabled,
            ar.oracle_addr,
            ar.aggregator_addr,
            ars.liquidity_index,
            ars.liquidity_rate,
            ars.variable_borrow_rate,
            ars.variable_borrow_index,
            ars.price_usd,
            ed.decimals,
            ars.updated_at
        FROM aavev3_reserves ar
        JOIN protocols_details pd ON ar.protocol_details_id = pd.id
        JOIN erc20_details ed ON ar.reserve = ed.address
        JOIN aavev3_reserves_stats ars ON ar.reserve = ars.reserve
        WHERE ed.network_id = $1 AND pd.protocol_id = $2
    "#;
    let rows = sqlx::query(QUERY)
        .bind(network)
        .bind(protocol)
        .fetch_all(pool)
        .await?;
    let reserves: Vec<Reserve> = rows
        .into_iter()
        .map(|row| Reserve {
            reserve: row.get::<String, _>("reserve").parse().unwrap(),
            protocol_details_id: row.get("protocol_details_id"),
            liquidation_threshold: row.get("liquidation_threshold"),
            liquidation_bonus: row.get("liquidation_bonus"),
            flashloan_enabled: row.get("flashloan_enabled"),
            oracle_addr: row.get::<String, _>("oracle_addr").parse().unwrap(),
            aggregator_addr: row
                .get::<Option<String>, _>("aggregator_addr")
                .map(|v| v.parse().unwrap()),
            stats: ReserveStats {
                liquidity_index: row.get("liquidity_index"),
                liquidity_rate: row.get("liquidity_rate"),
                variable_borrow_rate: row.get("variable_borrow_rate"),
                variable_borrow_index: row.get("variable_borrow_index"),
                price_usd: row.get::<Option<f64>, _>("price_usd").unwrap_or(0.0),
                updated_at: row.get("updated_at"),
            },
            decimals: row.get("decimals"),
        })
        .collect();
    Ok(reserves)
}

pub async fn get_reserves_users(
    pool: &PgPool,
    network: &str,
    protocol: &str,
) -> Result<(HashMap<Address, UserData>, HashMap<Address, ReserveData>), sqlx::Error> {
    const QUERY: &str = r#"
        SELECT
            au.address AS user_addr,
            aus.health_factor,
            EXTRACT(EPOCH FROM aus.updated_at)::NUMERIC::BIGINT as updated_at,
            ap.reserve AS reserve_addr,
            COALESCE(ars.price_usd, 0) AS price_usd
        FROM aavev3_users au
        JOIN aavev3_users_stats aus ON aus.user_address = au.address
        JOIN protocols_details pd ON pd.id = au.protocol_details_id
        JOIN aavev3_positions ap ON ap.user_address = au.address
        JOIN aavev3_reserves ar ON ar.reserve = ap.reserve
        JOIN erc20_details ed ON ed.address = ar.reserve
        JOIN erc20 e ON e.id = ed.erc20_id
        JOIN aavev3_reserves_stats ars ON ar.reserve = ars.reserve
        WHERE pd.network_id = $1 AND pd.protocol_id = $2
    "#;
    let rows = sqlx::query(QUERY)
        .bind(network)
        .bind(protocol)
        .fetch_all(pool)
        .await?;

    let mut users: HashMap<Address, UserData> = HashMap::new();
    let mut reserves: HashMap<Address, ReserveData> = HashMap::new();

    for row in rows {
        let user_addr = Address::from_str(&row.get::<String, _>("user_addr")).unwrap();
        let health_factor = row.get::<f64, _>("health_factor");
        let updated_at = row.get::<i64, _>("updated_at");
        let reserve_addr = Address::from_str(&row.get::<String, _>("reserve_addr")).unwrap();
        let price_usd = row.get::<f64, _>("price_usd");

        users.insert(
            user_addr,
            UserData {
                health_factor,
                last_update: updated_at,
            },
        );

        reserves
            .entry(reserve_addr)
            .or_insert(ReserveData {
                users: HashSet::new(),
                price: price_usd,
            })
            .users
            .insert(user_addr);
    }

    Ok((users, reserves))
}

pub async fn insert_liquidation_call(
    pool: &PgPool,
    protocol_details_id: i32,
    user_address: &str,
    collateral_asset: &str,
    debt_asset: &str,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
        INSERT INTO aavev3_liquidations (protocol_details_id, user_address, collateral_asset, debt_asset)
        VALUES ($1, $2, $3, $4)
    "#;
    sqlx::query(QUERY)
        .bind(protocol_details_id)
        .bind(user_address)
        .bind(collateral_asset)
        .bind(debt_asset)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn upsert_users_stats(
    pool: &PgPool,
    protocol_details_id: i32,
    users: HashMap<Address, f64>,
) -> Result<(), sqlx::Error> {
    const INSERT_USERS: &str = r#"
        INSERT INTO aavev3_users (address, protocol_details_id)
        VALUES ($1, $2)
        ON CONFLICT DO NOTHING
    "#;
    const UPSERT_STATS: &str = r#"
        INSERT INTO aavev3_users_stats (user_address, health_factor)
        VALUES ($1, $2)
        ON CONFLICT (user_address) DO UPDATE SET health_factor = $2, updated_at = NOW()
    "#;

    for (addr, hf) in users {
        let addr_str = addr.to_string();
        sqlx::query(INSERT_USERS)
            .bind(&addr_str)
            .bind(protocol_details_id)
            .execute(pool)
            .await?;
        sqlx::query(UPSERT_STATS)
            .bind(&addr_str)
            .bind(hf)
            .execute(pool)
            .await?;
    }
    Ok(())
}
