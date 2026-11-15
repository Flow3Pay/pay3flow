CREATE TABLE IF NOT EXISTS otc_p2p_messages (
 id BIGSERIAL PRIMARY KEY,
 message_id UUID NOT NULL UNIQUE,
 trade_id UUID NOT NULL REFERENCES otc_p2p_trades(id),
 sender_actor TEXT NOT NULL,
 ciphertext TEXT NOT NULL,
 created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS otc_p2p_messages_trade_recent
 ON otc_p2p_messages(trade_id, id DESC);
