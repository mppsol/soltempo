# Frontier submission — soltempo

Canonical source for the Colosseum Frontier submission form. Fields below are organized to mirror the form's 4-step structure. Copy-paste blocks are inside fenced code blocks.

**Deadline:** 2026-05-11 11:59pm PT.
**Hackathon:** Solana Frontier (Colosseum).
**Decision (2026-05-11):** Submitting **soltempo** as the single entry (one slot per individual). Earlier MPP.sol draft superseded.

---

## TODO before submit

- [ ] Replace project logo upload (currently MPP.sol's `logo.png`). Fallback: soltempo wordmark or fabrknt logo. Max 3MB, JPG/PNG/WEBP.
- [ ] Upload `demo-video/output/demo-v0.3-terminal.mp4` to YouTube (unlisted) — paste URL into "Demo video".
- [ ] Finish pitch video (VO recording against the 6 scene clips in `demo-video/output/scenes/`, recombine + mux audio + optional B-roll/music), upload to YouTube — paste URL into "Pitch video".
- [ ] Walk the form top-to-bottom replacing every MPP.sol text field with the soltempo blocks below.
- [ ] Submit form with 3–4 hours of buffer before 11:59pm PT.

---

## Step 1 / 4

### Project name *(Public)*
```
soltempo
```

### Brief description *(Public, ≤500 chars)*
```
Stripe merchants earn $0 on ~$20–40B of idle USDC. Tempo (Stripe + Paradigm's L1, mainnet 2026-03-18, Visa-anchored) brings them on-chain — but has no native yield layer.

soltempo delivers it: a cross-VM yield account routing idle merchant USDC into Kamino via real 17-account CPI, with on-chain Receipt PDAs binding settlements to Tempo origin. ~$0.001 per cycle — viable from $10K balances.

First proof that Solana is where payment merchants earn yield. Live on devnet.
```

### Project website *(Public)*
```
https://github.com/mppsol/soltempo
```
*(Repo URL since soltempo has no dedicated landing page. Leave blank if this field is optional and feels redundant with the GitHub link in Step 2.)*

### What are you building, and who is it for? *(≤1000 chars)*
```
soltempo is a working cross-VM yield account that routes idle USDC from Tempo merchants (Stripe + Paradigm's payment L1, mainnet 2026-03-18, Visa-anchored) into Solana DeFi (Kamino) — and pulls it back on demand for merchant payouts.

Three on-chain pieces:
- Solana Anchor vault (devnet): receives canonical 122-byte cross-VM intents via Chainlink CCIP, allocates USDC into Kamino's USDC reserve via real 17-account CPI, exposes pull-back instructions for payout flows.
- mppsol_cpi.pay_with_receipt: emits on-chain Receipt PDAs bound to each Tempo origin nonce — auditable cross-VM trail, no opaque off-chain ledger.
- Buffer.sol on Tempo Moderato: merchant-facing deposit + bridge contract for cross-VM payment intents.

For: Stripe-grade payment merchants holding USDC operating balances on Tempo (post-mainnet adoption), and the keeper/treasury operators settling payouts on their behalf.
```

### Why did you decide to build this, and why build it now? *(≤1000 chars)*
```
Stripe merchants today earn 0% on operating balances. Across ~4M Stripe merchants processing $1T+ annually, an estimated $20–40B sits idle at any moment. None earns yield because integrating DeFi has been operationally untenable — wrong UX, wrong tools, wrong fees.

Tempo launched mainnet 2026-03-18 with Stripe + Paradigm as cofounders and Visa as anchor validator. The merchant base is coming on-chain. Tempo has no native yield layer, and the yield layer has to be on Solana — the only L1 satisfying sub-second finality + cents-per-tx settlement + depth-of-market stablecoin lending + mature DeFi infrastructure.

Why now: Tempo is on mainnet. Chainlink CCIP activated on Tempo 2026-05-08 — the moment the rail existed. The window to be the canonical Tempo→Solana yield primitive is open exactly once. I took it.
```

### What technologies are you using or integrating with?
```
Anchor 0.32.1, Solana CLI 3.1.14 (platform-tools v1.52, rustc 1.89), Solana Web3.js, SPL Token, Kamino klend (USDC reserve, real 17-account CPI), mppsol_cpi (Anchor — Receipt PDA settlement primitive, devnet), Chainlink CCIP (Tempo↔Solana rail), Foundry (Solidity tests on Buffer.sol), Reth-based Tempo Moderato testnet, Next.js + TypeScript (merchant-web dashboard), Puppeteer (E2E browser-driven demo + video capture), Mocha + chai + ts-mocha (Anchor tests), drift-catcher tests (re-derive Kamino/CCIP/mppsol_cpi discriminators at test time), GitHub Actions CI, Claude Code (Opus 4.7, 1M context).
```

### Category *(Public)*
**Payments & Remittance**

*(Soltempo's surface is merchant-payments, not crypto-native DeFi. Lead with the merchant framing. Alt: "DeFi" or "Cross-Chain" if either is a dropdown option that fits better.)*

### Is your project a mobile-focused dApp?
`No`

---

## Step 2 / 4

### Project logo *(Public)*
**TODO** — replace MPP.sol's `logo.png`. Fallback: soltempo wordmark or fabrknt logo. Max 3MB, JPG/PNG/WEBP.

### GitHub link *(Public)*
```
https://github.com/mppsol/soltempo
```

### Repo context *(≤500 chars)*
```
soltempo monorepo: Anchor vault (Solana devnet), Foundry Buffer.sol (Tempo Moderato), Next.js merchant-web dashboard, keeper service, drift-catcher tests against Kamino + CCIP + mppsol_cpi discriminators.

Composes upstream: Kamino klend (real 17-account deposit CPI) and mppsol_cpi (github.com/mppsol/cpi — Receipt PDA primitive). Both with drift catchers re-deriving discriminators at test time.

37 vault tests + 9 Foundry tests passing. Apache-2.0.
```

### Demo video URL *
**TODO** — upload `demo-video/output/demo-v0.3-terminal.mp4` to YouTube (unlisted), paste URL here. Keep "Make demo video public in the project directory" checked.

YouTube metadata for this upload:

**Title:**
```
soltempo — Solana DeFi yield for Tempo merchants (live cross-VM demo)
```

**Description:**
```
soltempo is a working cross-VM yield account that routes idle USDC from Tempo merchants (Stripe + Paradigm's payment L1, mainnet 2026-03-18) into Solana DeFi (Kamino), and pulls it back on demand for payouts. Each settlement emits an on-chain Receipt PDA via mppsol_cpi, binding the Solana-side payout to its Tempo origin nonce.

Why Solana: a full settle cycle costs ~5,000 lamports (~$0.001) at 400ms finality. The same flow on Ethereum L1 would cost $5–50 and break the unit economics. Solana is the only L1 where Stripe-grade merchant volume can be served profitably.

Live end-to-end on Solana devnet + Tempo Moderato testnet today.

Submitted to the Solana Frontier hackathon (Colosseum), May 2026.

Links
• Vault program (Solana devnet): https://explorer.solana.com/address/2YhYmfCoCj3VvyN2HQ3cuavMiZzEUdUTrhvo6nmGRXe3?cluster=devnet
• Sample cross-VM payout tx: https://explorer.solana.com/tx/24CJ82MhCn7W1pfxjWAcgevPo6MWxrRT575Pb76KzPMawwNKAg7bV6LA7cncLqmYh6qZ6ifmF8EqZBFybfaQmK5d?cluster=devnet
• GitHub: https://github.com/mppsol/soltempo

Chapters (confirm timestamps after upload)
0:00 Opening
~0:25 Cross-VM cycle (terminal screencast)
~2:10 Kamino allocation proof
~2:45 Closing
```

### Live product link
```
https://explorer.solana.com/address/2YhYmfCoCj3VvyN2HQ3cuavMiZzEUdUTrhvo6nmGRXe3?cluster=devnet
```
*(Deployed vault program on Solana devnet — judges can inspect on-chain. Alt: leave blank — merchant-web is local-only.)*

### Access instructions
```
All on-chain artifacts are public on Solana devnet and Tempo Moderato testnet — no credentials needed.

Vault program: 2YhYmfCoCj3VvyN2HQ3cuavMiZzEUdUTrhvo6nmGRXe3 (Solana devnet)
Vault PDA: 8smibhXARvuYGEadHqc9C9tqTJWFmLUdXAtkabtFaA9M
Buffer contract: 0xe8c675523AFd81587c35Da2BeF6ECc268654D0BE (Tempo Moderato)
Sample cross-VM payout tx: 24CJ82MhCn7W1pfxjWAcgevPo6MWxrRT575Pb76KzPMawwNKAg7bV6LA7cncLqmYh6qZ6ifmF8EqZBFybfaQmK5d (Solana devnet)

To run the full cross-VM cycle locally: see DEMO.md in the repo. To run merchant-web locally: cd apps/merchant-web && pnpm dev.
```

### Pitch video *(Public)*
**TODO** — finish VO recording against `demo-video/output/scenes/`, recombine + mux audio (+ optional music/founder face), upload to YouTube, paste URL here.

YouTube metadata for this upload:

**Title:**
```
soltempo — Mercury Treasury for Stripe merchants (Solana Frontier pitch)
```

**Description:**
```
2-minute pitch for soltempo — submitted to the Solana Frontier hackathon (Colosseum), May 2026.

The thesis in one line: Solana is where the next wave of payment merchants earns yield, and soltempo is the first proof.

Stripe merchants today earn $0 on operating balances. Across ~4M Stripe merchants processing $1T+ annually, an estimated $20–40B sits idle at any moment. Tempo — Stripe and Paradigm's Reth-based payment L1, mainnet 2026-03-18, anchored by Visa — brings these merchants on-chain. soltempo connects that merchant base to Solana DeFi yield.

Why it has to be Solana: a full cross-VM settle cycle costs ~5,000 lamports (~$0.001) at 400ms finality. The same flow on Ethereum L1 would cost $5–50 and break the unit economics from $10K merchant balances. Solana is the only L1 where Stripe-grade merchant volume can be served profitably.

The footage in this pitch is real — live on Solana devnet + Tempo Moderato testnet today.

For the full end-to-end terminal demo, see: [PASTE DEMO YOUTUBE URL]

Links
• GitHub: https://github.com/mppsol/soltempo
• Vault program (Solana devnet): https://explorer.solana.com/address/2YhYmfCoCj3VvyN2HQ3cuavMiZzEUdUTrhvo6nmGRXe3?cluster=devnet
• Sample cross-VM payout tx: https://explorer.solana.com/tx/24CJ82MhCn7W1pfxjWAcgevPo6MWxrRT575Pb76KzPMawwNKAg7bV6LA7cncLqmYh6qZ6ifmF8EqZBFybfaQmK5d?cluster=devnet
• 60-second Tempo briefing: https://github.com/mppsol/soltempo/blob/main/TEMPO-FAQ.md
```

---

## Step 3 / 4

### Where is your team primarily based? *(Public)*
`Japan`

### Team members
`Hiroyuki Saito @psyto`

### Did anyone not listed do meaningful work? *(≤600 chars)*
```
Claude Code (Opus 4.7, 1M context) was used extensively as an AI pair programmer for code generation, spec drafting, test writing, and debugging across the Anchor vault program, the Foundry-based Buffer.sol on Tempo, the Next.js merchant-web dashboard, the keeper service, and the drift-catcher test suite. All architectural decisions — the canonical 122-byte cross-VM intent encoding, the Chainlink CCIP integration approach, the composition with mppsol_cpi for Receipt PDAs, the 17-account Kamino klend CPI layout — are by the human founder.
```

### Telegram contact
`psyto69`

### X profile *(Public)*
`psyto`

### Anything else judges should know? *(≤500 chars)*
```
soltempo is production-shaped, not toy-shaped. The cross-VM cycle, 17-account Kamino CPI, 122-byte canonical intent encoding, and Receipt PDA primitive all mirror upstream layouts exactly — drift catchers re-derive every external discriminator at test time, so Kamino/CCIP forks can't silently break us.

Mainnet is gated on audit + multisig + CCIP-on-Tempo activation — not a code rewrite. soltempo composes mppsol_cpi (sibling Anchor primitive) and Kamino's klend as real CPIs. No mocks.
```

### Accelerator
`Yes` — applying

---

## Step 4 / 4 — Accelerator

### How do you know people actually need this product? *(≤1000 chars)*
```
The demand signal is structural, not speculative.

Stripe processes ~$1T annually across ~4M merchants. Industry-standard operating-balance ratios put $20–40B of merchant USDC idle at any moment, earning 0%. Every previous attempt to bridge those balances into DeFi has failed for operational reasons (UX, fees, custody) — not absence of demand.

Adjacent demand signals are loud:
- Mercury Treasury and Brex earn fees by sweeping idle USD operating balances into MMFs. Same merchant impulse, lower yield, no on-chain composability.
- Tempo's L1 launched 2026-03-18 with Stripe + Paradigm as cofounders, Visa as anchor validator, and 100+ services in the payments directory at mainnet day one. The merchant pull is there.
- Chainlink CCIP activated on Tempo 2026-05-08. The rail to Solana exists. The yield layer doesn't.

soltempo is the yield layer. The chain has to be Solana — only L1 where the unit economics work at $10K merchant balances upward.
```

### How far along are you? Do you have users? *(≤1000 chars)*
```
Solo founder, ~4 weeks during the hackathon. Two repos shipped: github.com/mppsol/soltempo (vault + Buffer + keeper + merchant-web) and github.com/mppsol/cpi (Receipt PDA primitive).

Shipped:
- Vault Anchor program deployed Solana devnet — 8 instructions, IDL upgraded, vault PDA 8smibhXARvuYGEadHqc9C9tqTJWFmLUdXAtkabtFaA9M
- Buffer.sol deployed Tempo Moderato — 0xe8c675523AFd81587c35Da2BeF6ECc268654D0BE
- 37 vault unit tests + 9 Foundry tests passing
- Full bidirectional cross-VM cycle (deposit → CCIP → Kamino allocation → pull-back → settle) live on testnets — sample payout tx 24CJ82M…mK5d
- Drift-catcher tests against Kamino, mppsol_cpi, and CCIP discriminators re-derived at test time
- Next.js merchant-web dashboard for deposit/withdraw UX (Puppeteer-driven E2E demo)

Users: pre-launch (Tempo is days into mainnet; soltempo is devnet). First targets: Tempo merchants holding ≥$10K USDC balances. GTM: direct merchant outreach + Tempo DevRel post-audit.
```

### Who else is building in this space? *(≤1000 chars)*
```
Three adjacent categories, none of them this:

1. Solana yield aggregators (Drift Earn, Marginfi Lend, Kamino Vaults): solve yield optimization for Solana-native USDC. Don't bridge merchant payment volume from off-chain. No cross-VM rail.

2. Cross-chain bridges (LayerZero, Wormhole, CCTP): generic message passing. CCTP doesn't cover Tempo yet; Wormhole hasn't added Tempo; LayerZero hasn't added Tempo. Only Chainlink CCIP has a Tempo rail today (activated 2026-05-08). None bundle a yield destination.

3. Stripe-native DeFi attempts (Mountain Protocol USDM, Origin Dollar OUSD on EVM): yield-bearing stablecoins for treasury use. Wrong unit economics for merchant volume on Ethereum L1 ($5–50/cycle); no Stripe merchant distribution channel.

The empty intersection: a yield account composing Tempo's merchant distribution + Solana's DeFi depth, with on-chain audit trail. soltempo fills it. The shared miss is the chain assumption — Solana's fee structure is the entire business case.
```

### How do you make money? *(≤500 chars)*
```
Performance fee on yield generated. Industry-standard 10–20% take rate on net yield delivered to merchants — avoids asset-management regulatory framing (no AUM fees, no custody, no advice).

Free: vault program, drift catchers, mppsol_cpi composition (Apache-2.0). Paid:
- Hosted keeper / cross-VM relayer infrastructure
- Multi-venue allocation (Marginfi, Drift) post-v1.0
- Merchant treasury dashboards + API

Stripe-grade merchant scale + low Solana take per cycle = volume-driven, not extractive.
```

### How long have you been working on this? *(≤500 chars)*
```
Solo founder, ~4 weeks during the hackathon. Not full-time — Head of Sales Engineering at SBI R3 Japan is the day job.

Built on prior work:
- 15+ years payments engineering at Shinsei Bank (Zengin, FATCA-KYC, Flexcube core, mortgage onboarding)
- 4+ months running perp vaults on Drift and Hyperliquid mainnet
- 3rd Place — Solana Cypherpunk Hackathon (NTT Docomo R&D track)

Tempo mainnet launched 2026-03-18; CCIP activated 2026-05-08; soltempo built straight into the window.
```

### Where is each team member based? *(≤500 chars)*
```
Solo founder, based in Tokyo, Japan.

Currently Head of Sales Engineering at SBI R3 Japan — 9 years' continuous Tokyo presence. Working on soltempo remotely, part-time.

Open to relocating or establishing presence in Singapore, Hong Kong, or Dubai for regulatory clarity if accelerator-funded. Prior international experience: Hong Kong (2 yrs startup), India (2 yrs offshore engineering at iGate / Capgemini).

All infrastructure cloud-based; no in-person team requirement.
```

### Legal entity / Investment / Fundraising / Live token
Personal-factual yes/no answers — unchanged from the original MPP.sol draft. Re-verify they still match for soltempo before submitting.

---

## X post — for the submission moment

```
Just submitted soltempo to Solana Frontier 🟣

$20–40B of Stripe merchant USDC sits idle at 0%. soltempo routes it into Solana DeFi (Kamino), pulls back on demand for payouts. Full cross-VM cycle costs ~$0.001 — only Solana makes the unit economics work.

Pitch ↓
[PITCH YOUTUBE URL]
```

Optional thread:
- **Reply 1 — proof:** demo link + technical proof (3-min terminal demo, every tx verifiable on public block explorers)
- **Reply 2 — repo:** github.com/mppsol/soltempo + Tempo FAQ link

Post within ~15 min of form submission so "just submitted" framing is honest. Pin to profile for the judging window.

Tags to consider (verify handles before posting): @solana, @KaminoFinance, @colosseum. Skip any you're not confident on — wrong handle is worse than no handle.

---

## Pitch points (for any judge Q&A or freeform fields)

1. **Distribution is pre-solved.** Every Stripe merchant adopting Tempo is a soltempo prospect by default. Plug into Stripe-grade onboarding, not crypto-native acquisition.
2. **Replaces a $0 baseline.** Even 4% net yield (after take rate) is unambiguously additive. No incumbent to displace.
3. **Solana is the only chain that makes this profitable.** $0.001/cycle vs $5–50 on Ethereum L1. Solana's fee structure is the entire business case.
4. **Real CPI, not abstraction.** 17-account `deposit_to_kamino` ix mirrors klend's upstream layout exactly. Drift-catcher tests re-derive every external discriminator at test time.
5. **Auditable on-chain.** Every settlement emits a Receipt PDA via `mppsol_cpi.pay_with_receipt`, binding the Solana payout to its Tempo origin nonce.
6. **Production-shaped, not toy.** When CCIP ships on Tempo mainnet, swap one address. Mainnet wants audit + multisig — not a rewrite.

---

## Team

| Role | Person |
| --- | --- |
| Solo founder | Hiro Saito (psyto) — saito.hiroyuki@gmail.com |
| Background | Ex-Shinsei Bank, Fabrknt founder, Solana DeFi (Yogi/Nanuk/Syntx), learning Reth/Alloy via Telos |

---

## Key links

| Field | Value |
| --- | --- |
| GitHub repo | https://github.com/mppsol/soltempo |
| Live program (Solana devnet) | https://explorer.solana.com/address/2YhYmfCoCj3VvyN2HQ3cuavMiZzEUdUTrhvo6nmGRXe3?cluster=devnet |
| Sample cross-VM payout tx | https://explorer.solana.com/tx/24CJ82MhCn7W1pfxjWAcgevPo6MWxrRT575Pb76KzPMawwNKAg7bV6LA7cncLqmYh6qZ6ifmF8EqZBFybfaQmK5d?cluster=devnet |
| Tempo Buffer contract | `0xe8c675523AFd81587c35Da2BeF6ECc268654D0BE` (Tempo Moderato) |
| Tempo FAQ for judges | https://github.com/mppsol/soltempo/blob/main/TEMPO-FAQ.md |

---

## What to do if the form asks something not covered here

- **"How does this leverage Solana?"** — lead with the 5,000-lamport settlement number. Same flow on Ethereum L1 = $5–50, breaks unit economics. Solana is the only L1 where this works.
- **"Who is the target user?"** — Tempo merchants holding USDC balances. Specifically post-mainnet Stripe merchants who adopt Tempo for settlement. TAM: ~$20–40B idle across ~4M Stripe merchants.
- **"What is the business model?"** — performance fee on yield (10–20%, industry standard). Avoids asset-management regulatory framing.
- **"Why is this hackathon-relevant?"** — see "Why is this the right hackathon for soltempo?" in TEMPO-FAQ.md — maps 1:1 onto Frontier's six judging criteria.
- **"Anything special judges should know?"** — Solana's fee structure is the entire business case. This product doesn't exist on any other L1.
