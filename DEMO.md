# soltempo demo recording playbook

How to record a clean ~90-second demo video of the v0.2 cross-VM flow against the [reference deployment](DEPLOY.md#reference-deployment-verified-end-to-end-on-2026-05-09).

## Setup

**Screen recorder:** macOS built-in works fine — `Cmd-Shift-5` → Record Selected Portion. Capture at 1920×1080 or 1280×720 (your call).

**Terminal layout** (recommended): two side-by-side panes in iTerm2 or Terminal.app
- **Left pane** — keeper log
- **Right pane** — `cast send` commands + final state checks

If you want a single pane, that's fine too — the demo script's section headers create natural visual breakpoints.

## Pre-flight (do once, NOT recorded)

Make sure the keeper isn't already running:
```sh
pkill -f "tsx src/index.ts" 2>/dev/null || true
```

Set the merchant/keeper Tempo private key in your shell:
```sh
export TEMPO_KEEPER_PRIVATE_KEY=0x<your tempo wallet private key>
```

If you want to start from a clean Buffer state (useful for the recording — Buffer balance == bufferTarget exactly), do nothing — the demo script approves and deposits fresh each run, regardless of starting state.

## Recording — left pane (keeper)

```sh
./scripts/keeper.sh
```

Wait ~3s for the startup banner to print. The keeper now polls the Tempo router every 5s for new MockMessageSent events.

## Recording — right pane (demo flow)

```sh
./scripts/demo.sh
```

The script has 5 visually-separated sections (cyan header bars):
1. **BEFORE STATE** — Buffer balance, vault USDC ATA balance
2. **STEP 1 — MERCHANT APPROVES BUFFER** — `cast send approve(...)`
3. **STEP 2 — MERCHANT DEPOSITS pathUSD** — `cast send Buffer.deposit(...)`
4. **STEP 3 — TRIGGER CROSS-VM BRIDGE** — `cast send Buffer.sendIntentToSolana()`
5. **WAITING FOR KEEPER (~10s)** — pause for the keeper to relay
6. **AFTER STATE** — Buffer balance back to target, Solana vault USDC ATA grew, vault.total_deposits incremented

Total runtime ~30 seconds + the 12s wait = ~45 seconds. Plenty of room for a 60-90 second video.

## Narration script

If you want voice-over (or speaker notes if it's silent), here's a tight script:

> **[0:00, intro]** "soltempo brings Stripe-grade payments to Solana DeFi. This is a real cross-VM flow on testnets — Tempo Moderato on the left, Solana devnet on the right."
>
> **[0:08, before state]** "Starting state: Buffer holds the configured 100 pathUSD liquid balance. Solana vault has 900 from a previous run."
>
> **[0:14, approve]** "The merchant approves Buffer to spend 1000 pathUSD."
>
> **[0:20, deposit]** "Merchant deposits 1000 pathUSD into the Buffer on Tempo."
>
> **[0:28, bridge]** "`sendIntentToSolana` fires. Anything above the buffer target — 900 pathUSD — gets bridged. Buffer.sol emits a CCIP-shaped intent with the canonical 122-byte payload."
>
> **[0:35, keeper picks up]** "The off-chain keeper sees the MockMessageSent event within seconds, transfers Solana-side USDC to the vault PDA's ATA, and calls `vault.trusted_keeper_receive` — which validates the source chain, the sender (a 20-byte EVM address), the cross-VM intent format, and updates total_deposits atomically."
>
> **[0:50, after state]** "Buffer is back at the 100 buffer target. Solana vault grew by 900. The on-chain vault state confirms total_deposits incremented by exactly 900 million units."
>
> **[1:00, close]** "Cross-VM thesis: proven end-to-end on real testnets. The trusted-keeper bridge swaps for Chainlink CCIP unchanged when CCIP-on-Tempo ships."

## What the viewer should walk away thinking

- This isn't a hand-wave — the txs land on real testnets with real (test) USDC
- Buffer.sol on Tempo, vault on Solana, both verifiable on block explorers
- The architecture cleanly handles the trusted-keeper → CCIP swap when Chainlink CCIP arrives on Tempo

Optional: end the recording with both explorer URLs visible, so a viewer pausing the video can click through.

## After-recording cleanup

```sh
# Stop the keeper
pkill -f "tsx src/index.ts"

# (Optional) Re-confirm you're back to the same state for the next recording
# — Buffer at bufferTarget (100), vault.total_deposits incremented by 900
```

## Reference explorer URLs (clickable in the video preview)

- Buffer.sol on Moderato: `https://explore.testnet.tempo.xyz/address/0xe8c675523AFd81587c35Da2BeF6ECc268654D0BE`
- Vault program on Solana devnet: `https://explorer.solana.com/address/2YhYmfCoCj3VvyN2HQ3cuavMiZzEUdUTrhvo6nmGRXe3?cluster=devnet`
- Vault PDA on Solana devnet: `https://explorer.solana.com/address/8smibhXARvuYGEadHqc9C9tqTJWFmLUdXAtkabtFaA9M?cluster=devnet`
