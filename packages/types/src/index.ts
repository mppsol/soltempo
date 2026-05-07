export type ChainId = "tempo" | "solana";

export interface BridgeIntent {
  amount: bigint;
  source: ChainId;
  destination: ChainId;
}
