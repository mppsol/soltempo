/**
 * soltempo keeper — trusted-relayer mode for the v0.2 demo.
 *
 * Subscribes to Tempo Buffer's MockCCIPRouter.MockMessageSent. On each
 * event:
 *   1. Transfers the bridged USDC from the keeper's Solana inventory to
 *      vault_usdc_ata (out-of-band relay).
 *   2. Calls vault.trusted_keeper_receive on Solana with the
 *      reconstructed Any2SVMMessage.
 *
 * Once Chainlink CCIP is on Tempo testnet, swap MockCCIPRouter for the
 * real router and switch the keeper to listen for offramp execution
 * events instead. The vault.ccip_receive instruction (already
 * production-ready) handles that path.
 */

import anchor from "@coral-xyz/anchor";
const { AnchorProvider, Program, Wallet } = anchor;
type BN = anchor.BN;
import {
  createTransferCheckedInstruction,
  getAssociatedTokenAddress,
  getMint,
} from "@solana/spl-token";
import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
import {
  createPublicClient,
  createWalletClient,
  decodeEventLog,
  http,
  parseAbi,
  parseAbiItem,
  type Log,
} from "viem";
import { privateKeyToAccount } from "viem/accounts";

const TEMPO_RPC = process.env.TEMPO_RPC ?? "https://rpc.tempo.xyz";
const SOLANA_RPC = process.env.SOLANA_RPC ?? "https://api.devnet.solana.com";
const BUFFER_ADDRESS = process.env.BUFFER_ADDRESS as `0x${string}` | undefined;
const MOCK_ROUTER_ADDRESS = process.env.MOCK_ROUTER_ADDRESS as
  | `0x${string}`
  | undefined;
const VAULT_ADDRESS = process.env.VAULT_ADDRESS;
const VAULT_PROGRAM_ID = process.env.VAULT_PROGRAM_ID;
const SOLANA_KEYPAIR = process.env.SOLANA_KEYPAIR;
const TEMPO_KEEPER_PRIVATE_KEY = process.env.TEMPO_KEEPER_PRIVATE_KEY as
  | `0x${string}`
  | undefined;
const USDC_SOLANA = process.env.USDC_SOLANA;
const USDC_TEMPO = process.env.USDC_TEMPO;

const BUFFER_ABI = parseAbi([
  "event Deposit(address indexed from, uint256 amount)",
  "event Withdrawal(address indexed to, uint256 amount)",
  "event IntentSent(bytes32 indexed messageId, bytes32 indexed nonce, uint256 amount)",
  "event PullbackReceived(bytes32 indexed messageId, uint256 amount)",
  "function bufferTarget() view returns (uint256)",
]);

const MOCK_MESSAGE_SENT_EVENT = parseAbiItem(
  "event MockMessageSent(bytes32 indexed messageId, uint64 indexed destChainSelector, address indexed sender, bytes data, uint256 amount, address token, bytes receiver)",
);

const MOCK_ROUTER_ABI = [MOCK_MESSAGE_SENT_EVENT] as const;

const WITHDRAW_FOR_RELAY_ABI = parseAbi([
  "function withdrawForRelay(address token, address to, uint256 amount)",
]);

function loadKeypair(filePath: string): Keypair {
  const raw = fs.readFileSync(filePath, "utf8");
  const secret = JSON.parse(raw);
  if (!Array.isArray(secret) || secret.length !== 64) {
    throw new Error(`expected 64-byte secret key array in ${filePath}`);
  }
  return Keypair.fromSecretKey(Uint8Array.from(secret));
}

function loadIdl(): unknown {
  const idlPath = path.resolve(__dirname, "../../../target/idl/vault.json");
  const raw = fs.readFileSync(idlPath, "utf8");
  return JSON.parse(raw);
}

function requireEnv<T>(name: string, value: T | undefined): T {
  if (!value) {
    throw new Error(`${name} env var required`);
  }
  return value;
}

async function main(): Promise<void> {
  const bufferAddress = requireEnv("BUFFER_ADDRESS", BUFFER_ADDRESS);
  const mockRouterAddress = requireEnv("MOCK_ROUTER_ADDRESS", MOCK_ROUTER_ADDRESS);
  const vaultAddress = requireEnv("VAULT_ADDRESS", VAULT_ADDRESS);
  const vaultProgramId = requireEnv("VAULT_PROGRAM_ID", VAULT_PROGRAM_ID);
  const solanaKeypairPath = requireEnv("SOLANA_KEYPAIR", SOLANA_KEYPAIR);
  const tempoPrivateKey = requireEnv(
    "TEMPO_KEEPER_PRIVATE_KEY",
    TEMPO_KEEPER_PRIVATE_KEY,
  );
  const usdcSolana = requireEnv("USDC_SOLANA", USDC_SOLANA);
  const usdcTempo = requireEnv("USDC_TEMPO", USDC_TEMPO);

  // --- Solana side -----------------------------------------------
  const keeperKeypair = loadKeypair(solanaKeypairPath);
  const connection = new Connection(SOLANA_RPC, "confirmed");
  const wallet = new Wallet(keeperKeypair);
  const provider = new AnchorProvider(connection, wallet, {
    commitment: "confirmed",
  });
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const program = new Program(loadIdl() as any, provider);

  const vaultPubkey = new PublicKey(vaultAddress);
  const vaultProgramPubkey = new PublicKey(vaultProgramId);
  const usdcMintPubkey = new PublicKey(usdcSolana);

  const vaultUsdcAta = await getAssociatedTokenAddress(
    usdcMintPubkey,
    vaultPubkey,
    true, // allowOwnerOffCurve — vault is a PDA
  );
  const keeperUsdcAta = await getAssociatedTokenAddress(
    usdcMintPubkey,
    keeperKeypair.publicKey,
  );
  const usdcMintInfo = await getMint(connection, usdcMintPubkey);

  // --- Tempo side ------------------------------------------------
  const tempoAccount = privateKeyToAccount(tempoPrivateKey);
  const tempo = createPublicClient({ transport: http(TEMPO_RPC) });
  const tempoWallet = createWalletClient({
    account: tempoAccount,
    transport: http(TEMPO_RPC),
  });

  console.log("soltempo keeper started — trusted-relayer mode");
  console.log(`  Tempo RPC:        ${TEMPO_RPC}`);
  console.log(`  Solana RPC:       ${SOLANA_RPC}`);
  console.log(`  Buffer:           ${bufferAddress}`);
  console.log(`  Mock router:      ${mockRouterAddress}`);
  console.log(`  Vault PDA:        ${vaultPubkey.toBase58()}`);
  console.log(`  Vault program:    ${vaultProgramPubkey.toBase58()}`);
  console.log(`  Solana keeper:    ${keeperKeypair.publicKey.toBase58()}`);
  console.log(`  Tempo keeper:     ${tempoAccount.address}`);
  console.log(`  USDC (Solana):    ${usdcMintPubkey.toBase58()} (${usdcMintInfo.decimals} decimals)`);
  console.log(`  USDC (Tempo):     ${usdcTempo}`);
  console.log(`  Vault USDC ATA:   ${vaultUsdcAta.toBase58()}`);
  console.log(`  Keeper USDC ATA:  ${keeperUsdcAta.toBase58()}`);

  // Subscribe to MockMessageSent
  const unwatch = tempo.watchEvent({
    address: mockRouterAddress,
    events: MOCK_ROUTER_ABI,
    onLogs: async (logs) => {
      for (const log of logs) {
        try {
          await relayToSolana({
            log: log as Log,
            connection,
            program,
            keeperKeypair,
            vaultPubkey,
            usdcMintPubkey,
            vaultUsdcAta,
            keeperUsdcAta,
            usdcMintDecimals: usdcMintInfo.decimals,
            mockRouterAddress,
            usdcTempo: usdcTempo as `0x${string}`,
            tempoWallet,
          });
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

  setInterval(async () => {
    try {
      const target = await tempo.readContract({
        address: bufferAddress,
        abi: BUFFER_ABI,
        functionName: "bufferTarget",
      });
      const slot = await connection.getSlot();
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

interface RelayContext {
  log: Log;
  connection: Connection;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  program: anchor.Program<any>;
  keeperKeypair: Keypair;
  vaultPubkey: PublicKey;
  usdcMintPubkey: PublicKey;
  vaultUsdcAta: PublicKey;
  keeperUsdcAta: PublicKey;
  usdcMintDecimals: number;
  mockRouterAddress: `0x${string}`;
  usdcTempo: `0x${string}`;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  tempoWallet: any;
}

async function relayToSolana(ctx: RelayContext): Promise<void> {
  const decoded = decodeEventLog({
    abi: MOCK_ROUTER_ABI,
    data: ctx.log.data,
    topics: ctx.log.topics,
  });
  if (decoded.eventName !== "MockMessageSent") return;

  const { messageId, sender, data, amount } = decoded.args as {
    messageId: `0x${string}`;
    destChainSelector: bigint;
    sender: `0x${string}`;
    data: `0x${string}`;
    amount: bigint;
    token: `0x${string}`;
    receiver: `0x${string}`;
  };

  console.log(
    `[${new Date().toISOString()}] MockMessageSent` +
      `\n    messageId: ${messageId}` +
      `\n    sender:    ${sender}` +
      `\n    amount:    ${amount}` +
      `\n    data:      ${(data.length - 2) / 2} bytes`,
  );

  // 1. Transfer keeper's Solana USDC inventory to vault_usdc_ata.
  //    The amount must match what the Tempo side sent.
  const transferIx = createTransferCheckedInstruction(
    ctx.keeperUsdcAta,
    ctx.usdcMintPubkey,
    ctx.vaultUsdcAta,
    ctx.keeperKeypair.publicKey,
    BigInt(amount.toString()),
    ctx.usdcMintDecimals,
  );
  const transferTx = new Transaction().add(transferIx);
  const transferSig = await sendAndConfirmTransaction(
    ctx.connection,
    transferTx,
    [ctx.keeperKeypair],
    { commitment: "confirmed" },
  );
  console.log(`    SPL transfer sig: ${transferSig}`);

  // 2. Call vault.trusted_keeper_receive with the reconstructed message.
  //    sourceChainSelector is whatever the vault was configured with at
  //    initialize — we read it from on-chain vault state to avoid drift.
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const programAny = ctx.program as any;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const vaultAccount: any = await programAny.account.vault.fetch(ctx.vaultPubkey);
  const sourceChainSelector: BN = vaultAccount.expectedTempoChainSelector;

  // Sender = the EVM Buffer address (from the event), as raw 20 bytes.
  const senderBytes = Buffer.from(sender.slice(2), "hex");
  // Data = the canonical CrossVMIntent payload from the event.
  const dataBytes = Buffer.from(data.slice(2), "hex");
  // messageId = 32 bytes from event.
  const messageIdBytes = Array.from(Buffer.from(messageId.slice(2), "hex"));

  const sig: string = await programAny.methods
    .trustedKeeperReceive({
      messageId: messageIdBytes,
      sourceChainSelector,
      sender: senderBytes,
      data: dataBytes,
      tokenAmounts: [],
    })
    .accounts({
      vault: ctx.vaultPubkey,
      vaultUsdcAta: ctx.vaultUsdcAta,
      usdcMint: ctx.usdcMintPubkey,
      trustedKeeper: ctx.keeperKeypair.publicKey,
    })
    .rpc();

  console.log(`    trusted_keeper_receive sig: ${sig}`);
  console.log(
    `    explorer: https://explorer.solana.com/tx/${sig}?cluster=devnet`,
  );

  // 3. Optional: pull the bridged USDC out of the mock router on Tempo
  //    so the keeper's Tempo inventory grows symmetrically. For a clean
  //    accounting story across chains, do this every relay. Leave a
  //    TODO comment; non-blocking for the demo flow.
  // const pullbackTx = await ctx.tempoWallet.writeContract({...});
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
