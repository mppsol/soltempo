/**
 * Tempo client — viem read/write helpers for Buffer.sol + USDC.
 *
 * Read calls go through a public client; the write client is created
 * lazily from the demo merchant key. The chain definition is local —
 * Tempo Moderato is not a built-in viem chain.
 */

import {
  createPublicClient,
  createWalletClient,
  defineChain,
  formatUnits,
  http,
  parseUnits,
  type Address,
  type PublicClient,
  type WalletClient,
} from "viem";
import { privateKeyToAccount } from "viem/accounts";
import {
  BUFFER_ADDRESS,
  MERCHANT_PRIVATE_KEY,
  TEMPO_CHAIN_ID,
  TEMPO_RPC,
  USDC_TEMPO,
} from "./config";
import { BUFFER_ABI, ERC20_ABI } from "./abi";

export const tempoModerato = defineChain({
  id: TEMPO_CHAIN_ID,
  name: "Tempo Moderato",
  nativeCurrency: { name: "TempoETH", symbol: "ETH", decimals: 18 },
  rpcUrls: { default: { http: [TEMPO_RPC] } },
});

export const publicClient: PublicClient = createPublicClient({
  chain: tempoModerato,
  transport: http(),
});

let _walletClient: WalletClient | null = null;
export function getWalletClient(): WalletClient {
  if (_walletClient) return _walletClient;
  const account = privateKeyToAccount(MERCHANT_PRIVATE_KEY);
  _walletClient = createWalletClient({
    account,
    chain: tempoModerato,
    transport: http(),
  });
  return _walletClient;
}

export function getMerchantAddress(): Address {
  return privateKeyToAccount(MERCHANT_PRIVATE_KEY).address;
}

const USDC_DECIMALS = 6;

export function fmtUsdc(amount: bigint): string {
  return formatUnits(amount, USDC_DECIMALS);
}

export function parseUsdc(amount: string): bigint {
  return parseUnits(amount, USDC_DECIMALS);
}

export async function readMerchantUsdcBalance(): Promise<bigint> {
  return publicClient.readContract({
    address: USDC_TEMPO,
    abi: ERC20_ABI,
    functionName: "balanceOf",
    args: [getMerchantAddress()],
  }) as Promise<bigint>;
}

export async function readBufferUsdcBalance(): Promise<bigint> {
  return publicClient.readContract({
    address: USDC_TEMPO,
    abi: ERC20_ABI,
    functionName: "balanceOf",
    args: [BUFFER_ADDRESS],
  }) as Promise<bigint>;
}

export async function readBufferTarget(): Promise<bigint> {
  return publicClient.readContract({
    address: BUFFER_ADDRESS,
    abi: BUFFER_ABI,
    functionName: "bufferTarget",
  }) as Promise<bigint>;
}

export async function readMerchantAllowance(): Promise<bigint> {
  return publicClient.readContract({
    address: USDC_TEMPO,
    abi: ERC20_ABI,
    functionName: "allowance",
    args: [getMerchantAddress(), BUFFER_ADDRESS],
  }) as Promise<bigint>;
}

/**
 * Approve + deposit + bridge in three sequential txs. We don't bundle
 * via multicall because Tempo testnet may not have a deployed
 * Multicall3 we can rely on. Returns the last (sendIntentToSolana) tx
 * hash so the UI can link to a block explorer.
 */
export async function depositAndBridge(amountUsdc: bigint): Promise<{
  approveHash: `0x${string}` | null;
  depositHash: `0x${string}`;
  intentHash: `0x${string}`;
}> {
  const wallet = getWalletClient();
  const account = wallet.account!;

  let approveHash: `0x${string}` | null = null;
  const allowance = await readMerchantAllowance();
  if (allowance < amountUsdc) {
    approveHash = await wallet.writeContract({
      address: USDC_TEMPO,
      abi: ERC20_ABI,
      functionName: "approve",
      args: [BUFFER_ADDRESS, amountUsdc],
      chain: tempoModerato,
      account,
    });
    await publicClient.waitForTransactionReceipt({ hash: approveHash });
  }

  const depositHash = await wallet.writeContract({
    address: BUFFER_ADDRESS,
    abi: BUFFER_ABI,
    functionName: "deposit",
    args: [amountUsdc],
    chain: tempoModerato,
    account,
  });
  await publicClient.waitForTransactionReceipt({ hash: depositHash });

  const intentHash = await wallet.writeContract({
    address: BUFFER_ADDRESS,
    abi: BUFFER_ABI,
    functionName: "sendIntentToSolana",
    args: [],
    chain: tempoModerato,
    account,
  });
  await publicClient.waitForTransactionReceipt({ hash: intentHash });

  return { approveHash, depositHash, intentHash };
}
