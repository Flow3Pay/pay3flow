-- Provider latency tracking for route ranking.
-- One row per provider: average response time over the last 10 measurements.
CREATE TABLE IF NOT EXISTS provider_latency (
    provider_slug TEXT PRIMARY KEY,
    avg_latency_ms NUMERIC NOT NULL DEFAULT 0,
    last_10_latency_ms JSONB NOT NULL DEFAULT '[]'::jsonb,
    samples_count INTEGER NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE provider_latency IS 'Average P2P source response latency; used to prefer faster providers in routing';

COMMENT ON COLUMN provider_latency.provider_slug IS 'Lowercase source slug, e.g. binance';
COMMENT ON COLUMN provider_latency.avg_latency_ms IS 'Mean of the last 10 successful measurements (milliseconds)';
COMMENT ON COLUMN provider_latency.last_10_latency_ms IS 'JSON array of the last 10 latency_ms values, oldest first';
COMMENT ON COLUMN provider_latency.samples_count IS 'Total successful measurements ever recorded';
COMMENT ON COLUMN provider_latency.updated_at IS 'Timestamp of the last update';
