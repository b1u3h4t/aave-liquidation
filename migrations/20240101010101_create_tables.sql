CREATE TABLE IF NOT EXISTS networks (
    id VARCHAR(50) PRIMARY KEY,
    chain_id INTEGER NULL,
    created_at TIMESTAMP NOT NULL DEFAULT (NOW () AT TIME ZONE 'UTC')
);

CREATE TABLE IF NOT EXISTS protocols (
    id VARCHAR(50) PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    kind VARCHAR(50) NOT NULL,
    fork VARCHAR(50) REFERENCES protocols (id),
    created_at TIMESTAMP NOT NULL DEFAULT (NOW () AT TIME ZONE 'UTC')
);

CREATE TABLE IF NOT EXISTS protocols_details (
    id SERIAL PRIMARY KEY,
    protocol_id VARCHAR(50) NOT NULL REFERENCES protocols (id),
    network_id VARCHAR(50) NOT NULL REFERENCES networks (id),
    deployed_block BIGINT,
    deployed_at TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT (NOW () AT TIME ZONE 'UTC'),
    UNIQUE (protocol_id, network_id)
);

CREATE TABLE IF NOT EXISTS protocols_contracts (
    id SERIAL PRIMARY KEY,
    protocol_details_id INTEGER NOT NULL REFERENCES protocols_details (id),
    name VARCHAR(100) NOT NULL,
    address CHAR(42) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT (NOW () AT TIME ZONE 'UTC'),
    UNIQUE (protocol_details_id, address)
);

CREATE TABLE IF NOT EXISTS erc20 (
    id SERIAL PRIMARY KEY,
    symbol VARCHAR(10) NOT NULL,
    name VARCHAR(100) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT (NOW () AT TIME ZONE 'UTC')
);

CREATE TABLE IF NOT EXISTS erc20_details (
    id SERIAL PRIMARY KEY,
    address CHAR(42) NOT NULL UNIQUE,
    decimals INTEGER NOT NULL,
    erc20_id INTEGER NOT NULL REFERENCES erc20 (id),
    network_id CHAR(50) NOT NULL REFERENCES networks (id),
    created_at TIMESTAMP NOT NULL DEFAULT (NOW () AT TIME ZONE 'UTC'),
    UNIQUE (network_id, address)
);

CREATE TABLE IF NOT EXISTS aavev3_users (
    address CHAR(42) PRIMARY KEY,
    protocol_details_id INTEGER REFERENCES protocols_details (id),
    created_at TIMESTAMP NOT NULL DEFAULT (NOW () AT TIME ZONE 'UTC')
);

CREATE TABLE IF NOT EXISTS aavev3_users_stats (
    user_address CHAR(42) REFERENCES aavev3_users (address),
    health_factor DOUBLE PRECISION NOT NULL,
    updated_at TIMESTAMP NOT NULL DEFAULT (NOW () AT TIME ZONE 'UTC'),
    PRIMARY KEY (user_address)
);

CREATE TABLE IF NOT EXISTS aavev3_reserves (
    reserve CHAR(42) PRIMARY KEY REFERENCES erc20_details (address),
    protocol_details_id INTEGER NOT NULL REFERENCES protocols_details (id),
    liquidation_threshold DOUBLE PRECISION NOT NULL,
    liquidation_bonus DOUBLE PRECISION NOT NULL,
    flashloan_enabled BOOLEAN NOT NULL,
    oracle_addr CHAR(42) NOT NULL,
    -- some reserves don't have aggregators, i.e GHO.. https://etherscan.io/address/0xd110cac5d8682a3b045d5524a9903e031d70fccd
    aggregator_addr CHAR(42),
    created_at TIMESTAMP NOT NULL DEFAULT (NOW () AT TIME ZONE 'UTC')
);

CREATE TABLE IF NOT EXISTS aavev3_reserves_stats (
    reserve CHAR(42) NOT NULL REFERENCES aavev3_reserves (reserve),
    liquidity_index DOUBLE PRECISION NOT NULL,
    liquidity_rate DOUBLE PRECISION NOT NULL,
    variable_borrow_rate DOUBLE PRECISION NOT NULL,
    variable_borrow_index DOUBLE PRECISION NOT NULL,
    price_usd DOUBLE PRECISION,
    updated_at TIMESTAMP NOT NULL DEFAULT (NOW () AT TIME ZONE 'UTC'),
    created_at TIMESTAMP NOT NULL DEFAULT (NOW () AT TIME ZONE 'UTC'),
    PRIMARY KEY (reserve)
);

CREATE TABLE IF NOT EXISTS aavev3_positions (
    id SERIAL PRIMARY KEY,
    user_address CHAR(42) NOT NULL REFERENCES aavev3_users (address),
    reserve CHAR(42) NOT NULL REFERENCES aavev3_reserves (reserve),
    supply_amount DOUBLE PRECISION NOT NULL DEFAULT 0,
    borrow_amount DOUBLE PRECISION NOT NULL DEFAULT 0,
    updated_at TIMESTAMP NOT NULL DEFAULT (NOW () AT TIME ZONE 'UTC'),
    created_at TIMESTAMP NOT NULL DEFAULT (NOW () AT TIME ZONE 'UTC'),
    UNIQUE (user_address, reserve)
);

CREATE TABLE IF NOT EXISTS aavev3_liquidations (
    id SERIAL PRIMARY KEY,
    protocol_details_id INTEGER REFERENCES protocols_details (id),
    -- "user" is a reserved keyword in SQL
    --user_address CHAR(42) REFERENCES aavev3_users (address),
    user_address CHAR(42) NOT NULL,
    --liquidator_address CHAR(42) NOT NULL,
    collateral_asset CHAR(42) NOT NULL REFERENCES aavev3_reserves (reserve),
    debt_asset CHAR(42) NOT NULL REFERENCES aavev3_reserves (reserve),
    timestamp TIMESTAMP NOT NULL DEFAULT (NOW () AT TIME ZONE 'UTC')
    --missed BOOLEAN NOT NULL DEFAULT TRUE
);
