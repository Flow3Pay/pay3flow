#!/usr/bin/env python3
"""Read-only pair/amount coverage audit. Never creates orders or executes swaps."""

import argparse
from collections import defaultdict
import concurrent.futures
import datetime
import json
from pathlib import Path
import time
import urllib.parse
import urllib.request


ASSETS = [
    ("USDC", "ethereum", 100), ("AVAX", "avalanche-c", 10),
    ("BCH", "bitcoin-cash", 0.3), ("BTC", "bitcoin", 0.001),
    ("10SET", "bnb-smart-chain", 10000), ("AAVE", "ethereum", 1),
    ("ACNON", "bnb-smart-chain", 1), ("ADA", "cardano", 500),
    ("ETH", "ethereum", 0.05),
]
CONTROLS = [
    ("USDC", "ethereum", "ETH", "ethereum", 100),
    ("BTC", "bitcoin", "USDC", "ethereum", 0.001),
    ("AVAX", "avalanche-c", "USDC", "ethereum", 10),
    ("BCH", "bitcoin-cash", "USDT", "tron", 0.3),
    ("AAVE", "ethereum", "USDC", "ethereum", 1),
    ("ADA", "cardano", "USDC", "ethereum", 500),
    ("10SET", "bnb-smart-chain", "USDC", "ethereum", 10000),
    ("ACNON", "bnb-smart-chain", "USDC", "ethereum", 1),
    ("ETH", "ethereum", "BTC", "bitcoin", 0.05),
]
CATALOGS = {
    "cow": "https://files.cow.fi/tokens/CowSwap.json",
    "near": "https://1click.chaindefuser.com/v0/tokens",
    "symbiosis": "https://api.symbiosis.finance/crosschain/v2/tokens",
}


def fetch(url):
    request = urllib.request.Request(url, headers={"User-Agent": "Pay3Flow-Coverage-Audit/1.0"})
    with urllib.request.urlopen(request, timeout=75) as response:
        return json.load(response)


def probe(base, pair, multiplier, sources):
    source, source_network, target, target_network, amount = pair
    query = dict(source_fiat=source, source_network=source_network,
                 target_fiat=target, target_network=target_network,
                 source_amount=amount * multiplier, limit=100, allow_cross_venue="true")
    if sources:
        query["sources"] = sources
    started = time.monotonic()
    result = {"query": query}
    try:
        response = fetch(base + "/api/p2p/routes?" + urllib.parse.urlencode(query))
        # Do not persist signed execution tokens or visitor identifiers.
        result.update({key: response.get(key) for key in
                       ("routes_found", "routes_exhaustive", "provider_statuses", "assets_searched")})
        result["routes"] = [{key: route.get(key) for key in
                             ("route_kind", "route_provider", "market_path", "source_amount",
                              "target_amount", "route_path", "warnings", "cycle_legs")}
                            for route in response.get("routes", [])]
    except Exception as error:
        result["error"] = str(error)
    result["elapsed_seconds"] = round(time.monotonic() - started, 3)
    return result


def audit_catalogs():
    evidence = {"observed_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                "scope": "Structural audit of every catalog entry; metadata does not prove pair liquidity",
                "catalogs": {}}
    for name, url in CATALOGS.items():
        try:
            payload = fetch(url)
            tokens = payload["tokens"] if isinstance(payload, dict) else payload
            groups = defaultdict(set)
            for token in tokens:
                address = token.get("address", token.get("contractAddress", "native"))
                if address.startswith("0x"):
                    address = address.lower()
                key = (str(token.get("chainId", token.get("blockchain"))), token.get("symbol", "").upper())
                groups[key].add((address, token.get("decimals")))
            collisions = [{"network": key[0], "symbol": key[1],
                           "identities": [{"address": address, "decimals": decimals}
                                          for address, decimals in identities]}
                          for key, identities in groups.items() if len(identities) > 1]
            evidence["catalogs"][name] = {
                "url": url, "entries": len(tokens), "asset_network_groups": len(groups),
                "ordered_group_pairs_excluding_self": len(groups) * (len(groups) - 1),
                "ticker_collisions": collisions,
                "ever_entries": [token for token in tokens if token.get("symbol", "").upper() in ("EVER", "WEVER")],
                "requested_tokens": [token for token in tokens if token.get("symbol", "").upper() in ("10SET", "ACNON")],
            }
        except Exception as error:
            evidence["catalogs"][name] = {"url": url, "error": str(error)}
    return evidence


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-url", default="http://localhost:8080")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--multipliers", default="1,10", help="Comma-separated input size multipliers")
    parser.add_argument("--workers", type=int, default=2)
    parser.add_argument("--sources", default="")
    parser.add_argument("--catalogs-only", action="store_true", help="Inspect every entry in public token catalogs without requesting pair quotes")
    args = parser.parse_args()
    if args.catalogs_only:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(audit_catalogs(), ensure_ascii=False, indent=2) + "\n")
        return
    base = args.base_url.rstrip("/")
    pairs = [(source, network, "EVER", target, amount)
             for source, network, amount in ASSETS for target in ("ethereum", "everscale")] + CONTROLS
    multipliers = [float(value) for value in args.multipliers.split(",")]
    if not multipliers or any(value <= 0 for value in multipliers):
        parser.error("multipliers must be positive")
    evidence = {"observed_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                "base_url": base, "scope": "Bounded pair/amount sample; not exhaustive market coverage",
                "results": []}
    for endpoint in ("networks", "market-prices"):
        try:
            evidence[endpoint] = fetch(base + "/api/" + endpoint)
        except Exception as error:
            evidence[endpoint] = {"error": str(error)}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, min(args.workers, 4))) as pool:
        jobs = [pool.submit(probe, base, pair, multiplier, args.sources)
                for pair in pairs for multiplier in multipliers]
        for job in concurrent.futures.as_completed(jobs):
            result = job.result()
            evidence["results"].append(result)
            args.output.write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n")
            query = result["query"]
            print(f"{query['source_fiat']}@{query['source_network']} -> "
                  f"{query['target_fiat']}@{query['target_network']} "
                  f"amount={query['source_amount']:g}: {result.get('routes_found', result.get('error'))}", flush=True)


if __name__ == "__main__":
    main()
