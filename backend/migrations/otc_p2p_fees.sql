-- One service fee per peer-to-peer deal. The stablecoin leg pays net to the
-- seller; the fee is collected separately to the configured destination.
CREATE TABLE IF NOT EXISTS otc_p2p_fees (
 id UUID PRIMARY KEY,
 trade_id UUID NOT NULL REFERENCES otc_p2p_trades(id),
 offer_id UUID NOT NULL REFERENCES otc_public_offers(id),
 asset TEXT NOT NULL DEFAULT 'USDT' CHECK(asset IN ('USDT','USDC')),
 precision SMALLINT NOT NULL DEFAULT 6 CHECK(precision=6),
 basis_units NUMERIC NOT NULL CHECK(basis_units>0),
 fee_units NUMERIC NOT NULL CHECK(fee_units>0),
 net_units NUMERIC NOT NULL CHECK(net_units>=0),
 rate NUMERIC NOT NULL,
 version TEXT NOT NULL,
 state TEXT NOT NULL DEFAULT 'accrued' CHECK(state IN ('accrued','collected')),
 created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
 collected_at TIMESTAMPTZ,
 UNIQUE(trade_id)
);
ALTER TABLE otc_p2p_fees DROP CONSTRAINT IF EXISTS otc_p2p_fees_asset_check;
ALTER TABLE otc_p2p_fees ADD CONSTRAINT otc_p2p_fees_asset_check CHECK(asset IN ('USDT','USDC'));
CREATE INDEX IF NOT EXISTS otc_p2p_fees_state ON otc_p2p_fees(state,created_at);
