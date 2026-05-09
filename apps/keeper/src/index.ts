import {
  createPublicClient,
  http,
  parseAbi,
  parseAbiItem,
  decodeEventLog,
  type Log,
} from "viem";
import { Connection, PublicKey } from "@solana/web3.js";

const TEMPO_RPC = process.env.TEMPO_RPC ?? "https://rpc.tempo.xyz";
const SOLANA_RPC = process.env.SOLANA_RPC ?? "https://api.devnet.solana.com";
const BUFFER_ADDRESS = process.env.BUFFER_ADDRESS as `0x${string}` | undefined;
const MOCK_ROUTER_ADDRESS = process.env.MOCK_ROUTER_ADDRESS as
  | `0x${string}`
  | undefined;
const VAULT_ADDRESS = process.env.VAULT_ADDRESS;
const VAULT_PROGRAM_ID = process.env.VAULT_PROGRAM_ID;

const BUFFER_ABI = parseAbi([
  "event Deposit(address indexed from, uint256 amount)",
  "event Withdrawal(address indexed to, uint256 amount)",
  "event IntentSent(bytes32 indexed messageId, bytes32 indexed nonce, uint256 amount)",
  "event PullbackReceived(bytes32 indexed messageId, uint256 amount)",
  "function bufferTarget() view returns (uint256)",
  "function sendIntentToSolana() payable returns (bytes32)",
]);

const MOCK_MESSAGE_SENT_EVENT = parseAbiItem(
  "event MockMessageSent(bytes32 indexed messageId, uint64 indexed destChainSelector, address indexed sender, bytes data, uint256 amount, address token, bytes receiver)",
);

const MOCK_ROUTER_ABI = [MOCK_MESSAGE_SENT_EVENT] as const;

async function main(): Promise<void> {
  if (!BUFFER_ADDRESS) {
    throw new Error("BUFFER_ADDRESS env var required");
  }
  if (!MOCK_ROUTER_ADDRESS) {
    throw new Error(
      "MOCK_ROUTER_ADDRESS env var required (the deployed MockCCIPRouter address on Moderato)",
    );
  }
  if (!VAULT_ADDRESS) {
    throw new Error("VAULT_ADDRESS env var required (Solana base58 pubkey)");
  }
  if (!VAULT_PROGRAM_ID) {
    throw new Error("VAULT_PROGRAM_ID env var required");
  }

  const tempo = createPublicClient({ transport: http(TEMPO_RPC) });
  const solana = new Connection(SOLANA_RPC, "confirmed");
  const vaultPubkey = new PublicKey(VAULT_ADDRESS);
  const vaultProgramId = new PublicKey(VAULT_PROGRAM_ID);

  console.log("soltempo keeper started — trusted-relayer mode");
  console.log(`  Tempo RPC:     ${TEMPO_RPC}`);
  console.log(`  Solana RPC:    ${SOLANA_RPC}`);
  console.log(`  Buffer:        ${BUFFER_ADDRESS}`);
  console.log(`  Mock router:   ${MOCK_ROUTER_ADDRESS}`);
  console.log(`  Vault PDA:     ${VAULT_ADDRESS}`);
  console.log(`  Vault program: ${VAULT_PROGRAM_ID}`);

  // Subscribe to MockCCIPRouter.MockMessageSent on Tempo Moderato.
  // viem's watchEvent polls; for a real testnet we'd want WS subscriptions
  // but polling keeps the demo dependency surface minimal.
  const unwatch = tempo.watchEvent({
    address: MOCK_ROUTER_ADDRESS,
    events: MOCK_ROUTER_ABI,
    onLogs: async (logs) => {
      for (const log of logs) {
        try {
          await relayToSolana(log as Log, solana, vaultPubkey, vaultProgramId);
        } catch (err) {
          console.error(
            `[${new Date().toISOString()}] relay failed for log ${log.transactionHash}:${log.logIndex}:`,
            err,
          );
        }
      }
    },
    pollingInterval: 5_000,
  });

  // Heartbeat
  setInterval(async () => {
    try {
      const target = await tempo.readContract({
        address: BUFFER_ADDRESS,
        abi: BUFFER_ABI,
        functionName: "bufferTarget",
      });
      const slot = await solana.getSlot();
      console.log(
        `[${new Date().toISOString()}] heartbeat — bufferTarget=${target} solanaSlot=${slot}`,
      );
    } catch (err) {
      console.error(`[${new Date().toISOString()}] heartbeat error:`, err);
    }
  }, 30_000);

  process.on("SIGINT", () => {
    console.log("shutting down");
    unwatch();
    process.exit(0);
  });
}

/**
 * Relay one MockMessageSent event to Solana:
 *   1. Withdraw the bridged USDC from the mock router (out-of-band — the
 *      keeper holds custody between chains in this demo).
 *   2. Transfer the keeper's Solana-side USDC inventory to vault_usdc_ata.
 *   3. Build an Any2SVMMessage and call vault.ccip_receive.
 *   4. Log the resulting tx signature for the demo run.
 *
 * TODO(v0.2 implementation pass): wire (1)-(3) end to end. The skeleton
 * structure here documents the right flow; the actual web3.js calls
 * (Anchor IDL load, instruction builder, signed tx) are the work for
 * the next focused commit when there's a real testbed to debug against.
 */
async function relayToSolana(
  log: Log,
  _solana: Connection,
  _vaultPubkey: PublicKey,
  _vaultProgramId: PublicKey,
): Promise<void> {
  const decoded = decodeEventLog({
    abi: MOCK_ROUTER_ABI,
    data: log.data,
    topics: log.topics,
  });
  if (decoded.eventName !== "MockMessageSent") return;

  const { messageId, destChainSelector, sender, data, amount, token, receiver } =
    decoded.args as {
      messageId: `0x${string}`;
      destChainSelector: bigint;
      sender: `0x${string}`;
      data: `0x${string}`;
      amount: bigint;
      token: `0x${string}`;
      receiver: `0x${string}`;
    };

  console.log(
    `[${new Date().toISOString()}] MockMessageSent received` +
      `\n    messageId:        ${messageId}` +
      `\n    destChainSelector: ${destChainSelector}` +
      `\n    sender:            ${sender}` +
      `\n    amount:            ${amount}` +
      `\n    token:             ${token}` +
      `\n    receiver:          ${receiver}` +
      `\n    data length:       ${(data.length - 2) / 2} bytes`,
  );

  // TODO(v0.2):
  // 1. await pullTokensFromMockRouter(token, amount);
  // 2. await transferKeeperUsdcToVaultAta(amount);
  // 3. await callVaultCcipReceive({ messageId, destChainSelector, sender, data });
  //
  // The vault.ccip_receive instruction takes Any2SVMMessage:
  //   { message_id, source_chain_selector, sender (Vec<u8>), data (Vec<u8>),
  //     token_amounts (Vec<SVMTokenAmount>) }
  // For the demo we pass token_amounts = [] (since the keeper does the SPL
  // transfer separately) and source_chain_selector = vault.expected_tempo_chain_selector
  // (configured at vault init).

  console.log(
    `[${new Date().toISOString()}] relay TODO — see DEPLOY.md step 7 for the wiring`,
  );
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
