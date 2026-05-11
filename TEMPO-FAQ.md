# Tempo FAQ — context for Solana judges

soltempo is a Solana-native product, but it bridges from Tempo. Judges unfamiliar with Tempo can use this page as a 60-second briefing.

## What is Tempo?

**Tempo** is a payments-focused L1 blockchain built on Reth (Paradigm's Rust Ethereum execution client). Mainnet launched **2026-03-18**.

| Property | Value |
| --- | --- |
| Execution client | **Reth** (built by Paradigm) — Tempo is a node extension crate, not a fork |
| Backers | **Stripe + Paradigm** (cofounders) |
| Anchor validator | **Visa** (announced 2026-04-14) |
| First native stablecoin | **USD1** (launched 2026-05-08) |
| Cross-chain rail | **Chainlink CCIP** (activated 2026-05-08) |
| Public test network | **Tempo Moderato** |

Tempo's thesis: Stripe's distribution channel into onchain payments. Not designed as a DeFi chain — designed for merchant payment rails at Stripe scale.

## Why does Tempo matter to Solana?

Tempo brings the merchant base. **Tempo has no native yield layer.** Tempo merchants holding idle USDC need somewhere to earn yield — and that somewhere has to satisfy four constraints:

1. **Sub-second finality** for merchant UX
2. **Cents-per-tx settlement** for unit economics
3. **Depth-of-market stablecoin lending** for real yield
4. **Mature DeFi infrastructure** (Kamino, Marginfi, Drift)

**Only Solana satisfies all four.** Ethereum L1 fees alone disqualify it. Other L1s lack the DeFi depth.

soltempo is the working proof of this — Solana is the destination for the next wave of payment merchants.

## How is soltempo related to mppsol and `@solana/mpp`?

Three distinct things — judges should not conflate them:

| Project | Owner | Purpose |
| --- | --- | --- |
| **`@solana/mpp`** | Solana Foundation | Official Solana implementation of the Machine Payments Protocol (HTTP-based agent payments). Shipped 2026-03-18. Foundation territory. |
| **`mppsol_cpi`** | psyto (independent) | Anchor program providing **on-chain settlement primitives** — atomic CPI for payment-binding. Deployed Solana devnet, used by soltempo for Receipt PDAs. |
| **soltempo** | psyto (independent) | The consumer product. Uses `mppsol_cpi` for on-chain settlement; will integrate `@solana/mpp` for HTTP-MPP signals in v1.1+. |

soltempo composes both layers, but at MVP it only depends on `mppsol_cpi` directly. The HTTP layer (`@solana/mpp`) is roadmap.

## Why CCIP, not Wormhole / LayerZero / CCTP?

As of 2026-05-08, **Chainlink CCIP is the only confirmed Tempo↔Solana rail.**

- **CCTP** (Circle's bridge) — doesn't cover Tempo (Tempo doesn't natively host USDC yet)
- **Wormhole** — hasn't added Tempo
- **LayerZero** — hasn't added Tempo
- **CCIP** — activated on Tempo 2026-05-08, the moment we could integrate

The soltempo vault is designed to accept additional cross-chain rails by adding a new `*_receive` instruction. Same vault, same Kamino integration — no migration when alternatives ship.

## Is this production-ready?

No, and we say so plainly. Two known gates:

1. **Smart-contract audit** — vault + `Buffer.sol` + `mppsol_cpi` need third-party audit (~$50–150K, 6–8 weeks)
2. **CCIP-on-Tempo activation** — when CCIP ships on the Tempo side (waiting on Chainlink), soltempo switches the keeper from "trusted relay" to in-program `ccip_send` (instruction shape already prepared; only an address-swap from off-chain to on-chain rail)

Everything else (state machine, account constraints, drift catchers, Kamino CPI, Receipt PDAs, canonical 122-byte intent encoding) is production-shaped. Mainnet deployment requires audit and a multisig upgrade-authority transfer, **not a code rewrite.**

## Why is this the right hackathon for soltempo?

The Solana Frontier hackathon judges on:

1. **Functionality / code quality** — soltempo runs a real end-to-end cross-VM cycle on live testnets, with drift-catcher tests covering Kamino, mppsol_cpi, and CCIP discriminators
2. **Potential Impact / TAM / Solana ecosystem impact** — ~4M Stripe merchants, ~$20–40B idle balance, 1% TAM penetration = $2–4M run-rate
3. **Novelty** — first product to compose Tempo's payment distribution with Solana's DeFi depth, with on-chain Receipt PDAs as the audit primitive
4. **UX leveraging Solana's performance** — Solana's 400ms finality and 5,000-lamport settlement cost is the entire business case
5. **Open-source / composability with Solana primitives** — real 17-account CPI into Kamino, exposes `request_pullback_to_tempo` and `CrossVMIntent` as reusable primitives, drift-catcher tests for fork safety
6. **Business Plan viability** — performance fee model (10–20%), distribution pre-solved via Tempo, no incumbent to displace

soltempo doesn't fight on Solana's DeFi turf — it **expands Solana's market boundary into the Stripe merchant base.** That's a category win for the ecosystem, not a feature.
