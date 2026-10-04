export interface ToriiBrowserExplorerBlock {
  hash: string;
  height: number;
  created_at: string;
  prev_block_hash: string | null;
  transactions_hash: string | null;
  transactions_rejected: number;
  transactions_total: number;
}

export interface ToriiBrowserExplorerTransaction {
  authority: string;
  hash: string;
  block: number;
  created_at: string;
  executable: string;
  status: string;
}

export interface ToriiBrowserExplorerInstructionBox {
  encoded: string;
  framed_sha256: string;
  json: unknown;
}

export interface ToriiBrowserExplorerInstruction {
  authority: string;
  created_at: string;
  kind: string;
  box: ToriiBrowserExplorerInstructionBox;
  transaction_hash: string;
  transaction_status: string;
  block: number;
  index: number;
}

export interface ToriiBrowserExplorerAssetDefinition {
  readonly id: string;
  /** Null denotes an intentionally unowned global definition. */
  readonly owning_domain: string | null;
  readonly mintable: string;
  readonly logo: string | null;
  readonly metadata: Readonly<Record<string, unknown>>;
  readonly owned_by: string;
  readonly assets: number;
  readonly total_quantity: string;
  readonly locked_quantity: string | null;
  readonly circulating_quantity: string | null;
}
