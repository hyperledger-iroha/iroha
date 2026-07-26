export type KotodamaCompilerDiagnosticPhase =
  | "lex"
  | "parse"
  | "resolve"
  | "semantic"
  | "lowering"
  | "artifact";

export interface KotodamaCompilerSourcePosition {
  line: number;
  column: number;
}

export interface KotodamaCompilerSourceSpan {
  source: string | null;
  start: KotodamaCompilerSourcePosition;
  end: KotodamaCompilerSourcePosition;
  /** Exact half-open UTF-8 byte range, when the Rust frontend has source text. */
  byte_range: { start: number; end: number } | null;
}

export interface KotodamaCompilerDiagnosticLabel {
  span: KotodamaCompilerSourceSpan;
  message: string;
}

export interface KotodamaCompilerDiagnosticFix {
  span: KotodamaCompilerSourceSpan;
  replacement: string;
}

/** Exact semantic record emitted by `Diagnostic::to_json_value` in Rust. */
export interface KotodamaCompilerDiagnostic {
  code: string;
  severity: "error" | "warning";
  phase: KotodamaCompilerDiagnosticPhase;
  message: string;
  primary_span: KotodamaCompilerSourceSpan | null;
  labels: KotodamaCompilerDiagnosticLabel[];
  notes: string[];
  help: string | null;
  fix: KotodamaCompilerDiagnosticFix | null;
}

export interface KotodamaCompiledTriggerDescriptor {
  id: string;
  repeats: { Indefinitely: null } | { Exactly: number };
  /** Canonical standard-base64 NRT0 frame for `EventFilterBox`. */
  filter: string;
  authority: string | null;
  metadata: Record<string, unknown>;
  callback: {
    namespace: string | null;
    entrypoint: string;
  };
}

export interface KotodamaCompiledManifestEntrypointKind {
  kind: "Kotoage" | "View" | "Hajimari" | "Kaizen";
  value: null;
}

export interface KotodamaCompiledKotobaTranslation {
  lang: string;
  text: string;
}

export interface KotodamaCompiledKotobaEntry {
  msg_id: string;
  translations: KotodamaCompiledKotobaTranslation[];
}

export type KotodamaCompiledEntrypointValueKindName =
  | "Int"
  | "Decimal"
  | "Quantity"
  | "Bool"
  | "String"
  | "Json"
  | "Name"
  | "AccountId"
  | "AssetDefinitionId"
  | "AssetId"
  | "DomainId"
  | "NftId"
  | "DataSpaceId"
  | "Blob";

export interface KotodamaCompiledEntrypointValueKind {
  kind: KotodamaCompiledEntrypointValueKindName;
  value: null;
}

export interface KotodamaCompiledEntrypointValueType {
  nodes: KotodamaCompiledEntrypointValueTypeNode[];
}

export type KotodamaCompiledEntrypointValueTypeNode =
  | {
      kind: "Struct";
      value: { name: string; fields: string[] };
    }
  | { kind: "Tuple"; value: number }
  | { kind: "Option"; value: null }
  | { kind: "Result"; value: null }
  | {
      kind: "List";
      value: { capacity: number };
    }
  | { kind: "Leaf"; value: KotodamaCompiledEntrypointValueKind };

export interface KotodamaCompiledEntrypointArgumentSchema {
  fields: Array<{
    name: string;
    ty: KotodamaCompiledEntrypointValueType;
  }>;
}

export interface KotodamaCompiledEntrypoint {
  name: string;
  kind: KotodamaCompiledManifestEntrypointKind;
  params: Array<{
    name: string;
    type_name: string;
  }>;
  argument_schema: KotodamaCompiledEntrypointArgumentSchema | null;
  return_type: string | null;
  return_schema: KotodamaCompiledEntrypointValueType | null;
  permission: string | null;
  read_keys: string[];
  write_keys: string[];
  access_hints_complete: boolean | null;
  access_hints_skipped: string[];
  triggers: KotodamaCompiledTriggerDescriptor[];
}

export interface KotodamaCompiledSourceMapEntry {
  function_name: string;
  pc_start: number;
  pc_end: number;
  source_path: string | null;
  source_id: number;
  byte_start: number;
  byte_end: number;
  line: number;
  column: number;
}

export interface KotodamaCompiledBudgetEntry {
  function_name: string;
  pc_start: number;
  pc_end: number;
  bytecode_bytes: number;
  bytecode_words: number;
  frame_bytes: number;
  jump_span_words: number;
  jump_range_risk: boolean;
  source_path: string | null;
  source_id: number | null;
  byte_start: number | null;
  byte_end: number | null;
  line: number | null;
  column: number | null;
}

export interface KotodamaCompiledDynamicAccessHint {
  base_key: string;
  key_type: string;
  bound_kind: string;
  max_keys: number;
}

export interface KotodamaCompiledStateDescriptor {
  name: string;
  type_name: string;
}

export interface KotodamaCompiledErrorCodeDescriptor {
  namespace: string;
  name: string;
  code: number;
}

export interface KotodamaCompiledManifestProvenance {
  signer: string;
  signature: string;
}

export interface KotodamaCompiledManifestMetadata {
  seiyaku_name: string;
  code_hash: string;
  abi_hash: string;
  compiler_fingerprint: string;
  features_bitmap: number;
  entrypoints: KotodamaCompiledEntrypoint[];
  access_set_hints: {
    read_keys: string[];
    write_keys: string[];
    dynamic_reads: KotodamaCompiledDynamicAccessHint[];
    dynamic_writes: KotodamaCompiledDynamicAccessHint[];
  } | null;
  states: KotodamaCompiledStateDescriptor[];
  error_codes: KotodamaCompiledErrorCodeDescriptor[] | null;
  kotoba: KotodamaCompiledKotobaEntry[] | null;
  /** Signed provenance is not accepted until its exact V1 message can be verified. */
  provenance: null;
}

export interface KotodamaCompilerRequestOptions {
  /** Logical UTF-8 source path preserved in diagnostics and hash-keyed sidecars. */
  sourceName?: string;
  /** Select the canonical ZK contract policy required by `Secret<T>`. */
  zk?: boolean;
}

/** Exact bounded request sent to `iroha_js_host` or the compiler service. */
export interface KotodamaCompilerRequest {
  source: string;
  sourceName?: string;
  zk: boolean;
}

export interface KotodamaCompilerTransportOptions {
  /** Abort one remote compilation; the exact caller reason is preserved. */
  signal?: AbortSignal;
  /** Total fetch-and-body deadline in milliseconds (default 30,000; maximum 120,000). */
  timeoutMs?: number;
}

export interface KotodamaCompilerCallOptions
  extends KotodamaCompilerRequestOptions,
    KotodamaCompilerTransportOptions {}

export interface KotodamaCompilerOptions extends KotodamaCompilerCallOptions {
  /**
   * Canonical Rust compiler-service URL. Required in browsers; optional in
   * Node, which otherwise compiles asynchronously through `iroha_js_host`.
   * Remote services receive the complete source and must be trusted. They must
   * use HTTPS; loopback development URLs may use HTTP. Responses must be
   * uncompressed (`Content-Encoding` absent or `identity`).
   */
  compilerUrl?: string;
  /** Fetch implementation used only with `compilerUrl`. */
  fetchImpl?: typeof fetch;
}

export interface KotodamaCompilerOutput {
  /**
   * Bounded IVM 1.1/ABI-1 artifact with a validated CNTR frame and
   * word-aligned instruction stream.
   */
  artifactBytes: Uint8Array;
  codeHashHex: string;
  abiHashHex: string;
  compilerFingerprint: string;
  manifest: KotodamaCompiledManifestMetadata;
  sourceMap: KotodamaCompiledSourceMapEntry[];
  budgetReport: KotodamaCompiledBudgetEntry[];
}

/** Compiler errors are values; transport and malformed responses reject the promise. */
export type KotodamaCompilerResult =
  | { ok: true; output: KotodamaCompilerOutput }
  | { ok: false; diagnostics: KotodamaCompilerDiagnostic[] };

export declare function compileKotodamaProgram(
  source: string,
  options?: KotodamaCompilerOptions,
): Promise<KotodamaCompilerResult>;

export declare class KotodamaCompilerClient {
  constructor(baseUrl: string, options?: { fetchImpl?: typeof fetch });
  compile(
    source: string,
    options?: KotodamaCompilerCallOptions,
  ): Promise<KotodamaCompilerResult>;
}
