# Self-hosted EVM wallet

Pay3Flow now supports an optional self-hosted EVM wallet. The encrypted
keystore is stored on the Pay3Flow host; Circle, Coinbase and exchange custody
are not used.

## Create a wallet

Run this from `backend/` and keep the password out of shell history in real
deployments:

```bash
export WALLET_KEYSTORE_PASSWORD='change-this-password'
cargo run --bin wallet-init -- ./data/wallet
```

The command prints the wallet address and creates an encrypted keystore file.
Back up that file and its password separately. Losing either means losing
access to funds.

## Configure the backend

```env
WALLET_KEYSTORE_PATH=./data/wallet/UTC--...json
WALLET_KEYSTORE_PASSWORD=change-this-password
WALLET_RPC_URL=http://127.0.0.1:8545
```

Leave `WALLET_KEYSTORE_PATH` empty to keep wallet functionality disabled.

## Admin API

All wallet endpoints require `Authorization: Bearer $ADMIN_TOKEN`:

```text
GET  /api/admin/wallet
GET  /api/admin/wallet/balance
POST /api/admin/wallet/transfer
POST /api/admin/wallet/token-transfer
```

Native transfer:

```json
{"to":"0x...","value_wei":"1000000000000"}
```

ERC-20 transfer:

```json
{"token_contract":"0x...","to":"0x...","amount":"1000000"}
```

The token amount is in the token's smallest units. The current endpoint is
admin-only and is not connected automatically to exchange orders yet.

## Security boundary

The frontend never receives the private key. The backend decrypts the keystore
only when it needs the signer, signs locally, and sends the signed transaction
to the configured RPC. For production, move signing into a separate host or
HSM before enabling automatic payouts.
