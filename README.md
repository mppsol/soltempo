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
| CrossVMIntent canonical encoding | ⚠️ **OPEN** — Solidity side uses `abi.encode`; Solana side uses Borsh. Need to standardize on a shared format (likely manual byte packing). |
| Kamino USDC integration | ⏳ TODO — CPI calls stubbed; Kamino IDL not yet wired in |
| mppsol_cpi CPI integration | ⏳ TODO — account contexts and ProgramId set; instruction call stubbed pending vault-PDA-as-signer mechanics |
| Off-chain keeper | ✅ scaffolded — viem + @solana/web3.js wiring; event subscriptions stubbed |
| Foundry tests | ✅ basic Buffer constructor + threshold tests |
| Anchor tests | ⏳ TODO |
| Tempo testnet deployment | ⏳ pending Buffer.sol completion + Solana vault deployment |
| End-to-end CCIP demo | ⏳ pending all of the above |

## Known TODOs (intentional, marked in code)

1. **CrossVMIntent encoding standardization.** Solidity `abi.encode` and Solana Borsh produce incompatible bytes. Need a manual byte-packed canonical encoding both sides understand.
2. **mppsol_cpi CPI mechanics.** Vault PDA needs to sign the SPL transfer in `pay_with_receipt` — requires either delegate authority pattern or invoke_signed with vault seeds.
3. **Kamino integration.** CPI to Kamino lend program needs IDL + account derivation logic. Currently emits events but does not actually deposit.
4. **CCIP receiver validation.** `_ccipReceive` should validate `message.sender` matches the Tempo Buffer address (currently TODO).
5. **CCIP offramp PDA validation.** Solana `ccip_receive` should validate the first account is the canonical CCIP offramp CPI signer.

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
