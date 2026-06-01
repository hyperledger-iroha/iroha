package org.hyperledger.iroha.android.sccp;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.List;
import java.util.Objects;
import java.util.Set;
import org.bouncycastle.asn1.x9.X9ECParameters;
import org.bouncycastle.crypto.digests.KeccakDigest;
import org.bouncycastle.crypto.ec.CustomNamedCurves;
import org.hyperledger.iroha.android.crypto.Blake2b;

/** Source-chain SCCP proof-hash helpers for local-first Android proof generation. */
public final class SourceSccpProofs {
  public static final int DOMAIN_SORA = 0;
  public static final int DOMAIN_ETH = 1;
  public static final int DOMAIN_BSC = 2;
  public static final int DOMAIN_SOL = 3;
  public static final int DOMAIN_TON = 4;
  public static final int DOMAIN_TRON = 5;
  public static final int DOMAIN_SORA_KUSAMA = 6;
  public static final int DOMAIN_SORA_POLKADOT = 7;
  public static final int DOMAIN_SORA2 = 8;
  public static final long ETH_MAINNET_CHAIN_ID = 1L;
  public static final String ETH_MAINNET_NETWORK_ID =
      "0x0000000000000000000000000000000000000000000000000000000000000001";
  public static final long BSC_MAINNET_CHAIN_ID = 56L;
  public static final String BSC_MAINNET_NETWORK_ID =
      "0x0000000000000000000000000000000000000000000000000000000000000038";

  private static final BigInteger MAX_U64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);
  private static final int BSC_PARLIA_EXTRA_VANITY_BYTES = 32;
  private static final int BSC_PARLIA_EXTRA_SEAL_BYTES = 65;
  private static final int BSC_PARLIA_VALIDATOR_ADDRESS_BYTES = 20;
  private static final int BSC_PARLIA_VALIDATOR_BLS_KEY_BYTES = 48;
  private static final int BSC_MAX_PARLIA_VALIDATORS = 255;
  private static final int BSC_MAX_VALIDATOR_SET_PAYLOAD_BYTES =
      1 + 4 + BSC_MAX_PARLIA_VALIDATORS * (BSC_PARLIA_VALIDATOR_ADDRESS_BYTES + 8);
  private static final long BSC_PARLIA_EPOCH_LENGTH_BLOCKS = 200L;
  private static final String SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1 =
      "sccp-source-adapter-v1";
  private static final String SOURCE_ADAPTER_FASTPQ_PARAMETER_SET_V1 =
      "fastpq-lane-balanced";
  private static final String STARK_FRI_PROOF_FAMILY_V1 = "stark-fri-v1";
  private static final String EVM_DESTINATION_BINDING_LABEL_V1 =
      "iroha:sccp:evm-destination-binding:v1";
  private static final String TRON_DESTINATION_BINDING_LABEL_V1 =
      "iroha:sccp:tron-destination-binding:v1";
  private static final String BASE58_ALPHABET =
      "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
  public static final String SUBSTRATE_RUNTIME_STORAGE_OPEN_VERIFY_CIRCUIT_ID_V1 =
      "sccp-substrate-runtime-storage-v1";
  public static final String SOLANA_MAINNET_TOWER_REPLAY_VERIFIER_ID_V1 =
      "sccp:sol:light-client:tower-replay-mainnet-beta:v1";
  public static final String SOLANA_MAINNET_FULL_ACCOUNTSDB_LATTICE_VERIFIER_ID_V1 =
      "sccp:sol:light-client:full-accountsdb-lattice-mainnet-beta:v1";
  public static final String SOLANA_MAINNET_BANK_FORK_CHOICE_VERIFIER_ID_V1 =
      "sccp:sol:light-client:bank-fork-choice-mainnet-beta:v1";
  public static final String TON_MAINNET_MASTERCHAIN_CONFIG_VERIFIER_ID_V1 =
      "sccp:ton:light-client:masterchain-config-mainnet:v1";
  public static final String TON_MAINNET_VALIDATOR_SET_TRANSITION_VERIFIER_ID_V1 =
      "sccp:ton:light-client:validator-set-transition-mainnet:v1";
  public static final String TON_MAINNET_SHARD_ACCOUNTS_DICTIONARY_VERIFIER_ID_V1 =
      "sccp:ton:light-client:shard-accounts-dictionary-mainnet:v1";
  private static final String DESTINATION_BINDING_PREFIX_V1 = "sccp:destination:binding:v1";
  private static final String SOLANA_MAINNET_GENESIS_HASH = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp";
  private static final long SOURCE_ADAPTER_FASTPQ_TRACE_ROOT_V1 = 0x002A_247F_81C6_F850L;
  private static final long SOURCE_ADAPTER_FASTPQ_LDE_ROOT_V1 = 0x6026_3388_DBBF_9B2AL;
  private static final long SOURCE_ADAPTER_FASTPQ_OMEGA_COSET_V1 = 0x6AF3_25E8_25AD_5C18L;
  private static final int EVM_MAX_RECEIPT_VALUE_BYTES = 16 * 1024;
  private static final int ETH_EXECUTION_PAYLOAD_BODY_FIELD_INDEX = 9;
  private static final int ETH_EXECUTION_PAYLOAD_BODY_BRANCH_DEPTH = 4;
  private static final int ETH_MAX_SYNC_COMMITTEE_AUTHORITIES = 512;
  private static final int ETH_SYNC_COMMITTEE_PUBLIC_KEY_BYTES = 48;
  private static final int ETH_SYNC_COMMITTEE_POP_BYTES = 96;
  private static final int ETH_SYNC_COMMITTEE_SIGNATURE_BYTES = 96;
  private static final int ETH_MAX_SYNC_COMMITTEE_PUBLIC_KEY_BYTES = 96;
  private static final int ETH_MAX_SYNC_COMMITTEE_POP_BYTES = 256;
  private static final int ETH_MAX_SYNC_COMMITTEE_PAYLOAD_BYTES =
      1 + 4 + ETH_MAX_SYNC_COMMITTEE_AUTHORITIES
          * (4 + ETH_MAX_SYNC_COMMITTEE_PUBLIC_KEY_BYTES + 8 + 4
              + ETH_MAX_SYNC_COMMITTEE_POP_BYTES);
  private static final int TON_MAX_VALIDATORS = 1024;
  private static final int TRON_MAX_MPT_PROOF_NODES = 64;
  private static final int TRON_MAX_MPT_NODE_BYTES = 16 * 1024;
  private static final int TRON_MAX_RAW_HEADER_BYTES = 16 * 1024;
  private static final int TRON_MAX_RECEIPT_VALUE_BYTES = 16 * 1024;
  private static final int TRON_MAX_TRANSACTION_BYTES = 64 * 1024;
  private static final int TRON_MAX_TRANSACTION_MERKLE_BRANCH_NODES = 64;
  private static final int TRON_SOURCE_CALL_SIGNATURES = 1;
  private static final int TRON_MAX_WITNESSES = 64;
  private static final int SUBSTRATE_MAX_AUTHORITIES = 2048;
  private static final int SUBSTRATE_MAX_AUTHORITY_SET_PAYLOAD_BYTES =
      1 + 4 + SUBSTRATE_MAX_AUTHORITIES * (32 + 8);
  private static final byte[] SECP256K1_SCALAR_ORDER_BE =
      hexBytes("fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141",
          "secp256k1Order", 32);
  private static final byte[] SECP256K1_SCALAR_HALF_ORDER_BE =
      hexBytes("7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a0",
          "secp256k1HalfOrder", 32);
  private static final X9ECParameters SECP256K1_PARAMS =
      CustomNamedCurves.getByName("secp256k1");
  private static final BigInteger SECP256K1_SCALAR_ORDER = SECP256K1_PARAMS.getN();
  private static final BigInteger SECP256K1_FIELD_PRIME =
      new BigInteger("fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f", 16);
  private static final byte[] EVM_RECEIPT_ROOT_VALUE_MARKER =
      "sccp:evm:receipt-root-value:v1".getBytes(StandardCharsets.UTF_8);
  private static final byte[] TRON_RECEIPT_ROOT_VALUE_MARKER =
      "sccp:tron:receipt-root-value:v1".getBytes(StandardCharsets.UTF_8);
  private static final byte[] TRON_SOURCE_MESSAGE_CALL_ABI =
      "submitSccpSourceEvent(uint32,uint32,bytes32)".getBytes(StandardCharsets.UTF_8);
  private static final byte[] TRON_TRIGGER_SMART_CONTRACT_TYPE_URL =
      "type.googleapis.com/protocol.TriggerSmartContract".getBytes(StandardCharsets.UTF_8);
  private static final byte[] TRON_SOURCE_BRIDGE_CONFIG_LABEL =
      "iroha:sccp:tron-source-bridge-config:v1".getBytes(StandardCharsets.UTF_8);
  private static final byte[] SUBSTRATE_SYSTEM_EVENTS_STORAGE_KEY =
      hexBytes(
          "0x26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7",
          "systemEventsStorageKey",
          32);
  private static final String SUBSTRATE_RUNTIME_STORAGE_PROOF_PUBLIC_INPUTS_PREFIX_V1 =
      "sccp:substrate:runtime-storage-proof-public-inputs:v1";
  private static final String SUBSTRATE_RUNTIME_STORAGE_FASTPQ_DSID_PREFIX_V1 =
      "sccp:substrate:runtime-storage:fastpq:dsid:v1";
  private static final String SUBSTRATE_RUNTIME_STORAGE_FASTPQ_PARAMETER_SET_V1 =
      "fastpq-lane-balanced";
  private static final String SUBSTRATE_RUNTIME_STORAGE_FASTPQ_STATEMENT_KEY_V1 =
      "sccp:substrate:runtime-storage:v1:statement";
  private static final String SUBSTRATE_RUNTIME_STORAGE_FASTPQ_CONTEXT_KEY_V1 =
      "sccp:substrate:runtime-storage:v1:context";
  private static final String SUBSTRATE_RUNTIME_STORAGE_FASTPQ_STORAGE_KEY_V1 =
      "sccp:substrate:runtime-storage:v1:storage-key";
  private static final byte[] SORA_KUSAMA_TEMPLATE_SOURCE_STATE_VERIFIER_HASH =
      hexBytes(
          "0xaf2d28b3e07447239f28e90ce4fdee7e6cd3778c087eaeda7170781eb4b76b9c",
          "soraKusamaTemplateSourceStateVerifierHash",
          32);
  private static final byte[] SORA_POLKADOT_TEMPLATE_SOURCE_STATE_VERIFIER_HASH =
      hexBytes(
          "0x664576f1a2409099c3b7dba82512c8757501f2869aedda0e45f858572b940b5d",
          "soraPolkadotTemplateSourceStateVerifierHash",
          32);
  private static final byte[] SORA2_TEMPLATE_SOURCE_STATE_VERIFIER_HASH =
      hexBytes(
          "0x20509eb56524c727b6d028cc6b43f10c17048d31b92d5a96d41c0512d16267ef",
          "sora2TemplateSourceStateVerifierHash",
          32);
  private static final byte[] TON_TEMPLATE_SOURCE_STATE_VERIFIER_HASH =
      hexBytes(
          "0x540205f876591604ccf39f72a051ac5e82647c9e48dbd48cb129d2543971a34f",
          "tonTemplateSourceStateVerifierHash",
          32);
  private static final byte[] SOLANA_TEMPLATE_SOURCE_STATE_VERIFIER_HASH =
      hexBytes(
          SolanaSccpProver.TEMPLATE_SOURCE_STATE_VERIFIER_HASH_V1,
          "solanaTemplateSourceStateVerifierHash",
          32);
  private static final byte[] SOLANA_TEMPLATE_SOURCE_TRUST_ANCHOR_HASH =
      hexBytes(
          "0x113bdb7601d84f2098daec386346a7123857d181b3ac5bd23df50fa9e1b2cbe3",
          "solanaTemplateSourceTrustAnchorHash",
          32);
  private static final byte[] SOLANA_TEMPLATE_CONSENSUS_VERIFIER_HASH =
      hexBytes(
          "0x97ea89019e6c79305d06dfc27640ee14a6b42ba6eaf86e1835ee9b433dba48ba",
          "solanaTemplateConsensusVerifierHash",
          32);
  private static final byte[] SOLANA_TEMPLATE_MESSAGE_INCLUSION_VERIFIER_HASH =
      hexBytes(
          "0xb8358bfef1e428a6a7e9115687cb2b88d9c21dad4021bea3e11d43489eb3dcb0",
          "solanaTemplateMessageInclusionVerifierHash",
          32);
  private static final byte[] SOLANA_TEMPLATE_FINALITY_POLICY_HASH =
      hexBytes(
          "0x9df7ea90cf1bbba036788b14804f63f4be1e908390be89524fd4486f74344f56",
          "solanaTemplateFinalityPolicyHash",
          32);
  private static final byte[] TON_TEMPLATE_SOURCE_TRUST_ANCHOR_HASH =
      hexBytes(
          "0xd83b3a3eb920ac8338533535cf0d6c69c69d507e84aef8ec2094564b8427c56c",
          "tonTemplateSourceTrustAnchorHash",
          32);
  private static final byte[] TON_TEMPLATE_CONSENSUS_VERIFIER_HASH =
      hexBytes(
          "0xb0225e16477ea3420f7d0de76b87b6e99a43ab97f445d8565a384d4b655bc473",
          "tonTemplateConsensusVerifierHash",
          32);
  private static final byte[] TON_TEMPLATE_MESSAGE_INCLUSION_VERIFIER_HASH =
      hexBytes(
          "0x89254256421c15da8c92842c7d6f448ef6c1d5ca1e2a173754643425fcee6353",
          "tonTemplateMessageInclusionVerifierHash",
          32);
  private static final byte[] TON_TEMPLATE_FINALITY_POLICY_HASH =
      hexBytes(
          "0x50044ee6db0eb0cdef097e69406b6c30d3406d8f784e8ba34e9b923b38bd0c43",
          "tonTemplateFinalityPolicyHash",
          32);
  private static final byte[] TRON_TEMPLATE_SOURCE_TRUST_ANCHOR_HASH =
      hexBytes(
          "0x3550934cbdfe49449ec4aa383dcea7674541fedf66ab6159b1ed2f2c0be4755c",
          "tronTemplateSourceTrustAnchorHash",
          32);
  private static final byte[] TRON_TEMPLATE_CONSENSUS_VERIFIER_HASH =
      hexBytes(
          "0x8a1de96a869b2f28f197a7835597f17cf77ff45f7cbb77da2f7c48e87df8c5ea",
          "tronTemplateConsensusVerifierHash",
          32);
  private static final byte[] TRON_TEMPLATE_MESSAGE_INCLUSION_VERIFIER_HASH =
      hexBytes(
          "0xf39db56474b288680ad9561389cca7a841bd1fd223719255324705e1038fcacc",
          "tronTemplateMessageInclusionVerifierHash",
          32);
  private static final byte[] TRON_TEMPLATE_FINALITY_POLICY_HASH =
      hexBytes(
          "0xad5a6a4f200e070400b5aaa1b7976c639e67571eb711eb6f69d01e3615423864",
          "tronTemplateFinalityPolicyHash",
          32);
  private static final int TON_MASTERCHAIN_WORKCHAIN_ID = -1;
  private static final BigInteger TON_MASTERCHAIN_SHARD = BigInteger.ONE.shiftLeft(63);
  private static final int TON_BASECHAIN_WORKCHAIN_ID = 0;

  private SourceSccpProofs() {}

  /** FastPQ public inputs used by Substrate runtime-storage source-state proofs. */
  public static final class SubstrateRuntimeStorageFastpqPublicInputs {
    public final String dsid;
    public final String slot;
    public final String oldRoot;
    public final String newRoot;
    public final String permRoot;
    public final String txSetHash;

    private SubstrateRuntimeStorageFastpqPublicInputs(
        final String dsid,
        final String slot,
        final String oldRoot,
        final String newRoot,
        final String permRoot,
        final String txSetHash) {
      this.dsid = dsid;
      this.slot = slot;
      this.oldRoot = oldRoot;
      this.newRoot = newRoot;
      this.permRoot = permRoot;
      this.txSetHash = txSetHash;
    }
  }

  /** FastPQ metadata transition used by Substrate runtime-storage source-state proofs. */
  public static final class SubstrateRuntimeStorageFastpqTransition {
    public final String key;
    public final String operation;
    public final String oldValue;
    public final String newValue;

    private SubstrateRuntimeStorageFastpqTransition(
        final String key, final String operation, final String oldValue, final String newValue) {
      this.key = key;
      this.operation = operation;
      this.oldValue = oldValue;
      this.newValue = newValue;
    }
  }

  /** Canonical governed EVM-family destination binding derived from deployment material. */
  public static final class EvmDestinationBinding {
    public final int version;
    public final int sourceDomain;
    public final int targetDomain;
    public final String networkId;
    public final String verifierAddress;
    public final String bridgeAddress;
    public final String verifierCodeHash;
    public final String verifierKeyHash;
    public final String verifierBackend;
    public final String proofFamily;
    public final String key;
    public final String hash;

    private EvmDestinationBinding(
        final int version,
        final int sourceDomain,
        final int targetDomain,
        final String networkId,
        final String verifierAddress,
        final String bridgeAddress,
        final String verifierCodeHash,
        final String verifierKeyHash,
        final String verifierBackend,
        final String proofFamily,
        final String key,
        final String hash) {
      this.version = version;
      this.sourceDomain = sourceDomain;
      this.targetDomain = targetDomain;
      this.networkId = networkId;
      this.verifierAddress = verifierAddress;
      this.bridgeAddress = bridgeAddress;
      this.verifierCodeHash = verifierCodeHash;
      this.verifierKeyHash = verifierKeyHash;
      this.verifierBackend = verifierBackend;
      this.proofFamily = proofFamily;
      this.key = key;
      this.hash = hash;
    }
  }

  /** Canonical governed TRON destination binding derived from deployment material. */
  public static final class TronDestinationBinding {
    public final int version;
    public final int sourceDomain;
    public final int targetDomain;
    public final String networkId;
    public final String verifierAddress;
    public final String verifierCodeHash;
    public final String verifierKeyHash;
    public final String verifierBackend;
    public final String proofFamily;
    public final String key;
    public final String hash;

    private TronDestinationBinding(
        final int version,
        final int sourceDomain,
        final int targetDomain,
        final String networkId,
        final String verifierAddress,
        final String verifierCodeHash,
        final String verifierKeyHash,
        final String verifierBackend,
        final String proofFamily,
        final String key,
        final String hash) {
      this.version = version;
      this.sourceDomain = sourceDomain;
      this.targetDomain = targetDomain;
      this.networkId = networkId;
      this.verifierAddress = verifierAddress;
      this.verifierCodeHash = verifierCodeHash;
      this.verifierKeyHash = verifierKeyHash;
      this.verifierBackend = verifierBackend;
      this.proofFamily = proofFamily;
      this.key = key;
      this.hash = hash;
    }
  }

  /** Deterministic proof request for Android Substrate runtime-storage provers. */
  public static final class SubstrateRuntimeStorageProofRequest {
    public final int version;
    public final String proofFamily;
    public final String circuitId;
    public final String parameterSet;
    public final int sourceDomain;
    public final String finalizedBlockNumber;
    public final String grandpaSetId;
    public final String sourceStateVerifierId;
    public final String sourceStateVerifierHash;
    public final String runtimeStorageProofPublicInputsHash;
    public final String storageProofHash;
    private final byte[] statementBytes;
    private final byte[] verificationContextBytes;
    private final byte[] schemaDescriptor;
    public final List<List<String>> publicInputColumns;
    public final SubstrateRuntimeStorageFastpqPublicInputs fastpqPublicInputs;
    public final List<SubstrateRuntimeStorageFastpqTransition> fastpqTransitions;

    private SubstrateRuntimeStorageProofRequest(
        final int version,
        final String proofFamily,
        final String circuitId,
        final String parameterSet,
        final int sourceDomain,
        final String finalizedBlockNumber,
        final String grandpaSetId,
        final String sourceStateVerifierId,
        final String sourceStateVerifierHash,
        final String runtimeStorageProofPublicInputsHash,
        final String storageProofHash,
        final byte[] statementBytes,
        final byte[] verificationContextBytes,
        final byte[] schemaDescriptor,
        final List<List<String>> publicInputColumns,
        final SubstrateRuntimeStorageFastpqPublicInputs fastpqPublicInputs,
        final List<SubstrateRuntimeStorageFastpqTransition> fastpqTransitions) {
      this.version = version;
      this.proofFamily = proofFamily;
      this.circuitId = circuitId;
      this.parameterSet = parameterSet;
      this.sourceDomain = sourceDomain;
      this.finalizedBlockNumber = finalizedBlockNumber;
      this.grandpaSetId = grandpaSetId;
      this.sourceStateVerifierId = sourceStateVerifierId;
      this.sourceStateVerifierHash = sourceStateVerifierHash;
      this.runtimeStorageProofPublicInputsHash = runtimeStorageProofPublicInputsHash;
      this.storageProofHash = storageProofHash;
      this.statementBytes = Arrays.copyOf(statementBytes, statementBytes.length);
      this.verificationContextBytes = Arrays.copyOf(verificationContextBytes, verificationContextBytes.length);
      this.schemaDescriptor = Arrays.copyOf(schemaDescriptor, schemaDescriptor.length);
      this.publicInputColumns = copyStringColumns(publicInputColumns);
      this.fastpqPublicInputs = fastpqPublicInputs;
      this.fastpqTransitions = Collections.unmodifiableList(new ArrayList<>(fastpqTransitions));
    }

    public byte[] statementBytes() {
      return Arrays.copyOf(statementBytes, statementBytes.length);
    }

    public byte[] verificationContextBytes() {
      return Arrays.copyOf(verificationContextBytes, verificationContextBytes.length);
    }

    public byte[] schemaDescriptor() {
      return Arrays.copyOf(schemaDescriptor, schemaDescriptor.length);
    }
  }

  /** One BSC ValidatorSet storage-slot proof transcript entry. */
  public static final class BscValidatorStorageProof {
    public final int version;
    public final int validatorIndex;
    public final String storageSlot;
    public final byte[] storageValue;
    public final String storageValueHash;
    public final List<byte[]> storageProofNodes;

    public BscValidatorStorageProof(
        final int version,
        final int validatorIndex,
        final String storageSlot,
        final byte[] storageValue,
        final String storageValueHash,
        final List<byte[]> storageProofNodes) {
      this.version = version;
      this.validatorIndex = validatorIndex;
      this.storageSlot = Objects.requireNonNull(storageSlot, "storageSlot");
      this.storageValue = Objects.requireNonNull(storageValue, "storageValue");
      this.storageValueHash = Objects.requireNonNull(storageValueHash, "storageValueHash");
      this.storageProofNodes = Objects.requireNonNull(storageProofNodes, "storageProofNodes");
    }
  }

  private static List<List<String>> copyStringColumns(final List<List<String>> columns) {
    Objects.requireNonNull(columns, "columns");
    final List<List<String>> copied = new ArrayList<>(columns.size());
    for (final List<String> column : columns) {
      copied.add(Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(column, "column"))));
    }
    return Collections.unmodifiableList(copied);
  }

  /** BSC ValidatorSet account/storage proof transcript material. */
  public static final class BscValidatorSetMetadataProof {
    public final int version;
    public final String stateRoot;
    public final String nextValidatorSetPayloadHash;
    public final byte[] validatorContractAddress;
    public final List<byte[]> accountProofNodes;
    public final String storageRoot;
    public final String validatorSetLengthSlot;
    public final byte[] validatorSetLengthValue;
    public final String validatorSetLengthValueHash;
    public final List<byte[]> validatorSetLengthProofNodes;
    public final List<BscValidatorStorageProof> validatorStorageProofs;

    public BscValidatorSetMetadataProof(
        final int version,
        final String stateRoot,
        final String nextValidatorSetPayloadHash,
        final byte[] validatorContractAddress,
        final List<byte[]> accountProofNodes,
        final String storageRoot,
        final String validatorSetLengthSlot,
        final byte[] validatorSetLengthValue,
        final String validatorSetLengthValueHash,
        final List<byte[]> validatorSetLengthProofNodes,
        final List<BscValidatorStorageProof> validatorStorageProofs) {
      this.version = version;
      this.stateRoot = Objects.requireNonNull(stateRoot, "stateRoot");
      this.nextValidatorSetPayloadHash =
          Objects.requireNonNull(nextValidatorSetPayloadHash, "nextValidatorSetPayloadHash");
      this.validatorContractAddress =
          Objects.requireNonNull(validatorContractAddress, "validatorContractAddress");
      this.accountProofNodes = Objects.requireNonNull(accountProofNodes, "accountProofNodes");
      this.storageRoot = Objects.requireNonNull(storageRoot, "storageRoot");
      this.validatorSetLengthSlot =
          Objects.requireNonNull(validatorSetLengthSlot, "validatorSetLengthSlot");
      this.validatorSetLengthValue =
          Objects.requireNonNull(validatorSetLengthValue, "validatorSetLengthValue");
      this.validatorSetLengthValueHash =
          Objects.requireNonNull(validatorSetLengthValueHash, "validatorSetLengthValueHash");
      this.validatorSetLengthProofNodes =
          Objects.requireNonNull(validatorSetLengthProofNodes, "validatorSetLengthProofNodes");
      this.validatorStorageProofs =
          Objects.requireNonNull(validatorStorageProofs, "validatorStorageProofs");
    }
  }

  /** BSC Parlia commit-seal transcript material. */
  public static final class BscCommitSealProof {
    public final int version;
    public final String totalPower;
    public final String signedPower;
    public final String commitMessageHash;
    public final List<byte[]> validatorPublicKeys;
    public final List<String> validatorPowers;
    public final byte[] signersBitmap;
    public final List<byte[]> signatures;
    public final String validatorSetHash;

    public BscCommitSealProof(
        final int version,
        final String totalPower,
        final String signedPower,
        final String commitMessageHash,
        final List<byte[]> validatorPublicKeys,
        final List<String> validatorPowers,
        final byte[] signersBitmap,
        final List<byte[]> signatures,
        final String validatorSetHash) {
      this.version = version;
      this.totalPower = Objects.requireNonNull(totalPower, "totalPower");
      this.signedPower = Objects.requireNonNull(signedPower, "signedPower");
      this.commitMessageHash = Objects.requireNonNull(commitMessageHash, "commitMessageHash");
      this.validatorPublicKeys = Objects.requireNonNull(validatorPublicKeys, "validatorPublicKeys");
      this.validatorPowers = Objects.requireNonNull(validatorPowers, "validatorPowers");
      this.signersBitmap = Objects.requireNonNull(signersBitmap, "signersBitmap");
      this.signatures = Objects.requireNonNull(signatures, "signatures");
      this.validatorSetHash = validatorSetHash;
    }
  }

  /** TON masterchain validator-signature transcript material. */
  public static final class TonValidatorSignatureProof {
    public final int version;
    public final String totalWeight;
    public final String signedWeight;
    public final String blockMessageHash;
    public final List<byte[]> validatorPublicKeys;
    public final List<String> validatorWeights;
    public final byte[] signersBitmap;
    public final List<byte[]> signatures;
    public final String validatorSetHash;

    public TonValidatorSignatureProof(
        final int version,
        final String totalWeight,
        final String signedWeight,
        final String blockMessageHash,
        final List<byte[]> validatorPublicKeys,
        final List<String> validatorWeights,
        final byte[] signersBitmap,
        final List<byte[]> signatures,
        final String validatorSetHash) {
      this.version = version;
      this.totalWeight = Objects.requireNonNull(totalWeight, "totalWeight");
      this.signedWeight = Objects.requireNonNull(signedWeight, "signedWeight");
      this.blockMessageHash = Objects.requireNonNull(blockMessageHash, "blockMessageHash");
      this.validatorPublicKeys =
          Objects.requireNonNull(validatorPublicKeys, "validatorPublicKeys");
      this.validatorWeights = Objects.requireNonNull(validatorWeights, "validatorWeights");
      this.signersBitmap = Objects.requireNonNull(signersBitmap, "signersBitmap");
      this.signatures = Objects.requireNonNull(signatures, "signatures");
      this.validatorSetHash = validatorSetHash;
    }

    public TonValidatorSignatureProof(
        final int version,
        final String totalWeight,
        final String signedWeight,
        final String blockMessageHash,
        final List<byte[]> validatorPublicKeys,
        final List<String> validatorWeights,
        final byte[] signersBitmap,
        final List<byte[]> signatures) {
      this(
          version,
          totalWeight,
          signedWeight,
          blockMessageHash,
          validatorPublicKeys,
          validatorWeights,
          signersBitmap,
          signatures,
          null);
    }
  }

  /** Canonical OpenVerify verifier-key commitment for an SCCP source-adapter lane. */
  public static String sourceAdapterVerifierVkHash(final int sourceDomain) {
    return sourceAdapterVerifierVkHash(sourceDomain, DOMAIN_SORA);
  }

  /** Canonical OpenVerify verifier-key commitment for an SCCP source-adapter lane. */
  public static String sourceAdapterVerifierVkHash(
      final int sourceDomain, final int targetDomain) {
    final int normalizedSourceDomain = normalizeDomain(sourceDomain, "sourceDomain");
    final int normalizedTargetDomain = normalizeDomain(targetDomain, "targetDomain");
    if (normalizedTargetDomain != DOMAIN_SORA) {
      throw new IllegalArgumentException(
          "targetDomain must be SORA for SCCP source-adapter verifier VKs");
    }
    final SourceAdapterVerifierProfile profile =
        sourceAdapterVerifierProfile(normalizedSourceDomain);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeVector(out, SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1.getBytes(StandardCharsets.UTF_8));
    writeVector(out, profile.chain.getBytes(StandardCharsets.UTF_8));
    writeU32Le(out, normalizedSourceDomain);
    writeU32Le(out, normalizedTargetDomain);
    out.write(profile.proofPlan);
    out.write(profile.finalityModel);
    writeVector(out, SOURCE_ADAPTER_FASTPQ_PARAMETER_SET_V1.getBytes(StandardCharsets.UTF_8));
    writeU32Le(out, 128);
    writeU32Le(out, 23);
    writeU32Le(out, 16);
    writeU64Le(out, BigInteger.valueOf(SOURCE_ADAPTER_FASTPQ_TRACE_ROOT_V1));
    writeU32Le(out, 19);
    writeU64Le(out, BigInteger.valueOf(SOURCE_ADAPTER_FASTPQ_LDE_ROOT_V1));
    writeU32Le(out, 65_536);
    out.write(1);
    writeU32Le(out, 19);
    writeU64Le(out, BigInteger.valueOf(SOURCE_ADAPTER_FASTPQ_OMEGA_COSET_V1));
    writeVector(out, "Goldilocks".getBytes(StandardCharsets.UTF_8));
    writeVector(out, "18446744069414584321".getBytes(StandardCharsets.UTF_8));
    writeU32Le(out, 2);
    writeVector(out, "Poseidon2(Goldilocks)".getBytes(StandardCharsets.UTF_8));
    writeVector(out, "SHA3-256".getBytes(StandardCharsets.UTF_8));
    writeU32Le(out, 8);
    writeU32Le(out, 8);
    writeU32Le(out, 8);
    writeU32Le(out, 46);
    final byte[] circuitId = SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1.getBytes(
        StandardCharsets.UTF_8);
    final byte[] verifier = out.toByteArray();
    final byte[] preimage = new byte[circuitId.length + verifier.length];
    System.arraycopy(circuitId, 0, preimage, 0, circuitId.length);
    System.arraycopy(verifier, 0, preimage, circuitId.length, verifier.length);
    return "0x" + hexLower(sha256(preimage));
  }

  /** Governed destination binding key for a native SCCP lane. */
  public static String destinationBindingKey(final int domain) {
    final int targetDomain = normalizeDomain(domain, "targetDomain");
    return destinationBindingProfile(targetDomain).bindingKey;
  }

  /** Canonical SccpDestinationBindingV1 hash for a native SCCP lane. */
  public static String destinationBindingHash(final int domain) {
    final int targetDomain = normalizeDomain(domain, "targetDomain");
    final DestinationBindingProfile profile = destinationBindingProfile(targetDomain);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, DOMAIN_SORA);
    writeU32Le(out, targetDomain);
    out.write(1);
    out.write(1);
    out.write(profile.verifierTarget);
    out.write(profile.backendFamily);
    writeVector(out, profile.bindingKey.getBytes(StandardCharsets.UTF_8));
    writeVector(out, profile.manifestSeed.getBytes(StandardCharsets.UTF_8));
    writeVector(out, STARK_FRI_PROOF_FAMILY_V1.getBytes(StandardCharsets.UTF_8));
    writeVector(out, profile.verifierBackend.getBytes(StandardCharsets.UTF_8));
    return hashHex(DESTINATION_BINDING_PREFIX_V1, out.toByteArray());
  }

  /** Governed EVM-family destination binding for UI-side SCCP proof generation. */
  public static EvmDestinationBinding evmDestinationBinding(
      final int sourceDomain,
      final int targetDomain,
      final String networkId,
      final String verifierAddress,
      final String bridgeAddress,
      final String verifierCodeHash,
      final String verifierKeyHash) {
    final int normalizedSourceDomain = normalizeDomain(sourceDomain, "sourceDomain");
    final int normalizedTargetDomain = normalizeDomain(targetDomain, "targetDomain");
    if (normalizedSourceDomain != DOMAIN_SORA) {
      throw new IllegalArgumentException("sourceDomain must be SORA");
    }
    if (normalizedTargetDomain != DOMAIN_ETH && normalizedTargetDomain != DOMAIN_BSC) {
      throw new IllegalArgumentException("targetDomain must be ETH or BSC");
    }
    final byte[] networkIdBytes = nonZeroHex32Bytes(networkId, "networkId");
    final byte[] verifierAddressBytes =
        nonZeroHexBytes(verifierAddress, "verifierAddress", 20);
    final byte[] bridgeAddressBytes = nonZeroHexBytes(bridgeAddress, "bridgeAddress", 20);
    if (Arrays.equals(verifierAddressBytes, bridgeAddressBytes)) {
      throw new IllegalArgumentException("bridgeAddress must differ from verifierAddress");
    }
    final byte[] verifierCodeHashBytes =
        nonZeroHex32Bytes(verifierCodeHash, "verifierCodeHash");
    final byte[] verifierKeyHashBytes = nonZeroHex32Bytes(verifierKeyHash, "verifierKeyHash");
    final String normalizedNetworkId = "0x" + hexLower(networkIdBytes);
    final String normalizedVerifierAddress = "0x" + hexLower(verifierAddressBytes);
    final String normalizedBridgeAddress = "0x" + hexLower(bridgeAddressBytes);
    final String normalizedVerifierCodeHash = "0x" + hexLower(verifierCodeHashBytes);
    final String normalizedVerifierKeyHash = "0x" + hexLower(verifierKeyHashBytes);
    final String key =
        "evm:"
            + normalizedSourceDomain
            + ":"
            + normalizedTargetDomain
            + ":"
            + hexLower(networkIdBytes)
            + ":"
            + normalizedVerifierAddress
            + ":"
            + normalizedBridgeAddress
            + ":"
            + normalizedVerifierCodeHash
            + ":"
            + normalizedVerifierKeyHash;

    final ByteArrayOutputStream payload = new ByteArrayOutputStream();
    write(payload, keccak256(EVM_DESTINATION_BINDING_LABEL_V1.getBytes(StandardCharsets.UTF_8)));
    write(payload, keccak256(EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1.getBytes(StandardCharsets.UTF_8)));
    write(payload, keccak256(STARK_FRI_PROOF_FAMILY_V1.getBytes(StandardCharsets.UTF_8)));
    write(payload, networkIdBytes);
    write(payload, abiWordU32(normalizedSourceDomain, "sourceDomain"));
    write(payload, abiWordU32(normalizedTargetDomain, "targetDomain"));
    write(payload, abiWordAddress20(verifierAddressBytes, "verifierAddress"));
    write(payload, abiWordAddress20(bridgeAddressBytes, "bridgeAddress"));
    write(payload, verifierCodeHashBytes);
    write(payload, verifierKeyHashBytes);
    final String hash = "0x" + hexLower(keccak256(payload.toByteArray()));
    return new EvmDestinationBinding(
        1,
        normalizedSourceDomain,
        normalizedTargetDomain,
        normalizedNetworkId,
        normalizedVerifierAddress,
        normalizedBridgeAddress,
        normalizedVerifierCodeHash,
        normalizedVerifierKeyHash,
        EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
        STARK_FRI_PROOF_FAMILY_V1,
        key,
        hash);
  }

  /** Canonical governed EVM-family destination binding hash for UI-side proof requests. */
  public static String evmDestinationBindingHash(
      final int sourceDomain,
      final int targetDomain,
      final String networkId,
      final String verifierAddress,
      final String bridgeAddress,
      final String verifierCodeHash,
      final String verifierKeyHash) {
    return evmDestinationBinding(
            sourceDomain,
            targetDomain,
            networkId,
            verifierAddress,
            bridgeAddress,
            verifierCodeHash,
        verifierKeyHash)
        .hash;
  }

  /** Governed Ethereum mainnet destination binding for Android SCCP proof generation. */
  public static EvmDestinationBinding ethereumMainnetDestinationBinding(
      final String verifierAddress,
      final String bridgeAddress,
      final String verifierCodeHash,
      final String verifierKeyHash) {
    return ethereumMainnetDestinationBinding(
        verifierAddress,
        bridgeAddress,
        verifierCodeHash,
        verifierKeyHash,
        ETH_MAINNET_NETWORK_ID);
  }

  /** Governed Ethereum mainnet destination binding for Android SCCP proof generation. */
  public static EvmDestinationBinding ethereumMainnetDestinationBinding(
      final String verifierAddress,
      final String bridgeAddress,
      final String verifierCodeHash,
      final String verifierKeyHash,
      final String networkId) {
    final EvmDestinationBinding binding =
        evmDestinationBinding(
            DOMAIN_SORA,
            DOMAIN_ETH,
            networkId,
            verifierAddress,
            bridgeAddress,
            verifierCodeHash,
            verifierKeyHash);
    if (!ETH_MAINNET_NETWORK_ID.equals(binding.networkId)) {
      throw new IllegalArgumentException(
          "Ethereum mainnet destinationBinding.networkId must be chain id 1");
    }
    return binding;
  }

  /** Canonical governed Ethereum mainnet destination binding hash. */
  public static String ethereumMainnetDestinationBindingHash(
      final String verifierAddress,
      final String bridgeAddress,
      final String verifierCodeHash,
      final String verifierKeyHash) {
    return ethereumMainnetDestinationBinding(
            verifierAddress, bridgeAddress, verifierCodeHash, verifierKeyHash)
        .hash;
  }

  /** Governed BSC mainnet destination binding for UI-side SCCP proof generation. */
  public static EvmDestinationBinding bscMainnetDestinationBinding(
      final String verifierAddress,
      final String bridgeAddress,
      final String verifierCodeHash,
      final String verifierKeyHash) {
    return bscMainnetDestinationBinding(
        verifierAddress,
        bridgeAddress,
        verifierCodeHash,
        verifierKeyHash,
        BSC_MAINNET_NETWORK_ID);
  }

  /** Governed BSC mainnet destination binding for UI-side SCCP proof generation. */
  public static EvmDestinationBinding bscMainnetDestinationBinding(
      final String verifierAddress,
      final String bridgeAddress,
      final String verifierCodeHash,
      final String verifierKeyHash,
      final String networkId) {
    final EvmDestinationBinding binding =
        evmDestinationBinding(
            DOMAIN_SORA,
            DOMAIN_BSC,
            networkId,
            verifierAddress,
            bridgeAddress,
            verifierCodeHash,
            verifierKeyHash);
    if (!BSC_MAINNET_NETWORK_ID.equals(binding.networkId)) {
      throw new IllegalArgumentException("BSC mainnet networkId must be chain id 56");
    }
    return binding;
  }

  /** Canonical governed BSC mainnet destination binding hash. */
  public static String bscMainnetDestinationBindingHash(
      final String verifierAddress,
      final String bridgeAddress,
      final String verifierCodeHash,
      final String verifierKeyHash) {
    return bscMainnetDestinationBinding(
            verifierAddress, bridgeAddress, verifierCodeHash, verifierKeyHash)
        .hash;
  }

  /** Canonical governed BSC mainnet destination binding hash. */
  public static String bscMainnetDestinationBindingHash(
      final String verifierAddress,
      final String bridgeAddress,
      final String verifierCodeHash,
      final String verifierKeyHash,
      final String networkId) {
    return bscMainnetDestinationBinding(
            verifierAddress, bridgeAddress, verifierCodeHash, verifierKeyHash, networkId)
        .hash;
  }

  /** Governed TRON destination binding for UI-side SCCP proof generation. */
  public static TronDestinationBinding tronDestinationBinding(
      final int sourceDomain,
      final int targetDomain,
      final String networkId,
      final String verifierAddress,
      final String verifierCodeHash,
      final String verifierKeyHash) {
    final int normalizedSourceDomain = normalizeDomain(sourceDomain, "sourceDomain");
    final int normalizedTargetDomain = normalizeDomain(targetDomain, "targetDomain");
    if (normalizedSourceDomain != DOMAIN_SORA) {
      throw new IllegalArgumentException("sourceDomain must be SORA");
    }
    if (normalizedTargetDomain != DOMAIN_TRON) {
      throw new IllegalArgumentException("targetDomain must be TRON");
    }
    final byte[] networkIdBytes = nonZeroHex32Bytes(networkId, "networkId");
    final byte[] verifierAddressPayload = tronBase58CheckPayload(verifierAddress, "verifierAddress");
    final byte[] verifierCodeHashBytes =
        nonZeroHex32Bytes(verifierCodeHash, "verifierCodeHash");
    final byte[] verifierKeyHashBytes = nonZeroHex32Bytes(verifierKeyHash, "verifierKeyHash");
    final String normalizedNetworkId = "0x" + hexLower(networkIdBytes);
    final String normalizedVerifierAddress =
        Objects.requireNonNull(verifierAddress, "verifierAddress");
    final String normalizedVerifierCodeHash = "0x" + hexLower(verifierCodeHashBytes);
    final String normalizedVerifierKeyHash = "0x" + hexLower(verifierKeyHashBytes);
    final String key =
        "tron:"
            + normalizedSourceDomain
            + ":"
            + normalizedTargetDomain
            + ":"
            + hexLower(networkIdBytes)
            + ":"
            + normalizedVerifierAddress
            + ":"
            + normalizedVerifierCodeHash
            + ":"
            + normalizedVerifierKeyHash;

    final ByteArrayOutputStream payload = new ByteArrayOutputStream();
    write(payload, keccak256(TRON_DESTINATION_BINDING_LABEL_V1.getBytes(StandardCharsets.UTF_8)));
    write(payload, keccak256(TronSccpProver.GROTH16_BN254_PROOF_BACKEND_V1.getBytes(StandardCharsets.UTF_8)));
    write(payload, keccak256(STARK_FRI_PROOF_FAMILY_V1.getBytes(StandardCharsets.UTF_8)));
    write(payload, networkIdBytes);
    write(payload, abiWordU32(normalizedSourceDomain, "sourceDomain"));
    write(payload, abiWordU32(normalizedTargetDomain, "targetDomain"));
    write(payload, abiWordBytes21(verifierAddressPayload, "verifierAddress"));
    write(payload, verifierCodeHashBytes);
    write(payload, verifierKeyHashBytes);
    final String hash = "0x" + hexLower(keccak256(payload.toByteArray()));
    return new TronDestinationBinding(
        1,
        normalizedSourceDomain,
        normalizedTargetDomain,
        normalizedNetworkId,
        normalizedVerifierAddress,
        normalizedVerifierCodeHash,
        normalizedVerifierKeyHash,
        TronSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
        STARK_FRI_PROOF_FAMILY_V1,
        key,
        hash);
  }

  /** Canonical governed TRON destination binding hash for UI-side proof requests. */
  public static String tronDestinationBindingHash(
      final int sourceDomain,
      final int targetDomain,
      final String networkId,
      final String verifierAddress,
      final String verifierCodeHash,
      final String verifierKeyHash) {
    return tronDestinationBinding(
            sourceDomain, targetDomain, networkId, verifierAddress, verifierCodeHash, verifierKeyHash)
        .hash;
  }

  /** Canonical governed source-verifier material record bytes. */
  public static byte[] canonicalSourceVerifierMaterialBytes(
      final int sourceDomain,
      final String sourceTrustAnchorHash,
      final String consensusVerifierHash,
      final String messageInclusionVerifierHash,
      final String finalityPolicyHash,
      final String sourceStateVerifierHash,
      final String bridgeAddress,
      final String sourceBridgeEmitterCodeHash,
      final String networkId,
      final String ownerAddress,
      final String configHash) {
    final NormalizedSourceMaterial material =
        normalizeSourceMaterial(
            sourceDomain,
            sourceTrustAnchorHash,
            consensusVerifierHash,
            messageInclusionVerifierHash,
            finalityPolicyHash,
            sourceStateVerifierHash,
            bridgeAddress,
            sourceBridgeEmitterCodeHash,
            networkId,
            ownerAddress,
            configHash);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeSourceMaterialFields(out, material);
    out.write(0);
    return out.toByteArray();
  }

  /** Canonical governed source-verifier material record hash. */
  public static String sourceVerifierMaterialHash(
      final int sourceDomain,
      final String sourceTrustAnchorHash,
      final String consensusVerifierHash,
      final String messageInclusionVerifierHash,
      final String finalityPolicyHash,
      final String sourceStateVerifierHash,
      final String bridgeAddress,
      final String sourceBridgeEmitterCodeHash,
      final String networkId,
      final String ownerAddress,
      final String configHash) {
    return hashHex(
        "sccp:source-verifier-material-record:v1",
        canonicalSourceVerifierMaterialBytes(
            sourceDomain,
            sourceTrustAnchorHash,
            consensusVerifierHash,
            messageInclusionVerifierHash,
            finalityPolicyHash,
            sourceStateVerifierHash,
            bridgeAddress,
            sourceBridgeEmitterCodeHash,
            networkId,
            ownerAddress,
            configHash));
  }

  /** Canonical governed source-adapter deployment record bytes. */
  public static byte[] canonicalSourceAdapterEngineDeploymentBytes(
      final int sourceDomain,
      final String sourceTrustAnchorHash,
      final String consensusVerifierHash,
      final String messageInclusionVerifierHash,
      final String finalityPolicyHash,
      final String deploymentReceiptHash,
      final int targetDomain,
      final String adapterVerifierVkHash,
      final String sourceStateVerifierHash,
      final String bridgeAddress,
      final String sourceBridgeEmitterCodeHash,
      final String networkId,
      final String ownerAddress,
      final String configHash) {
    return canonicalSourceAdapterEngineDeploymentBytes(
        sourceDomain,
        sourceTrustAnchorHash,
        consensusVerifierHash,
        messageInclusionVerifierHash,
        finalityPolicyHash,
        deploymentReceiptHash,
        targetDomain,
        adapterVerifierVkHash,
        sourceStateVerifierHash,
        bridgeAddress,
        sourceBridgeEmitterCodeHash,
        networkId,
        ownerAddress,
        configHash,
        null,
        null,
        null);
  }

  /** Canonical governed source-adapter deployment record bytes with Solana audit material. */
  public static byte[] canonicalSourceAdapterEngineDeploymentBytes(
      final int sourceDomain,
      final String sourceTrustAnchorHash,
      final String consensusVerifierHash,
      final String messageInclusionVerifierHash,
      final String finalityPolicyHash,
      final String deploymentReceiptHash,
      final int targetDomain,
      final String adapterVerifierVkHash,
      final String sourceStateVerifierHash,
      final String bridgeAddress,
      final String sourceBridgeEmitterCodeHash,
      final String networkId,
      final String ownerAddress,
      final String configHash,
      final String solanaTowerReplayVerifierHash,
      final String solanaFullAccountsdbLatticeVerifierHash,
      final String solanaBankForkChoiceVerifierHash) {
    return canonicalSourceAdapterEngineDeploymentBytes(
        sourceDomain,
        sourceTrustAnchorHash,
        consensusVerifierHash,
        messageInclusionVerifierHash,
        finalityPolicyHash,
        deploymentReceiptHash,
        targetDomain,
        adapterVerifierVkHash,
        sourceStateVerifierHash,
        bridgeAddress,
        sourceBridgeEmitterCodeHash,
        networkId,
        ownerAddress,
        configHash,
        solanaTowerReplayVerifierHash,
        solanaFullAccountsdbLatticeVerifierHash,
        solanaBankForkChoiceVerifierHash,
        null,
        null,
        null);
  }

  /** Canonical governed source-adapter deployment record bytes with source audit material. */
  public static byte[] canonicalSourceAdapterEngineDeploymentBytes(
      final int sourceDomain,
      final String sourceTrustAnchorHash,
      final String consensusVerifierHash,
      final String messageInclusionVerifierHash,
      final String finalityPolicyHash,
      final String deploymentReceiptHash,
      final int targetDomain,
      final String adapterVerifierVkHash,
      final String sourceStateVerifierHash,
      final String bridgeAddress,
      final String sourceBridgeEmitterCodeHash,
      final String networkId,
      final String ownerAddress,
      final String configHash,
      final String solanaTowerReplayVerifierHash,
      final String solanaFullAccountsdbLatticeVerifierHash,
      final String solanaBankForkChoiceVerifierHash,
      final String tonMasterchainConfigVerifierHash,
      final String tonValidatorSetTransitionVerifierHash,
      final String tonShardAccountsDictionaryVerifierHash) {
    final int normalizedTargetDomain = normalizeDomain(targetDomain, "targetDomain");
    if (normalizedTargetDomain != DOMAIN_SORA) {
      throw new IllegalArgumentException(
          "targetDomain must be SORA for SCCP source-adapter deployments");
    }
    final NormalizedSourceMaterial material =
        normalizeSourceMaterial(
            sourceDomain,
            sourceTrustAnchorHash,
            consensusVerifierHash,
            messageInclusionVerifierHash,
            finalityPolicyHash,
            sourceStateVerifierHash,
            bridgeAddress,
            sourceBridgeEmitterCodeHash,
            networkId,
            ownerAddress,
            configHash);
    final String canonicalVkHash =
        sourceAdapterVerifierVkHash(material.sourceDomain, normalizedTargetDomain);
    final String normalizedVkHash =
        adapterVerifierVkHash == null ? canonicalVkHash : normalizeHex32(adapterVerifierVkHash);
    if (!canonicalVkHash.equals(normalizedVkHash)) {
      throw new IllegalArgumentException(
          "adapterVerifierVkHash must match the canonical source-adapter verifier profile");
    }
    final byte[] adapterVerifierVkHashBytes = hex32Bytes(normalizedVkHash, "adapterVerifierVkHash");
    final byte[] deploymentReceiptHashBytes =
        nonZeroHex32Bytes(deploymentReceiptHash, "deploymentReceiptHash");
    requirePairwiseNonzeroRoleHashSeparation(
        "SCCP source-adapter deployment",
        new String[] {
          "sourceTrustAnchorHash",
          "consensusVerifierHash",
          "messageInclusionVerifierHash",
          "finalityPolicyHash",
          "sourceStateVerifierHash",
          "adapterVerifierVkHash",
          "sourceBridgeEmitterCodeHash",
          "sourceBridgeNetworkId",
          "sourceBridgeConfigHash",
          "deploymentReceiptHash"
        },
        new byte[][] {
          material.sourceTrustAnchorHash,
          material.consensusVerifierHash,
          material.messageInclusionVerifierHash,
          material.finalityPolicyHash,
          material.sourceStateVerifierHash,
          adapterVerifierVkHashBytes,
          material.sourceBridgeEmitterCodeHash,
          material.sourceBridgeNetworkId,
          material.sourceBridgeConfigHash,
          deploymentReceiptHashBytes
        });
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, material.sourceDomain);
    writeU32Le(out, normalizedTargetDomain);
    writeVector(out, material.profile.chain.getBytes(StandardCharsets.UTF_8));
    out.write(material.profile.proofPlan);
    out.write(material.profile.finalityModel);
    writeVector(out, STARK_FRI_PROOF_FAMILY_V1.getBytes(StandardCharsets.UTF_8));
    writeVector(out, SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1.getBytes(StandardCharsets.UTF_8));
    write(out, adapterVerifierVkHashBytes);
    writeSourceComponentFields(out, material);
    write(out, deploymentReceiptHashBytes);
    writeSourceAdapterDeploymentSolanaAuditFields(
        out,
        material.sourceDomain,
        solanaTowerReplayVerifierHash,
        solanaFullAccountsdbLatticeVerifierHash,
        solanaBankForkChoiceVerifierHash,
        new byte[][] {
          material.sourceTrustAnchorHash,
          material.consensusVerifierHash,
          material.messageInclusionVerifierHash,
          material.finalityPolicyHash,
          material.sourceStateVerifierHash,
          adapterVerifierVkHashBytes,
          material.sourceBridgeEmitterCodeHash,
          material.sourceBridgeNetworkId,
          material.sourceBridgeConfigHash,
          deploymentReceiptHashBytes
        });
    writeSourceAdapterDeploymentTonAuditFields(
        out,
        material.sourceDomain,
        tonMasterchainConfigVerifierHash,
        tonValidatorSetTransitionVerifierHash,
        tonShardAccountsDictionaryVerifierHash,
        new byte[][] {
          material.sourceTrustAnchorHash,
          material.consensusVerifierHash,
          material.messageInclusionVerifierHash,
          material.finalityPolicyHash,
          material.sourceStateVerifierHash,
          adapterVerifierVkHashBytes,
          material.sourceBridgeEmitterCodeHash,
          material.sourceBridgeNetworkId,
          material.sourceBridgeConfigHash,
          deploymentReceiptHashBytes
        });
    return out.toByteArray();
  }

  /** Canonical governed source-adapter deployment record hash. */
  public static String sourceAdapterEngineDeploymentHash(
      final int sourceDomain,
      final String sourceTrustAnchorHash,
      final String consensusVerifierHash,
      final String messageInclusionVerifierHash,
      final String finalityPolicyHash,
      final String deploymentReceiptHash,
      final int targetDomain,
      final String adapterVerifierVkHash,
      final String sourceStateVerifierHash,
      final String bridgeAddress,
      final String sourceBridgeEmitterCodeHash,
      final String networkId,
      final String ownerAddress,
      final String configHash) {
    return sourceAdapterEngineDeploymentHash(
        sourceDomain,
        sourceTrustAnchorHash,
        consensusVerifierHash,
        messageInclusionVerifierHash,
        finalityPolicyHash,
        deploymentReceiptHash,
        targetDomain,
        adapterVerifierVkHash,
        sourceStateVerifierHash,
        bridgeAddress,
        sourceBridgeEmitterCodeHash,
        networkId,
        ownerAddress,
        configHash,
        null,
        null,
        null);
  }

  /** Canonical governed source-adapter deployment record hash with Solana audit material. */
  public static String sourceAdapterEngineDeploymentHash(
      final int sourceDomain,
      final String sourceTrustAnchorHash,
      final String consensusVerifierHash,
      final String messageInclusionVerifierHash,
      final String finalityPolicyHash,
      final String deploymentReceiptHash,
      final int targetDomain,
      final String adapterVerifierVkHash,
      final String sourceStateVerifierHash,
      final String bridgeAddress,
      final String sourceBridgeEmitterCodeHash,
      final String networkId,
      final String ownerAddress,
      final String configHash,
      final String solanaTowerReplayVerifierHash,
      final String solanaFullAccountsdbLatticeVerifierHash,
      final String solanaBankForkChoiceVerifierHash) {
    return hashHex(
        "sccp:source-adapter-engine-deployment:v1",
        canonicalSourceAdapterEngineDeploymentBytes(
            sourceDomain,
            sourceTrustAnchorHash,
            consensusVerifierHash,
            messageInclusionVerifierHash,
            finalityPolicyHash,
            deploymentReceiptHash,
            targetDomain,
            adapterVerifierVkHash,
            sourceStateVerifierHash,
            bridgeAddress,
            sourceBridgeEmitterCodeHash,
            networkId,
            ownerAddress,
            configHash,
            solanaTowerReplayVerifierHash,
            solanaFullAccountsdbLatticeVerifierHash,
            solanaBankForkChoiceVerifierHash));
  }

  /** Canonical governed source-adapter deployment record hash with source audit material. */
  public static String sourceAdapterEngineDeploymentHash(
      final int sourceDomain,
      final String sourceTrustAnchorHash,
      final String consensusVerifierHash,
      final String messageInclusionVerifierHash,
      final String finalityPolicyHash,
      final String deploymentReceiptHash,
      final int targetDomain,
      final String adapterVerifierVkHash,
      final String sourceStateVerifierHash,
      final String bridgeAddress,
      final String sourceBridgeEmitterCodeHash,
      final String networkId,
      final String ownerAddress,
      final String configHash,
      final String solanaTowerReplayVerifierHash,
      final String solanaFullAccountsdbLatticeVerifierHash,
      final String solanaBankForkChoiceVerifierHash,
      final String tonMasterchainConfigVerifierHash,
      final String tonValidatorSetTransitionVerifierHash,
      final String tonShardAccountsDictionaryVerifierHash) {
    return hashHex(
        "sccp:source-adapter-engine-deployment:v1",
        canonicalSourceAdapterEngineDeploymentBytes(
            sourceDomain,
            sourceTrustAnchorHash,
            consensusVerifierHash,
            messageInclusionVerifierHash,
            finalityPolicyHash,
            deploymentReceiptHash,
            targetDomain,
            adapterVerifierVkHash,
            sourceStateVerifierHash,
            bridgeAddress,
            sourceBridgeEmitterCodeHash,
            networkId,
            ownerAddress,
            configHash,
            solanaTowerReplayVerifierHash,
            solanaFullAccountsdbLatticeVerifierHash,
            solanaBankForkChoiceVerifierHash,
            tonMasterchainConfigVerifierHash,
            tonValidatorSetTransitionVerifierHash,
            tonShardAccountsDictionaryVerifierHash));
  }

  /** Canonical Solana full light-client deployment-gate hash for audited verifier bundles. */
  public static String solanaFullLightClientGateHash(
      final int sourceDomain,
      final String sourceTrustAnchorHash,
      final String consensusVerifierHash,
      final String messageInclusionVerifierHash,
      final String finalityPolicyHash,
      final String deploymentReceiptHash,
      final String solanaTowerReplayVerifierHash,
      final String solanaFullAccountsdbLatticeVerifierHash,
      final String solanaBankForkChoiceVerifierHash,
      final int targetDomain,
      final String adapterVerifierVkHash,
      final String sourceStateVerifierHash,
      final String bridgeAddress,
      final String sourceBridgeEmitterCodeHash,
      final String networkId,
      final String ownerAddress,
      final String configHash) {
    final int normalizedSourceDomain = normalizeDomain(sourceDomain, "sourceDomain");
    final int normalizedTargetDomain = normalizeDomain(targetDomain, "targetDomain");
    if (normalizedSourceDomain != DOMAIN_SOL || normalizedTargetDomain != DOMAIN_SORA) {
      throw new IllegalArgumentException(
          "Solana full light-client gate hash requires an audited Solana -> SORA deployment");
    }
    final NormalizedSourceMaterial material =
        normalizeSourceMaterial(
            normalizedSourceDomain,
            sourceTrustAnchorHash,
            consensusVerifierHash,
            messageInclusionVerifierHash,
            finalityPolicyHash,
            sourceStateVerifierHash,
            bridgeAddress,
            sourceBridgeEmitterCodeHash,
            networkId,
            ownerAddress,
            configHash);
    final String[] verifierIds = {
      SOLANA_MAINNET_TOWER_REPLAY_VERIFIER_ID_V1,
      SOLANA_MAINNET_FULL_ACCOUNTSDB_LATTICE_VERIFIER_ID_V1,
      SOLANA_MAINNET_BANK_FORK_CHOICE_VERIFIER_ID_V1
    };
    final byte[][] verifierHashes = {
      nonZeroHex32Bytes(solanaTowerReplayVerifierHash, "solanaTowerReplayVerifierHash"),
      nonZeroHex32Bytes(
          solanaFullAccountsdbLatticeVerifierHash, "solanaFullAccountsdbLatticeVerifierHash"),
      nonZeroHex32Bytes(solanaBankForkChoiceVerifierHash, "solanaBankForkChoiceVerifierHash")
    };
    final String materialHash =
        sourceVerifierMaterialHash(
            normalizedSourceDomain,
            sourceTrustAnchorHash,
            consensusVerifierHash,
            messageInclusionVerifierHash,
            finalityPolicyHash,
            sourceStateVerifierHash,
            bridgeAddress,
            sourceBridgeEmitterCodeHash,
            networkId,
            ownerAddress,
            configHash);
    final String deploymentHash =
        sourceAdapterEngineDeploymentHash(
            normalizedSourceDomain,
            sourceTrustAnchorHash,
            consensusVerifierHash,
            messageInclusionVerifierHash,
            finalityPolicyHash,
            deploymentReceiptHash,
            normalizedTargetDomain,
            adapterVerifierVkHash,
            sourceStateVerifierHash,
            bridgeAddress,
            sourceBridgeEmitterCodeHash,
            networkId,
            ownerAddress,
            configHash,
            solanaTowerReplayVerifierHash,
            solanaFullAccountsdbLatticeVerifierHash,
            solanaBankForkChoiceVerifierHash);
    final byte[] adapterVerifierVkHashBytes =
        hex32Bytes(
            adapterVerifierVkHash == null
                ? sourceAdapterVerifierVkHash(normalizedSourceDomain, normalizedTargetDomain)
                : normalizeHex32(adapterVerifierVkHash),
            "adapterVerifierVkHash");
    final byte[] deploymentReceiptHashBytes =
        nonZeroHex32Bytes(deploymentReceiptHash, "deploymentReceiptHash");
    requireSolanaFullLightClientAuditRoleSeparation(
        verifierIds,
        verifierHashes,
        new byte[][] {
          material.sourceTrustAnchorHash,
          material.consensusVerifierHash,
          material.messageInclusionVerifierHash,
          material.finalityPolicyHash,
          material.sourceStateVerifierHash,
          adapterVerifierVkHashBytes,
          material.sourceBridgeEmitterCodeHash,
          material.sourceBridgeNetworkId,
          material.sourceBridgeConfigHash,
          deploymentReceiptHashBytes
        });

    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, material.sourceDomain);
    writeU32Le(out, normalizedTargetDomain);
    writeVector(out, material.profile.chain.getBytes(StandardCharsets.UTF_8));
    out.write(material.profile.proofPlan);
    out.write(material.profile.finalityModel);
    writeVector(out, SOLANA_MAINNET_GENESIS_HASH.getBytes(StandardCharsets.UTF_8));
    write(out, hex32Bytes(materialHash, "sourceVerifierMaterialHash"));
    write(out, hex32Bytes(deploymentHash, "sourceAdapterDeploymentHash"));
    for (int i = 0; i < verifierHashes.length; i++) {
      writeVector(out, verifierIds[i].getBytes(StandardCharsets.UTF_8));
      write(out, verifierHashes[i]);
    }
    return hashHex("sccp:solana:full-light-client-gate:v1", out.toByteArray());
  }

  /** Canonical TON full light-client deployment-gate hash for audited verifier bundles. */
  public static String tonFullLightClientGateHash(
      final int sourceDomain,
      final String sourceTrustAnchorHash,
      final String consensusVerifierHash,
      final String messageInclusionVerifierHash,
      final String finalityPolicyHash,
      final String deploymentReceiptHash,
      final String tonMasterchainConfigVerifierHash,
      final String tonValidatorSetTransitionVerifierHash,
      final String tonShardAccountsDictionaryVerifierHash,
      final int targetDomain,
      final String adapterVerifierVkHash,
      final String sourceStateVerifierHash,
      final String bridgeAddress,
      final String sourceBridgeEmitterCodeHash,
      final String networkId,
      final String ownerAddress,
      final String configHash) {
    final int normalizedSourceDomain = normalizeDomain(sourceDomain, "sourceDomain");
    final int normalizedTargetDomain = normalizeDomain(targetDomain, "targetDomain");
    if (normalizedSourceDomain != DOMAIN_TON || normalizedTargetDomain != DOMAIN_SORA) {
      throw new IllegalArgumentException(
          "TON full light-client gate hash requires an audited TON -> SORA deployment");
    }
    final NormalizedSourceMaterial material =
        normalizeSourceMaterial(
            normalizedSourceDomain,
            sourceTrustAnchorHash,
            consensusVerifierHash,
            messageInclusionVerifierHash,
            finalityPolicyHash,
            sourceStateVerifierHash,
            bridgeAddress,
            sourceBridgeEmitterCodeHash,
            networkId,
            ownerAddress,
            configHash);
    final String[] verifierIds = {
      TON_MAINNET_MASTERCHAIN_CONFIG_VERIFIER_ID_V1,
      TON_MAINNET_VALIDATOR_SET_TRANSITION_VERIFIER_ID_V1,
      TON_MAINNET_SHARD_ACCOUNTS_DICTIONARY_VERIFIER_ID_V1
    };
    final byte[][] verifierHashes = {
      nonZeroHex32Bytes(tonMasterchainConfigVerifierHash, "tonMasterchainConfigVerifierHash"),
      nonZeroHex32Bytes(
          tonValidatorSetTransitionVerifierHash, "tonValidatorSetTransitionVerifierHash"),
      nonZeroHex32Bytes(
          tonShardAccountsDictionaryVerifierHash, "tonShardAccountsDictionaryVerifierHash")
    };
    final String materialHash =
        sourceVerifierMaterialHash(
            normalizedSourceDomain,
            sourceTrustAnchorHash,
            consensusVerifierHash,
            messageInclusionVerifierHash,
            finalityPolicyHash,
            sourceStateVerifierHash,
            bridgeAddress,
            sourceBridgeEmitterCodeHash,
            networkId,
            ownerAddress,
            configHash);
    final String deploymentHash =
        sourceAdapterEngineDeploymentHash(
            normalizedSourceDomain,
            sourceTrustAnchorHash,
            consensusVerifierHash,
            messageInclusionVerifierHash,
            finalityPolicyHash,
            deploymentReceiptHash,
            normalizedTargetDomain,
            adapterVerifierVkHash,
            sourceStateVerifierHash,
            bridgeAddress,
            sourceBridgeEmitterCodeHash,
            networkId,
            ownerAddress,
            configHash,
            null,
            null,
            null,
            tonMasterchainConfigVerifierHash,
            tonValidatorSetTransitionVerifierHash,
            tonShardAccountsDictionaryVerifierHash);

    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, material.sourceDomain);
    writeU32Le(out, normalizedTargetDomain);
    writeVector(out, material.profile.chain.getBytes(StandardCharsets.UTF_8));
    out.write(material.profile.proofPlan);
    out.write(material.profile.finalityModel);
    writeI32Le(out, -239);
    writeVector(out, TonSccpProver.SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1.getBytes(StandardCharsets.UTF_8));
    writeVector(out, SOURCE_ADAPTER_FASTPQ_PARAMETER_SET_V1.getBytes(StandardCharsets.UTF_8));
    writeVector(out, material.profile.sourceStateVerifierId.getBytes(StandardCharsets.UTF_8));
    write(out, material.sourceStateVerifierHash);
    write(out, hex32Bytes(materialHash, "sourceVerifierMaterialHash"));
    write(out, hex32Bytes(deploymentHash, "sourceAdapterDeploymentHash"));
    for (int i = 0; i < verifierHashes.length; i++) {
      writeVector(out, verifierIds[i].getBytes(StandardCharsets.UTF_8));
      write(out, verifierHashes[i]);
    }
    return hashHex("sccp:ton:full-light-client-gate:v1", out.toByteArray());
  }

  public static byte[] canonicalEvmReceiptProofBytes(
      final String sourceEventDigest,
      final String beaconSlot,
      final String executionBlockNumber,
      final String executionBlockHash,
      final String executionReceiptsRoot,
      final String beaconFinalizedRoot,
      final String syncCommitteeRoot,
      final String receiptRootIndex,
      final List<byte[]> receiptTrieProofNodes,
      final List<byte[]> inclusionBranch) {
    return canonicalEvmReceiptProofBytes(
        sourceEventDigest,
        beaconSlot,
        executionBlockNumber,
        executionBlockHash,
        executionReceiptsRoot,
        beaconFinalizedRoot,
        syncCommitteeRoot,
        receiptRootIndex,
        receiptTrieProofNodes,
        inclusionBranch,
        DOMAIN_ETH);
  }

  public static byte[] canonicalEvmReceiptProofBytes(
      final String sourceEventDigest,
      final String beaconSlot,
      final String executionBlockNumber,
      final String executionBlockHash,
      final String executionReceiptsRoot,
      final String beaconFinalizedRoot,
      final String syncCommitteeRoot,
      final String receiptRootIndex,
      final List<byte[]> receiptTrieProofNodes,
      final List<byte[]> inclusionBranch,
      final int sourceDomain) {
    validateTronMptProofNodes(receiptTrieProofNodes);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, normalizeDomain(sourceDomain, "sourceDomain"));
    write(out, nonZeroHex32Bytes(sourceEventDigest, "sourceEventDigest"));
    writeU64Le(out, normalizeU64(beaconSlot, "beaconSlot"));
    writeU64Le(out, normalizeU64(executionBlockNumber, "executionBlockNumber"));
    write(out, hex32Bytes(executionBlockHash, "executionBlockHash"));
    write(out, hex32Bytes(executionReceiptsRoot, "executionReceiptsRoot"));
    write(out, hex32Bytes(beaconFinalizedRoot, "beaconFinalizedRoot"));
    write(out, hex32Bytes(syncCommitteeRoot, "syncCommitteeRoot"));
    writeU64Le(out, normalizeU64(receiptRootIndex, "receiptRootIndex"));
    writeU32Le(out, receiptTrieProofNodes.size());
    for (final byte[] node : receiptTrieProofNodes) {
      writeVector(out, node);
    }
    writeBranch(out, inclusionBranch);
    return out.toByteArray();
  }

  public static String evmReceiptProofHash(
      final String sourceEventDigest,
      final String beaconSlot,
      final String executionBlockNumber,
      final String executionBlockHash,
      final String executionReceiptsRoot,
      final String beaconFinalizedRoot,
      final String syncCommitteeRoot,
      final String receiptRootIndex,
      final List<byte[]> receiptTrieProofNodes,
      final List<byte[]> inclusionBranch) {
    return evmReceiptProofHash(
        sourceEventDigest,
        beaconSlot,
        executionBlockNumber,
        executionBlockHash,
        executionReceiptsRoot,
        beaconFinalizedRoot,
        syncCommitteeRoot,
        receiptRootIndex,
        receiptTrieProofNodes,
        inclusionBranch,
        DOMAIN_ETH);
  }

  public static String evmReceiptProofHash(
      final String sourceEventDigest,
      final String beaconSlot,
      final String executionBlockNumber,
      final String executionBlockHash,
      final String executionReceiptsRoot,
      final String beaconFinalizedRoot,
      final String syncCommitteeRoot,
      final String receiptRootIndex,
      final List<byte[]> receiptTrieProofNodes,
      final List<byte[]> inclusionBranch,
      final int sourceDomain) {
    return hashHex(
        "sccp:evm:receipt-proof:v1",
        canonicalEvmReceiptProofBytes(
            sourceEventDigest,
            beaconSlot,
            executionBlockNumber,
            executionBlockHash,
            executionReceiptsRoot,
            beaconFinalizedRoot,
            syncCommitteeRoot,
            receiptRootIndex,
            receiptTrieProofNodes,
            inclusionBranch,
            sourceDomain));
  }

  public static byte[] canonicalEthSyncCommitteePayloadBytes(
      final List<byte[]> syncCommitteePublicKeys,
      final List<String> syncCommitteeWeights,
      final List<byte[]> syncCommitteePops) {
    Objects.requireNonNull(syncCommitteePublicKeys, "syncCommitteePublicKeys");
    Objects.requireNonNull(syncCommitteeWeights, "syncCommitteeWeights");
    Objects.requireNonNull(syncCommitteePops, "syncCommitteePops");
    if (syncCommitteePublicKeys.isEmpty()
        || syncCommitteePublicKeys.size() != syncCommitteeWeights.size()
        || syncCommitteePublicKeys.size() != syncCommitteePops.size()) {
      throw new IllegalArgumentException(
          "syncCommitteePublicKeys, syncCommitteeWeights, and syncCommitteePops must be non-empty equal-length arrays");
    }
    if (syncCommitteePublicKeys.size() > ETH_MAX_SYNC_COMMITTEE_AUTHORITIES) {
      throw new IllegalArgumentException(
          "syncCommitteePublicKeys must contain at most "
              + ETH_MAX_SYNC_COMMITTEE_AUTHORITIES
              + " entries");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, syncCommitteePublicKeys.size());
    final Set<String> seenPublicKeys = new HashSet<>();
    for (int i = 0; i < syncCommitteePublicKeys.size(); i++) {
      final byte[] publicKey =
          Objects.requireNonNull(syncCommitteePublicKeys.get(i), "syncCommitteePublicKeys[" + i + "]");
      if (publicKey.length == 0) {
        throw new IllegalArgumentException("syncCommitteePublicKeys[" + i + "] must not be empty");
      }
      if (publicKey.length != ETH_SYNC_COMMITTEE_PUBLIC_KEY_BYTES) {
        throw new IllegalArgumentException(
            "syncCommitteePublicKeys[" + i + "] must be "
                + ETH_SYNC_COMMITTEE_PUBLIC_KEY_BYTES
                + " bytes");
      }
      if (isZero(publicKey)) {
        throw new IllegalArgumentException("syncCommitteePublicKeys[" + i + "] must not be zero");
      }
      if (!seenPublicKeys.add(hexLower(publicKey))) {
        throw new IllegalArgumentException("syncCommitteePublicKeys[" + i + "] must be unique");
      }
      final BigInteger weight =
          normalizeU64(syncCommitteeWeights.get(i), "syncCommitteeWeights[" + i + "]");
      if (BigInteger.ZERO.equals(weight)) {
        throw new IllegalArgumentException("syncCommitteeWeights[" + i + "] must not be zero");
      }
      final byte[] pop =
          Objects.requireNonNull(syncCommitteePops.get(i), "syncCommitteePops[" + i + "]");
      if (pop.length == 0) {
        throw new IllegalArgumentException("syncCommitteePops[" + i + "] must not be empty");
      }
      if (pop.length != ETH_SYNC_COMMITTEE_POP_BYTES) {
        throw new IllegalArgumentException(
            "syncCommitteePops[" + i + "] must be "
                + ETH_SYNC_COMMITTEE_POP_BYTES
                + " bytes");
      }
      if (isZero(pop)) {
        throw new IllegalArgumentException("syncCommitteePops[" + i + "] must not be zero");
      }
      writeVector(out, publicKey);
      writeU64Le(out, weight);
      writeVector(out, pop);
    }
    return out.toByteArray();
  }

  public static String ethSyncCommitteeHashFromPayload(final byte[] payload) {
    validateEthSyncCommitteePayload(payload);
    return hashHex("sccp:eth:sync-committee:v1", payload);
  }

  public static String ethSyncCommitteeHash(
      final List<byte[]> syncCommitteePublicKeys,
      final List<String> syncCommitteeWeights,
      final List<byte[]> syncCommitteePops) {
    return ethSyncCommitteeHashFromPayload(
        canonicalEthSyncCommitteePayloadBytes(
            syncCommitteePublicKeys, syncCommitteeWeights, syncCommitteePops));
  }

  public static String ethSyncCommitteePayloadHash(final byte[] payload) {
    validateEthSyncCommitteePayload(payload);
    return hashHex("sccp:eth:sync-committee-payload:v1", payload);
  }

  public static String ethSyncCommitteePayloadHash(
      final List<byte[]> syncCommitteePublicKeys,
      final List<String> syncCommitteeWeights,
      final List<byte[]> syncCommitteePops) {
    return ethSyncCommitteePayloadHash(
        canonicalEthSyncCommitteePayloadBytes(
            syncCommitteePublicKeys, syncCommitteeWeights, syncCommitteePops));
  }

  public static byte[] canonicalEthSyncCommitteeTransitionMessageBytes(
      final String fromSyncPeriod,
      final String toSyncPeriod,
      final String transitionSlot,
      final String finalizedBeaconRoot,
      final String parentSyncCommitteeHash,
      final String nextSyncCommitteeHash,
      final String nextSyncCommitteePayloadHash,
      final String nextSyncCommitteeBranchHash) {
    return canonicalEthSyncCommitteeTransitionMessageBytes(
        fromSyncPeriod,
        toSyncPeriod,
        transitionSlot,
        finalizedBeaconRoot,
        parentSyncCommitteeHash,
        nextSyncCommitteeHash,
        nextSyncCommitteePayloadHash,
        nextSyncCommitteeBranchHash,
        DOMAIN_ETH);
  }

  public static byte[] canonicalEthSyncCommitteeTransitionMessageBytes(
      final String fromSyncPeriod,
      final String toSyncPeriod,
      final String transitionSlot,
      final String finalizedBeaconRoot,
      final String parentSyncCommitteeHash,
      final String nextSyncCommitteeHash,
      final String nextSyncCommitteePayloadHash,
      final String nextSyncCommitteeBranchHash,
      final int sourceDomain) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, normalizeDomain(sourceDomain, "sourceDomain"));
    writeU64Le(out, normalizeU64(fromSyncPeriod, "fromSyncPeriod"));
    writeU64Le(out, normalizeU64(toSyncPeriod, "toSyncPeriod"));
    writeU64Le(out, normalizeU64(transitionSlot, "transitionSlot"));
    write(out, hex32Bytes(finalizedBeaconRoot, "finalizedBeaconRoot"));
    write(out, hex32Bytes(parentSyncCommitteeHash, "parentSyncCommitteeHash"));
    write(out, hex32Bytes(nextSyncCommitteeHash, "nextSyncCommitteeHash"));
    write(out, hex32Bytes(nextSyncCommitteePayloadHash, "nextSyncCommitteePayloadHash"));
    write(out, hex32Bytes(nextSyncCommitteeBranchHash, "nextSyncCommitteeBranchHash"));
    return out.toByteArray();
  }

  public static String ethSyncCommitteeTransitionMessageHash(
      final String fromSyncPeriod,
      final String toSyncPeriod,
      final String transitionSlot,
      final String finalizedBeaconRoot,
      final String parentSyncCommitteeHash,
      final String nextSyncCommitteeHash,
      final String nextSyncCommitteePayloadHash,
      final String nextSyncCommitteeBranchHash) {
    return ethSyncCommitteeTransitionMessageHash(
        fromSyncPeriod,
        toSyncPeriod,
        transitionSlot,
        finalizedBeaconRoot,
        parentSyncCommitteeHash,
        nextSyncCommitteeHash,
        nextSyncCommitteePayloadHash,
        nextSyncCommitteeBranchHash,
        DOMAIN_ETH);
  }

  public static String ethSyncCommitteeTransitionMessageHash(
      final String fromSyncPeriod,
      final String toSyncPeriod,
      final String transitionSlot,
      final String finalizedBeaconRoot,
      final String parentSyncCommitteeHash,
      final String nextSyncCommitteeHash,
      final String nextSyncCommitteePayloadHash,
      final String nextSyncCommitteeBranchHash,
      final int sourceDomain) {
    return hashHex(
        "sccp:eth:sync-committee-transition-message:v1",
        canonicalEthSyncCommitteeTransitionMessageBytes(
            fromSyncPeriod,
            toSyncPeriod,
            transitionSlot,
            finalizedBeaconRoot,
            parentSyncCommitteeHash,
            nextSyncCommitteeHash,
            nextSyncCommitteePayloadHash,
            nextSyncCommitteeBranchHash,
            sourceDomain));
  }

  public static byte[] canonicalEthBeaconSyncCommitteeProofBytes(
      final String totalWeight,
      final String signedWeight,
      final String syncCommitteeMessageHash,
      final List<byte[]> syncCommitteePublicKeys,
      final List<String> syncCommitteeWeights,
      final List<byte[]> syncCommitteePops,
      final byte[] signersBitmap,
      final byte[] aggregateSignature) {
    return canonicalEthBeaconSyncCommitteeProofBytes(
        totalWeight,
        signedWeight,
        syncCommitteeMessageHash,
        syncCommitteePublicKeys,
        syncCommitteeWeights,
        syncCommitteePops,
        signersBitmap,
        aggregateSignature,
        1);
  }

  public static byte[] canonicalEthBeaconSyncCommitteeProofBytes(
      final String totalWeight,
      final String signedWeight,
      final String syncCommitteeMessageHash,
      final List<byte[]> syncCommitteePublicKeys,
      final List<String> syncCommitteeWeights,
      final List<byte[]> syncCommitteePops,
      final byte[] signersBitmap,
      final byte[] aggregateSignature,
      final int version) {
    requireV1Version(version, "syncCommitteeProof.version");
    canonicalEthSyncCommitteePayloadBytes(
        syncCommitteePublicKeys, syncCommitteeWeights, syncCommitteePops);
    final List<BigInteger> normalizedWeights = new ArrayList<>();
    for (int i = 0; i < syncCommitteeWeights.size(); i++) {
      normalizedWeights.add(
          normalizeU64(syncCommitteeWeights.get(i), "syncCommitteeWeights[" + i + "]"));
    }
    final byte[] signersBitmapBytes = Objects.requireNonNull(signersBitmap, "signersBitmap");
    if (signersBitmapBytes.length != (syncCommitteePublicKeys.size() + 7) / 8) {
      throw new IllegalArgumentException(
          "signersBitmap length must match syncCommitteePublicKeys");
    }
    final List<Integer> signerIndices =
        ethSyncCommitteeSignerIndices(signersBitmapBytes, syncCommitteePublicKeys.size());
    final byte[] aggregateSignatureBytes =
        Objects.requireNonNull(aggregateSignature, "aggregateSignature");
    if (aggregateSignatureBytes.length != ETH_SYNC_COMMITTEE_SIGNATURE_BYTES) {
      throw new IllegalArgumentException(
          "aggregateSignature must be "
              + ETH_SYNC_COMMITTEE_SIGNATURE_BYTES
              + " bytes");
    }
    if (isZero(aggregateSignatureBytes)) {
      throw new IllegalArgumentException("aggregateSignature must not be all zero");
    }
    final BigInteger totalWeightValue = normalizeU64(totalWeight, "totalWeight");
    final BigInteger signedWeightValue = normalizeU64(signedWeight, "signedWeight");
    BigInteger computedTotalWeight = BigInteger.ZERO;
    for (final BigInteger weight : normalizedWeights) {
      computedTotalWeight = computedTotalWeight.add(weight);
    }
    if (!totalWeightValue.equals(computedTotalWeight)) {
      throw new IllegalArgumentException("totalWeight must match syncCommitteeWeights");
    }
    BigInteger computedSignedWeight = BigInteger.ZERO;
    for (final int signerIndex : signerIndices) {
      computedSignedWeight = computedSignedWeight.add(normalizedWeights.get(signerIndex));
    }
    if (!signedWeightValue.equals(computedSignedWeight)) {
      throw new IllegalArgumentException("signedWeight must match signersBitmap");
    }
    if (signedWeightValue.multiply(BigInteger.valueOf(3))
        .compareTo(totalWeightValue.multiply(BigInteger.valueOf(2))) <= 0) {
      throw new IllegalArgumentException(
          "signedWeight must be greater than two thirds of totalWeight");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(version);
    writeU64Le(out, totalWeightValue);
    writeU64Le(out, signedWeightValue);
    write(out, hex32Bytes(syncCommitteeMessageHash, "syncCommitteeMessageHash"));
    writeU32Le(out, syncCommitteePublicKeys.size());
    for (final byte[] publicKey : syncCommitteePublicKeys) {
      writeVector(out, publicKey);
    }
    writeU32Le(out, syncCommitteeWeights.size());
    for (final BigInteger weight : normalizedWeights) {
      writeU64Le(out, weight);
    }
    writeU32Le(out, syncCommitteePops.size());
    for (final byte[] pop : syncCommitteePops) {
      writeVector(out, pop);
    }
    writeVector(out, signersBitmapBytes);
    writeVector(out, aggregateSignatureBytes);
    return out.toByteArray();
  }

  public static byte[] canonicalEthSyncCommitteeTransitionSignatureBytes(
      final String fromSyncPeriod,
      final String toSyncPeriod,
      final String transitionSlot,
      final String finalizedBeaconRoot,
      final String parentSyncCommitteeHash,
      final String nextSyncCommitteeHash,
      final byte[] nextSyncCommitteePayload,
      final String nextSyncCommitteePayloadHash,
      final String nextSyncCommitteeBranchHash,
      final String transitionMessageHash,
      final String totalWeight,
      final String signedWeight,
      final List<byte[]> syncCommitteePublicKeys,
      final List<String> syncCommitteeWeights,
      final List<byte[]> syncCommitteePops,
      final byte[] signersBitmap,
      final byte[] aggregateSignature) {
    return canonicalEthSyncCommitteeTransitionSignatureBytes(
        fromSyncPeriod,
        toSyncPeriod,
        transitionSlot,
        finalizedBeaconRoot,
        parentSyncCommitteeHash,
        nextSyncCommitteeHash,
        nextSyncCommitteePayload,
        nextSyncCommitteePayloadHash,
        nextSyncCommitteeBranchHash,
        transitionMessageHash,
        totalWeight,
        signedWeight,
        syncCommitteePublicKeys,
        syncCommitteeWeights,
        syncCommitteePops,
        signersBitmap,
        aggregateSignature,
        DOMAIN_ETH,
        1,
        1);
  }

  public static byte[] canonicalEthSyncCommitteeTransitionSignatureBytes(
      final String fromSyncPeriod,
      final String toSyncPeriod,
      final String transitionSlot,
      final String finalizedBeaconRoot,
      final String parentSyncCommitteeHash,
      final String nextSyncCommitteeHash,
      final byte[] nextSyncCommitteePayload,
      final String nextSyncCommitteePayloadHash,
      final String nextSyncCommitteeBranchHash,
      final String transitionMessageHash,
      final String totalWeight,
      final String signedWeight,
      final List<byte[]> syncCommitteePublicKeys,
      final List<String> syncCommitteeWeights,
      final List<byte[]> syncCommitteePops,
      final byte[] signersBitmap,
      final byte[] aggregateSignature,
      final int sourceDomain,
      final int version,
      final int proofVersion) {
    requireV1Version(version, "ETH sync-committee transition signature version");
    if (!ethSyncCommitteePayloadHash(nextSyncCommitteePayload)
        .equals(normalizeHex32(nextSyncCommitteePayloadHash))) {
      throw new IllegalArgumentException(
          "nextSyncCommitteePayloadHash must match nextSyncCommitteePayload");
    }
    if (!ethSyncCommitteeHashFromPayload(nextSyncCommitteePayload)
        .equals(normalizeHex32(nextSyncCommitteeHash))) {
      throw new IllegalArgumentException("nextSyncCommitteeHash must match nextSyncCommitteePayload");
    }
    final String parentHash =
        ethSyncCommitteeHash(syncCommitteePublicKeys, syncCommitteeWeights, syncCommitteePops);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(version);
    writeU32Le(out, normalizeDomain(sourceDomain, "sourceDomain"));
    writeU64Le(out, normalizeU64(fromSyncPeriod, "fromSyncPeriod"));
    writeU64Le(out, normalizeU64(toSyncPeriod, "toSyncPeriod"));
    writeU64Le(out, normalizeU64(transitionSlot, "transitionSlot"));
    write(out, hex32Bytes(finalizedBeaconRoot, "finalizedBeaconRoot"));
    write(out, hex32Bytes(parentSyncCommitteeHash, "parentSyncCommitteeHash"));
    write(out, hex32Bytes(nextSyncCommitteeHash, "nextSyncCommitteeHash"));
    writeVector(out, Objects.requireNonNull(nextSyncCommitteePayload, "nextSyncCommitteePayload"));
    write(out, hex32Bytes(nextSyncCommitteePayloadHash, "nextSyncCommitteePayloadHash"));
    write(out, hex32Bytes(nextSyncCommitteeBranchHash, "nextSyncCommitteeBranchHash"));
    write(out, hex32Bytes(transitionMessageHash, "transitionMessageHash"));
    write(out, hex32Bytes(parentHash, "parentSyncCommitteeHash"));
    write(
        out,
        canonicalEthBeaconSyncCommitteeProofBytes(
            totalWeight,
            signedWeight,
            transitionMessageHash,
            syncCommitteePublicKeys,
            syncCommitteeWeights,
            syncCommitteePops,
            signersBitmap,
            aggregateSignature,
            proofVersion));
    return out.toByteArray();
  }

  public static String ethSyncCommitteeTransitionSignatureHash(
      final String fromSyncPeriod,
      final String toSyncPeriod,
      final String transitionSlot,
      final String finalizedBeaconRoot,
      final String parentSyncCommitteeHash,
      final String nextSyncCommitteeHash,
      final byte[] nextSyncCommitteePayload,
      final String nextSyncCommitteePayloadHash,
      final String nextSyncCommitteeBranchHash,
      final String transitionMessageHash,
      final String totalWeight,
      final String signedWeight,
      final List<byte[]> syncCommitteePublicKeys,
      final List<String> syncCommitteeWeights,
      final List<byte[]> syncCommitteePops,
      final byte[] signersBitmap,
      final byte[] aggregateSignature) {
    return hashHex(
        "sccp:eth:sync-committee-transition-signature:v1",
        canonicalEthSyncCommitteeTransitionSignatureBytes(
            fromSyncPeriod,
            toSyncPeriod,
            transitionSlot,
            finalizedBeaconRoot,
            parentSyncCommitteeHash,
            nextSyncCommitteeHash,
            nextSyncCommitteePayload,
            nextSyncCommitteePayloadHash,
            nextSyncCommitteeBranchHash,
            transitionMessageHash,
            totalWeight,
            signedWeight,
            syncCommitteePublicKeys,
            syncCommitteeWeights,
            syncCommitteePops,
            signersBitmap,
            aggregateSignature));
  }

  public static byte[] canonicalBscReceiptProofBytes(
      final String sourceEventDigest,
      final String validatorEpoch,
      final String blockNumber,
      final String blockHash,
      final String receiptsRoot,
      final String validatorSetHash,
      final String commitSealHash,
      final String receiptRootIndex,
      final List<byte[]> receiptTrieProofNodes,
      final List<byte[]> inclusionBranch) {
    return canonicalBscReceiptProofBytes(
        sourceEventDigest,
        validatorEpoch,
        blockNumber,
        blockHash,
        receiptsRoot,
        validatorSetHash,
        commitSealHash,
        receiptRootIndex,
        receiptTrieProofNodes,
        inclusionBranch,
        DOMAIN_BSC);
  }

  public static byte[] canonicalBscReceiptProofBytes(
      final String sourceEventDigest,
      final String validatorEpoch,
      final String blockNumber,
      final String blockHash,
      final String receiptsRoot,
      final String validatorSetHash,
      final String commitSealHash,
      final String receiptRootIndex,
      final List<byte[]> receiptTrieProofNodes,
      final List<byte[]> inclusionBranch,
      final int sourceDomain) {
    validateTronMptProofNodes(receiptTrieProofNodes);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, normalizeDomain(sourceDomain, "sourceDomain"));
    write(out, nonZeroHex32Bytes(sourceEventDigest, "sourceEventDigest"));
    writeU64Le(out, normalizeU64(validatorEpoch, "validatorEpoch"));
    writeU64Le(out, normalizeU64(blockNumber, "blockNumber"));
    write(out, hex32Bytes(blockHash, "blockHash"));
    write(out, hex32Bytes(receiptsRoot, "receiptsRoot"));
    write(out, hex32Bytes(validatorSetHash, "validatorSetHash"));
    write(out, hex32Bytes(commitSealHash, "commitSealHash"));
    writeU64Le(out, normalizeU64(receiptRootIndex, "receiptRootIndex"));
    writeU32Le(out, receiptTrieProofNodes.size());
    for (final byte[] node : receiptTrieProofNodes) {
      writeVector(out, node);
    }
    writeBranch(out, inclusionBranch);
    return out.toByteArray();
  }

  public static String bscReceiptProofHash(
      final String sourceEventDigest,
      final String validatorEpoch,
      final String blockNumber,
      final String blockHash,
      final String receiptsRoot,
      final String validatorSetHash,
      final String commitSealHash,
      final String receiptRootIndex,
      final List<byte[]> receiptTrieProofNodes,
      final List<byte[]> inclusionBranch) {
    return bscReceiptProofHash(
        sourceEventDigest,
        validatorEpoch,
        blockNumber,
        blockHash,
        receiptsRoot,
        validatorSetHash,
        commitSealHash,
        receiptRootIndex,
        receiptTrieProofNodes,
        inclusionBranch,
        DOMAIN_BSC);
  }

  public static String bscReceiptProofHash(
      final String sourceEventDigest,
      final String validatorEpoch,
      final String blockNumber,
      final String blockHash,
      final String receiptsRoot,
      final String validatorSetHash,
      final String commitSealHash,
      final String receiptRootIndex,
      final List<byte[]> receiptTrieProofNodes,
      final List<byte[]> inclusionBranch,
      final int sourceDomain) {
    return hashHex(
        "sccp:bsc:receipt-proof:v1",
        canonicalBscReceiptProofBytes(
            sourceEventDigest,
            validatorEpoch,
            blockNumber,
            blockHash,
            receiptsRoot,
            validatorSetHash,
            commitSealHash,
            receiptRootIndex,
            receiptTrieProofNodes,
            inclusionBranch,
            sourceDomain));
  }

  public static byte[] canonicalBscValidatorSetPayloadBytes(
      final List<String> validatorAddresses, final List<String> validatorPowers) {
    Objects.requireNonNull(validatorAddresses, "validatorAddresses");
    Objects.requireNonNull(validatorPowers, "validatorPowers");
    if (validatorAddresses.isEmpty() || validatorAddresses.size() != validatorPowers.size()) {
      throw new IllegalArgumentException(
          "validatorAddresses and validatorPowers must be non-empty equal-length arrays");
    }
    if (validatorAddresses.size() > BSC_MAX_PARLIA_VALIDATORS) {
      throw new IllegalArgumentException(
          "validatorAddresses must contain at most " + BSC_MAX_PARLIA_VALIDATORS + " entries");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, validatorAddresses.size());
    final Set<String> seenAddresses = new HashSet<>();
    for (int i = 0; i < validatorAddresses.size(); i++) {
      final byte[] address = hexBytes(validatorAddresses.get(i), "validatorAddresses[" + i + "]", 20);
      if (isZero(address)) {
        throw new IllegalArgumentException("validatorAddresses[" + i + "] must not be zero");
      }
      final String addressHex = hexLower(address);
      if (!seenAddresses.add(addressHex)) {
        throw new IllegalArgumentException("validatorAddresses[" + i + "] must be unique");
      }
      final BigInteger power = normalizeU64(validatorPowers.get(i), "validatorPowers[" + i + "]");
      if (BigInteger.ZERO.equals(power)) {
        throw new IllegalArgumentException("validatorPowers[" + i + "] must not be zero");
      }
      write(out, address);
      writeU64Le(out, power);
    }
    return out.toByteArray();
  }

  public static String bscValidatorSetPayloadHash(final byte[] payload) {
    return keccakHashHex("sccp:bsc:validator-set-payload:v1", Objects.requireNonNull(payload, "payload"));
  }

  public static String bscValidatorSetPayloadHash(
      final List<String> validatorAddresses, final List<String> validatorPowers) {
    return bscValidatorSetPayloadHash(
        canonicalBscValidatorSetPayloadBytes(validatorAddresses, validatorPowers));
  }

  public static String bscValidatorSetHashFromPayload(final byte[] payload) {
    validateBscValidatorSetPayload(payload);
    return keccakHashHex("sccp:bsc:validator-set:v1", Objects.requireNonNull(payload, "payload"));
  }

  public static String bscValidatorSetHashFromPayload(
      final List<String> validatorAddresses, final List<String> validatorPowers) {
    return bscValidatorSetHashFromPayload(
        canonicalBscValidatorSetPayloadBytes(validatorAddresses, validatorPowers));
  }

  public static byte[] canonicalBscCommitMessageBytes(
      final String validatorEpoch,
      final String blockNumber,
      final String blockHash,
      final String receiptsRoot,
      final String validatorSetHash) {
    return canonicalBscCommitMessageBytes(
        validatorEpoch, blockNumber, blockHash, receiptsRoot, validatorSetHash, DOMAIN_BSC);
  }

  public static byte[] canonicalBscCommitMessageBytes(
      final String validatorEpoch,
      final String blockNumber,
      final String blockHash,
      final String receiptsRoot,
      final String validatorSetHash,
      final int sourceDomain) {
    if (sourceDomain != DOMAIN_BSC) {
      throw new IllegalArgumentException("sourceDomain must be BSC");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, normalizeDomain(sourceDomain, "sourceDomain"));
    writeU64Le(out, normalizeU64(validatorEpoch, "validatorEpoch"));
    writeU64Le(out, normalizeU64(blockNumber, "blockNumber"));
    write(out, hex32Bytes(blockHash, "blockHash"));
    write(out, hex32Bytes(receiptsRoot, "receiptsRoot"));
    write(out, hex32Bytes(validatorSetHash, "validatorSetHash"));
    return out.toByteArray();
  }

  public static String bscCommitMessageHash(
      final String validatorEpoch,
      final String blockNumber,
      final String blockHash,
      final String receiptsRoot,
      final String validatorSetHash) {
    return bscCommitMessageHash(
        validatorEpoch, blockNumber, blockHash, receiptsRoot, validatorSetHash, DOMAIN_BSC);
  }

  public static String bscCommitMessageHash(
      final String validatorEpoch,
      final String blockNumber,
      final String blockHash,
      final String receiptsRoot,
      final String validatorSetHash,
      final int sourceDomain) {
    return keccakHashHex(
        "sccp:bsc:commit-message:v1",
        canonicalBscCommitMessageBytes(
            validatorEpoch, blockNumber, blockHash, receiptsRoot, validatorSetHash, sourceDomain));
  }

  public static byte[] canonicalBscCommitSealBytes(final BscCommitSealProof proof) {
    Objects.requireNonNull(proof, "proof");
    requireV1Version(proof.version, "BSC commit seal version");
    final BigInteger totalPower = normalizeU64(proof.totalPower, "totalPower");
    final BigInteger signedPower = normalizeU64(proof.signedPower, "signedPower");
    final byte[] commitMessageHash = nonZeroHex32Bytes(proof.commitMessageHash, "commitMessageHash");
    if (proof.validatorPublicKeys.isEmpty()
        || proof.validatorPublicKeys.size() != proof.validatorPowers.size()
        || proof.validatorPublicKeys.size() > BSC_MAX_PARLIA_VALIDATORS) {
      throw new IllegalArgumentException(
          "validatorPublicKeys and validatorPowers must be non-empty bounded arrays");
    }

    final List<byte[]> validatorAddresses = new ArrayList<>();
    final List<BigInteger> validatorPowers = new ArrayList<>();
    final Set<String> seenAddresses = new HashSet<>();
    for (int i = 0; i < proof.validatorPublicKeys.size(); i++) {
      final byte[] publicKey = Objects.requireNonNull(proof.validatorPublicKeys.get(i), "validatorPublicKeys entry");
      final byte[] address = bscValidatorAddress20(publicKey, "validatorPublicKeys[" + i + "]");
      if (!seenAddresses.add(hexLower(address))) {
        throw new IllegalArgumentException(
            "validatorPublicKeys[" + i + "] must derive a unique address");
      }
      validatorAddresses.add(address);
      validatorPowers.add(normalizeU64(proof.validatorPowers.get(i), "validatorPowers[" + i + "]"));
    }

    final byte[] validatorSetPayload =
        canonicalBscValidatorSetPayloadBytesFromAddressPowers(validatorAddresses, validatorPowers);
    final byte[] validatorSetHash = keccakHashBytes("sccp:bsc:validator-set:v1", validatorSetPayload);
    if (proof.validatorSetHash != null) {
      if (!Arrays.equals(hex32Bytes(proof.validatorSetHash, "validatorSetHash"), validatorSetHash)) {
        throw new IllegalArgumentException(
            "validatorSetHash must match validatorPublicKeys and validatorPowers");
      }
    }
    BigInteger computedTotalPower = BigInteger.ZERO;
    for (final BigInteger power : validatorPowers) {
      computedTotalPower = computedTotalPower.add(power);
    }
    if (!computedTotalPower.equals(totalPower)) {
      throw new IllegalArgumentException("totalPower must equal validatorPowers sum");
    }

    final List<Integer> signerIndices = bscSignerIndicesFromBitmap(proof.signersBitmap, validatorAddresses.size());
    if (proof.signatures.size() != signerIndices.size()) {
      throw new IllegalArgumentException("signatures length must equal selected signers");
    }
    BigInteger computedSignedPower = BigInteger.ZERO;
    for (int i = 0; i < proof.signatures.size(); i++) {
      final byte[] signature = Objects.requireNonNull(proof.signatures.get(i), "signatures entry");
      if (!tronRecoverableSignatureIsCanonical(signature)) {
        throw new IllegalArgumentException(
            "signatures[" + i + "] must be a canonical recoverable secp256k1 signature");
      }
      final int signerIndex = signerIndices.get(i);
      final byte[] recoveredAddress = tronRecoveredSignerAddress20(commitMessageHash, signature);
      if (recoveredAddress == null || !Arrays.equals(recoveredAddress, validatorAddresses.get(signerIndex))) {
        throw new IllegalArgumentException(
            "signatures[" + i + "] must recover the selected validator address");
      }
      computedSignedPower = computedSignedPower.add(validatorPowers.get(signerIndex));
    }
    if (!computedSignedPower.equals(signedPower)) {
      throw new IllegalArgumentException("signedPower must equal selected validator power");
    }
    if (computedSignedPower.multiply(BigInteger.valueOf(3L))
        .compareTo(totalPower.multiply(BigInteger.valueOf(2L))) <= 0) {
      throw new IllegalArgumentException("signedPower must be greater than two thirds of totalPower");
    }

    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(proof.version);
    writeU64Le(out, totalPower);
    writeU64Le(out, signedPower);
    write(out, commitMessageHash);
    write(out, validatorSetHash);
    writeVector(out, proof.signersBitmap);
    writeU32Le(out, proof.signatures.size());
    for (final byte[] signature : proof.signatures) {
      writeVector(out, signature);
    }
    return out.toByteArray();
  }

  public static String bscCommitSealHash(final BscCommitSealProof proof) {
    return keccakHashHex("sccp:bsc:commit-seal:v1", canonicalBscCommitSealBytes(proof));
  }

  public static String bscValidatorSetStorageValueHash(final byte[] storageValue) {
    return keccakHashHex(
        "sccp:bsc:validator-set-storage-value:v1",
        Objects.requireNonNull(storageValue, "storageValue"));
  }

  public static byte[] canonicalBscValidatorSetMetadataProofBytes(
      final BscValidatorSetMetadataProof proof) {
    Objects.requireNonNull(proof, "proof");
    requireV1Version(proof.version, "BSC ValidatorSet metadata proof version");
    if (proof.validatorContractAddress.length != BSC_PARLIA_VALIDATOR_ADDRESS_BYTES) {
      throw new IllegalArgumentException("validatorContractAddress must be 20 bytes");
    }
    validateMptProofNodes(proof.accountProofNodes, "accountProofNodes");
    validateMptProofNodes(proof.validatorSetLengthProofNodes, "validatorSetLengthProofNodes");
    if (proof.validatorStorageProofs.isEmpty()
        || proof.validatorStorageProofs.size() > BSC_MAX_PARLIA_VALIDATORS) {
      throw new IllegalArgumentException(
          "validatorStorageProofs must contain 1.." + BSC_MAX_PARLIA_VALIDATORS + " entries");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(proof.version);
    write(out, hex32Bytes(proof.stateRoot, "stateRoot"));
    write(out, hex32Bytes(proof.nextValidatorSetPayloadHash, "nextValidatorSetPayloadHash"));
    writeVector(out, proof.validatorContractAddress);
    writeU32Le(out, proof.accountProofNodes.size());
    for (final byte[] node : proof.accountProofNodes) {
      writeVector(out, Objects.requireNonNull(node, "accountProofNodes entry"));
    }
    write(out, hex32Bytes(proof.storageRoot, "storageRoot"));
    write(out, hex32Bytes(proof.validatorSetLengthSlot, "validatorSetLengthSlot"));
    final byte[] validatorSetLengthValueHash =
        hex32Bytes(proof.validatorSetLengthValueHash, "validatorSetLengthValueHash");
    if (!Arrays.equals(
        validatorSetLengthValueHash,
        hex32Bytes(
            bscValidatorSetStorageValueHash(proof.validatorSetLengthValue),
            "validatorSetLengthValueHash"))) {
      throw new IllegalArgumentException(
          "validatorSetLengthValueHash must match validatorSetLengthValue");
    }
    writeVector(out, proof.validatorSetLengthValue);
    write(out, validatorSetLengthValueHash);
    writeU32Le(out, proof.validatorSetLengthProofNodes.size());
    for (final byte[] node : proof.validatorSetLengthProofNodes) {
      writeVector(out, Objects.requireNonNull(node, "validatorSetLengthProofNodes entry"));
    }
    writeU32Le(out, proof.validatorStorageProofs.size());
    for (final BscValidatorStorageProof storageProof : proof.validatorStorageProofs) {
      write(out, canonicalBscValidatorStorageProofBytes(storageProof));
    }
    return out.toByteArray();
  }

  public static String bscValidatorSetMetadataProofHash(
      final BscValidatorSetMetadataProof proof) {
    return keccakHashHex(
        "sccp:bsc:validator-set-metadata:v1",
        canonicalBscValidatorSetMetadataProofBytes(proof));
  }

  public static byte[] canonicalBscValidatorSetTransitionMessageBytes(
      final String fromValidatorEpoch,
      final String toValidatorEpoch,
      final String transitionBlockNumber,
      final String transitionBlockHash,
      final String parentValidatorSetHash,
      final String nextValidatorSetHash,
      final String nextValidatorSetPayloadHash,
      final String validatorSetMetadataProofHash) {
    return canonicalBscValidatorSetTransitionMessageBytes(
        fromValidatorEpoch,
        toValidatorEpoch,
        transitionBlockNumber,
        transitionBlockHash,
        parentValidatorSetHash,
        nextValidatorSetHash,
        nextValidatorSetPayloadHash,
        validatorSetMetadataProofHash,
        DOMAIN_BSC);
  }

  public static byte[] canonicalBscValidatorSetTransitionMessageBytes(
      final String fromValidatorEpoch,
      final String toValidatorEpoch,
      final String transitionBlockNumber,
      final String transitionBlockHash,
      final String parentValidatorSetHash,
      final String nextValidatorSetHash,
      final String nextValidatorSetPayloadHash,
      final String validatorSetMetadataProofHash,
      final int sourceDomain) {
    if (sourceDomain != DOMAIN_BSC) {
      throw new IllegalArgumentException("sourceDomain must be BSC");
    }
    final BigInteger fromEpoch = normalizeU64(fromValidatorEpoch, "fromValidatorEpoch");
    final BigInteger toEpoch = normalizeU64(toValidatorEpoch, "toValidatorEpoch");
    if (!fromEpoch.add(BigInteger.ONE).equals(toEpoch)) {
      throw new IllegalArgumentException("toValidatorEpoch must equal fromValidatorEpoch + 1");
    }
    final BigInteger transitionBlock = normalizeU64(transitionBlockNumber, "transitionBlockNumber");
    if (!transitionBlock.equals(toEpoch.multiply(BigInteger.valueOf(BSC_PARLIA_EPOCH_LENGTH_BLOCKS)))) {
      throw new IllegalArgumentException(
          "transitionBlockNumber must be the BSC Parlia epoch-start block");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, normalizeDomain(sourceDomain, "sourceDomain"));
    writeU64Le(out, fromEpoch);
    writeU64Le(out, toEpoch);
    writeU64Le(out, transitionBlock);
    write(out, hex32Bytes(transitionBlockHash, "transitionBlockHash"));
    write(out, hex32Bytes(parentValidatorSetHash, "parentValidatorSetHash"));
    write(out, hex32Bytes(nextValidatorSetHash, "nextValidatorSetHash"));
    write(out, hex32Bytes(nextValidatorSetPayloadHash, "nextValidatorSetPayloadHash"));
    write(out, hex32Bytes(validatorSetMetadataProofHash, "validatorSetMetadataProofHash"));
    return out.toByteArray();
  }

  public static String bscValidatorSetTransitionMessageHash(
      final String fromValidatorEpoch,
      final String toValidatorEpoch,
      final String transitionBlockNumber,
      final String transitionBlockHash,
      final String parentValidatorSetHash,
      final String nextValidatorSetHash,
      final String nextValidatorSetPayloadHash,
      final String validatorSetMetadataProofHash) {
    return bscValidatorSetTransitionMessageHash(
        fromValidatorEpoch,
        toValidatorEpoch,
        transitionBlockNumber,
        transitionBlockHash,
        parentValidatorSetHash,
        nextValidatorSetHash,
        nextValidatorSetPayloadHash,
        validatorSetMetadataProofHash,
        DOMAIN_BSC);
  }

  public static String bscValidatorSetTransitionMessageHash(
      final String fromValidatorEpoch,
      final String toValidatorEpoch,
      final String transitionBlockNumber,
      final String transitionBlockHash,
      final String parentValidatorSetHash,
      final String nextValidatorSetHash,
      final String nextValidatorSetPayloadHash,
      final String validatorSetMetadataProofHash,
      final int sourceDomain) {
    return keccakHashHex(
        "sccp:bsc:validator-set-transition-message:v1",
        canonicalBscValidatorSetTransitionMessageBytes(
            fromValidatorEpoch,
            toValidatorEpoch,
            transitionBlockNumber,
            transitionBlockHash,
            parentValidatorSetHash,
            nextValidatorSetHash,
            nextValidatorSetPayloadHash,
            validatorSetMetadataProofHash,
            sourceDomain));
  }

  public static byte[] bscValidatorSetPayloadFromParliaExtra(final byte[] extraData) {
    final List<byte[]> candidates =
        bscParliaValidatorSetPayloadCandidatesFromExtra(Objects.requireNonNull(extraData, "extraData"));
    if (candidates.size() != 1) {
      throw new IllegalArgumentException(
          "BSC Parlia extraData must contain one unambiguous validator set");
    }
    return candidates.get(0);
  }

  public static byte[] bscValidatorSetPayloadFromHeaderRlp(final byte[] headerRlp) {
    final List<byte[]> fields = rlpListByteFields(Objects.requireNonNull(headerRlp, "headerRlp"));
    if (fields.size() < 13) {
      throw new IllegalArgumentException("BSC Parlia header RLP must contain an extraData field");
    }
    return bscValidatorSetPayloadFromParliaExtra(fields.get(12));
  }

  public static byte[] canonicalTonValidatorSetBytes(
      final List<byte[]> validatorPublicKeys, final List<String> validatorWeights) {
    final TonValidatorSetParts normalized =
        normalizeTonValidatorSet(validatorPublicKeys, validatorWeights);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, normalized.publicKeys.size());
    for (int i = 0; i < normalized.publicKeys.size(); i++) {
      write(out, normalized.publicKeys.get(i));
      writeU64Le(out, normalized.weights.get(i));
    }
    return out.toByteArray();
  }

  public static byte[] canonicalTonValidatorSetPayloadBytes(
      final List<byte[]> validatorPublicKeys, final List<String> validatorWeights) {
    return canonicalTonValidatorSetBytes(validatorPublicKeys, validatorWeights);
  }

  public static String tonValidatorSetHashFromPayload(final byte[] payload) {
    validateTonValidatorSetPayload(Objects.requireNonNull(payload, "payload"));
    return hashHex("sccp:ton:validator-set:v1", payload);
  }

  public static String tonValidatorSetPayloadHash(final byte[] payload) {
    validateTonValidatorSetPayload(Objects.requireNonNull(payload, "payload"));
    return hashHex("sccp:ton:validator-set-payload:v1", payload);
  }

  public static String tonValidatorSetHash(
      final List<byte[]> validatorPublicKeys, final List<String> validatorWeights) {
    return tonValidatorSetHashFromPayload(
        canonicalTonValidatorSetBytes(validatorPublicKeys, validatorWeights));
  }

  public static byte[] canonicalTonMasterchainBlockMessageBytes(
      final String masterchainSeqno,
      final int masterchainWorkchainId,
      final String masterchainShard,
      final String masterchainBlockHash,
      final String masterchainFileHash,
      final String validatorSetHash,
      final String masterchainConfigRoot,
      final String masterchainConfigProofHash,
      final int shardWorkchainId,
      final String shardShard,
      final String shardSeqno,
      final String shardBlockHash,
      final String shardFileHash,
      final String shardStateRoot,
      final String transactionRoot,
      final String shardProofHash,
      final int sourceDomain) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, normalizeDomain(sourceDomain, "sourceDomain"));
    writeU64Le(out, normalizeU64(masterchainSeqno, "masterchainSeqno"));
    if (masterchainWorkchainId != TON_MASTERCHAIN_WORKCHAIN_ID) {
      throw new IllegalArgumentException("masterchainWorkchainId must be TON masterchain");
    }
    final BigInteger normalizedShard = normalizeU64(masterchainShard, "masterchainShard");
    if (!TON_MASTERCHAIN_SHARD.equals(normalizedShard)) {
      throw new IllegalArgumentException("masterchainShard must be TON masterchain shard");
    }
    writeI32Le(out, masterchainWorkchainId);
    writeU64Le(out, normalizedShard);
    write(out, nonZeroHex32Bytes(masterchainBlockHash, "masterchainBlockHash"));
    write(out, nonZeroHex32Bytes(masterchainFileHash, "masterchainFileHash"));
    write(out, hex32Bytes(validatorSetHash, "validatorSetHash"));
    write(out, hex32Bytes(masterchainConfigRoot, "masterchainConfigRoot"));
    write(out, hex32Bytes(masterchainConfigProofHash, "masterchainConfigProofHash"));
    if (shardWorkchainId != TON_BASECHAIN_WORKCHAIN_ID) {
      throw new IllegalArgumentException("shardWorkchainId must be TON basechain");
    }
    final BigInteger normalizedBasechainShard = normalizeU64(shardShard, "shardShard");
    if (BigInteger.ZERO.equals(normalizedBasechainShard)) {
      throw new IllegalArgumentException("shardShard must not be zero");
    }
    final BigInteger normalizedShardSeqno = normalizeU64(shardSeqno, "shardSeqno");
    if (BigInteger.ZERO.equals(normalizedShardSeqno)) {
      throw new IllegalArgumentException("shardSeqno must not be zero");
    }
    writeI32Le(out, shardWorkchainId);
    writeU64Le(out, normalizedBasechainShard);
    writeU64Le(out, normalizedShardSeqno);
    write(out, hex32Bytes(shardBlockHash, "shardBlockHash"));
    write(out, nonZeroHex32Bytes(shardFileHash, "shardFileHash"));
    write(out, hex32Bytes(shardStateRoot, "shardStateRoot"));
    write(out, hex32Bytes(transactionRoot, "transactionRoot"));
    write(out, hex32Bytes(shardProofHash, "shardProofHash"));
    return out.toByteArray();
  }

  public static String tonMasterchainBlockMessageHash(
      final String masterchainSeqno,
      final int masterchainWorkchainId,
      final String masterchainShard,
      final String masterchainBlockHash,
      final String masterchainFileHash,
      final String validatorSetHash,
      final String masterchainConfigRoot,
      final String masterchainConfigProofHash,
      final int shardWorkchainId,
      final String shardShard,
      final String shardSeqno,
      final String shardBlockHash,
      final String shardFileHash,
      final String shardStateRoot,
      final String transactionRoot,
      final String shardProofHash,
      final int sourceDomain) {
    return hashHex(
        "sccp:ton:masterchain-block-message:v1",
        canonicalTonMasterchainBlockMessageBytes(
            masterchainSeqno,
            masterchainWorkchainId,
            masterchainShard,
            masterchainBlockHash,
            masterchainFileHash,
            validatorSetHash,
            masterchainConfigRoot,
            masterchainConfigProofHash,
            shardWorkchainId,
            shardShard,
            shardSeqno,
            shardBlockHash,
            shardFileHash,
            shardStateRoot,
            transactionRoot,
            shardProofHash,
            sourceDomain));
  }

  public static byte[] canonicalTonMasterchainValidatorSignaturesBytes(
      final TonValidatorSignatureProof proof) {
    Objects.requireNonNull(proof, "proof");
    final String derivedValidatorSetHash =
        tonValidatorSetHash(proof.validatorPublicKeys, proof.validatorWeights);
    if (proof.validatorSetHash != null
        && !normalizeHex32(proof.validatorSetHash).equals(derivedValidatorSetHash)) {
      throw new IllegalArgumentException(
          "validatorSetHash must match validator public keys and weights");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    write(out, canonicalTonValidatorSignaturesProofBytes(proof));
    write(out, hex32Bytes(derivedValidatorSetHash, "validatorSetHash"));
    return out.toByteArray();
  }

  public static String tonMasterchainValidatorSignaturesHash(
      final TonValidatorSignatureProof proof) {
    return hashHex(
        "sccp:ton:masterchain-signatures:v1",
        canonicalTonMasterchainValidatorSignaturesBytes(proof));
  }

  public static String ethExecutionPayloadHeaderRootFromRlp(final byte[] headerRlp) {
    final byte[] headerBytes = Objects.requireNonNull(headerRlp, "headerRlp");
    final List<byte[]> fields = rlpListByteFields(headerBytes);
    if (fields.size() < 19) {
      throw new IllegalArgumentException(
          "headerRlp must include Deneb/Fulu execution payload fields");
    }
    final List<byte[]> chunks = new ArrayList<byte[]>();
    chunks.add(sszByteVectorRoot(fields.get(0), 32, "parentHash"));
    chunks.add(sszByteVectorRoot(fields.get(2), 20, "feeRecipient"));
    chunks.add(sszByteVectorRoot(fields.get(3), 32, "stateRoot"));
    chunks.add(sszByteVectorRoot(fields.get(5), 32, "receiptsRoot"));
    chunks.add(sszByteVectorRoot(fields.get(6), 256, "logsBloom"));
    chunks.add(sszByteVectorRoot(fields.get(13), 32, "prevRandao"));
    chunks.add(sszU64ChunkFromRlp(fields.get(8), "blockNumber"));
    chunks.add(sszU64ChunkFromRlp(fields.get(9), "gasLimit"));
    chunks.add(sszU64ChunkFromRlp(fields.get(10), "gasUsed"));
    chunks.add(sszU64ChunkFromRlp(fields.get(11), "timestamp"));
    chunks.add(sszByteListRoot(fields.get(12), 32, "extraData"));
    chunks.add(sszU256ChunkFromRlp(fields.get(15), "baseFeePerGas"));
    chunks.add(keccak256(headerBytes));
    chunks.add(sszByteVectorRoot(fields.get(4), 32, "transactionsRoot"));
    chunks.add(sszByteVectorRoot(fields.get(16), 32, "withdrawalsRoot"));
    chunks.add(sszU64ChunkFromRlp(fields.get(17), "blobGasUsed"));
    chunks.add(sszU64ChunkFromRlp(fields.get(18), "excessBlobGas"));
    return "0x" + hexLower(sszMerkleizeChunks(chunks));
  }

  public static String ethBeaconBodyRootFromExecutionPayloadBranch(
      final String executionPayloadHeaderRoot, final List<byte[]> executionPayloadBranch) {
    Objects.requireNonNull(executionPayloadBranch, "executionPayloadBranch");
    if (executionPayloadBranch.size() != ETH_EXECUTION_PAYLOAD_BODY_BRANCH_DEPTH) {
      throw new IllegalArgumentException(
          "executionPayloadBranch must contain "
              + ETH_EXECUTION_PAYLOAD_BODY_BRANCH_DEPTH
              + " siblings");
    }
    return "0x"
        + hexLower(
            sszMerkleRootFromBranch(
                hex32Bytes(executionPayloadHeaderRoot, "executionPayloadHeaderRoot"),
                ETH_EXECUTION_PAYLOAD_BODY_FIELD_INDEX,
                executionPayloadBranch,
                "executionPayloadBranch"));
  }

  public static String ethBeaconBlockHeaderRoot(
      final String beaconSlot,
      final String beaconProposerIndex,
      final String beaconParentRoot,
      final String beaconStateRoot,
      final String beaconBodyRoot) {
    final List<byte[]> chunks = new ArrayList<byte[]>();
    chunks.add(sszU64Chunk(normalizeU64(beaconSlot, "beaconSlot")));
    chunks.add(sszU64Chunk(normalizeU64(beaconProposerIndex, "beaconProposerIndex")));
    chunks.add(hex32Bytes(beaconParentRoot, "beaconParentRoot"));
    chunks.add(hex32Bytes(beaconStateRoot, "beaconStateRoot"));
    chunks.add(hex32Bytes(beaconBodyRoot, "beaconBodyRoot"));
    return "0x" + hexLower(sszMerkleizeChunks(chunks));
  }

  public static byte[] canonicalEvmReceiptRootMptValue(final String receiptRoot) {
    final List<byte[]> fields = new ArrayList<byte[]>();
    fields.add(rlpBytes(EVM_RECEIPT_ROOT_VALUE_MARKER));
    fields.add(rlpBytes(hex32Bytes(receiptRoot, "receiptRoot")));
    final byte[] value = rlpList(fields);
    if (value.length == 0 || value.length > EVM_MAX_RECEIPT_VALUE_BYTES) {
      throw new IllegalArgumentException(
          "EVM receipt root MPT value must contain 1.." + EVM_MAX_RECEIPT_VALUE_BYTES + " bytes");
    }
    return value;
  }

  public static byte[] canonicalTronReceiptRootMptValue(final String receiptRoot) {
    final List<byte[]> fields = new ArrayList<byte[]>();
    fields.add(rlpBytes(TRON_RECEIPT_ROOT_VALUE_MARKER));
    fields.add(rlpBytes(nonZeroHex32Bytes(receiptRoot, "receiptRoot")));
    final byte[] value = rlpList(fields);
    if (value.length == 0 || value.length > TRON_MAX_RECEIPT_VALUE_BYTES) {
      throw new IllegalArgumentException(
          "TRON receipt root MPT value must contain 1.." + TRON_MAX_RECEIPT_VALUE_BYTES + " bytes");
    }
    return value;
  }

  public static byte[] canonicalTronReceiptProofBytes(
      final String sourceEventDigest,
      final String receiptRoot,
      final String transactionRoot,
      final List<byte[]> inclusionBranch) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    write(out, nonZeroHex32Bytes(sourceEventDigest, "sourceEventDigest"));
    write(out, nonZeroHex32Bytes(receiptRoot, "receiptRoot"));
    write(out, nonZeroHex32Bytes(transactionRoot, "transactionRoot"));
    writeBranch(out, inclusionBranch, true);
    return out.toByteArray();
  }

  public static String tronReceiptProofHash(
      final String sourceEventDigest,
      final String receiptRoot,
      final String transactionRoot,
      final List<byte[]> inclusionBranch) {
    return hashHex(
        "sccp:tron:receipt-proof:v1",
        canonicalTronReceiptProofBytes(
            sourceEventDigest, receiptRoot, transactionRoot, inclusionBranch));
  }

  public static byte[] canonicalTronReceiptStateProofBytes(
      final String sourceEventDigest,
      final String receiptRoot,
      final String transactionRoot,
      final String receiptRootIndex,
      final List<byte[]> receiptTrieProofNodes,
      final List<byte[]> inclusionBranch) {
    validateTronMptProofNodes(receiptTrieProofNodes);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    write(out, nonZeroHex32Bytes(sourceEventDigest, "sourceEventDigest"));
    write(out, nonZeroHex32Bytes(receiptRoot, "receiptRoot"));
    write(out, nonZeroHex32Bytes(transactionRoot, "transactionRoot"));
    writeU64Le(out, normalizeU64(receiptRootIndex, "receiptRootIndex"));
    writeU32Le(out, receiptTrieProofNodes.size());
    for (final byte[] node : receiptTrieProofNodes) {
      writeVector(out, node);
    }
    writeBranch(out, inclusionBranch, true);
    return out.toByteArray();
  }

  public static String tronReceiptStateProofHash(
      final String sourceEventDigest,
      final String receiptRoot,
      final String transactionRoot,
      final String receiptRootIndex,
      final List<byte[]> receiptTrieProofNodes,
      final List<byte[]> inclusionBranch) {
    return hashHex(
        "sccp:tron:receipt-state-proof:v1",
        canonicalTronReceiptStateProofBytes(
            sourceEventDigest,
            receiptRoot,
            transactionRoot,
            receiptRootIndex,
            receiptTrieProofNodes,
            inclusionBranch));
  }

  public static byte[] tronSourceMessageCallData(
      final int sourceDomain, final int targetDomain, final String sourceEventDigest) {
    if (sourceDomain != DOMAIN_TRON) {
      throw new IllegalArgumentException(
          "sourceDomain must be TRON for SCCP TRON source-call calldata");
    }
    if (targetDomain != DOMAIN_SORA) {
      throw new IllegalArgumentException(
          "targetDomain must be SORA for SCCP TRON source-call calldata");
    }
    final byte[] selector = Arrays.copyOfRange(keccak256(TRON_SOURCE_MESSAGE_CALL_ABI), 0, 4);
    final byte[] digest = nonZeroHex32Bytes(sourceEventDigest, "sourceEventDigest");
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    write(out, selector);
    write(out, abiWordU32(sourceDomain, "sourceDomain"));
    write(out, abiWordU32(targetDomain, "targetDomain"));
    write(out, digest);
    return out.toByteArray();
  }

  public static byte[] canonicalTronTransactionSourceProofBytes(
      final String sourceEventDigest,
      final String receiptRoot,
      final String transactionRoot,
      final String transactionIndex,
      final String transactionCount,
      final byte[] transactionBytes,
      final List<byte[]> transactionMerkleBranch,
      final List<byte[]> inclusionBranch) {
    return canonicalTronTransactionSourceProofBytes(
        sourceEventDigest,
        receiptRoot,
        transactionRoot,
        transactionIndex,
        transactionCount,
        transactionBytes,
        transactionMerkleBranch,
        inclusionBranch,
        null,
        null);
  }

  public static byte[] canonicalTronTransactionSourceProofBytes(
      final String sourceEventDigest,
      final String receiptRoot,
      final String transactionRoot,
      final String transactionIndex,
      final String transactionCount,
      final byte[] transactionBytes,
      final List<byte[]> transactionMerkleBranch,
      final List<byte[]> inclusionBranch,
      final String sourceBridgeEmitterAddress,
      final String sourceBridgeOwnerAddress) {
    final BigInteger index = normalizeU64(transactionIndex, "transactionIndex");
    final BigInteger count = normalizeU64(transactionCount, "transactionCount");
    if (count.signum() == 0 || index.compareTo(count) >= 0) {
      throw new IllegalArgumentException("transactionIndex must be less than non-zero transactionCount");
    }
    final byte[] transaction = Objects.requireNonNull(transactionBytes, "transactionBytes");
    if (transaction.length == 0 || transaction.length > TRON_MAX_TRANSACTION_BYTES) {
      throw new IllegalArgumentException(
          "transactionBytes must contain 1.." + TRON_MAX_TRANSACTION_BYTES + " bytes");
    }
    final byte[] expectedContractAddress =
        sourceBridgeEmitterAddress == null
            ? null
            : nonZeroHexBytes(sourceBridgeEmitterAddress, "sourceBridgeEmitterAddress", 20);
    final byte[] expectedOwnerAddress =
        sourceBridgeOwnerAddress == null
            ? null
            : nonZeroHexBytes(sourceBridgeOwnerAddress, "sourceBridgeOwnerAddress", 20);
    validateTronTransactionSourceCall(
        transaction,
        nonZeroHex32Bytes(sourceEventDigest, "sourceEventDigest"),
        expectedContractAddress,
        expectedOwnerAddress);
    validateTronTransactionMerkleBranch(transactionMerkleBranch);
    final byte[] transactionRootBytes = nonZeroHex32Bytes(transactionRoot, "transactionRoot");
    if (!Arrays.equals(
        tronTransactionMerkleRootFromBranch(transaction, index, count, transactionMerkleBranch),
        transactionRootBytes)) {
      throw new IllegalArgumentException(
          "transactionRoot must match transactionBytes and transactionMerkleBranch");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    write(out, nonZeroHex32Bytes(sourceEventDigest, "sourceEventDigest"));
    write(out, nonZeroHex32Bytes(receiptRoot, "receiptRoot"));
    write(out, transactionRootBytes);
    writeU64Le(out, index);
    writeU64Le(out, count);
    writeVector(out, transaction);
    writeU32Le(out, transactionMerkleBranch.size());
    for (final byte[] sibling : transactionMerkleBranch) {
      write(out, sibling);
    }
    writeBranch(out, inclusionBranch, true);
    return out.toByteArray();
  }

  public static String tronTransactionSourceProofHash(
      final String sourceEventDigest,
      final String receiptRoot,
      final String transactionRoot,
      final String transactionIndex,
      final String transactionCount,
      final byte[] transactionBytes,
      final List<byte[]> transactionMerkleBranch,
      final List<byte[]> inclusionBranch) {
    return hashHex(
        "sccp:tron:transaction-source-proof:v1",
        canonicalTronTransactionSourceProofBytes(
            sourceEventDigest,
            receiptRoot,
            transactionRoot,
            transactionIndex,
            transactionCount,
            transactionBytes,
            transactionMerkleBranch,
            inclusionBranch));
  }

  public static String tronTransactionSourceProofHash(
      final String sourceEventDigest,
      final String receiptRoot,
      final String transactionRoot,
      final String transactionIndex,
      final String transactionCount,
      final byte[] transactionBytes,
      final List<byte[]> transactionMerkleBranch,
      final List<byte[]> inclusionBranch,
      final String sourceBridgeEmitterAddress,
      final String sourceBridgeOwnerAddress) {
    return hashHex(
        "sccp:tron:transaction-source-proof:v1",
        canonicalTronTransactionSourceProofBytes(
            sourceEventDigest,
            receiptRoot,
            transactionRoot,
            transactionIndex,
            transactionCount,
            transactionBytes,
            transactionMerkleBranch,
            inclusionBranch,
            sourceBridgeEmitterAddress,
            sourceBridgeOwnerAddress));
  }

  public static byte[] canonicalTronRawBlockHeaderBytes(
      final String number,
      final String txTrieRoot,
      final String accountStateRoot,
      final String parentBlockId,
      final String witnessAddress,
      final int headerVersion,
      final String timestampMs) {
    final BigInteger blockNumber = normalizeU64(number, "number");
    if (BigInteger.ZERO.equals(blockNumber)) {
      throw new IllegalArgumentException("number must not be zero");
    }
    final BigInteger timestamp = normalizeU64(timestampMs, "timestampMs");
    if (BigInteger.ZERO.equals(timestamp)) {
      throw new IllegalArgumentException("timestampMs must not be zero");
    }
    if (headerVersion <= 0) {
      throw new IllegalArgumentException("headerVersion must be a non-zero u32");
    }
    final byte[] txRoot = hex32Bytes(txTrieRoot, "txTrieRoot");
    if (isZero(txRoot)) {
      throw new IllegalArgumentException("txTrieRoot must not be zero");
    }
    final byte[] accountRoot = hex32Bytes(accountStateRoot, "accountStateRoot");
    if (isZero(accountRoot)) {
      throw new IllegalArgumentException("accountStateRoot must not be zero");
    }
    final byte[] parentId = hex32Bytes(parentBlockId, "parentBlockId");
    if (isZero(parentId)) {
      throw new IllegalArgumentException("parentBlockId must not be zero");
    }
    final byte[] witness = hexBytes(witnessAddress, "witnessAddress", 21);
    if (!isNonZeroTronAddress(witness)) {
      throw new IllegalArgumentException("witnessAddress must be a TRON 0x41-prefixed address");
    }

    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeProtobufU64(out, 1, timestamp);
    writeProtobufBytes(out, 2, txRoot);
    writeProtobufBytes(out, 3, parentId);
    writeProtobufU64(out, 7, blockNumber);
    writeProtobufBytes(out, 9, witness);
    writeProtobufU64(out, 10, BigInteger.valueOf(headerVersion));
    writeProtobufBytes(out, 11, accountRoot);
    return out.toByteArray();
  }

  public static String tronRawBlockHeaderHash(final byte[] rawData) {
    return "0x" + hexLower(sha256(Objects.requireNonNull(rawData, "rawData")));
  }

  public static String tronBlockIdFromRawDataHash(final String number, final String rawDataHash) {
    final BigInteger blockNumber = normalizeU64(number, "number");
    if (BigInteger.ZERO.equals(blockNumber)) {
      throw new IllegalArgumentException("number must not be zero");
    }
    return "0x" + hexLower(
        tronBlockIdBytesFromRawDataHash(blockNumber, hex32Bytes(rawDataHash, "rawDataHash")));
  }

  public static byte[] canonicalTronSolidBlockHeaderProofBytes(
      final byte[] rawData,
      final byte[] witnessSignature,
      final byte[] parentRawData,
      final byte[] parentWitnessSignature,
      final String rawDataHash,
      final String parentRawDataHash,
      final String blockId,
      final String txTrieRoot,
      final String accountStateRoot,
      final String parentBlockId,
      final String witnessAddress,
      final String timestampMs,
      final int headerVersion) {
    return canonicalTronSolidBlockHeaderProofBytes(
        rawData,
        witnessSignature,
        parentRawData,
        parentWitnessSignature,
        rawDataHash,
        parentRawDataHash,
        blockId,
        txTrieRoot,
        accountStateRoot,
        parentBlockId,
        witnessAddress,
        timestampMs,
        headerVersion,
        1);
  }

  public static byte[] canonicalTronSolidBlockHeaderProofBytes(
      final byte[] rawData,
      final byte[] witnessSignature,
      final byte[] parentRawData,
      final byte[] parentWitnessSignature,
      final String rawDataHash,
      final String parentRawDataHash,
      final String blockId,
      final String txTrieRoot,
      final String accountStateRoot,
      final String parentBlockId,
      final String witnessAddress,
      final String timestampMs,
      final int headerVersion,
      final int version) {
    if (version != 1) {
      throw new IllegalArgumentException("version must be 1");
    }
    final byte[] raw = Objects.requireNonNull(rawData, "rawData");
    final byte[] parentRaw = Objects.requireNonNull(parentRawData, "parentRawData");
    if (raw.length == 0 || parentRaw.length == 0) {
      throw new IllegalArgumentException("rawData and parentRawData must not be empty");
    }
    if (raw.length > TRON_MAX_RAW_HEADER_BYTES || parentRaw.length > TRON_MAX_RAW_HEADER_BYTES) {
      throw new IllegalArgumentException(
          "rawData and parentRawData must be at most " + TRON_MAX_RAW_HEADER_BYTES + " bytes");
    }
    final byte[] signature = Objects.requireNonNull(witnessSignature, "witnessSignature");
    final byte[] parentSignature =
        Objects.requireNonNull(parentWitnessSignature, "parentWitnessSignature");
    if (signature.length != 65 || parentSignature.length != 65) {
      throw new IllegalArgumentException("TRON header signatures must be 65 bytes");
    }
    if (!tronRecoverableSignatureIsCanonical(signature)
        || !tronRecoverableSignatureIsCanonical(parentSignature)) {
      throw new IllegalArgumentException(
          "TRON header signatures must be canonical low-S with recovery id 0..3 or 27..30");
    }
    final byte[] witness = hexBytes(witnessAddress, "witnessAddress", 21);
    if (!isNonZeroTronAddress(witness)) {
      throw new IllegalArgumentException("witnessAddress must be a TRON 0x41-prefixed address");
    }
    final BigInteger timestamp = normalizeU64(timestampMs, "timestampMs");
    if (BigInteger.ZERO.equals(timestamp)) {
      throw new IllegalArgumentException("timestampMs must not be zero");
    }
    if (headerVersion <= 0) {
      throw new IllegalArgumentException("headerVersion must be a non-zero u32");
    }
    final byte[] txRoot = hex32Bytes(txTrieRoot, "txTrieRoot");
    if (isZero(txRoot)) {
      throw new IllegalArgumentException("txTrieRoot must not be zero");
    }
    final byte[] accountRoot = hex32Bytes(accountStateRoot, "accountStateRoot");
    if (isZero(accountRoot)) {
      throw new IllegalArgumentException("accountStateRoot must not be zero");
    }
    final byte[] parentId = hex32Bytes(parentBlockId, "parentBlockId");
    if (isZero(parentId)) {
      throw new IllegalArgumentException("parentBlockId must not be zero");
    }
    final TronRawBlockHeaderFields fields = decodeTronRawBlockHeaderFields(raw, "rawData");
    final TronRawBlockHeaderFields parentFields =
        decodeTronRawBlockHeaderFields(parentRaw, "parentRawData");
    final byte[] rawHash = hex32Bytes(rawDataHash, "rawDataHash");
    final byte[] parentRawHash = hex32Bytes(parentRawDataHash, "parentRawDataHash");
    final byte[] suppliedBlockId = hex32Bytes(blockId, "blockId");
    if (!Arrays.equals(rawHash, sha256(raw))) {
      throw new IllegalArgumentException("rawDataHash must match rawData");
    }
    if (!Arrays.equals(parentRawHash, sha256(parentRaw))) {
      throw new IllegalArgumentException("parentRawDataHash must match parentRawData");
    }
    if (!Arrays.equals(suppliedBlockId, tronBlockIdBytesFromRawDataHash(fields.number, rawHash))) {
      throw new IllegalArgumentException("blockId must match rawDataHash and block number");
    }
    if (!Arrays.equals(parentId, fields.parentBlockId)) {
      throw new IllegalArgumentException("parentBlockId must match rawData");
    }
    if (!Arrays.equals(
        parentId,
        tronBlockIdBytesFromRawDataHash(parentFields.number, parentRawHash))) {
      throw new IllegalArgumentException(
          "parentBlockId must match parentRawDataHash and parent block number");
    }
    if (!parentFields.number.add(BigInteger.ONE).equals(fields.number)
        || parentFields.timestampMs.compareTo(fields.timestampMs) >= 0) {
      throw new IllegalArgumentException("rawData must be the direct child of parentRawData");
    }
    if (!Arrays.equals(txRoot, fields.txTrieRoot)
        || !Arrays.equals(accountRoot, fields.accountStateRoot)
        || !Arrays.equals(witness, fields.witnessAddress)
        || !timestamp.equals(fields.timestampMs)
        || headerVersion != fields.headerVersion) {
      throw new IllegalArgumentException("TRON solid-block header fields must match rawData");
    }

    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(version);
    writeVector(out, raw);
    writeVector(out, signature);
    writeVector(out, parentRaw);
    writeVector(out, parentSignature);
    write(out, rawHash);
    write(out, parentRawHash);
    write(out, suppliedBlockId);
    write(out, txRoot);
    write(out, accountRoot);
    write(out, parentId);
    writeVector(out, witness);
    writeU64Le(out, timestamp);
    writeU32Le(out, headerVersion);
    return out.toByteArray();
  }

  public static String tronSolidBlockHeaderProofHash(
      final byte[] rawData,
      final byte[] witnessSignature,
      final byte[] parentRawData,
      final byte[] parentWitnessSignature,
      final String rawDataHash,
      final String parentRawDataHash,
      final String blockId,
      final String txTrieRoot,
      final String accountStateRoot,
      final String parentBlockId,
      final String witnessAddress,
      final String timestampMs,
      final int headerVersion) {
    return tronSolidBlockHeaderProofHash(
        rawData,
        witnessSignature,
        parentRawData,
        parentWitnessSignature,
        rawDataHash,
        parentRawDataHash,
        blockId,
        txTrieRoot,
        accountStateRoot,
        parentBlockId,
        witnessAddress,
        timestampMs,
        headerVersion,
        1);
  }

  public static String tronSolidBlockHeaderProofHash(
      final byte[] rawData,
      final byte[] witnessSignature,
      final byte[] parentRawData,
      final byte[] parentWitnessSignature,
      final String rawDataHash,
      final String parentRawDataHash,
      final String blockId,
      final String txTrieRoot,
      final String accountStateRoot,
      final String parentBlockId,
      final String witnessAddress,
      final String timestampMs,
      final int headerVersion,
      final int version) {
    return hashHex(
        "sccp:tron:solid-block-header-proof:v1",
        canonicalTronSolidBlockHeaderProofBytes(
            rawData,
            witnessSignature,
            parentRawData,
            parentWitnessSignature,
            rawDataHash,
            parentRawDataHash,
            blockId,
            txTrieRoot,
            accountStateRoot,
            parentBlockId,
            witnessAddress,
            timestampMs,
            headerVersion,
            version));
  }

  public static byte[] canonicalSubstrateStorageProofBytes(
      final int sourceDomain,
      final String sourceEventDigest,
      final String sourceEventLeafIndex,
      final String finalizedBlockNumber,
      final String grandpaSetId,
      final String blockHash,
      final String authoritySetHash,
      final String eventsRoot,
      final List<byte[]> inclusionBranch) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, normalizeDomain(sourceDomain, "sourceDomain"));
    write(out, nonZeroHex32Bytes(sourceEventDigest, "sourceEventDigest"));
    write(out, SUBSTRATE_SYSTEM_EVENTS_STORAGE_KEY);
    writeU64Le(out, normalizeU64(sourceEventLeafIndex, "sourceEventLeafIndex"));
    writeU64Le(out, normalizeU64(finalizedBlockNumber, "finalizedBlockNumber"));
    writeU64Le(out, normalizeU64(grandpaSetId, "grandpaSetId"));
    write(out, hex32Bytes(blockHash, "blockHash"));
    write(out, hex32Bytes(authoritySetHash, "authoritySetHash"));
    write(out, hex32Bytes(eventsRoot, "eventsRoot"));
    writeBranch(out, inclusionBranch);
    return out.toByteArray();
  }

  public static String substrateStorageProofHash(
      final int sourceDomain,
      final String sourceEventDigest,
      final String sourceEventLeafIndex,
      final String finalizedBlockNumber,
      final String grandpaSetId,
      final String blockHash,
      final String authoritySetHash,
      final String eventsRoot,
      final List<byte[]> inclusionBranch) {
    return hashHex(
        "sccp:substrate:storage-proof:v1",
        canonicalSubstrateStorageProofBytes(
            sourceDomain,
            sourceEventDigest,
            sourceEventLeafIndex,
            finalizedBlockNumber,
            grandpaSetId,
            blockHash,
            authoritySetHash,
            eventsRoot,
            inclusionBranch));
  }

  private static boolean isSubstrateRuntimeStorageSourceDomain(final int sourceDomain) {
    return sourceDomain == DOMAIN_SORA_KUSAMA
        || sourceDomain == DOMAIN_SORA_POLKADOT
        || sourceDomain == DOMAIN_SORA2;
  }

  private static byte[] substrateTemplateSourceStateVerifierHash(final int sourceDomain) {
    switch (sourceDomain) {
      case DOMAIN_SORA_KUSAMA:
        return SORA_KUSAMA_TEMPLATE_SOURCE_STATE_VERIFIER_HASH;
      case DOMAIN_SORA_POLKADOT:
        return SORA_POLKADOT_TEMPLATE_SOURCE_STATE_VERIFIER_HASH;
      case DOMAIN_SORA2:
        return SORA2_TEMPLATE_SOURCE_STATE_VERIFIER_HASH;
      default:
        throw new IllegalArgumentException("sourceDomain must be a Substrate-family SCCP source domain");
    }
  }

  private static NormalizedSourceMaterial substrateRuntimeStorageSourceMaterial(
      final int sourceDomain,
      final String sourceTrustAnchorHash,
      final String consensusVerifierHash,
      final String messageInclusionVerifierHash,
      final String finalityPolicyHash,
      final String sourceStateVerifierHash) {
    if (!isSubstrateRuntimeStorageSourceDomain(sourceDomain)) {
      throw new IllegalArgumentException("sourceDomain must be a Substrate-family SCCP source domain");
    }
    final NormalizedSourceMaterial material =
        normalizeSourceMaterial(
            sourceDomain,
            sourceTrustAnchorHash,
            consensusVerifierHash,
            messageInclusionVerifierHash,
            finalityPolicyHash,
            sourceStateVerifierHash,
            null,
            null,
            null,
            null,
            null);
    if (material.profile.sourceStateVerifierId.isEmpty() || isZero(material.sourceStateVerifierHash)) {
      throw new IllegalArgumentException(
          "sourceStateVerifierHash must bind a deployed Substrate runtime-storage verifier");
    }
    if (Arrays.equals(
        material.sourceStateVerifierHash,
        substrateTemplateSourceStateVerifierHash(sourceDomain))) {
      throw new IllegalArgumentException(
          "sourceStateVerifierHash must not be the Substrate template verifier hash");
    }
    return material;
  }

  private static byte[] wordU32Le(final int value) {
    final byte[] out = new byte[32];
    final ByteArrayOutputStream word = new ByteArrayOutputStream();
    writeU32Le(word, value);
    System.arraycopy(word.toByteArray(), 0, out, 0, 4);
    return out;
  }

  private static byte[] wordU64Le(final BigInteger value) {
    final byte[] out = new byte[32];
    final ByteArrayOutputStream word = new ByteArrayOutputStream();
    writeU64Le(word, value);
    System.arraycopy(word.toByteArray(), 0, out, 0, 8);
    return out;
  }

  public static byte[] canonicalSubstrateRuntimeStorageVerificationStatementBytes(
      final int sourceDomain,
      final String sourceEventDigest,
      final String sourceEventLeafIndex,
      final String finalizedBlockNumber,
      final String grandpaSetId,
      final String blockHash,
      final String authoritySetHash,
      final String eventsRoot,
      final List<byte[]> inclusionBranch,
      final String storageProofHash) {
    if (!isSubstrateRuntimeStorageSourceDomain(normalizeDomain(sourceDomain, "sourceDomain"))) {
      throw new IllegalArgumentException("sourceDomain must be a Substrate-family SCCP source domain");
    }
    final byte[] statement =
        canonicalSubstrateStorageProofBytes(
            sourceDomain,
            sourceEventDigest,
            sourceEventLeafIndex,
            finalizedBlockNumber,
            grandpaSetId,
            blockHash,
            authoritySetHash,
            eventsRoot,
            inclusionBranch);
    if (storageProofHash != null) {
      final String supplied = "0x" + hexLower(hex32Bytes(storageProofHash, "storageProofHash"));
      if (!supplied.equals(hashHex("sccp:substrate:storage-proof:v1", statement))) {
        throw new IllegalArgumentException(
            "storageProofHash must match Substrate runtime-storage statement");
      }
    }
    return statement;
  }

  public static String substrateRuntimeStorageProofPublicInputsHash(
      final int sourceDomain,
      final String sourceEventDigest,
      final String sourceEventLeafIndex,
      final String finalizedBlockNumber,
      final String grandpaSetId,
      final String blockHash,
      final String authoritySetHash,
      final String eventsRoot,
      final List<byte[]> inclusionBranch,
      final String storageProofHash) {
    return hashHex(
        SUBSTRATE_RUNTIME_STORAGE_PROOF_PUBLIC_INPUTS_PREFIX_V1,
        canonicalSubstrateRuntimeStorageVerificationStatementBytes(
            sourceDomain,
            sourceEventDigest,
            sourceEventLeafIndex,
            finalizedBlockNumber,
            grandpaSetId,
            blockHash,
            authoritySetHash,
            eventsRoot,
            inclusionBranch,
            storageProofHash));
  }

  public static byte[] canonicalSubstrateRuntimeStorageVerificationContextBytes(
      final int sourceDomain,
      final String sourceEventDigest,
      final String sourceEventLeafIndex,
      final String finalizedBlockNumber,
      final String grandpaSetId,
      final String blockHash,
      final String authoritySetHash,
      final String eventsRoot,
      final String sourceTrustAnchorHash,
      final String consensusVerifierHash,
      final String messageInclusionVerifierHash,
      final String finalityPolicyHash,
      final String sourceStateVerifierHash,
      final List<byte[]> inclusionBranch,
      final String storageProofHash) {
    final NormalizedSourceMaterial material =
        substrateRuntimeStorageSourceMaterial(
            sourceDomain,
            sourceTrustAnchorHash,
            consensusVerifierHash,
            messageInclusionVerifierHash,
            finalityPolicyHash,
            sourceStateVerifierHash);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeVector(out, SUBSTRATE_RUNTIME_STORAGE_OPEN_VERIFY_CIRCUIT_ID_V1.getBytes(StandardCharsets.UTF_8));
    writeVector(out, SUBSTRATE_RUNTIME_STORAGE_FASTPQ_PARAMETER_SET_V1.getBytes(StandardCharsets.UTF_8));
    writeVector(out, material.profile.sourceStateVerifierId.getBytes(StandardCharsets.UTF_8));
    write(out, material.sourceStateVerifierHash);
    writeVector(out, material.profile.sourceTrustAnchorId.getBytes(StandardCharsets.UTF_8));
    write(out, material.sourceTrustAnchorHash);
    writeVector(out, material.profile.consensusVerifierId.getBytes(StandardCharsets.UTF_8));
    write(out, material.consensusVerifierHash);
    writeVector(out, material.profile.messageInclusionVerifierId.getBytes(StandardCharsets.UTF_8));
    write(out, material.messageInclusionVerifierHash);
    writeVector(out, material.profile.finalityPolicyId.getBytes(StandardCharsets.UTF_8));
    write(out, material.finalityPolicyHash);
    write(
        out,
        hex32Bytes(
            substrateRuntimeStorageProofPublicInputsHash(
                sourceDomain,
                sourceEventDigest,
                sourceEventLeafIndex,
                finalizedBlockNumber,
                grandpaSetId,
                blockHash,
                authoritySetHash,
                eventsRoot,
                inclusionBranch,
                storageProofHash),
            "runtimeStorageProofPublicInputsHash"));
    return out.toByteArray();
  }

  public static List<List<String>> substrateRuntimeStoragePublicInputColumns(
      final int sourceDomain,
      final String sourceEventDigest,
      final String sourceEventLeafIndex,
      final String finalizedBlockNumber,
      final String grandpaSetId,
      final String blockHash,
      final String authoritySetHash,
      final String eventsRoot,
      final List<byte[]> inclusionBranch,
      final String storageProofHash) {
    final String computedStorageProofHash =
        substrateStorageProofHash(
            sourceDomain,
            sourceEventDigest,
            sourceEventLeafIndex,
            finalizedBlockNumber,
            grandpaSetId,
            blockHash,
            authoritySetHash,
            eventsRoot,
            inclusionBranch);
    if (storageProofHash != null) {
      final String supplied = "0x" + hexLower(hex32Bytes(storageProofHash, "storageProofHash"));
      if (!supplied.equals(computedStorageProofHash)) {
        throw new IllegalArgumentException(
            "storageProofHash must match Substrate runtime-storage statement");
      }
    }
    final String publicInputsHash =
        substrateRuntimeStorageProofPublicInputsHash(
            sourceDomain,
            sourceEventDigest,
            sourceEventLeafIndex,
            finalizedBlockNumber,
            grandpaSetId,
            blockHash,
            authoritySetHash,
            eventsRoot,
            inclusionBranch,
            computedStorageProofHash);
    final List<List<String>> columns = new ArrayList<>();
    columns.add(Collections.singletonList("0x" + hexLower(wordU32Le(normalizeDomain(sourceDomain, "sourceDomain")))));
    columns.add(Collections.singletonList("0x" + hexLower(wordU64Le(normalizeU64(finalizedBlockNumber, "finalizedBlockNumber")))));
    columns.add(Collections.singletonList("0x" + hexLower(wordU64Le(normalizeU64(grandpaSetId, "grandpaSetId")))));
    columns.add(Collections.singletonList("0x" + hexLower(hex32Bytes(blockHash, "blockHash"))));
    columns.add(Collections.singletonList("0x" + hexLower(hex32Bytes(authoritySetHash, "authoritySetHash"))));
    columns.add(Collections.singletonList("0x" + hexLower(hex32Bytes(eventsRoot, "eventsRoot"))));
    columns.add(Collections.singletonList(computedStorageProofHash));
    columns.add(Collections.singletonList("0x" + hexLower(hex32Bytes(sourceEventDigest, "sourceEventDigest"))));
    columns.add(Collections.singletonList("0x" + hexLower(SUBSTRATE_SYSTEM_EVENTS_STORAGE_KEY)));
    columns.add(Collections.singletonList("0x" + hexLower(wordU64Le(normalizeU64(sourceEventLeafIndex, "sourceEventLeafIndex")))));
    columns.add(Collections.singletonList(publicInputsHash));
    return columns;
  }

  public static byte[] substrateRuntimeStorageOpenVerifySchemaDescriptor(final int sourceDomain) {
    final int normalizedSourceDomain = normalizeDomain(sourceDomain, "sourceDomain");
    if (!isSubstrateRuntimeStorageSourceDomain(normalizedSourceDomain)) {
      throw new IllegalArgumentException("sourceDomain must be a Substrate-family SCCP source domain");
    }
    final SourceAdapterVerifierProfile profile = sourceAdapterVerifierProfile(normalizedSourceDomain);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeVector(out, SUBSTRATE_RUNTIME_STORAGE_OPEN_VERIFY_CIRCUIT_ID_V1.getBytes(StandardCharsets.UTF_8));
    writeVector(out, SUBSTRATE_RUNTIME_STORAGE_FASTPQ_PARAMETER_SET_V1.getBytes(StandardCharsets.UTF_8));
    writeVector(out, profile.chain.getBytes(StandardCharsets.UTF_8));
    writeU32Le(out, normalizedSourceDomain);
    final String[] requiredInputs = {
      "source_domain",
      "finalized_block_number",
      "grandpa_set_id",
      "block_hash",
      "authority_set_hash",
      "events_root",
      "storage_proof_hash",
      "source_event_digest",
      "system_events_storage_key",
      "source_event_leaf_index",
      "runtime_storage_proof_public_inputs_hash"
    };
    for (final String requiredInput : requiredInputs) {
      writeVector(out, requiredInput.getBytes(StandardCharsets.UTF_8));
    }
    return out.toByteArray();
  }

  public static SubstrateRuntimeStorageProofRequest buildSubstrateRuntimeStorageProofRequest(
      final int sourceDomain,
      final String sourceEventDigest,
      final String sourceEventLeafIndex,
      final String finalizedBlockNumber,
      final String grandpaSetId,
      final String blockHash,
      final String authoritySetHash,
      final String eventsRoot,
      final String sourceTrustAnchorHash,
      final String consensusVerifierHash,
      final String messageInclusionVerifierHash,
      final String finalityPolicyHash,
      final String sourceStateVerifierHash,
      final List<byte[]> inclusionBranch,
      final String storageProofHash) {
    final NormalizedSourceMaterial material =
        substrateRuntimeStorageSourceMaterial(
            sourceDomain,
            sourceTrustAnchorHash,
            consensusVerifierHash,
            messageInclusionVerifierHash,
            finalityPolicyHash,
            sourceStateVerifierHash);
    final byte[] statement =
        canonicalSubstrateRuntimeStorageVerificationStatementBytes(
            sourceDomain,
            sourceEventDigest,
            sourceEventLeafIndex,
            finalizedBlockNumber,
            grandpaSetId,
            blockHash,
            authoritySetHash,
            eventsRoot,
            inclusionBranch,
            storageProofHash);
    final String computedStorageProofHash = hashHex("sccp:substrate:storage-proof:v1", statement);
    final String publicInputsHash =
        substrateRuntimeStorageProofPublicInputsHash(
            sourceDomain,
            sourceEventDigest,
            sourceEventLeafIndex,
            finalizedBlockNumber,
            grandpaSetId,
            blockHash,
            authoritySetHash,
            eventsRoot,
            inclusionBranch,
            computedStorageProofHash);
    final byte[] context =
        canonicalSubstrateRuntimeStorageVerificationContextBytes(
            sourceDomain,
            sourceEventDigest,
            sourceEventLeafIndex,
            finalizedBlockNumber,
            grandpaSetId,
            blockHash,
            authoritySetHash,
            eventsRoot,
            sourceTrustAnchorHash,
            consensusVerifierHash,
            messageInclusionVerifierHash,
            finalityPolicyHash,
            sourceStateVerifierHash,
            inclusionBranch,
            computedStorageProofHash);
    final byte[] dsidHash =
        hashBytes(
            SUBSTRATE_RUNTIME_STORAGE_FASTPQ_DSID_PREFIX_V1,
            hex32Bytes(publicInputsHash, "runtimeStorageProofPublicInputsHash"));
    final List<SubstrateRuntimeStorageFastpqTransition> transitions = new ArrayList<>();
    transitions.add(
        new SubstrateRuntimeStorageFastpqTransition(
            SUBSTRATE_RUNTIME_STORAGE_FASTPQ_STATEMENT_KEY_V1,
            "meta_set",
            "0x",
            "0x" + hexLower(statement)));
    transitions.add(
        new SubstrateRuntimeStorageFastpqTransition(
            SUBSTRATE_RUNTIME_STORAGE_FASTPQ_CONTEXT_KEY_V1,
            "meta_set",
            "0x",
            "0x" + hexLower(context)));
    transitions.add(
        new SubstrateRuntimeStorageFastpqTransition(
            SUBSTRATE_RUNTIME_STORAGE_FASTPQ_STORAGE_KEY_V1,
            "meta_set",
            "0x",
            "0x" + hexLower(SUBSTRATE_SYSTEM_EVENTS_STORAGE_KEY)));
    java.util.Collections.sort(
        transitions,
        (left, right) -> left.key.compareTo(right.key));
    return new SubstrateRuntimeStorageProofRequest(
        1,
        STARK_FRI_PROOF_FAMILY_V1,
        SUBSTRATE_RUNTIME_STORAGE_OPEN_VERIFY_CIRCUIT_ID_V1,
        SUBSTRATE_RUNTIME_STORAGE_FASTPQ_PARAMETER_SET_V1,
        normalizeDomain(sourceDomain, "sourceDomain"),
        normalizeU64(finalizedBlockNumber, "finalizedBlockNumber").toString(),
        normalizeU64(grandpaSetId, "grandpaSetId").toString(),
        material.profile.sourceStateVerifierId,
        "0x" + hexLower(material.sourceStateVerifierHash),
        publicInputsHash,
        computedStorageProofHash,
        statement,
        context,
        substrateRuntimeStorageOpenVerifySchemaDescriptor(sourceDomain),
        substrateRuntimeStoragePublicInputColumns(
            sourceDomain,
            sourceEventDigest,
            sourceEventLeafIndex,
            finalizedBlockNumber,
            grandpaSetId,
            blockHash,
            authoritySetHash,
            eventsRoot,
            inclusionBranch,
            computedStorageProofHash),
        new SubstrateRuntimeStorageFastpqPublicInputs(
            "0x" + hexLower(Arrays.copyOfRange(dsidHash, 0, 16)),
            normalizeU64(finalizedBlockNumber, "finalizedBlockNumber").toString(),
            "0x" + hexLower(hex32Bytes(authoritySetHash, "authoritySetHash")),
            "0x" + hexLower(hex32Bytes(blockHash, "blockHash")),
            "0x" + hexLower(hex32Bytes(eventsRoot, "eventsRoot")),
            publicInputsHash),
        transitions);
  }

  public static byte[] canonicalTronWitnessSchedulePayloadBytes(
      final List<String> witnessAddresses, final List<String> witnessWeights) {
    Objects.requireNonNull(witnessAddresses, "witnessAddresses");
    Objects.requireNonNull(witnessWeights, "witnessWeights");
    if (witnessAddresses.isEmpty() || witnessAddresses.size() != witnessWeights.size()) {
      throw new IllegalArgumentException(
          "witnessAddresses and witnessWeights must be non-empty equal-length arrays");
    }
    if (witnessAddresses.size() > TRON_MAX_WITNESSES) {
      throw new IllegalArgumentException(
          "witnessAddresses must contain at most " + TRON_MAX_WITNESSES + " entries");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, witnessAddresses.size());
    final Set<String> seenAddresses = new HashSet<>();
    BigInteger totalWeight = BigInteger.ZERO;
    for (int i = 0; i < witnessAddresses.size(); i++) {
      final byte[] address = hexBytes(witnessAddresses.get(i), "witnessAddresses[" + i + "]", 21);
      if (!isNonZeroTronAddress(address)) {
        throw new IllegalArgumentException(
            "witnessAddresses[" + i + "] must be a TRON 0x41-prefixed address");
      }
      final String addressHex = hexLower(address);
      if (!seenAddresses.add(addressHex)) {
        throw new IllegalArgumentException("witnessAddresses[" + i + "] must be unique");
      }
      final BigInteger weight = normalizeU64(witnessWeights.get(i), "witnessWeights[" + i + "]");
      if (BigInteger.ZERO.equals(weight)) {
        throw new IllegalArgumentException("witnessWeights[" + i + "] must not be zero");
      }
      totalWeight = totalWeight.add(weight);
      if (totalWeight.compareTo(MAX_U64) > 0) {
        throw new IllegalArgumentException("witnessWeights total must fit u64");
      }
      write(out, address);
      writeU64Le(out, weight);
    }
    return out.toByteArray();
  }

  public static String tronWitnessSchedulePayloadHash(final byte[] payload) {
    validateTronWitnessSchedulePayload(payload);
    return hashHex("sccp:tron:witness-schedule-payload:v1", payload);
  }

  public static String tronWitnessSchedulePayloadHash(
      final List<String> witnessAddresses, final List<String> witnessWeights) {
    return tronWitnessSchedulePayloadHash(
        canonicalTronWitnessSchedulePayloadBytes(witnessAddresses, witnessWeights));
  }

  public static String tronWitnessScheduleHashFromPayload(final byte[] payload) {
    validateTronWitnessSchedulePayload(payload);
    return hashHex("sccp:tron:witness-schedule:v1", payload);
  }

  public static String tronWitnessScheduleHashFromPayload(
      final List<String> witnessAddresses, final List<String> witnessWeights) {
    return tronWitnessScheduleHashFromPayload(
        canonicalTronWitnessSchedulePayloadBytes(witnessAddresses, witnessWeights));
  }

  public static byte[] canonicalTronSolidBlockMessageBytes(
      final int sourceDomain,
      final String solidBlockNumber,
      final String blockHash,
      final String witnessScheduleHash,
      final String receiptRoot,
      final String transactionRoot,
      final String receiptProofHash) {
    return canonicalTronSolidBlockMessageBytes(
        sourceDomain,
        solidBlockNumber,
        blockHash,
        witnessScheduleHash,
        receiptRoot,
        transactionRoot,
        receiptProofHash,
        1);
  }

  public static byte[] canonicalTronSolidBlockMessageBytes(
      final int sourceDomain,
      final String solidBlockNumber,
      final String blockHash,
      final String witnessScheduleHash,
      final String receiptRoot,
      final String transactionRoot,
      final String receiptProofHash,
      final int version) {
    requireV1Version(version, "TRON solid-block message version");
    final int normalizedSourceDomain = normalizeDomain(sourceDomain, "sourceDomain");
    if (normalizedSourceDomain != DOMAIN_TRON) {
      throw new IllegalArgumentException("sourceDomain must be TRON");
    }
    final BigInteger normalizedSolidBlockNumber =
        normalizeU64(solidBlockNumber, "solidBlockNumber");
    if (BigInteger.ZERO.equals(normalizedSolidBlockNumber)) {
      throw new IllegalArgumentException("solidBlockNumber must not be zero");
    }
    final byte[] blockHashBytes = nonZeroHex32Bytes(blockHash, "blockHash");
    final byte[] witnessScheduleHashBytes =
        nonZeroHex32Bytes(witnessScheduleHash, "witnessScheduleHash");
    final byte[] receiptRootBytes = nonZeroHex32Bytes(receiptRoot, "receiptRoot");
    final byte[] transactionRootBytes = nonZeroHex32Bytes(transactionRoot, "transactionRoot");
    final byte[] receiptProofHashBytes = nonZeroHex32Bytes(receiptProofHash, "receiptProofHash");
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(version);
    writeU32Le(out, normalizedSourceDomain);
    writeU64Le(out, normalizedSolidBlockNumber);
    write(out, blockHashBytes);
    write(out, witnessScheduleHashBytes);
    write(out, receiptRootBytes);
    write(out, transactionRootBytes);
    write(out, receiptProofHashBytes);
    return out.toByteArray();
  }

  public static String tronSolidBlockMessageHash(
      final int sourceDomain,
      final String solidBlockNumber,
      final String blockHash,
      final String witnessScheduleHash,
      final String receiptRoot,
      final String transactionRoot,
      final String receiptProofHash) {
    return tronSolidBlockMessageHash(
        sourceDomain,
        solidBlockNumber,
        blockHash,
        witnessScheduleHash,
        receiptRoot,
        transactionRoot,
        receiptProofHash,
        1);
  }

  public static String tronSolidBlockMessageHash(
      final int sourceDomain,
      final String solidBlockNumber,
      final String blockHash,
      final String witnessScheduleHash,
      final String receiptRoot,
      final String transactionRoot,
      final String receiptProofHash,
      final int version) {
    return keccakHashHex(
        "sccp:tron:solid-block-message:v1",
        canonicalTronSolidBlockMessageBytes(
            sourceDomain,
            solidBlockNumber,
            blockHash,
            witnessScheduleHash,
            receiptRoot,
            transactionRoot,
            receiptProofHash,
            version));
  }

  public static byte[] canonicalTronWitnessSealBytes(
      final String totalWeight,
      final String signedWeight,
      final String solidBlockMessageHash,
      final List<String> witnessAddresses,
      final List<String> witnessWeights,
      final byte[] signersBitmap,
      final List<byte[]> signatures) {
    return canonicalTronWitnessSealBytes(
        totalWeight,
        signedWeight,
        solidBlockMessageHash,
        witnessAddresses,
        witnessWeights,
        signersBitmap,
        signatures,
        1);
  }

  public static byte[] canonicalTronWitnessSealBytes(
      final String totalWeight,
      final String signedWeight,
      final String solidBlockMessageHash,
      final List<String> witnessAddresses,
      final List<String> witnessWeights,
      final byte[] signersBitmap,
      final List<byte[]> signatures,
      final int version) {
    final NormalizedTronWitnessSealProof proof =
        normalizeTronWitnessSealProof(
            version,
            totalWeight,
            signedWeight,
            solidBlockMessageHash,
            witnessAddresses,
            witnessWeights,
            signersBitmap,
            signatures);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    write(out, canonicalTronWitnessSealProofBytes(proof));
    write(out, proof.witnessScheduleHash);
    return out.toByteArray();
  }

  public static String tronWitnessSealHash(
      final String totalWeight,
      final String signedWeight,
      final String solidBlockMessageHash,
      final List<String> witnessAddresses,
      final List<String> witnessWeights,
      final byte[] signersBitmap,
      final List<byte[]> signatures) {
    return tronWitnessSealHash(
        totalWeight,
        signedWeight,
        solidBlockMessageHash,
        witnessAddresses,
        witnessWeights,
        signersBitmap,
        signatures,
        1);
  }

  public static String tronWitnessSealHash(
      final String totalWeight,
      final String signedWeight,
      final String solidBlockMessageHash,
      final List<String> witnessAddresses,
      final List<String> witnessWeights,
      final byte[] signersBitmap,
      final List<byte[]> signatures,
      final int version) {
    return hashHex(
        "sccp:tron:witness-seal:v1",
        canonicalTronWitnessSealBytes(
            totalWeight,
            signedWeight,
            solidBlockMessageHash,
            witnessAddresses,
            witnessWeights,
            signersBitmap,
            signatures,
            version));
  }

  public static byte[] canonicalTronWitnessScheduleTransitionMessageBytes(
      final int sourceDomain,
      final String fromWitnessScheduleEpoch,
      final String toWitnessScheduleEpoch,
      final String transitionBlockNumber,
      final String transitionBlockHash,
      final String parentWitnessScheduleHash,
      final String nextWitnessScheduleHash,
      final String nextWitnessSchedulePayloadHash,
      final byte[] nextWitnessSchedulePayload) {
    return canonicalTronWitnessScheduleTransitionMessageBytes(
        sourceDomain,
        fromWitnessScheduleEpoch,
        toWitnessScheduleEpoch,
        transitionBlockNumber,
        transitionBlockHash,
        parentWitnessScheduleHash,
        nextWitnessScheduleHash,
        nextWitnessSchedulePayloadHash,
        nextWitnessSchedulePayload,
        1);
  }

  public static byte[] canonicalTronWitnessScheduleTransitionMessageBytes(
      final int sourceDomain,
      final String fromWitnessScheduleEpoch,
      final String toWitnessScheduleEpoch,
      final String transitionBlockNumber,
      final String transitionBlockHash,
      final String parentWitnessScheduleHash,
      final String nextWitnessScheduleHash,
      final String nextWitnessSchedulePayloadHash,
      final byte[] nextWitnessSchedulePayload,
      final int version) {
    return canonicalTronWitnessScheduleTransitionMessageBytes(
        normalizeTronWitnessScheduleTransitionMessage(
            version,
            sourceDomain,
            fromWitnessScheduleEpoch,
            toWitnessScheduleEpoch,
            transitionBlockNumber,
            transitionBlockHash,
            parentWitnessScheduleHash,
            nextWitnessScheduleHash,
            nextWitnessSchedulePayloadHash,
            nextWitnessSchedulePayload));
  }

  public static String tronWitnessScheduleTransitionMessageHash(
      final int sourceDomain,
      final String fromWitnessScheduleEpoch,
      final String toWitnessScheduleEpoch,
      final String transitionBlockNumber,
      final String transitionBlockHash,
      final String parentWitnessScheduleHash,
      final String nextWitnessScheduleHash,
      final String nextWitnessSchedulePayloadHash,
      final byte[] nextWitnessSchedulePayload) {
    return tronWitnessScheduleTransitionMessageHash(
        sourceDomain,
        fromWitnessScheduleEpoch,
        toWitnessScheduleEpoch,
        transitionBlockNumber,
        transitionBlockHash,
        parentWitnessScheduleHash,
        nextWitnessScheduleHash,
        nextWitnessSchedulePayloadHash,
        nextWitnessSchedulePayload,
        1);
  }

  public static String tronWitnessScheduleTransitionMessageHash(
      final int sourceDomain,
      final String fromWitnessScheduleEpoch,
      final String toWitnessScheduleEpoch,
      final String transitionBlockNumber,
      final String transitionBlockHash,
      final String parentWitnessScheduleHash,
      final String nextWitnessScheduleHash,
      final String nextWitnessSchedulePayloadHash,
      final byte[] nextWitnessSchedulePayload,
      final int version) {
    return keccakHashHex(
        "sccp:tron:witness-schedule-transition-message:v1",
        canonicalTronWitnessScheduleTransitionMessageBytes(
            sourceDomain,
            fromWitnessScheduleEpoch,
            toWitnessScheduleEpoch,
            transitionBlockNumber,
            transitionBlockHash,
            parentWitnessScheduleHash,
            nextWitnessScheduleHash,
            nextWitnessSchedulePayloadHash,
            nextWitnessSchedulePayload,
            version));
  }

  public static byte[] canonicalTronWitnessScheduleTransitionSealBytes(
      final int sourceDomain,
      final String fromWitnessScheduleEpoch,
      final String toWitnessScheduleEpoch,
      final String transitionBlockNumber,
      final String transitionBlockHash,
      final String parentWitnessScheduleHash,
      final String nextWitnessScheduleHash,
      final byte[] nextWitnessSchedulePayload,
      final String transitionMessageHash,
      final String totalWeight,
      final String signedWeight,
      final List<String> witnessAddresses,
      final List<String> witnessWeights,
      final byte[] signersBitmap,
      final List<byte[]> signatures) {
    return canonicalTronWitnessScheduleTransitionSealBytes(
        sourceDomain,
        fromWitnessScheduleEpoch,
        toWitnessScheduleEpoch,
        transitionBlockNumber,
        transitionBlockHash,
        parentWitnessScheduleHash,
        nextWitnessScheduleHash,
        nextWitnessSchedulePayload,
        transitionMessageHash,
        totalWeight,
        signedWeight,
        witnessAddresses,
        witnessWeights,
        signersBitmap,
        signatures,
        1);
  }

  public static byte[] canonicalTronWitnessScheduleTransitionSealBytes(
      final int sourceDomain,
      final String fromWitnessScheduleEpoch,
      final String toWitnessScheduleEpoch,
      final String transitionBlockNumber,
      final String transitionBlockHash,
      final String parentWitnessScheduleHash,
      final String nextWitnessScheduleHash,
      final byte[] nextWitnessSchedulePayload,
      final String transitionMessageHash,
      final String totalWeight,
      final String signedWeight,
      final List<String> witnessAddresses,
      final List<String> witnessWeights,
      final byte[] signersBitmap,
      final List<byte[]> signatures,
      final int version) {
    final NormalizedTronWitnessScheduleTransitionMessage message =
        normalizeTronWitnessScheduleTransitionMessage(
            version,
            sourceDomain,
            fromWitnessScheduleEpoch,
            toWitnessScheduleEpoch,
            transitionBlockNumber,
            transitionBlockHash,
            parentWitnessScheduleHash,
            nextWitnessScheduleHash,
            null,
            nextWitnessSchedulePayload);
    final byte[] expectedTransitionMessageHash =
        hex32Bytes(
            tronWitnessScheduleTransitionMessageHash(
                sourceDomain,
                fromWitnessScheduleEpoch,
                toWitnessScheduleEpoch,
                transitionBlockNumber,
                transitionBlockHash,
                parentWitnessScheduleHash,
                nextWitnessScheduleHash,
                null,
                nextWitnessSchedulePayload,
                version),
            "transitionMessageHash");
    final byte[] transitionMessageHashBytes =
        nonZeroHex32Bytes(transitionMessageHash, "transitionMessageHash");
    if (!Arrays.equals(transitionMessageHashBytes, expectedTransitionMessageHash)) {
      throw new IllegalArgumentException(
          "transitionMessageHash must match transition message fields");
    }
    final NormalizedTronWitnessSealProof proof =
        normalizeTronWitnessSealProof(
            version,
            totalWeight,
            signedWeight,
            transitionMessageHash,
            witnessAddresses,
            witnessWeights,
            signersBitmap,
            signatures);
    if (!Arrays.equals(proof.witnessScheduleHash, message.parentWitnessScheduleHash)) {
      throw new IllegalArgumentException(
          "parentWitnessScheduleHash must match witness seal proof");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(version);
    writeU32Le(out, message.sourceDomain);
    writeU64Le(out, message.fromWitnessScheduleEpoch);
    writeU64Le(out, message.toWitnessScheduleEpoch);
    writeU64Le(out, message.transitionBlockNumber);
    write(out, message.transitionBlockHash);
    write(out, message.parentWitnessScheduleHash);
    write(out, message.nextWitnessScheduleHash);
    writeVector(out, nextWitnessSchedulePayload);
    write(out, message.nextWitnessSchedulePayloadHash);
    write(out, transitionMessageHashBytes);
    write(out, proof.witnessScheduleHash);
    write(out, canonicalTronWitnessSealProofBytes(proof));
    return out.toByteArray();
  }

  public static String tronWitnessScheduleTransitionSealHash(
      final int sourceDomain,
      final String fromWitnessScheduleEpoch,
      final String toWitnessScheduleEpoch,
      final String transitionBlockNumber,
      final String transitionBlockHash,
      final String parentWitnessScheduleHash,
      final String nextWitnessScheduleHash,
      final byte[] nextWitnessSchedulePayload,
      final String transitionMessageHash,
      final String totalWeight,
      final String signedWeight,
      final List<String> witnessAddresses,
      final List<String> witnessWeights,
      final byte[] signersBitmap,
      final List<byte[]> signatures) {
    return tronWitnessScheduleTransitionSealHash(
        sourceDomain,
        fromWitnessScheduleEpoch,
        toWitnessScheduleEpoch,
        transitionBlockNumber,
        transitionBlockHash,
        parentWitnessScheduleHash,
        nextWitnessScheduleHash,
        nextWitnessSchedulePayload,
        transitionMessageHash,
        totalWeight,
        signedWeight,
        witnessAddresses,
        witnessWeights,
        signersBitmap,
        signatures,
        1);
  }

  public static String tronWitnessScheduleTransitionSealHash(
      final int sourceDomain,
      final String fromWitnessScheduleEpoch,
      final String toWitnessScheduleEpoch,
      final String transitionBlockNumber,
      final String transitionBlockHash,
      final String parentWitnessScheduleHash,
      final String nextWitnessScheduleHash,
      final byte[] nextWitnessSchedulePayload,
      final String transitionMessageHash,
      final String totalWeight,
      final String signedWeight,
      final List<String> witnessAddresses,
      final List<String> witnessWeights,
      final byte[] signersBitmap,
      final List<byte[]> signatures,
      final int version) {
    return hashHex(
        "sccp:tron:witness-schedule-transition-seal:v1",
        canonicalTronWitnessScheduleTransitionSealBytes(
            sourceDomain,
            fromWitnessScheduleEpoch,
            toWitnessScheduleEpoch,
            transitionBlockNumber,
            transitionBlockHash,
            parentWitnessScheduleHash,
            nextWitnessScheduleHash,
            nextWitnessSchedulePayload,
            transitionMessageHash,
            totalWeight,
            signedWeight,
            witnessAddresses,
            witnessWeights,
            signersBitmap,
            signatures,
            version));
  }

  public static byte[] canonicalSubstrateAuthoritySetPayloadBytes(
      final List<String> authorityPublicKeys, final List<String> authorityWeights) {
    Objects.requireNonNull(authorityPublicKeys, "authorityPublicKeys");
    Objects.requireNonNull(authorityWeights, "authorityWeights");
    if (authorityPublicKeys.isEmpty() || authorityPublicKeys.size() != authorityWeights.size()) {
      throw new IllegalArgumentException(
          "authorityPublicKeys and authorityWeights must be non-empty equal-length arrays");
    }
    if (authorityPublicKeys.size() > SUBSTRATE_MAX_AUTHORITIES) {
      throw new IllegalArgumentException(
          "authorityPublicKeys must contain at most "
              + SUBSTRATE_MAX_AUTHORITIES
              + " entries");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, authorityPublicKeys.size());
    final Set<String> seenPublicKeys = new HashSet<>();
    for (int i = 0; i < authorityPublicKeys.size(); i++) {
      final byte[] publicKey =
          hexBytes(authorityPublicKeys.get(i), "authorityPublicKeys[" + i + "]", 32);
      if (isZero(publicKey)) {
        throw new IllegalArgumentException("authorityPublicKeys[" + i + "] must not be zero");
      }
      final String publicKeyHex = hexLower(publicKey);
      if (!seenPublicKeys.add(publicKeyHex)) {
        throw new IllegalArgumentException("authorityPublicKeys[" + i + "] must be unique");
      }
      final BigInteger weight = normalizeU64(authorityWeights.get(i), "authorityWeights[" + i + "]");
      if (BigInteger.ZERO.equals(weight)) {
        throw new IllegalArgumentException("authorityWeights[" + i + "] must not be zero");
      }
      write(out, publicKey);
      writeU64Le(out, weight);
    }
    return out.toByteArray();
  }

  public static String substrateAuthoritySetPayloadHash(final byte[] payload) {
    validateSubstrateAuthoritySetPayload(payload);
    return hashHex(
        "sccp:substrate:authority-set-payload:v1", Objects.requireNonNull(payload, "payload"));
  }

  public static String substrateAuthoritySetPayloadHash(
      final List<String> authorityPublicKeys, final List<String> authorityWeights) {
    return substrateAuthoritySetPayloadHash(
        canonicalSubstrateAuthoritySetPayloadBytes(authorityPublicKeys, authorityWeights));
  }

  public static String substrateAuthoritySetHashFromPayload(final byte[] payload) {
    validateSubstrateAuthoritySetPayload(payload);
    return hashHex("sccp:substrate:authority-set:v1", Objects.requireNonNull(payload, "payload"));
  }

  public static String substrateAuthoritySetHashFromPayload(
      final List<String> authorityPublicKeys, final List<String> authorityWeights) {
    return substrateAuthoritySetHashFromPayload(
        canonicalSubstrateAuthoritySetPayloadBytes(authorityPublicKeys, authorityWeights));
  }

  public static byte[] canonicalSubstrateAuthoritySetTransitionMessageBytes(
      final int sourceDomain,
      final String fromGrandpaSetId,
      final String toGrandpaSetId,
      final String transitionBlockNumber,
      final String transitionBlockHash,
      final String parentAuthoritySetHash,
      final String nextAuthoritySetHash,
      final String nextAuthoritySetPayloadHash) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, normalizeDomain(sourceDomain, "sourceDomain"));
    writeU64Le(out, normalizeU64(fromGrandpaSetId, "fromGrandpaSetId"));
    writeU64Le(out, normalizeU64(toGrandpaSetId, "toGrandpaSetId"));
    writeU64Le(out, normalizeU64(transitionBlockNumber, "transitionBlockNumber"));
    write(out, hex32Bytes(transitionBlockHash, "transitionBlockHash"));
    write(out, hex32Bytes(parentAuthoritySetHash, "parentAuthoritySetHash"));
    write(out, hex32Bytes(nextAuthoritySetHash, "nextAuthoritySetHash"));
    write(out, hex32Bytes(nextAuthoritySetPayloadHash, "nextAuthoritySetPayloadHash"));
    return out.toByteArray();
  }

  public static String substrateAuthoritySetTransitionMessageHash(
      final int sourceDomain,
      final String fromGrandpaSetId,
      final String toGrandpaSetId,
      final String transitionBlockNumber,
      final String transitionBlockHash,
      final String parentAuthoritySetHash,
      final String nextAuthoritySetHash,
      final String nextAuthoritySetPayloadHash) {
    return hashHex(
        "sccp:substrate:authority-set-transition-message:v1",
        canonicalSubstrateAuthoritySetTransitionMessageBytes(
            sourceDomain,
            fromGrandpaSetId,
            toGrandpaSetId,
            transitionBlockNumber,
            transitionBlockHash,
            parentAuthoritySetHash,
            nextAuthoritySetHash,
            nextAuthoritySetPayloadHash));
  }

  public static byte[] canonicalSubstrateGrandpaJustificationProofBytes(
      final int version,
      final String totalWeight,
      final String signedWeight,
      final String precommitMessageHash,
      final List<String> authorityPublicKeys,
      final List<String> authorityWeights,
      final byte[] signersBitmap,
      final List<byte[]> signatures) {
    requireV1Version(version, "Substrate GRANDPA justification version");
    Objects.requireNonNull(authorityPublicKeys, "authorityPublicKeys");
    Objects.requireNonNull(authorityWeights, "authorityWeights");
    Objects.requireNonNull(signatures, "signatures");
    if (authorityPublicKeys.isEmpty() || authorityPublicKeys.size() != authorityWeights.size()) {
      throw new IllegalArgumentException(
          "authorityPublicKeys and authorityWeights must be non-empty equal-length arrays");
    }
    if (authorityPublicKeys.size() > SUBSTRATE_MAX_AUTHORITIES) {
      throw new IllegalArgumentException(
          "authorityPublicKeys must contain at most "
              + SUBSTRATE_MAX_AUTHORITIES
              + " entries");
    }
    if (signatures.size() > SUBSTRATE_MAX_AUTHORITIES) {
      throw new IllegalArgumentException(
          "signatures must contain at most " + SUBSTRATE_MAX_AUTHORITIES + " entries");
    }
    final BigInteger totalWeightValue = normalizeU64(totalWeight, "totalWeight");
    final BigInteger signedWeightValue = normalizeU64(signedWeight, "signedWeight");
    final byte[] precommitMessageHashBytes =
        hex32Bytes(precommitMessageHash, "precommitMessageHash");
    final List<byte[]> publicKeys = new ArrayList<>();
    final Set<String> seenPublicKeys = new HashSet<>();
    for (int i = 0; i < authorityPublicKeys.size(); i++) {
      final byte[] publicKey = hex32Bytes(authorityPublicKeys.get(i), "authorityPublicKeys[" + i + "]");
      if (isZero(publicKey)) {
        throw new IllegalArgumentException("authorityPublicKeys[" + i + "] must not be zero");
      }
      if (!seenPublicKeys.add(hexLower(publicKey))) {
        throw new IllegalArgumentException("authorityPublicKeys[" + i + "] must be unique");
      }
      publicKeys.add(publicKey);
    }
    final List<BigInteger> weights = new ArrayList<>();
    for (int i = 0; i < authorityWeights.size(); i++) {
      final BigInteger weight = normalizeU64(authorityWeights.get(i), "authorityWeights[" + i + "]");
      if (BigInteger.ZERO.equals(weight)) {
        throw new IllegalArgumentException("authorityWeights[" + i + "] must not be zero");
      }
      weights.add(weight);
    }
    BigInteger computedTotalWeight = BigInteger.ZERO;
    for (final BigInteger weight : weights) {
      computedTotalWeight = computedTotalWeight.add(weight);
    }
    if (!totalWeightValue.equals(computedTotalWeight)) {
      throw new IllegalArgumentException("totalWeight must match authorityWeights");
    }
    final byte[] signersBitmapBytes = Objects.requireNonNull(signersBitmap, "signersBitmap");
    final List<Integer> signerIndices =
        substrateAuthoritySignerIndices(signersBitmapBytes, publicKeys.size());
    if (signatures.size() != signerIndices.size()) {
      throw new IllegalArgumentException("signatures length must match signersBitmap");
    }
    BigInteger computedSignedWeight = BigInteger.ZERO;
    for (final int signerIndex : signerIndices) {
      computedSignedWeight = computedSignedWeight.add(weights.get(signerIndex));
    }
    if (!signedWeightValue.equals(computedSignedWeight)) {
      throw new IllegalArgumentException("signedWeight must match signersBitmap");
    }
    if (signedWeightValue.multiply(BigInteger.valueOf(3))
        .compareTo(totalWeightValue.multiply(BigInteger.valueOf(2))) <= 0) {
      throw new IllegalArgumentException(
          "signedWeight must be greater than two thirds of totalWeight");
    }
    for (int i = 0; i < signatures.size(); i++) {
      final byte[] signature = Objects.requireNonNull(signatures.get(i), "signatures[" + i + "]");
      if (signature.length != 64) {
        throw new IllegalArgumentException("signatures[" + i + "] must be 64 bytes");
      }
      if (isZero(signature)) {
        throw new IllegalArgumentException("signatures[" + i + "] must not be all zero");
      }
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(version);
    writeU64Le(out, totalWeightValue);
    writeU64Le(out, signedWeightValue);
    write(out, precommitMessageHashBytes);
    writeU32Le(out, publicKeys.size());
    for (final byte[] publicKey : publicKeys) {
      writeVector(out, publicKey);
    }
    writeU32Le(out, weights.size());
    for (final BigInteger weight : weights) {
      writeU64Le(out, weight);
    }
    writeVector(out, signersBitmapBytes);
    writeU32Le(out, signatures.size());
    for (int i = 0; i < signatures.size(); i++) {
      writeVector(out, signatures.get(i));
    }
    return out.toByteArray();
  }

  public static byte[] canonicalSubstrateAuthoritySetTransitionJustificationBytes(
      final int version,
      final int sourceDomain,
      final String fromGrandpaSetId,
      final String toGrandpaSetId,
      final String transitionBlockNumber,
      final String transitionBlockHash,
      final String parentAuthoritySetHash,
      final String nextAuthoritySetHash,
      final byte[] nextAuthoritySetPayload,
      final String nextAuthoritySetPayloadHash,
      final String transitionMessageHash,
      final int proofVersion,
      final String totalWeight,
      final String signedWeight,
      final List<String> authorityPublicKeys,
      final List<String> authorityWeights,
      final byte[] signersBitmap,
      final List<byte[]> signatures) {
    requireV1Version(version, "Substrate authority-set transition justification version");
    if (!substrateAuthoritySetPayloadHash(nextAuthoritySetPayload)
        .equals(normalizeHex32(nextAuthoritySetPayloadHash))) {
      throw new IllegalArgumentException(
          "nextAuthoritySetPayloadHash must match nextAuthoritySetPayload");
    }
    if (!substrateAuthoritySetHashFromPayload(nextAuthoritySetPayload)
        .equals(normalizeHex32(nextAuthoritySetHash))) {
      throw new IllegalArgumentException("nextAuthoritySetHash must match nextAuthoritySetPayload");
    }
    final String parentHash =
        substrateAuthoritySetHashFromPayload(authorityPublicKeys, authorityWeights);
    if (!parentHash.equals(normalizeHex32(parentAuthoritySetHash))) {
      throw new IllegalArgumentException(
          "parentAuthoritySetHash must match grandpaJustification authority set");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(version);
    writeU32Le(out, normalizeDomain(sourceDomain, "sourceDomain"));
    writeU64Le(out, normalizeU64(fromGrandpaSetId, "fromGrandpaSetId"));
    writeU64Le(out, normalizeU64(toGrandpaSetId, "toGrandpaSetId"));
    writeU64Le(out, normalizeU64(transitionBlockNumber, "transitionBlockNumber"));
    write(out, hex32Bytes(transitionBlockHash, "transitionBlockHash"));
    write(out, hex32Bytes(parentAuthoritySetHash, "parentAuthoritySetHash"));
    write(out, hex32Bytes(nextAuthoritySetHash, "nextAuthoritySetHash"));
    writeVector(out, Objects.requireNonNull(nextAuthoritySetPayload, "nextAuthoritySetPayload"));
    write(out, hex32Bytes(nextAuthoritySetPayloadHash, "nextAuthoritySetPayloadHash"));
    write(out, hex32Bytes(transitionMessageHash, "transitionMessageHash"));
    write(out, hex32Bytes(parentHash, "parentAuthoritySetHash"));
    write(
        out,
        canonicalSubstrateGrandpaJustificationProofBytes(
            proofVersion,
            totalWeight,
            signedWeight,
            transitionMessageHash,
            authorityPublicKeys,
            authorityWeights,
            signersBitmap,
            signatures));
    return out.toByteArray();
  }

  public static String substrateAuthoritySetTransitionJustificationHash(
      final int version,
      final int sourceDomain,
      final String fromGrandpaSetId,
      final String toGrandpaSetId,
      final String transitionBlockNumber,
      final String transitionBlockHash,
      final String parentAuthoritySetHash,
      final String nextAuthoritySetHash,
      final byte[] nextAuthoritySetPayload,
      final String nextAuthoritySetPayloadHash,
      final String transitionMessageHash,
      final int proofVersion,
      final String totalWeight,
      final String signedWeight,
      final List<String> authorityPublicKeys,
      final List<String> authorityWeights,
      final byte[] signersBitmap,
      final List<byte[]> signatures) {
    return hashHex(
        "sccp:substrate:authority-set-transition-justification:v1",
        canonicalSubstrateAuthoritySetTransitionJustificationBytes(
            version,
            sourceDomain,
            fromGrandpaSetId,
            toGrandpaSetId,
            transitionBlockNumber,
            transitionBlockHash,
            parentAuthoritySetHash,
            nextAuthoritySetHash,
            nextAuthoritySetPayload,
            nextAuthoritySetPayloadHash,
            transitionMessageHash,
            proofVersion,
            totalWeight,
            signedWeight,
            authorityPublicKeys,
            authorityWeights,
            signersBitmap,
            signatures));
  }

  private static void writeVector(final ByteArrayOutputStream out, final byte[] value) {
    writeU32Le(out, value.length);
    write(out, value);
  }

  private static List<byte[]> copyByteArrayList(final List<byte[]> values) {
    final List<byte[]> copy = new ArrayList<byte[]>();
    for (final byte[] value : values) {
      copy.add(Arrays.copyOf(value, value.length));
    }
    return copy;
  }

  private static final class NormalizedTronWitnessSealProof {
    final int version;
    final BigInteger totalWeight;
    final BigInteger signedWeight;
    final byte[] solidBlockMessageHash;
    final List<byte[]> witnessAddresses;
    final List<BigInteger> witnessWeights;
    final byte[] signersBitmap;
    final List<byte[]> signatures;
    final byte[] witnessScheduleHash;

    NormalizedTronWitnessSealProof(
        final int version,
        final BigInteger totalWeight,
        final BigInteger signedWeight,
        final byte[] solidBlockMessageHash,
        final List<byte[]> witnessAddresses,
        final List<BigInteger> witnessWeights,
        final byte[] signersBitmap,
        final List<byte[]> signatures,
        final byte[] witnessScheduleHash) {
      this.version = version;
      this.totalWeight = totalWeight;
      this.signedWeight = signedWeight;
      this.solidBlockMessageHash = solidBlockMessageHash;
      this.witnessAddresses = witnessAddresses;
      this.witnessWeights = witnessWeights;
      this.signersBitmap = signersBitmap;
      this.signatures = signatures;
      this.witnessScheduleHash = witnessScheduleHash;
    }
  }

  private static final class NormalizedTronWitnessScheduleTransitionMessage {
    final int version;
    final int sourceDomain;
    final BigInteger fromWitnessScheduleEpoch;
    final BigInteger toWitnessScheduleEpoch;
    final BigInteger transitionBlockNumber;
    final byte[] transitionBlockHash;
    final byte[] parentWitnessScheduleHash;
    final byte[] nextWitnessScheduleHash;
    final byte[] nextWitnessSchedulePayloadHash;

    NormalizedTronWitnessScheduleTransitionMessage(
        final int version,
        final int sourceDomain,
        final BigInteger fromWitnessScheduleEpoch,
        final BigInteger toWitnessScheduleEpoch,
        final BigInteger transitionBlockNumber,
        final byte[] transitionBlockHash,
        final byte[] parentWitnessScheduleHash,
        final byte[] nextWitnessScheduleHash,
        final byte[] nextWitnessSchedulePayloadHash) {
      this.version = version;
      this.sourceDomain = sourceDomain;
      this.fromWitnessScheduleEpoch = fromWitnessScheduleEpoch;
      this.toWitnessScheduleEpoch = toWitnessScheduleEpoch;
      this.transitionBlockNumber = transitionBlockNumber;
      this.transitionBlockHash = transitionBlockHash;
      this.parentWitnessScheduleHash = parentWitnessScheduleHash;
      this.nextWitnessScheduleHash = nextWitnessScheduleHash;
      this.nextWitnessSchedulePayloadHash = nextWitnessSchedulePayloadHash;
    }
  }

  private static List<Integer> tronWitnessSealSignerIndices(
      final byte[] bitmap, final int rosterLength) {
    Objects.requireNonNull(bitmap, "bitmap");
    if (rosterLength <= 0 || bitmap.length != (rosterLength + 7) / 8) {
      throw new IllegalArgumentException("signersBitmap length must match witness roster");
    }
    final List<Integer> indices = new ArrayList<>();
    for (int byteIndex = 0; byteIndex < bitmap.length; byteIndex++) {
      final int value = bitmap[byteIndex] & 0xff;
      for (int bitIndex = 0; bitIndex < 8; bitIndex++) {
        if (((value >>> bitIndex) & 1) == 0) {
          continue;
        }
        final int witnessIndex = byteIndex * 8 + bitIndex;
        if (witnessIndex >= rosterLength) {
          throw new IllegalArgumentException(
              "signersBitmap sets a bit outside the witness roster");
        }
        indices.add(witnessIndex);
      }
    }
    if (indices.isEmpty()) {
      throw new IllegalArgumentException("signersBitmap must select at least one witness");
    }
    return indices;
  }

  private static NormalizedTronWitnessSealProof normalizeTronWitnessSealProof(
      final int version,
      final String totalWeight,
      final String signedWeight,
      final String solidBlockMessageHash,
      final List<String> witnessAddresses,
      final List<String> witnessWeights,
      final byte[] signersBitmap,
      final List<byte[]> signatures) {
    requireV1Version(version, "TRON witness seal version");
    Objects.requireNonNull(witnessAddresses, "witnessAddresses");
    Objects.requireNonNull(witnessWeights, "witnessWeights");
    Objects.requireNonNull(signersBitmap, "signersBitmap");
    Objects.requireNonNull(signatures, "signatures");
    final BigInteger totalWeightValue = normalizeU64(totalWeight, "totalWeight");
    final BigInteger signedWeightValue = normalizeU64(signedWeight, "signedWeight");
    if (BigInteger.ZERO.equals(totalWeightValue)) {
      throw new IllegalArgumentException("totalWeight must not be zero");
    }
    if (BigInteger.ZERO.equals(signedWeightValue)) {
      throw new IllegalArgumentException("signedWeight must not be zero");
    }
    final byte[] messageHash = nonZeroHex32Bytes(solidBlockMessageHash, "solidBlockMessageHash");
    if (witnessAddresses.isEmpty() || witnessAddresses.size() != witnessWeights.size()) {
      throw new IllegalArgumentException(
          "witnessAddresses and witnessWeights must be non-empty equal-length arrays");
    }
    if (witnessAddresses.size() > TRON_MAX_WITNESSES) {
      throw new IllegalArgumentException(
          "witnessAddresses must contain at most " + TRON_MAX_WITNESSES + " entries");
    }
    final List<byte[]> normalizedAddresses = new ArrayList<>();
    final List<BigInteger> normalizedWeights = new ArrayList<>();
    final Set<String> seenAddresses = new HashSet<>();
    BigInteger computedTotalWeight = BigInteger.ZERO;
    for (int i = 0; i < witnessAddresses.size(); i++) {
      final byte[] address = hexBytes(witnessAddresses.get(i), "witnessAddresses[" + i + "]", 21);
      if (!isNonZeroTronAddress(address)) {
        throw new IllegalArgumentException(
            "witnessAddresses[" + i + "] must be a TRON 0x41-prefixed address");
      }
      if (!seenAddresses.add(hexLower(address))) {
        throw new IllegalArgumentException("witnessAddresses[" + i + "] must be unique");
      }
      final BigInteger weight = normalizeU64(witnessWeights.get(i), "witnessWeights[" + i + "]");
      if (BigInteger.ZERO.equals(weight)) {
        throw new IllegalArgumentException("witnessWeights[" + i + "] must not be zero");
      }
      computedTotalWeight = computedTotalWeight.add(weight);
      if (computedTotalWeight.compareTo(MAX_U64) > 0) {
        throw new IllegalArgumentException("witnessWeights total must fit u64");
      }
      normalizedAddresses.add(address);
      normalizedWeights.add(weight);
    }
    if (!computedTotalWeight.equals(totalWeightValue)) {
      throw new IllegalArgumentException("totalWeight must equal the witness weight sum");
    }
    final List<Integer> signerIndices =
        tronWitnessSealSignerIndices(signersBitmap, normalizedAddresses.size());
    if (signatures.size() != signerIndices.size()) {
      throw new IllegalArgumentException("signatures length must match signersBitmap");
    }
    BigInteger computedSignedWeight = BigInteger.ZERO;
    final List<byte[]> normalizedSignatures = new ArrayList<>();
    for (int signatureIndex = 0; signatureIndex < signatures.size(); signatureIndex++) {
      final byte[] signature = Objects.requireNonNull(signatures.get(signatureIndex), "signature");
      if (!tronRecoverableSignatureIsCanonical(signature)) {
        throw new IllegalArgumentException(
            "signatures[" + signatureIndex + "] must be a canonical low-S 65-byte TRON signature");
      }
      final int witnessIndex = signerIndices.get(signatureIndex);
      final byte[] recoveredSigner = tronRecoveredSignerAddress20(messageHash, signature);
      final byte[] expectedAddress = normalizedAddresses.get(witnessIndex);
      if (recoveredSigner == null
          || !Arrays.equals(recoveredSigner, Arrays.copyOfRange(expectedAddress, 1, expectedAddress.length))) {
        throw new IllegalArgumentException(
            "witness seal signature does not recover to declared signer");
      }
      computedSignedWeight = computedSignedWeight.add(normalizedWeights.get(witnessIndex));
      normalizedSignatures.add(Arrays.copyOf(signature, signature.length));
    }
    if (!computedSignedWeight.equals(signedWeightValue)) {
      throw new IllegalArgumentException(
          "signedWeight must equal the signersBitmap witness weight sum");
    }
    if (computedSignedWeight.multiply(BigInteger.valueOf(3))
        .compareTo(computedTotalWeight.multiply(BigInteger.valueOf(2))) <= 0) {
      throw new IllegalArgumentException("signedWeight must exceed two thirds of totalWeight");
    }
    final byte[] witnessPayload =
        canonicalTronWitnessSchedulePayloadBytes(witnessAddresses, witnessWeights);
    return new NormalizedTronWitnessSealProof(
        version,
        totalWeightValue,
        signedWeightValue,
        messageHash,
        copyByteArrayList(normalizedAddresses),
        new ArrayList<BigInteger>(normalizedWeights),
        Arrays.copyOf(signersBitmap, signersBitmap.length),
        normalizedSignatures,
        hashBytes("sccp:tron:witness-schedule:v1", witnessPayload));
  }

  private static byte[] canonicalTronWitnessSealProofBytes(
      final NormalizedTronWitnessSealProof proof) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(proof.version);
    writeU64Le(out, proof.totalWeight);
    writeU64Le(out, proof.signedWeight);
    write(out, proof.solidBlockMessageHash);
    writeU32Le(out, proof.witnessAddresses.size());
    for (final byte[] address : proof.witnessAddresses) {
      writeVector(out, address);
    }
    writeU32Le(out, proof.witnessWeights.size());
    for (final BigInteger weight : proof.witnessWeights) {
      writeU64Le(out, weight);
    }
    writeVector(out, proof.signersBitmap);
    writeU32Le(out, proof.signatures.size());
    for (final byte[] signature : proof.signatures) {
      writeVector(out, signature);
    }
    return out.toByteArray();
  }

  private static NormalizedTronWitnessScheduleTransitionMessage
      normalizeTronWitnessScheduleTransitionMessage(
          final int version,
          final int sourceDomain,
          final String fromWitnessScheduleEpoch,
          final String toWitnessScheduleEpoch,
          final String transitionBlockNumber,
          final String transitionBlockHash,
          final String parentWitnessScheduleHash,
          final String nextWitnessScheduleHash,
          final String nextWitnessSchedulePayloadHash,
          final byte[] nextWitnessSchedulePayload) {
    requireV1Version(version, "TRON witness-schedule transition message version");
    final int normalizedSourceDomain = normalizeDomain(sourceDomain, "sourceDomain");
    if (normalizedSourceDomain != DOMAIN_TRON) {
      throw new IllegalArgumentException("sourceDomain must be TRON");
    }
    final BigInteger fromEpoch = normalizeU64(fromWitnessScheduleEpoch, "fromWitnessScheduleEpoch");
    final BigInteger toEpoch = normalizeU64(toWitnessScheduleEpoch, "toWitnessScheduleEpoch");
    if (!fromEpoch.add(BigInteger.ONE).equals(toEpoch)) {
      throw new IllegalArgumentException(
          "toWitnessScheduleEpoch must equal fromWitnessScheduleEpoch + 1");
    }
    final BigInteger blockNumber = normalizeU64(transitionBlockNumber, "transitionBlockNumber");
    if (BigInteger.ZERO.equals(blockNumber)) {
      throw new IllegalArgumentException("transitionBlockNumber must not be zero");
    }
    final byte[] transitionBlockHashBytes =
        nonZeroHex32Bytes(transitionBlockHash, "transitionBlockHash");
    final byte[] parentScheduleHash =
        nonZeroHex32Bytes(parentWitnessScheduleHash, "parentWitnessScheduleHash");
    final byte[] nextScheduleHash =
        nonZeroHex32Bytes(nextWitnessScheduleHash, "nextWitnessScheduleHash");
    final byte[] payloadHash;
    if (nextWitnessSchedulePayloadHash == null && nextWitnessSchedulePayload == null) {
      throw new IllegalArgumentException(
          "nextWitnessSchedulePayloadHash or nextWitnessSchedulePayload is required");
    } else if (nextWitnessSchedulePayloadHash == null) {
      payloadHash =
          hex32Bytes(
              tronWitnessSchedulePayloadHash(nextWitnessSchedulePayload),
              "nextWitnessSchedulePayloadHash");
    } else {
      payloadHash = nonZeroHex32Bytes(nextWitnessSchedulePayloadHash, "nextWitnessSchedulePayloadHash");
    }
    if (nextWitnessSchedulePayload != null) {
      final byte[] derivedPayloadHash =
          hex32Bytes(
              tronWitnessSchedulePayloadHash(nextWitnessSchedulePayload),
              "nextWitnessSchedulePayloadHash");
      if (!Arrays.equals(payloadHash, derivedPayloadHash)) {
        throw new IllegalArgumentException(
            "nextWitnessSchedulePayloadHash must match nextWitnessSchedulePayload");
      }
      final byte[] derivedScheduleHash =
          hex32Bytes(
              tronWitnessScheduleHashFromPayload(nextWitnessSchedulePayload),
              "nextWitnessScheduleHash");
      if (!Arrays.equals(nextScheduleHash, derivedScheduleHash)) {
        throw new IllegalArgumentException(
            "nextWitnessScheduleHash must match nextWitnessSchedulePayload");
      }
    }
    return new NormalizedTronWitnessScheduleTransitionMessage(
        version,
        normalizedSourceDomain,
        fromEpoch,
        toEpoch,
        blockNumber,
        transitionBlockHashBytes,
        parentScheduleHash,
        nextScheduleHash,
        payloadHash);
  }

  private static byte[] canonicalTronWitnessScheduleTransitionMessageBytes(
      final NormalizedTronWitnessScheduleTransitionMessage message) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(message.version);
    writeU32Le(out, message.sourceDomain);
    writeU64Le(out, message.fromWitnessScheduleEpoch);
    writeU64Le(out, message.toWitnessScheduleEpoch);
    writeU64Le(out, message.transitionBlockNumber);
    write(out, message.transitionBlockHash);
    write(out, message.parentWitnessScheduleHash);
    write(out, message.nextWitnessScheduleHash);
    write(out, message.nextWitnessSchedulePayloadHash);
    return out.toByteArray();
  }

  private static SourceAdapterVerifierProfile sourceAdapterVerifierProfile(
      final int sourceDomain) {
    switch (sourceDomain) {
      case DOMAIN_ETH:
        return new SourceAdapterVerifierProfile("eth", 1, 1);
      case DOMAIN_BSC:
        return new SourceAdapterVerifierProfile("bsc", 2, 2);
      case DOMAIN_SOL:
        return new SourceAdapterVerifierProfile("sol", 3, 3);
      case DOMAIN_TON:
        return new SourceAdapterVerifierProfile("ton", 4, 4);
      case DOMAIN_TRON:
        return new SourceAdapterVerifierProfile("tron", 5, 5);
      case DOMAIN_SORA_KUSAMA:
        return new SourceAdapterVerifierProfile("sora-kusama", 6, 6);
      case DOMAIN_SORA_POLKADOT:
        return new SourceAdapterVerifierProfile("sora-polkadot", 6, 6);
      case DOMAIN_SORA2:
        return new SourceAdapterVerifierProfile("sora2", 6, 6);
      default:
        throw new IllegalArgumentException(
            "sourceDomain is not a supported SCCP source-adapter lane");
    }
  }

  private static final class SourceAdapterVerifierProfile {
    private final String chain;
    private final int proofPlan;
    private final int finalityModel;

    private SourceAdapterVerifierProfile(
        final String chain, final int proofPlan, final int finalityModel) {
      this.chain = chain;
      this.proofPlan = proofPlan;
      this.finalityModel = finalityModel;
    }
  }

  private static DestinationBindingProfile destinationBindingProfile(final int targetDomain) {
    switch (targetDomain) {
      case DOMAIN_SOL:
        return new DestinationBindingProfile(
            2,
            2,
            "sccp:0:3:sol:solana-program-v1:2",
            "iroha:sccp:bridge-proof:message:stark-fri:v1:sol",
            "solana-program-v1");
      case DOMAIN_TON:
        return new DestinationBindingProfile(
            3,
            3,
            "sccp:0:4:ton:ton-contract-v1:3",
            "iroha:sccp:bridge-proof:message:stark-fri:v1:ton",
            "ton-contract-v1");
      case DOMAIN_SORA_KUSAMA:
        return new DestinationBindingProfile(
            5,
            5,
            "sccp:0:6:sora-kusama:substrate-runtime-v1:5",
            "iroha:sccp:bridge-proof:message:stark-fri:v1:sora-kusama",
            "substrate-runtime-v1");
      case DOMAIN_SORA_POLKADOT:
        return new DestinationBindingProfile(
            5,
            5,
            "sccp:0:7:sora-polkadot:substrate-runtime-v1:5",
            "iroha:sccp:bridge-proof:message:stark-fri:v1:sora-polkadot",
            "substrate-runtime-v1");
      case DOMAIN_SORA2:
        return new DestinationBindingProfile(
            5,
            5,
            "sccp:0:8:sora2:substrate-runtime-v1:5",
            "iroha:sccp:bridge-proof:message:stark-fri:v1:sora2",
            "substrate-runtime-v1");
      default:
        throw new IllegalArgumentException(
            "targetDomain is not a supported native SCCP destination lane");
    }
  }

  private static final class DestinationBindingProfile {
    private final int verifierTarget;
    private final int backendFamily;
    private final String bindingKey;
    private final String manifestSeed;
    private final String verifierBackend;

    private DestinationBindingProfile(
        final int verifierTarget,
        final int backendFamily,
        final String bindingKey,
        final String manifestSeed,
        final String verifierBackend) {
      this.verifierTarget = verifierTarget;
      this.backendFamily = backendFamily;
      this.bindingKey = bindingKey;
      this.manifestSeed = manifestSeed;
      this.verifierBackend = verifierBackend;
    }
  }

  private static final class SourceRecordProfile {
    private final String chain;
    private final int proofPlan;
    private final int finalityModel;
    private final String sourceTrustAnchorId;
    private final String consensusVerifierId;
    private final String messageInclusionVerifierId;
    private final String finalityPolicyId;
    private final String sourceStateVerifierId;
    private final String sourceBridgeEmitterId;
    private final boolean requiresSourceBridge;
    private final boolean requiresSourceBridgeConfig;

    private SourceRecordProfile(
        final String chain,
        final int proofPlan,
        final int finalityModel,
        final String sourceTrustAnchorId,
        final String consensusVerifierId,
        final String messageInclusionVerifierId,
        final String finalityPolicyId,
        final String sourceStateVerifierId,
        final String sourceBridgeEmitterId,
        final boolean requiresSourceBridge,
        final boolean requiresSourceBridgeConfig) {
      this.chain = chain;
      this.proofPlan = proofPlan;
      this.finalityModel = finalityModel;
      this.sourceTrustAnchorId = sourceTrustAnchorId;
      this.consensusVerifierId = consensusVerifierId;
      this.messageInclusionVerifierId = messageInclusionVerifierId;
      this.finalityPolicyId = finalityPolicyId;
      this.sourceStateVerifierId = sourceStateVerifierId;
      this.sourceBridgeEmitterId = sourceBridgeEmitterId;
      this.requiresSourceBridge = requiresSourceBridge;
      this.requiresSourceBridgeConfig = requiresSourceBridgeConfig;
    }
  }

  private static final class NormalizedSourceMaterial {
    private final int sourceDomain;
    private final SourceRecordProfile profile;
    private final byte[] sourceTrustAnchorHash;
    private final byte[] consensusVerifierHash;
    private final byte[] messageInclusionVerifierHash;
    private final byte[] finalityPolicyHash;
    private final byte[] sourceStateVerifierHash;
    private final byte[] sourceBridgeEmitterAddress;
    private final byte[] sourceBridgeEmitterCodeHash;
    private final byte[] sourceBridgeNetworkId;
    private final byte[] sourceBridgeOwnerAddress;
    private final byte[] sourceBridgeConfigHash;

    private NormalizedSourceMaterial(
        final int sourceDomain,
        final SourceRecordProfile profile,
        final byte[] sourceTrustAnchorHash,
        final byte[] consensusVerifierHash,
        final byte[] messageInclusionVerifierHash,
        final byte[] finalityPolicyHash,
        final byte[] sourceStateVerifierHash,
        final byte[] sourceBridgeEmitterAddress,
        final byte[] sourceBridgeEmitterCodeHash,
        final byte[] sourceBridgeNetworkId,
        final byte[] sourceBridgeOwnerAddress,
        final byte[] sourceBridgeConfigHash) {
      this.sourceDomain = sourceDomain;
      this.profile = profile;
      this.sourceTrustAnchorHash = sourceTrustAnchorHash;
      this.consensusVerifierHash = consensusVerifierHash;
      this.messageInclusionVerifierHash = messageInclusionVerifierHash;
      this.finalityPolicyHash = finalityPolicyHash;
      this.sourceStateVerifierHash = sourceStateVerifierHash;
      this.sourceBridgeEmitterAddress = sourceBridgeEmitterAddress;
      this.sourceBridgeEmitterCodeHash = sourceBridgeEmitterCodeHash;
      this.sourceBridgeNetworkId = sourceBridgeNetworkId;
      this.sourceBridgeOwnerAddress = sourceBridgeOwnerAddress;
      this.sourceBridgeConfigHash = sourceBridgeConfigHash;
    }
  }

  private static SourceRecordProfile sourceRecordProfile(final int sourceDomain) {
    final SourceAdapterVerifierProfile adapterProfile = sourceAdapterVerifierProfile(sourceDomain);
    switch (sourceDomain) {
      case DOMAIN_ETH:
        return new SourceRecordProfile(
            adapterProfile.chain,
            adapterProfile.proofPlan,
            adapterProfile.finalityModel,
            "sccp:eth:source-trust-anchor:ethereum-mainnet-beacon-finalized-checkpoint:v1",
            "sccp:eth:consensus-verifier:beacon-sync-committee-execution-header-mainnet:v1",
            "sccp:eth:message-inclusion-verifier:execution-receipt-trie-branch-mainnet:v1",
            "sccp:eth:finality-policy:beacon-finalized-checkpoint-mainnet:v1",
            "",
            "sccp:eth:source-bridge-emitter:ethereum-mainnet:v1",
            true,
            false);
      case DOMAIN_BSC:
        return new SourceRecordProfile(
            adapterProfile.chain,
            adapterProfile.proofPlan,
            adapterProfile.finalityModel,
            "sccp:bsc:source-trust-anchor:bsc-mainnet-validator-set:v1",
            "sccp:bsc:consensus-verifier:validator-set-seal-mainnet:v1",
            "sccp:bsc:message-inclusion-verifier:receipt-trie-branch-mainnet:v1",
            "sccp:bsc:finality-policy:validator-set-finality-mainnet:v1",
            "",
            "sccp:bsc:source-bridge-emitter:bsc-mainnet:v1",
            true,
            false);
      case DOMAIN_SOL:
        return new SourceRecordProfile(
            adapterProfile.chain,
            adapterProfile.proofPlan,
            adapterProfile.finalityModel,
            "sccp:sol:source-trust-anchor:solana-mainnet-beta-genesis:v1",
            "sccp:sol:consensus-verifier:finalized-slot-bankhash-mainnet-beta:v1",
            "sccp:sol:message-inclusion-verifier:transaction-status-root-branch:v1",
            "sccp:sol:finality-policy:finalized-slot-mainnet-beta:v1",
            "sccp:sol:accounts-db-verifier:accounts-lt-hash-mainnet-beta:v1",
            "",
            false,
            false);
      case DOMAIN_TON:
        return new SourceRecordProfile(
            adapterProfile.chain,
            adapterProfile.proofPlan,
            adapterProfile.finalityModel,
            "sccp:ton:source-trust-anchor:ton-mainnet-masterchain:v1",
            "sccp:ton:consensus-verifier:masterchain-block-proof:v1",
            "sccp:ton:message-inclusion-verifier:shard-transaction-branch:v1",
            "sccp:ton:finality-policy:masterchain-finality:v1",
            "sccp:ton:source-state-verifier:shard-state-light-client-mainnet:v1",
            "",
            false,
            false);
      case DOMAIN_TRON:
        return new SourceRecordProfile(
            adapterProfile.chain,
            adapterProfile.proofPlan,
            adapterProfile.finalityModel,
            "sccp:tron:source-trust-anchor:mainnet-witness-schedule:v1",
            "sccp:tron:consensus-verifier:dpos-solid-block-mainnet:v1",
            "sccp:tron:message-inclusion-verifier:transaction-source-mainnet:v1",
            "sccp:tron:finality-policy:solid-block-mainnet:v1",
            "",
            "sccp:tron:source-bridge-emitter:tron-mainnet:v1",
            true,
            true);
      case DOMAIN_SORA_KUSAMA:
        return new SourceRecordProfile(
            adapterProfile.chain,
            adapterProfile.proofPlan,
            adapterProfile.finalityModel,
            "sccp:sora-kusama:source-trust-anchor:grandpa-authority-set:v1",
            "sccp:sora-kusama:consensus-verifier:grandpa-finalized-header:v1",
            "sccp:sora-kusama:message-inclusion-verifier:events-storage-proof:v1",
            "sccp:sora-kusama:finality-policy:grandpa-finality:v1",
            "sccp:sora-kusama:source-state-verifier:runtime-storage-proof:v1",
            "",
            false,
            false);
      case DOMAIN_SORA_POLKADOT:
        return new SourceRecordProfile(
            adapterProfile.chain,
            adapterProfile.proofPlan,
            adapterProfile.finalityModel,
            "sccp:sora-polkadot:source-trust-anchor:grandpa-authority-set:v1",
            "sccp:sora-polkadot:consensus-verifier:grandpa-finalized-header:v1",
            "sccp:sora-polkadot:message-inclusion-verifier:events-storage-proof:v1",
            "sccp:sora-polkadot:finality-policy:grandpa-finality:v1",
            "sccp:sora-polkadot:source-state-verifier:runtime-storage-proof:v1",
            "",
            false,
            false);
      case DOMAIN_SORA2:
        return new SourceRecordProfile(
            adapterProfile.chain,
            adapterProfile.proofPlan,
            adapterProfile.finalityModel,
            "sccp:sora2:source-trust-anchor:grandpa-authority-set:v1",
            "sccp:sora2:consensus-verifier:grandpa-finalized-header:v1",
            "sccp:sora2:message-inclusion-verifier:events-storage-proof:v1",
            "sccp:sora2:finality-policy:grandpa-finality:v1",
            "sccp:sora2:source-state-verifier:runtime-storage-proof:v1",
            "",
            false,
            false);
      default:
        throw new IllegalArgumentException(
            "sourceDomain is not a supported SCCP source material lane");
    }
  }

  private static void rejectTonTemplateSourceMaterialComponent(
      final int sourceDomain, final byte[] value, final String label) {
    if (sourceDomain != DOMAIN_TON) {
      return;
    }
    final byte[] template;
    switch (label) {
      case "sourceTrustAnchorHash":
        template = TON_TEMPLATE_SOURCE_TRUST_ANCHOR_HASH;
        break;
      case "consensusVerifierHash":
        template = TON_TEMPLATE_CONSENSUS_VERIFIER_HASH;
        break;
      case "messageInclusionVerifierHash":
        template = TON_TEMPLATE_MESSAGE_INCLUSION_VERIFIER_HASH;
        break;
      case "finalityPolicyHash":
        template = TON_TEMPLATE_FINALITY_POLICY_HASH;
        break;
      case "sourceStateVerifierHash":
        template = TON_TEMPLATE_SOURCE_STATE_VERIFIER_HASH;
        break;
      default:
        return;
    }
    if (Arrays.equals(value, template)) {
      if ("sourceStateVerifierHash".equals(label)) {
        throw new IllegalArgumentException(
            "sourceStateVerifierHash must not be the TON template verifier hash");
      }
      throw new IllegalArgumentException(label + " must not be the TON template component hash");
    }
  }

  private static void rejectSolanaTemplateSourceMaterialComponent(
      final int sourceDomain, final byte[] value, final String label) {
    if (sourceDomain != DOMAIN_SOL) {
      return;
    }
    final byte[] template;
    switch (label) {
      case "sourceTrustAnchorHash":
        template = SOLANA_TEMPLATE_SOURCE_TRUST_ANCHOR_HASH;
        break;
      case "consensusVerifierHash":
        template = SOLANA_TEMPLATE_CONSENSUS_VERIFIER_HASH;
        break;
      case "messageInclusionVerifierHash":
        template = SOLANA_TEMPLATE_MESSAGE_INCLUSION_VERIFIER_HASH;
        break;
      case "finalityPolicyHash":
        template = SOLANA_TEMPLATE_FINALITY_POLICY_HASH;
        break;
      case "sourceStateVerifierHash":
        template = SOLANA_TEMPLATE_SOURCE_STATE_VERIFIER_HASH;
        break;
      default:
        return;
    }
    if (Arrays.equals(value, template)) {
      if ("sourceStateVerifierHash".equals(label)) {
        throw new IllegalArgumentException(
            "sourceStateVerifierHash must not be the Solana template verifier hash");
      }
      throw new IllegalArgumentException(label + " must not be the Solana template component hash");
    }
  }

  private static void rejectTronTemplateSourceMaterialComponent(
      final int sourceDomain, final byte[] value, final String label) {
    if (sourceDomain != DOMAIN_TRON) {
      return;
    }
    final byte[] template;
    switch (label) {
      case "sourceTrustAnchorHash":
        template = TRON_TEMPLATE_SOURCE_TRUST_ANCHOR_HASH;
        break;
      case "consensusVerifierHash":
        template = TRON_TEMPLATE_CONSENSUS_VERIFIER_HASH;
        break;
      case "messageInclusionVerifierHash":
        template = TRON_TEMPLATE_MESSAGE_INCLUSION_VERIFIER_HASH;
        break;
      case "finalityPolicyHash":
        template = TRON_TEMPLATE_FINALITY_POLICY_HASH;
        break;
      default:
        return;
    }
    if (Arrays.equals(value, template)) {
      throw new IllegalArgumentException(label + " must not be the TRON template component hash");
    }
  }

  private static byte[] abiWordAddress20(final byte[] value, final String label) {
    if (value.length != 20) {
      throw new IllegalArgumentException(label + " must be 20 bytes");
    }
    final byte[] out = new byte[32];
    System.arraycopy(value, 0, out, 12, 20);
    return out;
  }

  private static byte[] abiWordBytes21(final byte[] value, final String label) {
    if (value.length != 21) {
      throw new IllegalArgumentException(label + " must be 21 bytes");
    }
    final byte[] out = new byte[32];
    System.arraycopy(value, 0, out, 11, 21);
    return out;
  }

  static byte[] tronBase58CheckPayload(final String value, final String field) {
    final String text = Objects.requireNonNull(value, field);
    if (text.isEmpty()) {
      throw new IllegalArgumentException(field + " must not be empty");
    }
    if (!text.trim().equals(text)) {
      throw new IllegalArgumentException(field + " must be a canonical Base58Check address");
    }
    final byte[] decoded = base58Decode(text, field);
    if (decoded.length != 25) {
      throw new IllegalArgumentException(field + " must be a TRON Base58Check address");
    }
    final byte[] payload = Arrays.copyOfRange(decoded, 0, 21);
    final byte[] checksum = Arrays.copyOfRange(decoded, 21, 25);
    final byte[] expectedChecksum = Arrays.copyOfRange(sha256(sha256(payload)), 0, 4);
    if (!Arrays.equals(checksum, expectedChecksum) || !isNonZeroTronAddress(payload)) {
      throw new IllegalArgumentException(
          field + " must be a valid non-zero TRON Base58Check address");
    }
    return payload;
  }

  private static byte[] base58Decode(final String value, final String field) {
    final String text = Objects.requireNonNull(value, field);
    if (text.isEmpty()) {
      throw new IllegalArgumentException(field + " must not be empty");
    }
    final List<Integer> bytes = new ArrayList<>();
    for (int charIndex = 0; charIndex < text.length(); charIndex++) {
      final int digit = BASE58_ALPHABET.indexOf(text.charAt(charIndex));
      if (digit < 0) {
        throw new IllegalArgumentException(field + " must be Base58Check");
      }
      int carry = digit;
      for (int index = bytes.size() - 1; index >= 0; index--) {
        carry += bytes.get(index) * 58;
        bytes.set(index, carry & 0xff);
        carry >>>= 8;
      }
      while (carry > 0) {
        bytes.add(0, carry & 0xff);
        carry >>>= 8;
      }
    }
    int leadingZeroCount = 0;
    while (leadingZeroCount < text.length() && text.charAt(leadingZeroCount) == '1') {
      leadingZeroCount++;
    }
    final byte[] out = new byte[leadingZeroCount + bytes.size()];
    for (int index = 0; index < bytes.size(); index++) {
      out[leadingZeroCount + index] = (byte) (bytes.get(index) & 0xff);
    }
    return out;
  }

  private static byte[] tronSourceBridgeConfigHash(
      final int sourceDomain,
      final byte[] bridgeAddress,
      final byte[] networkId,
      final byte[] ownerAddress) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    write(out, keccak256(TRON_SOURCE_BRIDGE_CONFIG_LABEL));
    write(out, abiWordAddress20(bridgeAddress, "sourceBridgeEmitterAddress"));
    write(out, networkId);
    write(out, abiWordU32(sourceDomain, "sourceDomain"));
    write(out, abiWordU32(DOMAIN_SORA, "targetDomain"));
    write(out, abiWordAddress20(ownerAddress, "sourceBridgeOwnerAddress"));
    return keccak256(out.toByteArray());
  }

  private static NormalizedSourceMaterial normalizeSourceMaterial(
      final int sourceDomain,
      final String sourceTrustAnchorHash,
      final String consensusVerifierHash,
      final String messageInclusionVerifierHash,
      final String finalityPolicyHash,
      final String sourceStateVerifierHash,
      final String bridgeAddress,
      final String sourceBridgeEmitterCodeHash,
      final String networkId,
      final String ownerAddress,
      final String configHash) {
    final int normalizedSourceDomain = normalizeDomain(sourceDomain, "sourceDomain");
    final SourceRecordProfile profile = sourceRecordProfile(normalizedSourceDomain);
    final byte[] normalizedSourceStateVerifierHash =
        profile.sourceStateVerifierId.isEmpty()
            ? requireUnusedBytes(sourceStateVerifierHash, "sourceStateVerifierHash", 32)
            : nonZeroHex32Bytes(
                Objects.requireNonNull(
                    sourceStateVerifierHash, "sourceStateVerifierHash is required"),
                "sourceStateVerifierHash");
    rejectTonTemplateSourceMaterialComponent(
        normalizedSourceDomain, normalizedSourceStateVerifierHash, "sourceStateVerifierHash");
    rejectSolanaTemplateSourceMaterialComponent(
        normalizedSourceDomain, normalizedSourceStateVerifierHash, "sourceStateVerifierHash");
    final byte[] normalizedSourceTrustAnchorHash =
        nonZeroHex32Bytes(sourceTrustAnchorHash, "sourceTrustAnchorHash");
    rejectTonTemplateSourceMaterialComponent(
        normalizedSourceDomain, normalizedSourceTrustAnchorHash, "sourceTrustAnchorHash");
    rejectSolanaTemplateSourceMaterialComponent(
        normalizedSourceDomain, normalizedSourceTrustAnchorHash, "sourceTrustAnchorHash");
    rejectTronTemplateSourceMaterialComponent(
        normalizedSourceDomain, normalizedSourceTrustAnchorHash, "sourceTrustAnchorHash");
    final byte[] normalizedConsensusVerifierHash =
        nonZeroHex32Bytes(consensusVerifierHash, "consensusVerifierHash");
    rejectTonTemplateSourceMaterialComponent(
        normalizedSourceDomain, normalizedConsensusVerifierHash, "consensusVerifierHash");
    rejectSolanaTemplateSourceMaterialComponent(
        normalizedSourceDomain, normalizedConsensusVerifierHash, "consensusVerifierHash");
    rejectTronTemplateSourceMaterialComponent(
        normalizedSourceDomain, normalizedConsensusVerifierHash, "consensusVerifierHash");
    final byte[] normalizedMessageInclusionVerifierHash =
        nonZeroHex32Bytes(messageInclusionVerifierHash, "messageInclusionVerifierHash");
    rejectTonTemplateSourceMaterialComponent(
        normalizedSourceDomain,
        normalizedMessageInclusionVerifierHash,
        "messageInclusionVerifierHash");
    rejectSolanaTemplateSourceMaterialComponent(
        normalizedSourceDomain,
        normalizedMessageInclusionVerifierHash,
        "messageInclusionVerifierHash");
    rejectTronTemplateSourceMaterialComponent(
        normalizedSourceDomain,
        normalizedMessageInclusionVerifierHash,
        "messageInclusionVerifierHash");
    final byte[] normalizedFinalityPolicyHash =
        nonZeroHex32Bytes(finalityPolicyHash, "finalityPolicyHash");
    rejectTonTemplateSourceMaterialComponent(
        normalizedSourceDomain, normalizedFinalityPolicyHash, "finalityPolicyHash");
    rejectSolanaTemplateSourceMaterialComponent(
        normalizedSourceDomain, normalizedFinalityPolicyHash, "finalityPolicyHash");
    rejectTronTemplateSourceMaterialComponent(
        normalizedSourceDomain, normalizedFinalityPolicyHash, "finalityPolicyHash");
    final byte[] normalizedSourceBridgeEmitterAddress =
        profile.requiresSourceBridge
            ? nonZeroHexBytes(
                Objects.requireNonNull(bridgeAddress, "bridgeAddress is required"),
                "bridgeAddress",
                20)
            : requireUnusedBytes(bridgeAddress, "sourceBridgeEmitterAddress", 0);
    final byte[] normalizedSourceBridgeEmitterCodeHash =
        profile.requiresSourceBridge
            ? nonZeroHex32Bytes(
                Objects.requireNonNull(
                    sourceBridgeEmitterCodeHash, "sourceBridgeEmitterCodeHash is required"),
                "sourceBridgeEmitterCodeHash")
            : requireUnusedBytes(
                sourceBridgeEmitterCodeHash, "sourceBridgeEmitterCodeHash", 32);
    final byte[] normalizedSourceBridgeNetworkId =
        profile.requiresSourceBridgeConfig
            ? nonZeroHex32Bytes(
                Objects.requireNonNull(networkId, "networkId is required"), "networkId")
            : requireUnusedBytes(networkId, "sourceBridgeNetworkId", 32);
    final byte[] normalizedSourceBridgeOwnerAddress =
        profile.requiresSourceBridgeConfig
            ? nonZeroHexBytes(
                Objects.requireNonNull(ownerAddress, "ownerAddress is required"),
                "ownerAddress",
                20)
            : requireUnusedBytes(ownerAddress, "sourceBridgeOwnerAddress", 0);
    final byte[] normalizedSourceBridgeConfigHash =
        profile.requiresSourceBridgeConfig
            ? nonZeroHex32Bytes(
                Objects.requireNonNull(configHash, "configHash is required"), "configHash")
            : requireUnusedBytes(configHash, "sourceBridgeConfigHash", 32);
    if (normalizedSourceDomain == DOMAIN_TRON
        && !Arrays.equals(
            normalizedSourceBridgeConfigHash,
            tronSourceBridgeConfigHash(
                normalizedSourceDomain,
                normalizedSourceBridgeEmitterAddress,
                normalizedSourceBridgeNetworkId,
                normalizedSourceBridgeOwnerAddress))) {
      throw new IllegalArgumentException(
          "sourceBridgeConfigHash must match TRON source bridge config fields");
    }
    requirePairwiseNonzeroRoleHashSeparation(
        "SCCP source verifier material",
        new String[] {
          "sourceTrustAnchorHash",
          "consensusVerifierHash",
          "messageInclusionVerifierHash",
          "finalityPolicyHash",
          "sourceStateVerifierHash",
          "sourceBridgeEmitterCodeHash",
          "sourceBridgeNetworkId",
          "sourceBridgeConfigHash"
        },
        new byte[][] {
          normalizedSourceTrustAnchorHash,
          normalizedConsensusVerifierHash,
          normalizedMessageInclusionVerifierHash,
          normalizedFinalityPolicyHash,
          normalizedSourceStateVerifierHash,
          normalizedSourceBridgeEmitterCodeHash,
          normalizedSourceBridgeNetworkId,
          normalizedSourceBridgeConfigHash
        });
    return new NormalizedSourceMaterial(
        normalizedSourceDomain,
        profile,
        normalizedSourceTrustAnchorHash,
        normalizedConsensusVerifierHash,
        normalizedMessageInclusionVerifierHash,
        normalizedFinalityPolicyHash,
        normalizedSourceStateVerifierHash,
        normalizedSourceBridgeEmitterAddress,
        normalizedSourceBridgeEmitterCodeHash,
        normalizedSourceBridgeNetworkId,
        normalizedSourceBridgeOwnerAddress,
        normalizedSourceBridgeConfigHash);
  }

  private static void requirePairwiseNonzeroRoleHashSeparation(
      final String label, final String[] roleFields, final byte[][] roleHashes) {
    for (int i = 0; i < roleHashes.length; i++) {
      if (isZero(roleHashes[i])) {
        continue;
      }
      for (int j = i + 1; j < roleHashes.length; j++) {
        if (!isZero(roleHashes[j]) && Arrays.equals(roleHashes[i], roleHashes[j])) {
          throw new IllegalArgumentException(
              label + " hashes must be role-separated: " + roleFields[j] + " matches "
                  + roleFields[i]);
        }
      }
    }
  }

  private static void writeSourceMaterialFields(
      final ByteArrayOutputStream out, final NormalizedSourceMaterial material) {
    out.write(1);
    writeU32Le(out, material.sourceDomain);
    writeVector(out, material.profile.chain.getBytes(StandardCharsets.UTF_8));
    out.write(material.profile.proofPlan);
    out.write(material.profile.finalityModel);
    writeVector(out, SOURCE_ADAPTER_OPEN_VERIFY_CIRCUIT_ID_V1.getBytes(StandardCharsets.UTF_8));
    writeSourceComponentFields(out, material);
  }

  private static void writeSourceComponentFields(
      final ByteArrayOutputStream out, final NormalizedSourceMaterial material) {
    writeVector(out, material.profile.sourceTrustAnchorId.getBytes(StandardCharsets.UTF_8));
    write(out, material.sourceTrustAnchorHash);
    writeVector(out, material.profile.consensusVerifierId.getBytes(StandardCharsets.UTF_8));
    write(out, material.consensusVerifierHash);
    writeVector(out, material.profile.messageInclusionVerifierId.getBytes(StandardCharsets.UTF_8));
    write(out, material.messageInclusionVerifierHash);
    writeVector(out, material.profile.finalityPolicyId.getBytes(StandardCharsets.UTF_8));
    write(out, material.finalityPolicyHash);
    writeVector(out, material.profile.sourceStateVerifierId.getBytes(StandardCharsets.UTF_8));
    write(out, material.sourceStateVerifierHash);
    writeVector(out, material.profile.sourceBridgeEmitterId.getBytes(StandardCharsets.UTF_8));
    writeVector(out, material.sourceBridgeEmitterAddress);
    write(out, material.sourceBridgeEmitterCodeHash);
    write(out, material.sourceBridgeNetworkId);
    writeVector(out, material.sourceBridgeOwnerAddress);
    write(out, material.sourceBridgeConfigHash);
  }

  private static void writeSourceAdapterDeploymentSolanaAuditFields(
      final ByteArrayOutputStream out,
      final int sourceDomain,
      final String towerReplayVerifierHash,
      final String fullAccountsdbLatticeVerifierHash,
      final String bankForkChoiceVerifierHash,
      final byte[][] existingRoleHashes) {
    final String[] verifierIds = {
      SOLANA_MAINNET_TOWER_REPLAY_VERIFIER_ID_V1,
      SOLANA_MAINNET_FULL_ACCOUNTSDB_LATTICE_VERIFIER_ID_V1,
      SOLANA_MAINNET_BANK_FORK_CHOICE_VERIFIER_ID_V1
    };
    final byte[][] verifierHashes = {
      optionalHex32Bytes(towerReplayVerifierHash, "solanaTowerReplayVerifierHash"),
      optionalHex32Bytes(
          fullAccountsdbLatticeVerifierHash, "solanaFullAccountsdbLatticeVerifierHash"),
      optionalHex32Bytes(bankForkChoiceVerifierHash, "solanaBankForkChoiceVerifierHash")
    };
    int nonzeroCount = 0;
    for (final byte[] verifierHash : verifierHashes) {
      if (!isZero(verifierHash)) {
        nonzeroCount++;
      }
    }
    if (nonzeroCount == 0) {
      return;
    }
    if (sourceDomain != DOMAIN_SOL || nonzeroCount != verifierHashes.length) {
      throw new IllegalArgumentException(
          "Solana audit verifier hashes must be all non-zero and only used for Solana deployments");
    }
    requireSolanaFullLightClientAuditRoleSeparation(
        verifierIds, verifierHashes, existingRoleHashes);
    out.write(1);
    for (int i = 0; i < verifierHashes.length; i++) {
      writeVector(out, verifierIds[i].getBytes(StandardCharsets.UTF_8));
      write(out, verifierHashes[i]);
    }
  }

  private static void requireSolanaFullLightClientAuditRoleSeparation(
      final String[] verifierIds, final byte[][] verifierHashes, final byte[][] existingRoleHashes) {
    for (int i = 0; i < verifierHashes.length; i++) {
      for (int j = i + 1; j < verifierHashes.length; j++) {
        if (Arrays.equals(verifierHashes[i], verifierHashes[j])) {
          throw new IllegalArgumentException(
              "Solana full-light-client audit verifier hashes must be role-separated");
        }
      }
      final byte[][] templateHashes = {
        SOLANA_TEMPLATE_SOURCE_TRUST_ANCHOR_HASH,
        SOLANA_TEMPLATE_CONSENSUS_VERIFIER_HASH,
        SOLANA_TEMPLATE_MESSAGE_INCLUSION_VERIFIER_HASH,
        SOLANA_TEMPLATE_FINALITY_POLICY_HASH,
        SOLANA_TEMPLATE_SOURCE_STATE_VERIFIER_HASH
      };
      for (final byte[] templateHash : templateHashes) {
        if (Arrays.equals(verifierHashes[i], templateHash)) {
          throw new IllegalArgumentException(
              "Solana full-light-client audit verifier hash must not reuse built-in"
                  + " template material: "
                  + verifierIds[i]);
        }
      }
      for (final byte[] existingRoleHash : existingRoleHashes) {
        if (!isZero(existingRoleHash) && Arrays.equals(verifierHashes[i], existingRoleHash)) {
          throw new IllegalArgumentException(
              "Solana full-light-client audit verifier hash must not reuse existing"
                  + " source-adapter material: "
                  + verifierIds[i]);
        }
      }
    }
  }

  private static void writeSourceAdapterDeploymentTonAuditFields(
      final ByteArrayOutputStream out,
      final int sourceDomain,
      final String masterchainConfigVerifierHash,
      final String validatorSetTransitionVerifierHash,
      final String shardAccountsDictionaryVerifierHash,
      final byte[][] existingRoleHashes) {
    final String[] verifierIds = {
      TON_MAINNET_MASTERCHAIN_CONFIG_VERIFIER_ID_V1,
      TON_MAINNET_VALIDATOR_SET_TRANSITION_VERIFIER_ID_V1,
      TON_MAINNET_SHARD_ACCOUNTS_DICTIONARY_VERIFIER_ID_V1
    };
    final byte[][] verifierHashes = {
      optionalHex32Bytes(masterchainConfigVerifierHash, "tonMasterchainConfigVerifierHash"),
      optionalHex32Bytes(validatorSetTransitionVerifierHash, "tonValidatorSetTransitionVerifierHash"),
      optionalHex32Bytes(shardAccountsDictionaryVerifierHash, "tonShardAccountsDictionaryVerifierHash")
    };
    int nonzeroCount = 0;
    for (final byte[] verifierHash : verifierHashes) {
      if (!isZero(verifierHash)) {
        nonzeroCount++;
      }
    }
    if (nonzeroCount == 0) {
      return;
    }
    if (sourceDomain != DOMAIN_TON || nonzeroCount != verifierHashes.length) {
      throw new IllegalArgumentException(
          "TON audit verifier hashes must be all non-zero and only used for TON deployments");
    }
    requireTonFullLightClientAuditRoleSeparation(verifierIds, verifierHashes, existingRoleHashes);
    out.write(2);
    for (int i = 0; i < verifierHashes.length; i++) {
      writeVector(out, verifierIds[i].getBytes(StandardCharsets.UTF_8));
      write(out, verifierHashes[i]);
    }
  }

  private static void requireTonFullLightClientAuditRoleSeparation(
      final String[] verifierIds, final byte[][] verifierHashes, final byte[][] existingRoleHashes) {
    for (int i = 0; i < verifierHashes.length; i++) {
      for (int j = i + 1; j < verifierHashes.length; j++) {
        if (Arrays.equals(verifierHashes[i], verifierHashes[j])) {
          throw new IllegalArgumentException(
              "TON full-light-client audit verifier hashes must be role-separated");
        }
      }
      final byte[][] templateHashes = {
        TON_TEMPLATE_SOURCE_TRUST_ANCHOR_HASH,
        TON_TEMPLATE_CONSENSUS_VERIFIER_HASH,
        TON_TEMPLATE_MESSAGE_INCLUSION_VERIFIER_HASH,
        TON_TEMPLATE_FINALITY_POLICY_HASH,
        TON_TEMPLATE_SOURCE_STATE_VERIFIER_HASH
      };
      for (final byte[] templateHash : templateHashes) {
        if (Arrays.equals(verifierHashes[i], templateHash)) {
          throw new IllegalArgumentException(
              "TON full-light-client audit verifier hash must not reuse built-in"
                  + " template material: "
                  + verifierIds[i]);
        }
      }
      for (final byte[] existingRoleHash : existingRoleHashes) {
        if (!isZero(existingRoleHash) && Arrays.equals(verifierHashes[i], existingRoleHash)) {
          throw new IllegalArgumentException(
              "TON full-light-client audit verifier hash must not reuse existing"
                  + " source-adapter material: "
                  + verifierIds[i]);
        }
      }
    }
  }

  private static byte[] optionalHex32Bytes(final String value, final String field) {
    return value == null ? new byte[32] : hex32Bytes(value, field);
  }

  private static byte[] canonicalBscValidatorStorageProofBytes(
      final BscValidatorStorageProof proof) {
    Objects.requireNonNull(proof, "proof");
    requireV1Version(proof.version, "BSC validator storage proof version");
    validateMptProofNodes(proof.storageProofNodes, "storageProofNodes");
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(proof.version);
    writeU32Le(out, proof.validatorIndex);
    write(out, hex32Bytes(proof.storageSlot, "storageSlot"));
    final byte[] storageValueHash = hex32Bytes(proof.storageValueHash, "storageValueHash");
    if (!Arrays.equals(
        storageValueHash,
        hex32Bytes(bscValidatorSetStorageValueHash(proof.storageValue), "storageValueHash"))) {
      throw new IllegalArgumentException("storageValueHash must match storageValue");
    }
    writeVector(out, proof.storageValue);
    write(out, storageValueHash);
    writeU32Le(out, proof.storageProofNodes.size());
    for (final byte[] node : proof.storageProofNodes) {
      writeVector(out, Objects.requireNonNull(node, "storageProofNodes entry"));
    }
    return out.toByteArray();
  }

  private static final class TonValidatorSetParts {
    final List<byte[]> publicKeys;
    final List<BigInteger> weights;

    TonValidatorSetParts(final List<byte[]> publicKeys, final List<BigInteger> weights) {
      this.publicKeys = publicKeys;
      this.weights = weights;
    }
  }

  private static TonValidatorSetParts normalizeTonValidatorSet(
      final List<byte[]> validatorPublicKeys, final List<String> validatorWeights) {
    Objects.requireNonNull(validatorPublicKeys, "validatorPublicKeys");
    Objects.requireNonNull(validatorWeights, "validatorWeights");
    if (validatorPublicKeys.isEmpty() || validatorPublicKeys.size() != validatorWeights.size()) {
      throw new IllegalArgumentException(
          "validatorPublicKeys and validatorWeights must be non-empty equal-length arrays");
    }
    if (validatorPublicKeys.size() > TON_MAX_VALIDATORS) {
      throw new IllegalArgumentException(
          "validatorPublicKeys must contain at most " + TON_MAX_VALIDATORS + " entries");
    }
    final Set<String> seenPublicKeys = new HashSet<>();
    final List<byte[]> publicKeys = new ArrayList<byte[]>();
    final List<BigInteger> weights = new ArrayList<BigInteger>();
    for (int i = 0; i < validatorPublicKeys.size(); i++) {
      final byte[] publicKey =
          Objects.requireNonNull(validatorPublicKeys.get(i), "validatorPublicKeys[" + i + "]");
      if (publicKey.length != 32) {
        throw new IllegalArgumentException("validatorPublicKeys[" + i + "] must be 32 bytes");
      }
      if (isZero(publicKey)) {
        throw new IllegalArgumentException("validatorPublicKeys[" + i + "] must not be zero");
      }
      if (!seenPublicKeys.add(hexLower(publicKey))) {
        throw new IllegalArgumentException("validatorPublicKeys[" + i + "] must be unique");
      }
      final BigInteger weight =
          normalizeU64(validatorWeights.get(i), "validatorWeights[" + i + "]");
      if (BigInteger.ZERO.equals(weight)) {
        throw new IllegalArgumentException("validatorWeights[" + i + "] must not be zero");
      }
      publicKeys.add(publicKey);
      weights.add(weight);
    }
    return new TonValidatorSetParts(publicKeys, weights);
  }

  private static void validateTonValidatorSetPayload(final byte[] payload) {
    int cursor = 0;
    if (payload.length < 5 || payload[cursor] != 1) {
      throw new IllegalArgumentException("validatorSetPayload must have version 1");
    }
    cursor += 1;
    final int count = readU32Le(payload, cursor);
    cursor += 4;
    if (count <= 0 || count > TON_MAX_VALIDATORS || payload.length - cursor != count * 40) {
      throw new IllegalArgumentException("validatorSetPayload length is invalid");
    }
    final Set<String> seenPublicKeys = new HashSet<>();
    for (int i = 0; i < count; i++) {
      final byte[] publicKey = Arrays.copyOfRange(payload, cursor, cursor + 32);
      cursor += 32;
      if (isZero(publicKey)) {
        throw new IllegalArgumentException("validatorPublicKeys[" + i + "] must not be zero");
      }
      if (!seenPublicKeys.add(hexLower(publicKey))) {
        throw new IllegalArgumentException("validatorPublicKeys[" + i + "] must be unique");
      }
      final BigInteger weight = readU64Le(payload, cursor);
      cursor += 8;
      if (BigInteger.ZERO.equals(weight)) {
        throw new IllegalArgumentException("validatorWeights[" + i + "] must not be zero");
      }
    }
    if (cursor != payload.length) {
      throw new IllegalArgumentException("validatorSetPayload has trailing bytes");
    }
  }

  private static List<Integer> tonSignerIndicesFromBitmap(
      final byte[] bitmap, final int rosterLength) {
    Objects.requireNonNull(bitmap, "signersBitmap");
    if (bitmap.length != (rosterLength + 7) / 8) {
      throw new IllegalArgumentException("signersBitmap length must match validatorPublicKeys");
    }
    final ArrayList<Integer> indices = new ArrayList<Integer>();
    for (int byteIndex = 0; byteIndex < bitmap.length; byteIndex++) {
      final int value = bitmap[byteIndex] & 0xff;
      for (int bit = 0; bit < 8; bit++) {
        if (((value >>> bit) & 1) == 0) {
          continue;
        }
        final int index = byteIndex * 8 + bit;
        if (index >= rosterLength) {
          throw new IllegalArgumentException("signersBitmap must not set padding bits");
        }
        indices.add(index);
      }
    }
    return indices;
  }

  private static byte[] canonicalTonValidatorSignaturesProofBytes(
      final TonValidatorSignatureProof proof) {
    final TonValidatorSetParts normalized =
        normalizeTonValidatorSet(proof.validatorPublicKeys, proof.validatorWeights);
    requireV1Version(proof.version, "TON validator signature proof version");
    final BigInteger totalWeight = normalizeU64(proof.totalWeight, "totalWeight");
    final BigInteger signedWeight = normalizeU64(proof.signedWeight, "signedWeight");
    BigInteger computedTotalWeight = BigInteger.ZERO;
    for (final BigInteger weight : normalized.weights) {
      computedTotalWeight = computedTotalWeight.add(weight);
    }
    if (!totalWeight.equals(computedTotalWeight)) {
      throw new IllegalArgumentException("totalWeight must match validatorWeights");
    }
    final byte[] signersBitmap = Objects.requireNonNull(proof.signersBitmap, "signersBitmap");
    final List<Integer> signerIndices =
        tonSignerIndicesFromBitmap(signersBitmap, normalized.publicKeys.size());
    if (signerIndices.isEmpty()) {
      throw new IllegalArgumentException("signersBitmap must select at least one validator");
    }
    if (proof.signatures.size() != signerIndices.size()) {
      throw new IllegalArgumentException("signatures length must match signersBitmap");
    }
    BigInteger computedSignedWeight = BigInteger.ZERO;
    for (final int index : signerIndices) {
      computedSignedWeight = computedSignedWeight.add(normalized.weights.get(index));
    }
    if (!signedWeight.equals(computedSignedWeight)) {
      throw new IllegalArgumentException("signedWeight must match signersBitmap");
    }
    if (signedWeight
            .multiply(BigInteger.valueOf(3L))
            .compareTo(totalWeight.multiply(BigInteger.valueOf(2L)))
        <= 0) {
      throw new IllegalArgumentException("signedWeight must be greater than two thirds of totalWeight");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(proof.version);
    writeU64Le(out, totalWeight);
    writeU64Le(out, signedWeight);
    write(out, nonZeroHex32Bytes(proof.blockMessageHash, "blockMessageHash"));
    writeU32Le(out, normalized.publicKeys.size());
    for (final byte[] publicKey : normalized.publicKeys) {
      writeVector(out, publicKey);
    }
    writeU32Le(out, normalized.weights.size());
    for (final BigInteger weight : normalized.weights) {
      writeU64Le(out, weight);
    }
    writeVector(out, signersBitmap);
    writeU32Le(out, proof.signatures.size());
    for (int i = 0; i < proof.signatures.size(); i++) {
      final byte[] signature = Objects.requireNonNull(proof.signatures.get(i), "signatures[" + i + "]");
      if (signature.length != 64) {
        throw new IllegalArgumentException("signatures[" + i + "] must be 64 bytes");
      }
      if (isZero(signature)) {
        throw new IllegalArgumentException("signatures[" + i + "] must not be all zero");
      }
      writeVector(out, signature);
    }
    return out.toByteArray();
  }

  private static byte[] rlpLengthPrefix(
      final int length, final int shortOffset, final int longOffset) {
    if (length < 56) {
      return new byte[] {(byte) (shortOffset + length)};
    }
    int remaining = length;
    final ArrayList<Byte> lengthBytes = new ArrayList<Byte>();
    while (remaining > 0) {
      lengthBytes.add(0, (byte) (remaining & 0xff));
      remaining >>>= 8;
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(longOffset + lengthBytes.size());
    for (final Byte lengthByte : lengthBytes) {
      out.write(lengthByte.byteValue() & 0xff);
    }
    return out.toByteArray();
  }

  private static byte[] rlpBytes(final byte[] value) {
    if (value.length == 1 && (value[0] & 0xff) < 0x80) {
      return Arrays.copyOf(value, value.length);
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    write(out, rlpLengthPrefix(value.length, 0x80, 0xb7));
    write(out, value);
    return out.toByteArray();
  }

  private static byte[] rlpList(final List<byte[]> fields) {
    final ByteArrayOutputStream payload = new ByteArrayOutputStream();
    for (final byte[] field : fields) {
      write(payload, field);
    }
    final byte[] payloadBytes = payload.toByteArray();
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    write(out, rlpLengthPrefix(payloadBytes.length, 0xc0, 0xf7));
    write(out, payloadBytes);
    return out.toByteArray();
  }

  private static void validateTronMptProofNodes(final List<byte[]> nodes) {
    validateMptProofNodes(nodes, "receiptTrieProofNodes");
  }

  private static void validateMptProofNodes(final List<byte[]> nodes, final String field) {
    Objects.requireNonNull(nodes, field);
    if (nodes.isEmpty() || nodes.size() > TRON_MAX_MPT_PROOF_NODES) {
      throw new IllegalArgumentException(
          field + " must contain 1.." + TRON_MAX_MPT_PROOF_NODES + " entries");
    }
    for (int i = 0; i < nodes.size(); i++) {
      final byte[] node = Objects.requireNonNull(nodes.get(i), field + "[" + i + "]");
      if (node.length == 0 || node.length > TRON_MAX_MPT_NODE_BYTES) {
        throw new IllegalArgumentException(
            field + "[" + i + "] must contain 1.." + TRON_MAX_MPT_NODE_BYTES + " bytes");
      }
    }
  }

  private static void validateTronTransactionMerkleBranch(final List<byte[]> branch) {
    Objects.requireNonNull(branch, "transactionMerkleBranch");
    if (branch.size() > TRON_MAX_TRANSACTION_MERKLE_BRANCH_NODES) {
      throw new IllegalArgumentException(
          "transactionMerkleBranch must contain at most "
              + TRON_MAX_TRANSACTION_MERKLE_BRANCH_NODES
              + " entries");
    }
    for (int i = 0; i < branch.size(); i++) {
      final byte[] sibling = Objects.requireNonNull(branch.get(i), "transactionMerkleBranch[" + i + "]");
      if (sibling.length != 32) {
        throw new IllegalArgumentException("transactionMerkleBranch[" + i + "] must be 32 bytes");
      }
    }
  }

  private static byte[] tronTransactionMerkleRootFromBranch(
      final byte[] transactionBytes,
      final BigInteger transactionIndex,
      final BigInteger transactionCount,
      final List<byte[]> transactionMerkleBranch) {
    byte[] current = sha256(transactionBytes);
    BigInteger index = transactionIndex;
    BigInteger count = transactionCount;
    int branchCursor = 0;
    while (count.compareTo(BigInteger.ONE) > 0) {
      if (!index.testBit(0)) {
        if (index.add(BigInteger.ONE).compareTo(count) < 0) {
          if (branchCursor >= transactionMerkleBranch.size()) {
            throw new IllegalArgumentException(
                "transactionMerkleBranch is too short for transactionIndex/count");
          }
          current = sszHashNode(current, transactionMerkleBranch.get(branchCursor));
          branchCursor += 1;
        }
      } else {
        if (branchCursor >= transactionMerkleBranch.size()) {
          throw new IllegalArgumentException(
              "transactionMerkleBranch is too short for transactionIndex/count");
        }
        current = sszHashNode(transactionMerkleBranch.get(branchCursor), current);
        branchCursor += 1;
      }
      index = index.shiftRight(1);
      count = count.add(BigInteger.ONE).divide(BigInteger.valueOf(2));
    }
    if (branchCursor != transactionMerkleBranch.size()) {
      throw new IllegalArgumentException(
          "transactionMerkleBranch has unused siblings for transactionIndex/count");
    }
    return current;
  }

  private static IllegalArgumentException tronSourceTransactionError() {
    return new IllegalArgumentException(
        "transactionBytes must be a successful TRON TriggerSmartContract source call");
  }

  private static byte[] readProtobufBytesField(
      final byte[] bytes, final int[] cursor, final String label) {
    final int length;
    try {
      length = readCanonicalProtobufVarint(bytes, cursor, label).intValueExact();
    } catch (final ArithmeticException error) {
      throw new IllegalArgumentException(label + " protobuf length is too large", error);
    }
    final int end = cursor[0] + length;
    if (length < 0 || end > bytes.length) {
      throw new IllegalArgumentException(label + " contains truncated protobuf bytes field");
    }
    final byte[] value = Arrays.copyOfRange(bytes, cursor[0], end);
    cursor[0] = end;
    return value;
  }

  private static int protobufFieldNumber(final BigInteger key, final String label) {
    try {
      return key.shiftRight(3).intValueExact();
    } catch (final ArithmeticException error) {
      throw new IllegalArgumentException(label + " protobuf field number is too large", error);
    }
  }

  private static boolean tronTransactionResultSuccess(final byte[] result) {
    final int[] cursor = {0};
    boolean feeSeen = false;
    boolean retSeen = false;
    boolean contractRetSeen = false;
    while (cursor[0] < result.length) {
      final BigInteger key = readCanonicalProtobufVarint(result, cursor, "transactionResult");
      final int fieldNumber = protobufFieldNumber(key, "transactionResult");
      final int wireType = key.and(BigInteger.valueOf(0x07L)).intValue();
      if (fieldNumber == 1 && wireType == 0 && !feeSeen) {
        feeSeen = true;
        readCanonicalProtobufVarint(result, cursor, "transactionResult");
      } else if (fieldNumber == 2 && wireType == 0 && !retSeen) {
        retSeen = true;
        if (readCanonicalProtobufVarint(result, cursor, "transactionResult").signum() != 0) {
          return false;
        }
      } else if (fieldNumber == 3 && wireType == 0 && !contractRetSeen) {
        contractRetSeen = true;
        if (!BigInteger.ONE.equals(
            readCanonicalProtobufVarint(result, cursor, "transactionResult"))) {
          return false;
        }
      } else {
        return false;
      }
    }
    return contractRetSeen;
  }

  private static byte[] readTronProtobufAnyValue(final byte[] parameter) {
    final int[] cursor = {0};
    byte[] typeUrl = null;
    byte[] value = null;
    while (cursor[0] < parameter.length) {
      final BigInteger key = readCanonicalProtobufVarint(parameter, cursor, "triggerParameter");
      final int fieldNumber = protobufFieldNumber(key, "triggerParameter");
      final int wireType = key.and(BigInteger.valueOf(0x07L)).intValue();
      if (fieldNumber == 1 && wireType == 2 && typeUrl == null) {
        typeUrl = readProtobufBytesField(parameter, cursor, "triggerParameter");
      } else if (fieldNumber == 2 && wireType == 2 && value == null) {
        value = readProtobufBytesField(parameter, cursor, "triggerParameter");
      } else {
        return null;
      }
    }
    return Arrays.equals(typeUrl, TRON_TRIGGER_SMART_CONTRACT_TYPE_URL) ? value : null;
  }

  private static byte[] tronTriggerSourceCallOwnerAddress(
      final byte[] trigger,
      final byte[] sourceEventDigest,
      final byte[] expectedContractAddress,
      final byte[] expectedOwnerAddress) {
    final int[] cursor = {0};
    byte[] ownerAddress = null;
    byte[] contractAddress = null;
    byte[] data = null;
    boolean callValueSeen = false;
    boolean callTokenValueSeen = false;
    boolean tokenIdSeen = false;
    while (cursor[0] < trigger.length) {
      final BigInteger key = readCanonicalProtobufVarint(trigger, cursor, "triggerContract");
      final int fieldNumber = protobufFieldNumber(key, "triggerContract");
      final int wireType = key.and(BigInteger.valueOf(0x07L)).intValue();
      if (fieldNumber == 1 && wireType == 2 && ownerAddress == null) {
        ownerAddress = readProtobufBytesField(trigger, cursor, "triggerContract");
      } else if (fieldNumber == 2 && wireType == 2 && contractAddress == null) {
        contractAddress = readProtobufBytesField(trigger, cursor, "triggerContract");
      } else if (fieldNumber == 3 && wireType == 0 && !callValueSeen) {
        callValueSeen = true;
        if (readCanonicalProtobufVarint(trigger, cursor, "triggerContract").signum() != 0) {
          return null;
        }
      } else if (fieldNumber == 4 && wireType == 2 && data == null) {
        data = readProtobufBytesField(trigger, cursor, "triggerContract");
      } else if (fieldNumber == 5 && wireType == 0 && !callTokenValueSeen) {
        callTokenValueSeen = true;
        if (readCanonicalProtobufVarint(trigger, cursor, "triggerContract").signum() != 0) {
          return null;
        }
      } else if (fieldNumber == 6 && wireType == 0 && !tokenIdSeen) {
        tokenIdSeen = true;
        if (readCanonicalProtobufVarint(trigger, cursor, "triggerContract").signum() != 0) {
          return null;
        }
      } else {
        return null;
      }
    }
    final ByteArrayOutputStream expected = new ByteArrayOutputStream();
    write(expected, Arrays.copyOfRange(keccak256(TRON_SOURCE_MESSAGE_CALL_ABI), 0, 4));
    write(expected, abiWordU32(DOMAIN_TRON, "sourceDomain"));
    write(expected, abiWordU32(DOMAIN_SORA, "targetDomain"));
    write(expected, sourceEventDigest);
    if (ownerAddress == null
        || contractAddress == null
        || !isNonZeroTronAddress(ownerAddress)
        || !isNonZeroTronAddress(contractAddress)) {
      return null;
    }
    final byte[] ownerAddress20 = Arrays.copyOfRange(ownerAddress, 1, ownerAddress.length);
    final byte[] contractAddress20 = Arrays.copyOfRange(contractAddress, 1, contractAddress.length);
    if ((expectedContractAddress != null
            && !Arrays.equals(contractAddress20, expectedContractAddress))
        || (expectedOwnerAddress != null && !Arrays.equals(ownerAddress20, expectedOwnerAddress))
        || !Arrays.equals(data, expected.toByteArray())) {
      return null;
    }
    return ownerAddress20;
  }

  private static byte[] tronContractSourceCallOwnerAddress(
      final byte[] contract,
      final byte[] sourceEventDigest,
      final byte[] expectedContractAddress,
      final byte[] expectedOwnerAddress) {
    final int[] cursor = {0};
    BigInteger contractType = null;
    byte[] parameter = null;
    while (cursor[0] < contract.length) {
      final BigInteger key = readCanonicalProtobufVarint(contract, cursor, "transactionContract");
      final int fieldNumber = protobufFieldNumber(key, "transactionContract");
      final int wireType = key.and(BigInteger.valueOf(0x07L)).intValue();
      if (fieldNumber == 1 && wireType == 0 && contractType == null) {
        contractType = readCanonicalProtobufVarint(contract, cursor, "transactionContract");
      } else if (fieldNumber == 2 && wireType == 2 && parameter == null) {
        parameter = readProtobufBytesField(contract, cursor, "transactionContract");
      } else {
        return null;
      }
    }
    final byte[] trigger = parameter == null ? null : readTronProtobufAnyValue(parameter);
    return BigInteger.valueOf(31L).equals(contractType) && trigger != null
        ? tronTriggerSourceCallOwnerAddress(
            trigger, sourceEventDigest, expectedContractAddress, expectedOwnerAddress)
        : null;
  }

  private static byte[] tronRawDataSourceCallOwnerAddress(
      final byte[] rawData,
      final byte[] sourceEventDigest,
      final byte[] expectedContractAddress,
      final byte[] expectedOwnerAddress) {
    final int[] cursor = {0};
    boolean refBlockBytesSeen = false;
    boolean refBlockNumSeen = false;
    boolean refBlockHashSeen = false;
    BigInteger expirationMs = null;
    BigInteger timestampMs = null;
    boolean feeLimitSeen = false;
    int contractCount = 0;
    byte[] matchedContract = null;
    while (cursor[0] < rawData.length) {
      final BigInteger key = readCanonicalProtobufVarint(rawData, cursor, "rawData");
      final int fieldNumber = protobufFieldNumber(key, "rawData");
      final int wireType = key.and(BigInteger.valueOf(0x07L)).intValue();
      if (fieldNumber == 1 && wireType == 2 && !refBlockBytesSeen) {
        refBlockBytesSeen = true;
        final byte[] value = readProtobufBytesField(rawData, cursor, "rawData");
        if (value.length != 2 || isZero(value)) {
          return null;
        }
      } else if (fieldNumber == 3 && wireType == 0 && !refBlockNumSeen) {
        refBlockNumSeen = true;
        readCanonicalProtobufVarint(rawData, cursor, "rawData");
      } else if (fieldNumber == 4 && wireType == 2 && !refBlockHashSeen) {
        refBlockHashSeen = true;
        final byte[] value = readProtobufBytesField(rawData, cursor, "rawData");
        if (value.length != 8 || isZero(value)) {
          return null;
        }
      } else if (fieldNumber == 8 && wireType == 0 && expirationMs == null) {
        expirationMs = readCanonicalProtobufVarint(rawData, cursor, "rawData");
        if (expirationMs.signum() == 0) {
          return null;
        }
      } else if (fieldNumber == 11 && wireType == 2) {
        contractCount += 1;
        if (contractCount > 1) {
          return null;
        }
        matchedContract =
            tronContractSourceCallOwnerAddress(
                readProtobufBytesField(rawData, cursor, "rawData"),
                sourceEventDigest,
                expectedContractAddress,
                expectedOwnerAddress);
      } else if (fieldNumber == 14 && wireType == 0 && timestampMs == null) {
        timestampMs = readCanonicalProtobufVarint(rawData, cursor, "rawData");
        if (timestampMs.signum() == 0) {
          return null;
        }
      } else if (fieldNumber == 18 && wireType == 0 && !feeLimitSeen) {
        feeLimitSeen = true;
        if (readCanonicalProtobufVarint(rawData, cursor, "rawData").signum() == 0) {
          return null;
        }
      } else {
        return null;
      }
    }
    return refBlockBytesSeen
        && refBlockHashSeen
        && expirationMs != null
        && timestampMs != null
        && expirationMs.compareTo(timestampMs) > 0
        && feeLimitSeen
        && contractCount == 1
        ? matchedContract
        : null;
  }

  private static void validateTronTransactionSourceCall(
      final byte[] transactionBytes,
      final byte[] sourceEventDigest,
      final byte[] expectedContractAddress,
      final byte[] expectedOwnerAddress) {
    final int[] cursor = {0};
    byte[] rawData = null;
    final List<byte[]> signatures = new ArrayList<>();
    int resultCount = 0;
    boolean resultSuccess = false;
    while (cursor[0] < transactionBytes.length) {
      final BigInteger key = readCanonicalProtobufVarint(transactionBytes, cursor, "transactionBytes");
      final int fieldNumber = protobufFieldNumber(key, "transactionBytes");
      final int wireType = key.and(BigInteger.valueOf(0x07L)).intValue();
      if (fieldNumber == 1 && wireType == 2 && rawData == null) {
        rawData = readProtobufBytesField(transactionBytes, cursor, "transactionBytes");
      } else if (fieldNumber == 2 && wireType == 2) {
        if (signatures.size() >= TRON_SOURCE_CALL_SIGNATURES) {
          throw tronSourceTransactionError();
        }
        final byte[] signature = readProtobufBytesField(transactionBytes, cursor, "transactionBytes");
        if (!tronRecoverableSignatureIsCanonical(signature)) {
          throw tronSourceTransactionError();
        }
        signatures.add(signature);
      } else if (fieldNumber == 5 && wireType == 2) {
        if (resultCount >= 1) {
          throw tronSourceTransactionError();
        }
        resultSuccess =
            tronTransactionResultSuccess(
                readProtobufBytesField(transactionBytes, cursor, "transactionBytes"));
        resultCount += 1;
      } else {
        throw tronSourceTransactionError();
      }
    }
    if (rawData == null || signatures.size() != TRON_SOURCE_CALL_SIGNATURES) {
      throw tronSourceTransactionError();
    }
    final byte[] ownerAddress =
        tronRawDataSourceCallOwnerAddress(
            rawData, sourceEventDigest, expectedContractAddress, expectedOwnerAddress);
    final byte[] recoveredSigner = tronRecoveredSignerAddress20(sha256(rawData), signatures.get(0));
    if (resultCount != 1
        || !resultSuccess
        || ownerAddress == null
        || recoveredSigner == null
        || !Arrays.equals(recoveredSigner, ownerAddress)) {
      throw tronSourceTransactionError();
    }
  }

  private static void writeProtobufVarint(final ByteArrayOutputStream out, final BigInteger value) {
    BigInteger working = value;
    if (working.compareTo(BigInteger.ZERO) < 0 || working.compareTo(MAX_U64) > 0) {
      throw new IllegalArgumentException("protobuf varint value must fit u64");
    }
    while (working.compareTo(BigInteger.valueOf(0x80L)) >= 0) {
      out.write(working.and(BigInteger.valueOf(0x7fL)).or(BigInteger.valueOf(0x80L)).intValue());
      working = working.shiftRight(7);
    }
    out.write(working.intValue());
  }

  private static void writeProtobufU64(
      final ByteArrayOutputStream out, final int fieldNumber, final BigInteger value) {
    if (fieldNumber <= 0) {
      throw new IllegalArgumentException("protobuf field number must be positive");
    }
    writeProtobufVarint(out, BigInteger.valueOf(((long) fieldNumber) << 3));
    writeProtobufVarint(out, value);
  }

  private static void writeProtobufBytes(
      final ByteArrayOutputStream out, final int fieldNumber, final byte[] value) {
    if (fieldNumber <= 0) {
      throw new IllegalArgumentException("protobuf field number must be positive");
    }
    writeProtobufVarint(out, BigInteger.valueOf((((long) fieldNumber) << 3) | 2L));
    writeProtobufVarint(out, BigInteger.valueOf(value.length));
    write(out, value);
  }

  private static final class TronRawBlockHeaderFields {
    private final BigInteger number;
    private final byte[] txTrieRoot;
    private final byte[] accountStateRoot;
    private final byte[] parentBlockId;
    private final byte[] witnessAddress;
    private final int headerVersion;
    private final BigInteger timestampMs;

    private TronRawBlockHeaderFields(
        final BigInteger number,
        final byte[] txTrieRoot,
        final byte[] accountStateRoot,
        final byte[] parentBlockId,
        final byte[] witnessAddress,
        final int headerVersion,
        final BigInteger timestampMs) {
      this.number = number;
      this.txTrieRoot = txTrieRoot;
      this.accountStateRoot = accountStateRoot;
      this.parentBlockId = parentBlockId;
      this.witnessAddress = witnessAddress;
      this.headerVersion = headerVersion;
      this.timestampMs = timestampMs;
    }
  }

  private static int protobufVarintLength(BigInteger value) {
    int length = 1;
    while (value.compareTo(BigInteger.valueOf(0x80L)) >= 0) {
      length += 1;
      value = value.shiftRight(7);
    }
    return length;
  }

  private static BigInteger readCanonicalProtobufVarint(
      final byte[] bytes, final int[] cursor, final String label) {
    final int start = cursor[0];
    BigInteger value = BigInteger.ZERO;
    int shift = 0;
    for (int index = 0; index < 10; index++) {
      if (cursor[0] >= bytes.length) {
        throw new IllegalArgumentException(label + " contains truncated protobuf varint");
      }
      final int current = bytes[cursor[0]] & 0xff;
      cursor[0] += 1;
      final int chunk = current & 0x7f;
      if (index == 9 && chunk > 1) {
        throw new IllegalArgumentException(label + " protobuf varint must fit u64");
      }
      value = value.or(BigInteger.valueOf(chunk).shiftLeft(shift));
      if ((current & 0x80) == 0) {
        if (cursor[0] - start != protobufVarintLength(value)) {
          throw new IllegalArgumentException(label + " protobuf varint must be canonical");
        }
        return value;
      }
      shift += 7;
    }
    throw new IllegalArgumentException(label + " protobuf varint must fit u64");
  }

  private static byte[] readTronRawBlockHeaderBytesField(
      final byte[] rawData, final int[] cursor, final String label, final int byteLength) {
    final BigInteger lengthValue = readCanonicalProtobufVarint(rawData, cursor, label);
    final int length;
    try {
      length = lengthValue.intValueExact();
    } catch (final ArithmeticException error) {
      throw new IllegalArgumentException(label + " protobuf length is too large", error);
    }
    final int end = cursor[0] + length;
    if (length != byteLength || end > rawData.length) {
      throw new IllegalArgumentException(label + " bytes field must be " + byteLength + " bytes");
    }
    final byte[] value = Arrays.copyOfRange(rawData, cursor[0], end);
    cursor[0] = end;
    return value;
  }

  private static TronRawBlockHeaderFields decodeTronRawBlockHeaderFields(
      final byte[] rawData, final String label) {
    final int[] cursor = {0};
    BigInteger number = null;
    byte[] txTrieRoot = null;
    byte[] accountStateRoot = null;
    byte[] parentBlockId = null;
    boolean witnessIdSeen = false;
    byte[] witnessAddress = null;
    Integer headerVersion = null;
    BigInteger timestampMs = null;

    while (cursor[0] < rawData.length) {
      final BigInteger key = readCanonicalProtobufVarint(rawData, cursor, label);
      final int fieldNumber;
      try {
        fieldNumber = key.shiftRight(3).intValueExact();
      } catch (final ArithmeticException error) {
        throw new IllegalArgumentException(label + " protobuf field number is too large", error);
      }
      final int wireType = key.and(BigInteger.valueOf(0x07L)).intValue();
      switch (fieldNumber) {
        case 1:
          if (wireType != 0 || timestampMs != null) {
            throw new IllegalArgumentException(label + " must contain one canonical timestamp field");
          }
          timestampMs = readCanonicalProtobufVarint(rawData, cursor, label);
          break;
        case 2:
          if (wireType != 2 || txTrieRoot != null) {
            throw new IllegalArgumentException(label + " must contain one canonical txTrieRoot field");
          }
          txTrieRoot = readTronRawBlockHeaderBytesField(rawData, cursor, "txTrieRoot", 32);
          break;
        case 3:
          if (wireType != 2 || parentBlockId != null) {
            throw new IllegalArgumentException(label + " must contain one canonical parentBlockId field");
          }
          parentBlockId = readTronRawBlockHeaderBytesField(rawData, cursor, "parentBlockId", 32);
          break;
        case 7:
          if (wireType != 0 || number != null) {
            throw new IllegalArgumentException(label + " must contain one canonical number field");
          }
          number = readCanonicalProtobufVarint(rawData, cursor, label);
          break;
        case 8:
          if (wireType != 0 || witnessIdSeen) {
            throw new IllegalArgumentException(label + " must contain at most one canonical witnessId field");
          }
          witnessIdSeen = true;
          readCanonicalProtobufVarint(rawData, cursor, label);
          break;
        case 9:
          if (wireType != 2 || witnessAddress != null) {
            throw new IllegalArgumentException(label + " must contain one canonical witnessAddress field");
          }
          witnessAddress = readTronRawBlockHeaderBytesField(rawData, cursor, "witnessAddress", 21);
          break;
        case 10:
          if (wireType != 0 || headerVersion != null) {
            throw new IllegalArgumentException(label + " must contain one canonical headerVersion field");
          }
          final BigInteger parsedHeaderVersion = readCanonicalProtobufVarint(rawData, cursor, label);
          if (parsedHeaderVersion.compareTo(BigInteger.valueOf(0xffff_ffffL)) > 0) {
            throw new IllegalArgumentException("headerVersion must be a non-zero u32");
          }
          headerVersion = parsedHeaderVersion.intValue();
          break;
        case 11:
          if (wireType != 2 || accountStateRoot != null) {
            throw new IllegalArgumentException(label + " must contain one canonical accountStateRoot field");
          }
          accountStateRoot = readTronRawBlockHeaderBytesField(rawData, cursor, "accountStateRoot", 32);
          break;
        default:
          throw new IllegalArgumentException(label + " contains an unsupported protobuf field");
      }
    }

    if (number == null
        || timestampMs == null
        || headerVersion == null
        || txTrieRoot == null
        || accountStateRoot == null
        || parentBlockId == null
        || witnessAddress == null
        || BigInteger.ZERO.equals(number)
        || BigInteger.ZERO.equals(timestampMs)
        || headerVersion == 0
        || isZero(txTrieRoot)
        || isZero(accountStateRoot)
        || isZero(parentBlockId)
        || !isNonZeroTronAddress(witnessAddress)) {
      throw new IllegalArgumentException(label + " must be a canonical TRON raw block header");
    }
    return new TronRawBlockHeaderFields(
        number,
        txTrieRoot,
        accountStateRoot,
        parentBlockId,
        witnessAddress,
        headerVersion,
        timestampMs);
  }

  private static byte[] tronBlockIdBytesFromRawDataHash(
      final BigInteger number, final byte[] rawDataHash) {
    final byte[] blockId = Arrays.copyOf(rawDataHash, rawDataHash.length);
    final ByteArrayOutputStream numberBytes = new ByteArrayOutputStream();
    writeU64Be(numberBytes, number);
    final byte[] encodedNumber = numberBytes.toByteArray();
    System.arraycopy(encodedNumber, 0, blockId, 0, encodedNumber.length);
    return blockId;
  }

  private static void validateBscValidatorSetPayload(final byte[] payload) {
    Objects.requireNonNull(payload, "payload");
    if (payload.length > BSC_MAX_VALIDATOR_SET_PAYLOAD_BYTES) {
      throw new IllegalArgumentException(
          "validatorSetPayload must be at most "
              + BSC_MAX_VALIDATOR_SET_PAYLOAD_BYTES
              + " bytes");
    }
    int cursor = 0;
    if (payload.length == 0 || payload[cursor] != 1) {
      throw new IllegalArgumentException("validatorSetPayload must have version 1");
    }
    cursor += 1;
    final int count = readU32Le(payload, cursor);
    cursor += 4;
    if (count <= 0
        || count > BSC_MAX_PARLIA_VALIDATORS
        || payload.length - cursor != count * (BSC_PARLIA_VALIDATOR_ADDRESS_BYTES + 8)) {
      throw new IllegalArgumentException("validatorSetPayload has an invalid validator count");
    }
    final Set<String> seenAddresses = new HashSet<>();
    for (int i = 0; i < count; i++) {
      final byte[] address =
          Arrays.copyOfRange(payload, cursor, cursor + BSC_PARLIA_VALIDATOR_ADDRESS_BYTES);
      cursor += BSC_PARLIA_VALIDATOR_ADDRESS_BYTES;
      if (isZero(address)) {
        throw new IllegalArgumentException("validatorAddresses[" + i + "] must not be zero");
      }
      if (!seenAddresses.add(hexLower(address))) {
        throw new IllegalArgumentException("validatorAddresses[" + i + "] must be unique");
      }
      final BigInteger power = readU64Le(payload, cursor);
      cursor += 8;
      if (BigInteger.ZERO.equals(power)) {
        throw new IllegalArgumentException("validatorPowers[" + i + "] must not be zero");
      }
    }
    if (cursor != payload.length) {
      throw new IllegalArgumentException("validatorSetPayload has trailing bytes");
    }
  }

  private static void validateEthSyncCommitteePayload(final byte[] payload) {
    Objects.requireNonNull(payload, "payload");
    if (payload.length > ETH_MAX_SYNC_COMMITTEE_PAYLOAD_BYTES) {
      throw new IllegalArgumentException(
          "syncCommitteePayload must be at most "
              + ETH_MAX_SYNC_COMMITTEE_PAYLOAD_BYTES
              + " bytes");
    }
    int cursor = 0;
    if (payload.length < 5 || payload[cursor] != 1) {
      throw new IllegalArgumentException("syncCommitteePayload must have version 1");
    }
    cursor += 1;
    final int count = readU32Le(payload, cursor);
    cursor += 4;
    if (count <= 0) {
      throw new IllegalArgumentException("syncCommitteePayload must not be empty");
    }
    if (count > ETH_MAX_SYNC_COMMITTEE_AUTHORITIES) {
      throw new IllegalArgumentException(
          "syncCommitteePayload must contain at most "
              + ETH_MAX_SYNC_COMMITTEE_AUTHORITIES
              + " entries");
    }
    final Set<String> seenPublicKeys = new HashSet<>();
    for (int i = 0; i < count; i++) {
      final int publicKeyLen = readU32Le(payload, cursor);
      cursor += 4;
      if (publicKeyLen != ETH_SYNC_COMMITTEE_PUBLIC_KEY_BYTES
          || cursor + publicKeyLen > payload.length) {
        throw new IllegalArgumentException("syncCommitteePublicKeys[" + i + "] is invalid");
      }
      final byte[] publicKey = Arrays.copyOfRange(payload, cursor, cursor + publicKeyLen);
      cursor += publicKeyLen;
      if (isZero(publicKey)) {
        throw new IllegalArgumentException("syncCommitteePublicKeys[" + i + "] must not be zero");
      }
      if (!seenPublicKeys.add(hexLower(publicKey))) {
        throw new IllegalArgumentException("syncCommitteePublicKeys[" + i + "] must be unique");
      }
      final BigInteger weight = readU64Le(payload, cursor);
      cursor += 8;
      if (BigInteger.ZERO.equals(weight)) {
        throw new IllegalArgumentException("syncCommitteeWeights[" + i + "] must not be zero");
      }
      final int popLen = readU32Le(payload, cursor);
      cursor += 4;
      if (popLen != ETH_SYNC_COMMITTEE_POP_BYTES || cursor + popLen > payload.length) {
        throw new IllegalArgumentException("syncCommitteePops[" + i + "] is invalid");
      }
      final byte[] pop = Arrays.copyOfRange(payload, cursor, cursor + popLen);
      if (isZero(pop)) {
        throw new IllegalArgumentException("syncCommitteePops[" + i + "] must not be zero");
      }
      cursor += popLen;
    }
    if (cursor != payload.length) {
      throw new IllegalArgumentException("syncCommitteePayload has trailing bytes");
    }
  }

  private static List<Integer> ethSyncCommitteeSignerIndices(
      final byte[] signersBitmap, final int committeeSize) {
    Objects.requireNonNull(signersBitmap, "signersBitmap");
    if (signersBitmap.length != (committeeSize + 7) / 8) {
      throw new IllegalArgumentException(
          "signersBitmap length must match syncCommitteePublicKeys");
    }
    final List<Integer> signerIndices = new ArrayList<>();
    for (int byteIndex = 0; byteIndex < signersBitmap.length; byteIndex++) {
      final int value = signersBitmap[byteIndex] & 0xff;
      for (int bit = 0; bit < 8; bit++) {
        if (((value >> bit) & 1) == 0) {
          continue;
        }
        final int index = byteIndex * 8 + bit;
        if (index >= committeeSize) {
          throw new IllegalArgumentException("signersBitmap must not set padding bits");
        }
        signerIndices.add(index);
      }
    }
    if (signerIndices.isEmpty()) {
      throw new IllegalArgumentException(
          "signersBitmap must select at least one sync committee member");
    }
    return signerIndices;
  }

  private static List<Integer> substrateAuthoritySignerIndices(
      final byte[] signersBitmap, final int authorityCount) {
    Objects.requireNonNull(signersBitmap, "signersBitmap");
    if (signersBitmap.length != (authorityCount + 7) / 8) {
      throw new IllegalArgumentException("signersBitmap length must match authorityPublicKeys");
    }
    final List<Integer> signerIndices = new ArrayList<>();
    for (int byteIndex = 0; byteIndex < signersBitmap.length; byteIndex++) {
      final int value = signersBitmap[byteIndex] & 0xff;
      for (int bit = 0; bit < 8; bit++) {
        if (((value >> bit) & 1) == 0) {
          continue;
        }
        final int index = byteIndex * 8 + bit;
        if (index >= authorityCount) {
          throw new IllegalArgumentException("signersBitmap must not set padding bits");
        }
        signerIndices.add(index);
      }
    }
    if (signerIndices.isEmpty()) {
      throw new IllegalArgumentException("signersBitmap must select at least one authority");
    }
    return signerIndices;
  }

  private static void validateSubstrateAuthoritySetPayload(final byte[] payload) {
    Objects.requireNonNull(payload, "payload");
    if (payload.length > SUBSTRATE_MAX_AUTHORITY_SET_PAYLOAD_BYTES) {
      throw new IllegalArgumentException(
          "authoritySetPayload must be at most "
              + SUBSTRATE_MAX_AUTHORITY_SET_PAYLOAD_BYTES
              + " bytes");
    }
    int cursor = 0;
    if (payload.length == 0 || payload[cursor] != 1) {
      throw new IllegalArgumentException("authoritySetPayload must have version 1");
    }
    cursor += 1;
    final int count = readU32Le(payload, cursor);
    cursor += 4;
    if (count <= 0
        || count > SUBSTRATE_MAX_AUTHORITIES
        || payload.length - cursor != count * 40) {
      throw new IllegalArgumentException("authoritySetPayload length is invalid");
    }
    final Set<String> seenPublicKeys = new HashSet<>();
    for (int i = 0; i < count; i++) {
      final byte[] publicKey = Arrays.copyOfRange(payload, cursor, cursor + 32);
      cursor += 32;
      if (isZero(publicKey)) {
        throw new IllegalArgumentException("authorityPublicKeys[" + i + "] must not be zero");
      }
      if (!seenPublicKeys.add(hexLower(publicKey))) {
        throw new IllegalArgumentException("authorityPublicKeys[" + i + "] must be unique");
      }
      final BigInteger weight = readU64Le(payload, cursor);
      cursor += 8;
      if (BigInteger.ZERO.equals(weight)) {
        throw new IllegalArgumentException("authorityWeights[" + i + "] must not be zero");
      }
    }
    if (cursor != payload.length) {
      throw new IllegalArgumentException("authoritySetPayload has trailing bytes");
    }
  }

  private static void validateTronWitnessSchedulePayload(final byte[] payload) {
    Objects.requireNonNull(payload, "payload");
    if (payload.length < 5 || payload[0] != 1) {
      throw new IllegalArgumentException(
          "witnessSchedulePayload must be a canonical TRON witness schedule payload");
    }
    final int count = readU32Le(payload, 1);
    if (count <= 0 || count > TRON_MAX_WITNESSES || payload.length != 5 + count * 29) {
      throw new IllegalArgumentException(
          "witnessSchedulePayload must be a canonical TRON witness schedule payload");
    }
    final Set<String> seenAddresses = new HashSet<>();
    int cursor = 5;
    BigInteger totalWeight = BigInteger.ZERO;
    for (int i = 0; i < count; i++) {
      final byte[] address = Arrays.copyOfRange(payload, cursor, cursor + 21);
      cursor += 21;
      if (!isNonZeroTronAddress(address)) {
        throw new IllegalArgumentException(
            "witnessSchedulePayload witness " + i + " must be a TRON 0x41-prefixed address");
      }
      if (!seenAddresses.add(hexLower(address))) {
        throw new IllegalArgumentException("witnessSchedulePayload witness " + i + " must be unique");
      }
      final BigInteger weight = readU64Le(payload, cursor);
      cursor += 8;
      if (BigInteger.ZERO.equals(weight)) {
        throw new IllegalArgumentException(
            "witnessSchedulePayload witness " + i + " weight must not be zero");
      }
      totalWeight = totalWeight.add(weight);
      if (totalWeight.compareTo(MAX_U64) > 0) {
        throw new IllegalArgumentException("witnessSchedulePayload total weight must fit u64");
      }
    }
  }

  private static int readU32Le(final byte[] bytes, final int offset) {
    if (offset < 0 || offset + 4 > bytes.length) {
      throw new IllegalArgumentException("payload is truncated");
    }
    return (bytes[offset] & 0xff)
        | ((bytes[offset + 1] & 0xff) << 8)
        | ((bytes[offset + 2] & 0xff) << 16)
        | ((bytes[offset + 3] & 0xff) << 24);
  }

  private static BigInteger readU64Le(final byte[] bytes, final int offset) {
    if (offset < 0 || offset + 8 > bytes.length) {
      throw new IllegalArgumentException("payload is truncated");
    }
    BigInteger value = BigInteger.ZERO;
    for (int i = 7; i >= 0; i--) {
      value = value.shiftLeft(8).or(BigInteger.valueOf(bytes[offset + i] & 0xffL));
    }
    return value;
  }

  private static void writeBranch(final ByteArrayOutputStream out, final List<byte[]> branch) {
    writeBranch(out, branch, false);
  }

  private static void writeBranch(
      final ByteArrayOutputStream out, final List<byte[]> branch, final boolean requireNonEmpty) {
    Objects.requireNonNull(branch, "inclusionBranch");
    if (requireNonEmpty && branch.isEmpty()) {
      throw new IllegalArgumentException("inclusionBranch must not be empty");
    }
    writeU32Le(out, branch.size());
    for (int i = 0; i < branch.size(); i++) {
      final byte[] sibling = Objects.requireNonNull(branch.get(i), "inclusionBranch[" + i + "]");
      if (sibling.length != 32) {
        throw new IllegalArgumentException("inclusionBranch[" + i + "] must be 32 bytes");
      }
      write(out, sibling);
    }
  }

  private static int normalizeDomain(final int value, final String field) {
    if (value < 0) {
      throw new IllegalArgumentException(field + " must be a u32 domain id");
    }
    return value;
  }

  private static void requireV1Version(final int value, final String field) {
    if (value != 1) {
      throw new IllegalArgumentException(field + " must be 1");
    }
  }

  private static boolean isCanonicalDecimalText(final String value) {
    if ("0".equals(value)) {
      return true;
    }
    if (value == null || value.isEmpty()) {
      return false;
    }
    final char first = value.charAt(0);
    if (first < '1' || first > '9') {
      return false;
    }
    for (int index = 1; index < value.length(); index++) {
      final char current = value.charAt(index);
      if (current < '0' || current > '9') {
        return false;
      }
    }
    return true;
  }

  private static BigInteger normalizeU64(final String value, final String field) {
    if (!isCanonicalDecimalText(value)) {
      throw new IllegalArgumentException(field + " must be an unsigned integer");
    }
    final BigInteger numeric = new BigInteger(value);
    if (numeric.compareTo(MAX_U64) > 0) {
      throw new IllegalArgumentException(field + " must fit u64");
    }
    return numeric;
  }

  private static byte[] hex32Bytes(final String value, final String field) {
    return hexBytes(value, field, 32);
  }

  private static byte[] nonZeroHex32Bytes(final String value, final String field) {
    final byte[] bytes = hex32Bytes(value, field);
    if (isZero(bytes)) {
      throw new IllegalArgumentException(field + " must not be zero");
    }
    return bytes;
  }

  private static byte[] nonZeroHexBytes(
      final String value, final String field, final int byteLength) {
    final byte[] bytes = hexBytes(value, field, byteLength);
    if (isZero(bytes)) {
      throw new IllegalArgumentException(field + " must not be zero");
    }
    return bytes;
  }

  private static byte[] requireUnusedBytes(
      final String value, final String field, final int byteLength) {
    if (value != null) {
      throw new IllegalArgumentException(field + " is not used for sourceDomain");
    }
    return new byte[byteLength];
  }

  private static boolean isNonZeroTronAddress(final byte[] address) {
    if (address.length != 21 || address[0] != 0x41) {
      return false;
    }
    for (int i = 1; i < address.length; i++) {
      if (address[i] != 0) {
        return true;
      }
    }
    return false;
  }

  private static String normalizeHex32(final String value) {
    return "0x" + hexLower(hex32Bytes(value, "hex32"));
  }

  private static byte[] hexBytes(final String value, final String field, final int byteLength) {
    final String input = Objects.requireNonNull(value, field);
    if (!input.trim().equals(input)) {
      throw new IllegalArgumentException(field + " must be canonical hex");
    }
    String body = input;
    if (body.regionMatches(true, 0, "0x", 0, 2)) {
      body = body.substring(2);
    }
    if (body.length() != byteLength * 2) {
      throw new IllegalArgumentException(field + " must be " + byteLength + " bytes");
    }
    final byte[] out = new byte[byteLength];
    for (int i = 0; i < out.length; i++) {
      final int hi = Character.digit(body.charAt(i * 2), 16);
      final int lo = Character.digit(body.charAt(i * 2 + 1), 16);
      if (hi < 0 || lo < 0) {
        throw new IllegalArgumentException(field + " must be canonical hex");
      }
      out[i] = (byte) ((hi << 4) | lo);
    }
    return out;
  }

  private static void writeU32Le(final ByteArrayOutputStream out, final int value) {
    if (value < 0) {
      throw new IllegalArgumentException("u32 value must not be negative");
    }
    out.write(value & 0xff);
    out.write((value >>> 8) & 0xff);
    out.write((value >>> 16) & 0xff);
    out.write((value >>> 24) & 0xff);
  }

  private static byte[] abiWordU32(final int value, final String field) {
    if (value < 0) {
      throw new IllegalArgumentException(field + " must be a u32 domain id");
    }
    final byte[] out = new byte[32];
    out[28] = (byte) ((value >>> 24) & 0xff);
    out[29] = (byte) ((value >>> 16) & 0xff);
    out[30] = (byte) ((value >>> 8) & 0xff);
    out[31] = (byte) (value & 0xff);
    return out;
  }

  private static void writeI32Le(final ByteArrayOutputStream out, final int value) {
    out.write(value & 0xff);
    out.write((value >>> 8) & 0xff);
    out.write((value >>> 16) & 0xff);
    out.write((value >>> 24) & 0xff);
  }

  private static void writeU64Le(final ByteArrayOutputStream out, final BigInteger value) {
    BigInteger working = value;
    for (int i = 0; i < 8; i++) {
      out.write(working.and(BigInteger.valueOf(0xffL)).intValue());
      working = working.shiftRight(8);
    }
  }

  private static void writeU64Be(final ByteArrayOutputStream out, final BigInteger value) {
    for (int i = 7; i >= 0; i--) {
      out.write(value.shiftRight(i * 8).and(BigInteger.valueOf(0xffL)).intValue());
    }
  }

  private static byte[] canonicalBscValidatorSetPayloadBytesFromAddresses(
      final List<byte[]> addresses) {
    if (addresses.isEmpty() || addresses.size() > BSC_MAX_PARLIA_VALIDATORS) {
      throw new IllegalArgumentException("validatorAddresses must be a non-empty bounded array");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, addresses.size());
    final Set<String> seenAddresses = new HashSet<>();
    for (int i = 0; i < addresses.size(); i++) {
      final byte[] address = addresses.get(i);
      if (address.length != BSC_PARLIA_VALIDATOR_ADDRESS_BYTES) {
        throw new IllegalArgumentException("validatorAddresses[" + i + "] must be 20 bytes");
      }
      if (isZero(address)) {
        throw new IllegalArgumentException("validatorAddresses[" + i + "] must not be zero");
      }
      if (!seenAddresses.add(hexLower(address))) {
        throw new IllegalArgumentException("validatorAddresses[" + i + "] must be unique");
      }
      write(out, address);
      writeU64Le(out, BigInteger.ONE);
    }
    return out.toByteArray();
  }

  private static byte[] canonicalBscValidatorSetPayloadBytesFromAddressPowers(
      final List<byte[]> addresses, final List<BigInteger> powers) {
    if (addresses.isEmpty()
        || addresses.size() != powers.size()
        || addresses.size() > BSC_MAX_PARLIA_VALIDATORS) {
      throw new IllegalArgumentException(
          "validatorAddresses and validatorPowers must be non-empty bounded arrays");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, addresses.size());
    final Set<String> seenAddresses = new HashSet<>();
    for (int i = 0; i < addresses.size(); i++) {
      final byte[] address = addresses.get(i);
      if (address.length != BSC_PARLIA_VALIDATOR_ADDRESS_BYTES) {
        throw new IllegalArgumentException("validatorAddresses[" + i + "] must be 20 bytes");
      }
      if (isZero(address)) {
        throw new IllegalArgumentException("validatorAddresses[" + i + "] must not be zero");
      }
      if (!seenAddresses.add(hexLower(address))) {
        throw new IllegalArgumentException("validatorAddresses[" + i + "] must be unique");
      }
      final BigInteger power = powers.get(i);
      if (BigInteger.ZERO.equals(power)) {
        throw new IllegalArgumentException("validatorPowers[" + i + "] must not be zero");
      }
      write(out, address);
      writeU64Le(out, power);
    }
    return out.toByteArray();
  }

  private static List<Integer> bscSignerIndicesFromBitmap(
      final byte[] signersBitmap, final int rosterLength) {
    if (signersBitmap.length != (rosterLength + 7) / 8) {
      throw new IllegalArgumentException("signersBitmap has invalid length");
    }
    final List<Integer> indices = new ArrayList<>();
    for (int byteIndex = 0; byteIndex < signersBitmap.length; byteIndex++) {
      final int value = signersBitmap[byteIndex] & 0xff;
      for (int bit = 0; bit < 8; bit++) {
        final int validatorIndex = byteIndex * 8 + bit;
        final boolean bitSet = (value & (1 << bit)) != 0;
        if (validatorIndex >= rosterLength) {
          if (bitSet) {
            throw new IllegalArgumentException("signersBitmap padding bits must be zero");
          }
        } else if (bitSet) {
          indices.add(Integer.valueOf(validatorIndex));
        }
      }
    }
    if (indices.isEmpty()) {
      throw new IllegalArgumentException("signersBitmap must select at least one signer");
    }
    return indices;
  }

  private static byte[] bscValidatorAddress20(final byte[] publicKey, final String label) {
    if (!((publicKey.length == 33 && (publicKey[0] == 0x02 || publicKey[0] == 0x03))
        || (publicKey.length == 65 && publicKey[0] == 0x04))) {
      throw new IllegalArgumentException(
          label + " must be a compressed or uncompressed secp256k1 public key");
    }
    final org.bouncycastle.math.ec.ECPoint point;
    try {
      point = SECP256K1_PARAMS.getCurve().decodePoint(publicKey).normalize();
    } catch (final IllegalArgumentException exception) {
      throw new IllegalArgumentException(label + " must be a valid secp256k1 public key", exception);
    }
    if (!Arrays.equals(point.getEncoded(publicKey.length == 33), publicKey)) {
      throw new IllegalArgumentException(label + " must be a canonical secp256k1 public key");
    }
    final byte[] uncompressed = point.getEncoded(false);
    return Arrays.copyOfRange(
        keccak256(Arrays.copyOfRange(uncompressed, 1, uncompressed.length)), 12, 32);
  }

  private static List<byte[]> bscParliaValidatorSetPayloadCandidatesFromExtra(
      final byte[] extraData) {
    final List<byte[]> candidates = new ArrayList<>();
    final int minimumExtra = BSC_PARLIA_EXTRA_VANITY_BYTES + BSC_PARLIA_EXTRA_SEAL_BYTES;
    if (extraData.length <= minimumExtra) {
      return candidates;
    }
    final byte[] validatorRegion =
        Arrays.copyOfRange(
            extraData, BSC_PARLIA_EXTRA_VANITY_BYTES, extraData.length - BSC_PARLIA_EXTRA_SEAL_BYTES);
    if (validatorRegion.length == 0) {
      return candidates;
    }

    if (validatorRegion.length % BSC_PARLIA_VALIDATOR_ADDRESS_BYTES == 0) {
      final int count = validatorRegion.length / BSC_PARLIA_VALIDATOR_ADDRESS_BYTES;
      if (count <= BSC_MAX_PARLIA_VALIDATORS) {
        final List<byte[]> addresses = new ArrayList<>();
        for (int offset = 0; offset < validatorRegion.length; offset += BSC_PARLIA_VALIDATOR_ADDRESS_BYTES) {
          addresses.add(Arrays.copyOfRange(validatorRegion, offset, offset + BSC_PARLIA_VALIDATOR_ADDRESS_BYTES));
        }
        pushBscParliaPayloadCandidate(candidates, addresses);
      }
    }

    final int lubanCount = validatorRegion[0] & 0xff;
    final int lubanStride =
        BSC_PARLIA_VALIDATOR_ADDRESS_BYTES + BSC_PARLIA_VALIDATOR_BLS_KEY_BYTES;
    final int lubanRegionLength = 1 + lubanCount * lubanStride;
    if (lubanCount != 0
        && lubanCount <= BSC_MAX_PARLIA_VALIDATORS
        && validatorRegion.length >= lubanRegionLength) {
      final List<byte[]> addresses = new ArrayList<>();
      for (int i = 0; i < lubanCount; i++) {
        final int start = 1 + i * lubanStride;
        addresses.add(Arrays.copyOfRange(validatorRegion, start, start + BSC_PARLIA_VALIDATOR_ADDRESS_BYTES));
      }
      pushBscParliaPayloadCandidate(candidates, addresses);
    }

    return candidates;
  }

  private static void pushBscParliaPayloadCandidate(
      final List<byte[]> candidates, final List<byte[]> addresses) {
    try {
      final byte[] payload = canonicalBscValidatorSetPayloadBytesFromAddresses(addresses);
      for (final byte[] candidate : candidates) {
        if (Arrays.equals(candidate, payload)) {
          return;
        }
      }
      candidates.add(payload);
    } catch (final IllegalArgumentException ignored) {
      // Keep checking the other Parlia extraData layout.
    }
  }

  private static final class RlpReadResult {
    final boolean list;
    final byte[] value;
    final int next;

    RlpReadResult(final boolean list, final byte[] value, final int next) {
      this.list = list;
      this.value = value;
      this.next = next;
    }
  }

  private static int readRlpLength(final byte[] bytes, final int offset, final int lengthOfLength) {
    if (lengthOfLength <= 0 || lengthOfLength > 4 || offset + lengthOfLength > bytes.length) {
      throw new IllegalArgumentException("invalid RLP length");
    }
    if (bytes[offset] == 0) {
      throw new IllegalArgumentException("non-canonical RLP length");
    }
    int length = 0;
    for (int i = 0; i < lengthOfLength; i++) {
      length = length * 256 + (bytes[offset + i] & 0xff);
    }
    return length;
  }

  private static RlpReadResult rlpItemAt(final byte[] bytes, final int cursor) {
    if (cursor >= bytes.length) {
      throw new IllegalArgumentException("RLP cursor out of bounds");
    }
    final int first = bytes[cursor] & 0xff;
    if (first <= 0x7f) {
      return new RlpReadResult(false, new byte[] {bytes[cursor]}, cursor + 1);
    }
    if (first <= 0xb7) {
      final int length = first - 0x80;
      final int start = cursor + 1;
      final int end = start + length;
      if (end > bytes.length || (length == 1 && (bytes[start] & 0xff) < 0x80)) {
        throw new IllegalArgumentException("non-canonical RLP string");
      }
      return new RlpReadResult(false, Arrays.copyOfRange(bytes, start, end), end);
    }
    if (first <= 0xbf) {
      final int lengthOfLength = first - 0xb7;
      final int length = readRlpLength(bytes, cursor + 1, lengthOfLength);
      if (length < 56) {
        throw new IllegalArgumentException("non-canonical RLP long string");
      }
      final int start = cursor + 1 + lengthOfLength;
      final int end = start + length;
      if (end > bytes.length) {
        throw new IllegalArgumentException("RLP string out of bounds");
      }
      return new RlpReadResult(false, Arrays.copyOfRange(bytes, start, end), end);
    }
    if (first <= 0xf7) {
      final int length = first - 0xc0;
      final int start = cursor + 1;
      final int end = start + length;
      if (end > bytes.length) {
        throw new IllegalArgumentException("RLP list out of bounds");
      }
      return new RlpReadResult(true, Arrays.copyOfRange(bytes, start, end), end);
    }
    final int lengthOfLength = first - 0xf7;
    final int length = readRlpLength(bytes, cursor + 1, lengthOfLength);
    if (length < 56) {
      throw new IllegalArgumentException("non-canonical RLP long list");
    }
    final int start = cursor + 1 + lengthOfLength;
    final int end = start + length;
    if (end > bytes.length) {
      throw new IllegalArgumentException("RLP list out of bounds");
    }
    return new RlpReadResult(true, Arrays.copyOfRange(bytes, start, end), end);
  }

  private static List<byte[]> rlpListByteFields(final byte[] bytes) {
    final RlpReadResult outer = rlpItemAt(bytes, 0);
    if (!outer.list || outer.next != bytes.length) {
      throw new IllegalArgumentException("headerRlp must be an RLP list");
    }
    final List<byte[]> fields = new ArrayList<>();
    int cursor = 0;
    while (cursor < outer.value.length) {
      final RlpReadResult item = rlpItemAt(outer.value, cursor);
      if (item.list) {
        throw new IllegalArgumentException("headerRlp must contain only RLP byte fields");
      }
      fields.add(item.value);
      cursor = item.next;
    }
    return fields;
  }

  private static byte[] sszHashNode(final byte[] left, final byte[] right) {
    if (left.length != 32 || right.length != 32) {
      throw new IllegalArgumentException("SSZ node inputs must be 32 bytes");
    }
    final byte[] preimage = new byte[64];
    System.arraycopy(left, 0, preimage, 0, left.length);
    System.arraycopy(right, 0, preimage, left.length, right.length);
    return sha256(preimage);
  }

  private static byte[] sszMerkleizeChunks(final List<byte[]> inputChunks) {
    if (inputChunks.isEmpty()) {
      return new byte[32];
    }
    List<byte[]> chunks = new ArrayList<byte[]>(inputChunks.size());
    for (final byte[] chunk : inputChunks) {
      if (chunk.length != 32) {
        throw new IllegalArgumentException("SSZ chunk must be 32 bytes");
      }
      chunks.add(chunk);
    }
    int paddedLength = 1;
    while (paddedLength < chunks.size()) {
      paddedLength *= 2;
    }
    while (chunks.size() < paddedLength) {
      chunks.add(new byte[32]);
    }
    while (chunks.size() > 1) {
      final List<byte[]> next = new ArrayList<byte[]>(chunks.size() / 2);
      for (int i = 0; i < chunks.size(); i += 2) {
        next.add(sszHashNode(chunks.get(i), chunks.get(i + 1)));
      }
      chunks = next;
    }
    return chunks.get(0);
  }

  private static BigInteger readMinimalBeU64(final byte[] bytes, final String field) {
    if (bytes.length == 0) {
      return BigInteger.ZERO;
    }
    if (bytes.length > 8 || (bytes.length > 1 && bytes[0] == 0)) {
      throw new IllegalArgumentException(field + " must be a canonical RLP u64");
    }
    BigInteger out = BigInteger.ZERO;
    for (final byte value : bytes) {
      out = out.shiftLeft(8).or(BigInteger.valueOf(value & 0xffL));
    }
    return out;
  }

  private static byte[] sszU64Chunk(final BigInteger value) {
    if (value.signum() < 0 || value.compareTo(MAX_U64) > 0) {
      throw new IllegalArgumentException("SSZ u64 is out of range");
    }
    final byte[] out = new byte[32];
    BigInteger working = value;
    for (int i = 0; i < 8; i++) {
      out[i] = (byte) working.and(BigInteger.valueOf(0xffL)).intValue();
      working = working.shiftRight(8);
    }
    return out;
  }

  private static byte[] sszU64ChunkFromRlp(final byte[] bytes, final String field) {
    return sszU64Chunk(readMinimalBeU64(bytes, field));
  }

  private static byte[] sszU256ChunkFromRlp(final byte[] bytes, final String field) {
    if (bytes.length > 32 || (bytes.length > 1 && bytes[0] == 0)) {
      throw new IllegalArgumentException(field + " must be a canonical RLP uint256");
    }
    final byte[] out = new byte[32];
    for (int i = 0; i < bytes.length; i++) {
      out[i] = bytes[bytes.length - 1 - i];
    }
    return out;
  }

  private static byte[] sszByteVectorRoot(
      final byte[] bytes, final int expectedLength, final String field) {
    if (bytes.length != expectedLength) {
      throw new IllegalArgumentException(field + " must be " + expectedLength + " bytes");
    }
    final List<byte[]> chunks = new ArrayList<byte[]>();
    for (int offset = 0; offset < bytes.length; offset += 32) {
      final byte[] chunk = new byte[32];
      final int length = Math.min(32, bytes.length - offset);
      System.arraycopy(bytes, offset, chunk, 0, length);
      chunks.add(chunk);
    }
    return sszMerkleizeChunks(chunks);
  }

  private static byte[] sszMixInLength(final byte[] root, final int length) {
    return sszHashNode(root, sszU64Chunk(BigInteger.valueOf(length)));
  }

  private static byte[] sszByteListRoot(
      final byte[] bytes, final int maxLength, final String field) {
    if (bytes.length > maxLength) {
      throw new IllegalArgumentException(field + " must be at most " + maxLength + " bytes");
    }
    final int limitChunks = Math.max(1, (maxLength + 31) / 32);
    final List<byte[]> chunks = new ArrayList<byte[]>();
    for (int offset = 0; offset < bytes.length; offset += 32) {
      final byte[] chunk = new byte[32];
      final int length = Math.min(32, bytes.length - offset);
      System.arraycopy(bytes, offset, chunk, 0, length);
      chunks.add(chunk);
    }
    while (chunks.size() < limitChunks) {
      chunks.add(new byte[32]);
    }
    return sszMixInLength(sszMerkleizeChunks(chunks), bytes.length);
  }

  private static byte[] sszMerkleRootFromBranch(
      final byte[] leaf, final int leafIndex, final List<byte[]> branch, final String field) {
    if (leaf.length != 32) {
      throw new IllegalArgumentException(field + " leaf must be 32 bytes");
    }
    byte[] current = leaf;
    int index = leafIndex;
    for (int branchIndex = 0; branchIndex < branch.size(); branchIndex++) {
      final byte[] sibling = Objects.requireNonNull(branch.get(branchIndex), field + "[" + branchIndex + "]");
      if (sibling.length != 32) {
        throw new IllegalArgumentException(field + "[" + branchIndex + "] must be 32 bytes");
      }
      current =
          (index & 1) == 1 ? sszHashNode(sibling, current) : sszHashNode(current, sibling);
      index >>>= 1;
    }
    return current;
  }

  private static String hashHex(final String prefix, final byte[] payload) {
    return "0x" + hexLower(hashBytes(prefix, payload));
  }

  private static byte[] hashBytes(final String prefix, final byte[] payload) {
    final byte[] prefixBytes = prefix.getBytes(StandardCharsets.UTF_8);
    final byte[] preimage = new byte[prefixBytes.length + payload.length];
    System.arraycopy(prefixBytes, 0, preimage, 0, prefixBytes.length);
    System.arraycopy(payload, 0, preimage, prefixBytes.length, payload.length);
    return Blake2b.digest256(preimage);
  }

  private static String keccakHashHex(final String prefix, final byte[] payload) {
    return "0x" + hexLower(keccakHashBytes(prefix, payload));
  }

  private static byte[] keccakHashBytes(final String prefix, final byte[] payload) {
    final byte[] prefixBytes = prefix.getBytes(StandardCharsets.UTF_8);
    final byte[] preimage = new byte[prefixBytes.length + payload.length];
    System.arraycopy(prefixBytes, 0, preimage, 0, prefixBytes.length);
    System.arraycopy(payload, 0, preimage, prefixBytes.length, payload.length);
    return keccak256(preimage);
  }

  private static byte[] keccak256(final byte[] input) {
    final KeccakDigest digest = new KeccakDigest(256);
    digest.update(input, 0, input.length);
    final byte[] out = new byte[32];
    digest.doFinal(out, 0);
    return out;
  }

  private static byte[] sha256(final byte[] input) {
    try {
      return MessageDigest.getInstance("SHA-256").digest(input);
    } catch (final NoSuchAlgorithmException exception) {
      throw new IllegalStateException("SHA-256 is unavailable", exception);
    }
  }

  private static boolean isZero(final byte[] bytes) {
    for (final byte value : bytes) {
      if (value != 0) {
        return false;
      }
    }
    return true;
  }

  private static boolean tronRecoverableSignatureIsCanonical(final byte[] signature) {
    if (signature.length != 65) {
      return false;
    }
    final int recoveryId = signature[64] & 0xff;
    if (!((recoveryId >= 0 && recoveryId <= 3) || (recoveryId >= 27 && recoveryId <= 30))) {
      return false;
    }
    final byte[] rValue = Arrays.copyOfRange(signature, 0, 32);
    final byte[] sValue = Arrays.copyOfRange(signature, 32, 64);
    return !isZero(rValue)
        && compareUnsignedBytes(rValue, SECP256K1_SCALAR_ORDER_BE) < 0
        && !isZero(sValue)
        && compareUnsignedBytes(sValue, SECP256K1_SCALAR_HALF_ORDER_BE) <= 0;
  }

  private static byte[] tronRecoveredSignerAddress20(
      final byte[] messageHash, final byte[] signature) {
    if (messageHash.length != 32 || !tronRecoverableSignatureIsCanonical(signature)) {
      return null;
    }
    final int recoveryIdByte = signature[64] & 0xff;
    final int recoveryId = recoveryIdByte >= 27 ? recoveryIdByte - 27 : recoveryIdByte;
    final BigInteger rValue = new BigInteger(1, Arrays.copyOfRange(signature, 0, 32));
    final BigInteger sValue = new BigInteger(1, Arrays.copyOfRange(signature, 32, 64));
    final BigInteger x =
        rValue.add(SECP256K1_SCALAR_ORDER.multiply(BigInteger.valueOf(recoveryId / 2L)));
    if (x.compareTo(SECP256K1_FIELD_PRIME) >= 0) {
      return null;
    }
    final byte[] compressed = new byte[33];
    compressed[0] = (byte) ((recoveryId & 1) == 1 ? 0x03 : 0x02);
    System.arraycopy(bigIntegerToFixedBytes(x, 32), 0, compressed, 1, 32);
    final org.bouncycastle.math.ec.ECPoint rPoint;
    try {
      rPoint = SECP256K1_PARAMS.getCurve().decodePoint(compressed);
    } catch (final IllegalArgumentException exception) {
      return null;
    }
    if (!rPoint.multiply(SECP256K1_SCALAR_ORDER).isInfinity()) {
      return null;
    }
    final BigInteger eValue = new BigInteger(1, messageHash).mod(SECP256K1_SCALAR_ORDER);
    final org.bouncycastle.math.ec.ECPoint publicKey =
        rPoint
            .multiply(sValue)
            .subtract(SECP256K1_PARAMS.getG().multiply(eValue))
            .multiply(rValue.modInverse(SECP256K1_SCALAR_ORDER))
            .normalize();
    if (publicKey.isInfinity()) {
      return null;
    }
    final byte[] encoded = publicKey.getEncoded(false);
    return Arrays.copyOfRange(keccak256(Arrays.copyOfRange(encoded, 1, encoded.length)), 12, 32);
  }

  private static byte[] bigIntegerToFixedBytes(final BigInteger value, final int byteLength) {
    final byte[] raw = value.toByteArray();
    final byte[] out = new byte[byteLength];
    final int copyLength = Math.min(raw.length, byteLength);
    System.arraycopy(raw, raw.length - copyLength, out, byteLength - copyLength, copyLength);
    return out;
  }

  private static int compareUnsignedBytes(final byte[] left, final byte[] right) {
    if (left.length != right.length) {
      return left.length - right.length;
    }
    for (int index = 0; index < left.length; index++) {
      final int leftByte = left[index] & 0xff;
      final int rightByte = right[index] & 0xff;
      if (leftByte != rightByte) {
        return leftByte - rightByte;
      }
    }
    return 0;
  }

  private static String hexLower(final byte[] bytes) {
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte value : bytes) {
      builder.append(String.format("%02x", value & 0xff));
    }
    return builder.toString();
  }

  private static void write(final ByteArrayOutputStream out, final byte[] bytes) {
    out.write(bytes, 0, bytes.length);
  }
}
