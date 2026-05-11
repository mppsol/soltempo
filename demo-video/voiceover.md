# soltempo — voiceover script (segment-mapped)

Maps voice lines to each video segment. Stage 3 was tightened to fit its 5-second clip; everything else is well within budget.

The v0.3 combined cut inserts the **Kamino proof** segment between Stage 5 and Closing — adds the yield-leg + pull-back narrative without re-shooting any of the cross-VM cycle.

| Segment | File | Duration | Words | WPM |
| --- | --- | --- | --- | --- |
| Opening | `output/opening.mp4` | 29s | 67 | 138 |
| Stage 1 | `output/stages/stage-1-keeper-boots.mp4` | 22s | 14 | 38 (mostly silent — sleep is 18s) |
| Stage 2 | `output/stages/stage-2-merchant-deposits.mp4` | 11s | 8 | 44 |
| Stage 3 | `output/stages/stage-3-bridge-fires.mp4` | 5s | 11 | 132 |
| Stage 4 | `output/stages/stage-4-keeper-relays.mp4` | 20s | 35 | 105 |
| Stage 5 | `output/stages/stage-5-verify-state.mp4` | 7s | 5 | 43 |
| **Kamino (v0.3)** | `output/kamino-proof.mp4` | 45s | 103 | 138 |
| Closing | `output/closing.mp4` | 47s | 110 | 140 |
| **Total (v0.2 cut)** | `output/demo-3min.mp4` | 141s | 250 | 106 |
| **Total (v0.3 cut)** | (assemble: opening + stages 1–5 + kamino + closing) | 186s | **353** | **114** |

---

## Opening (29s) — `voiceover/opening.txt`

> **"This is soltempo. Stripe merchants earn zero on idle USDC — tens of billions sitting still. Solana is the only chain that can serve them profitably: sub-second finality at a thousandth of a cent per cycle.**
>
> **Watch a real merchant on Tempo — Stripe and Paradigm's L1 — bridge USDC to Solana, allocate into Kamino, settle atomically. Every transaction verifiable on-chain. Two real testnets. Recording starts now."**

*Revised for Solana Frontier judges (2026-05-11). Leads with the merchant pain ($0 baseline + $20–40B idle), pivots to "only Solana solves this" with two killer numbers (400ms finality + $0.001/cycle), then cues the live demo. Two scenes instead of three. Land "Solana is the only chain" hard — that's the thesis. Scene 2 ends with "Recording starts now" as cue-into-terminal.*

---

## Stage 1 — keeper boots (22s) — `voiceover/stage-1.txt`

> **"The keeper observes Tempo events and relays to Solana. It boots in the background."**

*Mostly silent stage — the script's `sleep 18` dominates. Land this line ~3 seconds after the stage starts, then leave the rest silent for the keeper banner to print at second 21. Almost an aside; don't compete with the visible terminal.*

---

## Stage 2 — merchant deposits (11s) — `voiceover/stage-2.txt`

> **"Merchant approves the buffer contract. Deposits 1000 pathUSD."**

*Two beats. Pause briefly between sentences so each `cast send` flash on screen lines up with its own line.*

---

## Stage 3 — bridge fires (5s) — `voiceover/stage-3.txt`

> **"Cross-VM bridge fires. Buffer keeps 100, emits intent for 900."**

*Tight beat — 11 words in 5s requires brisk delivery (~130 wpm). Originally a longer line in earlier drafts; trimmed to fit Stage 3's short clip.*

---

## Stage 4 — keeper relays (20s) — `voiceover/stage-4.txt`

> **"Keeper picks up the event. Transfers Solana-side USDC into the vault PDA. Calls trusted_keeper_receive — which validates the source chain, the sender bytes, and the cross-VM intent format before updating state atomically."**

*The longest narration block. Each clause its own cadence. The terminal will be silent for ~5s at the start (keeper polling), then SPL transfer + trusted_keeper_receive sigs flash — voice describes what just printed.*

---

## Stage 5 — verify state (7s) — `voiceover/stage-5.txt`

> **"On-chain proof: vault.total_deposits incremented exactly."**

*Confident period-stop. Brief silence after to let the on-screen number land before cutting to the closing.*

---

## Kamino (v0.3 — 45s) — `voiceover/kamino.txt`

> **"v0.3 adds the yield leg — real CPI into Kamino, then a round-trip back to Tempo. Klend's deposit instruction is wired by hand — seventeen accounts in upstream's exact order, every discriminator re-derived at test time. Fifteen tests catch drift the moment Kamino renames anything.**
>
> **All three new instructions are live on the upgraded devnet program. Deposit to Kamino. Init the Kamino obligation. Request pull-back to Tempo.**
>
> **The pull-back path is already running — request signed by the vault PDA, PullbackRequested event carrying the canonical intent for the keeper to settle on Tempo.**
>
> **Kamino itself ships on mainnet only — so production runs on localnet with klend cloned via one helper script."**

*Three terminal beats — tests passing, instructions list, on-chain pull-back tx. The narration tracks the visible blocks but doesn't have to land each line at its block; the segment is dense enough that 138 WPM still leaves breathing room. Solana-judge polish (2026-05-11): "real CPI into Kamino" leads the segment, and "vault PDA" replaces the generic "vault authority" — both anchor on Solana-native primitives. Land "wired by hand", "real CPI", and "live on the upgraded devnet program" hard — those are the technical-credibility hooks.*

---

## Closing (47s) — `voiceover/closing.txt`

> **"That was one full cross-VM cycle. Tempo merchant USDC settled atomically on Solana — bridge to receipt — in under twenty seconds.**
>
> **Vault state readable directly via `solana account` at offset 0x88. No SDK required.**
>
> **Four real transactions. Two on Tempo, two on Solana. Every hash on screen is verifiable on a block explorer right now.**
>
> **Production-shaped architecture. Same vault, same primitives, same CPI patterns. When CCIP ships on Tempo: one address swap. Mainnet wants audit and multisig — not a rewrite.**
>
> **Solana is where the next wave of payment merchants earns yield. soltempo is the first proof, live on devnet."**

*Five scenes, ~9s each. Solana-judge polish (2026-05-11): Scene 2 now explicitly highlights `solana account` direct-read with "No SDK required" — composability proof. Scene 4 adds "same CPI patterns" to reinforce the Solana-side primitive language. Scene 5 is the new bookend hook: "Solana is where the next wave of payment merchants earns yield" — matches the README's headline thesis and replaces the generic "Stripe-grade payments meet Solana DeFi" line. Slow, declarative — closes the loop with the opening.*

---

## How to use this

### Option A — single voice track for the combined video

`voiceover-plain.txt` is the continuous script for the **v0.3 cut** (opening + stages 1–5 + kamino + closing, 186s). TTS-render once, mux with the combined MP4:

```sh
say -v "Daniel" -r 145 -f voiceover-plain.txt -o voiceover.aiff
ffmpeg -i voiceover.aiff -i output/demo-v0.3.mp4 \
  -map 0:a -map 1:v -c:v copy -shortest demo-v0.3-with-voice.mp4
```

For the older v0.2 cut (no kamino segment), trim the kamino paragraph out of `voiceover-plain.txt` first or use the per-segment Option B.

### Option B — per-segment voice tracks

Use the per-segment files in `voiceover/`. TTS-render each one, mux with its corresponding clip. Useful if you want different voices per stage, or want to record manually one stage at a time.

```sh
for seg in opening stage-1 stage-2 stage-3 stage-4 stage-5 kamino closing; do
  say -v "Daniel" -r 145 -f voiceover/${seg}.txt -o voiceover/${seg}.aiff
done
```

Then mux each with its corresponding video clip via the same `ffmpeg` pattern.

### Voice picks

- **macOS `say`**: `-v "Daniel"` (British male, clear) or `-v "Samantha"` (American female). `-r 145` is comfortable.
- **ElevenLabs**: drop `voiceover-plain.txt` (or any per-segment file) into Speech Synthesis, pick "Adam" / "Bella" / similar calm voice. Render at 0.95-1.0x.
- **Coqui XTTS**: open-source local TTS; same plain text input.
