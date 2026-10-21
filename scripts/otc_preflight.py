#!/usr/bin/env python3
"""Read-only checks of the deployed OTC configuration; never enables trading.

No credentials, wallet addresses, RPC URLs or private configuration are included
in the report. With --ssh, chain requests run on the production host.
"""

import argparse
import concurrent.futures
import datetime
import json
import re
import shlex
import subprocess
import sys
import urllib.request


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--url", default="https://pay3flow.lefine.pro")
    parser.add_argument("--ssh", help="SSH destination for private configuration checks")
    parser.add_argument("--namespace", default="pay3flow")
    parser.add_argument("--output", help="Write the redacted JSON report to this file")
    args = parser.parse_args()
    if not re.fullmatch(r"[a-z0-9](?:[-a-z0-9]*[a-z0-9])?", args.namespace):
        parser.error("invalid Kubernetes namespace")
    if args.ssh and (args.ssh.startswith("-") or any(c.isspace() for c in args.ssh)):
        parser.error("invalid SSH destination")
    checks = []
    private_inspected = False

    def check(name, passed, detail):
        checks.append({"check": name, "passed": bool(passed), "detail": detail})

    def remote(command):
        result = subprocess.run(
            ["ssh", "-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=yes",
             "-o", "ConnectTimeout=10", args.ssh, command],
            capture_output=True, timeout=45, check=True,
        )
        return result.stdout.decode()

    def kube(*parts):
        return remote(shlex.join(["k3s", "kubectl", "-n", args.namespace, *parts]))

    def public(path):
        with urllib.request.urlopen(args.url.rstrip("/") + path, timeout=20) as response:
            return response.read()

    try:
        check("health", public("/health").strip() == b"ok", "Public backend health")
        cfg = json.loads(public("/api/otc/config"))
        check("live_mode", cfg.get("demo") is False, "Simulation must be disabled")
        check("trading_enabled", cfg.get("enabled") is True, "Configured trading switch")
        check("route_available", cfg.get("available") is True, "Route and observers ready")
        for chain in ("ethereum", "everscale"):
            check(f"{chain}_login", chain in cfg.get("wallet_login_chains", []),
                  "Wallet verification configured")
        listings = json.loads(public("/api/otc/listings"))
        check("public_proposals", isinstance(listings, list) and len(listings) >= 2,
              "Both desk directions need current structured Proposals")
    except Exception as error:
        check("public_api", False, type(error).__name__)

    if args.ssh:
        try:
            deployment = json.loads(kube("get", "deployment", "pay3flow-backend", "-o", "json"))
            container = next(c for c in deployment["spec"]["template"]["spec"]["containers"]
                             if c["name"] == "backend")
            config_path = next(e["value"] for e in container["env"]
                               if e["name"] == "OTC_CONFIG_FILE")
            mount = next(m for m in container["volumeMounts"] if m["mountPath"] == config_path)
            volume = next(v for v in deployment["spec"]["template"]["spec"]["volumes"]
                          if v["name"] == mount["name"])
            configmap = json.loads(kube("get", "configmap", volume["configMap"]["name"], "-o", "json"))
            private = json.loads(configmap["data"][mount["subPath"]])
            private_inspected = True
            check("backend_ready", deployment.get("status", {}).get("availableReplicas", 0) > 0,
                  "Backend has an available replica")
            required = ("desk_actor", "matcher_actor", "desk_name", "ever_wallet", "usdt_wallet",
                        "ever_resource", "usdt_resource", "support_owner", "incident_owner",
                        "incident_backup", "refund_policy", "net_delivery_policy",
                        "operating_requirements_evidence", "live_gate_evidence",
                        "alert_webhook", "heartbeat_url")
            missing = [name for name in required if not str(private.get(name) or "").strip()]
            check("desk_configuration", not missing, {"missing_fields": missing})
            for chain, field in (("ethereum", "ethereum_rpc"), ("everscale", "everscale_graphql")):
                endpoints = private.get(field, [])
                check(f"{chain}_providers", len(endpoints) >= 2 and endpoints[0] != endpoints[1]
                      and all(isinstance(u, str) and u.startswith("https://") for u in endpoints),
                      "Two distinct HTTPS providers required; operator independence needs review")
            check("everwallet_allowlist", bool(private.get("everwallet_code_hashes")),
                  "Approved deployed single-key wallet code hashes required")

            # Inspect lengths on the host: secret values never enter this process/report.
            lengths = json.loads(remote(
                shlex.join(["k3s", "kubectl", "-n", args.namespace, "get", "secret",
                            "pay3flow-secrets", "-o", "json"])
                + " | jq '{jwt: (.data.JWT_SECRET | @base64d | length), "
                  "encryption: (.data.SECRETS_KEY | @base64d | length)}'"))
            check("persistent_secrets", all(lengths.get(k, 0) >= 32 for k in ("jwt", "encryption")),
                  "JWT and encryption secrets must each contain at least 32 characters")

            sql = """SELECT json_build_object(
              'paused',(SELECT paused FROM otc_controls WHERE singleton),
              'signing_paused',(SELECT signing_paused FROM otc_controls WHERE singleton),
              'observers',(SELECT json_agg(json_build_object('chain',chain,'healthy',
                NOT paused AND last_scan IS NOT NULL AND last_scan>now()-interval '60 seconds'))
                FROM otc_observers),
              'trades',(SELECT count(*) FROM otc_trades),
              'unresolved_attempts',(SELECT count(*) FROM otc_attempts
                WHERE state IN ('prepared','unknown','submitted','confirming')));"""
            psql = 'exec psql -X -v ON_ERROR_STOP=1 -U "$POSTGRES_USER" -d "$POSTGRES_DB" -Atc ' + shlex.quote(sql)
            state = json.loads(kube("exec", "pay3flow-postgres-0", "--", "sh", "-c", psql))
            check("route_resumed", state.get("paused") is False, "Persistent route pause")
            check("signing_resumed", state.get("signing_paused") is False, "Persistent signing pause")
            observers = {o["chain"]: o["healthy"] for o in state.get("observers") or []}
            for chain in ("ethereum", "everscale"):
                check(f"{chain}_observer", observers.get(chain) is True, "Successful scan within 60 seconds")
            check("existing_obligations_inspected", True,
                  {"trades": state["trades"], "unresolved_attempts": state["unresolved_attempts"]})

            def rpc_probe(item):
                index, url = item
                payload = json.dumps({"jsonrpc": "2.0", "id": 1,
                                      "method": "eth_chainId", "params": []})
                try:
                    result = json.loads(remote(shlex.join([
                        "curl", "--fail", "--silent", "--show-error", "--max-time", "15",
                        "--header", "Content-Type: application/json", "--data", payload, url])))
                    return index, result.get("result") == "0x1", "Ethereum mainnet chain ID"
                except Exception as error:
                    return index, False, type(error).__name__

            with concurrent.futures.ThreadPoolExecutor(max_workers=2) as executor:
                for index, passed, detail in executor.map(rpc_probe, enumerate(private.get("ethereum_rpc", [])[:2], 1)):
                    check(f"ethereum_rpc_{index}_mainnet", passed, detail)
        except Exception as error:
            # Subprocess errors can contain command lines with private RPC URLs.
            check("private_inspection", False, type(error).__name__)

    report = {
        "checked_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "url": args.url, "private_configuration_inspected": private_inspected,
        "automated_checks_passed": bool(checks) and all(c["passed"] for c in checks),
        "checks": checks,
        "limits": "Read-only preflight. Passing checks cannot attest wallet ownership, provider independence, live settlement, relay delivery or operator evidence.",
    }
    output = json.dumps(report, indent=2, ensure_ascii=False) + "\n"
    if args.output:
        with open(args.output, "w", encoding="utf-8") as target:
            target.write(output)
    sys.stdout.write(output)
    return 0 if report["automated_checks_passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
