-- Generated from providers/**/Providerfile. Do not edit by hand.
-- Regenerate with: cargo run --bin providerfile
-- The application embeds this migration; Providerfiles are not read at runtime.
INSERT INTO providers (slug, operation, source_url, name, currencies, banks, source_file)
VALUES ('binance', 'buy', 'https://www.binance.com', 'Binance Buy', ARRAY['AMD', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], 'binance/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, source_file)
VALUES ('binance', 'sell', 'https://www.binance.com', 'Binance Sell', ARRAY['AMD', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], 'binance/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, source_file)
VALUES ('bitget', 'buy', 'https://www.bitget.com', 'Bitget Buy', ARRAY['AMD', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], 'bitget/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, source_file)
VALUES ('bitget', 'sell', 'https://www.bitget.com', 'Bitget Sell', ARRAY['AMD', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], 'bitget/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, source_file)
VALUES ('bybit', 'buy', 'https://www.bybit.com', 'Bybit Buy', ARRAY['AMD', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], 'bybit/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, source_file)
VALUES ('bybit', 'sell', 'https://www.bybit.com', 'Bybit Sell', ARRAY['AMD', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], 'bybit/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, source_file)
VALUES ('cifra-broker', 'buy', 'https://cifra-broker.ru', 'Cifra Broker Buy', ARRAY['CNY', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], 'cifra-broker/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, source_file)
VALUES ('cifra-broker', 'sell', 'https://cifra-broker.ru', 'Cifra Broker Sell', ARRAY['CNY', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], 'cifra-broker/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, source_file)
VALUES ('okx', 'buy', 'https://www.okx.com', 'OKX Buy', ARRAY['AMD', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], 'okx/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, source_file)
VALUES ('okx', 'sell', 'https://www.okx.com', 'OKX Sell', ARRAY['AMD', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], 'okx/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, source_file)
VALUES ('rapira', 'buy', 'https://rapira.net', 'Rapira Buy', ARRAY['RUB']::TEXT[], ARRAY[]::TEXT[], 'rapira/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, source_file)
VALUES ('rapira', 'sell', 'https://rapira.net', 'Rapira Sell', ARRAY['RUB']::TEXT[], ARRAY[]::TEXT[], 'rapira/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, source_file)
VALUES ('rate-am', 'buy', 'https://rate.am', 'Rate Buy', ARRAY['AMD', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], 'rate-am/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, source_file)
VALUES ('rate-am', 'sell', 'https://rate.am', 'Rate Sell', ARRAY['AMD', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], 'rate-am/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, source_file)
VALUES ('whitebird', 'buy', 'https://whitebird.io', 'Whitebird Buy', ARRAY['BYN', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], 'whitebird/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, source_file)
VALUES ('whitebird', 'sell', 'https://whitebird.io', 'Whitebird Sell', ARRAY['BYN', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], 'whitebird/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
source_file = EXCLUDED.source_file,
updated_at = now();
