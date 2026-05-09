#!/usr/bin/env bash
# Run the soltempo keeper in trusted-relayer mode against the verified
# reference deploy. Override env vars to point at a different deploy.
set -euo pipefail

export TEMPO_RPC="${TEMPO_RPC:-https://rpc.moderato.tempo.xyz}"
export SOLANA_RPC="${SOLANA_RPC:-https://api.devnet.solana.com}"
export BUFFER_ADDRESS="${BUFFER_ADDRESS:-0xe8c675523AFd81587c35Da2BeF6ECc268654D0BE}"
export MOCK_ROUTER_ADDRESS="${MOCK_ROUTER_ADDRESS:-0x989F1858c6f217d56DF0edaFBaEEa0F706124df2}"
export VAULT_ADDRESS="${VAULT_ADDRESS:-8smibhXARvuYGEadHqc9C9tqTJWFmLUdXAtkabtFaA9M}"
export VAULT_PROGRAM_ID="${VAULT_PROGRAM_ID:-2YhYmfCoCj3VvyN2HQ3cuavMiZzEUdUTrhvo6nmGRXe3}"
export SOLANA_KEYPAIR="${SOLANA_KEYPAIR:-$HOME/.config/solana/id.json}"
export USDC_TEMPO="${USDC_TEMPO:-0x20c0000000000000000000000000000000000000}"
export USDC_SOLANA="${USDC_SOLANA:-CTwxuhJgAv4Tkxzt8HceSv1c8tyNL3SDWLGYki4rrJAG}"

# Required from caller:
: "${TEMPO_KEEPER_PRIVATE_KEY:?must be set}"

cd "$(dirname "$0")/.."
exec pnpm --filter @soltempo/keeper dev
