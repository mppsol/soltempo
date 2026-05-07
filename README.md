# soltempo

Tempo merchant treasury that auto-yields idle USDC on Solana via [mppsol](https://github.com/mppsol).

## What

Merchants on Tempo accumulate operating-balance stablecoins that today earn nothing. soltempo provides a treasury layer that:

1. Holds a configurable liquid buffer on Tempo for immediate payouts.
2. Auto-bridges balances above the buffer to Solana via mppsol settlement.
3. Allocates bridged USDC across yield venues (Kamino, Marginfi, Drift Insurance Fund).
4. Pulls back from Solana on demand when payouts exceed the buffer.

The merchant sees one dashboard: balance, yield earned, current APY, time-to-liquid.

## Why

- Tempo is a payments rail, not a yield venue. Idle merchant balances earn 0% by default.
- Solana has the deepest stablecoin DeFi liquidity. Routing idle balances there is the obvious capital-efficiency play.
- mppsol provides a neutral cross-chain settlement primitive between Tempo and Solana, which makes the bridge leg auditable and reusable.
- Three-way convergence: Tempo merchants need yield, Solana DeFi needs stablecoin TVL, mppsol needs production consumers.

## Architecture

```
Merchant on Tempo
    │
    ▼
[ Tempo buffer contract ]  ←─── threshold logic, payout authorization
    │
    │  bridge intent (above buffer)
    ▼
[ mppsol settlement ]  ←─── neutral cross-chain primitive
    │
    ▼
[ Solana yield vault ]  ←─── allocates across Kamino / Marginfi / Drift IF
    │
    │  pull-back on demand
    ▲
    │
[ Off-chain keeper ]  ←─── thresholds, rebalances, payout-driven pull-backs
```

| Component | Stack | Status |
| --- | --- | --- |
| Solana yield vault | Anchor (Rust) | Planned |
| Tempo buffer contract | Solidity (Reth/Foundry) | Planned |
| Bridge primitive | mppsol | External dependency |
| Keeper | TypeScript | Planned |
| Merchant dashboard | Next.js | Planned (Phase 4) |

## MVP scope

Single venue (Kamino USDC), single merchant, manual signer for payouts, no JIT liquidity pool, minimal dashboard. Multi-venue allocation, JIT pool on Tempo, configurable risk profiles, and dashboard polish wait for v1.1 — driven by real beta-merchant feedback, not pre-launch guesses.

## Repo layout

Single monorepo. Polyglot by necessity (Anchor + Solidity + TS) but the components evolve together — version skew between vault and keeper would be a bug, not a feature.

```
soltempo/
├── programs/vault/        Solana Anchor program (Rust)
├── contracts/buffer/      Tempo buffer contract (Solidity, Foundry)
├── apps/keeper/           Off-chain keeper service (TypeScript)
├── packages/types/        Shared TS types between keeper and future dashboard
├── tests/                 Anchor integration tests
├── Anchor.toml            Solana workspace config
├── Cargo.toml             Rust workspace (members: programs/*)
├── pnpm-workspace.yaml    TS workspace (members: apps/*, packages/*)
└── tsconfig.base.json     Shared TS config
```

The Next.js merchant dashboard (Phase 4) will live at `apps/dashboard/` once it exists. Foundry config lives inside `contracts/buffer/` rather than at the root because Foundry expects a single project per directory tree.

## Getting started

Required toolchains:
- Node 20+ and `pnpm` 9+
- Rust + Solana CLI + Anchor 0.30.x
- Foundry (`forge`, `cast`, `anvil`)

```sh
pnpm install                  # install TS workspace deps
anchor build                  # build the Solana vault program
forge build --root contracts/buffer   # build the Tempo buffer contract
pnpm --filter @soltempo/keeper dev    # run the keeper stub
```

## Status

Scaffolded. 12-week build plan drafted. mppsol must be testnet-functional before the integrated demo at Week 8; until then the bridge leg is mocked.

## License

Apache 2.0 — see [LICENSE](./LICENSE).
