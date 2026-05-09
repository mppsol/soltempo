#!/usr/bin/env bash
# soltempo — recordable end-to-end demo
#
# Two terminals (recommended for a clean recording):
#   pane 1:  ./scripts/keeper.sh    (or just runs the keeper inline — see DEMO.md)
#   pane 2:  ./scripts/demo.sh      (this script)
#
# Or run as a single pane with sleeps between sections so the keeper
# output appears in the same scroll. See DEMO.md for narration cues.
set -euo pipefail

# ── Config (matches the verified reference deploy) ────────────────
TEMPO_RPC="${TEMPO_RPC:-https://rpc.moderato.tempo.xyz}"
SOLANA_RPC="${SOLANA_RPC:-https://api.devnet.solana.com}"
BUFFER="${BUFFER_ADDRESS:-0xe8c675523AFd81587c35Da2BeF6ECc268654D0BE}"
USDC_TEMPO="${USDC_TEMPO:-0x20c0000000000000000000000000000000000000}"
VAULT_USDC_ATA="${VAULT_USDC_ATA:-CBuM8CaG5Bzmm1nGZLmxjbnsAHEiFctmUd2bjmmYRak}"
VAULT_PDA="${VAULT_ADDRESS:-8smibhXARvuYGEadHqc9C9tqTJWFmLUdXAtkabtFaA9M}"
DEPOSIT_AMOUNT="${DEPOSIT_AMOUNT:-1000000000}"   # 1000 pathUSD (6 decimals)

# Required from caller: TEMPO_KEEPER_PRIVATE_KEY (the merchant key for cast send)
: "${TEMPO_KEEPER_PRIVATE_KEY:?must be set}"

# ── Helpers ───────────────────────────────────────────────────────
hr() { printf '\n\033[36m── %s ─────────────────────────────────\033[0m\n' "$1"; }
say() { printf '\033[33m▶ %s\033[0m\n' "$1"; }
pause() { sleep "${1:-2}"; }

# ── Demo flow ─────────────────────────────────────────────────────
hr "BEFORE STATE"
say "Buffer.sol on Tempo Moderato — bufferTarget:"
cast call "$BUFFER" "bufferTarget()(uint256)" --rpc-url "$TEMPO_RPC"
say "Buffer's pathUSD balance (should equal bufferTarget after a previous demo):"
cast call "$USDC_TEMPO" "balanceOf(address)(uint256)" "$BUFFER" --rpc-url "$TEMPO_RPC"
say "Solana vault USDC ATA (the destination) — current balance:"
spl-token balance --address "$VAULT_USDC_ATA" --url devnet || true
pause 3

hr "STEP 1 — MERCHANT APPROVES BUFFER"
say "Tempo: pathUSD.approve(Buffer, $DEPOSIT_AMOUNT)"
cast send "$USDC_TEMPO" "approve(address,uint256)" "$BUFFER" "$DEPOSIT_AMOUNT" \
  --rpc-url "$TEMPO_RPC" --private-key "$TEMPO_KEEPER_PRIVATE_KEY" \
  | grep -E "transactionHash|status" | head -2
pause 2

hr "STEP 2 — MERCHANT DEPOSITS pathUSD"
say "Tempo: Buffer.deposit($DEPOSIT_AMOUNT)"
cast send "$BUFFER" "deposit(uint256)" "$DEPOSIT_AMOUNT" \
  --rpc-url "$TEMPO_RPC" --private-key "$TEMPO_KEEPER_PRIVATE_KEY" \
  | grep -E "transactionHash|status" | head -2
pause 2

hr "STEP 3 — TRIGGER CROSS-VM BRIDGE"
say "Tempo: Buffer.sendIntentToSolana()"
cast send "$BUFFER" "sendIntentToSolana()" \
  --rpc-url "$TEMPO_RPC" --private-key "$TEMPO_KEEPER_PRIVATE_KEY" \
  | grep -E "transactionHash|status" | head -2

hr "WAITING FOR KEEPER (~10s)"
say "Watch the keeper log — it should pick up MockMessageSent, do an SPL transfer, and call vault.trusted_keeper_receive."
sleep 12

hr "AFTER STATE"
say "Buffer's pathUSD balance (should be back at bufferTarget):"
cast call "$USDC_TEMPO" "balanceOf(address)(uint256)" "$BUFFER" --rpc-url "$TEMPO_RPC"
say "Solana vault USDC ATA — should have grown by $((DEPOSIT_AMOUNT - 100000000)):"
spl-token balance --address "$VAULT_USDC_ATA" --url devnet
say "Solana vault total_deposits (should match):"
solana account "$VAULT_PDA" --url devnet --output json 2>/dev/null \
  | python3 -c "import json,sys,base64,struct; d=json.load(sys.stdin)['account']['data'][0]; b=base64.b64decode(d); td=struct.unpack('<Q', b[0x88:0x90])[0]; print(f'  vault.total_deposits = {td:,} ({td/1_000_000} demo USDC)')"

hr "DONE"
say "Cross-VM thesis proven end-to-end on real testnets."
