# soltempo

**Solana DeFi yield account for Tempo merchants.**

Soltempo bridges idle USDC from Tempo merchant balances into Solana DeFi (Kamino) for yield, then pulls back on demand for payouts. Each settlement is bound to its Tempo origin via an on-chain Receipt PDA emitted by [mppsol_cpi](https://github.com/mppsol/cpi). Soltempo is the **first concrete consumer of [mppsol](https://mppsol.org)** — the cross-VM settlement layer connecting Stripe-grade payments to Solana DeFi.

## Demo

**Primary submission video (v0.3):** [`demo-video/output/demo-v0.3-terminal.mp4`](demo-video/output/demo-v0.3-terminal.mp4) — 3:05, 1600×900. Real cross-VM cycle on live testnets (Tempo Moderato + Solana devnet) plus the v0.3 Kamino + pull-back proof segment. Opening → terminal screencast → Kamino proof → closing. All transactions verifiable on public block explorers.

**Alternate browser cut:** [`demo-video/output/demo-v0.3.mp4`](demo-video/output/demo-v0.3.mp4) — 2:47. Opening + the [merchant-web dashboard](apps/merchant-web/) driving the same flow through the UI + Kamino proof + closing. Same content, browser flavor for product/biz audiences.

Voiceover scripts (per-segment + continuous): [`demo-video/voiceover/`](demo-video/voiceover/) and [`demo-video/voiceover-plain.txt`](demo-video/voiceover-plain.txt). Recording playbooks: [`DEMO.md`](DEMO.md) (terminal cut), [`demo-video/record-merchant-web.mjs`](demo-video/record-merchant-web.mjs) (browser cut), [`demo-video/kamino-proof.tape`](demo-video/kamino-proof.tape) (VHS for the Kamino segment).

## Distribution thesis

Stripe brings tradfi merchant distribution (via Tempo). Solana brings DeFi yield distribution (via Kamino, Marginfi, Drift). Tempo merchants today earn 0% on operating balances. Soltempo connects the two — proving end-to-end that Tempo-originated payments can atomically reach Solana DeFi yield.

## Architecture

```
   ┌──────────────────────┐
   │ Merchant USDC balance │
   │     on Tempo (EVM)    │
   └──────────┬────────────┘
              │ deposit
              ▼
   ┌──────────────────────┐         ┌────────────────────┐
   │   Buffer.sol on Tempo │ ──CCIP─▶│  Vault on Solana   │
   │ - holds liquid buffer │         │  - receives intent │
   │ - emits intent above  │         │  - allocates USDC  │
   │   threshold via CCIP  │         │    to Kamino USDC  │
   └──────────────────────┘         └─────────┬──────────┘
                                              │
                                              │ CPI
                                              ▼
                                    ┌────────────────────┐
                                    │    mppsol_cpi      │
                                    │ pay_with_receipt   │
                                    │ → Receipt PDA      │
                                    │   bound to Tempo   │
                                    │   origin           │
                                    └────────────────────┘
```

Reverse path (payout): merchant requests payout → keeper triggers vault to withdraw from Kamino → vault calls `mppsol_cpi.pay_with_receipt` for the settlement Receipt → CCIP message back to Tempo Buffer → merchant withdraws.

## Locked architectural decisions (2026-05-09)

| Decision | Choice | Rationale |
| --- | --- | --- |
| Cross-chain rail | **Chainlink CCIP** | Activated on Tempo 2026-05-08; only confirmed Tempo↔Solana rail. CCTP doesn't cover Tempo; Wormhole hasn't added it. |
| Yield venue (v1.0 MVP) | **Kamino USDC** | Single venue. Multi-venue allocation deferred to v1.1+ post-beta-merchant feedback. |
| Solana-side settlement | **mppsol_cpi.pay_with_receipt** | Atomic on-chain payment-binding; Receipt PDA references the Tempo origin for cross-VM auditability. |
| HTTP-MPP integration | **Deferred to v1.1+** | MVP doesn't need paid signal feeds. When v1.1 adds depeg oracles/yield comparison feeds, soltempo will import `@solana/mpp` directly. |
| Tempo side at MVP | **Real testnet, not mocked** | Distribution thesis only matters if proven end-to-end. |

## Repo layout

Single monorepo. Polyglot by necessity (Anchor + Solidity + TS) but the components evolve together — version skew between vault and keeper would be a bug, not a feature.

```
soltempo/
├── programs/vault/           Solana Anchor program (Rust)
├── contracts/buffer/         Tempo Solidity contracts (Foundry)
│   └── src/
│       ├── Buffer.sol         Merchant treasury + CCIP send/receive
│       └── CrossVMIntent.sol  Canonical intent encoding (must match Solana side)
├── apps/keeper/              Off-chain keeper (TypeScript)
├── packages/types/           Shared TS types (CrossVMIntent, receipt formats)
├── tests/                    Anchor integration tests
├── Anchor.toml               Solana workspace config
├── Cargo.toml                Rust workspace
├── pnpm-workspace.yaml       TS workspace
└── tsconfig.base.json        Shared TS config
```

## Status

All v0.3 components live on testnet. Vault upgraded to program data length 346,616 bytes at slot 461142588 (devnet) — IDL upgraded at `4y5DcbBJgnuqe5fcSwEm8MFr6ATJ2oUvr9zq6EHodrAr`.

| Component | Status |
| --- | --- |
| **Buffer.sol on Tempo Moderato** | ✅ deployed — `0xe8c675523AFd81587c35Da2BeF6ECc268654D0BE`, with mock CCIP router + canonical CrossVMIntent send/receive. 9 Foundry tests passing. |
| **Vault on Solana devnet** | ✅ deployed — program `2YhYmfCoCj3VvyN2HQ3cuavMiZzEUdUTrhvo6nmGRXe3`, vault PDA `8smibhXARvuYGEadHqc9C9tqTJWFmLUdXAtkabtFaA9M`. 8 instructions live: `initialize`, `ccip_receive`, `trusted_keeper_receive`, `deposit_to_kamino`, `init_kamino_obligation`, `withdraw_from_kamino`, `settle_payout_to_tempo`, `request_pullback_to_tempo`. **37 unit tests passing.** |
| **CrossVMIntent canonical encoding** | ✅ fixed 122-byte big-endian layout shared across Solidity / Rust / TS. Cross-language test vector validated in all three. |
| **mppsol_cpi CPI integration** | ✅ `settle_payout_to_tempo` invokes `mppsol_cpi.pay_with_receipt` via `invoke_signed` with the vault PDA as signer — Receipt PDA bound to the cross-VM nonce on every settlement. |
| **Kamino integration** | ✅ `deposit_to_kamino` + `init_kamino_obligation` wire real CPIs to klend's `deposit_reserve_liquidity_and_obligation_collateral_v2`, `init_obligation`, `init_user_metadata`. 17-account context mirrors upstream exactly. End-to-end exercised on localnet via `scripts/localnet-with-klend.sh` (klend ships mainnet-only). |
| **Pull-back path** | ✅ `request_pullback_to_tempo` emits `PullbackRequested` event with full canonical intent — validated end-to-end on devnet (tx [`24CJ82M…mK5d`](https://explorer.solana.com/tx/24CJ82MhCn7W1pfxjWAcgevPo6MWxrRT575Pb76KzPMawwNKAg7bV6LA7cncLqmYh6qZ6ifmF8EqZBFybfaQmK5d?cluster=devnet)). Trusted-keeper relay handles the EVM-side settlement until CCIP ships on Tempo. |
| **CCIP receiver hardening** | ✅ canonical 3-account pattern (offramp signer PDA + allowlist PDA owned by router) matching `chainlink-ccip example-ccip-receiver` solana-v1.6. |
| **Off-chain keeper** | ✅ trusted-relayer mode running against the reference deploy — observes Tempo `MockMessageSent`, transfers SPL USDC from inventory, calls `trusted_keeper_receive`. |
| **Merchant frontend** | ✅ Next.js dashboard at `apps/merchant-web/`. Live cards, deposit + bridge button, pull-back button. Both buttons validated end-to-end against testnet via puppeteer. |
| **End-to-end cross-VM demo** | ✅ recorded — see [Demo](#demo). |

## Known TODOs (intentional, marked in code)

1. ~~**CrossVMIntent encoding standardization.**~~ ✅ Resolved 2026-05-09. See "Canonical CrossVMIntent encoding" below.
2. ~~**mppsol_cpi CPI mechanics.**~~ ✅ Resolved 2026-05-09. See "mppsol_cpi CPI integration" below.
3. **Vault PDA lamport top-up.** mppsol_cpi.pay_with_receipt creates a Receipt PDA whose rent is paid by `payer_authority` — i.e., the vault PDA. The Vault account itself only carries its own rent. The keeper must `SystemProgram::transfer` lamports to the vault PDA before calling `settle_payout_to_tempo`. Future: separate rent-payer from settlement authority via mppsol_cpi instruction shape change.
4. ~~**Kamino integration.**~~ ✅ Resolved 2026-05-07. `deposit_to_kamino` and `init_kamino_obligation` instructions wire real CPIs to klend's `deposit_reserve_liquidity_and_obligation_collateral_v2`, `init_obligation`, and `init_user_metadata`. The `KaminoDepositV2` account context mirrors klend's 17-account layout exactly. Caveat: klend has no devnet deployment, so end-to-end testing requires `scripts/localnet-with-klend.sh` (clones mainnet klend + Main market + USDC reserve via `solana-test-validator --clone`). Drift-catcher tests cover all 6 klend discriminators + the 17-account ix shape. See "Kamino integration" below.
5. ~~**CCIP receiver validation.**~~ ✅ Resolved 2026-05-09. See "CCIP receiver hardening" below.
6. **CCIP send-side from Solana — partial.** `settle_payout_to_tempo` and `request_pullback_to_tempo` now emit a richer `PullbackRequested` event carrying the full CrossVMIntent payload, destination chain selector, Tempo Buffer receiver address, and suggested gas limit — everything the keeper needs to construct the EVM-side tx. The `encode_generic_extra_args_v2(gas_limit, allow_ooo)` helper produces canonical CCIP `extra_args` bytes for EVM destinations. The actual `ccip_send` CPI is **deferred** because:
   - The router's `CcipSend` requires 18 named accounts plus 13-account pool blocks per token bridged, plus per-token Address Lookup Tables that the router validates against on-chain.
   - The recommended pattern (per chainlink-ccip `solana-v1.6.2`) is for the off-chain client to call `router.derive_accounts_ccip_send` (multi-stage) to discover the full account list + LUTs, then build the tx — this is fundamentally an off-chain plumbing problem, not an on-chain CPI problem.
   - CCIP is not yet deployed on Tempo Moderato testnet, so there's no destination to test against.

   Until CCIP-on-Tempo ships, soltempo uses the **trusted-keeper pull-back path**: `request_pullback_to_tempo` emits `PullbackRequested`, the keeper consumes off-chain, and the keeper performs the EVM-side settlement on Tempo. This mirrors the inbound `trusted_keeper_receive` path exactly. When CCIP ships on Tempo, the off-chain keeper switches to invoking `ccip_send` via `derive_accounts_ccip_send` — no vault redeploy required. Discriminators (`CCIP_SEND_DISCRIMINATOR`, `CCIP_GET_FEE_DISCRIMINATOR`), `CCIP_SENDER_SEED`, `SVM2AnyMessage` + `GetFeeResult` Borsh mirrors, and `encode_generic_extra_args_v2` are all in place ready for the in-program CPI variant.

## Canonical CrossVMIntent encoding

Fixed 122-byte big-endian layout, version-prefixed. Solidity, Rust, and TS implementations all round-trip the same shared hex vector — see test files for the canonical vector and round-trip assertions in each language.

```
Offset  Size  Field
─────────────────────────────────────────────────────────
0       1     version          (= 0x01)
1       1     kind             (0=DepositForYield, 1=PullbackForPayout, 2=ReceiptAck)
2       8     sourceChain      (BE u64 — CCIP chain selector)
10      16    amount           (BE u128 — USDC base units, 6 decimals)
26      32    sourceAddress
58      32    merchant
90      32    nonce
─────────────────────────────────────────────────────────
Total: 122 bytes, big-endian, no padding.
```

Implementation files:
- Solidity: `contracts/buffer/src/CrossVMIntent.sol`
- Rust: `programs/vault/src/lib.rs` (CrossVMIntentPayload)
- TypeScript: `packages/types/src/index.ts` (CrossVMIntent + encodeIntent + decodeIntent)

Tests:
- `contracts/buffer/test/Buffer.t.sol::test_canonicalEncoding_*` (Foundry)
- `programs/vault/src/lib.rs::tests` (`cargo test -p vault`)
- `packages/types/src/intent.test.ts` (`pnpm --filter @soltempo/types test`)

## mppsol_cpi CPI integration

`settle_payout_to_tempo` invokes `mppsol_cpi.pay_with_receipt` via `invoke_signed`. The vault PDA acts as `payer_authority` (signed via `[b"vault", authority, bump]` seeds), atomically transferring USDC from the vault to the pending-payout account AND emitting a Receipt PDA bound to the cross-VM nonce.

To avoid a Cargo dependency on `mppsol_cpi` (which lives in a separate Anchor workspace), soltempo includes a small `mppsol_cpi_client` module that builds the instruction by hand:

- `PROGRAM_ID` — `624xoctSeGzq1TAVwZU1xbM9RozAd3xZmjPeFXrAY14j` (devnet)
- `RECEIPT_SEED` — `b"receipt"` (mirrors mppsol_cpi)
- `PAY_WITH_RECEIPT_DISC` — `[45, 221, 79, 34, 209, 140, 222, 126]` = first 8 bytes of `sha256("global:pay_with_receipt")`. Re-derived at test time so any future drift (e.g., function rename in mppsol_cpi) fails the test rather than silently failing on mainnet.
- `PayArgs` — mirror of `mppsol_cpi::PayArgs` (Borsh layout: `amount(u64) | nonce([u8;32]) | request_hash([u8;32]) | expiry(i64)` = 80 bytes)
- `build_pay_with_receipt_ix(...)` — constructs the `Instruction` with discriminator + serialized args + 8 account metas in the canonical order
- `derive_receipt_pda(payer, nonce)` — `find_program_address([RECEIPT_SEED, payer, nonce])`

Tests cover discriminator correctness, args round-trip, account ordering + signer/writable flags, and PDA derivation determinism.

## CCIP receiver hardening

`vault::ccip_receive` follows the canonical Chainlink CCIP receiver pattern (per [smartcontractkit/chainlink-ccip example-ccip-receiver, solana-v1.6.0](https://github.com/smartcontractkit/chainlink-ccip/tree/solana-v1.6.0/chains/solana/contracts/programs/example-ccip-receiver)). Three security checks happen at the Anchor account-constraint level before the instruction body runs:

1. **`authority` (Signer)** — PDA derived as `[EXTERNAL_EXECUTION_CONFIG_SEED, our_program_id]` under the `offramp_program`. Only the offramp can produce a CPI where this PDA signs (the runtime enforces this). If anyone other than the offramp tries to forge a `ccip_receive` call, the signer constraint fails.
2. **`offramp_program` (UncheckedAccount)** — used as `seeds::program` for `authority` and in the `allowed_offramp` derivation.
3. **`allowed_offramp` (UncheckedAccount)** — PDA at `[ALLOWED_OFFRAMP_SEED, source_chain_le, offramp_program]` derived under `vault.ccip_router`, **owned by the router**. If the router has not allowlisted this offramp for this source chain, the account doesn't exist and the `owner` constraint fails.

Then the instruction body validates:
- `message.source_chain_selector == vault.expected_tempo_chain_selector` (configured at vault init)
- `sender_matches(message.sender, vault.expected_tempo_sender)` — accepts both 20-byte raw EVM addresses and 32-byte left-padded form

On the Solidity side, `Buffer.sol::_ccipReceive` performs the symmetric check: `message.sourceChainSelector == solanaChainSelector` and `keccak256(message.sender) == keccak256(solanaVaultAddress)`.

Constants:
- `EXTERNAL_EXECUTION_CONFIG_SEED = b"external_execution_config"`
- `ALLOWED_OFFRAMP_SEED = b"allowed_offramp"`
- `CCIP_ROUTER_DEVNET = Ccip842gzYHhvdDkSyi2YVCoAWPbYJoApMFzSxQroE9C`
- `CCIP_SOLANA_DEVNET_CHAIN_SELECTOR = 16423721717087811551`

6 unit tests verify sender_matches, the EXTERNAL_EXECUTION_CONFIG PDA derivation, and the ALLOWED_OFFRAMP PDA derivation.

## Kamino integration

The vault deposits idle USDC into Kamino's USDC reserve via real CPIs. Two instructions handle the lifecycle:

- **`init_kamino_obligation`** — runs once per vault. Calls klend's `init_user_metadata` + `init_obligation` in sequence, creating the on-chain obligation owned by the vault PDA. The vault PDA signs both CPIs via `invoke_signed` with `[b"vault", authority, bump]`. Fee payer is passed in (typically the merchant or keeper) and pays rent for the new accounts.
- **`deposit_to_kamino`** — invokes klend's `deposit_reserve_liquidity_and_obligation_collateral_v2` via CPI. The vault PDA acts as `obligation owner` (signer) and as the authority on `user_source_liquidity` (the vault's USDC ATA). The full 17-account `KaminoDepositV2` context mirrors klend's upstream `DepositReserveLiquidityAndObligationCollateralV2` exactly, including the legacy `placeholder_user_destination_collateral` slot and the `obligation_farm_user_state` / `reserve_farm_state` / `farms_program` farm-accounts trio (Optional accounts, but the slots must exist).

Caller responsibilities the vault doesn't re-validate (because klend rejects malformed setups at execution):
1. Transaction MUST include `klend.refresh_reserve(reserve)` and `klend.refresh_obligation(obligation, [reserves])` before `deposit_to_kamino`. v2 doesn't enforce this at the ix level, but LTV/borrow-cap checks inside klend assume fresh interest accruals.
2. Obligation must already be initialized via `init_kamino_obligation`.
3. The Kamino market chosen at vault init (`vault.kamino_market`) must have a USDC reserve.

### Why localnet, not devnet
klend has no devnet deployment — only mainnet (`KLend2g3...`) and a staging build that also lives on mainnet under a separate ID (`SLendK7y...`). To exercise the CPI without paying real mainnet fees:

```sh
./scripts/localnet-with-klend.sh
```

This boots `solana-test-validator` with the klend program, the Kamino farms program, the Main market, and the USDC reserve all cloned from mainnet via `--clone`. The vault then deploys to localnet and `deposit_to_kamino` runs against the cloned reserve.

For mainnet deployment: `vault.kamino_market` is set to one of Kamino's published markets (Main, JLP, Altcoins) at init. No vault code change required.

Discriminators (re-derived at test time via `sha256("global:<name>")[..8]` drift catchers):
- `INIT_OBLIGATION_DISC`, `DEPOSIT_RESERVE_LIQUIDITY_AND_OBLIGATION_COLLATERAL_V2_DISC`, `WITHDRAW_OBLIGATION_COLLATERAL_AND_REDEEM_RESERVE_COLLATERAL_V2_DISC`
- `REFRESH_RESERVE_DISC`, `REFRESH_OBLIGATION_DISC`
- `INIT_USER_METADATA_DISC`

Verified against klend master `3f7bd693`.

## Merchant dashboard

`apps/merchant-web` is a Next.js single-page dashboard that visualizes the cross-VM state in real time:

- Merchant pathUSD balance on Tempo
- `Buffer.sol` pathUSD balance + configured `bufferTarget`
- Vault PDA's USDC ATA balance (the bridged destination)
- `vault.total_deposits` decoded from the vault account at offset `0x88`
- Cross-VM activity feed — auto-detects buffer/vault deltas every 4s and surfaces them as events

The "Deposit + bridge" button performs `approve` → `deposit` → `sendIntentToSolana` through the merchant's hot wallet (env-loaded — testnet only) and waits for the keeper to relay. The polling loop catches the resulting `vault.total_deposits` increment within ~25s.

```sh
cp apps/merchant-web/.env.local.example apps/merchant-web/.env.local
# Set NEXT_PUBLIC_MERCHANT_PRIVATE_KEY to enable the deposit button
pnpm --filter @soltempo/merchant-web dev   # → http://localhost:4001
```

Without a merchant key, the dashboard runs in read-only mode — useful for showing live testnet state to viewers who don't have the demo key.

## Getting started

Required toolchains:
- Node 20+ and `pnpm` 9+
- Rust + Solana CLI 3.1.14+ + Anchor 0.30.x
- Foundry (`forge`, `cast`, `anvil`)

```sh
pnpm install                          # install TS workspace deps

# Foundry: install Chainlink CCIP contracts and forge-std
cd contracts/buffer
forge install foundry-rs/forge-std --no-commit
forge install smartcontractkit/chainlink-local --no-commit
cd ../..

forge build --root contracts/buffer   # build the Tempo buffer contract
forge test --root contracts/buffer    # run Foundry tests

anchor build                          # build the Solana vault program

pnpm --filter @soltempo/keeper dev          # run the keeper
pnpm --filter @soltempo/merchant-web dev    # run the merchant dashboard
```

## Why this matters

- **Tempo (EVM, Reth-based)** = Stripe's payments distribution, but no native yield
- **Solana** = deepest stablecoin DeFi liquidity (Kamino, Marginfi, Drift)
- **Soltempo** = the connector. Tempo merchants get Solana DeFi yield without leaving their tradfi-grade UX.
- **mppsol** = the underlying cross-VM settlement primitive that makes the connection auditable on-chain (every payout is a Receipt PDA)

## Composability

Soltempo is built from public Solana primitives and exposes its own as composable building blocks for downstream protocols.

**What soltempo composes with (CPI in):**

- **`mppsol_cpi.pay_with_receipt`** — every settlement on Solana invokes mppsol_cpi to mint a Receipt PDA bound to the cross-VM nonce. The vault PDA signs as `payer_authority` via `invoke_signed`. Manual instruction client (no Cargo dep) so soltempo stays independently cloneable.
- **`klend.deposit_reserve_liquidity_and_obligation_collateral_v2`** — `deposit_to_kamino` invokes klend via CPI with the vault PDA as obligation owner. 17-account context mirrors klend's upstream layout exactly. Refresh ix sequencing is the caller's responsibility.
- **`klend.init_obligation` + `init_user_metadata`** — `init_kamino_obligation` wraps both in one ix so a vault is Kamino-ready in a single tx.
- **Chainlink CCIP receiver pattern** — `ccip_receive` follows the canonical 3-account pattern (offramp signer PDA + allowlist PDA owned by router) verified against `chainlink-ccip example-ccip-receiver` solana-v1.6.

**What soltempo exposes (CPI out):**

- **`vault.request_pullback_to_tempo(amount, nonce)`** — emits a canonical `PullbackRequested` event with the full 122-byte CrossVMIntent. Any downstream protocol holding USDC in a soltempo vault can request a payout to Tempo through this single instruction; the keeper handles the EVM-side settlement.
- **`vault.settle_payout_to_tempo`** — composes `mppsol_cpi.pay_with_receipt` + `PullbackRequested` emit + USDC transfer in one ix. Other Solana programs can build payment-binding wrappers around this.
- **`CrossVMIntentPayload` (122-byte canonical encoding)** — shared layout between Solidity, Rust, and TS implementations. Any Solana ↔ EVM bridge can adopt the same encoding for cross-VM intents.

**Drift-catcher tests** (15 tests across vault) re-derive every Anchor discriminator from `sha256("global:<name>")[..8]` at test time. Anyone forking soltempo and seeing those tests pass has cryptographic confirmation that the on-chain Klend / mppsol_cpi / CCIP discriminators haven't drifted from upstream.

**Reference protocols that could compose:**

- A multi-venue yield aggregator could CPI into multiple `vault::deposit_to_kamino`-like adapters routed by oracle price.
- A merchant-facing wallet UI could read vault state directly (offset `0x88` for `total_deposits`) without an SDK.
- A treasury-management DAO could use `request_pullback_to_tempo` as a programmatic withdrawal primitive — the on-chain `PullbackRequested` event is the audit trail.

## Business model

Soltempo monetizes the spread between Solana DeFi yield and the 0% baseline merchants currently earn on idle Tempo balances.

**Revenue model:** soltempo takes a percentage of yield earned (industry standard 10–20%), passing the remainder to the merchant. At Kamino's USDC supply APY (~5–8% historically), a $10M merchant treasury earns $500K–$800K gross yield annually; soltempo captures $50K–$160K of that. Per-merchant operating cost is near-zero — a single keeper relays for many merchants and the vault PDA pattern is shared infrastructure.

**Why this is a real business, not a tech demo:**

- **Distribution is solved.** Tempo is Stripe's L2 — every Stripe merchant who adopts Tempo settlement is a soltempo prospect by default. We don't need to acquire merchants; we need to plug into Stripe's existing onboarding once Tempo mainnet ships.
- **The product replaces a $0 baseline.** Stripe merchants today earn nothing on operating balances. Even a 4% net yield (after take rate) is an unambiguous win — no comparable product to displace.
- **No exotic strategy required.** Kamino USDC supply is the most boring, most legible yield on Solana. Auditors and merchant CFOs can understand it in a sentence. v1.0 ships with one venue; v1.1+ adds Marginfi/Drift for redundancy and rate optimization.
- **Solana wins the unit economics.** Tx fees of ~5,000 lamports per settlement (~$0.001 at $200 SOL) make per-merchant profitability viable from $10K balances upward. Same flow on Ethereum L1 would cost $5–50 per settlement and break the model.

**Total addressable market:**

- Stripe merchants: ~4M businesses globally, ~$1T+ in annual processed payments
- Median operating balance to processed payments ratio: ~2–4%, so ~$20–40B sitting idle
- At 1% TAM penetration + 100 bps net take rate = $2–4M annual revenue (single-product run-rate)
- 10% TAM penetration unlocks $20–40M (still tiny vs Stripe's $14B revenue, so well within distribution-channel feasibility)

**Cost structure:**

- Keeper infrastructure: ~$200–500/month per region (single replica) — shared across all merchants
- Solana validator costs: pay-per-tx, scales linearly with merchant count
- Smart-contract audits: one-time $50–150K for vault + Buffer (gate before mainnet)

## Roadmap

**v0.3 (today, 2026-05-09)** — what's shipped on testnet:

- Cross-VM cycle live on Tempo Moderato + Solana devnet
- Vault deployed with 8 instructions including Kamino CPI and pull-back
- Trusted-keeper relay with verifiable on-chain Receipt PDAs
- Merchant dashboard with live state polling

**v1.0 (mainnet)** — gating items:

- [ ] Smart-contract audit (vault + Buffer + mppsol_cpi)
- [ ] Tempo mainnet deployment (waiting on Tempo)
- [ ] CCIP-on-Tempo activation (waiting on Chainlink) → swap from trusted-keeper to in-program `ccip_send`
- [ ] Multisig upgrade authority on vault program
- [ ] First merchant pilot (1–3 design partners, target $1–10M treasury each)

**v1.1+** — yield depth:

- Multi-venue routing (Marginfi + Drift in addition to Kamino) with rate-driven rebalancing
- Per-merchant yield-share tier negotiation
- Webhook integrations for merchant accounting systems

**v2.0** — beyond Tempo:

- Same vault + Receipt PDA pattern accepts intents from other EVM L2s (any chain Chainlink CCIP supports)
- mppsol_cpi.pay_with_receipt becomes the de-facto Solana-side settlement primitive for cross-VM payment apps

## License

Apache 2.0 — see [LICENSE](./LICENSE).
