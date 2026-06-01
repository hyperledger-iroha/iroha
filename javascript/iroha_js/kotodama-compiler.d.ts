export type KotodamaStudioCompilerDiagnosticPhase =
  | 'lex'
  | 'parse'
  | 'semantic'
  | 'lowering'
  | 'artifact';

export interface KotodamaStudioCompilerDiagnostic {
  severity: 'error' | 'warning'
  message: string
  line?: number
  column?: number
  code?: string | null
  phase?: KotodamaStudioCompilerDiagnosticPhase | null
}

export interface KotodamaStudioCompiledTriggerDescriptor {
  id: string
  callback: {
    namespace: string | null
    entrypoint: string
  }
}

export interface KotodamaStudioCompiledManifestEntrypointKind {
  kind: 'Public' | 'View' | 'Hajimari' | 'Kaizen'
  value: null
}

export interface KotodamaStudioCompiledKotobaTranslation {
  lang: string
  text: string
}

export interface KotodamaStudioCompiledKotobaEntry {
  msg_id: string
  translations: KotodamaStudioCompiledKotobaTranslation[]
}

export interface KotodamaStudioCompiledEntrypoint {
  name: string
  kind: 'public' | 'view' | 'hajimari' | 'kaizen' | KotodamaStudioCompiledManifestEntrypointKind
  params: Array<{
    name: string
    type_name: string
  }>
  return_type: string | null
  permission: string | null
  read_keys: string[]
  write_keys: string[]
  access_hints_complete: boolean | null
  access_hints_skipped: string[]
  triggers: KotodamaStudioCompiledTriggerDescriptor[]
}

export interface KotodamaStudioCompiledSourceMapEntry {
  function_name: string
  pc_start: number
  pc_end: number
  source_path: string | null
  line: number
  column: number
}

export interface KotodamaStudioCompiledBudgetEntry {
  function_name: string
  pc_start: number
  pc_end: number
  bytecode_bytes: number
  bytecode_words: number
  frame_bytes: number
  jump_span_words: number
  jump_range_risk: boolean
  source_path: string | null
  line: number | null
  column: number | null
}

export interface KotodamaStudioCompiledDynamicAccessHint {
  base_key: string
  key_type: string
  bound_kind: string
  max_keys: number
}

export interface KotodamaStudioCompiledStateDescriptor {
  name: string
  type_name: string
}

export interface KotodamaStudioCompiledManifestMetadata {
  code_hash: string
  abi_hash: string
  compiler_fingerprint: string
  features_bitmap: number
  entrypoints: KotodamaStudioCompiledEntrypoint[]
  access_set_hints: {
    read_keys: string[]
    write_keys: string[]
    dynamic_reads: KotodamaStudioCompiledDynamicAccessHint[]
    dynamic_writes: KotodamaStudioCompiledDynamicAccessHint[]
  } | null
  states: KotodamaStudioCompiledStateDescriptor[]
  kotoba: KotodamaStudioCompiledKotobaEntry[] | null
  provenance: null
}

export interface KotodamaStudioCompilerOptions {
  sourceName?: string | null
  embedDebug?: boolean
  mode?: 'production' | 'Production' | 'test' | 'Test' | null
  forceZk?: boolean
  forceVector?: boolean
}

export interface KotodamaStudioCompilerOutput {
  artifactBytes: Uint8Array
  codeHashHex: string
  abiHashHex: string
  compilerFingerprint: string
  diagnostics: KotodamaStudioCompilerDiagnostic[]
  warnings: KotodamaStudioCompilerDiagnostic[]
  manifest: KotodamaStudioCompiledManifestMetadata | null
  sourceMap: KotodamaStudioCompiledSourceMapEntry[]
  budgetReport: KotodamaStudioCompiledBudgetEntry[]
}


export declare function compileKotodamaStudioProgram(source: string, options?: KotodamaStudioCompilerOptions): KotodamaStudioCompilerOutput;

export declare function compileKotodamaProgram(source: string, options?: KotodamaStudioCompilerOptions): KotodamaStudioCompilerOutput;

export type KotodamaParitySourceMapEntry = KotodamaStudioCompiledSourceMapEntry;
export type KotodamaParityBudgetEntry = KotodamaStudioCompiledBudgetEntry;

export interface KotodamaParitySuccessExpectationV1 {
  kind: 'success'
  artifact_b64: string
  manifest: KotodamaStudioCompiledManifestMetadata | null
  source_map: KotodamaParitySourceMapEntry[]
  budget_report: KotodamaParityBudgetEntry[]
  diagnostics: KotodamaStudioCompilerDiagnostic[]
  compiler_stderr: string[]
}

export interface KotodamaParityFailureExpectationV1 {
  kind: 'failure'
  diagnostics: KotodamaStudioCompilerDiagnostic[]
  compiler_stderr: string[]
}

export type KotodamaParityFixtureExpectationV1 =
  | KotodamaParitySuccessExpectationV1
  | KotodamaParityFailureExpectationV1;

export interface KotodamaParityFixtureV1 {
  version: 1
  id: string
  source_name: string
  compiler_source_path?: string
  source: string
  rust: KotodamaParityFixtureExpectationV1
}

export declare function normalizeKotodamaParitySource(source: string): string;

export declare function normalizeKotodamaParitySourcePath(sourcePath: string | null): string | null;

export declare function normalizeKotodamaParityCompilerStderr(stderr: string): string[];

export declare function normalizeKotodamaParityDiagnostic(
  diagnostic: KotodamaStudioCompilerDiagnostic
): KotodamaStudioCompilerDiagnostic;

export declare function createKotodamaParityDiagnostic(input: {
  severity?: KotodamaStudioCompilerDiagnostic['severity']
  message: string
  line?: number
  column?: number
  code?: string | null
  phase?: KotodamaStudioCompilerDiagnostic['phase']
}): KotodamaStudioCompilerDiagnostic;
