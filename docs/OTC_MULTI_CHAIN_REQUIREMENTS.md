# OTC multi-chain stablecoin requirements

## Purpose

This document records the requested OTC product goals, decisions, and the
verified implementation status. It remains the working requirements baseline
for extending OTC from its Ethereum-centric USDT flow to multi-chain USDT and
USDC flows; implemented support is narrower than the full target.

The original request was to block test networks for OTC offers, add USDC alongside
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
4. Exclude all test networks from OTC offer creation and settlement.
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
  Base, Aptos, and Sui. The address values belong in the deployment
  environment, not in this requirements document or tests.

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
- All test networks are rejected by both UI availability and backend
  validation.
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

## Implementation status (2026-11-12)

The OTC principal-payment path supports cross-network USDT and USDC settlement
through NEAR Intents when the selected assets and route are present in the
provider catalog. This is a working subset, not completion of every network in
the requested target list. Offer and trade APIs constrain selectable assets
to configured wallet executors, exact provider token identities, fee
destinations, and available payment/fee routes. Production live-route checks
are deployment validation, not missing application implementation. The
interface describes these generically as payment routes and does not expose
provider branding.

Implemented:

- Public offers support USDT and USDC, explicit receiving networks, and exact
  catalog asset IDs. When taking a buy offer, the taker can select a supported
  receiving network and enter the destination address; accepted destination
  terms are bound to the trade. Legacy offers without a stablecoin still mean
  USDT.
- Sell offers persist their selected receiving network and exact asset ID.
  When accepting a buy offer, the counterparty selects the receiving network,
  asset ID, and address; the backend validates and freezes those terms into the
  deal before preparing a route.
- The P2P taker can select a source token/network from the live provider token
  catalog and review the calculated source amount, exact net quote output,
  expiry, deposit address, and memo before wallet submission. The source
  stablecoin may differ from the deal stablecoin, enabling a USDC-to-USDT or
  USDT-to-USDC principal conversion through the route.
- The end-to-end cross-network principal-payment path is implemented for both
  USDT and USDC deal assets: the backend prepares an exact-output quote bound to
  the exact source/destination asset IDs, recipient, and source refund address;
  the matching wallet signs and submits the deposit; the backend notifies and
  polls NEAR Intents. The deal advances only on the exact required output, while
  a provider-confirmed refund marks the attempt failed and permits an explicit
  retry. This shared path does not promise that every catalog pair is routable.
- The backend validates the selected source asset against its exact catalog
  asset ID and the selected OTC stablecoin. Quote creation binds exact source
  and destination asset IDs to avoid resolving duplicate tickers to a different
  token.
- The backend serializes quote preparation per trade and returns an existing
  attempt for a repeated idempotency key rather than creating another quote.
- Provider quote and deposit-notification failures are reported as a generic
  temporary route outage; provider-specific error details are kept out of the
  user response.
- OTC attempt instructions persist the source/destination asset identity and
  quote/deposit information. Provider status polling advances the trade only
  when the reported output amount exactly matches the required net amount; a
  confirmed refund releases the attempt for retry. Exact-output quote creation
  is covered by a mocked provider test that checks the requested output and
  decodes the calculated source amount.
- Route attempts are idempotent and persisted for restart-safe polling.
  Unknown provider state remains pending; only an exact expected output
  advances the trade, and a confirmed refund permits retry.
- For identical source and destination assets, the ordinary direct payment
  path is used without a provider quote only when the exact catalog asset IDs
  match and two independent observers are configured for that receiving
  network. Ethereum uses the peer Ethereum RPC pair; Base, Arbitrum One,
  Optimism, BNB Smart Chain, Gnosis, Polygon PoS, Avalanche C-Chain, and Sei
  use `p2p_evm_rpcs`. TRON, TON and Solana use `p2p_chain_rpcs`; TRON evidence
  requires one exact TRC-20 `Transfer` log in solidity-node transaction info,
  TON walks the wallet-to-jetton-wallet trace and waits for a five-minute
  finality window, and Solana requires one exact finalized
  `transferChecked`. The backend observes transfers on the booked network.
  A non-EVM direct payment can be signed from its booked wallet without a
  separate Ethereum login wallet. This applies when the booked payer is the
  owner (a buy offer); a sell offer books the taker's Ethereum login wallet, so
  a non-EVM taker payment uses the routed flow instead of direct settlement.
- The portfolio balance panel reads every USDT/USDC pair the provider catalog
  knows on a supported wallet network: Ethereum and every `p2p_evm_rpcs`
  network through pinned ERC-20 `balanceOf`, TRON through a solidity-node
  constant call, Solana through the owner's finalized token accounts, and TON
  through the owner's jetton wallet. Rows are display-only, use the verified
  Ethereum wallet as on every chain (the address booked as `usdt_payer`), and
  report `unavailable` when the network has no configured reader instead of a
  misleading zero.
- A direct EVM USDT fee transfer bypasses route lookup when the payment source
  is the exact USDT token on the receiving network. This avoids rejecting a
  valid direct payment because the route provider does not expose a redundant
  USDT-to-USDT route. Other fee sources require an exact-output route to the
  configured USDT recipient, and its source amount/route fee are preflighted
  before the principal transfer is signed.
- The recorded fee policy is denominated in USDT. For USDC offers, the fee
  amount uses the agreed nominal 1:1 USDC/USDT parity and is persisted
  separately for audit. The seller receives principal net of the fee; the
  payer separately signs an exact USDT fee transfer or exact-output route.
  Routed fee amount and provider fee are preflighted before signing the
  principal payment. Fee accrual starts after payment verification, and the
  seller's EVER payout is gated on verified fee collection.
- Database migrations include the P2P fee basis, fee collection, and supported
  EVM chain names.
- Destination choices are filtered by the configured fee address, wallet
  executor support, a six-decimal USDT token, and available payment/fee routes.
  Ethereum, supported EVM networks, NEAR, TRON, Solana, Sui, TON, and Aptos
  have wallet executors. Sei is handled as EVM chain ID 1329 and requires an
  EVM-format fee address plus two configured RPC endpoints for direct
  settlement; the example config now includes two independent mainnet
  endpoints. The Sui fee recipient is now configured; Sui is still offered as
  a destination only when the live token catalog and payment/fee routes pass
  the backend filters.

Not implemented or not yet verified:

- The browser wallet adapters and backend OTC gates include Solana, Sui, TON,
  Aptos, and Sei (EVM). Deployment config supplies public RPC/API defaults and
  a TON Connect manifest; production wallet/provider combinations still need
  deployment validation.
- Memo handling is implemented for NEAR, Solana, and TON deposits. Memo-bearing
  routes for other networks are rejected rather than sent without the memo.
- Direct same-network settlement is implemented for EVM, TRON, TON and
  Solana; NEAR, Sui, Aptos and other networks still require a routed
  cross-network payment or remain unavailable. The asset endpoint excludes a
  pair unless its observers are configured and the exact stablecoin identity is
  unambiguous. TRON and TON use a submitted transaction/message reference and
  do not provide a locate scan; an attempt without a reference stays prepared.
  No live TRON, TON or Solana payment, fee collection or refund has been
  executed in this work, so production wallet/provider combinations and
  failure/refund paths remain unverified. Cross-network principal and fee
  routes continue to use exact-output quotes and status verification.
- Real-wallet end-to-end payments, fee collection, refund handling, and status
  observation across production token/network combinations have not been run.
  A network is offered only when the live token catalog, configured fee
  recipient, and payment/fee routes pass the backend filters.
- Fee-routing availability is amount-dependent: a quote can still fail after
  the catalog shows a route, for example if the provider applies a minimum
  amount. The UI surfaces that failure and does not permit seller payout until
  the exact fee output is verified. The 3% USDT fee on USDC offers uses the
  explicitly agreed nominal 1:1 calculation, not a live USDC/USDT conversion.
- Database-backed OTC integration tests require an isolated
  `OTC_TEST_DATABASE_URL`; 32 such tests were skipped in the last recorded run.
- No confidentiality guarantees or “confidential swap” protocol are
  implemented or claimed.

## Remaining verification checklist

- Exact production USDT/USDC token identities in the live provider catalog,
  including whether multiple assets share a symbol/network pair.
- Whether each target network has a usable refund-address format and whether
  its transfer requires a memo/tag.
- Whether the configured wallet extensions and RPC/API endpoints support the
  signing and submission behavior used by each new adapter.
- Production verification of provider-reported output amounts and exact-output
  behavior across every enabled token/network pair (post-deployment validation).
- Whether each catalog network's provider assets, refund addresses, and memo
  requirements match the implemented wallet and status flow.
- Which configured fee destinations have a live USDT token and reliable
  exact-output route at the smallest applicable fee amount.
- Verify amount limits, exact-output behavior, fee route costs, expiry, status,
  and refund behavior on production routes, including minimum fee amounts.
- Run the skipped PostgreSQL-backed OTC integration suite with an isolated
  `OTC_TEST_DATABASE_URL`.
- What “confidential swap” is intended to mean technically; until specified,
  no privacy promise is in scope.
