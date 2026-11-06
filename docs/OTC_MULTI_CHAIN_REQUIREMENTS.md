# OTC multi-chain stablecoin requirements

## Purpose

This document records the requested OTC product goals, decisions, and the
verified implementation status. It remains the working requirements baseline
for extending OTC from its Ethereum-centric USDT flow to multi-chain USDT and
USDC flows; implemented support is narrower than the full target.

The original request was to block Sepolia for OTC offers, add USDC alongside
USDT, and provide a searchable token picker. The scope was subsequently
expanded: supported production networks must remain selectable, both stablecoins
must be available on those networks when supported, and wallet connections
must cover the networks required to pay and receive. Cross-network settlement
is expected to use NEAR Intents rather than assuming that a wallet address
identifies a network.

## Goals

1. Let users create and take OTC offers involving **USDT** and **USDC**.
2. Keep network selection visible and explicit for both tokens. Do not silently
   infer a network from an address or silently switch a user's selected
   network.
3. Make production networks available where the selected token is actually
   supported by the configured settlement provider and the full wallet/payment
   flow is implemented.
4. Exclude test networks (including Sepolia) from OTC offer creation and
   settlement.
5. Support cross-network settlement using NEAR Intents, so the input and output
   networks may differ. An example is paying USDT from TRON and receiving USDT
   on Solana.
6. Add the wallet connections needed to execute the selected network's
   transaction. A visible network option must not imply a wallet or transaction
   capability that is not present.
7. Preserve existing OTC behavior for unaffected offers and retain backward
   compatibility for older USDT offers that do not store a stablecoin field.

## Decisions and conversation history

### Stablecoins and networks

- USDT remains available.
- USDC is added as a first-class OTC stablecoin, not merely as a display option.
- The token-selection UI should be searchable and allow selection between
  USDT and USDC.
- The earlier Ethereum/ERC-20-only interpretation was superseded. Network
  selection for USDT and USDC must **not** be removed.
- Do not limit the product to ERC-20 or hide the network choice in OTC.
- Use production networks only. Test networks must not be selectable or
  accepted by the backend.
- The production-network examples discussed were Ethereum, BNB Smart Chain,
  TRON, Solana, NEAR, Optimism, Sei, Sui, TON, Polygon PoS, Base,
  Avalanche C-Chain, and Aptos. This list is a target to verify against live
  provider token support and implemented execution—not permission to show
  unsupported routes as available.
- Everscale may remain available for the existing EVER side of OTC. The request
  to hide network labels concerned stablecoin network selection and was later
  superseded for USDT/USDC by the explicit instruction to retain network
  selection.

### Network identity and swaps

- A user chooses a network for each stablecoin operation; an EVM address such
  as `0x…` does not identify whether the intended network is Ethereum, Base,
  Polygon, or another EVM chain.
- The server must persist and validate the selected network as part of the
  offer/trade terms. It must not choose a different network on the user's behalf.
- Cross-network conversion is intended to use NEAR Intents. The source wallet
  sends the chosen input asset on the source network; the route pays the chosen
  output asset to the destination network/address.
- NEAR Intents availability is not universal: only assets and routes actually
  present in its current token catalog and executable API may be offered.
- The phrase “confidential swap” was mentioned, but no confidentiality model,
  privacy guarantees, or separate confidential-swap protocol was specified.
  Do not claim confidentiality unless a concrete supported flow and its
  guarantees are established.

### Wallet connections

- The current connection dialog shows Ethereum/EVM, NEAR, TRON, and Everscale.
- The request is to add the connections needed for the expanded set of
  stablecoin networks, while retaining the searchable wallet dialog.
- EVM networks may share an EVM wallet family, but the selected chain still
  needs an explicit chain ID/network check before signing.
- Non-EVM networks need their own compatible wallet adapter, address
  validation, transaction signing/submission, and transaction-status handling.
- A wallet option must not be presented as complete merely because the wallet
  can connect: the corresponding OTC payment and refund path must also work.

## Expected user journeys

### Create an offer

1. The user opens the OTC offer form.
2. The user searches for and selects USDT or USDC.
3. The user selects an available production network for that token.
4. The user enters offer terms and confirms the selected asset/network.
5. The backend validates token, network, amount, and settlement support before
   publishing the offer.
6. The immutable accepted offer/trade terms include the exact stablecoin and
   network; no testnet can pass validation.

### Take an offer and pay across networks

1. The taker reviews the token and destination network in the offer.
2. The taker chooses a supported input network and connects the corresponding
   wallet if it differs from the destination network.
3. The backend requests an executable NEAR Intents quote using the exact input
   asset, output asset, source address, recipient address, and refund address.
4. The UI shows the route, network, amounts, fees/slippage, expiry, deposit
   instructions, and any memo/tag before the user signs.
5. The taker signs and submits the source-chain transaction from the matching
   wallet.
6. The backend submits/observes the deposit through the provider and advances
   OTC only after provider status establishes the required settlement outcome.
7. Failures, expiry, and refunds remain explicit states; a timeout or unknown
   provider status must not be treated as successful payment.

### Review and complete a trade

- Every offer, deal, history row, receipt, and transfer confirmation must show
  the selected stablecoin and relevant network(s).
- Cross-network source and destination networks must be shown separately.
- Transaction confirmation must validate the correct chain, token, amount,
  recipient, and provider execution state.
- Existing EVER settlement behavior remains explicit and must not be confused
  with USDT/USDC network selection.

## Backend and data requirements

- Treat `(stablecoin, network)` as an asset identity. A symbol alone is
  insufficient.
- Reuse the existing NEAR Intents token catalog and quote/status APIs where
  appropriate; do not duplicate or hard-code a stale catalog when the provider
  can supply current support.
- Validate the selected asset against the production catalog server-side for
  offer publication and executable payment preparation.
- Keep user-provided network/address/asset values bound to immutable accepted
  terms. Do not let a later UI change mutate terms for an accepted trade.
- Validate addresses using the selected network's address rules. For
  networks that require a memo/tag, require it and carry it through the quote,
  transaction, and status flow.
- Persist enough execution information to resume polling after a process
  restart: provider/deposit identifier, chain, asset, transaction reference,
  deposit memo, quote expiry, status, and refund outcome as applicable.
- Do not mark an OTC payment funded based solely on a client-reported
  transaction hash or a wallet broadcast response.
- Configure and validate per-network RPC/observer/provider capabilities rather
  than assuming one Ethereum RPC or one EVM address applies to every network.
- Keep testnet identifiers out of the production token catalog exposed to OTC,
  and independently reject them in backend validation.
- Preserve the compatibility rule that a legacy offer without `stablecoin`
  means USDT.

### Fees

- The OTC transaction fee is charged/denominated in **USDT**. The conversation
  explicitly settled the fee currency as USDT, including the intent to
  consolidate transaction value into USDT.
- For a USDC-denominated offer, the currently agreed calculation uses nominal
  parity (1 USDC = 1 USDT) rather than a live conversion quote. The immutable
  fee policy records this assumption; it is not an assertion that the market
  price is exactly equal.
- Do not charge the fee in the user's source token merely because that is the
  token or network used to fund a cross-chain route. If a route is funded with
  USDC or another supported asset, fee accounting and collection still need to
  resolve to USDT.
- The P2P fee policy names the stablecoin sender as payer and accrues the USDT
  obligation after the stablecoin payment is verified. The seller's principal
  payout is `gross - fee`; the fee payer separately signs a transfer of the
  exact fee to the configured recipient in the selected receiving network.
  Ethereum USDT uses a direct transfer when the payment source is already
  Ethereum USDT; other supported source/destination combinations use an
  exact-output route. The fee is marked collected only after the backend
  verifies the exact destination amount, and EVER payout stays blocked until
  then.
- A routed fee is a separate wallet-approved transaction. Its source amount
  and route fee are shown before signing; route costs are not counted as service
  fee or silently added to the service-fee amount.
- Show the USDT-denominated fee and its calculation/collection effect in the
  quote and immutable trade terms before the user commits.
- Fee destinations were supplied for TRON, NEAR, Bitcoin, Ethereum, Solana,
  Avalanche C-Chain, Sei, Polygon PoS, TON, BNB Smart Chain, Arbitrum One,
  Base, and Aptos. The address values belong in the deployment environment,
  not in this requirements document or tests.

- Fee destinations are enabled only when the configured chain/address pair is
  valid, six-decimal USDT exists in the provider catalog, and the required
  payment and fee routes are available. An address alone is not sufficient.
  Networks without an executable wallet path or a routable USDT token are
  excluded even if an address is configured. In particular, the configured
  Bitcoin address is not itself a USDT receiving address and Bitcoin is not
  currently an OTC settlement destination.
- Keep operational fee destinations in environment configuration, not
  hard-coded into frontend code or public application defaults.

## Frontend and wallet requirements

- Keep the token picker searchable and add an explicit network selector for
  each stablecoin.
- Populate or constrain available networks based on verified token/route
  support; indicate unavailable routes rather than allowing a dead-end choice.
- Show network name and token contract/asset identity where needed to
  disambiguate look-alike tokens.
- The wallet dialog search should find network and wallet names and continue
  to show connected address/status and disconnect actions.
- For EVM wallet use, request/switch to the selected chain and verify the
  connected account and chain again immediately before signing.
- For Solana, TRON, NEAR, Sui, TON, Aptos, and any other non-EVM network shown,
  implement a compatible provider connection and the transaction flow required
  by NEAR Intents. Address-only connection is insufficient.
- Keep clear errors for a missing wallet, wrong network, changed account,
  rejected signature, insufficient funds, expired quote, unsupported token,
  or provider outage.

## Security, reliability, and UX constraints

- Never silently fall back to another stablecoin, network, wallet, recipient,
  or refund address.
- Never display testnet offers as production offers.
- Clearly identify the source chain and destination chain before signing.
- Require the server to independently validate all execution-critical data;
  UI checks are supplemental.
- Protect against replayed submissions and duplicate deposits using the
  existing idempotency and unique-attempt conventions.
- Persist unknown/in-progress execution states and provide a safe retry/status
  check without creating a second deposit.
- A provider outage must be surfaced as pending/unavailable, not as success or
  as an invalid payment.
- Quote expiry and any minimum-output/slippage behavior must be visible before
  signature.

## Completion criteria

- USDT and USDC can each be selected through a searchable picker.
- A user can select a production network for the selected token, and the
  backend stores and validates that choice.
- Sepolia and all other test networks are rejected by both UI availability and
  backend validation.
- Every visible network has a working compatible wallet connection, signing,
  submission, confirmation, and failure/refund path for the OTC flow.
- At least one supported cross-network USDT route and one supported
  cross-network USDC route can complete through NEAR Intents end-to-end.
- Source and destination networks are independently represented in offer,
  quote, execution, status, and UI records.
- Provider failure, expiry, wrong-chain signing, wrong asset, and duplicate
  submission cases have focused automated coverage.
- Legacy USDT offers without a stablecoin field remain readable and behave as
  USDT.
- The existing EVER leg and unrelated wallet connections continue to work.

## Implementation status (2026-11-06)

The implementation below is on `master`. It is a working subset of the
product goal, not completion of every network in the requested target list.
Offer and trade APIs constrain selectable assets to configured wallet
executors, exact provider token identities, fee destinations, and available
payment/fee routes. The interface does not claim support just because a token
or address appears in an external catalog.

Implemented:

- Public offers support USDT and USDC, explicit receiving networks, and exact
  catalog asset IDs. When taking a buy offer, the taker can select a supported
  receiving network and enter the destination address; accepted destination
  terms are bound to the trade. Legacy offers without a stablecoin still mean
  USDT.
- Payment sources are selected from the live NEAR Intents token catalog and
  checked against exact asset IDs. Cross-network principal settlement uses an
  executable exact-output quote; instructions include source amount, expected
  output, route fee, expiry, deposit address, and memo when provided.
- Route attempts are idempotent and persisted for restart-safe polling.
  Unknown provider state remains pending; only an exact expected output
  advances the trade, and a confirmed refund permits retry.
- Same-token direct settlement is supported on Ethereum, Base, Arbitrum One,
  Optimism, BNB Smart Chain, Gnosis, Polygon PoS, and Avalanche C-Chain when
  the exact token identities match. Ethereum uses its configured peer RPC pair;
  every other EVM network requires two independently operated HTTPS RPCs in
  `p2p_evm_rpcs`. The backend validates chain IDs and observes transfers on
  the booked network.
- The fee policy is recorded in USDT; USDC deals use the agreed nominal 1:1
  fee basis, persisted separately for audit. The seller receives principal net
  of the fee. The payer separately signs an exact USDT fee transfer or
  exact-output route. A routed fee amount and provider fee are preflighted and
  shown before signing the principal payment. The fee is accrued after payment
  verification and EVER payout is blocked until fee collection is verified.
- Database migrations include the P2P fee basis, fee collection, and supported
  EVM chain names.
- Providerfile validation/tests, OTC unit tests, frontend type checks, and
  frontend unit tests passed for the merged implementation. On 2026-11-06,
  OTC unit tests reported 58 passed and 32 database-dependent tests ignored;
  frontend unit tests reported 149 passed. Providerfile validation passed.

Not implemented or not yet verified:

- Direct same-network settlement for NEAR, TRON, or any other non-EVM network.
  Those assets may be used only where an executable routed flow is available.
- Wallet signing/settlement for Solana, Sei, Sui, TON, Aptos, Bitcoin, and
  other networks not supported by the current OTC wallet executors. Configured
  addresses for these networks do not enable OTC.
- Memo/tag-dependent source flows other than the currently supported NEAR
  instruction path.
- Real-network end-to-end payments, fee collection, refund handling, and
  status observation across production token/network combinations. The
  successful route tests are automated/mocked; live settlement was not run.
- Database-backed OTC integration tests: `OTC_TEST_DATABASE_URL` was not
  available, so 32 such tests were skipped. Browser E2E and real-wallet
  signing were not validated in this run.
- A fee route can still be unavailable for a particular amount or provider
  state even if the asset pair is listed; the UI surfaces preparation errors
  and payout remains gated on verified fee collection.
- No confidentiality guarantees or “confidential swap” protocol are
  implemented or claimed.

These remaining items are release/implementation work, not permission to
advertise unsupported networks. Before calling the full multi-chain goal
complete, add wallet execution and durable observation/refund paths for the
remaining networks, run database-backed tests, and verify real-network
end-to-end settlements and fee collection.

## Remaining verification checklist

- Check exact USDT/USDC identities and ambiguity in the live provider catalog
  for every enabled source/destination pair.
- Verify amount limits, exact-output behavior, fee route costs, expiry, status,
  and refund behavior on production routes, including minimum fee amounts.
- Run the skipped PostgreSQL-backed OTC integration suite with an isolated
  `OTC_TEST_DATABASE_URL`.
- Add and validate direct non-EVM settlement only after implementing wallet
  signing, submission, observer, memo/tag, and refund handling for that chain.
- Define “confidential swap” technically before making any privacy claim.
