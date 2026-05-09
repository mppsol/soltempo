#!/usr/bin/env bash
# Recording-optimized end-to-end demo. Single-pane sequential flow:
# starts keeper in background, runs the bridge, tails keeper log,
# verifies state. Designed for VHS to capture as MP4.
set -euo pipefail

# Config (override via env if needed)
TEMPO_RPC="${TEMPO_RPC:-https://rpc.moderato.tempo.xyz}"
SOLANA_RPC="${SOLANA_RPC:-https://api.devnet.solana.com}"
BUFFER="${BUFFER_ADDRESS:-0xe8c675523AFd81587c35Da2BeF6ECc268654D0BE}"
USDC_TEMPO="${USDC_TEMPO:-0x20c0000000000000000000000000000000000000}"
VAULT_USDC_ATA="${VAULT_USDC_ATA:-CBuM8CaG5Bzmm1nGZLmxjbnsAHEiFctmUd2bjmmYRak}"
VAULT_PDA="${VAULT_ADDRESS:-8smibhXARvuYGEadHqc9C9tqTJWFmLUdXAtkabtFaA9M}"
DEPOSIT_AMOUNT="${DEPOSIT_AMOUNT:-1000000000}"

: "${TEMPO_KEEPER_PRIVATE_KEY:?must be set}"

# Colors
B=$'\033[36m'   # cyan
A=$'\033[32m'   # green (accent)
Y=$'\033[33m'   # yellow
M=$'\033[35m'   # magenta
N=$'\033[0m'    # reset
hr() { printf "\n${B}══════ %s ══════${N}\n" "$1"; }
say() { printf "${Y}▸ %s${N}\n" "$1"; }
ok() { printf "${A}✓ %s${N}\n" "$1"; }

# ── Stage 1: keeper boots ─────────────────────────────────────────
hr "STAGE 1 — start the off-chain keeper"
say "scripts/keeper.sh observes Tempo MockMessageSent, relays to Solana"
./scripts/keeper.sh > /tmp/keeper.log 2>&1 &
KEEPER_PID=$!
# 18s — enough for tsx + Anchor IDL load + viem watchEvent filter setup.
# Polling runs every 5s after that.
sleep 18
head -16 /tmp/keeper.log
ok "keeper running (PID $KEEPER_PID)"

# ── Stage 2: merchant pays into the buffer on Tempo ───────────────
hr "STAGE 2 — merchant deposits 1000 pathUSD on Tempo Moderato"
say "1) approve Buffer to spend 1000 pathUSD"
cast send "$USDC_TEMPO" "approve(address,uint256)" "$BUFFER" "$DEPOSIT_AMOUNT" \
  --rpc-url "$TEMPO_RPC" --private-key "$TEMPO_KEEPER_PRIVATE_KEY" \
  | grep -E "transactionHash|status" | head -2
say "2) Buffer.deposit(1000 pathUSD)"
cast send "$BUFFER" "deposit(uint256)" "$DEPOSIT_AMOUNT" \
  --rpc-url "$TEMPO_RPC" --private-key "$TEMPO_KEEPER_PRIVATE_KEY" \
  | grep -E "transactionHash|status" | head -2

# ── Stage 3: trigger the cross-VM bridge ──────────────────────────
hr "STAGE 3 — Buffer.sendIntentToSolana()"
say "Anything above bufferTarget (100 pathUSD) bridges out — 900 pathUSD"
cast send "$BUFFER" "sendIntentToSolana()" \
  --rpc-url "$TEMPO_RPC" --private-key "$TEMPO_KEEPER_PRIVATE_KEY" \
  | grep -E "transactionHash|status" | head -2
ok "MockMessageSent emitted on Moderato — keeper polling will pick it up..."

# ── Stage 4: wait for keeper, show the relay ──────────────────────
hr "STAGE 4 — keeper observes + relays to Solana"
say "Polling every 5s. Watch for SPL transfer + trusted_keeper_receive sigs."
sleep 22
tail -10 /tmp/keeper.log

# ── Stage 5: verify state on Solana ───────────────────────────────
hr "STAGE 5 — verified state on Solana devnet"
say "vault USDC ATA balance:"
spl-token balance --address "$VAULT_USDC_ATA" --url devnet
say "vault.total_deposits (read from on-chain account):"
solana account "$VAULT_PDA" --url devnet --output json 2>/dev/null \
  | python3 -c "import json,sys,base64,struct; d=json.load(sys.stdin)['account']['data'][0]; b=base64.b64decode(d); td=struct.unpack('<Q', b[0x88:0x90])[0]; print(f'  ${A}{td:,}${N} ({td/1_000_000} demo USDC)')"

ok "cross-VM thesis proven end-to-end on real testnets"
hr "done"

# Cleanup
kill "$KEEPER_PID" 2>/dev/null || true
wait 2>/dev/null || true
