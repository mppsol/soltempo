# soltempo v0.2 deployment runbook

End-to-end deployment for the **trusted-keeper variant** (option 3 from the strategy discussion). Demonstrates the cross-VM thesis on real testnets — Tempo Moderato + Solana devnet — using `MockCCIPRouter` on the Tempo side as a stand-in for Chainlink CCIP, which has not yet shipped on Tempo testnet.

The architecture is identical to the production CCIP flow; only the bridge layer is swapped. When CCIP-on-Tempo testnet ships, replace the `MockCCIPRouter` address with the real Chainlink router — no other code changes needed.

## Reference deployment (verified end-to-end on 2026-05-09)

Initial demo deploy completed and the deposit flow ran end-to-end. Keep these as reference; redeploy with your own keys when reproducing.

| | |
| --- | --- |
| Solana vault program | `2YhYmfCoCj3VvyN2HQ3cuavMiZzEUdUTrhvo6nmGRXe3` |
| Solana vault PDA (authority `AmSYugrt…`) | `8smibhXARvuYGEadHqc9C9tqTJWFmLUdXAtkabtFaA9M` |
| Solana vault USDC ATA | `CBuM8CaG5Bzmm1nGZLmxjbnsAHEiFctmUd2bjmmYRak` |
| Demo USDC mint (Solana, 6 decimals) | `CTwxuhJgAv4Tkxzt8HceSv1c8tyNL3SDWLGYki4rrJAG` |
| Tempo MockCCIPRouter | `0x989F1858c6f217d56DF0edaFBaEEa0F706124df2` |
| Tempo Buffer.sol | `0xe8c675523AFd81587c35Da2BeF6ECc268654D0BE` |
| Tempo merchant + keeper EVM addr | `0xA48c1a46a28bF58BFD226A9d7792e1DCDba1C049` |
| Tempo "USDC" used in demo (pathUSD) | `0x20c0000000000000000000000000000000000000` |
| Configured Tempo chain selector | `42431` (Moderato chain ID — sentinel) |
| Demo deposit (Tempo) | 1000 pathUSD → buffer holds 100, bridges 900 |
| End-to-end relay tx (Solana) | [`4sJjybjaEDM…`](https://explorer.solana.com/tx/4sJjybjaEDMondjCafjkLMeD2LctQdY5xmvi58UfBTaxzrTmQDt3CumA2MQGjGqqAoczrqmyD37L5yUT4A594J7J?cluster=devnet) |

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

Derive the vault PDA and note both forms:

```sh
pnpm --filter @soltempo/keeper derive-pda <merchant-authority-pubkey>
# → Vault PDA (base58): CeztYQPP...      (use this on Solana side)
# → Vault PDA (hex):    0xad2c8043...    (use this for Buffer.sol's solanaVaultAddress)
```

Vault PDA derivation: `[b"vault", authority]` under the vault program (`2YhYmfCoCj3VvyN2HQ3cuavMiZzEUdUTrhvo6nmGRXe3`).

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
pnpm --filter @soltempo/keeper init-vault \
  --merchant-id 0x<32-byte hex>         \  # arbitrary canonical merchant identifier
  --tempo-buffer $BUFFER_ADDR           \  # 20-byte EVM address from step 4
  --tempo-chain-selector 3963528237232804922 \
  --usdc-mint <Solana USDC mint pubkey> \
  --authority-keypair ~/.config/solana/id.json \
  --trusted-keeper <keeper Solana pubkey>  \
  # optional:
  # --kamino-market <pubkey>   (defaults to Pubkey.default() — placeholder for v0.2)
  # --ccip-router <pubkey>     (defaults to Ccip842gzYHhvdDkSyi2YVCoAWPbYJoApMFzSxQroE9C)
```

`--trusted-keeper` is the Solana wallet pubkey of the off-chain keeper that will sign `trusted_keeper_receive` calls. For the v0.2 demo this is typically the same Solana keypair the keeper service runs with. To disable the trusted-keeper path entirely (production CCIP-only mode), pass `11111111111111111111111111111111` (default Pubkey).

The init script handles EVM-address-to-32-byte-padding automatically. It prints the tx signature and explorer URL on success.

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
export SOLANA_KEYPAIR=~/.config/solana/id.json   # signs trusted_keeper_receive + SPL transfers
export TEMPO_KEEPER_PRIVATE_KEY=$TEMPO_PRIVATE_KEY  # signs withdrawForRelay (optional)
export USDC_TEMPO=$USDC_ADDR
export USDC_SOLANA=<devnet USDC mint>

pnpm --filter @soltempo/keeper dev
```

The keeper:
1. Subscribes to `MockCCIPRouter.MockMessageSent` events on Moderato
2. On each event, transfers the equivalent USDC from the keeper's Solana inventory ATA to the vault's USDC ATA (real SPL `transferChecked`)
3. Calls `vault.trusted_keeper_receive` with the reconstructed `Any2SVMMessage` (real Anchor instruction call via @coral-xyz/anchor)
4. Logs the tx signature + explorer URL

**Pre-keeper inventory setup (do this before step 8):**
- The keeper's Solana wallet must have an ATA for `USDC_SOLANA` with enough USDC to cover expected inbound deposits. Mint or transfer test USDC into it before any demo deposits.
- The keeper does NOT need to be the same as the vault `authority` — the authority initializes the vault, the keeper signs `trusted_keeper_receive`. They can be different wallets.

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

- ~~`apps/keeper/src/scripts/derive-vault-pda.ts`~~ ✅ done
- ~~`apps/keeper/src/scripts/initialize-vault.ts`~~ ✅ done (with `--trusted-keeper` flag)
- ~~Keeper main loop: real SPL transfer + Anchor `trusted_keeper_receive` call~~ ✅ done
- Optional: keeper periodically pulls bridged USDC out of `MockCCIPRouter` via `withdrawForRelay` to keep accounting symmetric across chains (currently a TODO in keeper code; not blocking for the demo)
- Recording the demo video against this end-to-end run

## Switching from trusted-keeper to real CCIP

Once Chainlink CCIP is on Tempo testnet (Moderato or successor):

1. Deploy the real Chainlink CCIP router on Tempo (or use the address from Chainlink's directory).
2. Redeploy `Buffer.sol` pointing at the real router address (Buffer code is unchanged).
3. Re-run `init-vault` setting `--trusted-keeper 11111111111111111111111111111111` — disables the trusted-keeper path. Update `--ccip-router` to the new Solana-side CCIP router for that lane if changed.
4. Stop the keeper relayer; CCIP delivers messages directly to `vault.ccip_receive` (which already has full validation).

No vault redeploy needed for the swap.

## Mainnet path (later)

When migrating to Tempo mainnet:
1. Replace `MockCCIPRouter` with the real Chainlink CCIP router on Tempo mainnet
2. Replace the devnet vault deployment with mainnet
3. Update `vault.ccip_router` to the Solana mainnet CCIP router
4. Get an audit before any real merchant funds

The audit gate is non-negotiable. soltempo handles real merchant USDC at mainnet.
