import { createPublicClient, http, parseAbi } from "viem";
import { Connection } from "@solana/web3.js";

const TEMPO_RPC = process.env.TEMPO_RPC ?? "https://rpc.testnet.tempo.xyz";
const SOLANA_RPC = process.env.SOLANA_RPC ?? "https://api.devnet.solana.com";
const BUFFER_ADDRESS = process.env.BUFFER_ADDRESS as `0x${string}` | undefined;
const VAULT_ADDRESS = process.env.VAULT_ADDRESS;

const BUFFER_ABI = parseAbi([
  "event Deposit(address indexed from, uint256 amount)",
  "event Withdrawal(address indexed to, uint256 amount)",
  "event IntentSent(bytes32 indexed messageId, bytes32 indexed nonce, uint256 amount)",
  "event PullbackReceived(bytes32 indexed messageId, uint256 amount)",
  "function bufferTarget() view returns (uint256)",
  "function sendIntentToSolana() payable returns (bytes32)",
]);

async function main(): Promise<void> {
  if (!BUFFER_ADDRESS) {
    throw new Error("BUFFER_ADDRESS env var required");
  }
  if (!VAULT_ADDRESS) {
    throw new Error("VAULT_ADDRESS env var required (Solana base58 pubkey)");
  }

  const tempo = createPublicClient({ transport: http(TEMPO_RPC) });
  const solana = new Connection(SOLANA_RPC, "confirmed");

  console.log("soltempo keeper started");
  console.log(`  Tempo RPC:  ${TEMPO_RPC}`);
  console.log(`  Solana RPC: ${SOLANA_RPC}`);
  console.log(`  Buffer:     ${BUFFER_ADDRESS}`);
  console.log(`  Vault:      ${VAULT_ADDRESS}`);

  // TODO(v0.2): subscribe to Buffer.IntentSent on Tempo, track CCIP message
  //              status via Chainlink CCIP explorer / OffRamp events on
  //              Solana, and confirm vault.IntentReceived fired.
  //
  // TODO(v0.2): subscribe to vault payout requests (off-chain queue or
  //              on-chain payout-request account) and trigger
  //              vault.settle_payout_to_tempo when buffer is insufficient.
  //
  // TODO(v0.2): persist BridgeCycle state (see @soltempo/types) for
  //              merchant dashboard + audit trail.

  // Heartbeat — also reads buffer state every cycle to confirm RPC works.
  setInterval(async () => {
    try {
      const target = await tempo.readContract({
        address: BUFFER_ADDRESS,
        abi: BUFFER_ABI,
        functionName: "bufferTarget",
      });
      const slot = await solana.getSlot();
      console.log(
        `[${new Date().toISOString()}] heartbeat — bufferTarget=${target} solanaSlot=${slot}`
      );
    } catch (err) {
      console.error(`[${new Date().toISOString()}] heartbeat error:`, err);
    }
  }, 30_000);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
