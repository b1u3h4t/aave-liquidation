INSERT INTO
    networks (id, chain_id)
VALUES
    ('ethereum', 1);

-- The protocol (i.e aave_v3)
INSERT INTO
    protocols (id, name, kind)
VALUES
    ('aave_v3', 'Aave V3', 'lending');

-------------- ethereum --------------
WITH
    inserted_protocol AS (
        INSERT INTO
            protocols_details (
                protocol_id,
                network_id,
                deployed_block,
                deployed_at
            )
        VALUES
            ('aave_v3', 'ethereum', 16291127, '2023-01-27') RETURNING id
    )
INSERT INTO
    protocols_contracts (protocol_details_id, name, address)
SELECT
    id,
    name,
    address
FROM
    inserted_protocol,
    (
        VALUES
            (
                'Pool',
                '0x87870Bca3F3fD6335C3F4ce8392D69350B4fA4E2'
            ),
            (
                'PoolAddressesProvider',
                '0x2f39d218133AFaB8F2B819B1066c7E434Ad94E9e'
            ),
            (
                'UiPoolDataProviderV3',
                '0x3F78BBD206e4D3c504Eb854232EdA7e47E9Fd8FC'
            )
    ) AS contracts (name, address);

--- uniswap
INSERT INTO
    protocols (id, name, kind)
VALUES
    ('uniswap_v3', 'Uniswap V3', 'dex');

-------------- ethereum --------------
WITH
    inserted_protocol AS (
        INSERT INTO
            protocols_details (
                protocol_id,
                network_id,
                deployed_block,
                deployed_at
            )
        VALUES
            ('uniswap_v3', 'ethereum', null, null) RETURNING id
    )
INSERT INTO
    protocols_contracts (protocol_details_id, name, address)
SELECT
    id,
    name,
    address
FROM
    inserted_protocol,
    (
        VALUES
            (
                'UniswapV3Factory',
                '0x1F98431c8aD98523631AE4a59f267346ea31F984'
            ),
            (
                'QuoterV2',
                '0x61fFE014bA17989E743c5F6cB21bF9697530B21e'
            )
    ) AS contracts (name, address);
