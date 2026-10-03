-- Generated from providers/**/Providerfile. Do not edit by hand.
-- Regenerate with: cargo run --bin providerfile
-- The application embeds this migration; Providerfiles are not read at runtime.
DELETE FROM providers
WHERE source_file LIKE '%/Providerfile'
AND (slug, operation) NOT IN (('0x', 'buy'), ('0x', 'sell'), ('1inch', 'buy'), ('1inch', 'sell'), ('bebop', 'buy'), ('bebop', 'sell'), ('bestchange', 'buy'), ('bestchange', 'sell'), ('binance', 'buy'), ('binance', 'sell'), ('bitcoin-center', 'buy'), ('bitcoin-center', 'sell'), ('bitget', 'buy'), ('bitget', 'sell'), ('bncex', 'buy'), ('bncex', 'sell'), ('bybit', 'buy'), ('bybit', 'sell'), ('cifra-broker', 'buy'), ('cifra-broker', 'sell'), ('cow-swap', 'buy'), ('cow-swap', 'sell'), ('dzengi', 'buy'), ('dzengi', 'sell'), ('enso', 'buy'), ('enso', 'sell'), ('id-pay', 'buy'), ('id-pay', 'sell'), ('kyberswap', 'buy'), ('kyberswap', 'sell'), ('lifi', 'buy'), ('lifi', 'sell'), ('mexc', 'buy'), ('mexc', 'sell'), ('near-intents', 'buy'), ('near-intents', 'sell'), ('nordstern', 'buy'), ('nordstern', 'sell'), ('okx', 'buy'), ('okx', 'sell'), ('papa-change', 'buy'), ('papa-change', 'sell'), ('rapira', 'buy'), ('rapira', 'sell'), ('skylabs', 'buy'), ('skylabs', 'sell'), ('symbiosis', 'buy'), ('symbiosis', 'sell'), ('velora', 'buy'), ('velora', 'sell'), ('whitebird', 'buy'), ('whitebird', 'sell'));

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('0x', 'buy', 'https://matcha.xyz/', '0x Buy', ARRAY['ETH', 'USDC', 'USDT']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{"description":"Catalog entry for 0x-powered token swaps through Matcha. Token and network availability depends on the selected route.","steps":["Open Matcha and select the source and destination tokens and networks.","Review the quoted output, network fees, and slippage in Matcha.","Connect your wallet and confirm the transaction only after checking the network and token addresses."],"links":[{"label":"Open Matcha by 0x","url":"https://matcha.xyz/"}]}'::JSONB, '0x/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('0x', 'sell', 'https://matcha.xyz/', '0x Sell', ARRAY['ETH', 'USDC', 'USDT']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{"description":"Catalog entry for 0x-powered token swaps through Matcha. Token and network availability depends on the selected route.","steps":["Open Matcha and select the source and destination tokens and networks.","Review the quoted output, network fees, and slippage in Matcha.","Connect your wallet and confirm the transaction only after checking the network and token addresses."],"links":[{"label":"Open Matcha by 0x","url":"https://matcha.xyz/"}]}'::JSONB, '0x/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('1inch', 'buy', 'https://1inch.com/swap/', '1inch Buy', ARRAY['ETH', 'USDC', 'USDT']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{"description":"Catalog entry for token swaps through 1inch. Token and network availability depends on the selected route.","steps":["Open 1inch and select the source and destination tokens and networks.","Review the quoted output, network fees, and slippage in 1inch.","Connect your wallet and confirm the transaction only after checking the network and token addresses."],"links":[{"label":"Open 1inch Swap","url":"https://1inch.com/swap/"}]}'::JSONB, '1inch/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('1inch', 'sell', 'https://1inch.com/swap/', '1inch Sell', ARRAY['ETH', 'USDC', 'USDT']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{"description":"Catalog entry for token swaps through 1inch. Token and network availability depends on the selected route.","steps":["Open 1inch and select the source and destination tokens and networks.","Review the quoted output, network fees, and slippage in 1inch.","Connect your wallet and confirm the transaction only after checking the network and token addresses."],"links":[{"label":"Open 1inch Swap","url":"https://1inch.com/swap/"}]}'::JSONB, '1inch/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('bebop', 'buy', 'https://bebop.xyz/', 'Bebop Buy', ARRAY['ETH', 'USDC', 'USDT']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{"description":"Catalog entry for token swaps through Bebop. Token and network availability depends on the selected route.","steps":["Open Bebop and select the source and destination tokens and networks.","Review the quoted output, fees, and execution details shown for the route.","Connect your wallet and confirm the transaction only after checking the network and token addresses."],"links":[{"label":"Open Bebop","url":"https://bebop.xyz/"}]}'::JSONB, 'bebop/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('bebop', 'sell', 'https://bebop.xyz/', 'Bebop Sell', ARRAY['ETH', 'USDC', 'USDT']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{"description":"Catalog entry for token swaps through Bebop. Token and network availability depends on the selected route.","steps":["Open Bebop and select the source and destination tokens and networks.","Review the quoted output, fees, and execution details shown for the route.","Connect your wallet and confirm the transaction only after checking the network and token addresses."],"links":[{"label":"Open Bebop","url":"https://bebop.xyz/"}]}'::JSONB, 'bebop/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('bestchange', 'buy', 'https://www.bestchange.com/?p=1345467', 'BestChange Buy', ARRAY['BYN', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{"p2p":null,"market":null,"bestchange":{"endpoint":"https://bestchange.app","api_key_env":"BESTCHANGE_API_KEY","public_endpoint":"https://www.bestchange.com","affiliate_id":"1345467","language":"ru","timeout_ms":10000,"max_results":100}}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, 'bestchange/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('bestchange', 'sell', 'https://www.bestchange.com/?p=1345467', 'BestChange Sell', ARRAY['BYN', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{"p2p":null,"market":null,"bestchange":{"endpoint":"https://bestchange.app","api_key_env":"BESTCHANGE_API_KEY","public_endpoint":"https://www.bestchange.com","affiliate_id":"1345467","language":"ru","timeout_ms":10000,"max_results":100}}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, 'bestchange/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('binance', 'buy', 'https://www.binance.com', 'Binance Buy', ARRAY['AMD', 'BYN', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], ARRAY['p2p', 'exchanger']::TEXT[], '{"p2p":{"kind":"http_json","endpoint":"https://p2p.binance.com/bapi/c2c/v2/friendly/c2c/adv/search","method":"POST","headers":{"Origin":"https://www.binance.com","Referer":"https://www.binance.com/"},"asset_codes":{},"supported_assets":["USDT","USDC","BTC","ETH","BNB"],"supported_fiats":["USD","RUB","EUR","AMD","BYN"],"payment_method_aliases":{"Alfa-Bank Belarus":["Alfa Bank Belarus","A-Bank"],"Bank Dabrabyt":["Dabrabyt"],"Bank Reshenie":["Bank Reshenie","Reshenie Bank"],"Belagroprombank":["Agrobank","Belagroprom Bank"],"Paritetbank":["Paritet Bank"],"Priorbank":["PriorBank"],"Sber Bank Belarus":["BPS-Sberbank","Sber Bank"],"Sberbank":["Sber"],"T-Bank":["Tinkoff","Tinkoff Bank"],"VTB Belarus":["VTB Bank Belarus","Bank VTB Belarus"]},"timeout_ms":4000,"max_results":20,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":null,"default_max_fiat":null,"default_available_asset":null,"auth":null,"buy":{"query":{},"request_json":"{\"fiat\":\"{{fiat}}\",\"page\":1,\"rows\":\"{{limit_number}}\",\"tradeType\":\"BUY\",\"asset\":\"{{asset}}\",\"countries\":[],\"proMerchantAds\":false,\"shieldMerchantAds\":false,\"publisherType\":null,\"payTypes\":[],\"additionalKycVerifyFilter\":0}","amount_mode":"query_or_empty","items_pointer":"/data","success_pointer":"/code","success_value":"000000","success_missing_allowed":false,"error_pointer":"/message","offer":null},"sell":{"query":{},"request_json":"{\"fiat\":\"{{fiat}}\",\"page\":1,\"rows\":\"{{limit_number}}\",\"tradeType\":\"SELL\",\"asset\":\"{{asset}}\",\"countries\":[],\"proMerchantAds\":false,\"shieldMerchantAds\":false,\"publisherType\":null,\"payTypes\":[],\"additionalKycVerifyFilter\":0}","amount_mode":"query_or_empty","items_pointer":"/data","success_pointer":"/code","success_value":"000000","success_missing_allowed":false,"error_pointer":"/message","offer":null},"offer":{"ad_id_pointer":"/adv/advNo","fiat_pointer":"/adv/fiatUnit","asset_pointer":"/adv/asset","price_pointer":"/adv/price","fiat_amount_pointer":null,"asset_amount_pointer":null,"price_inverted":false,"available_asset_pointer":"/adv/tradableQuantity","min_fiat_pointer":"/adv/minSingleTransAmount","max_fiat_pointer":"/adv/maxSingleTransAmount","payment_methods_pointer":"/adv/tradeMethods","payment_method_value_pointer":"/tradeMethodName","payment_method_fallback_pointer":"/identifier","pay_time_limit_pointer":"/adv/payTimeLimit","advertiser_id_pointer":"/advertiser/userNo","advertiser_nickname_pointer":"/advertiser/nickName","advertiser_user_type_pointer":"/advertiser/userType","merchant_conditions":[{"pointer":"/advertiser/merchantGroupMember","operator":"truthy","value":null},{"pointer":"/advertiser/userType","operator":"equals_ci","value":"merchant"}],"verified_conditions":[],"merchant_default":false,"verified_default":false,"verified_from_merchant":true,"completed_orders_pointer":"/advertiser/monthOrderCount","completion_rate_pointer":"/advertiser/monthFinishRate","positive_rate_pointer":"/advertiser/positiveRate","source_url_template":"https://c2c.binance.com/en/adv?code={{item:/adv/advNo}}","source_url_is_exact":true,"advertiser_profile_url_template":"https://c2c.binance.com/en/advertiserDetail?advertiserNo={{item:/advertiser/userNo}}"},"rate_table":null},"market":{"kind":"http_json","endpoint":"https://api.binance.com/api/v3/ticker/bookTicker","method":"GET","headers":{},"query":{},"request_json":null,"timeout_ms":4000,"items_pointer":null,"symbol_pointer":"/symbol","bid_pointer":"/bidPrice","ask_pointer":"/askPrice","symbol_remove":null,"success_pointer":null,"success_value":null,"success_missing_allowed":false,"error_pointer":null},"bestchange":null}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, 'binance/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('binance', 'sell', 'https://www.binance.com', 'Binance Sell', ARRAY['AMD', 'BYN', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], ARRAY['p2p', 'exchanger']::TEXT[], '{"p2p":{"kind":"http_json","endpoint":"https://p2p.binance.com/bapi/c2c/v2/friendly/c2c/adv/search","method":"POST","headers":{"Origin":"https://www.binance.com","Referer":"https://www.binance.com/"},"asset_codes":{},"supported_assets":["USDT","USDC","BTC","ETH","BNB"],"supported_fiats":["USD","RUB","EUR","AMD","BYN"],"payment_method_aliases":{"Alfa-Bank Belarus":["Alfa Bank Belarus","A-Bank"],"Bank Dabrabyt":["Dabrabyt"],"Bank Reshenie":["Bank Reshenie","Reshenie Bank"],"Belagroprombank":["Agrobank","Belagroprom Bank"],"Paritetbank":["Paritet Bank"],"Priorbank":["PriorBank"],"Sber Bank Belarus":["BPS-Sberbank","Sber Bank"],"Sberbank":["Sber"],"T-Bank":["Tinkoff","Tinkoff Bank"],"VTB Belarus":["VTB Bank Belarus","Bank VTB Belarus"]},"timeout_ms":4000,"max_results":20,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":null,"default_max_fiat":null,"default_available_asset":null,"auth":null,"buy":{"query":{},"request_json":"{\"fiat\":\"{{fiat}}\",\"page\":1,\"rows\":\"{{limit_number}}\",\"tradeType\":\"BUY\",\"asset\":\"{{asset}}\",\"countries\":[],\"proMerchantAds\":false,\"shieldMerchantAds\":false,\"publisherType\":null,\"payTypes\":[],\"additionalKycVerifyFilter\":0}","amount_mode":"query_or_empty","items_pointer":"/data","success_pointer":"/code","success_value":"000000","success_missing_allowed":false,"error_pointer":"/message","offer":null},"sell":{"query":{},"request_json":"{\"fiat\":\"{{fiat}}\",\"page\":1,\"rows\":\"{{limit_number}}\",\"tradeType\":\"SELL\",\"asset\":\"{{asset}}\",\"countries\":[],\"proMerchantAds\":false,\"shieldMerchantAds\":false,\"publisherType\":null,\"payTypes\":[],\"additionalKycVerifyFilter\":0}","amount_mode":"query_or_empty","items_pointer":"/data","success_pointer":"/code","success_value":"000000","success_missing_allowed":false,"error_pointer":"/message","offer":null},"offer":{"ad_id_pointer":"/adv/advNo","fiat_pointer":"/adv/fiatUnit","asset_pointer":"/adv/asset","price_pointer":"/adv/price","fiat_amount_pointer":null,"asset_amount_pointer":null,"price_inverted":false,"available_asset_pointer":"/adv/tradableQuantity","min_fiat_pointer":"/adv/minSingleTransAmount","max_fiat_pointer":"/adv/maxSingleTransAmount","payment_methods_pointer":"/adv/tradeMethods","payment_method_value_pointer":"/tradeMethodName","payment_method_fallback_pointer":"/identifier","pay_time_limit_pointer":"/adv/payTimeLimit","advertiser_id_pointer":"/advertiser/userNo","advertiser_nickname_pointer":"/advertiser/nickName","advertiser_user_type_pointer":"/advertiser/userType","merchant_conditions":[{"pointer":"/advertiser/merchantGroupMember","operator":"truthy","value":null},{"pointer":"/advertiser/userType","operator":"equals_ci","value":"merchant"}],"verified_conditions":[],"merchant_default":false,"verified_default":false,"verified_from_merchant":true,"completed_orders_pointer":"/advertiser/monthOrderCount","completion_rate_pointer":"/advertiser/monthFinishRate","positive_rate_pointer":"/advertiser/positiveRate","source_url_template":"https://c2c.binance.com/en/adv?code={{item:/adv/advNo}}","source_url_is_exact":true,"advertiser_profile_url_template":"https://c2c.binance.com/en/advertiserDetail?advertiserNo={{item:/advertiser/userNo}}"},"rate_table":null},"market":{"kind":"http_json","endpoint":"https://api.binance.com/api/v3/ticker/bookTicker","method":"GET","headers":{},"query":{},"request_json":null,"timeout_ms":4000,"items_pointer":null,"symbol_pointer":"/symbol","bid_pointer":"/bidPrice","ask_pointer":"/askPrice","symbol_remove":null,"success_pointer":null,"success_value":null,"success_missing_allowed":false,"error_pointer":null},"bestchange":null}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, 'binance/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('bitcoin-center', 'buy', 'https://www.bitcoincenter.am/en/?from=WIREAMD&to=USDTSOL', 'Bitcoin Center Buy', ARRAY['AMD']::TEXT[], ARRAY['Bank Transfer']::TEXT[], ARRAY['exchanger']::TEXT[], '{"p2p":{"kind":"http_json","market":"direct_exchange","endpoint":"https://www.bitcoincenter.am/service/api/v1/public/exchanger/route/get/one/","method":"GET","headers":{"Accept":"application/json","Lang":"en","Origin":"https://www.bitcoincenter.am","Referer":"https://www.bitcoincenter.am/en/"},"asset_codes":{"USDT":"USDTSOL"},"supported_assets":["USDT"],"supported_fiats":["AMD"],"timeout_ms":5000,"max_results":null,"fiat_probe_amount":100000.0,"asset_probe_amount":100.0,"default_min_fiat":50000.0,"default_max_fiat":10000000.0,"default_available_asset":1000000000.0,"auth":null,"buy":{"query":{"id":"6a75d602f50f4685ab92bfd0","lang":"en"},"request_json":null,"amount_mode":"fiat_probe","items_pointer":null,"success_pointer":"/success","success_value":"true","success_missing_allowed":false,"error_pointer":"/error/message","offer":{"ad_id_pointer":"/data/route/routeId","fiat_pointer":"/data/route/from/symbol","asset_pointer":"/data/route/to/xml","network":"solana","price_pointer":"/data/route/rate/in","fiat_amount_pointer":null,"asset_amount_pointer":null,"output_fee_pointer":"/data/route/rate/outFeeAmount","price_inverted":false,"available_asset_pointer":"/data/route/rate/amount","min_fiat_pointer":"/data/route/from/min","max_fiat_pointer":"/data/route/from/max","payment_methods_pointer":"/data/route/from/name","payment_method_value_pointer":null,"payment_method_fallback_pointer":null,"pay_time_limit_pointer":"/data/route/orderTTL","advertiser_id_pointer":null,"advertiser_nickname_pointer":null,"advertiser_user_type_pointer":null,"merchant_conditions":[],"verified_conditions":[],"merchant_default":true,"verified_default":true,"verified_from_merchant":false,"completed_orders_pointer":null,"completion_rate_pointer":null,"positive_rate_pointer":null,"source_url_template":"https://www.bitcoincenter.am/en/?from=WIREAMD&to=USDTSOL","source_url_is_exact":true,"advertiser_profile_url_template":null}},"sell":{"query":{"id":"6a75d54ef50f4685ab92bdd3","lang":"en"},"request_json":null,"amount_mode":"asset_probe","items_pointer":null,"success_pointer":"/success","success_value":"true","success_missing_allowed":false,"error_pointer":"/error/message","offer":{"ad_id_pointer":"/data/route/routeId","fiat_pointer":"/data/route/to/symbol","asset_pointer":"/data/route/from/xml","network":"solana","price_pointer":"/data/route/rate/out","fiat_amount_pointer":null,"asset_amount_pointer":null,"output_fee_pointer":"/data/route/rate/outFeeAmount","price_inverted":false,"available_asset_pointer":null,"min_fiat_pointer":null,"max_fiat_pointer":null,"payment_methods_pointer":"/data/route/to/name","payment_method_value_pointer":null,"payment_method_fallback_pointer":null,"pay_time_limit_pointer":"/data/route/orderTTL","advertiser_id_pointer":null,"advertiser_nickname_pointer":null,"advertiser_user_type_pointer":null,"merchant_conditions":[],"verified_conditions":[],"merchant_default":true,"verified_default":true,"verified_from_merchant":false,"completed_orders_pointer":null,"completion_rate_pointer":null,"positive_rate_pointer":null,"source_url_template":"https://www.bitcoincenter.am/en/?from=USDTSOL&to=WIREAMD","source_url_is_exact":true,"advertiser_profile_url_template":null}},"offer":null,"rate_table":null},"market":null,"bestchange":null}'::JSONB, '{}'::JSONB, '{"kind":"quote_dependent","description":"The effective Pay3Flow rate includes Bitcoin Center''s fixed output fee reported by the live route API; bank and blockchain charges may still apply.","docs_url":"https://www.bitcoincenter.am/en/?from=WIREAMD&to=USDTSOL"}'::JSONB, '{}'::JSONB, 'bitcoin-center/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('bitcoin-center', 'sell', 'https://www.bitcoincenter.am/en/?from=USDTSOL&to=WIREAMD', 'Bitcoin Center Sell', ARRAY['AMD']::TEXT[], ARRAY['Bank Transfer']::TEXT[], ARRAY['exchanger']::TEXT[], '{"p2p":{"kind":"http_json","market":"direct_exchange","endpoint":"https://www.bitcoincenter.am/service/api/v1/public/exchanger/route/get/one/","method":"GET","headers":{"Accept":"application/json","Lang":"en","Origin":"https://www.bitcoincenter.am","Referer":"https://www.bitcoincenter.am/en/"},"asset_codes":{"USDT":"USDTSOL"},"supported_assets":["USDT"],"supported_fiats":["AMD"],"timeout_ms":5000,"max_results":null,"fiat_probe_amount":100000.0,"asset_probe_amount":100.0,"default_min_fiat":50000.0,"default_max_fiat":10000000.0,"default_available_asset":1000000000.0,"auth":null,"buy":{"query":{"id":"6a75d602f50f4685ab92bfd0","lang":"en"},"request_json":null,"amount_mode":"fiat_probe","items_pointer":null,"success_pointer":"/success","success_value":"true","success_missing_allowed":false,"error_pointer":"/error/message","offer":{"ad_id_pointer":"/data/route/routeId","fiat_pointer":"/data/route/from/symbol","asset_pointer":"/data/route/to/xml","network":"solana","price_pointer":"/data/route/rate/in","fiat_amount_pointer":null,"asset_amount_pointer":null,"output_fee_pointer":"/data/route/rate/outFeeAmount","price_inverted":false,"available_asset_pointer":"/data/route/rate/amount","min_fiat_pointer":"/data/route/from/min","max_fiat_pointer":"/data/route/from/max","payment_methods_pointer":"/data/route/from/name","payment_method_value_pointer":null,"payment_method_fallback_pointer":null,"pay_time_limit_pointer":"/data/route/orderTTL","advertiser_id_pointer":null,"advertiser_nickname_pointer":null,"advertiser_user_type_pointer":null,"merchant_conditions":[],"verified_conditions":[],"merchant_default":true,"verified_default":true,"verified_from_merchant":false,"completed_orders_pointer":null,"completion_rate_pointer":null,"positive_rate_pointer":null,"source_url_template":"https://www.bitcoincenter.am/en/?from=WIREAMD&to=USDTSOL","source_url_is_exact":true,"advertiser_profile_url_template":null}},"sell":{"query":{"id":"6a75d54ef50f4685ab92bdd3","lang":"en"},"request_json":null,"amount_mode":"asset_probe","items_pointer":null,"success_pointer":"/success","success_value":"true","success_missing_allowed":false,"error_pointer":"/error/message","offer":{"ad_id_pointer":"/data/route/routeId","fiat_pointer":"/data/route/to/symbol","asset_pointer":"/data/route/from/xml","network":"solana","price_pointer":"/data/route/rate/out","fiat_amount_pointer":null,"asset_amount_pointer":null,"output_fee_pointer":"/data/route/rate/outFeeAmount","price_inverted":false,"available_asset_pointer":null,"min_fiat_pointer":null,"max_fiat_pointer":null,"payment_methods_pointer":"/data/route/to/name","payment_method_value_pointer":null,"payment_method_fallback_pointer":null,"pay_time_limit_pointer":"/data/route/orderTTL","advertiser_id_pointer":null,"advertiser_nickname_pointer":null,"advertiser_user_type_pointer":null,"merchant_conditions":[],"verified_conditions":[],"merchant_default":true,"verified_default":true,"verified_from_merchant":false,"completed_orders_pointer":null,"completion_rate_pointer":null,"positive_rate_pointer":null,"source_url_template":"https://www.bitcoincenter.am/en/?from=USDTSOL&to=WIREAMD","source_url_is_exact":true,"advertiser_profile_url_template":null}},"offer":null,"rate_table":null},"market":null,"bestchange":null}'::JSONB, '{}'::JSONB, '{"kind":"quote_dependent","description":"The effective Pay3Flow rate includes Bitcoin Center''s fixed output fee reported by the live route API; bank and blockchain charges may still apply.","docs_url":"https://www.bitcoincenter.am/en/?from=WIREAMD&to=USDTSOL"}'::JSONB, '{}'::JSONB, 'bitcoin-center/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('bitget', 'buy', 'https://www.bitget.com', 'Bitget Buy', ARRAY['AMD', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], ARRAY['p2p', 'exchanger']::TEXT[], '{"p2p":{"kind":"http_json","endpoint":"https://www.bitget.com/v1/p2p/pub/adv/queryAdvList","method":"POST","headers":{"Origin":"https://www.bitget.com","Referer":"https://www.bitget.com/p2p-trade"},"asset_codes":{},"supported_assets":["USDT","USDC","BTC","ETH"],"timeout_ms":4000,"max_results":null,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":null,"default_max_fiat":null,"default_available_asset":null,"auth":null,"buy":{"query":{},"request_json":"{\"side\":1,\"pageNo\":1,\"pageSize\":\"{{limit_number}}\",\"coinCode\":\"{{asset}}\",\"fiatCode\":\"{{fiat}}\"}","amount_mode":"query_or_empty","items_pointer":"/data/dataList","success_pointer":"/code","success_value":"00000","success_missing_allowed":false,"error_pointer":"/msg","offer":null},"sell":{"query":{},"request_json":"{\"side\":2,\"pageNo\":1,\"pageSize\":\"{{limit_number}}\",\"coinCode\":\"{{asset}}\",\"fiatCode\":\"{{fiat}}\"}","amount_mode":"query_or_empty","items_pointer":"/data/dataList","success_pointer":"/code","success_value":"00000","success_missing_allowed":false,"error_pointer":"/msg","offer":null},"offer":{"ad_id_pointer":"/adNo","fiat_pointer":"/fiatCode","asset_pointer":"/coinCode","price_pointer":"/price","fiat_amount_pointer":null,"asset_amount_pointer":null,"price_inverted":false,"available_asset_pointer":"/editAmount","min_fiat_pointer":"/minAmount","max_fiat_pointer":"/maxAmount","payment_methods_pointer":"/paymethodInfo","payment_method_value_pointer":"/paymethodName","payment_method_fallback_pointer":null,"pay_time_limit_pointer":"/payDuration","advertiser_id_pointer":"/encryptUserId","advertiser_nickname_pointer":"/nickName","advertiser_user_type_pointer":"/certifiedMerchant","merchant_conditions":[{"pointer":"/certifiedMerchant","operator":"truthy","value":null}],"verified_conditions":[],"merchant_default":false,"verified_default":false,"verified_from_merchant":true,"completed_orders_pointer":"/thirtyTunoverNum","completion_rate_pointer":"/thirtyCompletionRate","positive_rate_pointer":"/goodEvaluationRate","source_url_template":"https://www.bitget.com/p2p-trade/{{side}}?fiatName={{fiat}}","source_url_is_exact":false,"advertiser_profile_url_template":"https://www.bitget.com/p2p-trade/user/{{item:/encryptUserId}}"},"rate_table":null},"market":{"kind":"http_json","endpoint":"https://api.bitget.com/api/v2/spot/market/tickers","method":"GET","headers":{},"query":{},"request_json":null,"timeout_ms":4000,"items_pointer":"/data","symbol_pointer":"/symbol","bid_pointer":"/bidPr","ask_pointer":"/askPr","symbol_remove":null,"success_pointer":"/code","success_value":"00000","success_missing_allowed":false,"error_pointer":"/msg"},"bestchange":null}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, 'bitget/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('bitget', 'sell', 'https://www.bitget.com', 'Bitget Sell', ARRAY['AMD', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], ARRAY['p2p', 'exchanger']::TEXT[], '{"p2p":{"kind":"http_json","endpoint":"https://www.bitget.com/v1/p2p/pub/adv/queryAdvList","method":"POST","headers":{"Origin":"https://www.bitget.com","Referer":"https://www.bitget.com/p2p-trade"},"asset_codes":{},"supported_assets":["USDT","USDC","BTC","ETH"],"timeout_ms":4000,"max_results":null,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":null,"default_max_fiat":null,"default_available_asset":null,"auth":null,"buy":{"query":{},"request_json":"{\"side\":1,\"pageNo\":1,\"pageSize\":\"{{limit_number}}\",\"coinCode\":\"{{asset}}\",\"fiatCode\":\"{{fiat}}\"}","amount_mode":"query_or_empty","items_pointer":"/data/dataList","success_pointer":"/code","success_value":"00000","success_missing_allowed":false,"error_pointer":"/msg","offer":null},"sell":{"query":{},"request_json":"{\"side\":2,\"pageNo\":1,\"pageSize\":\"{{limit_number}}\",\"coinCode\":\"{{asset}}\",\"fiatCode\":\"{{fiat}}\"}","amount_mode":"query_or_empty","items_pointer":"/data/dataList","success_pointer":"/code","success_value":"00000","success_missing_allowed":false,"error_pointer":"/msg","offer":null},"offer":{"ad_id_pointer":"/adNo","fiat_pointer":"/fiatCode","asset_pointer":"/coinCode","price_pointer":"/price","fiat_amount_pointer":null,"asset_amount_pointer":null,"price_inverted":false,"available_asset_pointer":"/editAmount","min_fiat_pointer":"/minAmount","max_fiat_pointer":"/maxAmount","payment_methods_pointer":"/paymethodInfo","payment_method_value_pointer":"/paymethodName","payment_method_fallback_pointer":null,"pay_time_limit_pointer":"/payDuration","advertiser_id_pointer":"/encryptUserId","advertiser_nickname_pointer":"/nickName","advertiser_user_type_pointer":"/certifiedMerchant","merchant_conditions":[{"pointer":"/certifiedMerchant","operator":"truthy","value":null}],"verified_conditions":[],"merchant_default":false,"verified_default":false,"verified_from_merchant":true,"completed_orders_pointer":"/thirtyTunoverNum","completion_rate_pointer":"/thirtyCompletionRate","positive_rate_pointer":"/goodEvaluationRate","source_url_template":"https://www.bitget.com/p2p-trade/{{side}}?fiatName={{fiat}}","source_url_is_exact":false,"advertiser_profile_url_template":"https://www.bitget.com/p2p-trade/user/{{item:/encryptUserId}}"},"rate_table":null},"market":{"kind":"http_json","endpoint":"https://api.bitget.com/api/v2/spot/market/tickers","method":"GET","headers":{},"query":{},"request_json":null,"timeout_ms":4000,"items_pointer":"/data","symbol_pointer":"/symbol","bid_pointer":"/bidPr","ask_pointer":"/askPr","symbol_remove":null,"success_pointer":"/code","success_value":"00000","success_missing_allowed":false,"error_pointer":"/msg"},"bestchange":null}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, 'bitget/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('bncex', 'buy', 'https://www.bncex.com/en', 'bncex Buy', ARRAY['AMD']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{"p2p":{"kind":"http_json","market":"direct_exchange","endpoint":"https://www.bncex.com/api/exchange/quote","method":"POST","headers":{"Accept":"application/json","Origin":"https://www.bncex.com","Referer":"https://www.bncex.com/en"},"asset_codes":{},"supported_assets":["USDT","USDC"],"supported_fiats":["AMD"],"timeout_ms":5000,"max_results":null,"fiat_probe_amount":100000.0,"asset_probe_amount":100.0,"default_min_fiat":10000.0,"default_max_fiat":50000000.0,"default_available_asset":1000000000.0,"auth":null,"buy":{"query":{},"request_json":"{\"type\":\"BUY_{{asset}}\",\"network\":\"TRC20\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountAmd\":\"{{amount_number}}\"}","request_json_variants":["{\"type\":\"BUY_{{asset}}\",\"network\":\"ETH\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountAmd\":\"{{amount_number}}\"}","{\"type\":\"BUY_{{asset}}\",\"network\":\"BSC\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountAmd\":\"{{amount_number}}\"}","{\"type\":\"BUY_{{asset}}\",\"network\":\"POLYGON\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountAmd\":\"{{amount_number}}\"}","{\"type\":\"BUY_{{asset}}\",\"network\":\"ARBITRUM\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountAmd\":\"{{amount_number}}\"}","{\"type\":\"BUY_{{asset}}\",\"network\":\"OPTIMISM\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountAmd\":\"{{amount_number}}\"}","{\"type\":\"BUY_{{asset}}\",\"network\":\"BASE\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountAmd\":\"{{amount_number}}\"}","{\"type\":\"BUY_{{asset}}\",\"network\":\"AVALANCHE\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountAmd\":\"{{amount_number}}\"}","{\"type\":\"BUY_{{asset}}\",\"network\":\"SOLANA\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountAmd\":\"{{amount_number}}\"}"],"amount_mode":"fiat_probe","items_pointer":null,"success_pointer":null,"success_value":null,"success_missing_allowed":false,"error_pointer":null,"offer":{"ad_id_pointer":null,"fiat_pointer":null,"asset_pointer":"/token","network_pointer":"/network","price_pointer":null,"fiat_amount_pointer":"/amountAmd","asset_amount_pointer":"/amountUsdt","price_inverted":false,"available_asset_pointer":null,"min_fiat_pointer":null,"max_fiat_pointer":null,"payment_methods_pointer":null,"payment_method_value_pointer":null,"payment_method_fallback_pointer":null,"pay_time_limit_pointer":null,"advertiser_id_pointer":null,"advertiser_nickname_pointer":null,"advertiser_user_type_pointer":null,"merchant_conditions":[],"verified_conditions":[],"merchant_default":true,"verified_default":true,"verified_from_merchant":false,"completed_orders_pointer":null,"completion_rate_pointer":null,"positive_rate_pointer":null,"source_url_template":"https://www.bncex.com/en","source_url_is_exact":false,"advertiser_profile_url_template":null}},"sell":{"query":{},"request_json":"{\"type\":\"SELL_{{asset}}\",\"network\":\"TRC20\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountUsdt\":\"{{amount_number}}\"}","request_json_variants":["{\"type\":\"SELL_{{asset}}\",\"network\":\"ETH\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountUsdt\":\"{{amount_number}}\"}","{\"type\":\"SELL_{{asset}}\",\"network\":\"BSC\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountUsdt\":\"{{amount_number}}\"}","{\"type\":\"SELL_{{asset}}\",\"network\":\"POLYGON\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountUsdt\":\"{{amount_number}}\"}","{\"type\":\"SELL_{{asset}}\",\"network\":\"ARBITRUM\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountUsdt\":\"{{amount_number}}\"}","{\"type\":\"SELL_{{asset}}\",\"network\":\"OPTIMISM\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountUsdt\":\"{{amount_number}}\"}","{\"type\":\"SELL_{{asset}}\",\"network\":\"BASE\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountUsdt\":\"{{amount_number}}\"}","{\"type\":\"SELL_{{asset}}\",\"network\":\"AVALANCHE\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountUsdt\":\"{{amount_number}}\"}","{\"type\":\"SELL_{{asset}}\",\"network\":\"SOLANA\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountUsdt\":\"{{amount_number}}\"}"],"amount_mode":"asset_probe","items_pointer":null,"success_pointer":null,"success_value":null,"success_missing_allowed":false,"error_pointer":null,"offer":{"ad_id_pointer":null,"fiat_pointer":null,"asset_pointer":"/token","network_pointer":"/network","price_pointer":null,"fiat_amount_pointer":"/amountAmd","asset_amount_pointer":"/amountUsdt","price_inverted":false,"available_asset_pointer":null,"min_fiat_pointer":null,"max_fiat_pointer":null,"payment_methods_pointer":null,"payment_method_value_pointer":null,"payment_method_fallback_pointer":null,"pay_time_limit_pointer":null,"advertiser_id_pointer":null,"advertiser_nickname_pointer":null,"advertiser_user_type_pointer":null,"merchant_conditions":[],"verified_conditions":[],"merchant_default":true,"verified_default":true,"verified_from_merchant":false,"completed_orders_pointer":null,"completion_rate_pointer":null,"positive_rate_pointer":null,"source_url_template":"https://www.bncex.com/en","source_url_is_exact":false,"advertiser_profile_url_template":null}},"offer":null,"rate_table":null},"market":null,"bestchange":null}'::JSONB, '{}'::JSONB, '{"kind":"quote_dependent","description":"The live calculator quote includes bncex''s amount-tiered exchange percentage and selected network fee. Bank or card charges may be separate.","docs_url":"https://www.bncex.com/en"}'::JSONB, '{}'::JSONB, 'bncex/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('bncex', 'sell', 'https://www.bncex.com/en', 'bncex Sell', ARRAY['AMD']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{"p2p":{"kind":"http_json","market":"direct_exchange","endpoint":"https://www.bncex.com/api/exchange/quote","method":"POST","headers":{"Accept":"application/json","Origin":"https://www.bncex.com","Referer":"https://www.bncex.com/en"},"asset_codes":{},"supported_assets":["USDT","USDC"],"supported_fiats":["AMD"],"timeout_ms":5000,"max_results":null,"fiat_probe_amount":100000.0,"asset_probe_amount":100.0,"default_min_fiat":10000.0,"default_max_fiat":50000000.0,"default_available_asset":1000000000.0,"auth":null,"buy":{"query":{},"request_json":"{\"type\":\"BUY_{{asset}}\",\"network\":\"TRC20\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountAmd\":\"{{amount_number}}\"}","request_json_variants":["{\"type\":\"BUY_{{asset}}\",\"network\":\"ETH\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountAmd\":\"{{amount_number}}\"}","{\"type\":\"BUY_{{asset}}\",\"network\":\"BSC\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountAmd\":\"{{amount_number}}\"}","{\"type\":\"BUY_{{asset}}\",\"network\":\"POLYGON\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountAmd\":\"{{amount_number}}\"}","{\"type\":\"BUY_{{asset}}\",\"network\":\"ARBITRUM\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountAmd\":\"{{amount_number}}\"}","{\"type\":\"BUY_{{asset}}\",\"network\":\"OPTIMISM\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountAmd\":\"{{amount_number}}\"}","{\"type\":\"BUY_{{asset}}\",\"network\":\"BASE\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountAmd\":\"{{amount_number}}\"}","{\"type\":\"BUY_{{asset}}\",\"network\":\"AVALANCHE\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountAmd\":\"{{amount_number}}\"}","{\"type\":\"BUY_{{asset}}\",\"network\":\"SOLANA\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountAmd\":\"{{amount_number}}\"}"],"amount_mode":"fiat_probe","items_pointer":null,"success_pointer":null,"success_value":null,"success_missing_allowed":false,"error_pointer":null,"offer":{"ad_id_pointer":null,"fiat_pointer":null,"asset_pointer":"/token","network_pointer":"/network","price_pointer":null,"fiat_amount_pointer":"/amountAmd","asset_amount_pointer":"/amountUsdt","price_inverted":false,"available_asset_pointer":null,"min_fiat_pointer":null,"max_fiat_pointer":null,"payment_methods_pointer":null,"payment_method_value_pointer":null,"payment_method_fallback_pointer":null,"pay_time_limit_pointer":null,"advertiser_id_pointer":null,"advertiser_nickname_pointer":null,"advertiser_user_type_pointer":null,"merchant_conditions":[],"verified_conditions":[],"merchant_default":true,"verified_default":true,"verified_from_merchant":false,"completed_orders_pointer":null,"completion_rate_pointer":null,"positive_rate_pointer":null,"source_url_template":"https://www.bncex.com/en","source_url_is_exact":false,"advertiser_profile_url_template":null}},"sell":{"query":{},"request_json":"{\"type\":\"SELL_{{asset}}\",\"network\":\"TRC20\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountUsdt\":\"{{amount_number}}\"}","request_json_variants":["{\"type\":\"SELL_{{asset}}\",\"network\":\"ETH\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountUsdt\":\"{{amount_number}}\"}","{\"type\":\"SELL_{{asset}}\",\"network\":\"BSC\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountUsdt\":\"{{amount_number}}\"}","{\"type\":\"SELL_{{asset}}\",\"network\":\"POLYGON\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountUsdt\":\"{{amount_number}}\"}","{\"type\":\"SELL_{{asset}}\",\"network\":\"ARBITRUM\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountUsdt\":\"{{amount_number}}\"}","{\"type\":\"SELL_{{asset}}\",\"network\":\"OPTIMISM\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountUsdt\":\"{{amount_number}}\"}","{\"type\":\"SELL_{{asset}}\",\"network\":\"BASE\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountUsdt\":\"{{amount_number}}\"}","{\"type\":\"SELL_{{asset}}\",\"network\":\"AVALANCHE\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountUsdt\":\"{{amount_number}}\"}","{\"type\":\"SELL_{{asset}}\",\"network\":\"SOLANA\",\"paymentMethod\":\"NON_CASH\",\"token\":\"{{asset}}\",\"amountUsdt\":\"{{amount_number}}\"}"],"amount_mode":"asset_probe","items_pointer":null,"success_pointer":null,"success_value":null,"success_missing_allowed":false,"error_pointer":null,"offer":{"ad_id_pointer":null,"fiat_pointer":null,"asset_pointer":"/token","network_pointer":"/network","price_pointer":null,"fiat_amount_pointer":"/amountAmd","asset_amount_pointer":"/amountUsdt","price_inverted":false,"available_asset_pointer":null,"min_fiat_pointer":null,"max_fiat_pointer":null,"payment_methods_pointer":null,"payment_method_value_pointer":null,"payment_method_fallback_pointer":null,"pay_time_limit_pointer":null,"advertiser_id_pointer":null,"advertiser_nickname_pointer":null,"advertiser_user_type_pointer":null,"merchant_conditions":[],"verified_conditions":[],"merchant_default":true,"verified_default":true,"verified_from_merchant":false,"completed_orders_pointer":null,"completion_rate_pointer":null,"positive_rate_pointer":null,"source_url_template":"https://www.bncex.com/en","source_url_is_exact":false,"advertiser_profile_url_template":null}},"offer":null,"rate_table":null},"market":null,"bestchange":null}'::JSONB, '{}'::JSONB, '{"kind":"quote_dependent","description":"The live calculator quote includes bncex''s amount-tiered exchange percentage and selected network fee. Bank or card charges may be separate.","docs_url":"https://www.bncex.com/en"}'::JSONB, '{}'::JSONB, 'bncex/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('bybit', 'buy', 'https://www.bybit.com', 'Bybit Buy', ARRAY['AMD', 'BYN', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], ARRAY['p2p', 'exchanger']::TEXT[], '{"p2p":{"kind":"http_json","endpoint":"https://api2.bybit.com/fiat/otc/item/online","method":"POST","headers":{"Origin":"https://www.bybit.com","Referer":"https://www.bybit.com/"},"asset_codes":{},"supported_assets":["USDT","USDC","BTC","ETH"],"supported_fiats":["USD","RUB","EUR","AMD","BYN"],"payment_method_aliases":{"Alfa-Bank Belarus":["Alfa Bank Belarus","A-Bank"],"Bank Dabrabyt":["Dabrabyt"],"Bank Reshenie":["Reshenie Bank"],"Belagroprombank":["Agrobank","Belagroprom Bank"],"Paritetbank":["Paritet Bank"],"Priorbank":["PriorBank"],"Sber Bank Belarus":["BPS-Sberbank","Sber Bank"],"Sberbank":["Sber"],"T-Bank":["Tinkoff","Tinkoff Bank"],"VTB Belarus":["VTB Bank Belarus","Bank VTB Belarus"]},"timeout_ms":4000,"max_results":null,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":null,"default_max_fiat":null,"default_available_asset":null,"auth":null,"buy":{"query":{},"request_json":"{\"userId\":\"\",\"tokenId\":\"{{asset}}\",\"currencyId\":\"{{fiat}}\",\"payment\":[],\"side\":\"1\",\"size\":\"{{limit}}\",\"page\":\"1\",\"amount\":\"{{amount}}\",\"authMaker\":false,\"canTrade\":false}","amount_mode":"query_or_empty","items_pointer":"/result/items","success_pointer":"/ret_code","success_value":"0","success_missing_allowed":false,"error_pointer":"/ret_msg","offer":null},"sell":{"query":{},"request_json":"{\"userId\":\"\",\"tokenId\":\"{{asset}}\",\"currencyId\":\"{{fiat}}\",\"payment\":[],\"side\":\"0\",\"size\":\"{{limit}}\",\"page\":\"1\",\"amount\":\"{{amount}}\",\"authMaker\":false,\"canTrade\":false}","amount_mode":"query_or_empty","items_pointer":"/result/items","success_pointer":"/ret_code","success_value":"0","success_missing_allowed":false,"error_pointer":"/ret_msg","offer":null},"offer":{"ad_id_pointer":"/id","fiat_pointer":"/currencyId","asset_pointer":"/tokenId","price_pointer":"/price","fiat_amount_pointer":null,"asset_amount_pointer":null,"price_inverted":false,"available_asset_pointer":"/quantity","min_fiat_pointer":"/minAmount","max_fiat_pointer":"/maxAmount","payment_methods_pointer":"/payments","payment_method_value_pointer":null,"payment_method_fallback_pointer":null,"pay_time_limit_pointer":"/paymentPeriod","advertiser_id_pointer":"/userMaskId","advertiser_nickname_pointer":"/nickName","advertiser_user_type_pointer":"/userType","merchant_conditions":[{"pointer":"/authTag","operator":"non_empty","value":null},{"pointer":"/userType","operator":"not_equals_ci","value":"personal"}],"verified_conditions":[{"pointer":"/authStatus","operator":"equals","value":"1"}],"merchant_default":false,"verified_default":false,"verified_from_merchant":false,"completed_orders_pointer":"/recentOrderNum","completion_rate_pointer":"/recentExecuteRate","positive_rate_pointer":null,"source_url_template":"https://www.bybit.com/fiat/trade/otc/?token={{asset}}&fiat={{fiat}}","source_url_is_exact":false,"advertiser_profile_url_template":"https://www.bybit.com/en/p2p/profile/{{item:/userMaskId}}/{{asset}}/{{fiat}}/item"},"rate_table":null},"market":{"kind":"http_json","endpoint":"https://api.bybit.com/v5/market/tickers","method":"GET","headers":{},"query":{"category":"spot"},"request_json":null,"timeout_ms":4000,"items_pointer":"/result/list","symbol_pointer":"/symbol","bid_pointer":"/bid1Price","ask_pointer":"/ask1Price","symbol_remove":null,"success_pointer":"/retCode","success_value":"0","success_missing_allowed":false,"error_pointer":"/retMsg"},"bestchange":null}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, 'bybit/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('bybit', 'sell', 'https://www.bybit.com', 'Bybit Sell', ARRAY['AMD', 'BYN', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], ARRAY['p2p', 'exchanger']::TEXT[], '{"p2p":{"kind":"http_json","endpoint":"https://api2.bybit.com/fiat/otc/item/online","method":"POST","headers":{"Origin":"https://www.bybit.com","Referer":"https://www.bybit.com/"},"asset_codes":{},"supported_assets":["USDT","USDC","BTC","ETH"],"supported_fiats":["USD","RUB","EUR","AMD","BYN"],"payment_method_aliases":{"Alfa-Bank Belarus":["Alfa Bank Belarus","A-Bank"],"Bank Dabrabyt":["Dabrabyt"],"Bank Reshenie":["Reshenie Bank"],"Belagroprombank":["Agrobank","Belagroprom Bank"],"Paritetbank":["Paritet Bank"],"Priorbank":["PriorBank"],"Sber Bank Belarus":["BPS-Sberbank","Sber Bank"],"Sberbank":["Sber"],"T-Bank":["Tinkoff","Tinkoff Bank"],"VTB Belarus":["VTB Bank Belarus","Bank VTB Belarus"]},"timeout_ms":4000,"max_results":null,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":null,"default_max_fiat":null,"default_available_asset":null,"auth":null,"buy":{"query":{},"request_json":"{\"userId\":\"\",\"tokenId\":\"{{asset}}\",\"currencyId\":\"{{fiat}}\",\"payment\":[],\"side\":\"1\",\"size\":\"{{limit}}\",\"page\":\"1\",\"amount\":\"{{amount}}\",\"authMaker\":false,\"canTrade\":false}","amount_mode":"query_or_empty","items_pointer":"/result/items","success_pointer":"/ret_code","success_value":"0","success_missing_allowed":false,"error_pointer":"/ret_msg","offer":null},"sell":{"query":{},"request_json":"{\"userId\":\"\",\"tokenId\":\"{{asset}}\",\"currencyId\":\"{{fiat}}\",\"payment\":[],\"side\":\"0\",\"size\":\"{{limit}}\",\"page\":\"1\",\"amount\":\"{{amount}}\",\"authMaker\":false,\"canTrade\":false}","amount_mode":"query_or_empty","items_pointer":"/result/items","success_pointer":"/ret_code","success_value":"0","success_missing_allowed":false,"error_pointer":"/ret_msg","offer":null},"offer":{"ad_id_pointer":"/id","fiat_pointer":"/currencyId","asset_pointer":"/tokenId","price_pointer":"/price","fiat_amount_pointer":null,"asset_amount_pointer":null,"price_inverted":false,"available_asset_pointer":"/quantity","min_fiat_pointer":"/minAmount","max_fiat_pointer":"/maxAmount","payment_methods_pointer":"/payments","payment_method_value_pointer":null,"payment_method_fallback_pointer":null,"pay_time_limit_pointer":"/paymentPeriod","advertiser_id_pointer":"/userMaskId","advertiser_nickname_pointer":"/nickName","advertiser_user_type_pointer":"/userType","merchant_conditions":[{"pointer":"/authTag","operator":"non_empty","value":null},{"pointer":"/userType","operator":"not_equals_ci","value":"personal"}],"verified_conditions":[{"pointer":"/authStatus","operator":"equals","value":"1"}],"merchant_default":false,"verified_default":false,"verified_from_merchant":false,"completed_orders_pointer":"/recentOrderNum","completion_rate_pointer":"/recentExecuteRate","positive_rate_pointer":null,"source_url_template":"https://www.bybit.com/fiat/trade/otc/?token={{asset}}&fiat={{fiat}}","source_url_is_exact":false,"advertiser_profile_url_template":"https://www.bybit.com/en/p2p/profile/{{item:/userMaskId}}/{{asset}}/{{fiat}}/item"},"rate_table":null},"market":{"kind":"http_json","endpoint":"https://api.bybit.com/v5/market/tickers","method":"GET","headers":{},"query":{"category":"spot"},"request_json":null,"timeout_ms":4000,"items_pointer":"/result/list","symbol_pointer":"/symbol","bid_pointer":"/bid1Price","ask_pointer":"/ask1Price","symbol_remove":null,"success_pointer":"/retCode","success_value":"0","success_missing_allowed":false,"error_pointer":"/retMsg"},"bestchange":null}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, 'bybit/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('cifra-broker', 'buy', 'https://cifra.by/', 'Cifra Markets Buy', ARRAY['BYN', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{"p2p":{"kind":"http_json","market":"direct_exchange","endpoint":"https://api.cifra-broker.by/api/site/ticker-calculator","method":"GET","headers":{"Accept-Language":"ru-RU,ru;q=0.9","Origin":"https://cifra.by","Referer":"https://cifra.by/","User-Agent":"Mozilla/5.0 (compatible; Pay3Flow/0.1)"},"asset_codes":{},"supported_assets":["USDT","USDC","BTC","BNB","SOL","TRX","DOGE","LTC","DAI","XRP","ADA","DOT","LINK","AVAX","BCH","NEAR","APT","ATOM","UNI","SUI"],"timeout_ms":5000,"max_results":null,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":1.0,"default_max_fiat":1000000000.0,"default_available_asset":1000000000.0,"auth":null,"buy":{"query":{"key":"pay3flow"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":null,"success_pointer":"/success","success_value":"true","success_missing_allowed":false,"error_pointer":"/message","offer":null},"sell":{"query":{"key":"pay3flow"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":null,"success_pointer":"/success","success_value":"true","success_missing_allowed":false,"error_pointer":"/message","offer":null},"offer":null,"rate_table":{"fiat_items_pointer":"/data/currenciesReal","fiat_code_pointer":"/code","fiat_rate_pointer":"/rate/value","asset_items_pointer":"/data/currenciesNotReal","asset_code_pointer":"/code","asset_rates_pointer":"/data/currenciesNotRealRate","fiat_codes":{"RUB":"RUR"},"source_url":"https://tradernet.by/authentication/signup"}},"market":{"kind":"http_json","endpoint":"https://api.cifra-broker.by/api/site/ticker","method":"POST","headers":{"Accept-Language":"ru-RU,ru;q=0.9","Origin":"https://cifra.by","Referer":"https://cifra.by/catalog","User-Agent":"Mozilla/5.0 (compatible; Pay3Flow/0.1)"},"query":{},"request_json":"{\"limit\":1000,\"tags\":[]}","timeout_ms":5000,"items_pointer":"/data/ticker","symbol_pointer":"/name","bid_pointer":"/ltp","ask_pointer":"/ltp","symbol_remove":"-","success_pointer":"/success","success_value":"true","success_missing_allowed":false,"error_pointer":"/message"},"bestchange":null}'::JSONB, '{}'::JSONB, '{"kind":"indicative_rate","description":"Cifra''s public calculator exposes an indicative conversion rate but no transaction, deposit, or withdrawal fee fields; confirm all execution costs before trading.","docs_url":"https://cifra.by/"}'::JSONB, '{}'::JSONB, 'cifra-broker/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('cifra-broker', 'sell', 'https://cifra.by/', 'Cifra Markets Sell', ARRAY['BYN', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{"p2p":{"kind":"http_json","market":"direct_exchange","endpoint":"https://api.cifra-broker.by/api/site/ticker-calculator","method":"GET","headers":{"Accept-Language":"ru-RU,ru;q=0.9","Origin":"https://cifra.by","Referer":"https://cifra.by/","User-Agent":"Mozilla/5.0 (compatible; Pay3Flow/0.1)"},"asset_codes":{},"supported_assets":["USDT","USDC","BTC","BNB","SOL","TRX","DOGE","LTC","DAI","XRP","ADA","DOT","LINK","AVAX","BCH","NEAR","APT","ATOM","UNI","SUI"],"timeout_ms":5000,"max_results":null,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":1.0,"default_max_fiat":1000000000.0,"default_available_asset":1000000000.0,"auth":null,"buy":{"query":{"key":"pay3flow"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":null,"success_pointer":"/success","success_value":"true","success_missing_allowed":false,"error_pointer":"/message","offer":null},"sell":{"query":{"key":"pay3flow"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":null,"success_pointer":"/success","success_value":"true","success_missing_allowed":false,"error_pointer":"/message","offer":null},"offer":null,"rate_table":{"fiat_items_pointer":"/data/currenciesReal","fiat_code_pointer":"/code","fiat_rate_pointer":"/rate/value","asset_items_pointer":"/data/currenciesNotReal","asset_code_pointer":"/code","asset_rates_pointer":"/data/currenciesNotRealRate","fiat_codes":{"RUB":"RUR"},"source_url":"https://tradernet.by/authentication/signup"}},"market":{"kind":"http_json","endpoint":"https://api.cifra-broker.by/api/site/ticker","method":"POST","headers":{"Accept-Language":"ru-RU,ru;q=0.9","Origin":"https://cifra.by","Referer":"https://cifra.by/catalog","User-Agent":"Mozilla/5.0 (compatible; Pay3Flow/0.1)"},"query":{},"request_json":"{\"limit\":1000,\"tags\":[]}","timeout_ms":5000,"items_pointer":"/data/ticker","symbol_pointer":"/name","bid_pointer":"/ltp","ask_pointer":"/ltp","symbol_remove":"-","success_pointer":"/success","success_value":"true","success_missing_allowed":false,"error_pointer":"/message"},"bestchange":null}'::JSONB, '{}'::JSONB, '{"kind":"indicative_rate","description":"Cifra''s public calculator exposes an indicative conversion rate but no transaction, deposit, or withdrawal fee fields; confirm all execution costs before trading.","docs_url":"https://cifra.by/"}'::JSONB, '{}'::JSONB, 'cifra-broker/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('cow-swap', 'buy', 'https://swap.cow.fi', 'CoW Swap Buy', ARRAY['COW', 'DAI', 'ETH', 'USDC', 'USDT', 'WETH']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{}'::JSONB, '{}'::JSONB, '{"kind":"quote_dependent","description":"No universal fixed protocol percentage: market-order costs are included in the live quote; liquidity, gas, and optional partner fees can affect execution.","docs_url":"https://docs.cow.fi/cow-protocol"}'::JSONB, '{}'::JSONB, 'cow-swap/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('cow-swap', 'sell', 'https://swap.cow.fi', 'CoW Swap Sell', ARRAY['COW', 'DAI', 'ETH', 'USDC', 'USDT', 'WETH']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{}'::JSONB, '{}'::JSONB, '{"kind":"quote_dependent","description":"No universal fixed protocol percentage: market-order costs are included in the live quote; liquidity, gas, and optional partner fees can affect execution.","docs_url":"https://docs.cow.fi/cow-protocol"}'::JSONB, '{}'::JSONB, 'cow-swap/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('dzengi', 'buy', 'https://dzengi.com/ru/kalkulyator-kriptovalyut', 'Dzengi Buy', ARRAY['BYN', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{"p2p":null,"market":{"kind":"http_json","endpoint":"https://api-adapter.dzengi.com/api/v1/ticker/24hr","method":"GET","headers":{},"query":{},"request_json":null,"timeout_ms":10000,"items_pointer":null,"symbol_pointer":"/symbol","bid_pointer":"/bidPrice","ask_pointer":"/askPrice","symbol_remove":"/","success_pointer":null,"success_value":null,"success_missing_allowed":false,"error_pointer":null},"bestchange":null}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, 'dzengi/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('dzengi', 'sell', 'https://dzengi.com/ru/kalkulyator-kriptovalyut', 'Dzengi Sell', ARRAY['BYN', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{"p2p":null,"market":{"kind":"http_json","endpoint":"https://api-adapter.dzengi.com/api/v1/ticker/24hr","method":"GET","headers":{},"query":{},"request_json":null,"timeout_ms":10000,"items_pointer":null,"symbol_pointer":"/symbol","bid_pointer":"/bidPrice","ask_pointer":"/askPrice","symbol_remove":"/","success_pointer":null,"success_value":null,"success_missing_allowed":false,"error_pointer":null},"bestchange":null}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, 'dzengi/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('enso', 'buy', 'https://enso.finance/', 'Enso Buy', ARRAY['ETH', 'USDC', 'USDT']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{"description":"Catalog entry for routes powered by Enso. Token and network availability depends on the selected integration.","steps":["Check the application using Enso for supported tokens and networks.","Review the route, output amount, network fees, and slippage before signing.","Confirm transactions only in a wallet or application you trust."],"links":[{"label":"Enso","url":"https://enso.finance/"}]}'::JSONB, 'enso/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('enso', 'sell', 'https://enso.finance/', 'Enso Sell', ARRAY['ETH', 'USDC', 'USDT']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{"description":"Catalog entry for routes powered by Enso. Token and network availability depends on the selected integration.","steps":["Check the application using Enso for supported tokens and networks.","Review the route, output amount, network fees, and slippage before signing.","Confirm transactions only in a wallet or application you trust."],"links":[{"label":"Enso","url":"https://enso.finance/"}]}'::JSONB, 'enso/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('id-pay', 'buy', 'https://id-pay.ru/', 'ID Pay Buy', ARRAY['AMD', 'RUB']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{}'::JSONB, '{}'::JSONB, '{"kind":"quote_dependent","description":"ID Pay advertises commission-free transfers for supported corridors; confirm the live rate and any card or bank charges before sending.","docs_url":"https://id-pay.ru/tariff/"}'::JSONB, '{}'::JSONB, 'id-pay/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('id-pay', 'sell', 'https://id-pay.ru/', 'ID Pay Sell', ARRAY['AMD', 'RUB']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{}'::JSONB, '{}'::JSONB, '{"kind":"quote_dependent","description":"ID Pay advertises commission-free transfers for supported corridors; confirm the live rate and any card or bank charges before sending.","docs_url":"https://id-pay.ru/tariff/"}'::JSONB, '{}'::JSONB, 'id-pay/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('kyberswap', 'buy', 'https://kyberswap.com/', 'KyberSwap Buy', ARRAY['ETH', 'USDC', 'USDT']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{"description":"Catalog entry for token swaps through KyberSwap. Token and network availability depends on the selected route.","steps":["Open KyberSwap and select the source and destination tokens and networks.","Review the quoted output, network fees, and slippage in KyberSwap.","Connect your wallet and confirm the transaction only after checking the network and token addresses."],"links":[{"label":"Open KyberSwap","url":"https://kyberswap.com/"}]}'::JSONB, 'kyberswap/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('kyberswap', 'sell', 'https://kyberswap.com/', 'KyberSwap Sell', ARRAY['ETH', 'USDC', 'USDT']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{"description":"Catalog entry for token swaps through KyberSwap. Token and network availability depends on the selected route.","steps":["Open KyberSwap and select the source and destination tokens and networks.","Review the quoted output, network fees, and slippage in KyberSwap.","Connect your wallet and confirm the transaction only after checking the network and token addresses."],"links":[{"label":"Open KyberSwap","url":"https://kyberswap.com/"}]}'::JSONB, 'kyberswap/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('lifi', 'buy', 'https://jumper.exchange/', 'LI.FI Buy', ARRAY['ETH', 'USDC', 'USDT']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{"description":"Catalog entry for LI.FI-powered swaps and bridges through Jumper. Token and network availability depends on the selected route.","steps":["Open Jumper and select the source and destination tokens and networks.","Review the route, quoted output, fees, and estimated completion time.","Connect your wallet and confirm the transaction only after checking both networks and destination details."],"links":[{"label":"Open Jumper by LI.FI","url":"https://jumper.exchange/"}]}'::JSONB, 'lifi/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('lifi', 'sell', 'https://jumper.exchange/', 'LI.FI Sell', ARRAY['ETH', 'USDC', 'USDT']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{"description":"Catalog entry for LI.FI-powered swaps and bridges through Jumper. Token and network availability depends on the selected route.","steps":["Open Jumper and select the source and destination tokens and networks.","Review the route, quoted output, fees, and estimated completion time.","Connect your wallet and confirm the transaction only after checking both networks and destination details."],"links":[{"label":"Open Jumper by LI.FI","url":"https://jumper.exchange/"}]}'::JSONB, 'lifi/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('mexc', 'buy', 'https://www.mexc.com/buy-crypto/p2p', 'MEXC Buy', ARRAY['AMD', 'BYN', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], ARRAY['p2p', 'exchanger']::TEXT[], '{"p2p":{"kind":"http_json","endpoint":"https://www.mexc.co/api/platform/p2p/api/market","method":"GET","headers":{"Accept":"application/json","Origin":"https://www.mexc.co","Referer":"https://www.mexc.co/en-NG/buy-crypto/p2p","X-Client":"WEB","X-Device-Id":"unknowndeviceid"},"asset_codes":{"BTC":"febc9973be4d4d53bb374476239eb219","ETH":"93c38b0169214f8689763ce9a63a73ff","USDC":"34309140878b4ae99f195ac091d49bab","USDT":"128f589271cb4951b03e71e6323eb7be"},"supported_assets":["USDT","USDC","BTC","ETH"],"supported_fiats":["USD","RUB","EUR","AMD","BYN"],"payment_method_aliases":{"Alfa-Bank Belarus":["Alfa Bank Belarus","A-Bank"],"Bank Dabrabyt":["Dabrabyt"],"Bank Reshenie":["Reshenie Bank"],"Belagroprombank":["Agrobank","Belagroprom Bank"],"Paritetbank":["Paritet Bank"],"Priorbank":["PriorBank"],"Sber Bank Belarus":["BPS-Sberbank","Sber Bank"],"Sberbank":["Sber"],"T-Bank":["Tinkoff","Tinkoff Bank"],"VTB Belarus":["VTB Bank Belarus","Bank VTB Belarus"]},"timeout_ms":10000,"max_results":20,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":null,"default_max_fiat":null,"default_available_asset":null,"auth":null,"buy":{"query":{"adOrderSort":"","adOrderSortField":"","adsType":"1","allowTrade":"false","amount":"{{amount}}","blockTrade":"false","certifiedMerchant":"false","coinId":"{{asset}}","countryCode":"","currency":"{{fiat}}","follow":"false","haveTrade":"false","page":"1","pageSize":"{{limit_number}}","payMethod":"","tradeType":"SELL"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":"/data","success_pointer":"/code","success_value":"0","success_missing_allowed":false,"error_pointer":"/msg","offer":null},"sell":{"query":{"adOrderSort":"","adOrderSortField":"","adsType":"1","allowTrade":"false","amount":"{{amount}}","blockTrade":"false","certifiedMerchant":"false","coinId":"{{asset}}","countryCode":"","currency":"{{fiat}}","follow":"false","haveTrade":"false","page":"1","pageSize":"{{limit_number}}","payMethod":"","tradeType":"BUY"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":"/data","success_pointer":"/code","success_value":"0","success_missing_allowed":false,"error_pointer":"/msg","offer":null},"offer":{"ad_id_pointer":"/id","fiat_pointer":"/currency","asset_pointer":"/coinName","price_pointer":"/price","fiat_amount_pointer":null,"asset_amount_pointer":null,"price_inverted":false,"available_asset_pointer":"/availableQuantity","min_fiat_pointer":"/minTradeLimit","max_fiat_pointer":"/maxTradeLimit","payment_methods_pointer":"/payMethod","payment_method_value_pointer":null,"payment_method_fallback_pointer":null,"pay_time_limit_pointer":"/expirationTime","advertiser_id_pointer":"/merchant/memberId","advertiser_nickname_pointer":"/merchant/nickName","advertiser_user_type_pointer":"/merchant/merchantType","merchant_conditions":[{"pointer":"/merchant/merchantType","operator":"equals_ci","value":"merchant"},{"pointer":"/merchant/merchantType","operator":"equals_ci","value":"prime"},{"pointer":"/merchant/merchantType","operator":"equals_ci","value":"legacy"},{"pointer":"/merchant/merchantType","operator":"equals_ci","value":"biz"}],"verified_conditions":[],"merchant_default":false,"verified_default":false,"verified_from_merchant":true,"completed_orders_pointer":"/merchantStatistics/doneLastMonthCount","completion_rate_pointer":"/merchantStatistics/thirtyDayCompletionRate","positive_rate_pointer":"/merchantStatistics/goodRate","source_url_template":"https://www.mexc.com/buy-crypto/p2p","source_url_is_exact":false,"advertiser_profile_url_template":"https://www.mexc.com/buy-crypto/user-info/{{item:/merchant/memberId}}","merchant_profile_url_template":"https://www.mexc.com/buy-crypto/merchant/{{item:/merchant/memberId}}"},"rate_table":null},"market":{"kind":"http_json","endpoint":"https://api.mexc.com/api/v3/ticker/bookTicker","method":"GET","headers":{},"query":{},"request_json":null,"timeout_ms":4000,"items_pointer":null,"symbol_pointer":"/symbol","bid_pointer":"/bidPrice","ask_pointer":"/askPrice","symbol_remove":null,"success_pointer":null,"success_value":null,"success_missing_allowed":false,"error_pointer":null},"bestchange":null}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, 'mexc/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('mexc', 'sell', 'https://www.mexc.com/buy-crypto/p2p', 'MEXC Sell', ARRAY['AMD', 'BYN', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], ARRAY['p2p', 'exchanger']::TEXT[], '{"p2p":{"kind":"http_json","endpoint":"https://www.mexc.co/api/platform/p2p/api/market","method":"GET","headers":{"Accept":"application/json","Origin":"https://www.mexc.co","Referer":"https://www.mexc.co/en-NG/buy-crypto/p2p","X-Client":"WEB","X-Device-Id":"unknowndeviceid"},"asset_codes":{"BTC":"febc9973be4d4d53bb374476239eb219","ETH":"93c38b0169214f8689763ce9a63a73ff","USDC":"34309140878b4ae99f195ac091d49bab","USDT":"128f589271cb4951b03e71e6323eb7be"},"supported_assets":["USDT","USDC","BTC","ETH"],"supported_fiats":["USD","RUB","EUR","AMD","BYN"],"payment_method_aliases":{"Alfa-Bank Belarus":["Alfa Bank Belarus","A-Bank"],"Bank Dabrabyt":["Dabrabyt"],"Bank Reshenie":["Reshenie Bank"],"Belagroprombank":["Agrobank","Belagroprom Bank"],"Paritetbank":["Paritet Bank"],"Priorbank":["PriorBank"],"Sber Bank Belarus":["BPS-Sberbank","Sber Bank"],"Sberbank":["Sber"],"T-Bank":["Tinkoff","Tinkoff Bank"],"VTB Belarus":["VTB Bank Belarus","Bank VTB Belarus"]},"timeout_ms":10000,"max_results":20,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":null,"default_max_fiat":null,"default_available_asset":null,"auth":null,"buy":{"query":{"adOrderSort":"","adOrderSortField":"","adsType":"1","allowTrade":"false","amount":"{{amount}}","blockTrade":"false","certifiedMerchant":"false","coinId":"{{asset}}","countryCode":"","currency":"{{fiat}}","follow":"false","haveTrade":"false","page":"1","pageSize":"{{limit_number}}","payMethod":"","tradeType":"SELL"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":"/data","success_pointer":"/code","success_value":"0","success_missing_allowed":false,"error_pointer":"/msg","offer":null},"sell":{"query":{"adOrderSort":"","adOrderSortField":"","adsType":"1","allowTrade":"false","amount":"{{amount}}","blockTrade":"false","certifiedMerchant":"false","coinId":"{{asset}}","countryCode":"","currency":"{{fiat}}","follow":"false","haveTrade":"false","page":"1","pageSize":"{{limit_number}}","payMethod":"","tradeType":"BUY"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":"/data","success_pointer":"/code","success_value":"0","success_missing_allowed":false,"error_pointer":"/msg","offer":null},"offer":{"ad_id_pointer":"/id","fiat_pointer":"/currency","asset_pointer":"/coinName","price_pointer":"/price","fiat_amount_pointer":null,"asset_amount_pointer":null,"price_inverted":false,"available_asset_pointer":"/availableQuantity","min_fiat_pointer":"/minTradeLimit","max_fiat_pointer":"/maxTradeLimit","payment_methods_pointer":"/payMethod","payment_method_value_pointer":null,"payment_method_fallback_pointer":null,"pay_time_limit_pointer":"/expirationTime","advertiser_id_pointer":"/merchant/memberId","advertiser_nickname_pointer":"/merchant/nickName","advertiser_user_type_pointer":"/merchant/merchantType","merchant_conditions":[{"pointer":"/merchant/merchantType","operator":"equals_ci","value":"merchant"},{"pointer":"/merchant/merchantType","operator":"equals_ci","value":"prime"},{"pointer":"/merchant/merchantType","operator":"equals_ci","value":"legacy"},{"pointer":"/merchant/merchantType","operator":"equals_ci","value":"biz"}],"verified_conditions":[],"merchant_default":false,"verified_default":false,"verified_from_merchant":true,"completed_orders_pointer":"/merchantStatistics/doneLastMonthCount","completion_rate_pointer":"/merchantStatistics/thirtyDayCompletionRate","positive_rate_pointer":"/merchantStatistics/goodRate","source_url_template":"https://www.mexc.com/buy-crypto/p2p","source_url_is_exact":false,"advertiser_profile_url_template":"https://www.mexc.com/buy-crypto/user-info/{{item:/merchant/memberId}}","merchant_profile_url_template":"https://www.mexc.com/buy-crypto/merchant/{{item:/merchant/memberId}}"},"rate_table":null},"market":{"kind":"http_json","endpoint":"https://api.mexc.com/api/v3/ticker/bookTicker","method":"GET","headers":{},"query":{},"request_json":null,"timeout_ms":4000,"items_pointer":null,"symbol_pointer":"/symbol","bid_pointer":"/bidPrice","ask_pointer":"/askPrice","symbol_remove":null,"success_pointer":null,"success_value":null,"success_missing_allowed":false,"error_pointer":null},"bestchange":null}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, 'mexc/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('near-intents', 'buy', 'https://1click.chaindefuser.com', 'NEAR Intents Buy', ARRAY['BTC', 'DAI', 'ETH', 'NEAR', 'SOL', 'USDC', 'USDT', 'XRP']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{}'::JSONB, '{}'::JSONB, '{"kind":"quote_dependent","description":"Fees and the guaranteed output are returned by each live NEAR Intents quote and may differ from other providers.","docs_url":"https://docs.near-intents.org/near-intents"}'::JSONB, '{}'::JSONB, 'near-intents/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('near-intents', 'sell', 'https://1click.chaindefuser.com', 'NEAR Intents Sell', ARRAY['BTC', 'DAI', 'ETH', 'NEAR', 'SOL', 'USDC', 'USDT', 'XRP']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{}'::JSONB, '{}'::JSONB, '{"kind":"quote_dependent","description":"Fees and the guaranteed output are returned by each live NEAR Intents quote and may differ from other providers.","docs_url":"https://docs.near-intents.org/near-intents"}'::JSONB, '{}'::JSONB, 'near-intents/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('nordstern', 'buy', 'https://docs.nordstern.finance/', 'Nordstern Buy', ARRAY['ETH', 'USDC', 'USDT']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{"description":"Catalog entry for the Nordstern DEX aggregator API. Token and network availability depends on the selected route and integrator.","steps":["Check the integrator using Nordstern for supported tokens and networks.","Review the final route, output amount, gas estimate, and slippage before signing.","Confirm transactions only in a wallet or application you trust."],"links":[{"label":"Nordstern documentation","url":"https://docs.nordstern.finance/"}]}'::JSONB, 'nordstern/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('nordstern', 'sell', 'https://docs.nordstern.finance/', 'Nordstern Sell', ARRAY['ETH', 'USDC', 'USDT']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{"description":"Catalog entry for the Nordstern DEX aggregator API. Token and network availability depends on the selected route and integrator.","steps":["Check the integrator using Nordstern for supported tokens and networks.","Review the final route, output amount, gas estimate, and slippage before signing.","Confirm transactions only in a wallet or application you trust."],"links":[{"label":"Nordstern documentation","url":"https://docs.nordstern.finance/"}]}'::JSONB, 'nordstern/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('okx', 'buy', 'https://www.okx.com', 'OKX Buy', ARRAY['AMD', 'BYN', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], ARRAY['p2p', 'exchanger']::TEXT[], '{"p2p":{"kind":"http_json","endpoint":"https://www.okx.com/v3/c2c/tradingOrders/books","method":"GET","headers":{"Origin":"https://www.okx.com","Referer":"https://www.okx.com/p2p-markets/"},"asset_codes":{},"supported_assets":["USDT","USDC","BTC","ETH"],"supported_fiats":["USD","RUB","EUR","AMD","BYN"],"payment_method_aliases":{"Alfa-Bank Belarus":["Alfa Bank Belarus","A-Bank"],"Bank Dabrabyt":["Dabrabyt"],"Bank Reshenie":["Reshenie Bank"],"Belagroprombank":["Agrobank","Belagroprom Bank"],"Paritetbank":["Paritet Bank"],"Priorbank":["PriorBank"],"Sber Bank Belarus":["BPS-Sberbank","Sber Bank"],"Sberbank":["Sber"],"T-Bank":["Tinkoff","Tinkoff Bank"],"VTB Belarus":["VTB Bank Belarus","Bank VTB Belarus"]},"timeout_ms":10000,"max_results":null,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":null,"default_max_fiat":null,"default_available_asset":null,"auth":null,"buy":{"query":{"baseCurrency":"{{asset}}","isAbleFilter":"false","paymentMethod":"all","quoteCurrency":"{{fiat}}","showAlreadyTraded":"false","showFollow":"false","showTrade":"false","side":"sell","urlId":"0","userType":"all"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":"/data/sell","success_pointer":"/code","success_value":"0","success_missing_allowed":false,"error_pointer":"/detailMsg","offer":null},"sell":{"query":{"baseCurrency":"{{asset}}","isAbleFilter":"false","paymentMethod":"all","quoteCurrency":"{{fiat}}","showAlreadyTraded":"false","showFollow":"false","showTrade":"false","side":"buy","urlId":"0","userType":"all"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":"/data/buy","success_pointer":"/code","success_value":"0","success_missing_allowed":false,"error_pointer":"/detailMsg","offer":null},"offer":{"ad_id_pointer":"/id","fiat_pointer":"/quoteCurrency","asset_pointer":"/baseCurrency","price_pointer":"/price","fiat_amount_pointer":null,"asset_amount_pointer":null,"price_inverted":false,"available_asset_pointer":"/availableAmount","min_fiat_pointer":"/quoteMinAmountPerOrder","max_fiat_pointer":"/quoteMaxAmountPerOrder","payment_methods_pointer":"/paymentMethods","payment_method_value_pointer":null,"payment_method_fallback_pointer":null,"pay_time_limit_pointer":"/paymentTimeoutMinutes","advertiser_id_pointer":"/publicUserId","advertiser_nickname_pointer":"/nickName","advertiser_user_type_pointer":"/userType","merchant_conditions":[{"pointer":"/isInstitution","operator":"truthy","value":null},{"pointer":"/merchantId","operator":"non_empty","value":null},{"pointer":"/userType","operator":"equals_ci","value":"merchant"}],"verified_conditions":[],"merchant_default":false,"verified_default":false,"verified_from_merchant":true,"completed_orders_pointer":"/completedOrderQuantity","completion_rate_pointer":"/completedRate","positive_rate_pointer":"/posReviewPercentage","source_url_template":"https://www.okx.com/p2p-markets/{{fiat_lower}}/{{side}}-{{asset_lower}}","source_url_is_exact":false,"advertiser_profile_url_template":"https://www.okx.com/p2p/ads-merchant?publicUserId={{item:/publicUserId}}"},"rate_table":null},"market":{"kind":"http_json","endpoint":"https://www.okx.com/api/v5/market/tickers","method":"GET","headers":{},"query":{"instType":"SPOT"},"request_json":null,"timeout_ms":10000,"items_pointer":"/data","symbol_pointer":"/instId","bid_pointer":"/bidPx","ask_pointer":"/askPx","symbol_remove":"-","success_pointer":"/code","success_value":"0","success_missing_allowed":false,"error_pointer":"/msg"},"bestchange":null}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, 'okx/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('okx', 'sell', 'https://www.okx.com', 'OKX Sell', ARRAY['AMD', 'BYN', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], ARRAY['p2p', 'exchanger']::TEXT[], '{"p2p":{"kind":"http_json","endpoint":"https://www.okx.com/v3/c2c/tradingOrders/books","method":"GET","headers":{"Origin":"https://www.okx.com","Referer":"https://www.okx.com/p2p-markets/"},"asset_codes":{},"supported_assets":["USDT","USDC","BTC","ETH"],"supported_fiats":["USD","RUB","EUR","AMD","BYN"],"payment_method_aliases":{"Alfa-Bank Belarus":["Alfa Bank Belarus","A-Bank"],"Bank Dabrabyt":["Dabrabyt"],"Bank Reshenie":["Reshenie Bank"],"Belagroprombank":["Agrobank","Belagroprom Bank"],"Paritetbank":["Paritet Bank"],"Priorbank":["PriorBank"],"Sber Bank Belarus":["BPS-Sberbank","Sber Bank"],"Sberbank":["Sber"],"T-Bank":["Tinkoff","Tinkoff Bank"],"VTB Belarus":["VTB Bank Belarus","Bank VTB Belarus"]},"timeout_ms":10000,"max_results":null,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":null,"default_max_fiat":null,"default_available_asset":null,"auth":null,"buy":{"query":{"baseCurrency":"{{asset}}","isAbleFilter":"false","paymentMethod":"all","quoteCurrency":"{{fiat}}","showAlreadyTraded":"false","showFollow":"false","showTrade":"false","side":"sell","urlId":"0","userType":"all"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":"/data/sell","success_pointer":"/code","success_value":"0","success_missing_allowed":false,"error_pointer":"/detailMsg","offer":null},"sell":{"query":{"baseCurrency":"{{asset}}","isAbleFilter":"false","paymentMethod":"all","quoteCurrency":"{{fiat}}","showAlreadyTraded":"false","showFollow":"false","showTrade":"false","side":"buy","urlId":"0","userType":"all"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":"/data/buy","success_pointer":"/code","success_value":"0","success_missing_allowed":false,"error_pointer":"/detailMsg","offer":null},"offer":{"ad_id_pointer":"/id","fiat_pointer":"/quoteCurrency","asset_pointer":"/baseCurrency","price_pointer":"/price","fiat_amount_pointer":null,"asset_amount_pointer":null,"price_inverted":false,"available_asset_pointer":"/availableAmount","min_fiat_pointer":"/quoteMinAmountPerOrder","max_fiat_pointer":"/quoteMaxAmountPerOrder","payment_methods_pointer":"/paymentMethods","payment_method_value_pointer":null,"payment_method_fallback_pointer":null,"pay_time_limit_pointer":"/paymentTimeoutMinutes","advertiser_id_pointer":"/publicUserId","advertiser_nickname_pointer":"/nickName","advertiser_user_type_pointer":"/userType","merchant_conditions":[{"pointer":"/isInstitution","operator":"truthy","value":null},{"pointer":"/merchantId","operator":"non_empty","value":null},{"pointer":"/userType","operator":"equals_ci","value":"merchant"}],"verified_conditions":[],"merchant_default":false,"verified_default":false,"verified_from_merchant":true,"completed_orders_pointer":"/completedOrderQuantity","completion_rate_pointer":"/completedRate","positive_rate_pointer":"/posReviewPercentage","source_url_template":"https://www.okx.com/p2p-markets/{{fiat_lower}}/{{side}}-{{asset_lower}}","source_url_is_exact":false,"advertiser_profile_url_template":"https://www.okx.com/p2p/ads-merchant?publicUserId={{item:/publicUserId}}"},"rate_table":null},"market":{"kind":"http_json","endpoint":"https://www.okx.com/api/v5/market/tickers","method":"GET","headers":{},"query":{"instType":"SPOT"},"request_json":null,"timeout_ms":10000,"items_pointer":"/data","symbol_pointer":"/instId","bid_pointer":"/bidPx","ask_pointer":"/askPx","symbol_remove":"-","success_pointer":"/code","success_value":"0","success_missing_allowed":false,"error_pointer":"/msg"},"bestchange":null}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, 'okx/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('papa-change', 'buy', 'https://papa-change.biz/', 'Papa Change Buy', ARRAY['AED', 'CAD', 'EUR', 'KGS', 'KZT', 'RUB', 'TRY', 'UAH', 'USD', 'UZS']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{"p2p":null,"market":null,"bestchange":null,"papa_change":{"directions_endpoint":"https://api.papa-change.biz/users-directions/get-all-available","rates_endpoint":"https://api.papa-change.biz/exchange-rates-api/get-all","public_endpoint":"https://papa-change.biz/","timeout_ms":10000,"cache_ttl_ms":30000,"max_results":100}}'::JSONB, '{}'::JSONB, '{"kind":"included_in_quote","description":"The live rate includes Papa Change''s direction commission and public amount-tiered rate; payment-system and blockchain fees may still apply.","docs_url":"https://papa-change.biz/agreement"}'::JSONB, '{}'::JSONB, 'papa-change/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('papa-change', 'sell', 'https://papa-change.biz/', 'Papa Change Sell', ARRAY['AED', 'CAD', 'EUR', 'KGS', 'KZT', 'RUB', 'TRY', 'UAH', 'USD', 'UZS']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{"p2p":null,"market":null,"bestchange":null,"papa_change":{"directions_endpoint":"https://api.papa-change.biz/users-directions/get-all-available","rates_endpoint":"https://api.papa-change.biz/exchange-rates-api/get-all","public_endpoint":"https://papa-change.biz/","timeout_ms":10000,"cache_ttl_ms":30000,"max_results":100}}'::JSONB, '{}'::JSONB, '{"kind":"included_in_quote","description":"The live rate includes Papa Change''s direction commission and public amount-tiered rate; payment-system and blockchain fees may still apply.","docs_url":"https://papa-change.biz/agreement"}'::JSONB, '{}'::JSONB, 'papa-change/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('rapira', 'buy', 'https://rapira.net', 'Rapira Buy', ARRAY['RUB']::TEXT[], ARRAY[]::TEXT[], ARRAY['p2p']::TEXT[], '{"p2p":{"kind":"http_json","endpoint":"https://api.rapira.net/otc/offers/page-query/v2","method":"GET","headers":{"Origin":"https://rapira.net","Referer":"https://rapira.net/ru/p2p/BUY?p=1"},"asset_codes":{},"supported_assets":["USDT"],"timeout_ms":4000,"max_results":100,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":null,"default_max_fiat":null,"default_available_asset":null,"auth":null,"buy":{"query":{"amount":"{{amount}}","externalCoinUnit":"{{fiat}}","internalCoinUnit":"{{asset}}","listingType":"RECOMMENDED","merchantSide":"SELL","pageNo":"1","pageSize":"{{limit}}","paymentIds":"","showOnlyEligible":"false","sortByNumberOfMutualOrders":"false"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":"/content","success_pointer":"/code","success_value":"0","success_missing_allowed":true,"error_pointer":"/message","offer":null},"sell":{"query":{"amount":"{{amount}}","externalCoinUnit":"{{fiat}}","internalCoinUnit":"{{asset}}","listingType":"RECOMMENDED","merchantSide":"BUY","pageNo":"1","pageSize":"{{limit}}","paymentIds":"","showOnlyEligible":"false","sortByNumberOfMutualOrders":"false"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":"/content","success_pointer":"/code","success_value":"0","success_missing_allowed":true,"error_pointer":"/message","offer":null},"offer":{"ad_id_pointer":"/advertiseId","fiat_pointer":"/externalCoinUnit","asset_pointer":"/internalCoinUnit","price_pointer":"/price","fiat_amount_pointer":null,"asset_amount_pointer":null,"price_inverted":false,"available_asset_pointer":"/quantity","min_fiat_pointer":"/minLimit","max_fiat_pointer":"/maxLimit","payment_methods_pointer":"/paymentTypes","payment_method_value_pointer":"/paymentName","payment_method_fallback_pointer":null,"pay_time_limit_pointer":"/timeLimit","advertiser_id_pointer":"/merchant/profileUid","advertiser_nickname_pointer":"/merchant/username","advertiser_user_type_pointer":"/merchant/p2pLevel","merchant_conditions":[],"verified_conditions":[{"pointer":"/merchant/p2pLevel","operator":"equals_ci","value":"verified"},{"pointer":"/merchant/p2pLevel","operator":"equals_ci","value":"trusted"},{"pointer":"/merchant/p2pLevel","operator":"equals_ci","value":"premium"}],"merchant_default":true,"verified_default":false,"verified_from_merchant":false,"completed_orders_pointer":"/merchant/totalTerminatedAfterAcceptCount","completion_rate_pointer":"/merchant/totalCompletedPercent","positive_rate_pointer":null,"source_url_template":"https://rapira.net/p2p?adId={{item:/advertiseId}}","source_url_is_exact":true,"advertiser_profile_url_template":"https://rapira.net/ru/p2p/profileUser?p=1&profileUid={{item:/merchant/profileUid}}&msp=1"},"rate_table":null},"market":null,"bestchange":null}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, 'rapira/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('rapira', 'sell', 'https://rapira.net', 'Rapira Sell', ARRAY['RUB']::TEXT[], ARRAY[]::TEXT[], ARRAY['p2p']::TEXT[], '{"p2p":{"kind":"http_json","endpoint":"https://api.rapira.net/otc/offers/page-query/v2","method":"GET","headers":{"Origin":"https://rapira.net","Referer":"https://rapira.net/ru/p2p/BUY?p=1"},"asset_codes":{},"supported_assets":["USDT"],"timeout_ms":4000,"max_results":100,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":null,"default_max_fiat":null,"default_available_asset":null,"auth":null,"buy":{"query":{"amount":"{{amount}}","externalCoinUnit":"{{fiat}}","internalCoinUnit":"{{asset}}","listingType":"RECOMMENDED","merchantSide":"SELL","pageNo":"1","pageSize":"{{limit}}","paymentIds":"","showOnlyEligible":"false","sortByNumberOfMutualOrders":"false"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":"/content","success_pointer":"/code","success_value":"0","success_missing_allowed":true,"error_pointer":"/message","offer":null},"sell":{"query":{"amount":"{{amount}}","externalCoinUnit":"{{fiat}}","internalCoinUnit":"{{asset}}","listingType":"RECOMMENDED","merchantSide":"BUY","pageNo":"1","pageSize":"{{limit}}","paymentIds":"","showOnlyEligible":"false","sortByNumberOfMutualOrders":"false"},"request_json":null,"amount_mode":"query_or_empty","items_pointer":"/content","success_pointer":"/code","success_value":"0","success_missing_allowed":true,"error_pointer":"/message","offer":null},"offer":{"ad_id_pointer":"/advertiseId","fiat_pointer":"/externalCoinUnit","asset_pointer":"/internalCoinUnit","price_pointer":"/price","fiat_amount_pointer":null,"asset_amount_pointer":null,"price_inverted":false,"available_asset_pointer":"/quantity","min_fiat_pointer":"/minLimit","max_fiat_pointer":"/maxLimit","payment_methods_pointer":"/paymentTypes","payment_method_value_pointer":"/paymentName","payment_method_fallback_pointer":null,"pay_time_limit_pointer":"/timeLimit","advertiser_id_pointer":"/merchant/profileUid","advertiser_nickname_pointer":"/merchant/username","advertiser_user_type_pointer":"/merchant/p2pLevel","merchant_conditions":[],"verified_conditions":[{"pointer":"/merchant/p2pLevel","operator":"equals_ci","value":"verified"},{"pointer":"/merchant/p2pLevel","operator":"equals_ci","value":"trusted"},{"pointer":"/merchant/p2pLevel","operator":"equals_ci","value":"premium"}],"merchant_default":true,"verified_default":false,"verified_from_merchant":false,"completed_orders_pointer":"/merchant/totalTerminatedAfterAcceptCount","completion_rate_pointer":"/merchant/totalCompletedPercent","positive_rate_pointer":null,"source_url_template":"https://rapira.net/p2p?adId={{item:/advertiseId}}","source_url_is_exact":true,"advertiser_profile_url_template":"https://rapira.net/ru/p2p/profileUser?p=1&profileUid={{item:/merchant/profileUid}}&msp=1"},"rate_table":null},"market":null,"bestchange":null}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, 'rapira/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('skylabs', 'buy', 'https://skylabs.world/', 'SkyLabs Buy', ARRAY['AMD', 'USD']::TEXT[], ARRAY['Bank Transfer', 'EasyPay', 'SkyLabs ATM', 'Telcell']::TEXT[], ARRAY['exchanger']::TEXT[], '{"p2p":{"kind":"http_json","market":"direct_exchange","endpoint":"https://api.skylabs.world/api/rate/all","method":"GET","headers":{"Origin":"https://skylabs.world","Referer":"https://skylabs.world/"},"asset_codes":{},"supported_assets":["BTC","ETH","USDT","BNB","TRX","USDC","TON","LTC","MATIC","SOL"],"payment_method_aliases":{"Cash":["Cash USD","Cash Dollar","SkyLabs ATM"]},"timeout_ms":5000,"max_results":null,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":1.0,"default_max_fiat":1000000000.0,"default_available_asset":1000000000.0,"auth":null,"buy":{"endpoint":"https://api.skylabs.world/api/rate/{{asset}}/{{fiat}}/sell","query":{},"request_json":null,"amount_mode":"query_or_empty","items_pointer":null,"success_pointer":"/status","success_value":"true","success_missing_allowed":false,"error_pointer":null,"offer":null},"sell":{"endpoint":"https://api.skylabs.world/api/rate/{{asset}}/{{fiat}}/buy","query":{},"request_json":null,"amount_mode":"query_or_empty","items_pointer":null,"success_pointer":"/status","success_value":"true","success_missing_allowed":false,"error_pointer":null,"offer":null},"offer":{"ad_id_pointer":null,"fiat_pointer":null,"asset_pointer":null,"network_by_asset":{"SOL":"solana"},"price_pointer":"/result","fiat_amount_pointer":null,"asset_amount_pointer":null,"price_inverted":false,"available_asset_pointer":null,"min_fiat_pointer":null,"max_fiat_pointer":null,"payment_methods_pointer":null,"payment_method_value_pointer":null,"payment_method_fallback_pointer":null,"pay_time_limit_pointer":null,"advertiser_id_pointer":null,"advertiser_nickname_pointer":null,"advertiser_user_type_pointer":null,"merchant_conditions":[],"verified_conditions":[],"merchant_default":true,"verified_default":true,"verified_from_merchant":false,"completed_orders_pointer":null,"completion_rate_pointer":null,"positive_rate_pointer":null,"source_url_template":"https://skylabs.world/#rates","source_url_is_exact":false,"advertiser_profile_url_template":null},"rate_table":null},"market":null,"bestchange":null}'::JSONB, '{}'::JSONB, '{"kind":"quote_dependent","description":"The effective Pay3Flow rate includes SkyLabs'' live cash-in/cash-out or bank commission, conversion adjustment, and the selected network''s published crypto withdrawal fee.","docs_url":"https://skylabs.world/#calc"}'::JSONB, '{}'::JSONB, 'skylabs/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('skylabs', 'sell', 'https://skylabs.world/', 'SkyLabs Sell', ARRAY['AMD', 'USD']::TEXT[], ARRAY['Bank Transfer', 'EasyPay', 'SkyLabs ATM', 'Telcell']::TEXT[], ARRAY['exchanger']::TEXT[], '{"p2p":{"kind":"http_json","market":"direct_exchange","endpoint":"https://api.skylabs.world/api/rate/all","method":"GET","headers":{"Origin":"https://skylabs.world","Referer":"https://skylabs.world/"},"asset_codes":{},"supported_assets":["BTC","ETH","USDT","BNB","TRX","USDC","TON","LTC","MATIC","SOL"],"payment_method_aliases":{"Cash":["Cash USD","Cash Dollar","SkyLabs ATM"]},"timeout_ms":5000,"max_results":null,"fiat_probe_amount":null,"asset_probe_amount":null,"default_min_fiat":1.0,"default_max_fiat":1000000000.0,"default_available_asset":1000000000.0,"auth":null,"buy":{"endpoint":"https://api.skylabs.world/api/rate/{{asset}}/{{fiat}}/sell","query":{},"request_json":null,"amount_mode":"query_or_empty","items_pointer":null,"success_pointer":"/status","success_value":"true","success_missing_allowed":false,"error_pointer":null,"offer":null},"sell":{"endpoint":"https://api.skylabs.world/api/rate/{{asset}}/{{fiat}}/buy","query":{},"request_json":null,"amount_mode":"query_or_empty","items_pointer":null,"success_pointer":"/status","success_value":"true","success_missing_allowed":false,"error_pointer":null,"offer":null},"offer":{"ad_id_pointer":null,"fiat_pointer":null,"asset_pointer":null,"network_by_asset":{"SOL":"solana"},"price_pointer":"/result","fiat_amount_pointer":null,"asset_amount_pointer":null,"price_inverted":false,"available_asset_pointer":null,"min_fiat_pointer":null,"max_fiat_pointer":null,"payment_methods_pointer":null,"payment_method_value_pointer":null,"payment_method_fallback_pointer":null,"pay_time_limit_pointer":null,"advertiser_id_pointer":null,"advertiser_nickname_pointer":null,"advertiser_user_type_pointer":null,"merchant_conditions":[],"verified_conditions":[],"merchant_default":true,"verified_default":true,"verified_from_merchant":false,"completed_orders_pointer":null,"completion_rate_pointer":null,"positive_rate_pointer":null,"source_url_template":"https://skylabs.world/#rates","source_url_is_exact":false,"advertiser_profile_url_template":null},"rate_table":null},"market":null,"bestchange":null}'::JSONB, '{}'::JSONB, '{"kind":"quote_dependent","description":"The effective Pay3Flow rate includes SkyLabs'' live cash-in/cash-out or bank commission, conversion adjustment, and the selected network''s published crypto withdrawal fee.","docs_url":"https://skylabs.world/#calc"}'::JSONB, '{}'::JSONB, 'skylabs/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('symbiosis', 'buy', 'https://api.symbiosis.finance/crosschain/docs/', 'Symbiosis Buy', ARRAY['AVAX', 'BNB', 'BTC', 'ETH', 'MATIC', 'SOL', 'TON', 'TRX', 'USDC', 'USDT', 'XRP']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{}'::JSONB, '{}'::JSONB, '{"kind":"quote_dependent","description":"Symbiosis returns route and network fees with each live cross-chain quote; final execution costs depend on the selected route and network.","docs_url":"https://api.symbiosis.finance/crosschain/docs/"}'::JSONB, '{"description":"Cross-chain swap estimate. Pay3Flow does not connect your wallet or submit the transaction: open Symbiosis, verify the route, and approve the swap in your wallet.","steps":["Open Symbiosis and connect the wallet that holds the source asset.","Select the exact source and destination assets and networks shown in this route.","Check the amount received, slippage, provider fee, network fee, quote expiry, and the destination wallet address.","Submit the swap only after checking the network one more time, then wait for the destination transaction to complete."],"links":[{"label":"Open Symbiosis WebApp","url":"https://app.symbiosis.finance/"},{"label":"Symbiosis swap guide","url":"https://docs.symbiosis.finance/main-concepts/symbiosis-cross-chain-swaps"},{"label":"Symbiosis fees and troubleshooting","url":"https://docs.symbiosis.finance/user-guide-webapp/where-are-my-tokens-troubleshooting-guide"}]}'::JSONB, 'symbiosis/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('symbiosis', 'sell', 'https://api.symbiosis.finance/crosschain/docs/', 'Symbiosis Sell', ARRAY['AVAX', 'BNB', 'BTC', 'ETH', 'MATIC', 'SOL', 'TON', 'TRX', 'USDC', 'USDT', 'XRP']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{}'::JSONB, '{}'::JSONB, '{"kind":"quote_dependent","description":"Symbiosis returns route and network fees with each live cross-chain quote; final execution costs depend on the selected route and network.","docs_url":"https://api.symbiosis.finance/crosschain/docs/"}'::JSONB, '{"description":"Cross-chain swap estimate. Pay3Flow does not connect your wallet or submit the transaction: open Symbiosis, verify the route, and approve the swap in your wallet.","steps":["Open Symbiosis and connect the wallet that holds the source asset.","Select the exact source and destination assets and networks shown in this route.","Check the amount received, slippage, provider fee, network fee, quote expiry, and the destination wallet address.","Submit the swap only after checking the network one more time, then wait for the destination transaction to complete."],"links":[{"label":"Open Symbiosis WebApp","url":"https://app.symbiosis.finance/"},{"label":"Symbiosis swap guide","url":"https://docs.symbiosis.finance/main-concepts/symbiosis-cross-chain-swaps"},{"label":"Symbiosis fees and troubleshooting","url":"https://docs.symbiosis.finance/user-guide-webapp/where-are-my-tokens-troubleshooting-guide"}]}'::JSONB, 'symbiosis/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('velora', 'buy', 'https://app.velora.xyz/', 'Velora Buy', ARRAY['ETH', 'USDC', 'USDT']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{"description":"Catalog entry for token swaps through Velora. Token and network availability depends on the selected route.","steps":["Open Velora and select the source and destination tokens and networks.","Review the quoted output, network fees, and slippage in Velora.","Connect your wallet and confirm the transaction only after checking the network and token addresses."],"links":[{"label":"Open Velora","url":"https://app.velora.xyz/"}]}'::JSONB, 'velora/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('velora', 'sell', 'https://app.velora.xyz/', 'Velora Sell', ARRAY['ETH', 'USDC', 'USDT']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{}'::JSONB, '{}'::JSONB, '{}'::JSONB, '{"description":"Catalog entry for token swaps through Velora. Token and network availability depends on the selected route.","steps":["Open Velora and select the source and destination tokens and networks.","Review the quoted output, network fees, and slippage in Velora.","Connect your wallet and confirm the transaction only after checking the network and token addresses."],"links":[{"label":"Open Velora","url":"https://app.velora.xyz/"}]}'::JSONB, 'velora/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('whitebird', 'buy', 'https://whitebird.io/', 'Whitebird Buy', ARRAY['BYN', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{"p2p":{"kind":"http_json","market":"direct_exchange","endpoint":"https://api.whitebird.io/api/v3/exchange/client/quote","method":"POST","headers":{"Accept":"application/json","Origin":"https://whitebird.io","Referer":"https://whitebird.io/exchanger"},"asset_codes":{"USDC":"USDC_ERC","USDT":"USDT_TRC"},"supported_assets":["TRX","USDT","ETH","USDC","BTC","BNB","GRAM","SOL"],"timeout_ms":5000,"max_results":null,"fiat_probe_amount":1000.0,"asset_probe_amount":1.0,"default_min_fiat":1.0,"default_max_fiat":1000000000.0,"default_available_asset":1000000000.0,"auth":null,"buy":{"query":{},"request_json":"{\"input\":{\"asset\":\"{{fiat}}\",\"type\":\"FIAT_PROVIDER\",\"amount\":\"{{amount}}\"},\"output\":{\"asset\":\"{{asset}}\",\"type\":\"CRYPTO_TRANSFER\"},\"merchantId\":\"11111111-1111-1111-1111-111111111111\"}","amount_mode":"fiat_probe","items_pointer":null,"success_pointer":null,"success_value":null,"success_missing_allowed":false,"error_pointer":null,"offer":{"ad_id_pointer":null,"fiat_pointer":"/input/asset","asset_pointer":"/output/asset","price_pointer":null,"fiat_amount_pointer":"/input/amount","asset_amount_pointer":"/output/amount","price_inverted":false,"available_asset_pointer":null,"min_fiat_pointer":null,"max_fiat_pointer":null,"payment_methods_pointer":null,"payment_method_value_pointer":null,"payment_method_fallback_pointer":null,"pay_time_limit_pointer":null,"advertiser_id_pointer":null,"advertiser_nickname_pointer":null,"advertiser_user_type_pointer":null,"merchant_conditions":[],"verified_conditions":[],"merchant_default":true,"verified_default":true,"verified_from_merchant":false,"completed_orders_pointer":null,"completion_rate_pointer":null,"positive_rate_pointer":null,"source_url_template":"https://whitebird.io/exchanger","source_url_is_exact":false,"advertiser_profile_url_template":null}},"sell":{"query":{},"request_json":"{\"input\":{\"asset\":\"{{asset}}\",\"type\":\"CRYPTO_TRANSFER\",\"amount\":\"{{amount}}\"},\"output\":{\"asset\":\"{{fiat}}\",\"type\":\"FIAT_PROVIDER\"},\"merchantId\":\"11111111-1111-1111-1111-111111111111\"}","amount_mode":"asset_probe","items_pointer":null,"success_pointer":null,"success_value":null,"success_missing_allowed":false,"error_pointer":null,"offer":{"ad_id_pointer":null,"fiat_pointer":"/output/asset","asset_pointer":"/input/asset","price_pointer":null,"fiat_amount_pointer":"/output/amount","asset_amount_pointer":"/input/amount","price_inverted":false,"available_asset_pointer":null,"min_fiat_pointer":null,"max_fiat_pointer":null,"payment_methods_pointer":null,"payment_method_value_pointer":null,"payment_method_fallback_pointer":null,"pay_time_limit_pointer":null,"advertiser_id_pointer":null,"advertiser_nickname_pointer":null,"advertiser_user_type_pointer":null,"merchant_conditions":[],"verified_conditions":[],"merchant_default":true,"verified_default":true,"verified_from_merchant":false,"completed_orders_pointer":null,"completion_rate_pointer":null,"positive_rate_pointer":null,"source_url_template":"https://whitebird.io/exchanger","source_url_is_exact":false,"advertiser_profile_url_template":null}},"offer":null,"rate_table":null},"market":null,"bestchange":null}'::JSONB, '{}'::JSONB, '{"kind":"included_in_quote","description":"The effective Pay3Flow rate uses Whitebird''s final quoted input and output amounts, which already reflect the input and output fee amounts returned by the live quote API.","docs_url":"https://whitebird.io/exchanger"}'::JSONB, '{}'::JSONB, 'whitebird/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO providers (slug, operation, source_url, name, currencies, banks, exchange_methods, adapter, workflow, fee_model, guidance, source_file)
VALUES ('whitebird', 'sell', 'https://whitebird.io/', 'Whitebird Sell', ARRAY['BYN', 'EUR', 'RUB', 'USD']::TEXT[], ARRAY[]::TEXT[], ARRAY['exchanger']::TEXT[], '{"p2p":{"kind":"http_json","market":"direct_exchange","endpoint":"https://api.whitebird.io/api/v3/exchange/client/quote","method":"POST","headers":{"Accept":"application/json","Origin":"https://whitebird.io","Referer":"https://whitebird.io/exchanger"},"asset_codes":{"USDC":"USDC_ERC","USDT":"USDT_TRC"},"supported_assets":["TRX","USDT","ETH","USDC","BTC","BNB","GRAM","SOL"],"timeout_ms":5000,"max_results":null,"fiat_probe_amount":1000.0,"asset_probe_amount":1.0,"default_min_fiat":1.0,"default_max_fiat":1000000000.0,"default_available_asset":1000000000.0,"auth":null,"buy":{"query":{},"request_json":"{\"input\":{\"asset\":\"{{fiat}}\",\"type\":\"FIAT_PROVIDER\",\"amount\":\"{{amount}}\"},\"output\":{\"asset\":\"{{asset}}\",\"type\":\"CRYPTO_TRANSFER\"},\"merchantId\":\"11111111-1111-1111-1111-111111111111\"}","amount_mode":"fiat_probe","items_pointer":null,"success_pointer":null,"success_value":null,"success_missing_allowed":false,"error_pointer":null,"offer":{"ad_id_pointer":null,"fiat_pointer":"/input/asset","asset_pointer":"/output/asset","price_pointer":null,"fiat_amount_pointer":"/input/amount","asset_amount_pointer":"/output/amount","price_inverted":false,"available_asset_pointer":null,"min_fiat_pointer":null,"max_fiat_pointer":null,"payment_methods_pointer":null,"payment_method_value_pointer":null,"payment_method_fallback_pointer":null,"pay_time_limit_pointer":null,"advertiser_id_pointer":null,"advertiser_nickname_pointer":null,"advertiser_user_type_pointer":null,"merchant_conditions":[],"verified_conditions":[],"merchant_default":true,"verified_default":true,"verified_from_merchant":false,"completed_orders_pointer":null,"completion_rate_pointer":null,"positive_rate_pointer":null,"source_url_template":"https://whitebird.io/exchanger","source_url_is_exact":false,"advertiser_profile_url_template":null}},"sell":{"query":{},"request_json":"{\"input\":{\"asset\":\"{{asset}}\",\"type\":\"CRYPTO_TRANSFER\",\"amount\":\"{{amount}}\"},\"output\":{\"asset\":\"{{fiat}}\",\"type\":\"FIAT_PROVIDER\"},\"merchantId\":\"11111111-1111-1111-1111-111111111111\"}","amount_mode":"asset_probe","items_pointer":null,"success_pointer":null,"success_value":null,"success_missing_allowed":false,"error_pointer":null,"offer":{"ad_id_pointer":null,"fiat_pointer":"/output/asset","asset_pointer":"/input/asset","price_pointer":null,"fiat_amount_pointer":"/output/amount","asset_amount_pointer":"/input/amount","price_inverted":false,"available_asset_pointer":null,"min_fiat_pointer":null,"max_fiat_pointer":null,"payment_methods_pointer":null,"payment_method_value_pointer":null,"payment_method_fallback_pointer":null,"pay_time_limit_pointer":null,"advertiser_id_pointer":null,"advertiser_nickname_pointer":null,"advertiser_user_type_pointer":null,"merchant_conditions":[],"verified_conditions":[],"merchant_default":true,"verified_default":true,"verified_from_merchant":false,"completed_orders_pointer":null,"completion_rate_pointer":null,"positive_rate_pointer":null,"source_url_template":"https://whitebird.io/exchanger","source_url_is_exact":false,"advertiser_profile_url_template":null}},"offer":null,"rate_table":null},"market":null,"bestchange":null}'::JSONB, '{}'::JSONB, '{"kind":"included_in_quote","description":"The effective Pay3Flow rate uses Whitebird''s final quoted input and output amounts, which already reflect the input and output fee amounts returned by the live quote API.","docs_url":"https://whitebird.io/exchanger"}'::JSONB, '{}'::JSONB, 'whitebird/Providerfile')
ON CONFLICT (slug, operation) DO UPDATE SET
source_url = EXCLUDED.source_url,
name = EXCLUDED.name,
currencies = EXCLUDED.currencies,
banks = EXCLUDED.banks,
exchange_methods = EXCLUDED.exchange_methods,
adapter = EXCLUDED.adapter,
workflow = EXCLUDED.workflow,
fee_model = EXCLUDED.fee_model,
guidance = EXCLUDED.guidance,
source_file = EXCLUDED.source_file,
updated_at = now();


DELETE FROM banks
 WHERE picker_visible
   AND source_file LIKE '%/Providerfile'
   AND method_id NOT IN ('global-usd-cash', 'currency-amd', 'currency-rub', 'currency-usd', 'currency-byn', 'am-ameriabank-usd-account', 'am-ameriabank', 'am-idbank-usd-account', 'am-idbank', 'am-acba-usd-account', 'am-acba', 'am-ardshinbank-usd-account', 'am-ardshinbank', 'am-inecobank-usd-account', 'am-inecobank', 'am-evocabank-usd-account', 'am-evocabank', 'am-vtb-usd-account', 'am-vtb', 'ru-sberbank', 'ru-tbank', 'ru-tbank-usd-account', 'ru-alfabank', 'ru-vtb', 'ru-gazprombank', 'ru-raiffeisen', 'ru-ozon', 'by-belarusbank', 'by-belagroprombank', 'by-priorbank', 'by-belinvestbank', 'by-alfabank', 'by-belgazprombank', 'by-sberbank', 'by-belveb', 'by-mtbank', 'by-vtb', 'by-dabrabyt', 'by-technobank', 'by-btk', 'by-bnb', 'by-bsb', 'by-paritetbank', 'by-bank-reshenie', 'by-statusbank', 'by-neobank', 'by-zepterbank', 'by-brrb', 'global-usdt', 'global-usdc', 'global-btc', 'global-eth', 'global-bnb', 'global-sol', 'global-trx', 'global-ton', 'global-doge', 'global-ltc', 'global-dai', 'global-fdusd', 'global-xrp', 'global-ada', 'global-dot', 'global-link', 'global-avax', 'global-matic', 'global-bch', 'global-near', 'global-apt', 'global-atom', 'global-uni', 'global-sui');

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('global-usd-cash', 'global-usd-cash', 'Cash USD', 'both', 'GLOBAL', 'USD', '', '', '', 'enabled', 'cash', '#168451', '$', true, NULL, 'Cash', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('currency-amd', 'currency-amd', 'Armenian dram', 'both', 'AM', 'AMD', '', '', '', 'enabled', 'currency', '#6d2c91', '֏', false, NULL, 'AMD', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('currency-rub', 'currency-rub', 'Russian ruble', 'both', 'RU', 'RUB', '', '', '', 'enabled', 'currency', '#21a038', '₽', false, NULL, 'RUB', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('currency-usd', 'currency-usd', 'US dollar', 'both', 'GLOBAL', 'USD', '', '', '', 'enabled', 'currency', '#168451', '$', false, NULL, 'USD', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('currency-byn', 'currency-byn', 'Belarusian ruble', 'both', 'BY', 'BYN', '', '', '', 'enabled', 'currency', '#006b3f', 'Br', false, NULL, 'BYN', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('am-ameriabank-usd-account', 'am-ameriabank-usd-account', 'Ameriabank', 'both', 'AM', 'USD', '', '/icons/assets/ameriabank-green.png', '', 'enabled', 'bank', '#6d2c91', 'AM', true, 0, 'Ameriabank', 'ameriabank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('am-ameriabank', 'am-ameriabank', 'Ameriabank', 'both', 'AM', 'AMD', '', '/icons/assets/ameriabank-green.png', '', 'enabled', 'bank', '#6d2c91', 'AM', true, 0, 'Ameriabank', 'ameriabank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('am-idbank-usd-account', 'am-idbank-usd-account', 'IDBank', 'both', 'AM', 'USD', '', '/icons/assets/idbank.png', '', 'enabled', 'bank', '#21a366', 'ID', true, 0.75, 'IDBank', 'idbank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('am-idbank', 'am-idbank', 'IDBank', 'both', 'AM', 'AMD', '', '/icons/assets/idbank.png', '', 'enabled', 'bank', '#21a366', 'ID', true, 0.75, 'IDBank', 'idbank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('am-acba-usd-account', 'am-acba-usd-account', 'ACBA Bank', 'both', 'AM', 'USD', '', '/icons/assets/acba.png', '', 'enabled', 'bank', '#ef7f1a', 'AC', true, 0, 'ACBA', 'acba', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('am-acba', 'am-acba', 'ACBA Bank', 'both', 'AM', 'AMD', '', '/icons/assets/acba.png', '', 'enabled', 'bank', '#ef7f1a', 'AC', true, 0, 'ACBA', 'acba', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('am-ardshinbank-usd-account', 'am-ardshinbank-usd-account', 'Ardshinbank', 'both', 'AM', 'USD', '', '/icons/assets/ardshinbank.png', '', 'enabled', 'bank', '#0877bd', 'AR', false, NULL, 'Ardshinbank', 'ardshinbank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('am-ardshinbank', 'am-ardshinbank', 'Ardshinbank', 'both', 'AM', 'AMD', '', '/icons/assets/ardshinbank.png', '', 'enabled', 'bank', '#0877bd', 'AR', false, NULL, 'Ardshinbank', 'ardshinbank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('am-inecobank-usd-account', 'am-inecobank-usd-account', 'Inecobank', 'both', 'AM', 'USD', '', '/icons/assets/inecobank.png', '', 'enabled', 'bank', '#263f91', 'IN', false, 0.75, 'Inecobank', 'inecobank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('am-inecobank', 'am-inecobank', 'Inecobank', 'both', 'AM', 'AMD', '', '/icons/assets/inecobank.png', '', 'enabled', 'bank', '#263f91', 'IN', false, 0.75, 'Inecobank', 'inecobank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('am-evocabank-usd-account', 'am-evocabank-usd-account', 'Evocabank', 'both', 'AM', 'USD', '', '/icons/assets/evocabank.png', '', 'enabled', 'bank', '#111827', 'EV', false, NULL, 'Evocabank', 'evocabank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('am-evocabank', 'am-evocabank', 'Evocabank', 'both', 'AM', 'AMD', '', '/icons/assets/evocabank.png', '', 'enabled', 'bank', '#111827', 'EV', false, NULL, 'Evocabank', 'evocabank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('am-vtb-usd-account', 'am-vtb-usd-account', 'VTB Armenia', 'both', 'AM', 'USD', '', '/icons/assets/vtb.webp', '', 'enabled', 'bank', '#0a52bd', 'VT', false, NULL, 'VTB', 'vtb-armenia', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('am-vtb', 'am-vtb', 'VTB Armenia', 'both', 'AM', 'AMD', '', '/icons/assets/vtb.webp', '', 'enabled', 'bank', '#0a52bd', 'VT', false, NULL, 'VTB', 'vtb-armenia', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('ru-sberbank', 'ru-sberbank', 'Sberbank', 'both', 'RU', 'RUB', '', '/icons/assets/sberbank.webp', '', 'enabled', 'bank', '#21a038', 'SB', true, NULL, 'Sberbank', 'sberbank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('ru-tbank', 'ru-tbank', 'T-Bank', 'both', 'RU', 'RUB', '', '/icons/assets/tbank.webp', '', 'enabled', 'bank', '#ffdd2d', 'TB', true, NULL, 'T-Bank', 'tbank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('ru-tbank-usd-account', 'ru-tbank-usd-account', 'T-Bank', 'both', 'RU', 'USD', '', '/icons/assets/tbank.webp', '', 'enabled', 'bank', '#ffdd2d', 'TB', true, NULL, 'T-Bank', 'tbank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('ru-alfabank', 'ru-alfabank', 'Alfa-Bank', 'both', 'RU', 'RUB', '', '/icons/assets/alfabank.webp', '', 'enabled', 'bank', '#ef3124', 'AB', true, NULL, 'Alfa-Bank', 'alfabank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('ru-vtb', 'ru-vtb', 'VTB', 'both', 'RU', 'RUB', '', '/icons/assets/vtb.webp', '', 'enabled', 'bank', '#0a52bd', 'VT', false, NULL, 'VTB', 'vtb', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('ru-gazprombank', 'ru-gazprombank', 'Gazprombank', 'both', 'RU', 'RUB', '', '/icons/assets/gazprombank.webp', '', 'enabled', 'bank', '#006db7', 'GP', false, NULL, 'Gazprombank', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('ru-raiffeisen', 'ru-raiffeisen', 'Raiffeisenbank', 'both', 'RU', 'RUB', '', '/icons/assets/raiffeisenbank.webp', '', 'enabled', 'bank', '#ffe500', 'RB', false, NULL, 'Raiffeisenbank', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('ru-ozon', 'ru-ozon', 'Ozon Bank', 'both', 'RU', 'RUB', '', '/icons/assets/ozonbank.webp', '', 'enabled', 'bank', '#005bff', 'OZ', false, NULL, 'Ozon Bank', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('by-belarusbank', 'by-belarusbank', 'Belarusbank', 'both', 'BY', 'BYN', 'belarusbank.by', '/icons/assets/belarusbank.webp', '', 'enabled', 'bank', '#006b3f', 'BB', true, NULL, 'Belarusbank', 'belarusbank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('by-belagroprombank', 'by-belagroprombank', 'Belagroprombank', 'both', 'BY', 'BYN', 'belapb.by', '/icons/assets/belagroprombank.png', '', 'enabled', 'bank', '#f58220', 'BA', true, NULL, 'Belagroprombank', 'belagroprombank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('by-priorbank', 'by-priorbank', 'Priorbank', 'both', 'BY', 'BYN', 'priorbank.by', '/icons/assets/priorbank.jpg', '', 'enabled', 'bank', '#ffed00', 'PB', true, NULL, 'Priorbank', 'priorbank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('by-belinvestbank', 'by-belinvestbank', 'Belinvestbank', 'both', 'BY', 'BYN', 'belinvestbank.by', '/icons/assets/belinvestbank.jpg', '', 'enabled', 'bank', '#009b77', 'BI', false, NULL, 'Belinvestbank', 'belinvestbank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('by-alfabank', 'by-alfabank', 'Alfa-Bank', 'both', 'BY', 'BYN', 'alfabank.by', '/icons/assets/alfabank.webp', '', 'enabled', 'bank', '#ef3124', 'AB', true, NULL, 'Alfa-Bank Belarus', 'alfabank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('by-belgazprombank', 'by-belgazprombank', 'Belgazprombank', 'both', 'BY', 'BYN', 'belgazprombank.by', '/icons/assets/belgazprombank.png', '', 'enabled', 'bank', '#0079c2', 'BG', true, NULL, 'Belgazprombank', 'belgazprombank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('by-sberbank', 'by-sberbank', 'Sberbank', 'both', 'BY', 'BYN', 'sber-bank.by', '/icons/assets/sberbank.webp', '', 'enabled', 'bank', '#21a038', 'SB', true, NULL, 'Sber Bank Belarus', 'sberbank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('by-belveb', 'by-belveb', 'Bank BelVEB', 'both', 'BY', 'BYN', 'belveb.by', '/icons/assets/belveb.jpg', '', 'enabled', 'bank', '#006fb9', 'BV', false, NULL, 'Bank BelVEB', 'belveb', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('by-mtbank', 'by-mtbank', 'MTBank', 'both', 'BY', 'BYN', 'mtbank.by', '/icons/assets/mtbank.svg', '', 'enabled', 'bank', '#004ea3', 'MT', true, NULL, 'MTBank', 'mtbank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('by-vtb', 'by-vtb', 'VTB', 'both', 'BY', 'BYN', 'vtb-bank.by', '/icons/assets/vtb.webp', '', 'enabled', 'bank', '#0a52bd', 'VT', true, NULL, 'VTB Belarus', 'vtb', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('by-dabrabyt', 'by-dabrabyt', 'Bank Dabrabyt', 'both', 'BY', 'BYN', 'bankdabrabyt.by', '/icons/assets/dabrabyt.png', '', 'enabled', 'bank', '#00a651', 'DB', true, NULL, 'Bank Dabrabyt', 'dabrabyt', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('by-technobank', 'by-technobank', 'Technobank', 'both', 'BY', 'BYN', 'tb.by', '/icons/assets/technobank.png', '', 'enabled', 'bank', '#ed1c24', 'TB', true, NULL, 'Technobank', 'technobank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('by-btk', 'by-btk', 'BTK Bank', 'both', 'BY', 'BYN', 'btk.by', '/icons/assets/btk.png', '', 'enabled', 'bank', '#263f91', 'BT', false, NULL, 'BTK Bank', 'btk', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('by-bnb', 'by-bnb', 'BNB Bank', 'both', 'BY', 'BYN', 'bnb.by', '/icons/assets/bnb-bank.jpg', '', 'enabled', 'bank', '#e31e24', 'BN', false, NULL, 'BNB Bank', 'bnb-bank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('by-bsb', 'by-bsb', 'BSB Bank', 'both', 'BY', 'BYN', 'bsb.by', '/icons/assets/bsb.webp', '', 'enabled', 'bank', '#e30613', 'BS', false, NULL, 'BSB Bank', 'bsb-bank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('by-paritetbank', 'by-paritetbank', 'Paritetbank', 'both', 'BY', 'BYN', 'paritetbank.by', '/icons/assets/paritetbank.jpg', '', 'enabled', 'bank', '#0083ca', 'PA', true, NULL, 'Paritetbank', 'paritetbank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('by-bank-reshenie', 'by-bank-reshenie', 'Bank Reshenie', 'both', 'BY', 'BYN', 'rbank.by', '/icons/assets/bank-reshenie.webp', '', 'enabled', 'bank', '#6b2d90', 'BR', true, NULL, 'Bank Reshenie', 'bank-reshenie', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('by-statusbank', 'by-statusbank', 'StatusBank', 'both', 'BY', 'BYN', 'statusbank.by', '/icons/assets/statusbank.png', '', 'enabled', 'bank', '#003b71', 'ST', false, NULL, 'StatusBank', 'statusbank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('by-neobank', 'by-neobank', 'Neo Bank Asia', 'both', 'BY', 'BYN', 'neobank.by', '/icons/assets/neobank.png', '', 'enabled', 'bank', '#ff5a1f', 'NE', false, NULL, 'Neo Bank Asia', 'neo-bank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('by-zepterbank', 'by-zepterbank', 'Zepter Bank', 'both', 'BY', 'BYN', 'zepterbank.by', '/icons/assets/zepterbank.png', '', 'enabled', 'bank', '#8b1e3f', 'ZE', false, NULL, 'Zepter Bank', 'zepterbank', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('by-brrb', 'by-brrb', 'Bank of Growth and Business Development', 'both', 'BY', 'BYN', 'brrb.by', '/icons/assets/brrb.png', '', 'enabled', 'bank', '#005ca9', 'BR', false, NULL, 'BRRB Bank', 'brrb', TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('global-usdt', 'global-usdt', 'Tether', 'both', 'GLOBAL', 'USDT', '', '/icons/assets/usdt.webp', '', 'enabled', 'wallet', '#26a17b', 'USDT', false, NULL, 'USDT', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('global-usdc', 'global-usdc', 'USD Coin', 'both', 'GLOBAL', 'USDC', '', '/icons/assets/usdc.webp', '', 'enabled', 'wallet', '#2775ca', 'USDC', false, NULL, 'USDC', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('global-btc', 'global-btc', 'Bitcoin', 'both', 'GLOBAL', 'BTC', '', '/icons/assets/btc.webp', '', 'enabled', 'wallet', '#f7931a', 'BTC', false, NULL, 'BTC', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('global-eth', 'global-eth', 'Ethereum', 'both', 'GLOBAL', 'ETH', '', '/icons/assets/eth.webp', '', 'enabled', 'wallet', '#627eea', 'ETH', false, NULL, 'ETH', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('global-bnb', 'global-bnb', 'BNB', 'both', 'GLOBAL', 'BNB', '', '/icons/assets/bnb.webp', '', 'enabled', 'wallet', '#f3ba2f', 'BNB', false, NULL, 'BNB', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('global-sol', 'global-sol', 'Solana', 'both', 'GLOBAL', 'SOL', '', '/icons/assets/sol.webp', '', 'enabled', 'wallet', '#14f195', 'SOL', false, NULL, 'SOL', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('global-trx', 'global-trx', 'TRON', 'both', 'GLOBAL', 'TRX', '', '/icons/assets/trx.webp', '', 'enabled', 'wallet', '#ef0027', 'TRX', false, NULL, 'TRX', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('global-ton', 'global-ton', 'Toncoin', 'both', 'GLOBAL', 'TON', '', '/icons/assets/ton.webp', '', 'enabled', 'wallet', '#0098ea', 'TON', false, NULL, 'TON', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('global-doge', 'global-doge', 'Dogecoin', 'both', 'GLOBAL', 'DOGE', '', '/icons/assets/doge.webp', '', 'enabled', 'wallet', '#c2a633', 'DOGE', false, NULL, 'DOGE', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('global-ltc', 'global-ltc', 'Litecoin', 'both', 'GLOBAL', 'LTC', '', '/icons/assets/ltc.webp', '', 'enabled', 'wallet', '#345d9d', 'LTC', false, NULL, 'LTC', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('global-dai', 'global-dai', 'Dai', 'both', 'GLOBAL', 'DAI', '', '/icons/assets/dai.webp', '', 'enabled', 'wallet', '#f5ac37', 'DAI', false, NULL, 'DAI', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('global-fdusd', 'global-fdusd', 'First Digital USD', 'both', 'GLOBAL', 'FDUSD', '', '/icons/assets/fdusd.webp', '', 'enabled', 'wallet', '#1d1d1d', 'FD', false, NULL, 'FDUSD', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('global-xrp', 'global-xrp', 'XRP', 'both', 'GLOBAL', 'XRP', '', '/icons/assets/xrp.webp', '', 'enabled', 'wallet', '#23292f', 'XRP', false, NULL, 'XRP', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('global-ada', 'global-ada', 'Cardano', 'both', 'GLOBAL', 'ADA', '', '/icons/assets/ada.webp', '', 'enabled', 'wallet', '#0033ad', 'ADA', false, NULL, 'ADA', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('global-dot', 'global-dot', 'Polkadot', 'both', 'GLOBAL', 'DOT', '', '/icons/assets/dot.webp', '', 'enabled', 'wallet', '#e6007a', 'DOT', false, NULL, 'DOT', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('global-link', 'global-link', 'Chainlink', 'both', 'GLOBAL', 'LINK', '', '/icons/assets/link.webp', '', 'enabled', 'wallet', '#2a5ada', 'LINK', false, NULL, 'LINK', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('global-avax', 'global-avax', 'Avalanche', 'both', 'GLOBAL', 'AVAX', '', '/icons/assets/avax.webp', '', 'enabled', 'wallet', '#e84142', 'AVAX', false, NULL, 'AVAX', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('global-matic', 'global-matic', 'Polygon', 'both', 'GLOBAL', 'MATIC', '', '/icons/assets/matic.webp', '', 'enabled', 'wallet', '#8247e5', 'MATIC', false, NULL, 'MATIC', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('global-bch', 'global-bch', 'Bitcoin Cash', 'both', 'GLOBAL', 'BCH', '', '/icons/assets/bch.webp', '', 'enabled', 'wallet', '#8dc351', 'BCH', false, NULL, 'BCH', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('global-near', 'global-near', 'NEAR Protocol', 'both', 'GLOBAL', 'NEAR', '', '/icons/assets/near.webp', '', 'enabled', 'wallet', '#111827', 'NEAR', false, NULL, 'NEAR', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('global-apt', 'global-apt', 'Aptos', 'both', 'GLOBAL', 'APT', '', '/icons/assets/apt.webp', '', 'enabled', 'wallet', '#111827', 'APT', false, NULL, 'APT', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('global-atom', 'global-atom', 'Cosmos', 'both', 'GLOBAL', 'ATOM', '', '/icons/assets/atom.webp', '', 'enabled', 'wallet', '#2e3148', 'ATOM', false, NULL, 'ATOM', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('global-uni', 'global-uni', 'Uniswap', 'both', 'GLOBAL', 'UNI', '', '/icons/assets/uni.webp', '', 'enabled', 'wallet', '#ff007a', 'UNI', false, NULL, 'UNI', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();

INSERT INTO banks (name, method_id, display_name, role, country, currency, domain, icon_url, schemes, status, kind, color, initials, popular, bank_fee_percent, p2p_query, currency_group, picker_visible, source_file)
VALUES ('global-sui', 'global-sui', 'Sui', 'both', 'GLOBAL', 'SUI', '', '/icons/assets/sui.webp', '', 'enabled', 'wallet', '#6fbcf0', 'SUI', false, NULL, 'SUI', NULL, TRUE, 'payment-methods/Providerfile')
ON CONFLICT (name) DO UPDATE SET
method_id = EXCLUDED.method_id,
display_name = EXCLUDED.display_name,
role = EXCLUDED.role,
country = EXCLUDED.country,
currency = EXCLUDED.currency,
domain = EXCLUDED.domain,
icon_url = EXCLUDED.icon_url,
status = EXCLUDED.status,
kind = EXCLUDED.kind,
color = EXCLUDED.color,
initials = EXCLUDED.initials,
popular = EXCLUDED.popular,
bank_fee_percent = EXCLUDED.bank_fee_percent,
p2p_query = EXCLUDED.p2p_query,
currency_group = EXCLUDED.currency_group,
picker_visible = TRUE,
source_file = EXCLUDED.source_file,
updated_at = now();