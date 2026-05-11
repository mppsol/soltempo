# Frontier submission helper

Pre-filled fields for the Colosseum Frontier submission form (colosseum.com).

**Deadline:** 2026-05-11 11:59pm PT.

---

## Project name

```
soltempo
```

## Tagline (~140 chars, for cards/listings)

Option A (75 chars):
```
Solana DeFi yield for Tempo merchants. Live cross-VM demo on devnet.
```

Option B (139 chars):
```
Solana is where the next wave of payment merchants earns yield. soltempo is the first proof — live cross-VM cycle on devnet today.
```

Option C (133 chars, hook + tech):
```
soltempo bridges idle USDC from Tempo merchants into Kamino via real 17-account CPI. Receipt PDAs make every settlement auditable.
```

## Short description (~280 chars)

```
Stripe merchants earn $0 on $20–40B of idle USDC. soltempo turns that into Solana DeFi yield — real CPI into Kamino, on-chain Receipt PDAs via mppsol_cpi, Chainlink CCIP for the Tempo↔Solana rail. Solana is the only L1 where this is unit-economically viable. Live on devnet.
```

## Medium description (~500 chars)

```
soltempo is a working cross-VM yield account that routes idle USDC from Tempo merchants (Stripe + Paradigm's payment L1) into Solana DeFi (Kamino), and pulls it back on demand for payouts. Each settlement emits an on-chain Receipt PDA via mppsol_cpi, binding the Solana-side payout to its Tempo origin. Solana is the only L1 where Stripe-grade merchant volume works profitably: 400ms finality, ~5,000 lamports (~$0.001) per cycle. Live end-to-end on Solana devnet + Tempo Moderato testnet today.
```

## Long description (1–2 paragraphs)

```
Stripe merchants today earn 0% on operating balances. Across ~4M Stripe merchants processing $1T+ annually, an estimated $20–40B sits idle at any moment. Integrating DeFi has been operationally untenable — wrong UX, wrong tools, wrong fees. soltempo solves this by giving merchants a single button: deposit idle USDC on Tempo, earn yield on Solana, pull back on demand.

Behind the button: a Solana Anchor vault receives canonical 122-byte cross-VM intents via Chainlink CCIP, allocates USDC into Kamino's USDC reserve via real 17-account CPI, and on payout invokes `mppsol_cpi.pay_with_receipt` to emit an on-chain Receipt PDA bound to the cross-VM nonce. Solana is essential, not interchangeable: at ~5,000 lamports per settlement, the unit economics work from $10K merchant balances upward. The same flow on Ethereum L1 would cost $5–50/cycle and break the model. 37 vault unit tests + drift-catcher tests against Kamino, mppsol_cpi, and CCIP discriminators ensure the on-chain layouts can't silently drift from upstream. Full cycle live today on Solana devnet + Tempo Moderato — every tx verifiable on public block explorers.
```

## Tech stack tags

```
Solana, Anchor, Rust, Kamino (klend), Chainlink CCIP, Tempo, Reth,
EVM, Solidity, Foundry, TypeScript, Next.js, mppsol_cpi
```

## Key links

| Field | Value |
| --- | --- |
| GitHub repo | https://github.com/mppsol/soltempo |
| Live program (Solana devnet) | https://explorer.solana.com/address/2YhYmfCoCj3VvyN2HQ3cuavMiZzEUdUTrhvo6nmGRXe3?cluster=devnet |
| Sample cross-VM payout tx | https://explorer.solana.com/tx/24CJ82MhCn7W1pfxjWAcgevPo6MWxrRT575Pb76KzPMawwNKAg7bV6LA7cncLqmYh6qZ6ifmF8EqZBFybfaQmK5d?cluster=devnet |
| Tempo Buffer contract | `0xe8c675523AFd81587c35Da2BeF6ECc268654D0BE` (Tempo Moderato) |
| Demo video (terminal cut, 3:05) | **TODO: upload `demo-video/output/demo-v0.3-terminal.mp4` to YouTube** |
| Demo video (browser cut, 2:47) | **TODO: upload `demo-video/output/demo-v0.3.mp4` to YouTube** |
| Pitch deck | `demo-video/pitch.html` — host on GitHub Pages if needed |
| Related project: mppsol | https://mppsol.org |

## Pitch points (use as bullet talk-tracks)

1. **Distribution is pre-solved.** Every Stripe merchant adopting Tempo is a soltempo prospect by default. We plug into Stripe-grade onboarding, not crypto-native acquisition.
2. **The product replaces a $0 baseline.** Even 4% net yield (after take rate) is unambiguously additive. No incumbent to displace.
3. **Solana is the only chain that makes this profitable.** $0.001/cycle vs $5–50 on Ethereum L1. Solana's fee structure is the entire business case.
4. **Real CPI, not abstraction.** 17-account `deposit_to_kamino` ix mirrors klend's upstream layout exactly. Drift-catcher tests re-derive every external discriminator at test time.
5. **Auditable on-chain.** Every settlement emits a Receipt PDA via `mppsol_cpi.pay_with_receipt`, binding the Solana payout to its Tempo origin nonce.
6. **Production-shaped, not toy.** When CCIP ships on Tempo, swap one address. Mainnet wants audit + multisig — not a rewrite.

## Team

| Role | Person |
| --- | --- |
| Solo founder | Hiro Saito (psyto) — saito.hiroyuki@gmail.com |
| Background | Ex-Shinsei Bank, Fabrknt founder, Solana DeFi (Yogi/Nanuk/Syntx), learning Reth/Alloy via Telos |

## Submission checklist

Before clicking submit:

- [ ] Register/confirm individual entry on colosseum.com (registration deadline was 2026-05-04)
- [ ] **Upload demo video v0.3 to YouTube (unlisted is fine)** — terminal cut is the primary; browser cut as alternate
- [ ] Pitch deck hosted publicly (GitHub Pages from `demo-video/pitch.html` if not already)
- [ ] GitHub repo public + main branch reflects latest commit (`292c42d` or newer)
- [ ] Repo README first paragraph reads as Solana-judge-targeted (verified — rewritten 2026-05-11)
- [ ] `TEMPO-FAQ.md` linked from README so judges can self-serve on Tempo context
- [ ] All on-chain links resolve (vault program, vault PDA, sample tx)
- [ ] Tagline + descriptions copy-pasted into form fields
- [ ] Tech stack tags filled in
- [ ] Logo (if required by form) — use mppsol/soltempo wordmark or fabrknt logo
- [ ] Hit submit before 2026-05-11 11:59pm PT

## What to do if the form asks something not covered here

- **"How does this leverage Solana?"** — quote the "Why Solana, specifically" table from README. Lead with the 5,000-lamport settlement number.
- **"Who is the target user?"** — Tempo merchants holding USDC balances. Specifically, post-mainnet Stripe merchants who adopt Tempo for settlement. TAM section in README has the numbers.
- **"What is the business model?"** — performance fee on yield earned (10–20% of yield, industry standard). Avoids asset-management regulatory framing.
- **"Why is this hackathon-relevant?"** — see "Why is this the right hackathon for soltempo?" in TEMPO-FAQ.md — maps 1:1 onto Frontier's six judging criteria.
- **"Anything special judges should know?"** — Solana's fee structure is the entire business case. This product doesn't exist on any other L1.
