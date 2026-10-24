# OTC multisig

`evernode-rs otc` manages a native EVER wallet with three independent owners:
the EVER buyer, EVER seller and arbitrator. The bundled Everscale
SafeMultisig contract requires two owner signatures. Setup and import use
public keys; each participant signs with their own local private key file.

| Action | Destination | Approval |
| --- | --- | --- |
| Release | Agreed buyer address | Any two owners |
| Refund | Agreed seller address | Any two owners |

Before settlement signing, the CLI verifies the endpoint's network ID, the
pinned Safe code hash and the three-owner/two-signature quorum. Confirmation
checks the exact pending ID, recipient, amount, empty payload, bounce flag,
fixed-value send flags, signer index and existing signatures.

## Install and generate independent keys

Build and configure `evernode-rs` and the TVM backend as described in
[README.md](README.md#build-and-install). Use a persistent store on each
participant's machine:

```bash
export EVERNODE_BACKEND=/path/to/ever-cli
export EVERNODE_WALLET_DIR="$HOME/.local/share/evernode/wallets"
export EVERNODE_URL=https://YOUR_EVERSCALE_ENDPOINT

install -d -m 0700 "$HOME/.local/share/evernode/otc-keys"

# Each owner runs this independently on their own machine.
evernode-rs otc keygen \
  --output "$HOME/.local/share/evernode/otc-keys/owner.json" \
  --seed-output "$HOME/.local/share/evernode/otc-keys/owner-seed.txt"
```

Only the public key and file locations are printed. Keys and seed backups
contain plaintext secrets and use mode 0600. Their parent directory must be
owned by the current user and mode 0700. Existing files are never overwritten.
Share public keys with the other parties; keep private keys and mnemonics on
their owners' machines. Compatible existing keypair files can also be used.

The deployment operator, who can be one of the participants, generates one
additional fresh key for each trade:

```bash
evernode-rs otc keygen \
  --output "$HOME/.local/share/evernode/otc-keys/deal001-deployment.json" \
  --seed-output "$HOME/.local/share/evernode/otc-keys/deal001-deployment-seed.txt"
```

This key makes the contract address unique and signs its deployment. It is
not a fourth owner and gives no settlement authority after deployment.
Changing only a trade ID does not produce a fresh Safe address. Reusing an
address for another locally known trade is rejected.

## Agree and share the trade

Replace the example variables and addresses with independently reviewed
public keys and existing Everscale receiving addresses:

```bash
evernode-rs otc create -n deal001 \
  --buyer-pubkey "$BUYER_PUBLIC_KEY" \
  --buyer-address 0:BUYER_64_HEX_RECEIVING_ADDRESS \
  --seller-pubkey "$SELLER_PUBLIC_KEY" \
  --seller-address 0:SELLER_64_HEX_RECEIVING_ADDRESS \
  --arbitrator-pubkey "$ARBITRATOR_PUBLIC_KEY" \
  --deployment-pubkey "$DEPLOYMENT_PUBLIC_KEY" \
  --amount 100 --network-id 42

evernode-rs otc export -n deal001 --output deal001.json
```

Creation is offline and needs no private key. The returned `terms_hash`
commits to the trade ID, network, wallet, owners, recipients and exact EVER
amount. The exported document contains no signing material.

Each other participant reviews the complete document, compares the hash
directly with the counterparties, and imports it into their own store:

```bash
chmod 600 deal001.json
evernode-rs otc import --file deal001.json --terms-hash REVIEWED_TERMS_HASH
evernode-rs otc status -n deal001
```

A matching hash detects altered terms; it is not an identity proof or
counterparty signature. Records live at
`$EVERNODE_WALLET_DIR/otc/deal001/trade.json`. No private key is copied there.

## Deploy and fund

Prefund deployment gas at the displayed address. The CLI requires at least
0.9 EVER before deployment. The deployment operator then runs:

```bash
evernode-rs otc deploy -n deal001 \
  --signer "$HOME/.local/share/evernode/otc-keys/deal001-deployment.json" \
  --terms-hash REVIEWED_TERMS_HASH --yes

evernode-rs otc reconcile -n deal001
evernode-rs otc verify -n deal001
```

Every participant runs `otc verify` before funding the OTC amount. The address
alone does not commit to the constructor's owners; verification checks the
actual deployed code, owners, threshold and network. The seller then funds
the native EVER obligation plus a separate network-fee reserve. The buyer
and seller receiving wallets must be deployed because settlement uses
bounce=true. The amount is the outgoing native message value, rather than
a guaranteed net receiving-wallet balance increase.

Network ID defaults to 42 and is checked against a masterchain block's
`global_id`. Signing defaults to that ID for replay protection. An explicitly
supplied signature ID must match it or be `online`.

## Release or refund

After verifying the agreed counter-leg payment, one owner proposes release
from their own machine:

```bash
evernode-rs otc propose -n deal001 --action release \
  --signer "$HOME/.local/share/evernode/otc-keys/owner.json" \
  --terms-hash REVIEWED_TERMS_HASH --yes

evernode-rs otc reconcile -n deal001
```

A different owner reads the pending transaction ID and confirms it on their
own machine:

```bash
evernode-rs otc verify -n deal001

evernode-rs otc confirm -n deal001 --action release \
  --transaction-id PENDING_TRANSACTION_ID \
  --signer "$HOME/.local/share/evernode/otc-keys/owner.json" \
  --terms-hash REVIEWED_TERMS_HASH --yes

evernode-rs otc reconcile -n deal001
```

For an agreed refund, use `--action refund` in both commands. Arbitrator
participation uses the same commands with the arbitrator's independently
owned key. The CLI supplies the amount and destination from the reviewed
terms and rejects a substituted pending transfer.

Without `--yes`, these commands save a signed raw BOC without broadcasting.
`--output FILE` selects its location. Network preflight reads still run. An
existing pending transaction blocks another proposal.

Before broadcasting, the CLI saves the exact message identity. Backend
success, failure or timeout leaves this record in place until `otc reconcile`
finds its unique finalized receipt on the expected wallet and network.
Missing or ambiguous evidence keeps further local signing blocked.
Reconciliation archives the receipt and reports `accepted` or `failed` for
the multisig call; it does not report completed OTC settlement or verified
delivery to the buyer/seller.

## Scope and verification

This is an ordinary Safe multisig. The contract enforces the 2-of-3 threshold;
the OTC amount and destination checks belong to this CLI. Two owners using
another tool can authorize other transfers. There is no automatic timeout
refund, single-use escrow rule, token vault or atomic cross-chain swap.
Counter-leg payment and receiving-chain delivery require separate verification.
Pay3Flow can import these public agreements and sign through EVER Wallet in
its [native EVER workspace](../../../docs/otc-everscale-multisig.md). These
commands do not mark desk service trades complete or move Ethereum Safe funds.

```bash
cargo test --locked --manifest-path backend/Cargo.toml -p evernode-multisig --test otc -- --nocapture

EVERNODE_TEST_BACKEND=/path/to/ever-cli \
  cargo test --locked --manifest-path backend/Cargo.toml -p evernode-multisig --test otc real_tvm -- --nocapture
```

Tests cover all ordered owner pairs for release and refund, isolated stores,
public-only setup/import, key protection, changed terms, substituted pending
transactions, code/network/quorum mismatches and unknown-broadcast recovery.
The optional codec test uses ever-cli 0.44.0 for real Safe deployment, release,
refund and confirmation encoding/decoding with simulated chain reads. No live
funds are sent.
