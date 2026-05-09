/**
 * Derive the soltempo vault PDA for a given merchant authority.
 *
 * Usage:
 *   pnpm --filter @soltempo/keeper derive-pda <merchant-authority-pubkey>
 *
 * Outputs the vault PDA in both base58 (Solana) and hex (the form
 * needed for Buffer.sol's solanaVaultAddress constructor argument).
 */

import { PublicKey } from "@solana/web3.js";

const VAULT_PROGRAM_ID = new PublicKey(
  "2YhYmfCoCj3VvyN2HQ3cuavMiZzEUdUTrhvo6nmGRXe3",
);

function main(): void {
  const authority = process.argv[2];
  if (!authority) {
    console.error("Usage: pnpm --filter @soltempo/keeper derive-pda <merchant-authority-pubkey>");
    console.error();
    console.error("Example:");
    console.error("  pnpm --filter @soltempo/keeper derive-pda H1kS9...");
    process.exit(1);
  }

  let authorityKey: PublicKey;
  try {
    authorityKey = new PublicKey(authority);
  } catch (err) {
    console.error(`Invalid pubkey: ${authority}`);
    console.error(err);
    process.exit(1);
  }

  const [pda, bump] = PublicKey.findProgramAddressSync(
    [Buffer.from("vault"), authorityKey.toBuffer()],
    VAULT_PROGRAM_ID,
  );

  console.log(`Vault program: ${VAULT_PROGRAM_ID.toBase58()}`);
  console.log(`Authority:     ${authorityKey.toBase58()}`);
  console.log();
  console.log(`Vault PDA (base58): ${pda.toBase58()}`);
  console.log(`Vault PDA (hex):    0x${pda.toBuffer().toString("hex")}`);
  console.log(`Bump:               ${bump}`);
  console.log();
  console.log("For Buffer.sol's solanaVaultAddress constructor arg, use the 32-byte hex form above.");
}

main();
