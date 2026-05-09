# soltempo

**Solana DeFi yield account for Tempo merchants.**

Soltempo bridges idle USDC from Tempo merchant balances into Solana DeFi (Kamino) for yield, then pulls back on demand for payouts. Each settlement is bound to its Tempo origin via an on-chain Receipt PDA emitted by [mppsol_cpi](https://github.com/mppsol/cpi). Soltempo is the **first concrete consumer of [mppsol](https://mppsol.org)** — the cross-VM settlement layer connecting Stripe-grade payments to Solana DeFi.

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

| Component | Status |
| --- | --- |
| Solidity Buffer.sol with CCIP send/receive | ✅ scaffolded — compiles, needs deployment & integration tests |
| Solana Anchor vault skeleton | ✅ scaffolded — `ccip_receive`, `settle_payout_to_tempo`, mppsol_cpi CPI structure |
| **CrossVMIntent canonical encoding** | ✅ **RESOLVED** — fixed 122-byte big-endian layout, version-prefixed. All three implementations (Solidity, Rust, TS) round-trip the same shared hex vector. 8 Rust tests + 9 TS tests passing; Solidity test runs once Foundry deps are installed. |
| Kamino USDC integration | ⏳ TODO — CPI calls stubbed; Kamino IDL not yet wired in |
| **mppsol_cpi CPI integration** | ✅ **RESOLVED** — `settle_payout_to_tempo` now invokes `mppsol_cpi.pay_with_receipt` via `invoke_signed` with the vault PDA as signer. Manual instruction client (no Cargo dep on mppsol_cpi) with verified Anchor discriminator. 5 unit tests for client correctness. |
| Off-chain keeper | ✅ scaffolded — viem + @solana/web3.js wiring; event subscriptions stubbed |
| Foundry tests | ✅ Buffer constructor + intent encoding round-trip + canonical vector |
| Anchor tests (program integration) | ⏳ TODO |
| Tempo testnet deployment | ⏳ pending Buffer.sol completion + Solana vault deployment |
| End-to-end CCIP demo | ⏳ pending all of the above |

## Known TODOs (intentional, marked in code)

1. ~~**CrossVMIntent encoding standardization.**~~ ✅ Resolved 2026-05-09. See "Canonical CrossVMIntent encoding" below.
2. ~~**mppsol_cpi CPI mechanics.**~~ ✅ Resolved 2026-05-09. See "mppsol_cpi CPI integration" below.
3. **Vault PDA lamport top-up.** mppsol_cpi.pay_with_receipt creates a Receipt PDA whose rent is paid by `payer_authority` — i.e., the vault PDA. The Vault account itself only carries its own rent. The keeper must `SystemProgram::transfer` lamports to the vault PDA before calling `settle_payout_to_tempo`. Future: separate rent-payer from settlement authority via mppsol_cpi instruction shape change.
4. **Kamino integration.** CPI to Kamino lend program needs IDL + account derivation logic. Currently emits events but does not actually deposit.
5. ~~**CCIP receiver validation.**~~ ✅ Resolved 2026-05-09. See "CCIP receiver hardening" below.
6. **CCIP send-side from Solana.** `settle_payout_to_tempo` emits `PullbackInitiated` but does not yet invoke the Chainlink CCIP router program to actually deliver the message back to Tempo. The send-side is structurally documented in code but the router CPI call is the open work — needs verification of the current chainlink-svm v1.6 router instruction format.

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

pnpm --filter @soltempo/keeper dev    # run the keeper
```

## Why this matters

- **Tempo (EVM, Reth-based)** = Stripe's payments distribution, but no native yield
- **Solana** = deepest stablecoin DeFi liquidity (Kamino, Marginfi, Drift)
- **Soltempo** = the connector. Tempo merchants get Solana DeFi yield without leaving their tradfi-grade UX.
- **mppsol** = the underlying cross-VM settlement primitive that makes the connection auditable on-chain (every payout is a Receipt PDA)

## License

Apache 2.0 — see [LICENSE](./LICENSE).
