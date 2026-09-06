import {
  NetworkId,
  type IvmProvedContractCallInput,
  type RequiredIvmOverlayTransfer,
} from "../../../index.js";

const privateKey = new Uint8Array(32);
const networkId = NetworkId.parse(
  "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
);
const feePayment = {
  payer: "authority" as const,
  chargeLimits: [],
  gasLimit: 5_000,
};
const requiredOverlayTransfer: RequiredIvmOverlayTransfer = {
  sourceAssetHoldingId: "asset#account",
  quantity: "1",
  destinationAccountId: "destination",
};

const camel: IvmProvedContractCallInput = {
  networkId,
  authority: "account",
  privateKey,
  vkRef: { backend: "halo2/ipa", name: "ivm-exec-v1" },
  contractAlias: "router::dex.universal",
  feePayment,
  requiredOverlayTransfer,
  expectedCodeHashHex: "11".repeat(32),
  expectedArtifactSha256Hex: "22".repeat(32),
};

type AssertNever<Value extends never> = Value;
type RemovedInputAliases = AssertNever<
  Extract<
    keyof IvmProvedContractCallInput,
    | "private_key"
    | "private_key_algorithm"
    | "vk_ref"
    | "contract_address"
    | "contract_alias"
    | "fee_payment"
    | "required_overlay_transfer"
    | "creation_time_ms"
    | "ttl_ms"
    | "expected_code_hash_hex"
    | "expected_artifact_sha256_hex"
  >
>;
type RemovedOverlayAliases = AssertNever<
  Extract<
    keyof RequiredIvmOverlayTransfer,
    | "source_asset_holding_id"
    | "sourceAssetId"
    | "source_asset_id"
    | "destination_account_id"
  >
>;

void camel;
void requiredOverlayTransfer;
void (null as RemovedInputAliases);
void (null as RemovedOverlayAliases);

const retiredSnakeChain: IvmProvedContractCallInput = {
  ...camel,
  // @ts-expect-error retired chain_id is not an ordinary-transaction field.
  chain_id: "test-chain",
};

const retiredCamelChain: IvmProvedContractCallInput = {
  ...camel,
  // @ts-expect-error retired chainId is not an ordinary-transaction field.
  chainId: "test-chain",
};

const retiredBareChain: IvmProvedContractCallInput = {
  ...camel,
  // @ts-expect-error retired chain is not an ordinary-transaction field.
  chain: "test-chain",
};

// @ts-expect-error address and alias target selectors are mutually exclusive.
const duplicateTarget: IvmProvedContractCallInput = {
  ...camel,
  contractAddress: "irohac1contract",
};

// @ts-expect-error both independently trusted artifact identities are required.
const missingArtifactHash: IvmProvedContractCallInput = {
  networkId,
  authority: "account",
  privateKey,
  vkRef: { backend: "halo2/ipa", name: "ivm-exec-v1" },
  contractAlias: "router::dex.universal",
  feePayment,
  expectedCodeHashHex: "11".repeat(32),
};

void retiredSnakeChain;
void retiredCamelChain;
void retiredBareChain;
void duplicateTarget;
void missingArtifactHash;
