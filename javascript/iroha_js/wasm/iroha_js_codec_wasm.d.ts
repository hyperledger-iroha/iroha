/* tslint:disable */
/* eslint-disable */

/**
 * Parse an encoded account and return its canonical bytes and network prefix as JSON.
 */
export function accountAddressParseEncoded(input: string, expected_prefix?: number | null): string;

/**
 * Render canonical account bytes and return canonical hex and I105 as JSON.
 */
export function accountAddressRender(bytes: Uint8Array, network_prefix: number): string;

/**
 * Decode one canonical public Norito instruction frame into strict JSON.
 */
export function noritoDecodeInstruction(bytes: Uint8Array): string;

/**
 * Decode exactly one canonical transaction instruction archive into strict JSON.
 */
export function noritoDecodeInstructionBoxArchive(bytes: Uint8Array): string;

/**
 * Encode strict instruction JSON as its canonical public Norito frame.
 */
export function noritoEncodeInstruction(input: string): Uint8Array;

/**
 * Encode strict instruction JSON as the canonical transaction instruction archive.
 */
export function noritoEncodeInstructionBoxArchive(input: string): Uint8Array;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly accountAddressParseEncoded: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly accountAddressRender: (a: number, b: number, c: number) => [number, number, number, number];
    readonly noritoDecodeInstruction: (a: number, b: number) => [number, number, number, number];
    readonly noritoDecodeInstructionBoxArchive: (a: number, b: number) => [number, number, number, number];
    readonly noritoEncodeInstruction: (a: number, b: number) => [number, number, number, number];
    readonly noritoEncodeInstructionBoxArchive: (a: number, b: number) => [number, number, number, number];
    readonly sorafs_reference_free_buffer: (a: number) => void;
    readonly sorafs_reference_validate_appeal_finance_cancel_asset_lock_json: (a: number, b: number, c: number, d: number, e: number, f: bigint) => void;
    readonly sorafs_reference_validate_bundle_json: (a: number, b: number, c: number, d: bigint, e: bigint) => void;
    readonly sorafs_reference_validate_governance_dag_block_json: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: bigint) => void;
    readonly sorafs_reference_validate_governance_dag_head_chain_json: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: bigint) => void;
    readonly sorafs_reference_validate_governance_json: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: bigint) => void;
    readonly sorafs_reference_validate_hedging_json: (a: number, b: number, c: number, d: number, e: number, f: number, g: bigint) => void;
    readonly sorafs_reference_validate_orderbook_json: (a: number, b: number, c: number, d: number, e: number, f: number, g: bigint) => void;
    readonly sorafs_reference_validate_pdp_challenge_json: (a: number, b: number, c: number, d: number, e: number, f: bigint) => void;
    readonly sorafs_reference_validate_pdp_challenge_proof_json: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: bigint) => void;
    readonly sorafs_reference_validate_pdp_commitment_challenge_json: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: bigint) => void;
    readonly sorafs_reference_validate_pdp_commitment_json: (a: number, b: number, c: number, d: number, e: number, f: bigint) => void;
    readonly sorafs_reference_validate_pdp_json: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: bigint) => void;
    readonly sorafs_reference_validate_pdp_proof_json: (a: number, b: number, c: number, d: number, e: number, f: bigint) => void;
    readonly sorafs_reference_validate_pop_json: (a: number, b: number, c: number, d: number, e: number, f: number, g: bigint) => void;
    readonly sorafs_reference_validate_por_json: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: bigint) => void;
    readonly sorafs_reference_validate_potr_json: (a: number, b: number, c: number, d: number, e: number, f: number, g: bigint) => void;
    readonly sorafs_reference_validate_provider_admission_json: (a: number, b: number, c: number, d: number, e: number, f: bigint) => void;
    readonly sorafs_reference_validate_provider_admission_renewal_json: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: bigint) => void;
    readonly sorafs_reference_validate_provider_admission_revocation_json: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: bigint) => void;
    readonly sorafs_reference_validate_provider_advert_json: (a: number, b: number, c: number, d: number, e: number, f: bigint, g: bigint) => void;
    readonly sorafs_reference_validate_repair_json: (a: number, b: number, c: number, d: number, e: number, f: number, g: bigint) => void;
    readonly sorafs_reference_validate_replication_order_json: (a: number, b: number, c: number, d: number, e: number, f: bigint) => void;
    readonly sorafs_reference_validate_signed_replication_order_json: (a: number, b: number, c: number, d: number, e: number, f: bigint) => void;
    readonly soranet_mldsa_generate_keypair: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly soranet_mldsa_parameters: (a: number, b: number, c: number, d: number) => number;
    readonly soranet_mldsa_sign: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly soranet_mldsa_verify: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly soranet_mlkem_decapsulate: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly soranet_mlkem_encapsulate: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly soranet_mlkem_generate_keypair: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly soranet_mlkem_parameters: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly PQCRYPTO_RUST_randombytes: (a: number, b: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
