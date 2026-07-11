import type {
  IvmProvedContractCallInput,
  IvmValidationFeePolicyIntent,
  ValidationFeeIvmProvedContractCallInput,
} from "../../../index.js";

const privateKey = new Uint8Array(32);
const validationFeePolicy = {} as IvmValidationFeePolicyIntent;

const camel: IvmProvedContractCallInput = {
  chainId: "test-chain",
  authority: "account",
  privateKey,
  vkRef: { backend: "halo2/ipa", name: "ivm-exec-v1" },
  contractAlias: "router::dex.universal",
  gasLimit: 5_000,
  expectedCodeHashHex: "11".repeat(32),
  expectedArtifactSha256Hex: "22".repeat(32),
};

const snake: IvmProvedContractCallInput = {
  chain_id: "test-chain",
  authority: "account",
  private_key: privateKey,
  vk_ref: { backend: "halo2/ipa", name: "ivm-exec-v1" },
  contract_address: "tairac1contract",
  gas_limit: 5_000,
  expected_code_hash_hex: "11".repeat(32),
  expected_artifact_sha256_hex: "22".repeat(32),
};

const strict: ValidationFeeIvmProvedContractCallInput = {
  ...camel,
  validationFeePolicy,
};

void camel;
void snake;
void strict;

// @ts-expect-error camel/snake aliases are mutually exclusive.
const duplicateChain: IvmProvedContractCallInput = {
  ...camel,
  chain_id: "test-chain",
};

// @ts-expect-error address and alias target selectors are mutually exclusive.
const duplicateTarget: IvmProvedContractCallInput = {
  ...camel,
  contractAddress: "tairac1contract",
};

// @ts-expect-error both independently trusted artifact identities are required.
const missingArtifactHash: IvmProvedContractCallInput = {
  chainId: "test-chain",
  authority: "account",
  privateKey,
  vkRef: { backend: "halo2/ipa", name: "ivm-exec-v1" },
  contractAlias: "router::dex.universal",
  gasLimit: 5_000,
  expectedCodeHashHex: "11".repeat(32),
};

void duplicateChain;
void duplicateTarget;
void missingArtifactHash;
