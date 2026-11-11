# B2B launch remains separate from Georgian support

Production previously returned 404 for `/api/b2b/orders`; Georgian support must
not implicitly launch this unrelated, unfinished payment API. `b2b_enabled`
defaults to false. The HTTP gate rejects merchant/payer routes before business
handlers, hides their OpenAPI operations, and prevents the worker from starting.
It is a non-secret TOML setting in the mounted `pay3flow-config` configuration;
older configurations that omit it also keep the API off.

The following round-4 findings are deferred behind that disabled launch gate.
Revisit all of them before enabling B2B or accepting the first real transfer:

- Bound settlement-observation capacity and separate live/test work. Fair
  rotation avoids starvation but an unbounded abandoned queue increases latency.
- Reconcile the price-expiry state, event contract and settlement observation.
  An expired price must never make an in-flight deposit or refund invisible.
- Remove or implement the unreachable re-quote capability. Replacing a funded
  address requires observing every issued attempt; the current single pointer
  cannot safely provide that capability.
- Require encrypted webhook delivery and define any compatibility policy before
  accepting merchant endpoints. Private-address validation alone does not provide
  transport confidentiality.

The shared catalog-refresh finding was fixed for the active Georgian release:
failed empty refreshes preserve a previously usable snapshot, and every registered
provider is periodically retried after startup failure. The partial index uses a
new name so environments with an earlier prototype index receive the new one.

The SQL integration harness is supplemental to Cargo tests and is run explicitly
for a release; it refuses a database container without the fixture label and test
name. Hosted Docker-CI wiring is deferred until that environment is provisioned.
