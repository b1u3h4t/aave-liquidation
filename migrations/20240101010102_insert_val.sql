INSERT INTO
    networks (id, chain_id)
VALUES
    ('avalanche', 43114);

-- The protocol (i.e aave_v3)
INSERT INTO
    protocols (id, name, kind)
VALUES
    ('aave_v3', 'Aave V3', 'lending');

-------------- avalanche --------------
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
            ('aave_v3', 'avalanche', 11970506, '2022-05-11') RETURNING id
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
                '0x794a61358D6845594F94dc1DB02A252b5b4814aD'
            ),
            (
                'PoolAddressesProvider',
                '0xa97684ead0e402dC232d5A977953DF7ECBaB3CDb'
            ),
            (
                'UiPoolDataProviderV3',
                '0x50B4a66bF4D41e6252540eA7427D7A933Bc3c088'
            )
    ) AS contracts (name, address);

--- uniswap
INSERT INTO
    protocols (id, name, kind)
VALUES
    ('uniswap_v3', 'Uniswap V3', 'dex');

-------------- avalanche --------------
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
            ('uniswap_v3', 'avalanche', null, null) RETURNING id
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
                '0x740b1c1de25031C31FF4fC9A62f554A55cdC1baD'
            ),
            (
                'QuoterV2',
                '0xbe0F5544EC67e9B3b2D979aaA43f18Fd87E6257F'
            )
    ) AS contracts (name, address);
