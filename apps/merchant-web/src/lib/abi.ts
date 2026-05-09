/**
 * Minimal ABI fragments — only what the dashboard actually calls.
 *
 * Mirrors contracts/buffer/src/Buffer.sol. We don't pull the full
 * generated artifact here to avoid a forge-build dependency on the
 * frontend build. If Buffer's surface changes, update both.
 */

export const ERC20_ABI = [
  {
    type: "function",
    name: "balanceOf",
    stateMutability: "view",
    inputs: [{ name: "account", type: "address" }],
    outputs: [{ type: "uint256" }],
  },
  {
    type: "function",
    name: "allowance",
    stateMutability: "view",
    inputs: [
      { name: "owner", type: "address" },
      { name: "spender", type: "address" },
    ],
    outputs: [{ type: "uint256" }],
  },
  {
    type: "function",
    name: "approve",
    stateMutability: "nonpayable",
    inputs: [
      { name: "spender", type: "address" },
      { name: "amount", type: "uint256" },
    ],
    outputs: [{ type: "bool" }],
  },
  {
    type: "function",
    name: "decimals",
    stateMutability: "view",
    inputs: [],
    outputs: [{ type: "uint8" }],
  },
] as const;

export const BUFFER_ABI = [
  {
    type: "function",
    name: "merchant",
    stateMutability: "view",
    inputs: [],
    outputs: [{ type: "address" }],
  },
  {
    type: "function",
    name: "bufferTarget",
    stateMutability: "view",
    inputs: [],
    outputs: [{ type: "uint256" }],
  },
  {
    type: "function",
    name: "deposit",
    stateMutability: "nonpayable",
    inputs: [{ name: "amount", type: "uint256" }],
    outputs: [],
  },
  {
    type: "function",
    name: "sendIntentToSolana",
    stateMutability: "payable",
    inputs: [],
    outputs: [{ type: "bytes32" }],
  },
  {
    type: "event",
    name: "Deposit",
    inputs: [
      { indexed: true, name: "from", type: "address" },
      { indexed: false, name: "amount", type: "uint256" },
    ],
  },
  {
    type: "event",
    name: "IntentSent",
    inputs: [
      { indexed: true, name: "messageId", type: "bytes32" },
      { indexed: true, name: "nonce", type: "bytes32" },
      { indexed: false, name: "amount", type: "uint256" },
    ],
  },
] as const;
