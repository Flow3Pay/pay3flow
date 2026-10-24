# evernode-multisig in Pay3Flow

This crate includes the Rust `evernode-rs` CLI and the public-key-only
`Trade::browser_review` adapter used by Pay3Flow's native EVER OTC workspace.
The backend calls the adapter to verify deployed contracts and prepare unsigned
ABI calls. It never invokes this crate's key generation, signing or broadcast
operations through HTTP. The standalone CLI runs on each owner's machine.

## Build and install

From the Pay3Flow checkout:

```sh
cargo build --release --manifest-path backend/Cargo.toml -p evernode-multisig --bin evernode-rs
cargo install --path backend/crates/evernode-multisig --locked
evernode-rs --help
evernode-rs otc --help
```

Install a compatible `ever-cli` TVM backend separately and set `EVERNODE_BACKEND`
to its executable path. Network operations require `EVERNODE_URL` explicitly.
`ever-cli` 0.44.0 was used for offline ABI/signature compatibility checks.
Rust 1.85+ and a Unix system are required. The Pay3Flow backend uses its own
configured executable/endpoints; it does not inherit participants' CLI stores.

See [the CLI OTC guide](OTC.md) for creating independent owners, allocating a
fresh deployment key, importing/exporting public agreements and settling through
the CLI. See [Pay3Flow integration](../../../docs/otc-everscale-multisig.md)
for browser signing and server setup.

## Source and verification

This is the Rust port developed from
[`somefriday/evernode_cli`](https://github.com/somefriday/evernode_cli), Python
source snapshot `8ce35357d5f537a4e147cf806e3479f3e217ae5e`. ABI/TVC bytes are
bundled from that repository's `vendor/ever-validator/contracts/EverX` tree.
The library/CLI retain [GPL-3.0-only](LICENSE); contract assets retain their
[upstream MIT notice](assets/LICENSE). The Pay3Flow adapter adds unsigned
browser-call preparation; existing standalone commands remain available.

The included tests retain native key derivation vectors, process-boundary CLI
tests, independent-key OTC checks, pinned asset digests and the frozen 1,226-case
Python parity corpus. The original Python runtime is not required in Pay3Flow.
The original port's scope is recorded in [PARITY-CONTRACT.md](PARITY-CONTRACT.md).

```sh
cargo test --manifest-path backend/Cargo.toml -p evernode-multisig
cargo clippy --manifest-path backend/Cargo.toml -p evernode-multisig --all-targets -- -D warnings
```
