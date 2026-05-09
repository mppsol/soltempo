/**
 * Initialize the soltempo vault PDA on Solana for a single merchant.
 *
 * Usage:
 *   pnpm --filter @soltempo/keeper init-vault \
 *     --merchant-id <32-byte hex> \
 *     --tempo-buffer 0x<20-byte EVM address> \
 *     --tempo-chain-selector <u64> \
 *     --usdc-mint <Solana base58 pubkey> \
 *     --authority-keypair <path to JSON keypair file> \
 *     [--kamino-market <Solana base58 pubkey>] \
 *     [--ccip-router <Solana base58 pubkey>]   (default: devnet router)
 *
 * Defaults:
 *   --kamino-market   Pubkey.default() (placeholder for v0.2 — soltempo
 *                     doesn't actually call Kamino yet)
 *   --ccip-router     Ccip842gzYHhvdDkSyi2YVCoAWPbYJoApMFzSxQroE9C (devnet)
 *
 * Tempo Moderato chain selector: 3963528237232804922 (preserved from
 * Andantino — verify against Chainlink directory when Moderato joins CCIP).
 */

import { AnchorProvider, BN, Program, Wallet } from "@coral-xyz/anchor";
import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
} from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";

const SOLANA_RPC = process.env.SOLANA_RPC ?? "https://api.devnet.solana.com";
const VAULT_PROGRAM_ID = new PublicKey(
  "2YhYmfCoCj3VvyN2HQ3cuavMiZzEUdUTrhvo6nmGRXe3",
);
const CCIP_ROUTER_DEVNET = new PublicKey(
  "Ccip842gzYHhvdDkSyi2YVCoAWPbYJoApMFzSxQroE9C",
);

interface InitArgs {
  merchantId: string;
  tempoBuffer: string;
  tempoChainSelector: bigint;
  usdcMint: string;
  authorityKeypair: string;
  kaminoMarket?: string;
  ccipRouter?: string;
}

function evmAddressToPaddedBytes(addr: string): Uint8Array {
  const clean = addr.startsWith("0x") ? addr.slice(2) : addr;
  if (clean.length !== 40) {
    throw new Error(
      `expected 20-byte EVM address (40 hex chars), got ${clean.length / 2} bytes`,
    );
  }
  const padded = new Uint8Array(32);
  for (let i = 0; i < 20; i++) {
    padded[12 + i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16);
  }
  return padded;
}

function hexToBytes32(hex: string): Uint8Array {
  const clean = hex.startsWith("0x") ? hex.slice(2) : hex;
  if (clean.length !== 64) {
    throw new Error(
      `expected 32-byte hex (64 chars), got ${clean.length / 2} bytes`,
    );
  }
  const out = new Uint8Array(32);
  for (let i = 0; i < 32; i++) {
    out[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

function loadKeypair(filePath: string): Keypair {
  const raw = fs.readFileSync(filePath, "utf8");
  const secret = JSON.parse(raw);
  if (!Array.isArray(secret) || secret.length !== 64) {
    throw new Error(`expected 64-byte secret key array in ${filePath}`);
  }
  return Keypair.fromSecretKey(Uint8Array.from(secret));
}

function loadIdl(): unknown {
  // The keeper script is at apps/keeper/src/scripts/initialize-vault.ts.
  // The IDL is at <repo-root>/target/idl/vault.json.
  // From the script's resolved __dirname, ../../../../target/idl/vault.json.
  const idlPath = path.resolve(__dirname, "../../../../target/idl/vault.json");
  const raw = fs.readFileSync(idlPath, "utf8");
  return JSON.parse(raw);
}

async function main(): Promise<void> {
  const args = parseArgs();

  const authority = loadKeypair(args.authorityKeypair);
  const connection = new Connection(SOLANA_RPC, "confirmed");
  const wallet = new Wallet(authority);
  const provider = new AnchorProvider(connection, wallet, {
    commitment: "confirmed",
  });

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const program = new Program(loadIdl() as any, provider);

  const [vaultPda, _bump] = PublicKey.findProgramAddressSync(
    [Buffer.from("vault"), authority.publicKey.toBuffer()],
    VAULT_PROGRAM_ID,
  );

  const merchantIdBytes = hexToBytes32(args.merchantId);
  const tempoBufferPadded = evmAddressToPaddedBytes(args.tempoBuffer);
  const kaminoMarket = args.kaminoMarket
    ? new PublicKey(args.kaminoMarket)
    : PublicKey.default;
  const ccipRouter = args.ccipRouter
    ? new PublicKey(args.ccipRouter)
    : CCIP_ROUTER_DEVNET;
  const usdcMint = new PublicKey(args.usdcMint);

  console.log("Initializing vault:");
  console.log(`  Vault PDA:       ${vaultPda.toBase58()}`);
  console.log(`  Authority:       ${authority.publicKey.toBase58()}`);
  console.log(`  USDC mint:       ${usdcMint.toBase58()}`);
  console.log(`  CCIP router:     ${ccipRouter.toBase58()}`);
  console.log(`  Kamino market:   ${kaminoMarket.toBase58()} (placeholder)`);
  console.log(`  Tempo Buffer:    ${args.tempoBuffer}`);
  console.log(`  Tempo chain sel: ${args.tempoChainSelector}`);
  console.log(`  Solana RPC:      ${SOLANA_RPC}`);
  console.log();

  const txSig = await program.methods
    .initialize(
      Array.from(merchantIdBytes),
      kaminoMarket,
      ccipRouter,
      Array.from(tempoBufferPadded),
      new BN(args.tempoChainSelector.toString()),
    )
    .accounts({
      vault: vaultPda,
      usdcMint,
      authority: authority.publicKey,
      systemProgram: SystemProgram.programId,
    })
    .rpc();

  console.log(`Initialize tx: ${txSig}`);
  console.log(
    `Explorer:      https://explorer.solana.com/tx/${txSig}?cluster=devnet`,
  );
}

function parseArgs(): InitArgs {
  const argv = process.argv.slice(2);
  const args: Record<string, string> = {};
  for (let i = 0; i < argv.length; i++) {
    if (argv[i]?.startsWith("--")) {
      const key = argv[i]!.slice(2);
      const val = argv[i + 1];
      if (val && !val.startsWith("--")) {
        args[key] = val;
        i++;
      }
    }
  }

  const required = [
    "merchant-id",
    "tempo-buffer",
    "tempo-chain-selector",
    "usdc-mint",
    "authority-keypair",
  ];
  for (const r of required) {
    if (!args[r]) {
      console.error(`Missing required arg: --${r}`);
      console.error();
      console.error("Usage:");
      console.error("  pnpm --filter @soltempo/keeper init-vault \\");
      console.error("    --merchant-id <32-byte hex> \\");
      console.error("    --tempo-buffer 0x<20-byte EVM address> \\");
      console.error("    --tempo-chain-selector <u64> \\");
      console.error("    --usdc-mint <Solana base58 pubkey> \\");
      console.error("    --authority-keypair <path to keypair JSON> \\");
      console.error("    [--kamino-market <Solana base58 pubkey>] \\");
      console.error("    [--ccip-router <Solana base58 pubkey>]");
      process.exit(1);
    }
  }

  return {
    merchantId: args["merchant-id"]!,
    tempoBuffer: args["tempo-buffer"]!,
    tempoChainSelector: BigInt(args["tempo-chain-selector"]!),
    usdcMint: args["usdc-mint"]!,
    authorityKeypair: args["authority-keypair"]!,
    kaminoMarket: args["kamino-market"],
    ccipRouter: args["ccip-router"],
  };
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
