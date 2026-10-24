#!/usr/bin/env python3
"""Deterministic TVM process-boundary fixture; never accesses a network."""
import json
import os
from pathlib import Path
import sys
import time

state_path = Path(__file__).with_suffix(".json")
state = json.loads(state_path.read_text())
args = sys.argv[1:]
while args and args[0] in ("-j", "--config", "--url"):
    del args[:1 if args[0] == "-j" else 2]
command = args[0]
with state_path.with_suffix(".events").open("a") as output:
    output.write(json.dumps(args) + "\n")
if state.get("fail") == command:
    print("PRIVATE_LEAK_SENTINEL", file=sys.stderr)
    print("PRIVATE_LEAK_SENTINEL")
    sys.exit(7)
if state.get("malformed") == command:
    print("malformed-backend-json")
    sys.exit(0)
if state.get("sleep") == command:
    time.sleep(10)
if os.environ.get("FAKE_TVM_BACKEND") and command in ("genaddr", "message", "deploy_message"):
    # Hybrid test: real offline TVM encoding, deterministic fake chain reads.
    executable = os.environ["FAKE_TVM_BACKEND"]
    os.execv(executable, [executable, "-j", *args])

magic = bytes.fromhex("b5ee9c72")
if command == "genaddr":
    result = {"raw_address": state["address"]}
elif command == "account":
    if "--dumpboc" in args:
        Path(args[args.index("--dumpboc") + 1]).write_bytes(magic + b"test account state")
    result = {"address": args[1], "acc_type": state.get("acc_type", "Active"), "balance": state.get("balance", "100000000000"), "code_hash": state.get("code_hash", "80d6c47c4a25543c9b397b71716f3fae1e2c5d247174c52e2c19bd896442b105")}
elif command == "query-raw":
    if args[1] == "blocks":
        result = [{"global_id": state.get("network_id", 42)}]
    elif args[1] == "transactions":
        result = state.get("receipts", [])
    else:
        raise RuntimeError("Unexpected query collection")
elif command == "run":
    method = args[3]
    if method == "getCustodians":
        result = {"custodians": [{"index": str(i), "pubkey": key} for i, key in enumerate(state["owners"])]}
    elif method == "getParameters":
        result = {"requiredTxnConfirms": str(state.get("required", 2))}
    elif method == "getTransactions":
        result = {"transactions": state.get("transactions", [])}
    else:
        raise RuntimeError("Unexpected getter")
elif command in ("message", "deploy_message"):
    output = Path(args[args.index("--output") + 1])
    output.write_bytes((b"WRONG" if state.get("bad_boc") else magic) + b"test signed message")
    result = {"MessageId": "a" * 64, "Message": "saved to file " + str(output)}
elif command == "sendfile":
    result = {"sent": True}
elif command == "decode" and state.get("codec"):
    os.execv(state["codec"], [state["codec"], "-j", *args])
else:
    raise RuntimeError("Unexpected backend command")
print(json.dumps(result))
