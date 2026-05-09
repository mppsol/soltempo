# soltempo v0.2 deployment runbook

End-to-end deployment for the **trusted-keeper variant** (option 3 from the strategy discussion). Demonstrates the cross-VM thesis on real testnets — Tempo Moderato + Solana devnet — using `MockCCIPRouter` on the Tempo side as a stand-in for Chainlink CCIP, which has not yet shipped on Tempo testnet.

The architecture is identical to the production CCIP flow; only the bridge layer is swapped. When CCIP-on-Tempo testnet ships, replace the `MockCCIPRouter` address with the real Chainlink router — no other code changes needed.

## Prerequisites

| Tool | Version | Why |
| --- | --- | --- |
| Node | ≥ 20 | TS workspaces + keeper |
| pnpm | ≥ 9 | Workspace package manager |
| Rust toolchain | stable | Cargo build |
| Solana CLI | 3.1.14+ | `solana program deploy` |
| Anchor | 0.32.1 | `anchor build` + `anchor deploy` |
| Foundry | ≥ 1.0 | `forge build` + `forge create` |

Wallets:
- Solana keypair with ≥ 5 SOL on devnet (one-time deploy + per-tx rent)
- EVM wallet with Tempo Moderato testnet currency (faucet via the official Tempo testnet portal)

## 1. Solana side — vault program

```sh
# Already done in repo: anchor build produces target/deploy/vault.so
# (270 KB). Program ID is locked at:
#   2YhYmfCoCj3VvyN2HQ3cuavMiZzEUdUTrhvo6nmGRXe3
solana config set --url devnet

# Fund the deployer wallet
solana airdrop 5
solana balance

# Deploy
anchor deploy --provider.cluster devnet --provider.wallet ~/.config/solana/id.json

# Verify the program is on devnet
solana program show 2YhYmfCoCj3VvyN2HQ3cuavMiZzEUdUTrhvo6nmGRXe3
```

After deploy, use the IDL on chain:

```sh
anchor idl init -f target/idl/vault.json 2YhYmfCoCj3VvyN2HQ3cuavMiZzEUdUTrhvo6nmGRXe3 \
  --provider.cluster devnet
```

## 2. Solana side — vault initialization

The vault PDA is per-merchant. Initialize it with the configured Tempo Buffer address (we'll have it after step 4) and the chain selector for Tempo Moderato.

For now, derive the vault PDA and note its address:

```sh
# pnpm run scripts/derive-vault-pda.ts -- <merchant-authority-pubkey>
# (script TBD — this is what the keeper code will produce)
```

Vault PDA derivation: `[b"vault", authority]` under the vault program.

**Wait to call `initialize` until step 4 gives us the Tempo Buffer address.**

## 3. Tempo side — install Foundry deps

```sh
cd contracts/buffer
forge install foundry-rs/forge-std --no-commit
forge install smartcontractkit/chainlink-local --no-commit
cd ../..

forge build --root contracts/buffer
forge test --root contracts/buffer  # should pass: Buffer tests + MockCCIPRouter tests
```

## 4. Tempo side — deploy MockCCIPRouter + Buffer

Pre-requisites you need (from Tempo testnet faucet / docs):
- `TEMPO_RPC` — Moderato RPC (e.g., `https://rpc.tempo.xyz` or your provider)
- `TEMPO_PRIVATE_KEY` — deployer EVM key (with testnet currency)
- `MERCHANT_ADDR` — the merchant wallet (can be your test wallet)
- `USDC_ADDR` — Tempo Moderato USDC address (from Tempo docs or testnet portal). If no native USDC, deploy a `MockUSDC` first.

```sh
cd contracts/buffer

# Deploy the mock router
forge create src/MockCCIPRouter.sol:MockCCIPRouter \
  --rpc-url $TEMPO_RPC \
  --private-key $TEMPO_PRIVATE_KEY \
  --broadcast

# Note the deployed address — call it MOCK_ROUTER_ADDR.

# Deploy Buffer — needs all 6 constructor args:
# (mockRouterAddr, merchantAddr, usdcAddr, solanaChainSelector,
#  solanaVaultAddress as 32 bytes, bufferTarget)
#
# solanaChainSelector for devnet: 16423721717087811551
# solanaVaultAddress: the vault PDA from step 2 (32-byte Solana pubkey,
#                     pass as bytes — encode the base58 pubkey to bytes)
# bufferTarget: e.g., 100_000000 (= 100 USDC at 6 decimals)
#
# Use cast to encode the Solana pubkey:
SOLANA_VAULT_PDA_HEX=$(echo "<your vault PDA in base58>" | python3 -c "
import sys, base58
print('0x' + base58.b58decode(sys.stdin.read().strip()).hex())
")

forge create src/Buffer.sol:Buffer \
  --rpc-url $TEMPO_RPC \
  --private-key $TEMPO_PRIVATE_KEY \
  --broadcast \
  --constructor-args \
    $MOCK_ROUTER_ADDR \
    $MERCHANT_ADDR \
    $USDC_ADDR \
    16423721717087811551 \
    $SOLANA_VAULT_PDA_HEX \
    100000000

# Note the Buffer address — call it BUFFER_ADDR.
```

## 5. Solana side — initialize the vault

Now that we have the Tempo Buffer address, initialize the vault.

The Tempo Moderato chain selector for CCIP would normally come from Chainlink's directory. Since CCIP isn't on Moderato yet, use `3963528237232804922` (the previous Tempo testnet selector — preserved here for the swap-when-ready path).

```sh
# pnpm run scripts/initialize-vault.ts -- \
#   --merchant-id <32-byte hex> \
#   --kamino-market <pubkey> \      # placeholder ok for v0.2
#   --ccip-router Ccip842gzYHhvdDkSyi2YVCoAWPbYJoApMFzSxQroE9C \  # devnet
#   --tempo-buffer <BUFFER_ADDR>    # left-padded to 32 bytes
#   --tempo-chain-selector 3963528237232804922
```

(The init script lives in `apps/keeper/src/scripts/initialize-vault.ts` — TBD, see "Open work" below.)

## 6. Bridge USDC supply to both sides

The trusted-keeper bridge needs USDC on both chains. Get them however your test environment allows:

- **Tempo USDC**: faucet, or mint via your test USDC contract
- **Solana devnet USDC**: faucet via SPL Token Faucet, or mint your own test SPL token if devnet USDC is restricted

Mint enough on the Tempo side to give the merchant a working balance. Mint a roughly equal "bridge inventory" on the Solana side to the keeper's wallet — the keeper will transfer from this inventory to the vault when relaying messages.

## 7. Run the keeper

```sh
export TEMPO_RPC=https://rpc.tempo.xyz
export SOLANA_RPC=https://api.devnet.solana.com
export BUFFER_ADDRESS=$BUFFER_ADDR
export MOCK_ROUTER_ADDRESS=$MOCK_ROUTER_ADDR
export VAULT_ADDRESS=<vault PDA from step 2>
export VAULT_PROGRAM_ID=2YhYmfCoCj3VvyN2HQ3cuavMiZzEUdUTrhvo6nmGRXe3
export SOLANA_KEYPAIR=~/.config/solana/id.json
export TEMPO_KEEPER_PRIVATE_KEY=$TEMPO_PRIVATE_KEY
export USDC_TEMPO=$USDC_ADDR
export USDC_SOLANA=<devnet USDC mint>

pnpm --filter @soltempo/keeper dev
```

The keeper:
1. Subscribes to `MockCCIPRouter.MockMessageSent` events on Moderato
2. On each event, withdraws the bridged USDC from the mock router (`withdrawForRelay`) to its own wallet
3. Transfers the equivalent amount of Solana-side USDC to the vault's USDC ATA
4. Calls `vault.ccip_receive` with a manufactured `Any2SVMMessage`
5. Logs the resulting Receipt PDA address (emitted by `mppsol_cpi.pay_with_receipt` once `settle_payout_to_tempo` is called)

## 8. Demo run

```sh
# As the merchant, deposit USDC into the buffer
cast send $BUFFER_ADDR "deposit(uint256)" 500000000 \
  --rpc-url $TEMPO_RPC --private-key $TEMPO_PRIVATE_KEY

# Anyone can trigger the bridge once balance > buffer
cast send $BUFFER_ADDR "sendIntentToSolana()" --value 0 \
  --rpc-url $TEMPO_RPC --private-key $TEMPO_PRIVATE_KEY

# Watch the keeper logs — within ~10s the Solana side should receive
# the message, validate, and update vault.total_deposits.
```

## Swap path: when CCIP-on-Tempo ships

1. Update `MOCK_ROUTER_ADDR` to the real Chainlink router address from
   docs.chain.link/ccip/directory/testnet/chain/tempo-(moderato-or-successor)
2. Redeploy `Buffer.sol` with the real router (the contract code is unchanged)
3. Update vault with the real CCIP chain selector for Tempo
4. Stop the keeper relayer; CCIP delivers the message directly to `vault.ccip_receive`

That's the only change. The architecture is intentionally identical between mock and real CCIP paths.

## Open work for v0.2 demo

Tracked separately because they need real-environment iteration:

- `apps/keeper/src/scripts/derive-vault-pda.ts`
- `apps/keeper/src/scripts/initialize-vault.ts`
- Keeper main loop: event subscription on `MockCCIPRouter`, `vault.ccip_receive` invocation
- Recording the demo video against this end-to-end run

## Mainnet path (later)

When migrating to Tempo mainnet:
1. Replace `MockCCIPRouter` with the real Chainlink CCIP router on Tempo mainnet
2. Replace the devnet vault deployment with mainnet
3. Update `vault.ccip_router` to the Solana mainnet CCIP router
4. Get an audit before any real merchant funds

The audit gate is non-negotiable. soltempo handles real merchant USDC at mainnet.
