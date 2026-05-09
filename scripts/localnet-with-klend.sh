#!/usr/bin/env bash
# Boot a local solana-test-validator with Kamino klend mainnet cloned in.
#
# klend has no devnet deployment — the only way to exercise the
# `deposit_to_kamino` CPI without paying real mainnet fees is to clone
# the program (and a USDC reserve + market) into a localnet validator
# via --clone, then point the vault at PROGRAM_ID_MAINNET locally.
#
# What gets cloned:
#   1. KLend2g3...        — klend program (mainnet)
#   2. FarmsPZ...         — Kamino farms program (required by v2 ix)
#   3. The "Main" market  — Kamino's flagship market on mainnet
#   4. The USDC reserve in that market
#   5. The reserve's liquidity_supply, collateral_mint, and dest_collateral
#
# After bootup, the vault can be deployed to localnet, and the
# `deposit_to_kamino` ix can be invoked against the cloned reserve.
#
# Usage:
#   ./scripts/localnet-with-klend.sh
#
# Then in another terminal:
#   solana config set --url localhost
#   anchor deploy --provider.cluster localnet
#   pnpm --filter @soltempo/keeper init-vault -- --kamino-market <KAMINO_MAIN_MARKET>
set -euo pipefail

KLEND_PROGRAM="KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD"
FARMS_PROGRAM="FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr"

# Kamino "Main" market on mainnet — verify against
# https://app.kamino.finance/lending before each run; markets get
# upgraded periodically.
KAMINO_MAIN_MARKET="${KAMINO_MAIN_MARKET:-7u3HeHxYDLhnCoErrtycNokbQYbWGzLs6JSDqGAv5PfF}"

# USDC reserve in the Main market.
USDC_RESERVE="${USDC_RESERVE:-D6q6wuQSrifJKZYpR1M8R4YawnLDtDsMmWM1NbBmgJ59}"

# Mainnet RPC to clone from. Devnet RPC won't work — these accounts
# only exist on mainnet.
SRC_RPC="${SRC_RPC:-https://api.mainnet-beta.solana.com}"

echo "Booting solana-test-validator with Kamino klend mainnet cloned…"
echo "  klend:        $KLEND_PROGRAM"
echo "  farms:        $FARMS_PROGRAM"
echo "  market:       $KAMINO_MAIN_MARKET"
echo "  USDC reserve: $USDC_RESERVE"
echo "  src RPC:      $SRC_RPC"
echo

exec solana-test-validator \
  --reset \
  --quiet \
  --url "$SRC_RPC" \
  --clone "$KLEND_PROGRAM" \
  --clone "$FARMS_PROGRAM" \
  --clone "$KAMINO_MAIN_MARKET" \
  --clone "$USDC_RESERVE" \
  --clone-upgradeable-program "$KLEND_PROGRAM"
