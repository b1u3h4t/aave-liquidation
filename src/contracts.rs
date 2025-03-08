use alloy::sol_types::sol;

pub mod liquidator {
    use super::sol;

    sol! {
        #[sol(rpc)]
        #[derive(Debug)]
        LiquidatoorContract,
        "./abis/Liquidator.json"
    }
}

pub mod aave_v3 {
    use serde::{Deserialize, Serialize};

    use super::sol;

    sol! {
        #[allow(clippy::too_many_arguments)]
        #[sol(rpc)]
        #[derive(Debug, Deserialize, Serialize)]
        PoolContract,
        "./abis/aave_v3/PoolInstance.json"
    }

    sol! {
        #[sol(rpc)]
        #[derive(Debug)]
        AddressProviderContract,
        "./abis/aave_v3/PoolAddressesProvider.json"
    }

    // workaround of err
    // `previous definition of the module `DataTypes``
    mod tmp {
        use super::sol;

        sol! {
            #[sol(rpc)]
            #[derive(Debug)]
            DataProviderContract,
            "./abis/aave_v3/UIPoolDataProviderV3.json"
        }
    }

    pub use tmp::DataProviderContract;
}

pub mod chainlink {
    use super::sol;

    sol! {
        #[sol(rpc)]
        #[derive(Debug)]
        CLRatePriceCapAdapterContract,
        "./abis/chainlink/CLRatePriceCapAdapter.json"
    }

    sol! {
        #[sol(rpc)]
        #[derive(Debug)]
        CLSynchronicityPriceAdapterPegToBaseContract,
        "./abis/chainlink/CLSynchronicityPriceAdapterPegToBase.json"
    }

    sol! {
        #[sol(rpc)]
        #[derive(Debug)]
        PriceCapAdapterStableContract,
        "./abis/chainlink/PriceCapAdapterStable.json"
    }

    sol! {
        #[sol(rpc)]
        #[derive(Debug)]
        EACAggregatorProxyContract,
        "./abis/chainlink/EACAggregatorProxy.json"
    }

    sol! {
        #[sol(rpc)]
        #[derive(Debug)]
        OffchainAggregatorContract,
        "./abis/chainlink/AccessControlledOffchainAggregator.json"
    }
}

pub mod uniswap_v3 {
    use super::sol;

    sol! {
      #[allow(missing_docs)]
      #[sol(rpc)]
      #[derive(Debug)]
      QuoterContract,
      "./abis/uniswap_v3/QuoterV2.json"
    }

    sol! {
      #[allow(missing_docs)]
      #[sol(rpc)]
      #[derive(Debug)]
      FactoryContract,
      "./abis/uniswap_v3/UniswapV3Factory.json"
    }

    sol! {
        #[sol(rpc)]
        #[derive(Debug)]
        PoolContract,
        "./abis/uniswap_v3/UniswapV3Pool.json"
    }
}
