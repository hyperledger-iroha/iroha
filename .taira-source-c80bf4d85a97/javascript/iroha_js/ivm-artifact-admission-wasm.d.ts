export const IVM_ARTIFACT_ADMISSION_MAX_INPUT_BYTES: 4194304;

export type IvmArtifactAdmissionWasmBytes =
  | ArrayBuffer
  | ArrayBufferView
  | Uint8Array
  | {
      arrayBuffer(): ArrayBuffer | Promise<ArrayBuffer>;
    };

export interface InstantiateIvmArtifactAdmissionWasmOptions {
  wasmBytes: IvmArtifactAdmissionWasmBytes;
  /** SHA-256 trust anchor from signed application or release metadata. */
  expectedSha256Hex: string;
  imports?: WebAssembly.Imports;
}

export interface AcceptedIvmContractArtifactAdmission {
  readonly ok: true;
  readonly codeHashHex: string;
  readonly abiHashHex: string;
  readonly headerLength: number;
  readonly codeOffset: number;
  readonly entrypointCount: number;
  readonly manifest: Readonly<Record<string, unknown>>;
}

export interface RejectedIvmContractArtifactAdmission {
  readonly ok: false;
  readonly error: string;
}

export type IvmContractArtifactAdmissionResult =
  | AcceptedIvmContractArtifactAdmission
  | RejectedIvmContractArtifactAdmission;

export interface IvmArtifactAdmissionWasmVerifier {
  readonly verifierSha256Hex: string;
  verify(
    artifactBytes: ArrayBuffer | ArrayBufferView | Uint8Array,
  ): IvmContractArtifactAdmissionResult;
}

export function instantiateIvmArtifactAdmissionWasm(
  options: InstantiateIvmArtifactAdmissionWasmOptions,
): Promise<IvmArtifactAdmissionWasmVerifier>;

export function verifyIvmContractArtifactAdmission(
  verifier: IvmArtifactAdmissionWasmVerifier,
  artifactBytes: ArrayBuffer | ArrayBufferView | Uint8Array,
): IvmContractArtifactAdmissionResult;
