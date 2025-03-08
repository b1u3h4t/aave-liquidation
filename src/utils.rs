use std::collections::HashMap;

use crate::contracts;
use alloy::{
    primitives::{utils::format_ether, Address, Uint, U256},
    providers::Provider,
};
use tracing::info;

// Liquidators can only close a certain amount of collateral defined by a close factor.
// Currently the close factor is 0.5. In other words,
// liquidators can only liquidate a maximum of 50% of the amount pending to be repaid in a position.
// The liquidation discount applies to this amount.
pub const CLOSE_FACTOR: f64 = 0.5;

pub fn norm<T>(val: T, factor: Option<f64>) -> eyre::Result<f64>
where
    T: ToString,
{
    Ok(val.to_string().parse::<f64>()? * factor.unwrap_or(1.0))
}

pub async fn health_factor<P: Provider + Clone>(
    contract: &contracts::aave_v3::PoolContract::PoolContractInstance<(), P>,
    user: Address,
) -> Option<f64> {
    let data = contract.getUserAccountData(user).call().await.unwrap();
    let health_factor = format_ether(data.healthFactor).parse::<f64>().unwrap();

    // sanity check — ensure health factor is within a reasonable range
    if health_factor > 10_000.0 {
        return None;
    }

    Some(health_factor)
}

pub async fn user_liquidation_data<P: Provider + Clone>(
    datap_contract: &contracts::aave_v3::DataProviderContract::DataProviderContractInstance<(), P>,
    provider_addr: Address,
    user: Address,
    indices: &HashMap<String, (f64, f64)>,
) -> eyre::Result<(Address, Address, U256)> {
    let user_reserves = datap_contract
        .getUserReservesData(provider_addr, user)
        .call()
        .await?;

    let debt = user_reserves
        ._0
        .iter()
        .find(|v| v.scaledVariableDebt > U256::from(0))
        .ok_or(eyre::eyre!("No debt asset found"))?;

    let (_, var_idx) = indices
        .get(&debt.underlyingAsset.to_string())
        .unwrap_or(&(1.0, 1.0));
    let normalized_debt = norm(debt.scaledVariableDebt, Some(*var_idx))?;
    let debt_to_cover = debt_to_cover(U256::from(normalized_debt));

    let collateral = user_reserves
        ._0
        .iter()
        .find(|v| v.scaledATokenBalance > U256::from(0))
        .ok_or(eyre::eyre!("No collateral asset found"))?;

    info!(
        debt_asset = ?debt.underlyingAsset,
        debt = ?debt.scaledVariableDebt,
        collateral = ?collateral.scaledATokenBalance,
        collateral_asset = ?collateral.underlyingAsset,
        debt_to_cover = ?debt_to_cover,
    );

    let debt_asset = debt.underlyingAsset;
    let collateral_asset = collateral.underlyingAsset;

    Ok((debt_asset, collateral_asset, debt_to_cover))
}

pub fn debt_to_cover(variable_debt: U256) -> U256 {
    variable_debt * Uint::from(CLOSE_FACTOR)
}

pub async fn find_most_liquid_uniswap_pool<P: Provider + Clone>(
    provider: &P,
    factory_contract: &contracts::uniswap_v3::FactoryContract::FactoryContractInstance<(), P>,
    collateral_asset: Address,
    debt_asset: Address,
) -> eyre::Result<(Address, u16)> {
    const FEES: [u16; 3] = [500, 3000, 10000];

    let mut max_liquidity = U256::ZERO;
    let mut best_pool = None;

    for fee in FEES {
        let pool = factory_contract
            .getPool(collateral_asset, debt_asset, Uint::from(fee))
            .call()
            .await?;

        if pool._0 != Address::ZERO {
            let contract = contracts::uniswap_v3::PoolContract::new(pool._0, provider.clone());
            let liquidity = U256::from(contract.liquidity().call().await?._0);

            if liquidity > max_liquidity {
                max_liquidity = liquidity;
                best_pool = Some((pool._0, fee));
            }
        }
    }

    info!(?best_pool, "most liquid uniswap_v3 pool");
    best_pool.ok_or(eyre::eyre!("No valid pool found"))
}

pub async fn user_positions<P: Provider + Clone>(
    datap_contract: &contracts::aave_v3::DataProviderContract::DataProviderContractInstance<(), P>,
    addressp_addr: &Address,
    user: &Address,
    indices: &HashMap<String, (f64, f64)>,
) -> eyre::Result<Vec<(Address, f64, f64)>> {
    let user_data = datap_contract
        .getUserReservesData(*addressp_addr, *user)
        .call()
        .await?;

    Ok(user_data
        ._0
        .iter()
        .filter(|r| !r.scaledATokenBalance.is_zero() || !r.scaledVariableDebt.is_zero())
        .map(|r| {
            let addr = r.underlyingAsset;
            let (liq_idx, var_idx) = indices.get(&addr.to_string()).unwrap_or(&(1.0, 1.0));
            let collateral =
                norm(r.scaledATokenBalance, Some(*liq_idx)).expect("supply calc failed");
            let debt = norm(r.scaledVariableDebt, Some(*var_idx)).expect("borrow calc failed");
            (addr, collateral, debt)
        })
        .collect())
}
