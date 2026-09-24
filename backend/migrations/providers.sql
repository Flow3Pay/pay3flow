-- Generated from providers/**/Providerfile. Do not edit by hand.
-- Regenerate with: cargo run --bin providerfile
-- The application embeds this migration; Providerfiles are not read at runtime.
DELETE FROM providers
WHERE source_file LIKE '%/Providerfile'
AND (slug, operation) NOT IN (('bestchange', 'buy'), ('bestchange', 'sell'), ('binance', 'buy'), ('binance', 'sell'), ('bitget', 'buy'), ('bitget', 'sell'), ('bybit', 'buy'), ('bybit', 'sell'), ('cifra-broker', 'buy'), ('cifra-broker', 'sell'), ('dzengi', 'buy'), ('dzengi', 'sell'), ('exnode', 'buy'), ('exnode', 'sell'), ('okx', 'buy'), ('okx', 'sell'), ('rapira', 'buy'), ('rapira', 'sell'), ('whitebird', 'buy'), ('whitebird', 'sell'));

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, adapter, workflow, source_file)
VALUES ('bestchange', 'buy', 'https://bestchange.biz/ru', 'BestChange Buy', ARRAY['BYN', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], '{"p2p":null,"market":null,"bestchange":{"endpoint":"https://bestchange.biz","language":"ru","timeout_ms":10000,"max_results":100}}'::JSONB, '{}'::JSONB, 'bestchange/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, adapter, workflow, source_file)
VALUES ('bestchange', 'sell', 'https://bestchange.biz/ru', 'BestChange Sell', ARRAY['BYN', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], '{"p2p":null,"market":null,"bestchange":{"endpoint":"https://bestchange.biz","language":"ru","timeout_ms":10000,"max_results":100}}'::JSONB, '{}'::JSONB, 'bestchange/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, adapter, workflow, source_file)
VALUES ('binance', 'buy', 'https://www.binance.com', 'Binance Buy', ARRAY['AMD', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], '{"p2p":{"kind":"http_json","endpoint":"https://p2p.binance.com/bapi/c2c/v2/friendly/c2c/adv/search","method":"POST","headers":{"Origin":"https://www.binance.com","Referer":"https://www.binance.com/"},"asset_codes":{},"supported_assets":["USDT","USDC","BTC","ETH","BNB"],"timeout_ms":4000,"max_results":20,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":null,"default_max_fiat":null,"default_available_asset":null,"buy":{"query":{},"request_json":"{\"fiat\":\"{{fiat}}\",\"page\":1,\"rows\":\"{{limit_number}}\",\"tradeType\":\"BUY\",\"asset\":\"{{asset}}\",\"countries\":[],\"proMerchantAds\":false,\"shieldMerchantAds\":false,\"publisherType\":null,\"payTypes\":[],\"additionalKycVerifyFilter\":0}","amount_mode":"query_or_empty","items_pointer":"/data","success_pointer":"/code","success_value":"000000","success_missing_allowed":false,"error_pointer":"/message","offer":null},"sell":{"query":{},"request_json":"{\"fiat\":\"{{fiat}}\",\"page\":1,\"rows\":\"{{limit_number}}\",\"tradeType\":\"SELL\",\"asset\":\"{{asset}}\",\"countries\":[],\"proMerchantAds\":false,\"shieldMerchantAds\":false,\"publisherType\":null,\"payTypes\":[],\"additionalKycVerifyFilter\":0}","amount_mode":"query_or_empty","items_pointer":"/data","success_pointer":"/code","success_value":"000000","success_missing_allowed":false,"error_pointer":"/message","offer":null},"offer":{"ad_id_pointer":"/adv/advNo","fiat_pointer":"/adv/fiatUnit","asset_pointer":"/adv/asset","price_pointer":"/adv/price","fiat_amount_pointer":null,"asset_amount_pointer":null,"price_inverted":false,"available_asset_pointer":"/adv/tradableQuantity","min_fiat_pointer":"/adv/minSingleTransAmount","max_fiat_pointer":"/adv/maxSingleTransAmount","payment_methods_pointer":"/adv/tradeMethods","payment_method_value_pointer":"/tradeMethodName","payment_method_fallback_pointer":"/identifier","pay_time_limit_pointer":"/adv/payTimeLimit","advertiser_id_pointer":"/advertiser/userNo","advertiser_nickname_pointer":"/advertiser/nickName","advertiser_user_type_pointer":"/advertiser/userType","merchant_conditions":[{"pointer":"/advertiser/merchantGroupMember","operator":"truthy","value":null},{"pointer":"/advertiser/userType","operator":"equals_ci","value":"merchant"}],"verified_conditions":[],"merchant_default":false,"verified_default":false,"verified_from_merchant":true,"completed_orders_pointer":"/advertiser/monthOrderCount","completion_rate_pointer":"/advertiser/monthFinishRate","positive_rate_pointer":"/advertiser/positiveRate","source_url_template":"https://c2c.binance.com/en/adv?code={{item:/adv/advNo}}","source_url_is_exact":true,"advertiser_profile_url_template":"https://c2c.binance.com/en/advertiserDetail?advertiserNo={{item:/advertiser/userNo}}"},"rate_table":null},"market":{"kind":"http_json","endpoint":"https://api.binance.com/api/v3/ticker/bookTicker","method":"GET","headers":{},"query":{},"request_json":null,"timeout_ms":4000,"items_pointer":null,"symbol_pointer":"/symbol","bid_pointer":"/bidPrice","ask_pointer":"/askPrice","symbol_remove":null,"success_pointer":null,"success_value":null,"success_missing_allowed":false,"error_pointer":null},"bestchange":null}'::JSONB, '{}'::JSONB, 'binance/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, adapter, workflow, source_file)
VALUES ('binance', 'sell', 'https://www.binance.com', 'Binance Sell', ARRAY['AMD', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], '{"p2p":{"kind":"http_json","endpoint":"https://p2p.binance.com/bapi/c2c/v2/friendly/c2c/adv/search","method":"POST","headers":{"Origin":"https://www.binance.com","Referer":"https://www.binance.com/"},"asset_codes":{},"supported_assets":["USDT","USDC","BTC","ETH","BNB"],"timeout_ms":4000,"max_results":20,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":null,"default_max_fiat":null,"default_available_asset":null,"buy":{"query":{},"request_json":"{\"fiat\":\"{{fiat}}\",\"page\":1,\"rows\":\"{{limit_number}}\",\"tradeType\":\"BUY\",\"asset\":\"{{asset}}\",\"countries\":[],\"proMerchantAds\":false,\"shieldMerchantAds\":false,\"publisherType\":null,\"payTypes\":[],\"additionalKycVerifyFilter\":0}","amount_mode":"query_or_empty","items_pointer":"/data","success_pointer":"/code","success_value":"000000","success_missing_allowed":false,"error_pointer":"/message","offer":null},"sell":{"query":{},"request_json":"{\"fiat\":\"{{fiat}}\",\"page\":1,\"rows\":\"{{limit_number}}\",\"tradeType\":\"SELL\",\"asset\":\"{{asset}}\",\"countries\":[],\"proMerchantAds\":false,\"shieldMerchantAds\":false,\"publisherType\":null,\"payTypes\":[],\"additionalKycVerifyFilter\":0}","amount_mode":"query_or_empty","items_pointer":"/data","success_pointer":"/code","success_value":"000000","success_missing_allowed":false,"error_pointer":"/message","offer":null},"offer":{"ad_id_pointer":"/adv/advNo","fiat_pointer":"/adv/fiatUnit","asset_pointer":"/adv/asset","price_pointer":"/adv/price","fiat_amount_pointer":null,"asset_amount_pointer":null,"price_inverted":false,"available_asset_pointer":"/adv/tradableQuantity","min_fiat_pointer":"/adv/minSingleTransAmount","max_fiat_pointer":"/adv/maxSingleTransAmount","payment_methods_pointer":"/adv/tradeMethods","payment_method_value_pointer":"/tradeMethodName","payment_method_fallback_pointer":"/identifier","pay_time_limit_pointer":"/adv/payTimeLimit","advertiser_id_pointer":"/advertiser/userNo","advertiser_nickname_pointer":"/advertiser/nickName","advertiser_user_type_pointer":"/advertiser/userType","merchant_conditions":[{"pointer":"/advertiser/merchantGroupMember","operator":"truthy","value":null},{"pointer":"/advertiser/userType","operator":"equals_ci","value":"merchant"}],"verified_conditions":[],"merchant_default":false,"verified_default":false,"verified_from_merchant":true,"completed_orders_pointer":"/advertiser/monthOrderCount","completion_rate_pointer":"/advertiser/monthFinishRate","positive_rate_pointer":"/advertiser/positiveRate","source_url_template":"https://c2c.binance.com/en/adv?code={{item:/adv/advNo}}","source_url_is_exact":true,"advertiser_profile_url_template":"https://c2c.binance.com/en/advertiserDetail?advertiserNo={{item:/advertiser/userNo}}"},"rate_table":null},"market":{"kind":"http_json","endpoint":"https://api.binance.com/api/v3/ticker/bookTicker","method":"GET","headers":{},"query":{},"request_json":null,"timeout_ms":4000,"items_pointer":null,"symbol_pointer":"/symbol","bid_pointer":"/bidPrice","ask_pointer":"/askPrice","symbol_remove":null,"success_pointer":null,"success_value":null,"success_missing_allowed":false,"error_pointer":null},"bestchange":null}'::JSONB, '{}'::JSONB, 'binance/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, adapter, workflow, source_file)
VALUES ('bitget', 'buy', 'https://www.bitget.com', 'Bitget Buy', ARRAY['AMD', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], '{"p2p":{"kind":"http_json","endpoint":"https://www.bitget.com/v1/p2p/pub/adv/queryAdvList","method":"POST","headers":{"Origin":"https://www.bitget.com","Referer":"https://www.bitget.com/p2p-trade"},"asset_codes":{},"supported_assets":["USDT","USDC","BTC","ETH"],"timeout_ms":4000,"max_results":null,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":null,"default_max_fiat":null,"default_available_asset":null,"buy":{"query":{},"request_json":"{\"side\":1,\"pageNo\":1,\"pageSize\":\"{{limit_number}}\",\"coinCode\":\"{{asset}}\",\"fiatCode\":\"{{fiat}}\"}","amount_mode":"query_or_empty","items_pointer":"/data/dataList","success_pointer":"/code","success_value":"00000","success_missing_allowed":false,"error_pointer":"/msg","offer":null},"sell":{"query":{},"request_json":"{\"side\":2,\"pageNo\":1,\"pageSize\":\"{{limit_number}}\",\"coinCode\":\"{{asset}}\",\"fiatCode\":\"{{fiat}}\"}","amount_mode":"query_or_empty","items_pointer":"/data/dataList","success_pointer":"/code","success_value":"00000","success_missing_allowed":false,"error_pointer":"/msg","offer":null},"offer":{"ad_id_pointer":"/adNo","fiat_pointer":"/fiatCode","asset_pointer":"/coinCode","price_pointer":"/price","fiat_amount_pointer":null,"asset_amount_pointer":null,"price_inverted":false,"available_asset_pointer":"/editAmount","min_fiat_pointer":"/minAmount","max_fiat_pointer":"/maxAmount","payment_methods_pointer":"/paymethodInfo","payment_method_value_pointer":"/paymethodName","payment_method_fallback_pointer":null,"pay_time_limit_pointer":"/payDuration","advertiser_id_pointer":"/encryptUserId","advertiser_nickname_pointer":"/nickName","advertiser_user_type_pointer":"/certifiedMerchant","merchant_conditions":[{"pointer":"/certifiedMerchant","operator":"truthy","value":null}],"verified_conditions":[],"merchant_default":false,"verified_default":false,"verified_from_merchant":true,"completed_orders_pointer":"/thirtyTunoverNum","completion_rate_pointer":"/thirtyCompletionRate","positive_rate_pointer":"/goodEvaluationRate","source_url_template":"https://www.bitget.com/p2p-trade/{{side}}?fiatName={{fiat}}","source_url_is_exact":false,"advertiser_profile_url_template":"https://www.bitget.com/p2p-trade/user/{{item:/encryptUserId}}"},"rate_table":null},"market":{"kind":"http_json","endpoint":"https://api.bitget.com/api/v2/spot/market/tickers","method":"GET","headers":{},"query":{},"request_json":null,"timeout_ms":4000,"items_pointer":"/data","symbol_pointer":"/symbol","bid_pointer":"/bidPr","ask_pointer":"/askPr","symbol_remove":null,"success_pointer":"/code","success_value":"00000","success_missing_allowed":false,"error_pointer":"/msg"},"bestchange":null}'::JSONB, '{}'::JSONB, 'bitget/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, adapter, workflow, source_file)
VALUES ('bitget', 'sell', 'https://www.bitget.com', 'Bitget Sell', ARRAY['AMD', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], '{"p2p":{"kind":"http_json","endpoint":"https://www.bitget.com/v1/p2p/pub/adv/queryAdvList","method":"POST","headers":{"Origin":"https://www.bitget.com","Referer":"https://www.bitget.com/p2p-trade"},"asset_codes":{},"supported_assets":["USDT","USDC","BTC","ETH"],"timeout_ms":4000,"max_results":null,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":null,"default_max_fiat":null,"default_available_asset":null,"buy":{"query":{},"request_json":"{\"side\":1,\"pageNo\":1,\"pageSize\":\"{{limit_number}}\",\"coinCode\":\"{{asset}}\",\"fiatCode\":\"{{fiat}}\"}","amount_mode":"query_or_empty","items_pointer":"/data/dataList","success_pointer":"/code","success_value":"00000","success_missing_allowed":false,"error_pointer":"/msg","offer":null},"sell":{"query":{},"request_json":"{\"side\":2,\"pageNo\":1,\"pageSize\":\"{{limit_number}}\",\"coinCode\":\"{{asset}}\",\"fiatCode\":\"{{fiat}}\"}","amount_mode":"query_or_empty","items_pointer":"/data/dataList","success_pointer":"/code","success_value":"00000","success_missing_allowed":false,"error_pointer":"/msg","offer":null},"offer":{"ad_id_pointer":"/adNo","fiat_pointer":"/fiatCode","asset_pointer":"/coinCode","price_pointer":"/price","fiat_amount_pointer":null,"asset_amount_pointer":null,"price_inverted":false,"available_asset_pointer":"/editAmount","min_fiat_pointer":"/minAmount","max_fiat_pointer":"/maxAmount","payment_methods_pointer":"/paymethodInfo","payment_method_value_pointer":"/paymethodName","payment_method_fallback_pointer":null,"pay_time_limit_pointer":"/payDuration","advertiser_id_pointer":"/encryptUserId","advertiser_nickname_pointer":"/nickName","advertiser_user_type_pointer":"/certifiedMerchant","merchant_conditions":[{"pointer":"/certifiedMerchant","operator":"truthy","value":null}],"verified_conditions":[],"merchant_default":false,"verified_default":false,"verified_from_merchant":true,"completed_orders_pointer":"/thirtyTunoverNum","completion_rate_pointer":"/thirtyCompletionRate","positive_rate_pointer":"/goodEvaluationRate","source_url_template":"https://www.bitget.com/p2p-trade/{{side}}?fiatName={{fiat}}","source_url_is_exact":false,"advertiser_profile_url_template":"https://www.bitget.com/p2p-trade/user/{{item:/encryptUserId}}"},"rate_table":null},"market":{"kind":"http_json","endpoint":"https://api.bitget.com/api/v2/spot/market/tickers","method":"GET","headers":{},"query":{},"request_json":null,"timeout_ms":4000,"items_pointer":"/data","symbol_pointer":"/symbol","bid_pointer":"/bidPr","ask_pointer":"/askPr","symbol_remove":null,"success_pointer":"/code","success_value":"00000","success_missing_allowed":false,"error_pointer":"/msg"},"bestchange":null}'::JSONB, '{}'::JSONB, 'bitget/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, adapter, workflow, source_file)
VALUES ('bybit', 'buy', 'https://www.bybit.com', 'Bybit Buy', ARRAY['AMD', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], '{"p2p":{"kind":"http_json","endpoint":"https://api2.bybit.com/fiat/otc/item/online","method":"POST","headers":{"Origin":"https://www.bybit.com","Referer":"https://www.bybit.com/"},"asset_codes":{},"supported_assets":["USDT","USDC","BTC","ETH"],"timeout_ms":4000,"max_results":null,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":null,"default_max_fiat":null,"default_available_asset":null,"buy":{"query":{},"request_json":"{\"userId\":\"\",\"tokenId\":\"{{asset}}\",\"currencyId\":\"{{fiat}}\",\"payment\":[],\"side\":\"1\",\"size\":\"{{limit}}\",\"page\":\"1\",\"amount\":\"{{amount}}\",\"authMaker\":false,\"canTrade\":false}","amount_mode":"query_or_empty","items_pointer":"/result/items","success_pointer":"/ret_code","success_value":"0","success_missing_allowed":false,"error_pointer":"/ret_msg","offer":null},"sell":{"query":{},"request_json":"{\"userId\":\"\",\"tokenId\":\"{{asset}}\",\"currencyId\":\"{{fiat}}\",\"payment\":[],\"side\":\"0\",\"size\":\"{{limit}}\",\"page\":\"1\",\"amount\":\"{{amount}}\",\"authMaker\":false,\"canTrade\":false}","amount_mode":"query_or_empty","items_pointer":"/result/items","success_pointer":"/ret_code","success_value":"0","success_missing_allowed":false,"error_pointer":"/ret_msg","offer":null},"offer":{"ad_id_pointer":"/id","fiat_pointer":"/currencyId","asset_pointer":"/tokenId","price_pointer":"/price","fiat_amount_pointer":null,"asset_amount_pointer":null,"price_inverted":false,"available_asset_pointer":"/quantity","min_fiat_pointer":"/minAmount","max_fiat_pointer":"/maxAmount","payment_methods_pointer":"/payments","payment_method_value_pointer":null,"payment_method_fallback_pointer":null,"pay_time_limit_pointer":"/paymentPeriod","advertiser_id_pointer":"/userMaskId","advertiser_nickname_pointer":"/nickName","advertiser_user_type_pointer":"/userType","merchant_conditions":[{"pointer":"/authTag","operator":"non_empty","value":null},{"pointer":"/userType","operator":"not_equals_ci","value":"personal"}],"verified_conditions":[{"pointer":"/authStatus","operator":"equals","value":"1"}],"merchant_default":false,"verified_default":false,"verified_from_merchant":false,"completed_orders_pointer":"/recentOrderNum","completion_rate_pointer":"/recentExecuteRate","positive_rate_pointer":null,"source_url_template":"https://www.bybit.com/fiat/trade/otc/?token={{asset}}&fiat={{fiat}}","source_url_is_exact":false,"advertiser_profile_url_template":"https://www.bybit.com/en/p2p/profile/{{item:/userMaskId}}/{{asset}}/{{fiat}}/item"},"rate_table":null},"market":{"kind":"http_json","endpoint":"https://api.bybit.com/v5/market/tickers","method":"GET","headers":{},"query":{"category":"spot"},"request_json":null,"timeout_ms":4000,"items_pointer":"/result/list","symbol_pointer":"/symbol","bid_pointer":"/bid1Price","ask_pointer":"/ask1Price","symbol_remove":null,"success_pointer":"/retCode","success_value":"0","success_missing_allowed":false,"error_pointer":"/retMsg"},"bestchange":null}'::JSONB, '{}'::JSONB, 'bybit/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, adapter, workflow, source_file)
VALUES ('bybit', 'sell', 'https://www.bybit.com', 'Bybit Sell', ARRAY['AMD', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], '{"p2p":{"kind":"http_json","endpoint":"https://api2.bybit.com/fiat/otc/item/online","method":"POST","headers":{"Origin":"https://www.bybit.com","Referer":"https://www.bybit.com/"},"asset_codes":{},"supported_assets":["USDT","USDC","BTC","ETH"],"timeout_ms":4000,"max_results":null,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":null,"default_max_fiat":null,"default_available_asset":null,"buy":{"query":{},"request_json":"{\"userId\":\"\",\"tokenId\":\"{{asset}}\",\"currencyId\":\"{{fiat}}\",\"payment\":[],\"side\":\"1\",\"size\":\"{{limit}}\",\"page\":\"1\",\"amount\":\"{{amount}}\",\"authMaker\":false,\"canTrade\":false}","amount_mode":"query_or_empty","items_pointer":"/result/items","success_pointer":"/ret_code","success_value":"0","success_missing_allowed":false,"error_pointer":"/ret_msg","offer":null},"sell":{"query":{},"request_json":"{\"userId\":\"\",\"tokenId\":\"{{asset}}\",\"currencyId\":\"{{fiat}}\",\"payment\":[],\"side\":\"0\",\"size\":\"{{limit}}\",\"page\":\"1\",\"amount\":\"{{amount}}\",\"authMaker\":false,\"canTrade\":false}","amount_mode":"query_or_empty","items_pointer":"/result/items","success_pointer":"/ret_code","success_value":"0","success_missing_allowed":false,"error_pointer":"/ret_msg","offer":null},"offer":{"ad_id_pointer":"/id","fiat_pointer":"/currencyId","asset_pointer":"/tokenId","price_pointer":"/price","fiat_amount_pointer":null,"asset_amount_pointer":null,"price_inverted":false,"available_asset_pointer":"/quantity","min_fiat_pointer":"/minAmount","max_fiat_pointer":"/maxAmount","payment_methods_pointer":"/payments","payment_method_value_pointer":null,"payment_method_fallback_pointer":null,"pay_time_limit_pointer":"/paymentPeriod","advertiser_id_pointer":"/userMaskId","advertiser_nickname_pointer":"/nickName","advertiser_user_type_pointer":"/userType","merchant_conditions":[{"pointer":"/authTag","operator":"non_empty","value":null},{"pointer":"/userType","operator":"not_equals_ci","value":"personal"}],"verified_conditions":[{"pointer":"/authStatus","operator":"equals","value":"1"}],"merchant_default":false,"verified_default":false,"verified_from_merchant":false,"completed_orders_pointer":"/recentOrderNum","completion_rate_pointer":"/recentExecuteRate","positive_rate_pointer":null,"source_url_template":"https://www.bybit.com/fiat/trade/otc/?token={{asset}}&fiat={{fiat}}","source_url_is_exact":false,"advertiser_profile_url_template":"https://www.bybit.com/en/p2p/profile/{{item:/userMaskId}}/{{asset}}/{{fiat}}/item"},"rate_table":null},"market":{"kind":"http_json","endpoint":"https://api.bybit.com/v5/market/tickers","method":"GET","headers":{},"query":{"category":"spot"},"request_json":null,"timeout_ms":4000,"items_pointer":"/result/list","symbol_pointer":"/symbol","bid_pointer":"/bid1Price","ask_pointer":"/ask1Price","symbol_remove":null,"success_pointer":"/retCode","success_value":"0","success_missing_allowed":false,"error_pointer":"/retMsg"},"bestchange":null}'::JSONB, '{}'::JSONB, 'bybit/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, adapter, workflow, source_file)
VALUES ('cifra-broker', 'buy', 'https://cifra.by/', 'Cifra Markets Buy', ARRAY['BYN', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], '{"p2p":{"kind":"http_json","endpoint":"https://api.cifra-broker.by/api/site/ticker-calculator","method":"GET","headers":{"Accept-Language":"ru-RU,ru;q=0.9","Origin":"https://cifra.by","Referer":"https://cifra.by/","User-Agent":"Mozilla/5.0 (compatible; Pay3Flow/0.1)"},"asset_codes":{},"supported_assets":["USDT","USDC","BTC","BNB","SOL","TRX","DOGE","LTC","DAI","XRP","ADA","DOT","LINK","AVAX","BCH","NEAR","APT","ATOM","UNI","SUI"],"timeout_ms":5000,"max_results":null,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":1.0,"default_max_fiat":1000000000.0,"default_available_asset":1000000000.0,"buy":{"query":{"key":"pay3flow"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":null,"success_pointer":"/success","success_value":"true","success_missing_allowed":false,"error_pointer":"/message","offer":null},"sell":{"query":{"key":"pay3flow"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":null,"success_pointer":"/success","success_value":"true","success_missing_allowed":false,"error_pointer":"/message","offer":null},"offer":null,"rate_table":{"fiat_items_pointer":"/data/currenciesReal","fiat_code_pointer":"/code","fiat_rate_pointer":"/rate/value","asset_items_pointer":"/data/currenciesNotReal","asset_code_pointer":"/code","asset_rates_pointer":"/data/currenciesNotRealRate","fiat_codes":{"RUB":"RUR"},"source_url":"https://tradernet.by/authentication/signup"}},"market":{"kind":"http_json","endpoint":"https://api.cifra-broker.by/api/site/ticker","method":"POST","headers":{"Accept-Language":"ru-RU,ru;q=0.9","Origin":"https://cifra.by","Referer":"https://cifra.by/catalog","User-Agent":"Mozilla/5.0 (compatible; Pay3Flow/0.1)"},"query":{},"request_json":"{\"limit\":1000,\"tags\":[]}","timeout_ms":5000,"items_pointer":"/data/ticker","symbol_pointer":"/name","bid_pointer":"/ltp","ask_pointer":"/ltp","symbol_remove":"-","success_pointer":"/success","success_value":"true","success_missing_allowed":false,"error_pointer":"/message"},"bestchange":null}'::JSONB, '{}'::JSONB, 'cifra-broker/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, adapter, workflow, source_file)
VALUES ('cifra-broker', 'sell', 'https://cifra.by/', 'Cifra Markets Sell', ARRAY['BYN', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], '{"p2p":{"kind":"http_json","endpoint":"https://api.cifra-broker.by/api/site/ticker-calculator","method":"GET","headers":{"Accept-Language":"ru-RU,ru;q=0.9","Origin":"https://cifra.by","Referer":"https://cifra.by/","User-Agent":"Mozilla/5.0 (compatible; Pay3Flow/0.1)"},"asset_codes":{},"supported_assets":["USDT","USDC","BTC","BNB","SOL","TRX","DOGE","LTC","DAI","XRP","ADA","DOT","LINK","AVAX","BCH","NEAR","APT","ATOM","UNI","SUI"],"timeout_ms":5000,"max_results":null,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":1.0,"default_max_fiat":1000000000.0,"default_available_asset":1000000000.0,"buy":{"query":{"key":"pay3flow"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":null,"success_pointer":"/success","success_value":"true","success_missing_allowed":false,"error_pointer":"/message","offer":null},"sell":{"query":{"key":"pay3flow"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":null,"success_pointer":"/success","success_value":"true","success_missing_allowed":false,"error_pointer":"/message","offer":null},"offer":null,"rate_table":{"fiat_items_pointer":"/data/currenciesReal","fiat_code_pointer":"/code","fiat_rate_pointer":"/rate/value","asset_items_pointer":"/data/currenciesNotReal","asset_code_pointer":"/code","asset_rates_pointer":"/data/currenciesNotRealRate","fiat_codes":{"RUB":"RUR"},"source_url":"https://tradernet.by/authentication/signup"}},"market":{"kind":"http_json","endpoint":"https://api.cifra-broker.by/api/site/ticker","method":"POST","headers":{"Accept-Language":"ru-RU,ru;q=0.9","Origin":"https://cifra.by","Referer":"https://cifra.by/catalog","User-Agent":"Mozilla/5.0 (compatible; Pay3Flow/0.1)"},"query":{},"request_json":"{\"limit\":1000,\"tags\":[]}","timeout_ms":5000,"items_pointer":"/data/ticker","symbol_pointer":"/name","bid_pointer":"/ltp","ask_pointer":"/ltp","symbol_remove":"-","success_pointer":"/success","success_value":"true","success_missing_allowed":false,"error_pointer":"/message"},"bestchange":null}'::JSONB, '{}'::JSONB, 'cifra-broker/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, adapter, workflow, source_file)
VALUES ('dzengi', 'buy', 'https://dzengi.com/ru/kalkulyator-kriptovalyut', 'Dzengi Buy', ARRAY['BYN', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], '{}'::JSONB, '{}'::JSONB, 'dzengi/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, adapter, workflow, source_file)
VALUES ('dzengi', 'sell', 'https://dzengi.com/ru/kalkulyator-kriptovalyut', 'Dzengi Sell', ARRAY['BYN', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], '{}'::JSONB, '{}'::JSONB, 'dzengi/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, adapter, workflow, source_file)
VALUES ('exnode', 'buy', 'https://exnode.ru/exchange', 'Exnode Buy', ARRAY['BYN', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], '{}'::JSONB, '{}'::JSONB, 'exnode/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, adapter, workflow, source_file)
VALUES ('exnode', 'sell', 'https://exnode.ru/exchange', 'Exnode Sell', ARRAY['BYN', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], '{}'::JSONB, '{}'::JSONB, 'exnode/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, adapter, workflow, source_file)
VALUES ('okx', 'buy', 'https://www.okx.com', 'OKX Buy', ARRAY['AMD', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], '{"p2p":{"kind":"http_json","endpoint":"https://www.okx.com/v3/c2c/tradingOrders/books","method":"GET","headers":{"Origin":"https://www.okx.com","Referer":"https://www.okx.com/p2p-markets/"},"asset_codes":{},"supported_assets":["USDT","USDC","BTC","ETH"],"timeout_ms":10000,"max_results":null,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":null,"default_max_fiat":null,"default_available_asset":null,"buy":{"query":{"baseCurrency":"{{asset}}","isAbleFilter":"false","paymentMethod":"all","quoteCurrency":"{{fiat}}","showAlreadyTraded":"false","showFollow":"false","showTrade":"false","side":"sell","urlId":"0","userType":"all"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":"/data/sell","success_pointer":"/code","success_value":"0","success_missing_allowed":false,"error_pointer":"/detailMsg","offer":null},"sell":{"query":{"baseCurrency":"{{asset}}","isAbleFilter":"false","paymentMethod":"all","quoteCurrency":"{{fiat}}","showAlreadyTraded":"false","showFollow":"false","showTrade":"false","side":"buy","urlId":"0","userType":"all"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":"/data/buy","success_pointer":"/code","success_value":"0","success_missing_allowed":false,"error_pointer":"/detailMsg","offer":null},"offer":{"ad_id_pointer":"/id","fiat_pointer":"/quoteCurrency","asset_pointer":"/baseCurrency","price_pointer":"/price","fiat_amount_pointer":null,"asset_amount_pointer":null,"price_inverted":false,"available_asset_pointer":"/availableAmount","min_fiat_pointer":"/quoteMinAmountPerOrder","max_fiat_pointer":"/quoteMaxAmountPerOrder","payment_methods_pointer":"/paymentMethods","payment_method_value_pointer":null,"payment_method_fallback_pointer":null,"pay_time_limit_pointer":"/paymentTimeoutMinutes","advertiser_id_pointer":"/publicUserId","advertiser_nickname_pointer":"/nickName","advertiser_user_type_pointer":"/userType","merchant_conditions":[{"pointer":"/isInstitution","operator":"truthy","value":null},{"pointer":"/merchantId","operator":"non_empty","value":null},{"pointer":"/userType","operator":"equals_ci","value":"merchant"}],"verified_conditions":[],"merchant_default":false,"verified_default":false,"verified_from_merchant":true,"completed_orders_pointer":"/completedOrderQuantity","completion_rate_pointer":"/completedRate","positive_rate_pointer":"/posReviewPercentage","source_url_template":"https://www.okx.com/p2p-markets/{{fiat_lower}}/{{side}}-{{asset_lower}}","source_url_is_exact":false,"advertiser_profile_url_template":"https://www.okx.com/p2p/ads-merchant?publicUserId={{item:/publicUserId}}"},"rate_table":null},"market":{"kind":"http_json","endpoint":"https://www.okx.com/api/v5/market/tickers","method":"GET","headers":{},"query":{"instType":"SPOT"},"request_json":null,"timeout_ms":10000,"items_pointer":"/data","symbol_pointer":"/instId","bid_pointer":"/bidPx","ask_pointer":"/askPx","symbol_remove":"-","success_pointer":"/code","success_value":"0","success_missing_allowed":false,"error_pointer":"/msg"},"bestchange":null}'::JSONB, '{}'::JSONB, 'okx/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, adapter, workflow, source_file)
VALUES ('okx', 'sell', 'https://www.okx.com', 'OKX Sell', ARRAY['AMD', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], '{"p2p":{"kind":"http_json","endpoint":"https://www.okx.com/v3/c2c/tradingOrders/books","method":"GET","headers":{"Origin":"https://www.okx.com","Referer":"https://www.okx.com/p2p-markets/"},"asset_codes":{},"supported_assets":["USDT","USDC","BTC","ETH"],"timeout_ms":10000,"max_results":null,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":null,"default_max_fiat":null,"default_available_asset":null,"buy":{"query":{"baseCurrency":"{{asset}}","isAbleFilter":"false","paymentMethod":"all","quoteCurrency":"{{fiat}}","showAlreadyTraded":"false","showFollow":"false","showTrade":"false","side":"sell","urlId":"0","userType":"all"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":"/data/sell","success_pointer":"/code","success_value":"0","success_missing_allowed":false,"error_pointer":"/detailMsg","offer":null},"sell":{"query":{"baseCurrency":"{{asset}}","isAbleFilter":"false","paymentMethod":"all","quoteCurrency":"{{fiat}}","showAlreadyTraded":"false","showFollow":"false","showTrade":"false","side":"buy","urlId":"0","userType":"all"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":"/data/buy","success_pointer":"/code","success_value":"0","success_missing_allowed":false,"error_pointer":"/detailMsg","offer":null},"offer":{"ad_id_pointer":"/id","fiat_pointer":"/quoteCurrency","asset_pointer":"/baseCurrency","price_pointer":"/price","fiat_amount_pointer":null,"asset_amount_pointer":null,"price_inverted":false,"available_asset_pointer":"/availableAmount","min_fiat_pointer":"/quoteMinAmountPerOrder","max_fiat_pointer":"/quoteMaxAmountPerOrder","payment_methods_pointer":"/paymentMethods","payment_method_value_pointer":null,"payment_method_fallback_pointer":null,"pay_time_limit_pointer":"/paymentTimeoutMinutes","advertiser_id_pointer":"/publicUserId","advertiser_nickname_pointer":"/nickName","advertiser_user_type_pointer":"/userType","merchant_conditions":[{"pointer":"/isInstitution","operator":"truthy","value":null},{"pointer":"/merchantId","operator":"non_empty","value":null},{"pointer":"/userType","operator":"equals_ci","value":"merchant"}],"verified_conditions":[],"merchant_default":false,"verified_default":false,"verified_from_merchant":true,"completed_orders_pointer":"/completedOrderQuantity","completion_rate_pointer":"/completedRate","positive_rate_pointer":"/posReviewPercentage","source_url_template":"https://www.okx.com/p2p-markets/{{fiat_lower}}/{{side}}-{{asset_lower}}","source_url_is_exact":false,"advertiser_profile_url_template":"https://www.okx.com/p2p/ads-merchant?publicUserId={{item:/publicUserId}}"},"rate_table":null},"market":{"kind":"http_json","endpoint":"https://www.okx.com/api/v5/market/tickers","method":"GET","headers":{},"query":{"instType":"SPOT"},"request_json":null,"timeout_ms":10000,"items_pointer":"/data","symbol_pointer":"/instId","bid_pointer":"/bidPx","ask_pointer":"/askPx","symbol_remove":"-","success_pointer":"/code","success_value":"0","success_missing_allowed":false,"error_pointer":"/msg"},"bestchange":null}'::JSONB, '{}'::JSONB, 'okx/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, adapter, workflow, source_file)
VALUES ('rapira', 'buy', 'https://rapira.net', 'Rapira Buy', ARRAY['RUB']::TEXT[], ARRAY[]::TEXT[], '{"p2p":{"kind":"http_json","endpoint":"https://api.rapira.net/otc/offers/page-query/v2","method":"GET","headers":{"Origin":"https://rapira.net","Referer":"https://rapira.net/ru/p2p/BUY?p=1"},"asset_codes":{},"supported_assets":["USDT"],"timeout_ms":4000,"max_results":100,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":null,"default_max_fiat":null,"default_available_asset":null,"buy":{"query":{"amount":"{{amount}}","externalCoinUnit":"{{fiat}}","internalCoinUnit":"{{asset}}","listingType":"RECOMMENDED","merchantSide":"SELL","pageNo":"1","pageSize":"{{limit}}","paymentIds":"","showOnlyEligible":"false","sortByNumberOfMutualOrders":"false"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":"/content","success_pointer":"/code","success_value":"0","success_missing_allowed":true,"error_pointer":"/message","offer":null},"sell":{"query":{"amount":"{{amount}}","externalCoinUnit":"{{fiat}}","internalCoinUnit":"{{asset}}","listingType":"RECOMMENDED","merchantSide":"BUY","pageNo":"1","pageSize":"{{limit}}","paymentIds":"","showOnlyEligible":"false","sortByNumberOfMutualOrders":"false"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":"/content","success_pointer":"/code","success_value":"0","success_missing_allowed":true,"error_pointer":"/message","offer":null},"offer":{"ad_id_pointer":"/advertiseId","fiat_pointer":"/externalCoinUnit","asset_pointer":"/internalCoinUnit","price_pointer":"/price","fiat_amount_pointer":null,"asset_amount_pointer":null,"price_inverted":false,"available_asset_pointer":"/quantity","min_fiat_pointer":"/minLimit","max_fiat_pointer":"/maxLimit","payment_methods_pointer":"/paymentTypes","payment_method_value_pointer":"/paymentName","payment_method_fallback_pointer":null,"pay_time_limit_pointer":"/timeLimit","advertiser_id_pointer":"/merchant/profileUid","advertiser_nickname_pointer":"/merchant/username","advertiser_user_type_pointer":"/merchant/p2pLevel","merchant_conditions":[],"verified_conditions":[{"pointer":"/merchant/p2pLevel","operator":"equals_ci","value":"verified"},{"pointer":"/merchant/p2pLevel","operator":"equals_ci","value":"trusted"},{"pointer":"/merchant/p2pLevel","operator":"equals_ci","value":"premium"}],"merchant_default":true,"verified_default":false,"verified_from_merchant":false,"completed_orders_pointer":"/merchant/totalTerminatedAfterAcceptCount","completion_rate_pointer":"/merchant/totalCompletedPercent","positive_rate_pointer":null,"source_url_template":"https://rapira.net/p2p?adId={{item:/advertiseId}}","source_url_is_exact":true,"advertiser_profile_url_template":"https://rapira.net/ru/p2p/profileUser?p=1&profileUid={{item:/merchant/profileUid}}&msp=1"},"rate_table":null},"market":null,"bestchange":null}'::JSONB, '{}'::JSONB, 'rapira/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, adapter, workflow, source_file)
VALUES ('rapira', 'sell', 'https://rapira.net', 'Rapira Sell', ARRAY['RUB']::TEXT[], ARRAY[]::TEXT[], '{"p2p":{"kind":"http_json","endpoint":"https://api.rapira.net/otc/offers/page-query/v2","method":"GET","headers":{"Origin":"https://rapira.net","Referer":"https://rapira.net/ru/p2p/BUY?p=1"},"asset_codes":{},"supported_assets":["USDT"],"timeout_ms":4000,"max_results":100,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":null,"default_max_fiat":null,"default_available_asset":null,"buy":{"query":{"amount":"{{amount}}","externalCoinUnit":"{{fiat}}","internalCoinUnit":"{{asset}}","listingType":"RECOMMENDED","merchantSide":"SELL","pageNo":"1","pageSize":"{{limit}}","paymentIds":"","showOnlyEligible":"false","sortByNumberOfMutualOrders":"false"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":"/content","success_pointer":"/code","success_value":"0","success_missing_allowed":true,"error_pointer":"/message","offer":null},"sell":{"query":{"amount":"{{amount}}","externalCoinUnit":"{{fiat}}","internalCoinUnit":"{{asset}}","listingType":"RECOMMENDED","merchantSide":"BUY","pageNo":"1","pageSize":"{{limit}}","paymentIds":"","showOnlyEligible":"false","sortByNumberOfMutualOrders":"false"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":"/content","success_pointer":"/code","success_value":"0","success_missing_allowed":true,"error_pointer":"/message","offer":null},"offer":{"ad_id_pointer":"/advertiseId","fiat_pointer":"/externalCoinUnit","asset_pointer":"/internalCoinUnit","price_pointer":"/price","fiat_amount_pointer":null,"asset_amount_pointer":null,"price_inverted":false,"available_asset_pointer":"/quantity","min_fiat_pointer":"/minLimit","max_fiat_pointer":"/maxLimit","payment_methods_pointer":"/paymentTypes","payment_method_value_pointer":"/paymentName","payment_method_fallback_pointer":null,"pay_time_limit_pointer":"/timeLimit","advertiser_id_pointer":"/merchant/profileUid","advertiser_nickname_pointer":"/merchant/username","advertiser_user_type_pointer":"/merchant/p2pLevel","merchant_conditions":[],"verified_conditions":[{"pointer":"/merchant/p2pLevel","operator":"equals_ci","value":"verified"},{"pointer":"/merchant/p2pLevel","operator":"equals_ci","value":"trusted"},{"pointer":"/merchant/p2pLevel","operator":"equals_ci","value":"premium"}],"merchant_default":true,"verified_default":false,"verified_from_merchant":false,"completed_orders_pointer":"/merchant/totalTerminatedAfterAcceptCount","completion_rate_pointer":"/merchant/totalCompletedPercent","positive_rate_pointer":null,"source_url_template":"https://rapira.net/p2p?adId={{item:/advertiseId}}","source_url_is_exact":true,"advertiser_profile_url_template":"https://rapira.net/ru/p2p/profileUser?p=1&profileUid={{item:/merchant/profileUid}}&msp=1"},"rate_table":null},"market":null,"bestchange":null}'::JSONB, '{}'::JSONB, 'rapira/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, adapter, workflow, source_file)
VALUES ('whitebird', 'buy', 'https://whitebird.io/', 'Whitebird Buy', ARRAY['BYN', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], '{}'::JSONB, '{"source_url":"https://whitebird.io/","get_exchange":"https://whitebird.io/exchanger","timeout_ms":30000,"navigation_retries":1,"navigation_retry_delay_ms":1000,"asset_codes":{},"supported_assets":["TRX","USDT","ETH","USDC","BTC","BNB","GRAM","SOL"],"fiat_probe_amount":1000.0,"asset_probe_amount":100.0,"default_min_fiat":1.0,"default_max_fiat":1000000000.0,"default_available_asset":1000000000.0,"is_merchant":true,"is_verified":true,"buy":{"amount_mode":"fiat_probe","steps":[{"action":"wait_for","selector":"input[name=\"amountFrom\"]"},{"action":"react_select","selector":"div:has(> input[name=\"currencyFrom\"]) > div","value":"{{fiat}}"},{"action":"wait","milliseconds":800},{"action":"react_select","selector":"div:has(> input[name=\"currencyTo\"]) > div","value":"{{asset}}"},{"action":"wait","milliseconds":800},{"action":"fill","selector":"input[name=\"amountFrom\"]","value":"{{amount}}"},{"action":"wait_for","selector":"input[name=\"amountTo\"]"},{"action":"wait","milliseconds":800}],"fiat_amount":{"selector":"input[name=\"amountFrom\"]","property":"value"},"asset_amount":{"selector":"input[name=\"amountTo\"]","property":"value"}},"sell":{"amount_mode":"asset_probe","steps":[{"action":"wait_for","selector":"input[name=\"amountFrom\"]"},{"action":"react_select","selector":"div:has(> input[name=\"currencyFrom\"]) > div","value":"{{asset}}"},{"action":"wait","milliseconds":800},{"action":"react_select","selector":"div:has(> input[name=\"currencyTo\"]) > div","value":"{{fiat}}"},{"action":"wait","milliseconds":800},{"action":"fill","selector":"input[name=\"amountFrom\"]","value":"{{amount}}"},{"action":"wait_for","selector":"input[name=\"amountTo\"]"},{"action":"wait","milliseconds":800}],"fiat_amount":{"selector":"input[name=\"amountTo\"]","property":"value"},"asset_amount":{"selector":"input[name=\"amountFrom\"]","property":"value"}}}'::JSONB, 'whitebird/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, adapter, workflow, source_file)
VALUES ('whitebird', 'sell', 'https://whitebird.io/', 'Whitebird Sell', ARRAY['BYN', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], '{}'::JSONB, '{"source_url":"https://whitebird.io/","get_exchange":"https://whitebird.io/exchanger","timeout_ms":30000,"navigation_retries":1,"navigation_retry_delay_ms":1000,"asset_codes":{},"supported_assets":["TRX","USDT","ETH","USDC","BTC","BNB","GRAM","SOL"],"fiat_probe_amount":1000.0,"asset_probe_amount":100.0,"default_min_fiat":1.0,"default_max_fiat":1000000000.0,"default_available_asset":1000000000.0,"is_merchant":true,"is_verified":true,"buy":{"amount_mode":"fiat_probe","steps":[{"action":"wait_for","selector":"input[name=\"amountFrom\"]"},{"action":"react_select","selector":"div:has(> input[name=\"currencyFrom\"]) > div","value":"{{fiat}}"},{"action":"wait","milliseconds":800},{"action":"react_select","selector":"div:has(> input[name=\"currencyTo\"]) > div","value":"{{asset}}"},{"action":"wait","milliseconds":800},{"action":"fill","selector":"input[name=\"amountFrom\"]","value":"{{amount}}"},{"action":"wait_for","selector":"input[name=\"amountTo\"]"},{"action":"wait","milliseconds":800}],"fiat_amount":{"selector":"input[name=\"amountFrom\"]","property":"value"},"asset_amount":{"selector":"input[name=\"amountTo\"]","property":"value"}},"sell":{"amount_mode":"asset_probe","steps":[{"action":"wait_for","selector":"input[name=\"amountFrom\"]"},{"action":"react_select","selector":"div:has(> input[name=\"currencyFrom\"]) > div","value":"{{asset}}"},{"action":"wait","milliseconds":800},{"action":"react_select","selector":"div:has(> input[name=\"currencyTo\"]) > div","value":"{{fiat}}"},{"action":"wait","milliseconds":800},{"action":"fill","selector":"input[name=\"amountFrom\"]","value":"{{amount}}"},{"action":"wait_for","selector":"input[name=\"amountTo\"]"},{"action":"wait","milliseconds":800}],"fiat_amount":{"selector":"input[name=\"amountTo\"]","property":"value"},"asset_amount":{"selector":"input[name=\"amountFrom\"]","property":"value"}}}'::JSONB, 'whitebird/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
source_file = EXCLUDED.source_file,
updated_at = now();
