# @soltempo/merchant-web

Single-page merchant dashboard for soltempo. Reads live state across
Tempo Moderato + Solana devnet and exposes a one-click "deposit + bridge"
flow for testnet demos.

## What it shows

- Merchant pathUSD balance on Tempo
- Buffer.sol pathUSD balance + configured `bufferTarget`
- Solana vault PDA's USDC ATA balance (the bridged destination)
- `vault.total_deposits` decoded from the vault account at offset `0x88`
- Cross-VM activity feed — auto-detects buffer/vault deltas and surfaces
  them as events with timestamps

## What it does

The "Deposit + bridge" button runs three sequential txs through the
merchant's hot wallet (env-loaded — testnet only):

1. `pathUSD.approve(buffer, amount)` (skipped if allowance is sufficient)
2. `buffer.deposit(amount)`
3. `buffer.sendIntentToSolana()`

Then waits for the keeper to relay the cross-VM intent to Solana.
The polling loop catches the resulting `vault.total_deposits` increment
and shows it in the activity feed within ~25s.

## Run

```sh
cp .env.local.example .env.local
# Edit NEXT_PUBLIC_MERCHANT_PRIVATE_KEY to enable the deposit button.
# Without it, the dashboard runs in read-only mode.

pnpm install
pnpm --filter @soltempo/merchant-web dev
# → http://localhost:4001
```

## Demo mode caveat

The hot-wallet pattern (private key in `NEXT_PUBLIC_*` → browser bundle)
is only acceptable for testnet demos. For mainnet a wallet adapter
integration (RainbowKit / wagmi) replaces the env-loaded key. The UI
banner says so.
