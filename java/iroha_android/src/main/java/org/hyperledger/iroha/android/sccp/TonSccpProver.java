package org.hyperledger.iroha.android.sccp;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.Comparator;
import java.util.List;
import java.util.Objects;
import org.hyperledger.iroha.android.crypto.Blake2b;

/** TON SCCP proof request and internal-message helpers for local-first Android proof generation. */
public final class TonSccpProver {
  public static final int DOMAIN_TON = 4;
  public static final String CONTRACT_PROOF_BACKEND_V1 = "ton-contract-v1";
  public static final String MESSAGE_BODY_BOC_V1 = "ton_message_body_boc_v1";
  public static final String STARK_FRI_PROOF_FAMILY_V1 = "stark-fri-v1";
  public static final int NATIVE_RECURSIVE_MAX_PROOF_BYTES = 2 * 1024 * 1024;
  public static final String MAINNET_SHARD_STATE_VERIFIER_ID_V1 =
      "sccp:ton:source-state-verifier:shard-state-light-client-mainnet:v1";
  public static final String SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1 =
      "sccp-ton-shard-state-light-client-v1";
  public static final String MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1 =
      "sccp-ton-masterchain-config-v1";
  public static final String VALIDATOR_SET_TRANSITION_OPEN_VERIFY_CIRCUIT_ID_V1 =
      "sccp-ton-validator-set-transition-v1";
  public static final String SHARD_ACCOUNTS_DICTIONARY_OPEN_VERIFY_CIRCUIT_ID_V1 =
      "sccp-ton-shard-accounts-dictionary-v1";
  private static final byte[] TEMPLATE_SHARD_STATE_VERIFIER_HASH =
      hex32Bytes(
          "0x540205f876591604ccf39f72a051ac5e82647c9e48dbd48cb129d2543971a34f",
          "templateShardStateVerifierHash");
  private static final byte[][] TEMPLATE_SOURCE_MATERIAL_HASHES = {
    hex32Bytes(
        "0xd83b3a3eb920ac8338533535cf0d6c69c69d507e84aef8ec2094564b8427c56c",
        "tonTemplateSourceTrustAnchorHash"),
    hex32Bytes(
        "0xb0225e16477ea3420f7d0de76b87b6e99a43ab97f445d8565a384d4b655bc473",
        "tonTemplateConsensusVerifierHash"),
    hex32Bytes(
        "0x89254256421c15da8c92842c7d6f448ef6c1d5ca1e2a173754643425fcee6353",
        "tonTemplateMessageInclusionVerifierHash"),
    hex32Bytes(
        "0x50044ee6db0eb0cdef097e69406b6c30d3406d8f784e8ba34e9b923b38bd0c43",
        "tonTemplateFinalityPolicyHash"),
    TEMPLATE_SHARD_STATE_VERIFIER_HASH
  };
  public static final long CURRENT_VALIDATOR_SET_CONFIG_PARAM = 34L;
  public static final int CONFIG_PARAM_KEY_BITS = 32;

  private static final String SHARD_PROOF_PREFIX_V1 = "sccp:ton:shard-proof:v1";
  private static final String VALIDATOR_SET_PREFIX_V1 = "sccp:ton:validator-set:v1";
  private static final String VALIDATOR_SET_PAYLOAD_PREFIX_V1 =
      "sccp:ton:validator-set-payload:v1";
  private static final String MASTERCHAIN_CONFIG_LEAF_PREFIX_V1 =
      "sccp:ton:masterchain-config-leaf:v1";
  private static final String MASTERCHAIN_CONFIG_PROOF_PREFIX_V1 =
      "sccp:ton:masterchain-config-proof:v1";
  private static final String MASTERCHAIN_BLOCK_MESSAGE_PREFIX_V1 =
      "sccp:ton:masterchain-block-message:v1";
  private static final String MASTERCHAIN_SIGNATURES_PREFIX_V1 =
      "sccp:ton:masterchain-signatures:v1";
  private static final String VALIDATOR_SET_TRANSITION_MESSAGE_PREFIX_V1 =
      "sccp:ton:validator-set-transition-message:v1";
  private static final String VALIDATOR_SET_TRANSITION_SIGNATURES_PREFIX_V1 =
      "sccp:ton:validator-set-transition-signatures:v1";
  private static final String VALIDATOR_SET_TRANSITION_CHAIN_PREFIX_V1 =
      "sccp:ton:validator-set-transition-chain:v1";
  private static final String SHARD_STATE_PROOF_PUBLIC_INPUTS_PREFIX_V1 =
      "sccp:ton:shard-state-proof-public-inputs:v1";
  private static final String SHARD_STATE_FASTPQ_DSID_PREFIX_V1 =
      "sccp:ton:shard-state:fastpq:dsid:v1";
  private static final String SHARD_STATE_FASTPQ_PARAMETER_SET_V1 = "fastpq-lane-balanced";
  private static final String SHARD_STATE_FASTPQ_STATEMENT_KEY_V1 =
      "sccp:ton:shard-state:v1:statement";
  private static final String SHARD_STATE_FASTPQ_WITNESS_KEY_V1 =
      "sccp:ton:shard-state:v1:witness";
  private static final String SHARD_STATE_FASTPQ_CONTEXT_KEY_V1 =
      "sccp:ton:shard-state:v1:context";
  private static final String SHARD_STATE_PROOF_BOC_PREFIX_V1 =
      "sccp:ton:shard-state-proof-boc:v1";
  private static final String SHARD_ACCOUNTS_PROOF_BOC_PREFIX_V1 =
      "sccp:ton:shard-accounts-proof-boc:v1";
  private static final String CONFIG_PROOF_BOC_PREFIX_V1 = "sccp:ton:config-proof-boc:v1";
  private static final String FULL_LIGHT_CLIENT_AUDIT_FASTPQ_DSID_PREFIX_V1 =
      "sccp:ton:full-light-client-audit:fastpq:dsid:v1";
  private static final String FULL_LIGHT_CLIENT_AUDIT_FASTPQ_PARAMETER_SET_V1 =
      "fastpq-lane-balanced";
  private static final String FULL_LIGHT_CLIENT_AUDIT_FASTPQ_STATEMENT_KEY_V1 =
      "sccp:ton:full-light-client-audit:v1:statement";
  private static final String FULL_LIGHT_CLIENT_AUDIT_FASTPQ_CONTEXT_KEY_V1 =
      "sccp:ton:full-light-client-audit:v1:context";
  private static final String FULL_LIGHT_CLIENT_AUDIT_FASTPQ_GATE_KEY_V1 =
      "sccp:ton:full-light-client-audit:v1:gate";
  private static final String FULL_LIGHT_CLIENT_AUDIT_STATEMENT_PREFIX_V1 =
      "sccp:ton:full-light-client-audit:statement:v1";
  private static final String ROUTE_CANARY_LIVE_ACCOUNT_PREFIX_V1 =
      "iroha:sccp:ton-route-canary-live-account:v1";
  private static final long SUBMIT_OP_V1 = 0x53434350L;
  private static final int MESSAGE_SCHEMA_VERSION_V1 = 1;
  private static final int MAX_CELL_DATA_BYTES = 127;
  private static final int MAX_CELL_SERIALIZED_DATA_BYTES = 128;
  private static final int MAX_BOC_BYTES = 64 * 1024;
  private static final int MAX_BOC_CELLS = 4096;
  private static final int MAX_REFS = 4;
  private static final int MAX_VALIDATORS = 1024;
  private static final int SHARD_ACCOUNT_KEY_BITS = 256;
  private static final int VALIDATOR_SET_KEY_BITS = 16;
  private static final int VALIDATOR_CONSTRUCTOR = 0x53;
  private static final int VALIDATOR_ADDR_CONSTRUCTOR = 0x73;
  private static final int VALIDATORS_CONSTRUCTOR = 0x11;
  private static final int VALIDATORS_EXT_CONSTRUCTOR = 0x12;
  private static final int ED25519_PUBKEY_CONSTRUCTOR = 0x8e81278a;
  private static final int MAX_SOURCE_MERKLE_BRANCH_NODES = 64;
  private static final int CRC32C_REFLECTED_POLY = -2097792136;
  private static final int SHARD_STATE_UNSPLIT_TAG = 0x9023afe2;
  private static final int TON_MAINNET_GLOBAL_ID = -239;
  private static final int TON_MASTERCHAIN_WORKCHAIN_ID = -1;
  private static final BigInteger TON_MASTERCHAIN_SHARD = BigInteger.ONE.shiftLeft(63);
  private static final int TON_BASECHAIN_WORKCHAIN_ID = 0;
  private static final byte[] BOC_MAGIC = {
    (byte) 0xb5, (byte) 0xee, (byte) 0x9c, 0x72
  };
  private static final BigInteger MAX_U64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);

  private final WitnessProvider witnessProvider;
  private final ProofEngine proofEngine;

  /** Selected TON ShardAccount last transaction identity. */
  public static final class ShardAccountLastTransaction {
    private final String hash;
    private final BigInteger lt;

    public ShardAccountLastTransaction(final String hash, final BigInteger lt) {
      this.hash = Objects.requireNonNull(hash, "hash");
      this.lt = Objects.requireNonNull(lt, "lt");
    }

    public String hash() {
      return hash;
    }

    public BigInteger lt() {
      return lt;
    }

    @Override
    public boolean equals(final Object other) {
      if (this == other) {
        return true;
      }
      if (!(other instanceof ShardAccountLastTransaction)) {
        return false;
      }
      final ShardAccountLastTransaction that = (ShardAccountLastTransaction) other;
      return hash.equals(that.hash) && lt.equals(that.lt);
    }

    @Override
    public int hashCode() {
      return Objects.hash(hash, lt);
    }
  }

  public TonSccpProver() {
    this(null, null);
  }

  public TonSccpProver(final WitnessProvider witnessProvider, final ProofEngine proofEngine) {
    this.witnessProvider = witnessProvider;
    this.proofEngine = proofEngine;
  }

  public ProofRequest buildRequest(final ProofRequestInput input) {
    final ProofRequestInput resolved =
        witnessProvider == null
            ? input
            : witnessProvider.resolveWitness(witnessProviderInputSnapshot(input));
    return buildProofRequest(resolved);
  }

  public ProofResult prove(final ProofRequestInput input) {
    final ProofRequest request = buildRequest(input);
    if (proofEngine == null) {
      throw new IllegalStateException("TON SCCP local prover is not linked");
    }
    requireProductionProofRequest(request);
    return wrapProofResult(proofEngine.prove(callbackRequestSnapshot(request)), request);
  }

  private static ProofRequestInput witnessProviderInputSnapshot(final ProofRequestInput input) {
    final byte[] bundleBytes = Objects.requireNonNull(input.bundleBytes(), "bundleBytes");
    final byte[] sourceProofBytes =
        Objects.requireNonNull(input.sourceProofBytes(), "sourceProofBytes");
    return new ProofRequestInput(
        input.publicInputs(),
        Arrays.copyOf(bundleBytes, bundleBytes.length),
        Arrays.copyOf(sourceProofBytes, sourceProofBytes.length),
        input.statementHash(),
        input.destinationBindingHash(),
        input.sourceStateVerifierId(),
        input.sourceStateVerifierHash(),
        input.sourceAdapterDeploymentHash(),
        input.sourceAdapterDeploymentReceiptHash(),
        input.backend(),
        input.sourceDomain());
  }

  public static byte[] canonicalPublicInputsBytes(final PublicInputsInput input) {
    Objects.requireNonNull(input, "input");
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(input.version());
    write(out, hex32Bytes(input.messageId(), "messageId"));
    write(out, hex32Bytes(input.payloadHash(), "payloadHash"));
    writeU32Le(out, input.targetDomain());
    write(out, hex32Bytes(input.commitmentRoot(), "commitmentRoot"));
    writeU64Le(out, normalizeU64(input.finalityHeight(), "finalityHeight"));
    write(out, hex32Bytes(input.finalityBlockHash(), "finalityBlockHash"));
    return out.toByteArray();
  }

  /** Canonical live-account evidence bytes for the SORA -> TON route canary. */
  public static byte[] canonicalRouteCanaryEvidenceBytes(final RouteCanaryEvidenceInput input) {
    Objects.requireNonNull(input, "input");
    final byte[] routeAllowlistHash =
        nonZeroHex32Bytes(input.routeAllowlistHash(), "routeAllowlistHash");
    final byte[] destinationBindingHash =
        nonZeroHex32Bytes(input.destinationBindingHash(), "destinationBindingHash");
    final String canonicalTonDestinationBindingHash =
        SourceSccpProofs.destinationBindingHash(DOMAIN_TON);
    final String expectedDestinationBindingHash =
        normalizeNonZeroHex32(
            input.expectedDestinationBindingHash() == null
                ? canonicalTonDestinationBindingHash
                : input.expectedDestinationBindingHash(),
            "expectedDestinationBindingHash");
    if (!expectedDestinationBindingHash.equals(canonicalTonDestinationBindingHash)) {
      throw new IllegalArgumentException(
          "expectedDestinationBindingHash must match canonical TON destination binding");
    }
    if (!("0x" + hexLower(destinationBindingHash)).equals(canonicalTonDestinationBindingHash)) {
      throw new IllegalArgumentException(
          "destinationBindingHash must match canonical TON destination binding");
    }
    final byte[] sourceVerifierMaterialHash =
        nonZeroHex32Bytes(input.sourceVerifierMaterialHash(), "sourceVerifierMaterialHash");
    final byte[] sourceAdapterEngineDeploymentHash =
        nonZeroHex32Bytes(
            input.sourceAdapterEngineDeploymentHash(), "sourceAdapterEngineDeploymentHash");
    final String verifierContractAddress =
        normalizeTonRawAddress(input.verifierContractAddress(), "verifierContractAddress");
    final byte[] verifierCodeHash = nonZeroHex32Bytes(input.verifierCodeHash(), "verifierCodeHash");
    final String accountStatus =
        normalizeTonActiveAccountStatus(input.accountStatus(), "accountStatus");
    final byte[] accountStateHash = nonZeroHex32Bytes(input.accountStateHash(), "accountStateHash");
    final String lastTransactionLt =
        normalizePositiveDecimalText(input.lastTransactionLt(), "lastTransactionLt");
    final byte[] lastTransactionHash =
        nonZeroHex32Bytes(input.lastTransactionHash(), "lastTransactionHash");
    final byte[] verifierCodeBocRootHash =
        nonZeroHex32Bytes(input.verifierCodeBocRootHash(), "verifierCodeBocRootHash");
    if (!Arrays.equals(verifierCodeBocRootHash, verifierCodeHash)) {
      throw new IllegalArgumentException("verifierCodeBocRootHash must match verifierCodeHash");
    }

    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, SolanaSccpProver.DOMAIN_SORA);
    writeU32Le(out, DOMAIN_TON);
    write(out, routeAllowlistHash);
    write(out, destinationBindingHash);
    write(out, sourceVerifierMaterialHash);
    write(out, sourceAdapterEngineDeploymentHash);
    writeVector(out, verifierContractAddress.getBytes(java.nio.charset.StandardCharsets.UTF_8));
    write(out, verifierCodeHash);
    writeVector(out, accountStatus.getBytes(java.nio.charset.StandardCharsets.UTF_8));
    write(out, accountStateHash);
    writeVector(out, lastTransactionLt.getBytes(java.nio.charset.StandardCharsets.UTF_8));
    write(out, lastTransactionHash);
    write(out, verifierCodeBocRootHash);
    return out.toByteArray();
  }

  /** Hash Rust verifies for the SORA -> TON live-account route canary. */
  public static String routeCanaryEvidenceHash(final RouteCanaryEvidenceInput input) {
    return hashHex(ROUTE_CANARY_LIVE_ACCOUNT_PREFIX_V1, canonicalRouteCanaryEvidenceBytes(input));
  }

  public static byte[] canonicalShardProofBytes(
      final String sourceEventDigest,
      final String masterchainSeqno,
      final String masterchainBlockHash,
      final int shardWorkchainId,
      final String shardShard,
      final String shardSeqno,
      final String shardBlockHash,
      final String shardFileHash,
      final String shardStateRoot,
      final String transactionRoot,
      final String transactionLt,
      final String shardStateLeafIndex,
      final List<byte[]> shardStateInclusionBranch,
      final List<byte[]> inclusionBranch) {
    return canonicalShardProofBytes(
        sourceEventDigest,
        masterchainSeqno,
        masterchainBlockHash,
        shardWorkchainId,
        shardShard,
        shardSeqno,
        shardBlockHash,
        shardFileHash,
        shardStateRoot,
        transactionRoot,
        transactionLt,
        shardStateLeafIndex,
        shardStateInclusionBranch,
        inclusionBranch,
        null,
        null,
        new byte[0],
        new byte[0],
        new byte[0]);
  }

  public static byte[] canonicalShardProofBytes(
      final String sourceEventDigest,
      final String masterchainSeqno,
      final String masterchainBlockHash,
      final int shardWorkchainId,
      final String shardShard,
      final String shardSeqno,
      final String shardBlockHash,
      final String shardFileHash,
      final String shardStateRoot,
      final String transactionRoot,
      final String transactionLt,
      final String shardStateLeafIndex,
      final List<byte[]> shardStateInclusionBranch,
      final List<byte[]> inclusionBranch,
      final String shardStateDictionaryRoot,
      final Integer shardStateDictionaryKeyBitLen,
      final byte[] shardStateDictionaryKey,
      final byte[] shardStateDictionaryProofBoc,
      final byte[] shardStateProofBoc) {
    final List<byte[]> shardStateBranch = normalizeInclusionBranch(shardStateInclusionBranch);
    final List<byte[]> branch = normalizeInclusionBranch(inclusionBranch);
    final byte[] dictionaryKey =
        shardStateDictionaryKey == null ? new byte[0] : shardStateDictionaryKey;
    final byte[] dictionaryProofBoc =
        shardStateDictionaryProofBoc == null ? new byte[0] : shardStateDictionaryProofBoc;
    final byte[] stateProofBoc = shardStateProofBoc == null ? new byte[0] : shardStateProofBoc;
    final boolean hasDictionaryOpening =
        shardStateDictionaryRoot != null
            || shardStateDictionaryKeyBitLen != null
            || dictionaryKey.length != 0
            || dictionaryProofBoc.length != 0;
    if (hasDictionaryOpening && stateProofBoc.length == 0) {
      throw new IllegalArgumentException(
          "shardStateProofBoc is required for TON shard-state dictionary openings");
    }
    if (!hasDictionaryOpening && stateProofBoc.length != 0) {
      throw new IllegalArgumentException(
          "shardStateProofBoc requires a TON shard-state dictionary opening");
    }
    if (hasDictionaryOpening && !shardStateBranch.isEmpty()) {
      throw new IllegalArgumentException(
          "shardStateInclusionBranch must be empty for TON shard-state dictionary openings");
    }
    final BigInteger normalizedMasterchainSeqno = normalizeU64(masterchainSeqno, "masterchainSeqno");
    if (shardWorkchainId != TON_BASECHAIN_WORKCHAIN_ID) {
      throw new IllegalArgumentException("shardWorkchainId must be TON basechain");
    }
    final BigInteger normalizedShard = normalizeU64(shardShard, "shardShard");
    if (BigInteger.ZERO.equals(normalizedShard)) {
      throw new IllegalArgumentException("shardShard must not be zero");
    }
    final BigInteger normalizedShardSeqno = normalizeU64(shardSeqno, "shardSeqno");
    if (BigInteger.ZERO.equals(normalizedShardSeqno)) {
      throw new IllegalArgumentException("shardSeqno must not be zero");
    }
    final BigInteger normalizedTransactionLt = normalizeU64(transactionLt, "transactionLt");
    if (BigInteger.ZERO.equals(normalizedTransactionLt)) {
      throw new IllegalArgumentException("transactionLt must not be zero");
    }
    final byte[] shardFileHashBytes = nonZeroHex32Bytes(shardFileHash, "shardFileHash");
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    write(out, hex32Bytes(sourceEventDigest, "sourceEventDigest"));
    writeU64Le(out, normalizedMasterchainSeqno);
    write(out, hex32Bytes(masterchainBlockHash, "masterchainBlockHash"));
    writeI32Le(out, shardWorkchainId);
    writeU64Le(out, normalizedShard);
    writeU64Le(out, normalizedShardSeqno);
    write(out, hex32Bytes(shardBlockHash, "shardBlockHash"));
    write(out, shardFileHashBytes);
    final byte[] shardStateRootBytes = hex32Bytes(shardStateRoot, "shardStateRoot");
    final byte[] transactionRootBytes = hex32Bytes(transactionRoot, "transactionRoot");
    write(out, shardStateRootBytes);
    write(out, transactionRootBytes);
    writeU64Le(out, normalizedTransactionLt);
    if (stateProofBoc.length != 0) {
      writeVector(out, stateProofBoc);
    }
    if (hasDictionaryOpening) {
      if (shardStateDictionaryRoot == null) {
        throw new IllegalArgumentException("shardStateDictionaryRoot is required");
      }
      if (shardStateDictionaryKeyBitLen == null) {
        throw new IllegalArgumentException("shardStateDictionaryKeyBitLen is required");
      }
      final byte[] dictionaryRoot = hex32Bytes(shardStateDictionaryRoot, "shardStateDictionaryRoot");
      boolean nonzeroRoot = false;
      for (final byte value : dictionaryRoot) {
        nonzeroRoot |= value != 0;
      }
      if (!nonzeroRoot) {
        throw new IllegalArgumentException("shardStateDictionaryRoot must not be zero");
      }
      if (shardStateDictionaryKeyBitLen < 0 || shardStateDictionaryKeyBitLen > 0xffff) {
        throw new IllegalArgumentException("shardStateDictionaryKeyBitLen must fit u16");
      }
      if (shardStateDictionaryKeyBitLen != SHARD_ACCOUNT_KEY_BITS) {
        throw new IllegalArgumentException("TON ShardAccounts key bit length must be 256");
      }
      if (!hashmapKeyIsCanonical(dictionaryKey, shardStateDictionaryKeyBitLen)) {
        throw new IllegalArgumentException("shardStateDictionaryKey length is invalid");
      }
      if (dictionaryProofBoc.length == 0) {
        throw new IllegalArgumentException("shardStateDictionaryProofBoc must not be empty");
      }
      if (!shardStateProofRootHash(stateProofBoc).equals("0x" + hexLower(shardStateRootBytes))) {
        throw new IllegalArgumentException("shardStateProofBoc root must match shardStateRoot");
      }
      final ShardStateAccountsOpening shardStateOpening = shardStateAccountsOpening(stateProofBoc);
      if (!shardStateOpening.accountsRootHash.equals("0x" + hexLower(dictionaryRoot))) {
        throw new IllegalArgumentException(
            "shardStateProofBoc accounts root must match shardStateDictionaryRoot");
      }
      if (shardStateOpening.globalId != TON_MAINNET_GLOBAL_ID) {
        throw new IllegalArgumentException(
            "shardStateProofBoc ShardStateUnsplit global_id must be TON mainnet");
      }
      if (shardStateOpening.workchainId != TON_BASECHAIN_WORKCHAIN_ID) {
        throw new IllegalArgumentException(
            "shardStateProofBoc ShardIdent workchain_id must be TON basechain");
      }
      if (shardStateOpening.workchainId != shardWorkchainId) {
        throw new IllegalArgumentException(
            "shardStateProofBoc ShardIdent workchain_id must match shardWorkchainId");
      }
      if (!BigInteger.valueOf(Integer.toUnsignedLong(shardStateOpening.seqNo))
          .equals(normalizedShardSeqno)) {
        throw new IllegalArgumentException(
            "shardStateProofBoc ShardStateUnsplit seq_no must match shardSeqno");
      }
      if (!shardStateOpening.shardId.equals(normalizedShard)) {
        throw new IllegalArgumentException(
            "shardStateProofBoc ShardIdent shard must match shardShard");
      }
      if (shardStateOpening.seqNo == 0) {
        throw new IllegalArgumentException(
            "shardStateProofBoc ShardStateUnsplit seq_no must be non-zero");
      }
      if (shardStateOpening.genUtime == 0) {
        throw new IllegalArgumentException(
            "shardStateProofBoc ShardStateUnsplit gen_utime must be non-zero");
      }
      if (shardStateOpening.genLt == 0L) {
        throw new IllegalArgumentException(
            "shardStateProofBoc ShardStateUnsplit gen_lt must be non-zero");
      }
      if (BigInteger.valueOf(Integer.toUnsignedLong(shardStateOpening.minRefMcSeqno))
              .compareTo(normalizedMasterchainSeqno)
          > 0) {
        throw new IllegalArgumentException(
            "shardStateProofBoc ShardStateUnsplit min_ref_mc_seqno exceeds masterchainSeqno");
      }
      if (!shardStateAccountKeyMatchesShardPrefix(
          dictionaryKey, shardStateDictionaryKeyBitLen, shardStateOpening)) {
        throw new IllegalArgumentException(
            "shardStateDictionaryKey must match shardStateProofBoc ShardIdent prefix");
      }
      final ShardAccountLastTransaction selectedTransaction =
          shardAccountsLastTransaction(
              dictionaryProofBoc, dictionaryKey, shardStateDictionaryKeyBitLen);
      if (selectedTransaction == null
          || !selectedTransaction.hash().equals("0x" + hexLower(transactionRootBytes))) {
        throw new IllegalArgumentException(
            "shardStateDictionaryProofBoc ShardAccount last transaction hash must match transactionRoot");
      }
      if (!selectedTransaction.lt().equals(normalizedTransactionLt)) {
        throw new IllegalArgumentException(
            "shardStateDictionaryProofBoc ShardAccount last transaction lt must match transactionLt");
      }
      write(out, dictionaryRoot);
      writeU16Le(out, shardStateDictionaryKeyBitLen);
      writeVector(out, dictionaryKey);
      writeVector(out, dictionaryProofBoc);
    }
    writeU64Le(out, normalizeU64(shardStateLeafIndex, "shardStateLeafIndex"));
    writeU32Le(out, shardStateBranch.size());
    for (final byte[] sibling : shardStateBranch) {
      write(out, sibling);
    }
    writeU32Le(out, branch.size());
    for (final byte[] sibling : branch) {
      write(out, sibling);
    }
    return out.toByteArray();
  }

  public static String shardProofHash(
      final String sourceEventDigest,
      final String masterchainSeqno,
      final String masterchainBlockHash,
      final int shardWorkchainId,
      final String shardShard,
      final String shardSeqno,
      final String shardBlockHash,
      final String shardFileHash,
      final String shardStateRoot,
      final String transactionRoot,
      final String transactionLt,
      final String shardStateLeafIndex,
      final List<byte[]> shardStateInclusionBranch,
      final List<byte[]> inclusionBranch) {
    return shardProofHash(
        sourceEventDigest,
        masterchainSeqno,
        masterchainBlockHash,
        shardWorkchainId,
        shardShard,
        shardSeqno,
        shardBlockHash,
        shardFileHash,
        shardStateRoot,
        transactionRoot,
        transactionLt,
        shardStateLeafIndex,
        shardStateInclusionBranch,
        inclusionBranch,
        null,
        null,
        new byte[0],
        new byte[0],
        new byte[0]);
  }

  public static String shardProofHash(
      final String sourceEventDigest,
      final String masterchainSeqno,
      final String masterchainBlockHash,
      final int shardWorkchainId,
      final String shardShard,
      final String shardSeqno,
      final String shardBlockHash,
      final String shardFileHash,
      final String shardStateRoot,
      final String transactionRoot,
      final String transactionLt,
      final String shardStateLeafIndex,
      final List<byte[]> shardStateInclusionBranch,
      final List<byte[]> inclusionBranch,
      final String shardStateDictionaryRoot,
      final Integer shardStateDictionaryKeyBitLen,
      final byte[] shardStateDictionaryKey,
      final byte[] shardStateDictionaryProofBoc,
      final byte[] shardStateProofBoc) {
    return hashHex(
        SHARD_PROOF_PREFIX_V1,
        canonicalShardProofBytes(
            sourceEventDigest,
            masterchainSeqno,
            masterchainBlockHash,
            shardWorkchainId,
            shardShard,
            shardSeqno,
            shardBlockHash,
            shardFileHash,
            shardStateRoot,
            transactionRoot,
            transactionLt,
            shardStateLeafIndex,
            shardStateInclusionBranch,
            inclusionBranch,
            shardStateDictionaryRoot,
            shardStateDictionaryKeyBitLen,
            shardStateDictionaryKey,
            shardStateDictionaryProofBoc,
            shardStateProofBoc));
  }

  public static List<String> bocRootHashes(final byte[] input) {
    final ParsedBoc parsed = parseBocCompleteOrdinary(input);
    final List<BocComputedCell> hashes = bocCellHashes(parsed.cells);
    final ArrayList<String> roots = new ArrayList<>(parsed.roots.size());
    for (final int root : parsed.roots) {
      if (root < 0 || root >= hashes.size()) {
        throw new IllegalArgumentException("TON BoC root index is invalid");
      }
      roots.add("0x" + hexLower(hashes.get(root).hashes.get(3)));
    }
    return roots;
  }

  public static String bocSingleRootHash(final byte[] input) {
    final List<String> roots = bocRootHashes(input);
    if (roots.size() != 1) {
      throw new IllegalArgumentException("TON BoC must contain exactly one root");
    }
    return roots.get(0);
  }

  public static String shardStateProofRootHash(final byte[] input) {
    final ParsedBoc parsed = parseBocCompleteOrdinary(input);
    final List<BocComputedCell> computed = bocCellHashes(parsed.cells);
    return "0x" + hexLower(bocProofRootAndChildIndex(parsed, computed).rootHash);
  }

  public static String hashmapEProofRootHash(final byte[] input) {
    final ParsedBoc parsed = parseBocCompleteOrdinary(input);
    final List<BocComputedCell> computed = bocCellHashes(parsed.cells);
    return "0x" + hexLower(bocProofRootAndChildIndex(parsed, computed).rootHash);
  }

  public static String shardStateAccountsRootHash(final byte[] input) {
    return shardStateAccountsOpening(input).accountsRootHash;
  }

  private static ShardStateAccountsOpening shardStateAccountsOpening(final byte[] input) {
    final ParsedBoc parsed = parseBocCompleteOrdinary(input);
    final List<BocComputedCell> computed = bocCellHashes(parsed.cells);
    final int childIndex = bocProofRootAndChildIndex(parsed, computed).childIndex;
    return shardStateUnsplitAccountsOpeningFromCell(parsed.cells, computed, childIndex);
  }

  public static String hashmapECellRefValueHash(
      final byte[] input, final byte[] key, final int keyBitLen) {
    if (!hashmapKeyIsCanonical(key, keyBitLen)) {
      throw new IllegalArgumentException("TON HashmapE key length is invalid");
    }
    final ParsedBoc parsed = parseBocCompleteOrdinary(input);
    final List<BocComputedCell> computed = bocCellHashes(parsed.cells);
    if (parsed.roots.size() != 1) {
      throw new IllegalArgumentException("TON BoC must contain exactly one root");
    }
    final Integer rootIndex = hashmapUnwrapMerkleProofCell(parsed.cells, parsed.roots.get(0));
    if (rootIndex == null) {
      throw new IllegalArgumentException("TON HashmapE root is pruned or unsupported");
    }
    final BocCell root = parsed.cells.get(rootIndex);
    if (bocCellKind(root) != BocCellKind.ORDINARY) {
      throw new IllegalArgumentException("TON HashmapE root must be ordinary");
    }
    final BocBitReader reader = new BocBitReader(root);
    final boolean hasRoot = reader.readBit();
    if (!hasRoot) {
      if (!reader.isExhausted()) {
        throw new IllegalArgumentException("TON HashmapE empty root is invalid");
      }
      return null;
    }
    if (reader.remainingBits() != 0 || reader.remainingRefs() != 1) {
      throw new IllegalArgumentException("TON HashmapE root is invalid");
    }
    return hashmapCellRefValueHash(
        parsed.cells, computed, reader.readRef(), key.clone(), keyBitLen);
  }

  public static byte[] configValidatorSetPayloadFromProofBoc(final byte[] input) {
    final ParsedBoc parsed = parseBocCompleteOrdinary(input);
    bocCellHashes(parsed.cells);
    if (parsed.roots.size() != 1) {
      throw new IllegalArgumentException("TON BoC must contain exactly one root");
    }
    final Integer rootIndex = hashmapUnwrapMerkleProofCell(parsed.cells, parsed.roots.get(0));
    if (rootIndex == null) {
      throw new IllegalArgumentException("TON config dictionary root is pruned or unsupported");
    }
    final BocCell root = parsed.cells.get(rootIndex);
    if (bocCellKind(root) != BocCellKind.ORDINARY) {
      throw new IllegalArgumentException("TON config dictionary root must be ordinary");
    }
    final BocBitReader reader = new BocBitReader(root);
    final boolean hasRoot = reader.readBit();
    if (!hasRoot) {
      if (!reader.isExhausted()) {
        throw new IllegalArgumentException("TON config dictionary empty root is invalid");
      }
      return null;
    }
    if (reader.remainingBits() != 0 || reader.remainingRefs() != 1) {
      throw new IllegalArgumentException("TON config dictionary root is invalid");
    }
    final Integer valueRef =
        hashmapCellRefValueIndex(
            parsed.cells,
            reader.readRef(),
            currentValidatorSetConfigKey(),
            CONFIG_PARAM_KEY_BITS);
    return valueRef == null ? null : validatorSetPayloadFromCell(parsed.cells, valueRef);
  }

  public static String configValidatorSetPayloadHashFromProofBoc(final byte[] input) {
    final byte[] payload = configValidatorSetPayloadFromProofBoc(input);
    return payload == null ? null : validatorSetPayloadHash(payload);
  }

  public static ShardAccountLastTransaction shardAccountsLastTransaction(
      final byte[] input, final byte[] key, final int keyBitLen) {
    if (keyBitLen != SHARD_ACCOUNT_KEY_BITS) {
      throw new IllegalArgumentException("TON ShardAccounts key bit length must be 256");
    }
    if (!hashmapKeyIsCanonical(key, keyBitLen)) {
      throw new IllegalArgumentException("TON ShardAccounts key length is invalid");
    }
    final ParsedBoc parsed = parseBocCompleteOrdinary(input);
    final List<BocComputedCell> computed = bocCellHashes(parsed.cells);
    if (parsed.roots.size() != 1) {
      throw new IllegalArgumentException("TON BoC must contain exactly one root");
    }
    final Integer rootIndex = hashmapUnwrapMerkleProofCell(parsed.cells, parsed.roots.get(0));
    if (rootIndex == null) {
      throw new IllegalArgumentException("TON ShardAccounts root is pruned or unsupported");
    }
    final BocCell root = parsed.cells.get(rootIndex);
    if (bocCellKind(root) != BocCellKind.ORDINARY) {
      throw new IllegalArgumentException("TON ShardAccounts root must be ordinary");
    }
    final BocBitReader reader = new BocBitReader(root);
    final boolean hasRoot = reader.readBit();
    if (!hasRoot) {
      if (!reader.isExhausted()) {
        throw new IllegalArgumentException("TON ShardAccounts empty root is invalid");
      }
      return null;
    }
    if (reader.remainingBits() != 0 || reader.remainingRefs() != 1) {
      throw new IllegalArgumentException("TON ShardAccounts root is invalid");
    }
    return hashmapShardAccountsLastTransaction(
        parsed.cells, computed, reader.readRef(), key.clone(), keyBitLen);
  }

  public static String shardAccountsLastTransactionHash(
      final byte[] input, final byte[] key, final int keyBitLen) {
    final ShardAccountLastTransaction transaction =
        shardAccountsLastTransaction(input, key, keyBitLen);
    return transaction == null ? null : transaction.hash();
  }

  public static byte[] canonicalValidatorSetBytes(
      final List<byte[]> validatorPublicKeys, final List<String> validatorWeights) {
    final ValidatorSetParts parts = normalizeValidatorSet(validatorPublicKeys, validatorWeights);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, parts.publicKeys.size());
    for (int index = 0; index < parts.publicKeys.size(); index++) {
      write(out, parts.publicKeys.get(index));
      writeU64Le(out, parts.weights.get(index));
    }
    return out.toByteArray();
  }

  public static String validatorSetHash(
      final List<byte[]> validatorPublicKeys, final List<String> validatorWeights) {
    return validatorSetHashFromPayload(canonicalValidatorSetBytes(validatorPublicKeys, validatorWeights));
  }

  public static byte[] canonicalValidatorSetPayloadBytes(
      final List<byte[]> validatorPublicKeys, final List<String> validatorWeights) {
    return canonicalValidatorSetBytes(validatorPublicKeys, validatorWeights);
  }

  private static byte[] canonicalValidatorSetBytesFromParts(
      final List<byte[]> validatorPublicKeys, final List<BigInteger> validatorWeights) {
    if (validatorPublicKeys.isEmpty()
        || validatorPublicKeys.size() > MAX_VALIDATORS
        || validatorPublicKeys.size() != validatorWeights.size()) {
      throw new IllegalArgumentException(
          "TON validator public keys and weights must be same-length arrays");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, validatorPublicKeys.size());
    for (int index = 0; index < validatorPublicKeys.size(); index++) {
      final byte[] publicKey = Objects.requireNonNull(validatorPublicKeys.get(index), "validatorPublicKeys");
      if (publicKey.length != 32) {
        throw new IllegalArgumentException("validatorPublicKeys[" + index + "] must be 32 bytes");
      }
      if (!containsNonZero(publicKey)) {
        throw new IllegalArgumentException("validatorPublicKeys[" + index + "] must not be zero");
      }
      final BigInteger weight = Objects.requireNonNull(validatorWeights.get(index), "validatorWeights");
      if (BigInteger.ZERO.equals(weight) || weight.compareTo(MAX_U64) > 0) {
        throw new IllegalArgumentException(
            "validatorWeights[" + index + "] must fit u64 and be non-zero");
      }
      write(out, publicKey);
      writeU64Le(out, weight);
    }
    final byte[] payload = out.toByteArray();
    validateValidatorSetPayload(payload);
    return payload;
  }

  public static String validatorSetHashFromPayload(final byte[] payload) {
    validateValidatorSetPayload(payload);
    return hashHex(VALIDATOR_SET_PREFIX_V1, payload);
  }

  public static String validatorSetPayloadHash(final byte[] payload) {
    validateValidatorSetPayload(payload);
    return hashHex(VALIDATOR_SET_PAYLOAD_PREFIX_V1, payload);
  }

  public static byte[] canonicalMasterchainConfigLeafBytes(
      final int sourceDomain,
      final String masterchainSeqno,
      final String masterchainBlockHash,
      final String shardStateRoot,
      final String validatorSetHash,
      final String validatorSetPayloadHash) {
    return canonicalMasterchainConfigLeafBytes(
        1,
        sourceDomain,
        masterchainSeqno,
        masterchainBlockHash,
        shardStateRoot,
        validatorSetHash,
        validatorSetPayloadHash);
  }

  public static byte[] canonicalMasterchainConfigLeafBytes(
      final int version,
      final int sourceDomain,
      final String masterchainSeqno,
      final String masterchainBlockHash,
      final String shardStateRoot,
      final String validatorSetHash,
      final String validatorSetPayloadHash) {
    if (version != 1) {
      throw new IllegalArgumentException("TON masterchain config leaf version must be 1");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(version);
    writeU32Le(out, sourceDomain);
    writeU64Le(out, normalizeU64(masterchainSeqno, "masterchainSeqno"));
    write(out, hex32Bytes(masterchainBlockHash, "masterchainBlockHash"));
    write(out, hex32Bytes(shardStateRoot, "shardStateRoot"));
    write(out, hex32Bytes(validatorSetHash, "validatorSetHash"));
    write(out, hex32Bytes(validatorSetPayloadHash, "validatorSetPayloadHash"));
    return out.toByteArray();
  }

  public static String masterchainConfigLeafHash(
      final int sourceDomain,
      final String masterchainSeqno,
      final String masterchainBlockHash,
      final String shardStateRoot,
      final String validatorSetHash,
      final String validatorSetPayloadHash) {
    return hashHex(
        MASTERCHAIN_CONFIG_LEAF_PREFIX_V1,
        canonicalMasterchainConfigLeafBytes(
            sourceDomain,
            masterchainSeqno,
            masterchainBlockHash,
            shardStateRoot,
            validatorSetHash,
            validatorSetPayloadHash));
  }

  public static byte[] canonicalMasterchainConfigProofBytes(
      final int sourceDomain,
      final String masterchainSeqno,
      final String masterchainBlockHash,
      final String shardStateRoot,
      final String configRoot,
      final String validatorSetHash,
      final String validatorSetPayloadHash,
      final String configLeafHash,
      final String configLeafIndex,
      final String configValueHash,
      final byte[] configDictionaryProofBoc,
      final List<byte[]> configInclusionBranch) {
    return canonicalMasterchainConfigProofBytes(
        1,
        sourceDomain,
        masterchainSeqno,
        masterchainBlockHash,
        shardStateRoot,
        configRoot,
        validatorSetHash,
        validatorSetPayloadHash,
        configLeafHash,
        configLeafIndex,
        configValueHash,
        configDictionaryProofBoc,
        configInclusionBranch);
  }

  public static byte[] canonicalMasterchainConfigProofBytes(
      final int version,
      final int sourceDomain,
      final String masterchainSeqno,
      final String masterchainBlockHash,
      final String shardStateRoot,
      final String configRoot,
      final String validatorSetHash,
      final String validatorSetPayloadHash,
      final String configLeafHash,
      final String configLeafIndex,
      final String configValueHash,
      final byte[] configDictionaryProofBoc,
      final List<byte[]> configInclusionBranch) {
    if (version != 1) {
      throw new IllegalArgumentException("TON masterchain config proof version must be 1");
    }
    if (sourceDomain != DOMAIN_TON) {
      throw new IllegalArgumentException("sourceDomain must be TON");
    }
    final BigInteger normalizedMasterchainSeqno = normalizeU64(masterchainSeqno, "masterchainSeqno");
    if (BigInteger.ZERO.equals(normalizedMasterchainSeqno)) {
      throw new IllegalArgumentException("masterchainSeqno must be non-zero");
    }
    final List<byte[]> branch = normalizeInclusionBranch(configInclusionBranch);
    if (!branch.isEmpty()) {
      throw new IllegalArgumentException(
          "configInclusionBranch must be empty when configDictionaryProofBoc is used");
    }
    final byte[] masterchainBlockHashBytes =
        nonZeroHex32Bytes(masterchainBlockHash, "masterchainBlockHash");
    final byte[] shardStateRootBytes = nonZeroHex32Bytes(shardStateRoot, "shardStateRoot");
    final byte[] configRootBytes = nonZeroHex32Bytes(configRoot, "configRoot");
    final byte[] configValueHashBytes = nonZeroHex32Bytes(configValueHash, "configValueHash");
    final byte[] dictionaryProof =
        Objects.requireNonNull(configDictionaryProofBoc, "configDictionaryProofBoc").clone();
    if (dictionaryProof.length == 0) {
      throw new IllegalArgumentException("configDictionaryProofBoc must be non-empty");
    }
    if (!hashmapEProofRootHash(dictionaryProof).equals("0x" + hexLower(configRootBytes))) {
      throw new IllegalArgumentException("configDictionaryProofBoc root does not match configRoot");
    }
    if (!Objects.equals(
        hashmapECellRefValueHash(
            dictionaryProof, currentValidatorSetConfigKey(), CONFIG_PARAM_KEY_BITS),
        "0x" + hexLower(configValueHashBytes))) {
      throw new IllegalArgumentException(
          "configDictionaryProofBoc value does not match configValueHash");
    }
    final byte[] validatorSetPayloadHashBytes =
        hex32Bytes(validatorSetPayloadHash, "validatorSetPayloadHash");
    if (!containsNonZero(validatorSetPayloadHashBytes)) {
      throw new IllegalArgumentException("validatorSetPayloadHash must be non-zero");
    }
    final byte[] validatorSetPayload = configValidatorSetPayloadFromProofBoc(dictionaryProof);
    if (validatorSetPayload == null) {
      throw new IllegalArgumentException("configDictionaryProofBoc must open config param 34");
    }
    if (!validatorSetPayloadHash(validatorSetPayload)
        .equals("0x" + hexLower(validatorSetPayloadHashBytes))) {
      throw new IllegalArgumentException(
          "configDictionaryProofBoc ValidatorSet does not match validatorSetPayloadHash");
    }
    final byte[] validatorSetHashBytes = nonZeroHex32Bytes(validatorSetHash, "validatorSetHash");
    if (!validatorSetHashFromPayload(validatorSetPayload)
        .equals("0x" + hexLower(validatorSetHashBytes))) {
      throw new IllegalArgumentException(
          "validatorSetHash must match configDictionaryProofBoc ValidatorSet");
    }
    final byte[] configLeafHashBytes = nonZeroHex32Bytes(configLeafHash, "configLeafHash");
    final String expectedConfigLeafHash =
        masterchainConfigLeafHash(
            sourceDomain,
            normalizedMasterchainSeqno.toString(),
            "0x" + hexLower(masterchainBlockHashBytes),
            "0x" + hexLower(shardStateRootBytes),
            "0x" + hexLower(validatorSetHashBytes),
            "0x" + hexLower(validatorSetPayloadHashBytes));
    if (!Arrays.equals(configLeafHashBytes, hex32Bytes(expectedConfigLeafHash, "configLeafHash"))) {
      throw new IllegalArgumentException("configLeafHash must match TON config proof fields");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(version);
    writeU32Le(out, sourceDomain);
    writeU64Le(out, normalizedMasterchainSeqno);
    write(out, masterchainBlockHashBytes);
    write(out, shardStateRootBytes);
    write(out, configRootBytes);
    write(out, validatorSetHashBytes);
    write(out, validatorSetPayloadHashBytes);
    write(out, configLeafHashBytes);
    final BigInteger normalizedConfigLeafIndex = normalizeU64(configLeafIndex, "configLeafIndex");
    if (!normalizedConfigLeafIndex.equals(BigInteger.valueOf(CURRENT_VALIDATOR_SET_CONFIG_PARAM))) {
      throw new IllegalArgumentException(
          "configLeafIndex must be TON current validator set config param 34");
    }
    writeU16Le(out, CONFIG_PARAM_KEY_BITS);
    writeU64Le(out, normalizedConfigLeafIndex);
    write(out, configValueHashBytes);
    writeVector(out, dictionaryProof);
    writeU32Le(out, branch.size());
    for (final byte[] sibling : branch) {
      writeVector(out, sibling);
    }
    return out.toByteArray();
  }

  public static String masterchainConfigProofHash(
      final int sourceDomain,
      final String masterchainSeqno,
      final String masterchainBlockHash,
      final String shardStateRoot,
      final String configRoot,
      final String validatorSetHash,
      final String validatorSetPayloadHash,
      final String configLeafHash,
      final String configLeafIndex,
      final String configValueHash,
      final byte[] configDictionaryProofBoc,
      final List<byte[]> configInclusionBranch) {
    return hashHex(
        MASTERCHAIN_CONFIG_PROOF_PREFIX_V1,
        canonicalMasterchainConfigProofBytes(
            sourceDomain,
            masterchainSeqno,
            masterchainBlockHash,
            shardStateRoot,
            configRoot,
            validatorSetHash,
            validatorSetPayloadHash,
            configLeafHash,
            configLeafIndex,
            configValueHash,
            configDictionaryProofBoc,
            configInclusionBranch));
  }

  public static byte[] canonicalMasterchainBlockMessageBytes(
      final int sourceDomain,
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
      final String shardProofHash) {
    if (masterchainWorkchainId != TON_MASTERCHAIN_WORKCHAIN_ID) {
      throw new IllegalArgumentException("masterchainWorkchainId must be TON masterchain");
    }
    final BigInteger normalizedShard = normalizeU64(masterchainShard, "masterchainShard");
    if (!TON_MASTERCHAIN_SHARD.equals(normalizedShard)) {
      throw new IllegalArgumentException("masterchainShard must be TON masterchain shard");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, sourceDomain);
    writeU64Le(out, normalizeU64(masterchainSeqno, "masterchainSeqno"));
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

  public static String masterchainBlockMessageHash(
      final int sourceDomain,
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
      final String shardProofHash) {
    return hashHex(
        MASTERCHAIN_BLOCK_MESSAGE_PREFIX_V1,
        canonicalMasterchainBlockMessageBytes(
            sourceDomain,
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
            shardProofHash));
  }

  public static byte[] canonicalMasterchainValidatorSignaturesBytes(
      final ValidatorSignatureProofInput input, final String providedValidatorSetHash) {
    final String derivedValidatorSetHash =
        validatorSetHash(input.validatorPublicKeys(), input.validatorWeights());
    if (providedValidatorSetHash != null
        && !Arrays.equals(
            hex32Bytes(providedValidatorSetHash, "validatorSetHash"),
            hex32Bytes(derivedValidatorSetHash, "validatorSetHash"))) {
      throw new IllegalArgumentException(
          "validatorSetHash must match validator public keys and weights");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    write(out, canonicalValidatorSignatureProofBytes(input));
    write(out, hex32Bytes(derivedValidatorSetHash, "validatorSetHash"));
    return out.toByteArray();
  }

  public static String masterchainValidatorSignaturesHash(
      final ValidatorSignatureProofInput input, final String providedValidatorSetHash) {
    return hashHex(
        MASTERCHAIN_SIGNATURES_PREFIX_V1,
        canonicalMasterchainValidatorSignaturesBytes(input, providedValidatorSetHash));
  }

  public static byte[] canonicalValidatorSetTransitionMessageBytes(
      final int sourceDomain,
      final String fromValidatorSetSeqno,
      final String toValidatorSetSeqno,
      final String masterchainSeqno,
      final int masterchainWorkchainId,
      final String masterchainShard,
      final String masterchainBlockHash,
      final String masterchainFileHash,
      final String parentValidatorSetHash,
      final String nextValidatorSetHash,
      final String nextValidatorSetPayloadHash,
      final String nextValidatorSetConfigHash) {
    if (sourceDomain != DOMAIN_TON) {
      throw new IllegalArgumentException("sourceDomain must be TON");
    }
    final BigInteger normalizedFromSeqno =
        normalizeU64(fromValidatorSetSeqno, "fromValidatorSetSeqno");
    final BigInteger normalizedToSeqno = normalizeU64(toValidatorSetSeqno, "toValidatorSetSeqno");
    if (!normalizedFromSeqno.add(BigInteger.ONE).equals(normalizedToSeqno)) {
      throw new IllegalArgumentException(
          "toValidatorSetSeqno must be exactly one greater than fromValidatorSetSeqno");
    }
    final BigInteger normalizedMasterchainSeqno = normalizeU64(masterchainSeqno, "masterchainSeqno");
    if (BigInteger.ZERO.equals(normalizedMasterchainSeqno)) {
      throw new IllegalArgumentException("masterchainSeqno must be non-zero");
    }
    if (masterchainWorkchainId != TON_MASTERCHAIN_WORKCHAIN_ID) {
      throw new IllegalArgumentException("masterchainWorkchainId must be TON masterchain");
    }
    final BigInteger normalizedShard = normalizeU64(masterchainShard, "masterchainShard");
    if (!TON_MASTERCHAIN_SHARD.equals(normalizedShard)) {
      throw new IllegalArgumentException("masterchainShard must be TON masterchain shard");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, sourceDomain);
    writeU64Le(out, normalizedFromSeqno);
    writeU64Le(out, normalizedToSeqno);
    writeU64Le(out, normalizedMasterchainSeqno);
    writeI32Le(out, masterchainWorkchainId);
    writeU64Le(out, normalizedShard);
    write(out, nonZeroHex32Bytes(masterchainBlockHash, "masterchainBlockHash"));
    write(out, nonZeroHex32Bytes(masterchainFileHash, "masterchainFileHash"));
    write(out, nonZeroHex32Bytes(parentValidatorSetHash, "parentValidatorSetHash"));
    write(out, nonZeroHex32Bytes(nextValidatorSetHash, "nextValidatorSetHash"));
    write(out, hex32Bytes(nextValidatorSetPayloadHash, "nextValidatorSetPayloadHash"));
    write(out, nonZeroHex32Bytes(nextValidatorSetConfigHash, "nextValidatorSetConfigHash"));
    return out.toByteArray();
  }

  public static String validatorSetTransitionMessageHash(
      final int sourceDomain,
      final String fromValidatorSetSeqno,
      final String toValidatorSetSeqno,
      final String masterchainSeqno,
      final int masterchainWorkchainId,
      final String masterchainShard,
      final String masterchainBlockHash,
      final String masterchainFileHash,
      final String parentValidatorSetHash,
      final String nextValidatorSetHash,
      final String nextValidatorSetPayloadHash,
      final String nextValidatorSetConfigHash) {
    return hashHex(
        VALIDATOR_SET_TRANSITION_MESSAGE_PREFIX_V1,
        canonicalValidatorSetTransitionMessageBytes(
            sourceDomain,
            fromValidatorSetSeqno,
            toValidatorSetSeqno,
            masterchainSeqno,
            masterchainWorkchainId,
            masterchainShard,
            masterchainBlockHash,
            masterchainFileHash,
            parentValidatorSetHash,
            nextValidatorSetHash,
            nextValidatorSetPayloadHash,
            nextValidatorSetConfigHash));
  }

  public static byte[] canonicalValidatorSetTransitionSignatureBytes(
      final int version,
      final int sourceDomain,
      final String fromValidatorSetSeqno,
      final String toValidatorSetSeqno,
      final String masterchainSeqno,
      final int masterchainWorkchainId,
      final String masterchainShard,
      final String masterchainBlockHash,
      final String masterchainFileHash,
      final String parentValidatorSetHash,
      final String nextValidatorSetHash,
      final byte[] nextValidatorSetPayload,
      final String nextValidatorSetPayloadHash,
      final String nextValidatorSetConfigHash,
      final String transitionMessageHash,
      final ValidatorSignatureProofInput validatorSignatureProof) {
    if (version != 1) {
      throw new IllegalArgumentException("TON validator-set transition proof version must be 1");
    }
    final String parentHash =
        validatorSetHash(
            validatorSignatureProof.validatorPublicKeys(),
            validatorSignatureProof.validatorWeights());
    final byte[] parentHashBytes = hex32Bytes(parentHash, "parentValidatorSetHash");
    final byte[] providedParentHashBytes =
        hex32Bytes(parentValidatorSetHash, "parentValidatorSetHash");
    if (!Arrays.equals(providedParentHashBytes, parentHashBytes)) {
      throw new IllegalArgumentException(
          "parentValidatorSetHash must match validatorSignatureProof");
    }
    if (!validatorSetPayloadHash(nextValidatorSetPayload).equals(nextValidatorSetPayloadHash)) {
      throw new IllegalArgumentException(
          "nextValidatorSetPayloadHash must match nextValidatorSetPayload");
    }
    if (!validatorSetHashFromPayload(nextValidatorSetPayload).equals(nextValidatorSetHash)) {
      throw new IllegalArgumentException("nextValidatorSetHash must match nextValidatorSetPayload");
    }
    final byte[] transitionMessageHashBytes =
        hex32Bytes(transitionMessageHash, "transitionMessageHash");
    final String expectedTransitionMessageHash =
        validatorSetTransitionMessageHash(
            sourceDomain,
            fromValidatorSetSeqno,
            toValidatorSetSeqno,
            masterchainSeqno,
            masterchainWorkchainId,
            masterchainShard,
            masterchainBlockHash,
            masterchainFileHash,
            parentValidatorSetHash,
            nextValidatorSetHash,
            nextValidatorSetPayloadHash,
            nextValidatorSetConfigHash);
    if (!Arrays.equals(
        transitionMessageHashBytes,
        hex32Bytes(expectedTransitionMessageHash, "transitionMessageHash"))) {
      throw new IllegalArgumentException(
          "transitionMessageHash must match transition message fields");
    }
    if (!Arrays.equals(
        hex32Bytes(validatorSignatureProof.blockMessageHash(), "blockMessageHash"),
        transitionMessageHashBytes)) {
      throw new IllegalArgumentException(
          "validatorSignatureProof.blockMessageHash must match transitionMessageHash");
    }
    if (masterchainWorkchainId != TON_MASTERCHAIN_WORKCHAIN_ID) {
      throw new IllegalArgumentException("masterchainWorkchainId must be TON masterchain");
    }
    final BigInteger normalizedMasterchainShard = normalizeU64(masterchainShard, "masterchainShard");
    if (!TON_MASTERCHAIN_SHARD.equals(normalizedMasterchainShard)) {
      throw new IllegalArgumentException("masterchainShard must be TON masterchain shard");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(version);
    writeU32Le(out, sourceDomain);
    writeU64Le(out, normalizeU64(fromValidatorSetSeqno, "fromValidatorSetSeqno"));
    writeU64Le(out, normalizeU64(toValidatorSetSeqno, "toValidatorSetSeqno"));
    writeU64Le(out, normalizeU64(masterchainSeqno, "masterchainSeqno"));
    writeI32Le(out, masterchainWorkchainId);
    writeU64Le(out, normalizedMasterchainShard);
    write(out, nonZeroHex32Bytes(masterchainBlockHash, "masterchainBlockHash"));
    write(out, nonZeroHex32Bytes(masterchainFileHash, "masterchainFileHash"));
    write(out, providedParentHashBytes);
    write(out, hex32Bytes(nextValidatorSetHash, "nextValidatorSetHash"));
    writeVector(out, nextValidatorSetPayload);
    write(out, hex32Bytes(nextValidatorSetPayloadHash, "nextValidatorSetPayloadHash"));
    write(out, hex32Bytes(nextValidatorSetConfigHash, "nextValidatorSetConfigHash"));
    write(out, transitionMessageHashBytes);
    write(out, parentHashBytes);
    write(out, canonicalValidatorSignatureProofBytes(validatorSignatureProof));
    return out.toByteArray();
  }

  public static String validatorSetTransitionSignatureHash(
      final int version,
      final int sourceDomain,
      final String fromValidatorSetSeqno,
      final String toValidatorSetSeqno,
      final String masterchainSeqno,
      final int masterchainWorkchainId,
      final String masterchainShard,
      final String masterchainBlockHash,
      final String masterchainFileHash,
      final String parentValidatorSetHash,
      final String nextValidatorSetHash,
      final byte[] nextValidatorSetPayload,
      final String nextValidatorSetPayloadHash,
      final String nextValidatorSetConfigHash,
      final String transitionMessageHash,
      final ValidatorSignatureProofInput validatorSignatureProof) {
    return hashHex(
        VALIDATOR_SET_TRANSITION_SIGNATURES_PREFIX_V1,
        canonicalValidatorSetTransitionSignatureBytes(
            version,
            sourceDomain,
            fromValidatorSetSeqno,
            toValidatorSetSeqno,
            masterchainSeqno,
            masterchainWorkchainId,
            masterchainShard,
            masterchainBlockHash,
            masterchainFileHash,
            parentValidatorSetHash,
            nextValidatorSetHash,
            nextValidatorSetPayload,
            nextValidatorSetPayloadHash,
            nextValidatorSetConfigHash,
            transitionMessageHash,
            validatorSignatureProof));
  }

  public static byte[] canonicalShardStateProofPublicInputsBytes(
      final ShardStateProofRequestInput input) {
    final NormalizedShardStateSourceStateInput normalized =
        normalizeShardStateSourceStateInput(input);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(normalized.version);
    writeU32Le(out, normalized.sourceDomain);
    writeU64Le(out, normalized.masterchainSeqno);
    writeI32Le(out, normalized.masterchainWorkchainId);
    writeU64Le(out, normalized.masterchainShard);
    write(out, hex32Bytes(normalized.masterchainBlockHash, "masterchainBlockHash"));
    write(out, hex32Bytes(normalized.masterchainFileHash, "masterchainFileHash"));
    write(out, hex32Bytes(normalized.validatorSetHash, "validatorSetHash"));
    write(out, hex32Bytes(normalized.masterchainConfigRoot, "masterchainConfigRoot"));
    write(out, hex32Bytes(normalized.masterchainConfigProofHash, "masterchainConfigProofHash"));
    writeI32Le(out, normalized.shardWorkchainId);
    writeU64Le(out, normalized.shardShard);
    writeU64Le(out, normalized.shardSeqno);
    write(out, hex32Bytes(normalized.shardBlockHash, "shardBlockHash"));
    write(out, hex32Bytes(normalized.shardFileHash, "shardFileHash"));
    write(out, hex32Bytes(normalized.shardStateRoot, "shardStateRoot"));
    write(out, hex32Bytes(normalized.transactionRoot, "transactionRoot"));
    writeU64Le(out, normalized.transactionLt);
    write(out, hex32Bytes(normalized.shardStateDictionaryRoot, "shardStateDictionaryRoot"));
    writeU16Le(out, normalized.shardStateDictionaryKeyBitLen);
    writeVector(out, normalized.shardStateDictionaryKey);
    write(out, hex32Bytes(normalized.masterchainSignatureHash, "masterchainSignatureHash"));
    write(out, hex32Bytes(normalized.shardProofHash, "shardProofHash"));
    write(out, hex32Bytes(normalized.shardStateProofBocHash, "shardStateProofBocHash"));
    write(out, hex32Bytes(normalized.shardAccountsProofBocHash, "shardAccountsProofBocHash"));
    write(out, hex32Bytes(normalized.configProofBocHash, "configProofBocHash"));
    write(out, hex32Bytes(normalized.transitionChainHash, "transitionChainHash"));
    return out.toByteArray();
  }

  public static String shardStateProofPublicInputsHash(
      final ShardStateProofRequestInput input) {
    return hashHex(
        SHARD_STATE_PROOF_PUBLIC_INPUTS_PREFIX_V1,
        canonicalShardStateProofPublicInputsBytes(input));
  }

  public static byte[] canonicalShardStateWitnessCommitmentBytes(
      final ShardStateProofRequestInput input) {
    final NormalizedShardStateSourceStateInput normalized =
        normalizeShardStateSourceStateInput(input);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(normalized.version);
    writeVector(out, normalized.shardStateProofBoc);
    writeVector(out, normalized.shardStateDictionaryProofBoc);
    writeVector(out, normalized.configDictionaryProofBoc);
    writeU32Le(out, normalized.validatorSetTransitionProofs.size());
    for (final NormalizedValidatorSetTransitionProof transition :
        normalized.validatorSetTransitionProofs) {
      write(out, canonicalValidatorSetTransitionProofBytes(transition));
    }
    return out.toByteArray();
  }

  public static byte[] canonicalShardStateVerificationContextBytes(
      final ShardStateProofRequestInput input) {
    final NormalizedShardStateSourceStateInput normalized =
        normalizeShardStateSourceStateInput(input);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(normalized.version);
    writeString(out, normalized.sourceStateVerifierId, "sourceStateVerifierId");
    write(out, hex32Bytes(normalized.sourceStateVerifierHash, "sourceStateVerifierHash"));
    writeString(out, normalized.sourceTrustAnchorId, "sourceTrustAnchorId");
    write(out, hex32Bytes(normalized.sourceTrustAnchorHash, "sourceTrustAnchorHash"));
    writeString(out, normalized.consensusVerifierId, "consensusVerifierId");
    write(out, hex32Bytes(normalized.consensusVerifierHash, "consensusVerifierHash"));
    writeString(out, normalized.messageInclusionVerifierId, "messageInclusionVerifierId");
    write(out, hex32Bytes(normalized.messageInclusionVerifierHash, "messageInclusionVerifierHash"));
    writeString(out, normalized.finalityPolicyId, "finalityPolicyId");
    write(out, hex32Bytes(normalized.finalityPolicyHash, "finalityPolicyHash"));
    return out.toByteArray();
  }

  public static List<List<String>> shardStatePublicInputColumns(
      final ShardStateProofRequestInput input) {
    final NormalizedShardStateSourceStateInput normalized =
        normalizeShardStateSourceStateInput(input);
    final String publicInputsHash = shardStateProofPublicInputsHash(input);
    final ArrayList<List<String>> columns = new ArrayList<List<String>>();
    columns.add(Collections.singletonList("0x" + hexLower(sccpWordU32Le(normalized.sourceDomain))));
    columns.add(Collections.singletonList("0x" + hexLower(sccpWordU64Le(normalized.masterchainSeqno))));
    columns.add(
        Collections.singletonList("0x" + hexLower(sccpWordI32Le(normalized.masterchainWorkchainId))));
    columns.add(Collections.singletonList("0x" + hexLower(sccpWordU64Le(normalized.masterchainShard))));
    columns.add(Collections.singletonList(normalized.masterchainBlockHash));
    columns.add(Collections.singletonList(normalized.validatorSetHash));
    columns.add(Collections.singletonList(normalized.masterchainConfigRoot));
    columns.add(Collections.singletonList("0x" + hexLower(sccpWordI32Le(normalized.shardWorkchainId))));
    columns.add(Collections.singletonList("0x" + hexLower(sccpWordU64Le(normalized.shardShard))));
    columns.add(Collections.singletonList("0x" + hexLower(sccpWordU64Le(normalized.shardSeqno))));
    columns.add(Collections.singletonList(normalized.shardBlockHash));
    columns.add(Collections.singletonList(normalized.shardStateRoot));
    columns.add(Collections.singletonList(normalized.shardStateDictionaryRoot));
    columns.add(Collections.singletonList(normalized.transactionRoot));
    columns.add(Collections.singletonList("0x" + hexLower(sccpWordU64Le(normalized.transactionLt))));
    columns.add(Collections.singletonList(publicInputsHash));
    return Collections.unmodifiableList(columns);
  }

  public static byte[] shardStateOpenVerifySchemaDescriptor(
      final ShardStateProofRequestInput input) {
    final NormalizedShardStateSourceStateInput normalized =
        normalizeShardStateSourceStateInput(input);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(normalized.version);
    writeString(out, SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1, "circuitId");
    writeString(out, SHARD_STATE_FASTPQ_PARAMETER_SET_V1, "parameterSet");
    writeI32Le(out, TON_MAINNET_GLOBAL_ID);
    writeU32Le(out, normalized.sourceDomain);
    final String[] requiredInputs = {
      "source_domain",
      "masterchain_seqno",
      "masterchain_workchain_id",
      "masterchain_shard",
      "masterchain_block_hash",
      "validator_set_hash",
      "masterchain_config_root",
      "shard_workchain_id",
      "shard_shard",
      "shard_seqno",
      "shard_block_hash",
      "shard_state_root",
      "shard_state_dictionary_root",
      "transaction_root",
      "transaction_lt",
      "shard_state_proof_public_inputs_hash"
    };
    for (final String requiredInput : requiredInputs) {
      writeString(out, requiredInput, "requiredInput");
    }
    return out.toByteArray();
  }

  public static ShardStateProofRequest buildShardStateProofRequest(
      final ShardStateProofRequestInput input) {
    final NormalizedShardStateSourceStateInput normalized =
        normalizeShardStateSourceStateInput(input);
    final byte[] statementBytes = canonicalShardStateProofPublicInputsBytes(input);
    final byte[] witnessCommitmentBytes = canonicalShardStateWitnessCommitmentBytes(input);
    final byte[] verificationContextBytes = canonicalShardStateVerificationContextBytes(input);
    final String publicInputsHash = shardStateProofPublicInputsHash(input);
    final byte[] dsidHash =
        prefixedHashBytes(
            SHARD_STATE_FASTPQ_DSID_PREFIX_V1,
            hex32Bytes(publicInputsHash, "shardStateProofPublicInputsHash"));
    final ArrayList<ShardStateFastpqTransition> transitions =
        new ArrayList<ShardStateFastpqTransition>();
    transitions.add(
        new ShardStateFastpqTransition(
            SHARD_STATE_FASTPQ_STATEMENT_KEY_V1,
            "meta_set",
            "0x",
            "0x" + hexLower(statementBytes)));
    transitions.add(
        new ShardStateFastpqTransition(
            SHARD_STATE_FASTPQ_WITNESS_KEY_V1,
            "meta_set",
            "0x",
            "0x" + hexLower(witnessCommitmentBytes)));
    transitions.add(
        new ShardStateFastpqTransition(
            SHARD_STATE_FASTPQ_CONTEXT_KEY_V1,
            "meta_set",
            "0x",
            "0x" + hexLower(verificationContextBytes)));
    return new ShardStateProofRequest(
        1,
        STARK_FRI_PROOF_FAMILY_V1,
        SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1,
        SHARD_STATE_FASTPQ_PARAMETER_SET_V1,
        normalized.sourceDomain,
        normalized.masterchainSeqno.toString(),
        normalized.shardSeqno.toString(),
        normalized.sourceStateVerifierId,
        normalized.sourceStateVerifierHash,
        publicInputsHash,
        statementBytes,
        witnessCommitmentBytes,
        verificationContextBytes,
        shardStateOpenVerifySchemaDescriptor(input),
        shardStatePublicInputColumns(input),
        new ShardStateFastpqPublicInputs(
            "0x" + hexLower(Arrays.copyOfRange(dsidHash, 0, 16)),
            normalized.masterchainSeqno.toString(),
            normalized.masterchainConfigRoot,
            normalized.shardStateRoot,
            normalized.shardStateDictionaryRoot,
            publicInputsHash),
        transitions);
  }

  public static byte[] canonicalSourceStateVerificationProofBytes(
      final SourceStateVerificationProof proof) {
    Objects.requireNonNull(proof, "proof");
    if (proof.version() != 1) {
      throw new IllegalArgumentException("source-state verification proof version must be 1");
    }
    if (!STARK_FRI_PROOF_FAMILY_V1.equals(proof.proofFamily())) {
      throw new IllegalArgumentException("source-state verification proof family must be stark-fri-v1");
    }
    if (!isSourceStateVerificationCircuitId(proof.circuitId())) {
      throw new IllegalArgumentException(
          "source-state verification proof circuit must be a TON source-state circuit");
    }
    if (proof.proofBytes().length == 0) {
      throw new IllegalArgumentException("source-state verification proof bytes must not be empty");
    }
    if (proof.proofBytes().length > NATIVE_RECURSIVE_MAX_PROOF_BYTES) {
      throw new IllegalArgumentException(
          "proofBytes must be at most " + NATIVE_RECURSIVE_MAX_PROOF_BYTES + " bytes");
    }
    if (!containsNonZero(proof.proofBytes())) {
      throw new IllegalArgumentException("source-state verification proof bytes must not be all zero");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(proof.version());
    writeString(out, proof.proofFamily(), "proofFamily");
    writeString(out, proof.circuitId(), "circuitId");
    writeVector(out, proof.proofBytes());
    return out.toByteArray();
  }

  public static String shardStateVerificationProofHash(
      final SourceStateVerificationProof proof) {
    if (!SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1.equals(proof.circuitId())) {
      throw new IllegalArgumentException(
          "shardStateVerificationProof must be the TON shard-state stark-fri-v1 proof");
    }
    return hashHex(
        "sccp:ton:source-state-verification-proof:v1",
        canonicalSourceStateVerificationProofBytes(proof));
  }

  public static SourceStateVerificationProof wrapSourceStateVerificationProof(
      final byte[] proofBytes, final ShardStateProofRequest request) {
    Objects.requireNonNull(request, "request");
    requireSourceStateProofRequestForWrapping(request);
    return wrapSourceStateVerificationProof(
        proofBytes,
        request.version(),
        request.proofFamily(),
        request.circuitId(),
        request.sourceDomain());
  }

  public static SourceStateVerificationProof wrapSourceStateVerificationProof(
      final byte[] proofBytes, final FullLightClientAuditProofRequest request) {
    Objects.requireNonNull(request, "request");
    requireSourceStateProofRequestForWrapping(request);
    return wrapSourceStateVerificationProof(
        proofBytes,
        request.version(),
        request.proofFamily(),
        request.circuitId(),
        request.sourceDomain());
  }

  private static void requireSourceStateProofRequestForProverCallback(
      final ShardStateProofRequest request) {
    Objects.requireNonNull(request, "request");
    requireSourceStateProofRequestForWrapping(request);
  }

  private static void requireSourceStateProofRequestForProverCallback(
      final FullLightClientAuditProofRequest request) {
    Objects.requireNonNull(request, "request");
    requireSourceStateProofRequestForWrapping(request);
  }

  private static void requireSourceStateProofRequestForWrapping(
      final ShardStateProofRequest request) {
    if (request.version() != 1) {
      throw new IllegalArgumentException("TON source-state proof request.version must be 1");
    }
    if (!STARK_FRI_PROOF_FAMILY_V1.equals(request.proofFamily())) {
      throw new IllegalArgumentException(
          "TON source-state proof request.proofFamily must be stark-fri-v1");
    }
    if (!SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1.equals(request.circuitId())) {
      throw new IllegalArgumentException(
          "request.circuitId must be the TON shard-state OpenVerify circuit");
    }
    if (!SHARD_STATE_FASTPQ_PARAMETER_SET_V1.equals(request.parameterSet())) {
      throw new IllegalArgumentException("request.parameterSet must be fastpq-lane-balanced");
    }
    if (request.sourceDomain() != DOMAIN_TON) {
      throw new IllegalArgumentException(
          "TON source-state proof request.sourceDomain must be TON");
    }
    if (BigInteger.ZERO.equals(normalizeU64(request.masterchainSeqno(), "request.masterchainSeqno"))) {
      throw new IllegalArgumentException("request.masterchainSeqno must not be zero");
    }
    if (BigInteger.ZERO.equals(normalizeU64(request.shardSeqno(), "request.shardSeqno"))) {
      throw new IllegalArgumentException("request.shardSeqno must not be zero");
    }
    if (!MAINNET_SHARD_STATE_VERIFIER_ID_V1.equals(request.sourceStateVerifierId())) {
      throw new IllegalArgumentException(
          "request.sourceStateVerifierId must match TON shard-state verifier profile");
    }
    final byte[] sourceStateVerifierHash =
        nonZeroHex32Bytes(request.sourceStateVerifierHash(), "request.sourceStateVerifierHash");
    if (Arrays.equals(sourceStateVerifierHash, TEMPLATE_SHARD_STATE_VERIFIER_HASH)) {
      throw new IllegalArgumentException(
          "request.sourceStateVerifierHash must not be the TON template verifier hash");
    }
    final String derivedPublicInputsHash =
        hashHex(SHARD_STATE_PROOF_PUBLIC_INPUTS_PREFIX_V1, request.statementBytes());
    if (!normalizeNonZeroHex32(
            request.shardStateProofPublicInputsHash(),
            "request.shardStateProofPublicInputsHash")
        .equals(derivedPublicInputsHash)) {
      throw new IllegalArgumentException(
          "request.shardStateProofPublicInputsHash must match request.statementBytes");
    }
    final byte[] shardDsidHash =
        prefixedHashBytes(
            SHARD_STATE_FASTPQ_DSID_PREFIX_V1,
            hex32Bytes(derivedPublicInputsHash, "request.shardStateProofPublicInputsHash"));
    if (!request.fastpqPublicInputs().dsid().equals(
        "0x" + hexLower(Arrays.copyOfRange(shardDsidHash, 0, 16)))) {
      throw new IllegalArgumentException(
          "request.fastpqPublicInputs.dsid must match request.statementBytes");
    }
    if (!normalizeNonZeroHex32(
            request.fastpqPublicInputs().txSetHash(),
            "request.fastpqPublicInputs.txSetHash")
        .equals(derivedPublicInputsHash)) {
      throw new IllegalArgumentException(
          "request.fastpqPublicInputs.txSetHash must match request.statementBytes");
    }
    requireTonOpenVerifyRequestPayloadForWrapping(
        request.statementBytes(),
        request.witnessCommitmentBytes(),
        request.verificationContextBytes(),
        request.schemaDescriptor(),
        request.publicInputColumns(),
        new String[] {
          request.fastpqPublicInputs().dsid(),
          request.fastpqPublicInputs().slot(),
          request.fastpqPublicInputs().oldRoot(),
          request.fastpqPublicInputs().newRoot(),
          request.fastpqPublicInputs().permRoot(),
          request.fastpqPublicInputs().txSetHash()
        },
        tonTransitionChecks(request.fastpqTransitions()),
        tonShardStateExpectedTransitionChecks(
            request.statementBytes(),
            request.witnessCommitmentBytes(),
            request.verificationContextBytes()));
  }

  private static void requireSourceStateProofRequestForWrapping(
      final FullLightClientAuditProofRequest request) {
    if (request.version() != 1) {
      throw new IllegalArgumentException("TON source-state proof request.version must be 1");
    }
    if (!STARK_FRI_PROOF_FAMILY_V1.equals(request.proofFamily())) {
      throw new IllegalArgumentException(
          "TON source-state proof request.proofFamily must be stark-fri-v1");
    }
    if (!FULL_LIGHT_CLIENT_AUDIT_FASTPQ_PARAMETER_SET_V1.equals(request.parameterSet())) {
      throw new IllegalArgumentException("request.parameterSet must be fastpq-lane-balanced");
    }
    if (request.sourceDomain() != DOMAIN_TON) {
      throw new IllegalArgumentException(
          "TON source-state proof request.sourceDomain must be TON");
    }
    final AuditRoleProfile profile = auditRoleProfileForRequest(request.role());
    if (request.roleCode() != profile.code) {
      throw new IllegalArgumentException("request.roleCode must match request.role");
    }
    if (!profile.circuitId.equals(request.circuitId())) {
      throw new IllegalArgumentException("request.circuitId must match request.role");
    }
    if (!profile.verifierId.equals(request.verifierId())) {
      throw new IllegalArgumentException("request.verifierId must match request.role");
    }
    if (BigInteger.ZERO.equals(normalizeU64(request.masterchainSeqno(), "request.masterchainSeqno"))) {
      throw new IllegalArgumentException("request.masterchainSeqno must not be zero");
    }
    if (BigInteger.ZERO.equals(normalizeU64(request.shardSeqno(), "request.shardSeqno"))) {
      throw new IllegalArgumentException("request.shardSeqno must not be zero");
    }
    normalizeNonZeroHex32(request.verifierHash(), "request.verifierHash");
    if (!MAINNET_SHARD_STATE_VERIFIER_ID_V1.equals(request.sourceStateVerifierId())) {
      throw new IllegalArgumentException(
          "request.sourceStateVerifierId must match TON shard-state verifier profile");
    }
    final byte[] sourceStateVerifierHash =
        nonZeroHex32Bytes(request.sourceStateVerifierHash(), "request.sourceStateVerifierHash");
    if (Arrays.equals(sourceStateVerifierHash, TEMPLATE_SHARD_STATE_VERIFIER_HASH)) {
      throw new IllegalArgumentException(
          "request.sourceStateVerifierHash must not be the TON template verifier hash");
    }
    final String[][] hashes = {
      {"request.sourceVerifierMaterialHash", request.sourceVerifierMaterialHash()},
      {"request.sourceAdapterDeploymentHash", request.sourceAdapterDeploymentHash()},
      {"request.fullLightClientGateHash", request.fullLightClientGateHash()},
      {"request.shardStateProofPublicInputsHash", request.shardStateProofPublicInputsHash()},
      {"request.shardStateVerificationProofHash", request.shardStateVerificationProofHash()},
      {"request.auditStatementHash", request.auditStatementHash()}
    };
    for (final String[] hash : hashes) {
      normalizeNonZeroHex32(hash[1], hash[0]);
    }
    final String normalizedAuditStatementHash =
        normalizeNonZeroHex32(request.auditStatementHash(), "request.auditStatementHash");
    final String derivedAuditStatementHash =
        hashHex(FULL_LIGHT_CLIENT_AUDIT_STATEMENT_PREFIX_V1, request.statementBytes());
    if (!normalizedAuditStatementHash.equals(derivedAuditStatementHash)) {
      throw new IllegalArgumentException("request.auditStatementHash must match request.statementBytes");
    }
    final ByteArrayOutputStream auditDsidPreimage = new ByteArrayOutputStream();
    auditDsidPreimage.write(profile.code);
    write(auditDsidPreimage, hex32Bytes(normalizedAuditStatementHash, "request.auditStatementHash"));
    final byte[] auditDsidHash =
        prefixedHashBytes(
            FULL_LIGHT_CLIENT_AUDIT_FASTPQ_DSID_PREFIX_V1,
            auditDsidPreimage.toByteArray());
    if (!request.fastpqPublicInputs().dsid().equals(
        "0x" + hexLower(Arrays.copyOfRange(auditDsidHash, 0, 16)))) {
      throw new IllegalArgumentException(
          "request.fastpqPublicInputs.dsid must match request.statementBytes");
    }
    if (!normalizeNonZeroHex32(
            request.fastpqPublicInputs().txSetHash(),
            "request.fastpqPublicInputs.txSetHash")
        .equals(derivedAuditStatementHash)) {
      throw new IllegalArgumentException(
          "request.fastpqPublicInputs.txSetHash must match request.statementBytes");
    }
    requireTonOpenVerifyRequestPayloadForWrapping(
        request.statementBytes(),
        null,
        request.verificationContextBytes(),
        request.schemaDescriptor(),
        request.publicInputColumns(),
        new String[] {
          request.fastpqPublicInputs().dsid(),
          request.fastpqPublicInputs().slot(),
          request.fastpqPublicInputs().oldRoot(),
          request.fastpqPublicInputs().newRoot(),
          request.fastpqPublicInputs().permRoot(),
          request.fastpqPublicInputs().txSetHash()
        },
        tonAuditTransitionChecks(request.fastpqTransitions()),
        tonAuditExpectedTransitionChecks(
            profile,
            request.statementBytes(),
            request.verificationContextBytes(),
            normalizeNonZeroHex32(
                request.fullLightClientGateHash(), "request.fullLightClientGateHash")));
  }

  private static AuditRoleProfile auditRoleProfileForRequest(final String role) {
    switch (normalizeNonEmpty(role, "request.role")) {
      case "masterchainConfig":
      case "masterchain_config":
        return auditRoleProfile(FullLightClientAuditRole.MASTERCHAIN_CONFIG);
      case "validatorSetTransition":
      case "validator_set_transition":
        return auditRoleProfile(FullLightClientAuditRole.VALIDATOR_SET_TRANSITION);
      case "shardAccountsDictionary":
      case "shard_accounts_dictionary":
        return auditRoleProfile(FullLightClientAuditRole.SHARD_ACCOUNTS_DICTIONARY);
      default:
        throw new IllegalArgumentException(
            "request.role must be masterchain_config, validator_set_transition, or shard_accounts_dictionary");
    }
  }

  private record TonSourceStateTransitionCheck(
      String key,
      String operation,
      String oldValue,
      String newValue) {}

  private static List<TonSourceStateTransitionCheck> tonTransitionChecks(
      final List<ShardStateFastpqTransition> transitions) {
    final ArrayList<TonSourceStateTransitionCheck> checks = new ArrayList<TonSourceStateTransitionCheck>();
    for (final ShardStateFastpqTransition transition : transitions) {
      checks.add(new TonSourceStateTransitionCheck(
          transition.key(), transition.operation(), transition.oldValue(), transition.newValue()));
    }
    return checks;
  }

  private static List<TonSourceStateTransitionCheck> tonAuditTransitionChecks(
      final List<FullLightClientAuditFastpqTransition> transitions) {
    final ArrayList<TonSourceStateTransitionCheck> checks = new ArrayList<TonSourceStateTransitionCheck>();
    for (final FullLightClientAuditFastpqTransition transition : transitions) {
      checks.add(new TonSourceStateTransitionCheck(
          transition.key(), transition.operation(), transition.oldValue(), transition.newValue()));
    }
    return checks;
  }

  private static List<TonSourceStateTransitionCheck> tonShardStateExpectedTransitionChecks(
      final byte[] statementBytes,
      final byte[] witnessCommitmentBytes,
      final byte[] verificationContextBytes) {
    final ArrayList<TonSourceStateTransitionCheck> expected =
        new ArrayList<TonSourceStateTransitionCheck>();
    expected.add(
        new TonSourceStateTransitionCheck(
            SHARD_STATE_FASTPQ_STATEMENT_KEY_V1,
            "meta_set",
            "0x",
            "0x" + hexLower(statementBytes)));
    expected.add(
        new TonSourceStateTransitionCheck(
            SHARD_STATE_FASTPQ_WITNESS_KEY_V1,
            "meta_set",
            "0x",
            "0x" + hexLower(witnessCommitmentBytes)));
    expected.add(
        new TonSourceStateTransitionCheck(
            SHARD_STATE_FASTPQ_CONTEXT_KEY_V1,
            "meta_set",
            "0x",
            "0x" + hexLower(verificationContextBytes)));
    return expected;
  }

  private static List<TonSourceStateTransitionCheck> tonAuditExpectedTransitionChecks(
      final AuditRoleProfile profile,
      final byte[] statementBytes,
      final byte[] verificationContextBytes,
      final String fullLightClientGateHash) {
    final ArrayList<TonSourceStateTransitionCheck> expected =
        new ArrayList<TonSourceStateTransitionCheck>();
    expected.add(
        new TonSourceStateTransitionCheck(
            "0x"
                + hexLower(
                    fullLightClientAuditFastpqKey(
                        FULL_LIGHT_CLIENT_AUDIT_FASTPQ_STATEMENT_KEY_V1, profile)),
            "meta_set",
            "0x",
            "0x" + hexLower(statementBytes)));
    expected.add(
        new TonSourceStateTransitionCheck(
            "0x"
                + hexLower(
                    fullLightClientAuditFastpqKey(
                        FULL_LIGHT_CLIENT_AUDIT_FASTPQ_CONTEXT_KEY_V1, profile)),
            "meta_set",
            "0x",
            "0x" + hexLower(verificationContextBytes)));
    expected.add(
        new TonSourceStateTransitionCheck(
            "0x"
                + hexLower(
                    fullLightClientAuditFastpqKey(
                        FULL_LIGHT_CLIENT_AUDIT_FASTPQ_GATE_KEY_V1, profile)),
            "meta_set",
            "0x",
            fullLightClientGateHash));
    return expected;
  }

  private static void requireTonOpenVerifyRequestPayloadForWrapping(
      final byte[] statementBytes,
      final byte[] witnessCommitmentBytes,
      final byte[] verificationContextBytes,
      final byte[] schemaDescriptor,
      final List<List<String>> publicInputColumns,
      final String[] fastpqFields,
      final List<TonSourceStateTransitionCheck> transitions,
      final List<TonSourceStateTransitionCheck> expectedTransitions) {
    if (statementBytes.length == 0) {
      throw new IllegalArgumentException("request.statementBytes must not be empty");
    }
    if (witnessCommitmentBytes != null && witnessCommitmentBytes.length == 0) {
      throw new IllegalArgumentException("request.witnessCommitmentBytes must not be empty");
    }
    if (verificationContextBytes.length == 0) {
      throw new IllegalArgumentException("request.verificationContextBytes must not be empty");
    }
    if (schemaDescriptor.length == 0) {
      throw new IllegalArgumentException("request.schemaDescriptor must not be empty");
    }
    if (publicInputColumns == null || publicInputColumns.isEmpty()) {
      throw new IllegalArgumentException("request.publicInputColumns is required");
    }
    for (int index = 0; index < publicInputColumns.size(); index++) {
      final List<String> column = publicInputColumns.get(index);
      if (column == null || column.isEmpty()) {
        throw new IllegalArgumentException(
            "request.publicInputColumns[" + index + "] must not be empty");
      }
      for (int valueIndex = 0; valueIndex < column.size(); valueIndex++) {
        normalizeNonEmpty(
            column.get(valueIndex),
            "request.publicInputColumns[" + index + "][" + valueIndex + "]");
      }
    }
    for (int index = 0; index < fastpqFields.length; index++) {
      normalizeNonEmpty(fastpqFields[index], "request.fastpqPublicInputs[" + index + "]");
    }
    if (transitions == null || transitions.isEmpty()) {
      throw new IllegalArgumentException("request.fastpqTransitions is required");
    }
    for (int index = 0; index < transitions.size(); index++) {
      final TonSourceStateTransitionCheck transition = transitions.get(index);
      normalizeNonEmpty(transition.key(), "request.fastpqTransitions[" + index + "].key");
      normalizeNonEmpty(transition.operation(), "request.fastpqTransitions[" + index + "].operation");
      normalizeNonEmpty(transition.oldValue(), "request.fastpqTransitions[" + index + "].oldValue");
      normalizeNonEmpty(transition.newValue(), "request.fastpqTransitions[" + index + "].newValue");
    }
    final ArrayList<TonSourceStateTransitionCheck> actual =
        new ArrayList<TonSourceStateTransitionCheck>(transitions);
    final ArrayList<TonSourceStateTransitionCheck> expected =
        new ArrayList<TonSourceStateTransitionCheck>(expectedTransitions);
    final Comparator<TonSourceStateTransitionCheck> byKey =
        new Comparator<TonSourceStateTransitionCheck>() {
          @Override
          public int compare(
              final TonSourceStateTransitionCheck left,
              final TonSourceStateTransitionCheck right) {
            return left.key().compareTo(right.key());
          }
        };
    Collections.sort(actual, byKey);
    Collections.sort(expected, byKey);
    if (actual.size() != expected.size()) {
      throw new IllegalArgumentException(
          "request.fastpqTransitions must match the canonical TON source-state request");
    }
    for (int index = 0; index < expected.size(); index++) {
      final TonSourceStateTransitionCheck actualTransition = actual.get(index);
      final TonSourceStateTransitionCheck expectedTransition = expected.get(index);
      if (!actualTransition.key().equals(expectedTransition.key())
          || !actualTransition.operation().equals(expectedTransition.operation())
          || !actualTransition.oldValue().equals(expectedTransition.oldValue())
          || !actualTransition.newValue().equals(expectedTransition.newValue())) {
        throw new IllegalArgumentException(
            "request.fastpqTransitions must match the canonical TON source-state request");
      }
    }
  }

  private static SourceStateVerificationProof wrapSourceStateVerificationProof(
      final byte[] proofBytes,
      final int version,
      final String proofFamily,
      final String circuitId,
      final int sourceDomain) {
    final byte[] normalizedProofBytes =
        Arrays.copyOf(Objects.requireNonNull(proofBytes, "proofBytes"), proofBytes.length);
    if (version != 1) {
      throw new IllegalArgumentException("sourceStateProof.version must be 1");
    }
    if (!STARK_FRI_PROOF_FAMILY_V1.equals(proofFamily)) {
      throw new IllegalArgumentException("sourceStateProof.proofFamily must be stark-fri-v1");
    }
    if (sourceDomain != DOMAIN_TON) {
      throw new IllegalArgumentException("sourceStateProof.sourceDomain must be TON");
    }
    if (!isSourceStateVerificationCircuitId(circuitId)) {
      throw new IllegalArgumentException(
          "sourceStateProof.circuitId must be a TON source-state verification circuit");
    }
    if (normalizedProofBytes.length == 0) {
      throw new IllegalArgumentException("proofBytes must not be empty");
    }
    if (normalizedProofBytes.length > NATIVE_RECURSIVE_MAX_PROOF_BYTES) {
      throw new IllegalArgumentException(
          "proofBytes must be at most " + NATIVE_RECURSIVE_MAX_PROOF_BYTES + " bytes");
    }
    if (!containsNonZero(normalizedProofBytes)) {
      throw new IllegalArgumentException("proofBytes must not be all zero");
    }
    return new SourceStateVerificationProof(version, proofFamily, circuitId, normalizedProofBytes);
  }

  private static boolean isSourceStateVerificationCircuitId(final String circuitId) {
    return SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1.equals(circuitId)
        || MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1.equals(circuitId)
        || VALIDATOR_SET_TRANSITION_OPEN_VERIFY_CIRCUIT_ID_V1.equals(circuitId)
        || SHARD_ACCOUNTS_DICTIONARY_OPEN_VERIFY_CIRCUIT_ID_V1.equals(circuitId);
  }

  public static byte[] canonicalFullLightClientAuditStatementBytes(
      final FullLightClientAuditProofInput input,
      final FullLightClientAuditRole role) {
    final NormalizedFullLightClientAuditInput normalized =
        normalizeFullLightClientAuditInput(input, role);
    final AuditRoleProfile profile = auditRoleProfile(role);
    final NormalizedShardStateSourceStateInput shardState = normalized.shardState;
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    out.write(profile.code);
    writeString(out, profile.circuitId, "circuitId");
    writeString(out, CONTRACT_PROOF_BACKEND_V1, "backend");
    writeI32Le(out, TON_MAINNET_GLOBAL_ID);
    writeU32Le(out, shardState.sourceDomain);
    writeU64Le(out, shardState.masterchainSeqno);
    writeI32Le(out, shardState.masterchainWorkchainId);
    writeU64Le(out, shardState.masterchainShard);
    write(out, hex32Bytes(shardState.masterchainBlockHash, "masterchainBlockHash"));
    write(out, hex32Bytes(shardState.masterchainFileHash, "masterchainFileHash"));
    write(out, hex32Bytes(shardState.validatorSetHash, "validatorSetHash"));
    write(out, hex32Bytes(shardState.masterchainConfigRoot, "masterchainConfigRoot"));
    write(out, hex32Bytes(shardState.masterchainConfigProofHash, "masterchainConfigProofHash"));
    writeI32Le(out, shardState.shardWorkchainId);
    writeU64Le(out, shardState.shardShard);
    writeU64Le(out, shardState.shardSeqno);
    write(out, hex32Bytes(shardState.shardBlockHash, "shardBlockHash"));
    write(out, hex32Bytes(shardState.shardFileHash, "shardFileHash"));
    write(out, hex32Bytes(shardState.shardStateRoot, "shardStateRoot"));
    write(out, hex32Bytes(shardState.shardStateDictionaryRoot, "shardStateDictionaryRoot"));
    write(out, hex32Bytes(shardState.transactionRoot, "transactionRoot"));
    writeU64Le(out, shardState.transactionLt);
    write(out, hex32Bytes(shardState.masterchainSignatureHash, "masterchainSignatureHash"));
    write(out, hex32Bytes(shardState.shardProofHash, "shardProofHash"));
    write(out, hex32Bytes(normalized.shardStateVerificationProofHash, "shardStateVerificationProofHash"));
    write(out, hex32Bytes(normalized.shardStateProofPublicInputsHash, "shardStateProofPublicInputsHash"));
    switch (role) {
      case MASTERCHAIN_CONFIG:
        write(out, hex32Bytes(normalized.validatorSetPayloadHash, "validatorSetPayloadHash"));
        write(out, hex32Bytes(normalized.configLeafHash, "configLeafHash"));
        write(out, hex32Bytes(normalized.configValueHash, "configValueHash"));
        write(out, hex32Bytes(shardState.configProofBocHash, "configProofBocHash"));
        break;
      case VALIDATOR_SET_TRANSITION:
        write(out, hex32Bytes(shardState.transitionChainHash, "transitionChainHash"));
        writeU32Le(out, shardState.validatorSetTransitionProofs.size());
        for (final NormalizedValidatorSetTransitionProof transition :
            shardState.validatorSetTransitionProofs) {
          write(out, canonicalValidatorSetTransitionProofBytes(transition));
        }
        break;
      case SHARD_ACCOUNTS_DICTIONARY:
        write(out, hex32Bytes(shardState.shardStateProofBocHash, "shardStateProofBocHash"));
        write(out, hex32Bytes(shardState.shardAccountsProofBocHash, "shardAccountsProofBocHash"));
        writeU16Le(out, shardState.shardStateDictionaryKeyBitLen);
        writeVector(out, shardState.shardStateDictionaryKey);
        write(out, hex32Bytes(normalized.shardStateProofPublicInputsHash, "shardStateProofPublicInputsHash"));
        break;
      default:
        throw new IllegalArgumentException("unknown TON full light-client audit role");
    }
    return out.toByteArray();
  }

  public static String fullLightClientAuditStatementHash(
      final FullLightClientAuditProofInput input,
      final FullLightClientAuditRole role) {
    return hashHex(
        FULL_LIGHT_CLIENT_AUDIT_STATEMENT_PREFIX_V1,
        canonicalFullLightClientAuditStatementBytes(input, role));
  }

  public static List<List<String>> fullLightClientAuditPublicInputColumns(
      final FullLightClientAuditProofInput input,
      final FullLightClientAuditRole role) {
    final NormalizedFullLightClientAuditInput normalized =
        normalizeFullLightClientAuditInput(input, role);
    final AuditRoleProfile profile = auditRoleProfile(role);
    final NormalizedShardStateSourceStateInput shardState = normalized.shardState;
    final String statementHash = fullLightClientAuditStatementHash(input, role);
    requireFullLightClientAuditRequestHashSeparation(normalized, statementHash);
    final ArrayList<List<String>> columns = new ArrayList<List<String>>();
    columns.add(Collections.singletonList("0x" + hexLower(sccpWordU8(profile.code))));
    columns.add(Collections.singletonList("0x" + hexLower(sccpWordU32Le(shardState.sourceDomain))));
    columns.add(Collections.singletonList("0x" + hexLower(sccpWordU64Le(shardState.masterchainSeqno))));
    columns.add(Collections.singletonList(shardState.masterchainBlockHash));
    columns.add(Collections.singletonList("0x" + hexLower(sccpWordU64Le(shardState.shardSeqno))));
    columns.add(Collections.singletonList(shardState.shardBlockHash));
    columns.add(Collections.singletonList(statementHash));
    columns.add(Collections.singletonList(normalized.sourceVerifierMaterialHash));
    columns.add(Collections.singletonList(normalized.sourceAdapterDeploymentHash));
    columns.add(Collections.singletonList(normalized.fullLightClientGateHash));
    columns.add(Collections.singletonList(normalized.verifierHash));
    for (final String value : auditRoleColumns(normalized)) {
      columns.add(Collections.singletonList(value));
    }
    return copyColumns(columns);
  }

  public static byte[] fullLightClientAuditOpenVerifySchemaDescriptor(
      final FullLightClientAuditProofInput input,
      final FullLightClientAuditRole role) {
    final NormalizedFullLightClientAuditInput normalized =
        normalizeFullLightClientAuditInput(input, role);
    requireFullLightClientAuditRequestHashSeparation(normalized);
    final AuditRoleProfile profile = auditRoleProfile(role);
    final NormalizedShardStateSourceStateInput shardState = normalized.shardState;
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    out.write(profile.code);
    writeString(out, profile.circuitId, "circuitId");
    writeString(out, FULL_LIGHT_CLIENT_AUDIT_FASTPQ_PARAMETER_SET_V1, "parameterSet");
    writeI32Le(out, TON_MAINNET_GLOBAL_ID);
    writeU32Le(out, shardState.sourceDomain);
    writeString(out, "verifier_id", "schemaField");
    writeString(out, profile.verifierId, "verifierId");
    writeString(out, "verifier_hash", "schemaField");
    write(out, hex32Bytes(normalized.verifierHash, "verifierHash"));
    writeString(out, "source_verifier_material_hash", "schemaField");
    write(out, hex32Bytes(normalized.sourceVerifierMaterialHash, "sourceVerifierMaterialHash"));
    writeString(out, "source_adapter_deployment_hash", "schemaField");
    write(out, hex32Bytes(normalized.sourceAdapterDeploymentHash, "sourceAdapterDeploymentHash"));
    writeString(out, "full_light_client_gate_hash", "schemaField");
    write(out, hex32Bytes(normalized.fullLightClientGateHash, "fullLightClientGateHash"));
    final String[] baseInputs = {
      "role",
      "source_domain",
      "masterchain_seqno",
      "masterchain_block_hash",
      "shard_seqno",
      "shard_block_hash",
      "audit_statement_hash",
      "source_verifier_material_hash",
      "source_adapter_deployment_hash",
      "full_light_client_gate_hash",
      "verifier_hash"
    };
    for (final String requiredInput : baseInputs) {
      writeString(out, requiredInput, "requiredInput");
    }
    for (final String requiredInput : profile.requiredInputNames) {
      writeString(out, requiredInput, "requiredInput");
    }
    return out.toByteArray();
  }

  public static FullLightClientAuditProofRequest buildFullLightClientAuditProofRequest(
      final FullLightClientAuditProofInput input,
      final FullLightClientAuditRole role) {
    final NormalizedFullLightClientAuditInput normalized =
        normalizeFullLightClientAuditInput(input, role);
    final AuditRoleProfile profile = auditRoleProfile(role);
    final NormalizedShardStateSourceStateInput shardState = normalized.shardState;
    final byte[] statementBytes = canonicalFullLightClientAuditStatementBytes(input, role);
    final String auditStatementHash = fullLightClientAuditStatementHash(input, role);
    final byte[] verificationContextBytes =
        canonicalFullLightClientAuditContextBytes(normalized, auditStatementHash);
    final ArrayList<FullLightClientAuditFastpqTransition> transitions =
        new ArrayList<FullLightClientAuditFastpqTransition>();
    transitions.add(
        new FullLightClientAuditFastpqTransition(
            "0x"
                + hexLower(
                    fullLightClientAuditFastpqKey(
                        FULL_LIGHT_CLIENT_AUDIT_FASTPQ_STATEMENT_KEY_V1, profile)),
            "meta_set",
            "0x",
            "0x" + hexLower(statementBytes)));
    transitions.add(
        new FullLightClientAuditFastpqTransition(
            "0x"
                + hexLower(
                    fullLightClientAuditFastpqKey(
                        FULL_LIGHT_CLIENT_AUDIT_FASTPQ_CONTEXT_KEY_V1, profile)),
            "meta_set",
            "0x",
            "0x" + hexLower(verificationContextBytes)));
    transitions.add(
        new FullLightClientAuditFastpqTransition(
            "0x"
                + hexLower(
                    fullLightClientAuditFastpqKey(
                        FULL_LIGHT_CLIENT_AUDIT_FASTPQ_GATE_KEY_V1, profile)),
            "meta_set",
            "0x",
            normalized.fullLightClientGateHash));
    transitions.sort((left, right) -> left.key().compareTo(right.key()));
    return new FullLightClientAuditProofRequest(
        1,
        STARK_FRI_PROOF_FAMILY_V1,
        profile.circuitId,
        FULL_LIGHT_CLIENT_AUDIT_FASTPQ_PARAMETER_SET_V1,
        profile.name,
        profile.code,
        DOMAIN_TON,
        shardState.masterchainSeqno.toString(),
        shardState.shardSeqno.toString(),
        profile.verifierId,
        normalized.verifierHash,
        shardState.sourceStateVerifierId,
        shardState.sourceStateVerifierHash,
        normalized.sourceVerifierMaterialHash,
        normalized.sourceAdapterDeploymentHash,
        normalized.fullLightClientGateHash,
        normalized.shardStateProofPublicInputsHash,
        normalized.shardStateVerificationProofHash,
        auditStatementHash,
        statementBytes,
        verificationContextBytes,
        fullLightClientAuditOpenVerifySchemaDescriptor(input, role),
        fullLightClientAuditPublicInputColumns(input, role),
        fullLightClientAuditFastpqPublicInputs(normalized, auditStatementHash),
        transitions);
  }

  public static FullLightClientAuditProofRequest buildMasterchainConfigProofRequest(
      final FullLightClientAuditProofInput input) {
    return buildFullLightClientAuditProofRequest(input, FullLightClientAuditRole.MASTERCHAIN_CONFIG);
  }

  public static FullLightClientAuditProofRequest buildValidatorSetTransitionProofRequest(
      final FullLightClientAuditProofInput input) {
    return buildFullLightClientAuditProofRequest(input, FullLightClientAuditRole.VALIDATOR_SET_TRANSITION);
  }

  public static FullLightClientAuditProofRequest buildShardAccountsDictionaryProofRequest(
      final FullLightClientAuditProofInput input) {
    return buildFullLightClientAuditProofRequest(input, FullLightClientAuditRole.SHARD_ACCOUNTS_DICTIONARY);
  }

  public static FullLightClientAuditProofRequests buildFullLightClientAuditProofRequests(
      final FullLightClientAuditProofInput input) {
    return new FullLightClientAuditProofRequests(
        buildMasterchainConfigProofRequest(input),
        buildValidatorSetTransitionProofRequest(input),
        buildShardAccountsDictionaryProofRequest(input));
  }

  public static String submissionQueryId(final PublicInputsInput publicInputs) {
    final byte[] messageId = hex32Bytes(publicInputs.messageId(), "messageId");
    BigInteger value = BigInteger.ZERO;
    for (int i = 0; i < 8; i++) {
      value = value.shiftLeft(8).or(BigInteger.valueOf(messageId[i] & 0xffL));
    }
    return value.toString();
  }

  private static String[] normalizeSubmissionDestinationBinding(
      final SubmissionDestinationBindingInput binding, final String field) {
    Objects.requireNonNull(binding, field);
    return new String[] {
      normalizeNonEmpty(binding.key(), field + ".key"),
      normalizeNonZeroHex32(binding.bindingHash(), field + ".bindingHash")
    };
  }

  public static byte[] canonicalSubmissionMetadataBytes(final SubmissionMetadataInput input) {
    Objects.requireNonNull(input, "input");
    final SubmissionManifestInput manifest = Objects.requireNonNull(input.manifest(), "manifest");
    if (manifest.version() != 1) {
      throw new IllegalArgumentException("manifest.version must be 1");
    }
    if (manifest.localDomain() != SolanaSccpProver.DOMAIN_SORA) {
      throw new IllegalArgumentException("manifest.localDomain must be SORA");
    }
    if (manifest.counterpartyDomain() != DOMAIN_TON) {
      throw new IllegalArgumentException("manifest.counterpartyDomain must be TON");
    }
    if (!"RecursiveZk".equals(manifest.securityModel())) {
      throw new IllegalArgumentException("securityModel is unsupported");
    }
    if (!"CryptographicProof".equals(manifest.anchorGovernance())) {
      throw new IllegalArgumentException("anchorGovernance is unsupported");
    }
    if (!"TonContract".equals(manifest.verifierTarget())) {
      throw new IllegalArgumentException("verifierTarget is unsupported");
    }
    if (!"TonContract".equals(manifest.verifierBackendFamily())) {
      throw new IllegalArgumentException("verifierBackendFamily is unsupported");
    }
    if (!STARK_FRI_PROOF_FAMILY_V1.equals(manifest.proofFamily())) {
      throw new IllegalArgumentException("proofFamily must be stark-fri-v1");
    }
    if (!CONTRACT_PROOF_BACKEND_V1.equals(manifest.verifierBackendKey())) {
      throw new IllegalArgumentException("verifierBackendKey must be ton-contract-v1");
    }
    if (input.publicInputs().targetDomain() != DOMAIN_TON) {
      throw new IllegalArgumentException("publicInputs.targetDomain must be TON");
    }
    final SubmissionDestinationBindingInput resolvedBinding =
        input.destinationBinding() != null
            ? input.destinationBinding()
            : manifest.destinationBinding();
    if (resolvedBinding == null) {
      throw new IllegalArgumentException("destinationBinding must be provided");
    }
    final String[] destinationBinding =
        normalizeSubmissionDestinationBinding(resolvedBinding, "destinationBinding");
    if (input.destinationBinding() != null && manifest.destinationBinding() != null) {
      final String[] explicitBinding =
          normalizeSubmissionDestinationBinding(input.destinationBinding(), "destinationBinding");
      final String[] manifestBinding =
          normalizeSubmissionDestinationBinding(
              manifest.destinationBinding(), "manifest.destinationBinding");
      if (!Arrays.equals(explicitBinding, manifestBinding)) {
        throw new IllegalArgumentException(
            "destinationBinding must match manifest.destinationBinding");
      }
    }
    if (input.destinationBindingHash() != null) {
      final String destinationBindingHash =
          normalizeNonZeroHex32(input.destinationBindingHash(), "destinationBindingHash");
      if (!destinationBindingHash.equals(destinationBinding[1])) {
        throw new IllegalArgumentException(
            "destinationBindingHash must match destinationBinding.bindingHash");
      }
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, manifest.localDomain());
    writeU32Le(out, manifest.counterpartyDomain());
    out.write(1);
    out.write(1);
    out.write(3);
    out.write(3);
    writeString(out, manifest.proofFamily(), "proofFamily");
    writeString(out, manifest.verifierBackendKey(), "verifierBackendKey");
    writeString(out, manifest.messageBackend(), "messageBackend");
    writeString(out, manifest.registryBackend(), "registryBackend");
    writeString(out, manifest.manifestSeed(), "manifestSeed");
    writeString(out, destinationBinding[0], "destinationBinding.key");
    write(out, hex32Bytes(destinationBinding[1], "destinationBinding.bindingHash"));
    write(out, nonZeroHex32Bytes(input.statementHash(), "statementHash"));
    write(out, canonicalPublicInputsBytes(input.publicInputs()));
    return out.toByteArray();
  }

  public static byte[] buildMessageBodyBoc(final MessageBodyInput input) {
    Objects.requireNonNull(input, "input");
    final ProofResult proofResult = requireWrappedProofResultForSubmission(input.proofResult());
    if (!Objects.equals(input.publicInputs(), proofResult.publicInputs())) {
      throw new IllegalArgumentException("publicInputs must match proofResult.publicInputs");
    }
    if (!Arrays.equals(input.proofBytes(), proofResult.proofBytes())) {
      throw new IllegalArgumentException("proofBytes must match proofResult.proofBytes");
    }
    if (!Arrays.equals(input.bundleBytes(), proofResult.bundleBytes())) {
      throw new IllegalArgumentException("bundleBytes must match proofResult.bundleBytes");
    }
    if (!Objects.equals(input.statementHash(), proofResult.proofContext().statementHash())) {
      throw new IllegalArgumentException(
          "statementHash must match proofResult.proofContext.statementHash");
    }
    if (!Objects.equals(
        input.destinationBindingHash(), proofResult.proofContext().destinationBindingHash())) {
      throw new IllegalArgumentException(
          "destinationBindingHash must match proofResult.proofContext.destinationBindingHash");
    }
    if (input.publicInputs().targetDomain() != DOMAIN_TON) {
      throw new IllegalArgumentException("publicInputs.targetDomain must be TON");
    }
    final byte[] publicInputsBytes = canonicalPublicInputsBytes(input.publicInputs());
    final byte[] statementHash = nonZeroHex32Bytes(input.statementHash(), "statementHash");
    final byte[] destinationBindingHash =
        nonZeroHex32Bytes(input.destinationBindingHash(), "destinationBindingHash");
    final byte[] proofBytes = input.proofBytes();
    final byte[] bundleBytes = input.bundleBytes();
    final byte[] metadataBytes = input.metadataBytes();
    requireNonZeroProofBytes(proofBytes);
    if (bundleBytes.length == 0) {
      throw new IllegalArgumentException("bundleBytes must not be empty");
    }
    final ByteArrayOutputStream rootData = new ByteArrayOutputStream();
    writeU32Be(rootData, SUBMIT_OP_V1);
    writeU64Be(
        rootData,
        normalizeU64(
            input.queryId() == null ? submissionQueryId(input.publicInputs()) : input.queryId(),
            "queryId"));
    writeU16Be(rootData, MESSAGE_SCHEMA_VERSION_V1);
    write(rootData, statementHash);
    write(rootData, destinationBindingHash);

    final List<TonCell> cells = new ArrayList<>();
    cells.add(new TonCell(rootData.toByteArray(), new ArrayList<>()));
    final int publicInputsRoot = pushSnakeCells(cells, publicInputsBytes);
    final int proofRoot = pushSnakeCells(cells, proofBytes);
    final int bundleRoot = pushSnakeCells(cells, bundleBytes);
    final int metadataRoot = pushSnakeCells(cells, metadataBytes);
    cells.get(0).refs.add(publicInputsRoot);
    cells.get(0).refs.add(proofRoot);
    cells.get(0).refs.add(bundleRoot);
    cells.get(0).refs.add(metadataRoot);
    return encodeBocSingleRoot(cells, 0);
  }

  public static Submission buildSubmission(final MessageBodyInput input) {
    final byte[] messageBodyBoc = buildMessageBodyBoc(input);
    final String messageBodyBocHex = "0x" + hexLower(messageBodyBoc);
    return new Submission(
        1,
        MESSAGE_BODY_BOC_V1,
        "internal_message",
        "op::submit_sccp_message_proof",
        Arrays.copyOf(messageBodyBoc, messageBodyBoc.length),
        messageBodyBocHex,
        Collections.singletonList(
            new SubmissionArgument("message_body_boc", "ton_boc", messageBodyBocHex)),
        Arrays.copyOf(messageBodyBoc, messageBodyBoc.length),
        messageBodyBocHex);
  }

  public static ProofRequest buildProofRequest(final ProofRequestInput input) {
    Objects.requireNonNull(input, "input");
    if (input.sourceDomain() != DOMAIN_TON) {
      throw new IllegalArgumentException("TON SCCP proof request sourceDomain must be TON");
    }
    if (!CONTRACT_PROOF_BACKEND_V1.equals(input.backend())) {
      throw new IllegalArgumentException("TON SCCP proof request backend must be ton-contract-v1");
    }
    if (input.publicInputs().targetDomain() != DOMAIN_TON) {
      throw new IllegalArgumentException("publicInputs.targetDomain must be TON");
    }
    if (input.bundleBytes().length == 0) {
      throw new IllegalArgumentException("bundleBytes must not be empty");
    }
    final byte[] publicInputsBytes = canonicalPublicInputsBytes(input.publicInputs());
    final ProofContext proofContext =
        normalizeProofContext(input.statementHash(), input.destinationBindingHash());
    final String sourceStateVerifierId =
        normalizeNonEmpty(input.sourceStateVerifierId(), "sourceStateVerifierId");
    if (!MAINNET_SHARD_STATE_VERIFIER_ID_V1.equals(sourceStateVerifierId)) {
      throw new IllegalArgumentException(
          "sourceStateVerifierId must match TON shard-state verifier profile");
    }
    final byte[] sourceStateVerifierHashBytes =
        nonZeroHex32Bytes(input.sourceStateVerifierHash(), "sourceStateVerifierHash");
    if (Arrays.equals(sourceStateVerifierHashBytes, TEMPLATE_SHARD_STATE_VERIFIER_HASH)) {
      throw new IllegalArgumentException(
          "sourceStateVerifierHash must not be the TON template verifier hash");
    }
    final String sourceStateVerifierHash = "0x" + hexLower(sourceStateVerifierHashBytes);
    final SolanaSccpProver.SourceAdapterDeploymentBinding deploymentBinding =
        SolanaSccpProver.normalizeSourceAdapterDeploymentBinding(
            input.sourceDomain(),
            SolanaSccpProver.DOMAIN_SORA,
            input.sourceAdapterDeploymentHash(),
            input.sourceAdapterDeploymentReceiptHash());
    if (SolanaSccpProver.ZERO_HASH_V1.equals(
        deploymentBinding.sourceAdapterDeploymentHash())) {
      throw new IllegalArgumentException(
          "TON SCCP proof request requires non-zero source adapter deployment binding");
    }
    final String deploymentBindingHash =
        SolanaSccpProver.sourceAdapterDeploymentBindingHash(deploymentBinding);
    final ByteArrayOutputStream preimage = new ByteArrayOutputStream();
    write(preimage, publicInputsBytes);
    writeVector(preimage, input.bundleBytes());
    requireOptionalNonZeroBytes(input.sourceProofBytes(), "sourceProofBytes");
    writeVector(preimage, input.sourceProofBytes());
    writeString(preimage, sourceStateVerifierId, "sourceStateVerifierId");
    write(preimage, sourceStateVerifierHashBytes);
    write(preimage, hex32Bytes(proofContext.statementHash(), "statementHash"));
    write(preimage, hex32Bytes(proofContext.destinationBindingHash(), "destinationBindingHash"));
    write(preimage, hex32Bytes(deploymentBindingHash, "sourceAdapterDeploymentBindingHash"));
    return new ProofRequest(
        1,
        input.backend(),
        input.sourceDomain(),
        input.publicInputs().targetDomain(),
        input.publicInputs(),
        publicInputsBytes,
        Arrays.copyOf(input.bundleBytes(), input.bundleBytes().length),
        Arrays.copyOf(input.sourceProofBytes(), input.sourceProofBytes().length),
        proofContext,
        proofContext.statementHash(),
        proofContext.destinationBindingHash(),
        sourceStateVerifierId,
        sourceStateVerifierHash,
        deploymentBindingHash,
        deploymentBinding,
        hashHex("sccp:ton:proof-request:v1", preimage.toByteArray()));
  }

  public static ProofResult wrapProofResult(final byte[] proofBytes, final ProofRequest request) {
    Objects.requireNonNull(request, "request");
    if (!CONTRACT_PROOF_BACKEND_V1.equals(request.backend())) {
      throw new IllegalArgumentException("TON SCCP proof request backend must be ton-contract-v1");
    }
    requireNonZeroProofBytes(proofBytes);
    requireProductionProofRequest(request);
    final ByteArrayOutputStream envelopePayload = new ByteArrayOutputStream();
    write(envelopePayload, hex32Bytes(request.requestHash(), "requestHash"));
    write(
        envelopePayload,
        hex32Bytes(
            request.sourceAdapterDeploymentBindingHash(),
            "sourceAdapterDeploymentBindingHash"));
    write(envelopePayload, proofBytes);
    return new ProofResult(
        1,
        request.backend(),
        Arrays.copyOf(proofBytes, proofBytes.length),
        Base64.getEncoder().encodeToString(proofBytes),
        request.publicInputs(),
        request.bundleBytes(),
        request.sourceProofBytes(),
        request.proofContext(),
        request.statementHash(),
        request.destinationBindingHash(),
        request.sourceStateVerifierId(),
        request.sourceStateVerifierHash(),
        request.sourceAdapterDeploymentBindingHash(),
        request.sourceAdapterDeploymentBinding(),
        request.requestHash(),
        hashHex("sccp:ton:proof-envelope:v1", envelopePayload.toByteArray()));
  }

  private static ProofResult requireWrappedProofResultForSubmission(final ProofResult proofResult) {
    Objects.requireNonNull(proofResult, "proofResult");
    if (!CONTRACT_PROOF_BACKEND_V1.equals(proofResult.backend())) {
      throw new IllegalArgumentException("proofResult.backend must be ton-contract-v1");
    }
    final ProofContext proofContext =
        Objects.requireNonNull(proofResult.proofContext(), "proofResult.proofContext");
    final ProofContext expectedProofContext =
        normalizeProofContext(proofResult.statementHash(), proofResult.destinationBindingHash());
    if (!proofContext.equals(expectedProofContext)) {
      throw new IllegalArgumentException(
          "proofResult.proofContext must match statementHash and destinationBindingHash");
    }
    if (proofResult.publicInputs().targetDomain() != DOMAIN_TON) {
      throw new IllegalArgumentException("proofResult.publicInputs.targetDomain must be TON");
    }
    final byte[] proofBytes = proofResult.proofBytes();
    requireNonZeroProofBytes(proofBytes);
    if (!Base64.getEncoder().encodeToString(proofBytes).equals(proofResult.proofBase64())) {
      throw new IllegalArgumentException(
          "proofResult.proofBase64 must match proofResult.proofBytes");
    }
    final String sourceStateVerifierId =
        normalizeNonEmpty(
            proofResult.sourceStateVerifierId(), "proofResult.sourceStateVerifierId");
    if (!MAINNET_SHARD_STATE_VERIFIER_ID_V1.equals(sourceStateVerifierId)) {
      throw new IllegalArgumentException(
          "proofResult.sourceStateVerifierId must match TON shard-state verifier profile");
    }
    final byte[] sourceStateVerifierHashBytes =
        nonZeroHex32Bytes(
            proofResult.sourceStateVerifierHash(), "proofResult.sourceStateVerifierHash");
    if (Arrays.equals(sourceStateVerifierHashBytes, TEMPLATE_SHARD_STATE_VERIFIER_HASH)) {
      throw new IllegalArgumentException(
          "proofResult.sourceStateVerifierHash must not be the TON template verifier hash");
    }
    final String requestHash = normalizeHex32(proofResult.requestHash(), "proofResult.requestHash");
    if (SolanaSccpProver.ZERO_HASH_V1.equals(requestHash)) {
      throw new IllegalArgumentException("proofResult.requestHash must be non-zero");
    }
    final String sourceAdapterDeploymentBindingHash =
        normalizeHex32(
            proofResult.sourceAdapterDeploymentBindingHash(),
            "proofResult.sourceAdapterDeploymentBindingHash");
    if (SolanaSccpProver.ZERO_HASH_V1.equals(sourceAdapterDeploymentBindingHash)) {
      throw new IllegalArgumentException(
          "proofResult.sourceAdapterDeploymentBindingHash must be non-zero");
    }
    final SolanaSccpProver.SourceAdapterDeploymentBinding deploymentBinding =
        SolanaSccpProver.normalizeSourceAdapterDeploymentBinding(
            proofResult.sourceAdapterDeploymentBinding().sourceDomain(),
            proofResult.sourceAdapterDeploymentBinding().targetDomain(),
            proofResult.sourceAdapterDeploymentBinding().sourceAdapterDeploymentHash(),
            proofResult.sourceAdapterDeploymentBinding().sourceAdapterDeploymentReceiptHash());
    if (deploymentBinding.sourceDomain() != DOMAIN_TON) {
      throw new IllegalArgumentException(
          "proofResult.sourceAdapterDeploymentBinding.sourceDomain must be TON");
    }
    if (deploymentBinding.targetDomain() != SolanaSccpProver.DOMAIN_SORA) {
      throw new IllegalArgumentException(
          "proofResult.sourceAdapterDeploymentBinding.targetDomain must be SORA");
    }
    if (SolanaSccpProver.ZERO_HASH_V1.equals(
        deploymentBinding.sourceAdapterDeploymentHash())) {
      throw new IllegalArgumentException(
          "proofResult.sourceAdapterDeploymentBinding must be non-zero");
    }
    if (!SolanaSccpProver.sourceAdapterDeploymentBindingHash(deploymentBinding)
        .equals(sourceAdapterDeploymentBindingHash)) {
      throw new IllegalArgumentException(
          "proofResult.sourceAdapterDeploymentBindingHash must match sourceAdapterDeploymentBinding");
    }
    final String envelopeHash =
        normalizeHex32(proofResult.envelopeHash(), "proofResult.envelopeHash");
    if (SolanaSccpProver.ZERO_HASH_V1.equals(envelopeHash)) {
      throw new IllegalArgumentException("proofResult.envelopeHash must be non-zero");
    }
    final ByteArrayOutputStream envelopePayload = new ByteArrayOutputStream();
    write(envelopePayload, hex32Bytes(requestHash, "proofResult.requestHash"));
    write(
        envelopePayload,
        hex32Bytes(
            sourceAdapterDeploymentBindingHash,
            "proofResult.sourceAdapterDeploymentBindingHash"));
    write(envelopePayload, proofBytes);
    if (!envelopeHash.equals(hashHex("sccp:ton:proof-envelope:v1", envelopePayload.toByteArray()))) {
      throw new IllegalArgumentException(
          "proofResult.envelopeHash must match wrapped proof bytes");
    }
    requireOptionalNonZeroBytes(proofResult.sourceProofBytes(), "proofResult.sourceProofBytes");
    final ProofRequest expectedRequest =
        buildProofRequest(
            new ProofRequestInput(
                proofResult.publicInputs(),
                proofResult.bundleBytes(),
                proofResult.sourceProofBytes(),
                proofResult.statementHash(),
                proofResult.destinationBindingHash(),
                sourceStateVerifierId,
                proofResult.sourceStateVerifierHash(),
                deploymentBinding.sourceAdapterDeploymentHash(),
                deploymentBinding.sourceAdapterDeploymentReceiptHash(),
                proofResult.backend(),
                DOMAIN_TON));
    if (!expectedRequest.requestHash().equals(requestHash)) {
      throw new IllegalArgumentException(
          "proofResult.requestHash must match bundleBytes and sourceProofBytes");
    }
    return proofResult;
  }

  private static void requireNonZeroProofBytes(final byte[] proofBytes) {
    if (proofBytes == null || proofBytes.length == 0) {
      throw new IllegalArgumentException("proofBytes must not be empty");
    }
    if (proofBytes.length > NATIVE_RECURSIVE_MAX_PROOF_BYTES) {
      throw new IllegalArgumentException(
          "proofBytes must be at most " + NATIVE_RECURSIVE_MAX_PROOF_BYTES + " bytes");
    }
    for (final byte value : proofBytes) {
      if (value != 0) {
        return;
      }
    }
    throw new IllegalArgumentException("proofBytes must not be all zero");
  }

  private static void requireOptionalNonZeroBytes(final byte[] bytes, final String label) {
    if (bytes.length == 0) {
      return;
    }
    if (!containsNonZero(bytes)) {
      throw new IllegalArgumentException(label + " must not be all zero");
    }
  }

  private static void requireNonEmptyNonZeroBytes(final byte[] bytes, final String label) {
    if (bytes.length == 0) {
      throw new IllegalArgumentException(label + " must not be empty");
    }
    requireOptionalNonZeroBytes(bytes, label);
  }

  private static void requireCanonicalProofRequest(final ProofRequest request) {
    final ProofRequest expected =
        buildProofRequest(
            new ProofRequestInput(
                request.publicInputs(),
                request.bundleBytes(),
                request.sourceProofBytes(),
                request.statementHash(),
                request.destinationBindingHash(),
                request.sourceStateVerifierId(),
                request.sourceStateVerifierHash(),
                request.sourceAdapterDeploymentBinding().sourceAdapterDeploymentHash(),
                request.sourceAdapterDeploymentBinding().sourceAdapterDeploymentReceiptHash(),
                request.backend(),
                request.sourceDomain()));
    if (request.version() != expected.version()
        || !Objects.equals(request.backend(), expected.backend())
        || request.sourceDomain() != expected.sourceDomain()
        || request.targetDomain() != expected.targetDomain()
        || !Objects.equals(request.publicInputs(), expected.publicInputs())
        || !Arrays.equals(request.publicInputsBytes(), expected.publicInputsBytes())
        || !Arrays.equals(request.bundleBytes(), expected.bundleBytes())
        || !Arrays.equals(request.sourceProofBytes(), expected.sourceProofBytes())
        || !Objects.equals(request.proofContext(), expected.proofContext())
        || !Objects.equals(request.statementHash(), expected.statementHash())
        || !Objects.equals(request.destinationBindingHash(), expected.destinationBindingHash())
        || !Objects.equals(request.sourceStateVerifierId(), expected.sourceStateVerifierId())
        || !Objects.equals(request.sourceStateVerifierHash(), expected.sourceStateVerifierHash())
        || !Objects.equals(
            request.sourceAdapterDeploymentBindingHash(),
            expected.sourceAdapterDeploymentBindingHash())
        || !Objects.equals(
            request.sourceAdapterDeploymentBinding(),
            expected.sourceAdapterDeploymentBinding())
        || !Objects.equals(request.requestHash(), expected.requestHash())) {
      throw new IllegalArgumentException("proof request must be canonical");
    }
  }

  private static void requireProductionProofRequest(final ProofRequest request) {
    requireCanonicalProofRequest(request);
    if (request.version() != 1) {
      throw new IllegalArgumentException("proof request version must be 1");
    }
    if (request.sourceDomain() != DOMAIN_TON) {
      throw new IllegalArgumentException("TON SCCP production proof sourceDomain must be TON");
    }
    if (request.targetDomain() != DOMAIN_TON || request.publicInputs().targetDomain() != DOMAIN_TON) {
      throw new IllegalArgumentException("TON SCCP production proofs must target TON public inputs");
    }
    if (!CONTRACT_PROOF_BACKEND_V1.equals(request.backend())) {
      throw new IllegalArgumentException("TON SCCP proof request backend must be ton-contract-v1");
    }
    if (request.bundleBytes().length == 0) {
      throw new IllegalArgumentException("TON SCCP proof request bundleBytes must not be empty");
    }
    requireOptionalNonZeroBytes(
        request.sourceProofBytes(), "TON SCCP proof request sourceProofBytes");
    if (!MAINNET_SHARD_STATE_VERIFIER_ID_V1.equals(request.sourceStateVerifierId())) {
      throw new IllegalArgumentException(
          "sourceStateVerifierId must match TON shard-state verifier profile");
    }
    final byte[] sourceStateVerifierHashBytes =
        nonZeroHex32Bytes(request.sourceStateVerifierHash(), "sourceStateVerifierHash");
    if (Arrays.equals(sourceStateVerifierHashBytes, TEMPLATE_SHARD_STATE_VERIFIER_HASH)) {
      throw new IllegalArgumentException(
          "sourceStateVerifierHash must not be the TON template verifier hash");
    }
    final SolanaSccpProver.SourceAdapterDeploymentBinding deploymentBinding =
        Objects.requireNonNull(
            request.sourceAdapterDeploymentBinding(), "sourceAdapterDeploymentBinding");
    if (deploymentBinding.sourceDomain() != DOMAIN_TON) {
      throw new IllegalArgumentException("sourceAdapterDeploymentBinding.sourceDomain must be TON");
    }
    if (deploymentBinding.targetDomain() != SolanaSccpProver.DOMAIN_SORA) {
      throw new IllegalArgumentException("sourceAdapterDeploymentBinding.targetDomain must be SORA");
    }
    if (SolanaSccpProver.ZERO_HASH_V1.equals(
        deploymentBinding.sourceAdapterDeploymentHash())) {
      throw new IllegalArgumentException("sourceAdapterDeploymentBinding must be non-zero");
    }
    if (!SolanaSccpProver.sourceAdapterDeploymentBindingHash(deploymentBinding)
        .equals(request.sourceAdapterDeploymentBindingHash())) {
      throw new IllegalArgumentException(
          "sourceAdapterDeploymentBindingHash must match sourceAdapterDeploymentBinding");
    }
  }

  private static int pushSnakeCells(final List<TonCell> cells, final byte[] bytes) {
    final int start = cells.size();
    final byte[] data = bytes == null ? new byte[0] : bytes;
    if (data.length == 0) {
      if (cells.size() + 1 > MAX_BOC_CELLS) {
        throw new IllegalArgumentException("TON BOC contains too many cells");
      }
      cells.add(new TonCell(new byte[0], new ArrayList<>()));
      return start;
    }
    final int chunkCount = (data.length + MAX_CELL_DATA_BYTES - 1) / MAX_CELL_DATA_BYTES;
    if (cells.size() + chunkCount > MAX_BOC_CELLS) {
      throw new IllegalArgumentException("TON BOC contains too many cells");
    }
    for (int index = 0; index < chunkCount; index++) {
      final int chunkStart = index * MAX_CELL_DATA_BYTES;
      final int chunkEnd = Math.min(chunkStart + MAX_CELL_DATA_BYTES, data.length);
      final byte[] chunk = Arrays.copyOfRange(data, chunkStart, chunkEnd);
      final ArrayList<Integer> refs = new ArrayList<>();
      if (index + 1 != chunkCount) {
        refs.add(start + index + 1);
      }
      cells.add(new TonCell(chunk, refs));
    }
    return start;
  }

  private static byte[] encodeBocSingleRoot(final List<TonCell> cells, final int rootIndex) {
    if (cells.isEmpty() || rootIndex < 0 || rootIndex >= cells.size()) {
      throw new IllegalArgumentException("invalid TON BOC root");
    }
    if (cells.size() > MAX_BOC_CELLS) {
      throw new IllegalArgumentException("TON BOC contains too many cells");
    }
    final int sizeBytes = minSizeBytes(Math.max(cells.size(), rootIndex));
    final byte[] cellsBytes = serializeCells(cells, sizeBytes);
    final int offsetBytes = minSizeBytes(cellsBytes.length);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    write(out, BOC_MAGIC);
    out.write(sizeBytes);
    out.write(offsetBytes);
    write(out, sizedUInt(cells.size(), sizeBytes));
    write(out, sizedUInt(1, sizeBytes));
    write(out, sizedUInt(0, sizeBytes));
    write(out, sizedUInt(cellsBytes.length, offsetBytes));
    write(out, sizedUInt(rootIndex, sizeBytes));
    write(out, cellsBytes);
    return out.toByteArray();
  }

  private static byte[] serializeCells(final List<TonCell> cells, final int sizeBytes) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    for (int index = 0; index < cells.size(); index++) {
      final TonCell cell = cells.get(index);
      if (cell.data.length > MAX_CELL_DATA_BYTES || cell.refs.size() > MAX_REFS) {
        throw new IllegalArgumentException("invalid TON cell at index " + index);
      }
      out.write(cell.refs.size());
      out.write(cell.data.length * 2);
      write(out, cell.data);
      for (final int ref : cell.refs) {
        if (ref < 0 || ref >= cells.size()) {
          throw new IllegalArgumentException("invalid TON cell ref");
        }
        write(out, sizedUInt(ref, sizeBytes));
      }
    }
    return out.toByteArray();
  }

  private static List<byte[]> normalizeInclusionBranch(final List<byte[]> value) {
    Objects.requireNonNull(value, "inclusionBranch");
    if (value.size() > MAX_SOURCE_MERKLE_BRANCH_NODES) {
      throw new IllegalArgumentException(
          "inclusionBranch must contain at most " + MAX_SOURCE_MERKLE_BRANCH_NODES + " entries");
    }
    final List<byte[]> out = new ArrayList<>(value.size());
    for (int index = 0; index < value.size(); index++) {
      final byte[] sibling = Objects.requireNonNull(value.get(index), "inclusionBranch");
      if (sibling.length != 32) {
        throw new IllegalArgumentException("inclusionBranch[" + index + "] must be 32 bytes");
      }
      out.add(Arrays.copyOf(sibling, sibling.length));
    }
    return out;
  }

  private static ValidatorSetParts normalizeValidatorSet(
      final List<byte[]> validatorPublicKeys, final List<String> validatorWeights) {
    Objects.requireNonNull(validatorPublicKeys, "validatorPublicKeys");
    Objects.requireNonNull(validatorWeights, "validatorWeights");
    if (validatorPublicKeys.isEmpty()
        || validatorPublicKeys.size() > MAX_VALIDATORS
        || validatorPublicKeys.size() != validatorWeights.size()) {
      throw new IllegalArgumentException(
          "TON validator public keys and weights must be same-length arrays");
    }
    final ArrayList<byte[]> keys = new ArrayList<>(validatorPublicKeys.size());
    final ArrayList<String> seen = new ArrayList<>(validatorPublicKeys.size());
    for (int index = 0; index < validatorPublicKeys.size(); index++) {
      final byte[] publicKey = Objects.requireNonNull(validatorPublicKeys.get(index), "validatorPublicKeys");
      if (publicKey.length != 32) {
        throw new IllegalArgumentException("validatorPublicKeys[" + index + "] must be 32 bytes");
      }
      if (!containsNonZero(publicKey)) {
        throw new IllegalArgumentException("validatorPublicKeys[" + index + "] must not be zero");
      }
      final String encoded = hexLower(publicKey);
      if (seen.contains(encoded)) {
        throw new IllegalArgumentException("TON validator public keys must be unique");
      }
      seen.add(encoded);
      keys.add(Arrays.copyOf(publicKey, publicKey.length));
    }
    final ArrayList<BigInteger> weights = new ArrayList<>(validatorWeights.size());
    for (int index = 0; index < validatorWeights.size(); index++) {
      final BigInteger weight = normalizeU64(validatorWeights.get(index), "validatorWeights[" + index + "]");
      if (BigInteger.ZERO.equals(weight)) {
        throw new IllegalArgumentException("validatorWeights[" + index + "] must not be zero");
      }
      weights.add(weight);
    }
    return new ValidatorSetParts(keys, weights);
  }

  private static void validateValidatorSetPayload(final byte[] payload) {
    Objects.requireNonNull(payload, "validatorSetPayload");
    if (payload.length < 5 || payload[0] != 1) {
      throw new IllegalArgumentException("validatorSetPayload must use version 1");
    }
    final int count =
        (payload[1] & 0xff)
            | ((payload[2] & 0xff) << 8)
            | ((payload[3] & 0xff) << 16)
            | ((payload[4] & 0xff) << 24);
    if (count <= 0 || count > MAX_VALIDATORS || payload.length != 5 + count * 40) {
      throw new IllegalArgumentException(
          "validatorSetPayload has invalid validator count or length");
    }
    final ArrayList<String> seen = new ArrayList<String>(count);
    int offset = 5;
    for (int index = 0; index < count; index++) {
      final byte[] publicKey = Arrays.copyOfRange(payload, offset, offset + 32);
      offset += 32;
      if (!containsNonZero(publicKey)) {
        throw new IllegalArgumentException("validatorPublicKeys[" + index + "] must not be zero");
      }
      final String keyHex = hexLower(publicKey);
      if (seen.contains(keyHex)) {
        throw new IllegalArgumentException("TON validator public keys must be unique");
      }
      seen.add(keyHex);
      BigInteger weight = BigInteger.ZERO;
      for (int byteIndex = 0; byteIndex < 8; byteIndex++) {
        weight =
            weight.or(
                BigInteger.valueOf(payload[offset + byteIndex] & 0xffL)
                    .shiftLeft(byteIndex * 8));
      }
      offset += 8;
      if (BigInteger.ZERO.equals(weight)) {
        throw new IllegalArgumentException("validatorWeights[" + index + "] must not be zero");
      }
    }
  }

  private static void writeVector(final ByteArrayOutputStream out, final byte[] bytes) {
    writeU32Le(out, bytes.length);
    write(out, bytes);
  }

  private static List<Integer> signerIndicesFromBitmap(
      final byte[] bitmap, final int rosterLength) {
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

  private static byte[] canonicalValidatorSignatureProofBytes(
      final ValidatorSignatureProofInput input) {
    Objects.requireNonNull(input, "validatorSignatureProof");
    final ValidatorSetParts parts =
        normalizeValidatorSet(input.validatorPublicKeys(), input.validatorWeights());
    if (input.version() != 1) {
      throw new IllegalArgumentException("TON validator signature proof version must be 1");
    }
    final BigInteger totalWeight = normalizeU64(input.totalWeight(), "totalWeight");
    final BigInteger signedWeight = normalizeU64(input.signedWeight(), "signedWeight");
    BigInteger computedTotalWeight = BigInteger.ZERO;
    for (final BigInteger weight : parts.weights) {
      computedTotalWeight = computedTotalWeight.add(weight);
    }
    if (!totalWeight.equals(computedTotalWeight)) {
      throw new IllegalArgumentException("totalWeight must match validatorWeights");
    }
    final byte[] signersBitmap = Objects.requireNonNull(input.signersBitmap(), "signersBitmap");
    final List<Integer> signerIndices =
        signerIndicesFromBitmap(signersBitmap, parts.publicKeys.size());
    if (signerIndices.isEmpty()) {
      throw new IllegalArgumentException("signersBitmap must select at least one validator");
    }
    if (input.signatures().size() != signerIndices.size()) {
      throw new IllegalArgumentException("signatures length must match signersBitmap");
    }
    BigInteger computedSignedWeight = BigInteger.ZERO;
    for (final int index : signerIndices) {
      computedSignedWeight = computedSignedWeight.add(parts.weights.get(index));
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
    out.write(input.version());
    writeU64Le(out, totalWeight);
    writeU64Le(out, signedWeight);
    write(out, nonZeroHex32Bytes(input.blockMessageHash(), "blockMessageHash"));
    writeU32Le(out, parts.publicKeys.size());
    for (final byte[] publicKey : parts.publicKeys) {
      writeVector(out, publicKey);
    }
    writeU32Le(out, parts.weights.size());
    for (final BigInteger weight : parts.weights) {
      writeU64Le(out, weight);
    }
    writeVector(out, signersBitmap);
    writeU32Le(out, input.signatures().size());
    for (int index = 0; index < input.signatures().size(); index++) {
      final byte[] signature = Objects.requireNonNull(input.signatures().get(index), "signatures");
      if (signature.length != 64) {
        throw new IllegalArgumentException("signatures[" + index + "] must be 64 bytes");
      }
      boolean nonZero = false;
      for (final byte value : signature) {
        if (value != 0) {
          nonZero = true;
          break;
        }
      }
      if (!nonZero) {
        throw new IllegalArgumentException("signatures[" + index + "] must not be all zero");
      }
      writeVector(out, signature);
    }
    return out.toByteArray();
  }

  private static BoundedBocHash boundedBocHash(
      final String prefix, final byte[] value, final String field) {
    final byte[] raw = Arrays.copyOf(Objects.requireNonNull(value, field), value.length);
    if (raw.length == 0) {
      throw new IllegalArgumentException(field + " must not be empty");
    }
    if (raw.length > MAX_BOC_BYTES) {
      throw new IllegalArgumentException(field + " exceeds TON BoC proof byte limit");
    }
    return new BoundedBocHash(raw, hashHex(prefix, raw));
  }

  private static NormalizedValidatorSetTransitionProof normalizeValidatorSetTransitionForSourceState(
      final ValidatorSetTransitionProofInput input) {
    Objects.requireNonNull(input, "validatorSetTransitionProof");
    if (input.version() != 1) {
      throw new IllegalArgumentException("TON validator-set transition proof version must be 1");
    }
    if (input.sourceDomain() != DOMAIN_TON) {
      throw new IllegalArgumentException("sourceDomain must be TON");
    }
    if (input.masterchainWorkchainId() != TON_MASTERCHAIN_WORKCHAIN_ID) {
      throw new IllegalArgumentException("masterchainWorkchainId must be TON masterchain");
    }
    final BigInteger masterchainShard = normalizeU64(input.masterchainShard(), "masterchainShard");
    if (!TON_MASTERCHAIN_SHARD.equals(masterchainShard)) {
      throw new IllegalArgumentException("masterchainShard must be TON masterchain shard");
    }
    final String transitionSignatureHash =
        normalizeHex32(input.transitionSignatureHash(), "transitionSignatureHash");
    final String expectedTransitionSignatureHash =
        validatorSetTransitionSignatureHash(
            input.version(),
            input.sourceDomain(),
            input.fromValidatorSetSeqno(),
            input.toValidatorSetSeqno(),
            input.masterchainSeqno(),
            input.masterchainWorkchainId(),
            input.masterchainShard(),
            input.masterchainBlockHash(),
            input.masterchainFileHash(),
            input.parentValidatorSetHash(),
            input.nextValidatorSetHash(),
            input.nextValidatorSetPayload(),
            input.nextValidatorSetPayloadHash(),
            input.nextValidatorSetConfigHash(),
            input.transitionMessageHash(),
            input.validatorSignatureProof());
    if (!Arrays.equals(
        hex32Bytes(transitionSignatureHash, "transitionSignatureHash"),
        hex32Bytes(expectedTransitionSignatureHash, "transitionSignatureHash"))) {
      throw new IllegalArgumentException(
          "transitionSignatureHash must match transition signature fields");
    }
    return new NormalizedValidatorSetTransitionProof(
        input.version(),
        input.sourceDomain(),
        normalizeU64(input.fromValidatorSetSeqno(), "fromValidatorSetSeqno"),
        normalizeU64(input.toValidatorSetSeqno(), "toValidatorSetSeqno"),
        normalizeU64(input.masterchainSeqno(), "masterchainSeqno"),
        input.masterchainWorkchainId(),
        masterchainShard,
        normalizeNonZeroHex32(input.masterchainBlockHash(), "masterchainBlockHash"),
        normalizeNonZeroHex32(input.masterchainFileHash(), "masterchainFileHash"),
        normalizeHex32(input.parentValidatorSetHash(), "parentValidatorSetHash"),
        normalizeHex32(input.nextValidatorSetHash(), "nextValidatorSetHash"),
        Arrays.copyOf(input.nextValidatorSetPayload(), input.nextValidatorSetPayload().length),
        normalizeHex32(input.nextValidatorSetPayloadHash(), "nextValidatorSetPayloadHash"),
        normalizeHex32(input.nextValidatorSetConfigHash(), "nextValidatorSetConfigHash"),
        normalizeHex32(input.transitionMessageHash(), "transitionMessageHash"),
        transitionSignatureHash,
        input.validatorSignatureProof());
  }

  private static byte[] canonicalValidatorSetTransitionProofBytes(
      final NormalizedValidatorSetTransitionProof transition) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(transition.version);
    writeU32Le(out, transition.sourceDomain);
    writeU64Le(out, transition.fromValidatorSetSeqno);
    writeU64Le(out, transition.toValidatorSetSeqno);
    writeU64Le(out, transition.masterchainSeqno);
    writeI32Le(out, transition.masterchainWorkchainId);
    writeU64Le(out, transition.masterchainShard);
    write(out, hex32Bytes(transition.masterchainBlockHash, "masterchainBlockHash"));
    write(out, hex32Bytes(transition.masterchainFileHash, "masterchainFileHash"));
    write(out, hex32Bytes(transition.parentValidatorSetHash, "parentValidatorSetHash"));
    write(out, hex32Bytes(transition.nextValidatorSetHash, "nextValidatorSetHash"));
    writeVector(out, transition.nextValidatorSetPayload);
    write(out, hex32Bytes(transition.nextValidatorSetPayloadHash, "nextValidatorSetPayloadHash"));
    write(out, hex32Bytes(transition.nextValidatorSetConfigHash, "nextValidatorSetConfigHash"));
    write(out, hex32Bytes(transition.transitionMessageHash, "transitionMessageHash"));
    write(out, hex32Bytes(transition.transitionSignatureHash, "transitionSignatureHash"));
    write(out, canonicalValidatorSignatureProofBytes(transition.validatorSignatureProof));
    return out.toByteArray();
  }

  private static String validatorSetTransitionChainHash(
      final List<NormalizedValidatorSetTransitionProof> transitions) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeU32Le(out, transitions.size());
    for (final NormalizedValidatorSetTransitionProof transition : transitions) {
      write(out, canonicalValidatorSetTransitionProofBytes(transition));
    }
    return hashHex(VALIDATOR_SET_TRANSITION_CHAIN_PREFIX_V1, out.toByteArray());
  }

  private static AuditRoleProfile auditRoleProfile(final FullLightClientAuditRole role) {
    Objects.requireNonNull(role, "role");
    switch (role) {
      case MASTERCHAIN_CONFIG:
        return new AuditRoleProfile(
            "masterchain_config",
            1,
            MASTERCHAIN_CONFIG_OPEN_VERIFY_CIRCUIT_ID_V1,
            SourceSccpProofs.TON_MAINNET_MASTERCHAIN_CONFIG_VERIFIER_ID_V1,
            new String[] {
              "masterchain_config_root",
              "masterchain_config_proof_hash",
              "validator_set_payload_hash",
              "config_leaf_hash",
              "config_value_hash",
              "config_proof_boc_hash"
            });
      case VALIDATOR_SET_TRANSITION:
        return new AuditRoleProfile(
            "validator_set_transition",
            2,
            VALIDATOR_SET_TRANSITION_OPEN_VERIFY_CIRCUIT_ID_V1,
            SourceSccpProofs.TON_MAINNET_VALIDATOR_SET_TRANSITION_VERIFIER_ID_V1,
            new String[] {
              "source_trust_anchor_hash",
              "validator_set_hash",
              "validator_set_transition_chain_hash",
              "masterchain_signature_hash",
              "validator_set_transition_count"
            });
      case SHARD_ACCOUNTS_DICTIONARY:
        return new AuditRoleProfile(
            "shard_accounts_dictionary",
            3,
            SHARD_ACCOUNTS_DICTIONARY_OPEN_VERIFY_CIRCUIT_ID_V1,
            SourceSccpProofs.TON_MAINNET_SHARD_ACCOUNTS_DICTIONARY_VERIFIER_ID_V1,
            new String[] {
              "shard_state_root",
              "shard_state_dictionary_root",
              "transaction_root",
              "shard_state_proof_boc_hash",
              "shard_accounts_proof_boc_hash",
              "shard_state_verification_proof_hash"
            });
      default:
        throw new IllegalArgumentException("unknown TON full light-client audit role");
    }
  }

  private static String auditRoleVerifierHash(
      final FullLightClientAuditProofInput input,
      final FullLightClientAuditRole role) {
    switch (role) {
      case MASTERCHAIN_CONFIG:
        return normalizeNonZeroHex32(
            input.tonMasterchainConfigVerifierHash(), "tonMasterchainConfigVerifierHash");
      case VALIDATOR_SET_TRANSITION:
        return normalizeNonZeroHex32(
            input.tonValidatorSetTransitionVerifierHash(),
            "tonValidatorSetTransitionVerifierHash");
      case SHARD_ACCOUNTS_DICTIONARY:
        return normalizeNonZeroHex32(
            input.tonShardAccountsDictionaryVerifierHash(),
            "tonShardAccountsDictionaryVerifierHash");
      default:
        throw new IllegalArgumentException("unknown TON full light-client audit role");
    }
  }

  private static NormalizedFullLightClientAuditInput normalizeFullLightClientAuditInput(
      final FullLightClientAuditProofInput input,
      final FullLightClientAuditRole role) {
    Objects.requireNonNull(input, "input");
    final NormalizedShardStateSourceStateInput shardState =
        normalizeShardStateSourceStateInput(input.shardState());
    final String sourceVerifierMaterialHash =
        SourceSccpProofs.sourceVerifierMaterialHash(
            shardState.sourceDomain,
            shardState.sourceTrustAnchorHash,
            shardState.consensusVerifierHash,
            shardState.messageInclusionVerifierHash,
            shardState.finalityPolicyHash,
            shardState.sourceStateVerifierHash,
            null,
            null,
            null,
            null,
            null);
    if (!normalizeNonZeroHex32(input.sourceVerifierMaterialHash(), "sourceVerifierMaterialHash")
        .equals(sourceVerifierMaterialHash)) {
      throw new IllegalArgumentException(
          "sourceVerifierMaterialHash must match TON shard-state verification context");
    }
    final String sourceAdapterDeploymentHash =
        normalizeNonZeroHex32(input.sourceAdapterDeploymentHash(), "sourceAdapterDeploymentHash");
    final String fullLightClientGateHash =
        normalizeNonZeroHex32(input.fullLightClientGateHash(), "fullLightClientGateHash");
    final String[] auditRoleHashes = {
      auditRoleVerifierHash(input, FullLightClientAuditRole.MASTERCHAIN_CONFIG),
      auditRoleVerifierHash(input, FullLightClientAuditRole.VALIDATOR_SET_TRANSITION),
      auditRoleVerifierHash(input, FullLightClientAuditRole.SHARD_ACCOUNTS_DICTIONARY)
    };
    requireFullLightClientAuditRoleSeparation(
        auditRoleHashes,
        new String[] {
          shardState.sourceTrustAnchorHash,
          shardState.consensusVerifierHash,
          shardState.messageInclusionVerifierHash,
          shardState.finalityPolicyHash,
          shardState.sourceStateVerifierHash
        });
    final String shardStateProofPublicInputsHash = shardStateProofPublicInputsHash(input.shardState());
    if (input.shardStateProofPublicInputsHash() != null
        && !normalizeHex32(input.shardStateProofPublicInputsHash(), "shardStateProofPublicInputsHash")
            .equals(shardStateProofPublicInputsHash)) {
      throw new IllegalArgumentException(
          "shardStateProofPublicInputsHash must match TON shard-state inputs");
    }
    final String shardStateVerificationProofHash =
        shardStateVerificationProofHash(input.shardStateVerificationProof());
    if (input.shardStateVerificationProofHash() != null
        && !normalizeHex32(input.shardStateVerificationProofHash(), "shardStateVerificationProofHash")
            .equals(shardStateVerificationProofHash)) {
      throw new IllegalArgumentException(
          "shardStateVerificationProofHash must match shardStateVerificationProof");
    }
    if (shardState.sourceTrustAnchorHash.equals(shardState.validatorSetHash)
        && !shardState.validatorSetTransitionProofs.isEmpty()) {
      throw new IllegalArgumentException(
          "validatorSetTransitionProofs must be empty when validator set matches source trust anchor");
    }
    if (!shardState.sourceTrustAnchorHash.equals(shardState.validatorSetHash)
        && shardState.validatorSetTransitionProofs.isEmpty()) {
      throw new IllegalArgumentException(
          "validatorSetTransitionProofs must connect source trust anchor to validatorSetHash");
    }
    final String validatorSetPayloadHash =
        normalizeNonZeroHex32(input.validatorSetPayloadHash(), "validatorSetPayloadHash");
    final String configLeafHash = normalizeNonZeroHex32(input.configLeafHash(), "configLeafHash");
    final String configValueHash = normalizeNonZeroHex32(input.configValueHash(), "configValueHash");
    final String expectedConfigProofHash =
        masterchainConfigProofHash(
            shardState.sourceDomain,
            shardState.masterchainSeqno.toString(),
            shardState.masterchainBlockHash,
            shardState.shardStateRoot,
            shardState.masterchainConfigRoot,
            shardState.validatorSetHash,
            validatorSetPayloadHash,
            configLeafHash,
            Long.toString(CURRENT_VALIDATOR_SET_CONFIG_PARAM),
            configValueHash,
            shardState.configDictionaryProofBoc,
            Collections.<byte[]>emptyList());
    if (!expectedConfigProofHash.equals(shardState.masterchainConfigProofHash)) {
      throw new IllegalArgumentException(
          "masterchainConfigProofHash must match TON config proof fields");
    }
    return new NormalizedFullLightClientAuditInput(
        role,
        shardState,
        sourceVerifierMaterialHash,
        sourceAdapterDeploymentHash,
        fullLightClientGateHash,
        auditRoleHashes[auditRoleProfile(role).code - 1],
        shardStateProofPublicInputsHash,
        shardStateVerificationProofHash,
        validatorSetPayloadHash,
        configLeafHash,
        configValueHash);
  }

  private static void requireFullLightClientAuditRoleSeparation(
      final String[] auditRoleHashes,
      final String[] existingHashes) {
    final byte[][] auditBytes = new byte[auditRoleHashes.length][];
    for (int i = 0; i < auditRoleHashes.length; i++) {
      auditBytes[i] = hex32Bytes(auditRoleHashes[i], "tonAuditVerifierHash");
    }
    for (int i = 0; i < auditBytes.length; i++) {
      for (int j = i + 1; j < auditBytes.length; j++) {
        if (Arrays.equals(auditBytes[i], auditBytes[j])) {
          throw new IllegalArgumentException(
              "TON full-light-client audit verifier hashes must be role-separated");
        }
      }
      for (final byte[] templateHash : TEMPLATE_SOURCE_MATERIAL_HASHES) {
        if (Arrays.equals(auditBytes[i], templateHash)) {
          throw new IllegalArgumentException(
              "TON full-light-client audit verifier hash must not reuse built-in template material");
        }
      }
      for (final String existingHash : existingHashes) {
        final byte[] existingBytes = hex32Bytes(existingHash, "tonAuditExistingHash");
        if (!isZero(existingBytes) && Arrays.equals(auditBytes[i], existingBytes)) {
          throw new IllegalArgumentException(
              "TON full-light-client audit verifier hash must not reuse existing source-adapter material");
        }
      }
    }
  }

  private static void requireFullLightClientAuditRequestHashSeparation(
      final NormalizedFullLightClientAuditInput normalized) {
    requireFullLightClientAuditRequestHashSeparation(normalized, null);
  }

  private static void requireFullLightClientAuditRequestHashSeparation(
      final NormalizedFullLightClientAuditInput normalized,
      final String statementHash) {
    final byte[] verifierHash = hex32Bytes(normalized.verifierHash, "tonAuditVerifierHash");
    final ArrayList<String> requestHashes = new ArrayList<String>();
    requestHashes.add(normalized.shardState.sourceStateVerifierHash);
    requestHashes.add(normalized.sourceVerifierMaterialHash);
    requestHashes.add(normalized.sourceAdapterDeploymentHash);
    requestHashes.add(normalized.fullLightClientGateHash);
    requestHashes.add(normalized.shardStateProofPublicInputsHash);
    requestHashes.add(normalized.shardStateVerificationProofHash);
    requestHashes.add(normalized.shardState.masterchainConfigProofHash);
    requestHashes.add(normalized.shardState.masterchainSignatureHash);
    requestHashes.add(normalized.shardState.shardProofHash);
    requestHashes.add(normalized.shardState.transitionChainHash);
    requestHashes.addAll(auditRoleColumns(normalized));
    if (statementHash != null) {
      requestHashes.add(statementHash);
    }
    for (final String requestHash : requestHashes) {
      final byte[] requestBytes = hex32Bytes(requestHash, "tonAuditRequestHash");
      if (!isZero(requestBytes) && Arrays.equals(verifierHash, requestBytes)) {
        throw new IllegalArgumentException(
            "TON full-light-client audit verifier hash must not reuse request-bound hashes");
      }
    }
  }

  private static List<String> auditRoleColumns(
      final NormalizedFullLightClientAuditInput normalized) {
    final NormalizedShardStateSourceStateInput shardState = normalized.shardState;
    switch (normalized.role) {
      case MASTERCHAIN_CONFIG:
        return Arrays.asList(
            shardState.masterchainConfigRoot,
            shardState.masterchainConfigProofHash,
            normalized.validatorSetPayloadHash,
            normalized.configLeafHash,
            normalized.configValueHash,
            shardState.configProofBocHash);
      case VALIDATOR_SET_TRANSITION:
        return Arrays.asList(
            shardState.sourceTrustAnchorHash,
            shardState.validatorSetHash,
            shardState.transitionChainHash,
            shardState.masterchainSignatureHash,
            "0x"
                + hexLower(
                    sccpWordU64Le(
                        BigInteger.valueOf(shardState.validatorSetTransitionProofs.size()))));
      case SHARD_ACCOUNTS_DICTIONARY:
        return Arrays.asList(
            shardState.shardStateRoot,
            shardState.shardStateDictionaryRoot,
            shardState.transactionRoot,
            shardState.shardStateProofBocHash,
            shardState.shardAccountsProofBocHash,
            normalized.shardStateVerificationProofHash);
      default:
        throw new IllegalArgumentException("unknown TON full light-client audit role");
    }
  }

  private static FullLightClientAuditFastpqPublicInputs fullLightClientAuditFastpqPublicInputs(
      final NormalizedFullLightClientAuditInput normalized,
      final String statementHash) {
    final AuditRoleProfile profile = auditRoleProfile(normalized.role);
    final ByteArrayOutputStream dsidPreimage = new ByteArrayOutputStream();
    dsidPreimage.write(profile.code);
    write(dsidPreimage, hex32Bytes(statementHash, "auditStatementHash"));
    final byte[] dsidHash =
        prefixedHashBytes(
            FULL_LIGHT_CLIENT_AUDIT_FASTPQ_DSID_PREFIX_V1,
            dsidPreimage.toByteArray());
    final NormalizedShardStateSourceStateInput shardState = normalized.shardState;
    final String oldRoot;
    final String newRoot;
    final String permRoot;
    switch (normalized.role) {
      case MASTERCHAIN_CONFIG:
        oldRoot = shardState.masterchainConfigRoot;
        newRoot = shardState.validatorSetHash;
        permRoot = shardState.masterchainConfigProofHash;
        break;
      case VALIDATOR_SET_TRANSITION:
        oldRoot = shardState.sourceTrustAnchorHash;
        newRoot = shardState.validatorSetHash;
        permRoot = shardState.transitionChainHash;
        break;
      case SHARD_ACCOUNTS_DICTIONARY:
        oldRoot = shardState.shardStateRoot;
        newRoot = shardState.transactionRoot;
        permRoot = shardState.shardStateDictionaryRoot;
        break;
      default:
        throw new IllegalArgumentException("unknown TON full light-client audit role");
    }
    return new FullLightClientAuditFastpqPublicInputs(
        "0x" + hexLower(Arrays.copyOfRange(dsidHash, 0, 16)),
        shardState.masterchainSeqno.toString(),
        oldRoot,
        newRoot,
        permRoot,
        statementHash);
  }

  private static byte[] canonicalFullLightClientAuditContextBytes(
      final NormalizedFullLightClientAuditInput normalized,
      final String statementHash) {
    requireFullLightClientAuditRequestHashSeparation(normalized, statementHash);
    final AuditRoleProfile profile = auditRoleProfile(normalized.role);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    out.write(profile.code);
    writeString(out, profile.circuitId, "circuitId");
    writeString(out, FULL_LIGHT_CLIENT_AUDIT_FASTPQ_PARAMETER_SET_V1, "parameterSet");
    writeString(out, profile.verifierId, "verifierId");
    write(out, hex32Bytes(normalized.verifierHash, "verifierHash"));
    write(out, hex32Bytes(normalized.sourceVerifierMaterialHash, "sourceVerifierMaterialHash"));
    write(out, hex32Bytes(normalized.sourceAdapterDeploymentHash, "sourceAdapterDeploymentHash"));
    write(out, hex32Bytes(normalized.fullLightClientGateHash, "fullLightClientGateHash"));
    write(out, hex32Bytes(normalized.shardStateProofPublicInputsHash, "shardStateProofPublicInputsHash"));
    write(out, hex32Bytes(statementHash, "auditStatementHash"));
    return out.toByteArray();
  }

  private static byte[] fullLightClientAuditFastpqKey(
      final String prefix,
      final AuditRoleProfile profile) {
    final byte[] prefixBytes = prefix.getBytes(java.nio.charset.StandardCharsets.UTF_8);
    final byte[] circuitBytes = profile.circuitId.getBytes(java.nio.charset.StandardCharsets.UTF_8);
    final byte[] out = new byte[prefixBytes.length + 1 + circuitBytes.length];
    System.arraycopy(prefixBytes, 0, out, 0, prefixBytes.length);
    out[prefixBytes.length] = 0;
    System.arraycopy(circuitBytes, 0, out, prefixBytes.length + 1, circuitBytes.length);
    return out;
  }

  private static NormalizedShardStateSourceStateInput normalizeShardStateSourceStateInput(
      final ShardStateProofRequestInput input) {
    Objects.requireNonNull(input, "input");
    if (input.sourceDomain() != DOMAIN_TON) {
      throw new IllegalArgumentException("sourceDomain must be TON");
    }
    if (input.masterchainWorkchainId() != TON_MASTERCHAIN_WORKCHAIN_ID) {
      throw new IllegalArgumentException("masterchainWorkchainId must be TON masterchain");
    }
    final BigInteger masterchainShard = normalizeU64(input.masterchainShard(), "masterchainShard");
    if (!TON_MASTERCHAIN_SHARD.equals(masterchainShard)) {
      throw new IllegalArgumentException("masterchainShard must be TON masterchain shard");
    }
    if (input.shardWorkchainId() != TON_BASECHAIN_WORKCHAIN_ID) {
      throw new IllegalArgumentException("shardWorkchainId must be TON basechain");
    }
    final BigInteger masterchainSeqno = normalizeU64(input.masterchainSeqno(), "masterchainSeqno");
    final BigInteger shardShard = normalizeU64(input.shardShard(), "shardShard");
    if (BigInteger.ZERO.equals(shardShard)) {
      throw new IllegalArgumentException("shardShard must be non-zero");
    }
    final BigInteger shardSeqno = normalizeU64(input.shardSeqno(), "shardSeqno");
    if (BigInteger.ZERO.equals(shardSeqno)) {
      throw new IllegalArgumentException("shardSeqno must be non-zero");
    }
    final BigInteger transactionLt = normalizeU64(input.transactionLt(), "transactionLt");
    if (BigInteger.ZERO.equals(transactionLt)) {
      throw new IllegalArgumentException("transactionLt must be non-zero");
    }
    if (input.shardStateDictionaryKeyBitLen() != SHARD_ACCOUNT_KEY_BITS) {
      throw new IllegalArgumentException("TON ShardAccounts key bit length must be 256");
    }
    final byte[] dictionaryKey =
        Arrays.copyOf(input.shardStateDictionaryKey(), input.shardStateDictionaryKey().length);
    if (!hashmapKeyIsCanonical(dictionaryKey, input.shardStateDictionaryKeyBitLen())) {
      throw new IllegalArgumentException("shardStateDictionaryKey length is invalid");
    }
    final BoundedBocHash shardStateProofBoc =
        boundedBocHash(
            SHARD_STATE_PROOF_BOC_PREFIX_V1,
            input.shardStateProofBoc(),
            "shardStateProofBoc");
    final BoundedBocHash shardAccountsProofBoc =
        boundedBocHash(
            SHARD_ACCOUNTS_PROOF_BOC_PREFIX_V1,
            input.shardStateDictionaryProofBoc(),
            "shardStateDictionaryProofBoc");
    final BoundedBocHash configProofBoc =
        boundedBocHash(CONFIG_PROOF_BOC_PREFIX_V1, input.configDictionaryProofBoc(), "configDictionaryProofBoc");
    final String shardStateRoot = normalizeNonZeroHex32(input.shardStateRoot(), "shardStateRoot");
    final String transactionRoot = normalizeNonZeroHex32(input.transactionRoot(), "transactionRoot");
    final String dictionaryRoot =
        normalizeNonZeroHex32(input.shardStateDictionaryRoot(), "shardStateDictionaryRoot");
    if (!shardStateProofRootHash(shardStateProofBoc.raw).equals(shardStateRoot)) {
      throw new IllegalArgumentException("shardStateProofBoc root must match shardStateRoot");
    }
    final ShardStateAccountsOpening opening = shardStateAccountsOpening(shardStateProofBoc.raw);
    if (!opening.accountsRootHash.equals(dictionaryRoot)) {
      throw new IllegalArgumentException(
          "shardStateProofBoc accounts root must match shardStateDictionaryRoot");
    }
    if (opening.globalId != TON_MAINNET_GLOBAL_ID) {
      throw new IllegalArgumentException(
          "shardStateProofBoc ShardStateUnsplit global_id must be TON mainnet");
    }
    if (opening.workchainId != TON_BASECHAIN_WORKCHAIN_ID
        || opening.workchainId != input.shardWorkchainId()) {
      throw new IllegalArgumentException(
          "shardStateProofBoc ShardIdent workchain_id must match shardWorkchainId");
    }
    if (!BigInteger.valueOf(Integer.toUnsignedLong(opening.seqNo)).equals(shardSeqno)) {
      throw new IllegalArgumentException(
          "shardStateProofBoc ShardStateUnsplit seq_no must match shardSeqno");
    }
    if (!opening.shardId.equals(shardShard)) {
      throw new IllegalArgumentException(
          "shardStateProofBoc ShardIdent shard must match shardShard");
    }
    if (opening.seqNo == 0 || opening.genUtime == 0 || opening.genLt == 0L) {
      throw new IllegalArgumentException(
          "shardStateProofBoc ShardStateUnsplit metadata must be non-zero");
    }
    if (BigInteger.valueOf(Integer.toUnsignedLong(opening.minRefMcSeqno))
            .compareTo(masterchainSeqno)
        > 0) {
      throw new IllegalArgumentException(
          "shardStateProofBoc ShardStateUnsplit min_ref_mc_seqno exceeds masterchainSeqno");
    }
    if (!shardStateAccountKeyMatchesShardPrefix(
        dictionaryKey, input.shardStateDictionaryKeyBitLen(), opening)) {
      throw new IllegalArgumentException(
          "shardStateDictionaryKey must match shardStateProofBoc ShardIdent prefix");
    }
    if (!hashmapEProofRootHash(shardAccountsProofBoc.raw).equals(dictionaryRoot)) {
      throw new IllegalArgumentException(
          "shardStateDictionaryProofBoc root must match shardStateDictionaryRoot");
    }
    final ShardAccountLastTransaction selectedTransaction =
        shardAccountsLastTransaction(
            shardAccountsProofBoc.raw, dictionaryKey, input.shardStateDictionaryKeyBitLen());
    if (selectedTransaction == null || !selectedTransaction.hash().equals(transactionRoot)) {
      throw new IllegalArgumentException(
          "shardStateDictionaryProofBoc ShardAccount last transaction hash must match transactionRoot");
    }
    if (!selectedTransaction.lt().equals(transactionLt)) {
      throw new IllegalArgumentException(
          "shardStateDictionaryProofBoc ShardAccount last transaction lt must match transactionLt");
    }
    final ArrayList<NormalizedValidatorSetTransitionProof> transitionProofs =
        new ArrayList<NormalizedValidatorSetTransitionProof>();
    for (final ValidatorSetTransitionProofInput transition : input.validatorSetTransitionProofs()) {
      transitionProofs.add(normalizeValidatorSetTransitionForSourceState(transition));
    }
    final String sourceStateVerifierId =
        normalizeNonEmpty(input.sourceStateVerifierId(), "sourceStateVerifierId");
    if (!MAINNET_SHARD_STATE_VERIFIER_ID_V1.equals(sourceStateVerifierId)) {
      throw new IllegalArgumentException(
          "sourceStateVerifierId must match TON shard-state verifier profile");
    }
    final byte[] sourceStateVerifierHashBytes =
        nonZeroHex32Bytes(input.sourceStateVerifierHash(), "sourceStateVerifierHash");
    if (Arrays.equals(sourceStateVerifierHashBytes, TEMPLATE_SHARD_STATE_VERIFIER_HASH)) {
      throw new IllegalArgumentException(
          "sourceStateVerifierHash must not be the TON template verifier hash");
    }
    final String sourceStateVerifierHash = "0x" + hexLower(sourceStateVerifierHashBytes);
    return new NormalizedShardStateSourceStateInput(
        1,
        input.sourceDomain(),
        masterchainSeqno,
        input.masterchainWorkchainId(),
        masterchainShard,
        normalizeNonZeroHex32(input.masterchainBlockHash(), "masterchainBlockHash"),
        normalizeNonZeroHex32(input.masterchainFileHash(), "masterchainFileHash"),
        normalizeHex32(input.validatorSetHash(), "validatorSetHash"),
        normalizeHex32(input.masterchainConfigRoot(), "masterchainConfigRoot"),
        normalizeHex32(input.masterchainConfigProofHash(), "masterchainConfigProofHash"),
        input.shardWorkchainId(),
        shardShard,
        shardSeqno,
        normalizeHex32(input.shardBlockHash(), "shardBlockHash"),
        normalizeNonZeroHex32(input.shardFileHash(), "shardFileHash"),
        shardStateRoot,
        transactionRoot,
        transactionLt,
        dictionaryRoot,
        input.shardStateDictionaryKeyBitLen(),
        dictionaryKey,
        normalizeHex32(input.masterchainSignatureHash(), "masterchainSignatureHash"),
        normalizeHex32(input.shardProofHash(), "shardProofHash"),
        shardStateProofBoc.raw,
        shardAccountsProofBoc.raw,
        configProofBoc.raw,
        shardStateProofBoc.hash,
        shardAccountsProofBoc.hash,
        configProofBoc.hash,
        Collections.unmodifiableList(transitionProofs),
        validatorSetTransitionChainHash(transitionProofs),
        sourceStateVerifierId,
        sourceStateVerifierHash,
        normalizeNonEmpty(input.sourceTrustAnchorId(), "sourceTrustAnchorId"),
        normalizeNonZeroHex32(input.sourceTrustAnchorHash(), "sourceTrustAnchorHash"),
        normalizeNonEmpty(input.consensusVerifierId(), "consensusVerifierId"),
        normalizeNonZeroHex32(input.consensusVerifierHash(), "consensusVerifierHash"),
        normalizeNonEmpty(input.messageInclusionVerifierId(), "messageInclusionVerifierId"),
        normalizeNonZeroHex32(input.messageInclusionVerifierHash(), "messageInclusionVerifierHash"),
        normalizeNonEmpty(input.finalityPolicyId(), "finalityPolicyId"),
        normalizeNonZeroHex32(input.finalityPolicyHash(), "finalityPolicyHash"));
  }

  private static ParsedBoc parseBocCompleteOrdinary(final byte[] input) {
    final byte[] boc = Objects.requireNonNull(input, "boc").clone();
    if (boc.length < BOC_MAGIC.length + 2 || boc.length > MAX_BOC_BYTES) {
      throw new IllegalArgumentException("TON BoC length is invalid");
    }
    for (int index = 0; index < BOC_MAGIC.length; index++) {
      if (boc[index] != BOC_MAGIC[index]) {
        throw new IllegalArgumentException("TON BoC magic is invalid");
      }
    }
    final Cursor cursor = new Cursor(BOC_MAGIC.length);
    final int flagsSize = boc[cursor.offset] & 0xff;
    cursor.offset++;
    final boolean hasIndex = (flagsSize & 0x80) != 0;
    final boolean hasCrc32c = (flagsSize & 0x40) != 0;
    final boolean hasCacheBits = (flagsSize & 0x20) != 0;
    final int flags = (flagsSize >>> 3) & 0x03;
    final int sizeBytes = flagsSize & 0x07;
    final int offsetBytes = boc[cursor.offset] & 0xff;
    cursor.offset++;
    if (hasCacheBits
        || flags != 0
        || sizeBytes < 1
        || sizeBytes > 4
        || offsetBytes < 1
        || offsetBytes > 8) {
      throw new IllegalArgumentException("TON BoC header flags are unsupported");
    }
    final int cellsCount = readSizedUInt(boc, cursor, sizeBytes);
    final int rootsCount = readSizedUInt(boc, cursor, sizeBytes);
    final int absentCount = readSizedUInt(boc, cursor, sizeBytes);
    final int totalCellsSize = readSizedUInt(boc, cursor, offsetBytes);
    if (cellsCount <= 0
        || cellsCount > MAX_BOC_CELLS
        || rootsCount <= 0
        || rootsCount > cellsCount
        || absentCount != 0
        || rootsCount + absentCount > cellsCount) {
      throw new IllegalArgumentException("TON BoC counts are invalid");
    }
    final ArrayList<Integer> roots = new ArrayList<>(rootsCount);
    for (int index = 0; index < rootsCount; index++) {
      final int root = readSizedUInt(boc, cursor, sizeBytes);
      if (root >= cellsCount) {
        throw new IllegalArgumentException("TON BoC root index is invalid");
      }
      roots.add(root);
    }
    if (hasIndex) {
      int previous = 0;
      for (int index = 0; index < cellsCount; index++) {
        final int cellOffset = readSizedUInt(boc, cursor, offsetBytes);
        if (cellOffset < previous || cellOffset > totalCellsSize) {
          throw new IllegalArgumentException("TON BoC index is invalid");
        }
        if (index + 1 == cellsCount && cellOffset != totalCellsSize) {
          throw new IllegalArgumentException("TON BoC index is invalid");
        }
        previous = cellOffset;
      }
    }
    if (totalCellsSize > boc.length - cursor.offset) {
      throw new IllegalArgumentException("TON BoC cell data length is invalid");
    }
    final int cellDataEnd = cursor.offset + totalCellsSize;
    final int expectedEnd = cellDataEnd + (hasCrc32c ? 4 : 0);
    if (expectedEnd != boc.length) {
      throw new IllegalArgumentException("TON BoC cell data length is invalid");
    }
    if (hasCrc32c) {
      final int expectedCrc = crc32c(boc, cellDataEnd);
      if ((boc[cellDataEnd] & 0xff) != (expectedCrc & 0xff)
          || (boc[cellDataEnd + 1] & 0xff) != ((expectedCrc >>> 8) & 0xff)
          || (boc[cellDataEnd + 2] & 0xff) != ((expectedCrc >>> 16) & 0xff)
          || (boc[cellDataEnd + 3] & 0xff) != ((expectedCrc >>> 24) & 0xff)) {
        throw new IllegalArgumentException("TON BoC CRC32C is invalid");
      }
    }
    final byte[] cellData = Arrays.copyOfRange(boc, cursor.offset, cellDataEnd);
    final Cursor cellCursor = new Cursor(0);
    final ArrayList<BocCell> cells = new ArrayList<>(cellsCount);
    for (int cellIndex = 0; cellIndex < cellsCount; cellIndex++) {
      if (cellCursor.offset + 2 > cellData.length) {
        throw new IllegalArgumentException("TON BoC cell is truncated");
      }
      final int descriptor = cellData[cellCursor.offset] & 0xff;
      cellCursor.offset++;
      final int dataDescriptor = cellData[cellCursor.offset] & 0xff;
      cellCursor.offset++;
      final int refsCount = descriptor & 0x07;
      final boolean exotic = (descriptor & 0x08) != 0;
      final boolean hasHashes = (descriptor & 0x10) != 0;
      final int level = (descriptor >>> 5) & 0x07;
      final int dataBytes = (dataDescriptor + 1) / 2;
      if (refsCount > MAX_REFS
          || hasHashes
          || dataBytes > MAX_CELL_SERIALIZED_DATA_BYTES
          || cellCursor.offset + dataBytes > cellData.length) {
        throw new IllegalArgumentException("TON BoC cell descriptor is unsupported");
      }
      final byte[] data = Arrays.copyOfRange(cellData, cellCursor.offset, cellCursor.offset + dataBytes);
      if (!cellDataPaddingIsValid(dataDescriptor, data)) {
        throw new IllegalArgumentException("TON BoC cell data padding is invalid");
      }
      cellCursor.offset += dataBytes;
      final ArrayList<Integer> refs = new ArrayList<>(refsCount);
      for (int ref = 0; ref < refsCount; ref++) {
        final int refIndex = readSizedUInt(cellData, cellCursor, sizeBytes);
        if (refIndex >= cellsCount || refIndex <= cellIndex) {
          throw new IllegalArgumentException("TON BoC cell refs must be forward internal refs");
        }
        refs.add(refIndex);
      }
      cells.add(
          new BocCell((byte) (descriptor & 0xef), (byte) dataDescriptor, data, refs, level, exotic));
    }
    if (cellCursor.offset != cellData.length) {
      throw new IllegalArgumentException("TON BoC has trailing cell data");
    }
    return new ParsedBoc(roots, cells);
  }

  private static List<BocComputedCell> bocCellHashes(final List<BocCell> cells) {
    final ArrayList<BocComputedCell> computed = new ArrayList<>(cells.size());
    for (int index = 0; index < cells.size(); index++) {
      computed.add(emptyBocComputedCell());
    }
    for (int index = cells.size() - 1; index >= 0; index--) {
      final BocCell cell = cells.get(index);
      final BocCellKind cellKind = bocCellKind(cell);
      final BocPrunedBranch pruned =
          cellKind == BocCellKind.PRUNED_BRANCH ? parsePrunedBranch(cell) : null;
      final int mask;
      if (cellKind == BocCellKind.ORDINARY) {
        int ordinaryMask = 0;
        for (final int ref : cell.refs) {
          ordinaryMask |= computed.get(ref).mask;
        }
        mask = ordinaryMask;
      } else if (cellKind == BocCellKind.PRUNED_BRANCH) {
        mask = pruned.mask;
      } else if (cellKind == BocCellKind.MERKLE_PROOF) {
        if (!cellSerializedBitLenIsByteAligned(cell.dataDescriptor & 0xff, cell.data)
            || cell.data.length != 35
            || cell.refs.size() != 1) {
          throw new IllegalArgumentException("TON BoC Merkle proof cell is invalid");
        }
        final BocComputedCell childComputed = computed.get(cell.refs.get(0));
        final HashDepth child = childHashDepthForLevel(childComputed, 0);
        final byte[] proofHash = Arrays.copyOfRange(cell.data, 1, 33);
        final int proofDepth = ((cell.data[33] & 0xff) << 8) | (cell.data[34] & 0xff);
        if (!Arrays.equals(proofHash, child.hash) || proofDepth != child.depth) {
          throw new IllegalArgumentException("TON BoC Merkle proof cell is invalid");
        }
        mask = levelMaskValue(childComputed.mask >>> 1);
      } else {
        if (!cellSerializedBitLenIsByteAligned(cell.dataDescriptor & 0xff, cell.data)
            || cell.data.length != 69
            || cell.refs.size() != 2) {
          throw new IllegalArgumentException("TON BoC Merkle update cell is invalid");
        }
        final int[] hashOffsets = {1, 33};
        final int[] depthOffsets = {65, 67};
        for (int refPos = 0; refPos < 2; refPos++) {
          final HashDepth child = childHashDepthForLevel(computed.get(cell.refs.get(refPos)), 0);
          final int hashOffset = hashOffsets[refPos];
          final int depthOffset = depthOffsets[refPos];
          final byte[] proofHash = Arrays.copyOfRange(cell.data, hashOffset, hashOffset + 32);
          final int proofDepth =
              ((cell.data[depthOffset] & 0xff) << 8) | (cell.data[depthOffset + 1] & 0xff);
          if (!Arrays.equals(proofHash, child.hash) || proofDepth != child.depth) {
            throw new IllegalArgumentException("TON BoC Merkle update cell is invalid");
          }
        }
        mask =
            levelMaskValue(
                (computed.get(cell.refs.get(0)).mask | computed.get(cell.refs.get(1)).mask) >>> 1);
      }
      if (cell.level != mask) {
        throw new IllegalArgumentException("TON BoC cell level mask is invalid");
      }

      final int totalHashCount = levelMaskHashIndex(mask) + 1;
      final int hashCount = cellKind == BocCellKind.PRUNED_BRANCH ? 1 : totalHashCount;
      final int hashOffset = totalHashCount - hashCount;
      final ArrayList<byte[]> computedHashes = new ArrayList<>(hashCount);
      final ArrayList<Integer> computedDepths = new ArrayList<>(hashCount);
      final int level = levelMaskLevel(mask);
      int hashIndex = 0;
      for (int levelIndex = 0; levelIndex <= level; levelIndex++) {
        if (!levelMaskIsSignificant(mask, levelIndex)) {
          continue;
        }
        if (hashIndex < hashOffset) {
          hashIndex++;
          continue;
        }
        final byte[] currentData;
        if (hashIndex == hashOffset) {
          if (levelIndex != 0 && cellKind != BocCellKind.PRUNED_BRANCH) {
            throw new IllegalArgumentException("TON BoC cell hash level is invalid");
          }
          currentData = cell.data;
        } else {
          currentData = computedHashes.get(hashIndex - hashOffset - 1);
        }

        int currentDepth = 0;
        for (final int ref : cell.refs) {
          final HashDepth child = bocChildForHashLevel(cellKind, computed.get(ref), levelIndex);
          currentDepth = Math.max(currentDepth, child.depth);
        }
        if (!cell.refs.isEmpty()) {
          currentDepth++;
        }
        if (currentDepth > 0xffff) {
          throw new IllegalArgumentException("TON BoC cell depth is invalid");
        }

        final int appliedMask = levelMaskApply(mask, levelIndex);
        final int descriptor =
            cell.refs.size()
                | (cellKind == BocCellKind.ORDINARY ? 0 : 0x08)
                | (appliedMask << 5);
        final ByteArrayOutputStream representation = new ByteArrayOutputStream();
        representation.write(descriptor);
        representation.write(cell.dataDescriptor & 0xff);
        write(representation, currentData);
        for (final int ref : cell.refs) {
          final HashDepth child = bocChildForHashLevel(cellKind, computed.get(ref), levelIndex);
          writeU16Be(representation, child.depth);
        }
        for (final int ref : cell.refs) {
          final HashDepth child = bocChildForHashLevel(cellKind, computed.get(ref), levelIndex);
          write(representation, child.hash);
        }
        computedHashes.add(sha256(representation.toByteArray()));
        computedDepths.add(currentDepth);
        hashIndex++;
      }
      if (computedHashes.size() != hashCount || computedDepths.size() != hashCount) {
        throw new IllegalArgumentException("TON BoC cell hashes are invalid");
      }

      final ArrayList<byte[]> resolvedHashes = new ArrayList<>(4);
      final ArrayList<Integer> resolvedDepths = new ArrayList<>(4);
      for (int resolvedLevel = 0; resolvedLevel < 4; resolvedLevel++) {
        final int resolvedHashIndex = levelMaskHashIndex(levelMaskApply(mask, resolvedLevel));
        if (pruned != null) {
          final int thisHashIndex = levelMaskHashIndex(mask);
          if (resolvedHashIndex != thisHashIndex) {
            resolvedHashes.add(pruned.hashes.get(resolvedHashIndex));
            resolvedDepths.add(pruned.depths.get(resolvedHashIndex));
          } else {
            resolvedHashes.add(computedHashes.get(0));
            resolvedDepths.add(computedDepths.get(0));
          }
        } else {
          resolvedHashes.add(computedHashes.get(resolvedHashIndex));
          resolvedDepths.add(computedDepths.get(resolvedHashIndex));
        }
      }

      computed.set(index, new BocComputedCell(mask, resolvedHashes, resolvedDepths));
    }
    return computed;
  }

  private static BocRootAndChildIndex bocProofRootAndChildIndex(
      final ParsedBoc parsed, final List<BocComputedCell> computed) {
    if (parsed.roots.size() != 1) {
      throw new IllegalArgumentException("TON BoC must contain exactly one root");
    }
    final int rootIndex = parsed.roots.get(0);
    if (rootIndex < 0 || rootIndex >= parsed.cells.size() || rootIndex >= computed.size()) {
      throw new IllegalArgumentException("TON BoC root index is invalid");
    }
    final BocCell root = parsed.cells.get(rootIndex);
    final BocCellKind rootKind = bocCellKind(root);
    if (rootKind == BocCellKind.ORDINARY) {
      return new BocRootAndChildIndex(computed.get(rootIndex).hashes.get(3), rootIndex);
    }
    if (rootKind == BocCellKind.MERKLE_PROOF) {
      if (root.refs.size() != 1 || root.data.length < 33) {
        throw new IllegalArgumentException("TON BoC Merkle proof cell is invalid");
      }
      return new BocRootAndChildIndex(Arrays.copyOfRange(root.data, 1, 33), root.refs.get(0));
    }
    throw new IllegalArgumentException("TON shard-state proof root is pruned or unsupported");
  }

  private static boolean shardStateAccountKeyMatchesShardPrefix(
      final byte[] key, final int keyBitLen, final ShardStateAccountsOpening opening) {
    if (keyBitLen != SHARD_ACCOUNT_KEY_BITS) {
      return false;
    }
    for (int bitIndex = 0; bitIndex < opening.shardPfxBits; bitIndex++) {
      if (hashmapKeyBit(key, keyBitLen, bitIndex) != opening.shardPrefixBits.get(bitIndex)) {
        return false;
      }
    }
    return true;
  }

  private static BigInteger shardIdFromPrefixBits(
      final int shardPfxBits, final List<Boolean> shardPrefixBits) {
    if (shardPfxBits < 0 || shardPfxBits > 60) {
      throw new IllegalArgumentException("TON ShardIdent prefix length is invalid");
    }
    BigInteger shardId = BigInteger.ZERO;
    for (int bitIndex = 0; bitIndex < shardPfxBits; bitIndex++) {
      if (Boolean.TRUE.equals(shardPrefixBits.get(bitIndex))) {
        shardId = shardId.setBit(63 - bitIndex);
      }
    }
    return shardId.setBit(63 - shardPfxBits);
  }

  private static ShardStateAccountsOpening shardStateUnsplitAccountsOpeningFromCell(
      final List<BocCell> cells, final List<BocComputedCell> computed, final int cellIndex) {
    if (cellIndex < 0 || cellIndex >= cells.size()) {
      throw new IllegalArgumentException("TON ShardStateUnsplit cell index is invalid");
    }
    final BocCell cell = cells.get(cellIndex);
    if (bocCellKind(cell) != BocCellKind.ORDINARY) {
      throw new IllegalArgumentException("TON ShardStateUnsplit root must be ordinary");
    }
    final BocBitReader reader = new BocBitReader(cell);
    if (reader.readUInt(32) != SHARD_STATE_UNSPLIT_TAG) {
      throw new IllegalArgumentException("TON ShardStateUnsplit tag is invalid");
    }
    final int globalId = reader.readUInt(32);
    if (reader.readUInt(2) != 0) {
      throw new IllegalArgumentException("TON ShardIdent tag is invalid");
    }
    final int shardPfxBits = reader.readUInt(6);
    if (shardPfxBits > 60) {
      throw new IllegalArgumentException("TON ShardIdent prefix length is invalid");
    }
    final int workchainId = reader.readUInt(32);
    final ArrayList<Boolean> shardPrefixBits = new ArrayList<>(64);
    for (int bitIndex = 0; bitIndex < 64; bitIndex++) {
      shardPrefixBits.add(reader.readBit());
    }
    final int seqNo = reader.readUInt(32);
    reader.readUInt(32);
    final int genUtime = reader.readUInt(32);
    final long genLt = reader.readUInt64(64);
    final int minRefMcSeqno = reader.readUInt(32);
    final int outMsgQueueInfoRef = reader.readRef();
    if (outMsgQueueInfoRef < 0 || outMsgQueueInfoRef >= computed.size()) {
      throw new IllegalArgumentException("TON ShardStateUnsplit out_msg_queue_info ref is invalid");
    }
    reader.readBit();
    final int accountsRef = reader.readRef();
    if (accountsRef < 0 || accountsRef >= computed.size()) {
      throw new IllegalArgumentException("TON ShardStateUnsplit accounts ref is invalid");
    }
    final int trailingFieldsRef = reader.readRef();
    if (trailingFieldsRef < 0 || trailingFieldsRef >= computed.size()) {
      throw new IllegalArgumentException("TON ShardStateUnsplit trailing fields ref is invalid");
    }
    if (reader.readBit()) {
      if (workchainId == TON_BASECHAIN_WORKCHAIN_ID) {
        throw new IllegalArgumentException("TON basechain ShardStateUnsplit custom must be absent");
      }
      final int customRef = reader.readRef();
      if (customRef < 0 || customRef >= computed.size()) {
        throw new IllegalArgumentException("TON ShardStateUnsplit custom ref is invalid");
      }
    }
    if (!reader.isExhausted()) {
      throw new IllegalArgumentException("TON ShardStateUnsplit has trailing data");
    }
    return new ShardStateAccountsOpening(
        "0x" + hexLower(computed.get(accountsRef).hashes.get(3)),
        globalId,
        workchainId,
        seqNo,
        genUtime,
        genLt,
        minRefMcSeqno,
        shardPfxBits,
        shardPrefixBits,
        shardIdFromPrefixBits(shardPfxBits, shardPrefixBits));
  }

  private static BocComputedCell emptyBocComputedCell() {
    final ArrayList<byte[]> hashes = new ArrayList<>(4);
    final ArrayList<Integer> depths = new ArrayList<>(4);
    for (int index = 0; index < 4; index++) {
      hashes.add(new byte[32]);
      depths.add(0);
    }
    return new BocComputedCell(0, hashes, depths);
  }

  private static HashDepth bocChildForHashLevel(
      final BocCellKind cellKind, final BocComputedCell computed, final int level) {
    final int childLevel =
        cellKind == BocCellKind.MERKLE_PROOF || cellKind == BocCellKind.MERKLE_UPDATE
            ? level + 1
            : level;
    return childHashDepthForLevel(computed, childLevel);
  }

  private static int readSizedUInt(final byte[] bytes, final Cursor cursor, final int size) {
    if (size < 1 || size > 8 || cursor.offset + size > bytes.length) {
      throw new IllegalArgumentException("TON BoC is truncated");
    }
    long value = 0;
    for (int index = 0; index < size; index++) {
      value = (value << 8) | (bytes[cursor.offset + index] & 0xffL);
    }
    cursor.offset += size;
    if (value > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("TON sized integer overflows");
    }
    return (int) value;
  }

  private static int crc32c(final byte[] bytes, final int end) {
    int crc = -1;
    for (int index = 0; index < end; index++) {
      crc ^= bytes[index] & 0xff;
      for (int bit = 0; bit < 8; bit++) {
        final int mask = -(crc & 1);
        crc = (crc >>> 1) ^ (CRC32C_REFLECTED_POLY & mask);
      }
    }
    return ~crc;
  }

  private static boolean cellDataPaddingIsValid(final int dataDescriptor, final byte[] data) {
    return (dataDescriptor & 1) == 0 || (data.length > 0 && data[data.length - 1] != 0);
  }

  private static boolean cellSerializedBitLenIsByteAligned(
      final int dataDescriptor, final byte[] data) {
    return (dataDescriptor & 1) == 0 && dataDescriptor / 2 == data.length;
  }

  private static int cellSerializedBitLen(final int dataDescriptor, final byte[] data) {
    if ((dataDescriptor & 1) == 0) {
      final int byteLen = dataDescriptor / 2;
      if (byteLen != data.length) {
        throw new IllegalArgumentException("TON BoC cell data length is invalid");
      }
      return byteLen * 8;
    }
    final int fullBytes = (dataDescriptor + 1) / 2;
    final int floorBytes = dataDescriptor / 2;
    if (fullBytes != data.length || floorBytes + 1 != fullBytes || data.length == 0) {
      throw new IllegalArgumentException("TON BoC cell data length is invalid");
    }
    final int last = data[data.length - 1] & 0xff;
    if (last == 0) {
      throw new IllegalArgumentException("TON BoC cell data padding is invalid");
    }
    return floorBytes * 8 + (7 - Integer.numberOfTrailingZeros(last));
  }

  private static int hashmapUintLenBits(final int maxValue) {
    int value = maxValue;
    int bits = 0;
    while (value > 0) {
      bits++;
      value >>>= 1;
    }
    return bits;
  }

  private static boolean hashmapKeyIsCanonical(final byte[] key, final int keyBitLen) {
    if (keyBitLen < 0 || keyBitLen > 0xffff) {
      return false;
    }
    final int expectedBytes = (keyBitLen + 7) / 8;
    if (key.length != expectedBytes) {
      return false;
    }
    final int unused = expectedBytes * 8 - keyBitLen;
    return unused == 0 || ((key[key.length - 1] & ((1 << unused) - 1)) == 0);
  }

  private static boolean hashmapKeyBit(
      final byte[] key, final int keyBitLen, final int bitIndex) {
    if (bitIndex >= keyBitLen) {
      throw new IllegalArgumentException("TON HashmapE key bit is out of range");
    }
    return (((key[bitIndex / 8] & 0xff) >>> (7 - (bitIndex % 8))) & 1) != 0;
  }

  private static Integer hashmapUnwrapMerkleProofCell(
      final List<BocCell> cells, final int cellIndex) {
    if (cellIndex < 0 || cellIndex >= cells.size()) {
      throw new IllegalArgumentException("TON HashmapE cell index is invalid");
    }
    final BocCell cell = cells.get(cellIndex);
    final BocCellKind kind = bocCellKind(cell);
    if (kind == BocCellKind.ORDINARY) {
      return cellIndex;
    }
    if (kind == BocCellKind.MERKLE_PROOF) {
      if (cell.refs.size() != 1) {
        throw new IllegalArgumentException("TON BoC Merkle proof cell is invalid");
      }
      return cell.refs.get(0);
    }
    return null;
  }

  private static Integer hashmapReadLabel(
      final BocBitReader reader,
      final byte[] key,
      final int keyBitLen,
      final int keyOffset,
      final int maxLen) {
    if (!reader.readBit()) {
      int labelLen = 0;
      while (reader.readBit()) {
        labelLen++;
        if (labelLen > maxLen) {
          return null;
        }
      }
      for (int offset = 0; offset < labelLen; offset++) {
        if (reader.readBit() != hashmapKeyBit(key, keyBitLen, keyOffset + offset)) {
          return null;
        }
      }
      return labelLen;
    }
    if (!reader.readBit()) {
      final int labelLen = reader.readUInt(hashmapUintLenBits(maxLen));
      if (labelLen > maxLen) {
        return null;
      }
      for (int offset = 0; offset < labelLen; offset++) {
        if (reader.readBit() != hashmapKeyBit(key, keyBitLen, keyOffset + offset)) {
          return null;
        }
      }
      return labelLen;
    }
    final boolean labelBit = reader.readBit();
    final int labelLen = reader.readUInt(hashmapUintLenBits(maxLen));
    if (labelLen > maxLen) {
      return null;
    }
    for (int offset = 0; offset < labelLen; offset++) {
      if (labelBit != hashmapKeyBit(key, keyBitLen, keyOffset + offset)) {
        return null;
      }
    }
    return labelLen;
  }

  private static ArrayList<Boolean> hashmapReadLabelBits(
      final BocBitReader reader, final int maxLen) {
    final ArrayList<Boolean> bits = new ArrayList<>();
    if (!reader.readBit()) {
      int labelLen = 0;
      while (reader.readBit()) {
        labelLen++;
        if (labelLen > maxLen) {
          return null;
        }
      }
      for (int offset = 0; offset < labelLen; offset++) {
        bits.add(reader.readBit());
      }
      return bits;
    }
    if (!reader.readBit()) {
      final int labelLen = reader.readUInt(hashmapUintLenBits(maxLen));
      if (labelLen > maxLen) {
        return null;
      }
      for (int offset = 0; offset < labelLen; offset++) {
        bits.add(reader.readBit());
      }
      return bits;
    }
    final boolean labelBit = reader.readBit();
    final int labelLen = reader.readUInt(hashmapUintLenBits(maxLen));
    if (labelLen > maxLen) {
      return null;
    }
    for (int offset = 0; offset < labelLen; offset++) {
      bits.add(labelBit);
    }
    return bits;
  }

  private static String hashmapCellRefValueHash(
      final List<BocCell> cells,
      final List<BocComputedCell> computed,
      final int rootIndex,
      final byte[] key,
      final int keyBitLen) {
    Integer maybeCellIndex = hashmapUnwrapMerkleProofCell(cells, rootIndex);
    if (maybeCellIndex == null) {
      return null;
    }
    int cellIndex = maybeCellIndex;
    int keyOffset = 0;
    int remaining = keyBitLen;
    for (int step = 0; step <= cells.size(); step++) {
      maybeCellIndex = hashmapUnwrapMerkleProofCell(cells, cellIndex);
      if (maybeCellIndex == null) {
        return null;
      }
      cellIndex = maybeCellIndex;
      final BocBitReader reader = new BocBitReader(cells.get(cellIndex));
      final Integer labelLen = hashmapReadLabel(reader, key, keyBitLen, keyOffset, remaining);
      if (labelLen == null) {
        return null;
      }
      keyOffset += labelLen;
      remaining -= labelLen;
      if (remaining == 0) {
        if (reader.remainingBits() != 0 || reader.remainingRefs() != 1) {
          return null;
        }
        final int valueRef = reader.readRef();
        if (bocCellKind(cells.get(valueRef)) == BocCellKind.PRUNED_BRANCH) {
          return null;
        }
        return "0x" + hexLower(computed.get(valueRef).hashes.get(3));
      }
      if (reader.remainingBits() != 0 || reader.remainingRefs() != 2) {
        return null;
      }
      final boolean nextBit = hashmapKeyBit(key, keyBitLen, keyOffset);
      keyOffset++;
      remaining--;
      final int leftRef = reader.readRef();
      final int rightRef = reader.readRef();
      cellIndex = nextBit ? rightRef : leftRef;
    }
    return null;
  }

  private static Integer hashmapCellRefValueIndex(
      final List<BocCell> cells, final int rootIndex, final byte[] key, final int keyBitLen) {
    Integer maybeCellIndex = hashmapUnwrapMerkleProofCell(cells, rootIndex);
    if (maybeCellIndex == null) {
      return null;
    }
    int cellIndex = maybeCellIndex;
    int keyOffset = 0;
    int remaining = keyBitLen;
    for (int step = 0; step <= cells.size(); step++) {
      maybeCellIndex = hashmapUnwrapMerkleProofCell(cells, cellIndex);
      if (maybeCellIndex == null) {
        return null;
      }
      cellIndex = maybeCellIndex;
      final BocBitReader reader = new BocBitReader(cells.get(cellIndex));
      final Integer labelLen = hashmapReadLabel(reader, key, keyBitLen, keyOffset, remaining);
      if (labelLen == null) {
        return null;
      }
      keyOffset += labelLen;
      remaining -= labelLen;
      if (remaining == 0) {
        if (reader.remainingBits() != 0 || reader.remainingRefs() != 1) {
          return null;
        }
        final int valueRef = reader.readRef();
        if (valueRef < 0
            || valueRef >= cells.size()
            || bocCellKind(cells.get(valueRef)) == BocCellKind.PRUNED_BRANCH) {
          return null;
        }
        return valueRef;
      }
      if (reader.remainingBits() != 0 || reader.remainingRefs() != 2) {
        return null;
      }
      final boolean nextBit = hashmapKeyBit(key, keyBitLen, keyOffset);
      keyOffset++;
      remaining--;
      final int leftRef = reader.readRef();
      final int rightRef = reader.readRef();
      cellIndex = nextBit ? rightRef : leftRef;
    }
    return null;
  }

  private static Integer bitsToU16(final List<Boolean> bits) {
    if (bits.size() > 16) {
      return null;
    }
    int value = 0;
    for (final Boolean bit : bits) {
      value = (value << 1) | (Boolean.TRUE.equals(bit) ? 1 : 0);
    }
    return value;
  }

  private static byte[] readEd25519SigPubkey(final BocBitReader reader) {
    if (reader.readUInt(32) != ED25519_PUBKEY_CONSTRUCTOR) {
      return null;
    }
    return reader.readBytes(32);
  }

  private static TonValidatorDescr readValidatorDescr(final BocBitReader reader) {
    final int constructor = reader.readUInt(8);
    if (constructor != VALIDATOR_CONSTRUCTOR && constructor != VALIDATOR_ADDR_CONSTRUCTOR) {
      return null;
    }
    final byte[] publicKey = readEd25519SigPubkey(reader);
    if (publicKey == null) {
      return null;
    }
    final BigInteger weight = reader.readUIntBigInteger(64);
    if (BigInteger.ZERO.equals(weight)) {
      return null;
    }
    if (constructor == VALIDATOR_ADDR_CONSTRUCTOR) {
      reader.skipBits(256);
    }
    return new TonValidatorDescr(0, publicKey, weight);
  }

  private static void trimPrefix(final ArrayList<Boolean> prefix, final int count) {
    for (int index = 0; index < count; index++) {
      prefix.remove(prefix.size() - 1);
    }
  }

  private static boolean collectValidatorDescrsFromReader(
      final List<BocCell> cells,
      final BocBitReader reader,
      final int remaining,
      final ArrayList<Boolean> prefix,
      final ArrayList<TonValidatorDescr> out,
      final int[] budget) {
    if (budget[0] <= 0 || out.size() > MAX_VALIDATORS) {
      return false;
    }
    budget[0]--;
    final ArrayList<Boolean> labelBits = hashmapReadLabelBits(reader, remaining);
    if (labelBits == null) {
      return false;
    }
    prefix.addAll(labelBits);
    final int nextRemaining = remaining - labelBits.size();
    if (nextRemaining == 0) {
      final Integer key = bitsToU16(prefix);
      final TonValidatorDescr validator = readValidatorDescr(reader);
      trimPrefix(prefix, labelBits.size());
      if (key == null || validator == null || !reader.isExhausted()) {
        return false;
      }
      out.add(new TonValidatorDescr(key, validator.publicKey, validator.weight));
      return true;
    }
    if (reader.remainingBits() != 0 || reader.remainingRefs() != 2) {
      trimPrefix(prefix, labelBits.size());
      return false;
    }
    final int leftRef = reader.readRef();
    final int rightRef = reader.readRef();
    prefix.add(false);
    final boolean leftOk =
        collectValidatorDescrsFromCell(cells, leftRef, nextRemaining - 1, prefix, out, budget);
    prefix.remove(prefix.size() - 1);
    if (!leftOk) {
      trimPrefix(prefix, labelBits.size());
      return false;
    }
    prefix.add(true);
    final boolean rightOk =
        collectValidatorDescrsFromCell(cells, rightRef, nextRemaining - 1, prefix, out, budget);
    prefix.remove(prefix.size() - 1);
    trimPrefix(prefix, labelBits.size());
    return rightOk;
  }

  private static boolean collectValidatorDescrsFromCell(
      final List<BocCell> cells,
      final int cellIndex,
      final int remaining,
      final ArrayList<Boolean> prefix,
      final ArrayList<TonValidatorDescr> out,
      final int[] budget) {
    if (cellIndex < 0
        || cellIndex >= cells.size()
        || bocCellKind(cells.get(cellIndex)) != BocCellKind.ORDINARY) {
      return false;
    }
    return collectValidatorDescrsFromReader(
        cells, new BocBitReader(cells.get(cellIndex)), remaining, prefix, out, budget);
  }

  private static byte[] validatorSetPayloadFromCell(final List<BocCell> cells, final int cellIndex) {
    if (cellIndex < 0 || cellIndex >= cells.size()) {
      throw new IllegalArgumentException("TON ValidatorSet cell index is invalid");
    }
    final BocCell cell = cells.get(cellIndex);
    if (bocCellKind(cell) != BocCellKind.ORDINARY) {
      throw new IllegalArgumentException("TON ValidatorSet cell must be ordinary");
    }
    final BocBitReader reader = new BocBitReader(cell);
    final int constructor = reader.readUInt(8);
    if (constructor != VALIDATORS_CONSTRUCTOR && constructor != VALIDATORS_EXT_CONSTRUCTOR) {
      throw new IllegalArgumentException("TON ValidatorSet constructor is unsupported");
    }
    final BigInteger utimeSince = reader.readUIntBigInteger(32);
    final BigInteger utimeUntil = reader.readUIntBigInteger(32);
    if (utimeUntil.compareTo(utimeSince) <= 0) {
      throw new IllegalArgumentException("TON ValidatorSet validity interval is invalid");
    }
    final int total = reader.readUInt(16);
    final int main = reader.readUInt(16);
    if (total == 0 || total > MAX_VALIDATORS || main == 0 || main > total) {
      throw new IllegalArgumentException("TON ValidatorSet counts are invalid");
    }
    final BigInteger declaredTotalWeight =
        constructor == VALIDATORS_EXT_CONSTRUCTOR ? reader.readUIntBigInteger(64) : null;
    final ArrayList<TonValidatorDescr> entries = new ArrayList<>(total);
    final int[] budget = {cells.size() + 1};
    final boolean ok;
    if (constructor == VALIDATORS_EXT_CONSTRUCTOR) {
      final boolean hasRoot = reader.readBit();
      if (!hasRoot || reader.remainingBits() != 0 || reader.remainingRefs() != 1) {
        throw new IllegalArgumentException("TON ValidatorSet dictionary root is invalid");
      }
      ok =
          collectValidatorDescrsFromCell(
              cells,
              reader.readRef(),
              VALIDATOR_SET_KEY_BITS,
              new ArrayList<Boolean>(),
              entries,
              budget);
    } else {
      ok =
          collectValidatorDescrsFromReader(
              cells,
              reader,
              VALIDATOR_SET_KEY_BITS,
              new ArrayList<Boolean>(),
              entries,
              budget);
    }
    if (!ok) {
      throw new IllegalArgumentException("TON ValidatorSet dictionary is invalid");
    }
    if (entries.size() != total || entries.size() > MAX_VALIDATORS) {
      throw new IllegalArgumentException("TON ValidatorSet validator count is invalid");
    }
    entries.sort((left, right) -> Integer.compare(left.key, right.key));
    for (int index = 1; index < entries.size(); index++) {
      if (entries.get(index - 1).key >= entries.get(index).key) {
        throw new IllegalArgumentException(
            "TON ValidatorSet dictionary keys must be unique and ordered");
      }
    }
    BigInteger totalWeight = BigInteger.ZERO;
    final ArrayList<byte[]> publicKeys = new ArrayList<>(entries.size());
    final ArrayList<BigInteger> weights = new ArrayList<>(entries.size());
    for (final TonValidatorDescr entry : entries) {
      totalWeight = totalWeight.add(entry.weight);
      publicKeys.add(entry.publicKey);
      weights.add(entry.weight);
    }
    if (declaredTotalWeight != null
        && (BigInteger.ZERO.equals(declaredTotalWeight)
            || !declaredTotalWeight.equals(totalWeight))) {
      throw new IllegalArgumentException("TON ValidatorSet total weight is invalid");
    }
    return canonicalValidatorSetBytesFromParts(publicKeys, weights);
  }

  private static void skipVarUInt(final BocBitReader reader, final int lengthBits) {
    reader.skipBits(reader.readUInt(lengthBits) * 8);
  }

  private static void skipCurrencyCollection(final BocBitReader reader) {
    skipVarUInt(reader, 4);
    if (reader.readBit()) {
      reader.readRef();
    }
  }

  private static void skipDepthBalanceInfo(final BocBitReader reader) {
    final int splitDepth = reader.readUInt(5);
    if (splitDepth > 30) {
      throw new IllegalArgumentException("TON DepthBalanceInfo split depth is invalid");
    }
    skipCurrencyCollection(reader);
  }

  private static ShardAccountLastTransaction readShardAccountLastTransaction(
      final List<BocComputedCell> computed, final BocBitReader reader) {
    skipDepthBalanceInfo(reader);
    final int accountRef = reader.readRef();
    if (accountRef < 0 || accountRef >= computed.size()) {
      throw new IllegalArgumentException("TON ShardAccount account ref is invalid");
    }
    final byte[] lastTransactionHash = reader.readBytes(32);
    final BigInteger lastTransactionLt = reader.readUIntBigInteger(64);
    if (BigInteger.ZERO.equals(lastTransactionLt)) {
      throw new IllegalArgumentException("TON ShardAccount last transaction lt must be non-zero");
    }
    if (!reader.isExhausted()) {
      throw new IllegalArgumentException("TON ShardAccount has trailing data");
    }
    return new ShardAccountLastTransaction("0x" + hexLower(lastTransactionHash), lastTransactionLt);
  }

  private static ShardAccountLastTransaction hashmapShardAccountsLastTransaction(
      final List<BocCell> cells,
      final List<BocComputedCell> computed,
      final int rootIndex,
      final byte[] key,
      final int keyBitLen) {
    Integer maybeCellIndex = hashmapUnwrapMerkleProofCell(cells, rootIndex);
    if (maybeCellIndex == null) {
      return null;
    }
    int cellIndex = maybeCellIndex;
    int keyOffset = 0;
    int remaining = keyBitLen;
    for (int step = 0; step <= cells.size(); step++) {
      maybeCellIndex = hashmapUnwrapMerkleProofCell(cells, cellIndex);
      if (maybeCellIndex == null) {
        return null;
      }
      cellIndex = maybeCellIndex;
      final BocBitReader reader = new BocBitReader(cells.get(cellIndex));
      final Integer labelLen = hashmapReadLabel(reader, key, keyBitLen, keyOffset, remaining);
      if (labelLen == null) {
        return null;
      }
      keyOffset += labelLen;
      remaining -= labelLen;
      if (remaining == 0) {
        return readShardAccountLastTransaction(computed, reader);
      }
      final boolean nextBit = hashmapKeyBit(key, keyBitLen, keyOffset);
      keyOffset++;
      remaining--;
      final int leftRef = reader.readRef();
      final int rightRef = reader.readRef();
      skipDepthBalanceInfo(reader);
      if (!reader.isExhausted()) {
        return null;
      }
      cellIndex = nextBit ? rightRef : leftRef;
    }
    return null;
  }

  private static int levelMaskValue(final int mask) {
    return mask & 0x07;
  }

  private static int levelMaskLevel(final int mask) {
    final int value = levelMaskValue(mask);
    return value == 0 ? 0 : 32 - Integer.numberOfLeadingZeros(value);
  }

  private static int levelMaskHashIndex(final int mask) {
    int value = levelMaskValue(mask);
    int count = 0;
    while (value != 0) {
      count += value & 1;
      value >>>= 1;
    }
    return count;
  }

  private static int levelMaskApply(final int mask, final int level) {
    return level == 0 ? 0 : levelMaskValue(mask) & ((1 << level) - 1);
  }

  private static boolean levelMaskIsSignificant(final int mask, final int level) {
    return level == 0 || ((levelMaskValue(mask) >>> (level - 1)) & 1) != 0;
  }

  private static HashDepth childHashDepthForLevel(
      final BocComputedCell computed, final int level) {
    final int index = Math.min(level, 3);
    return new HashDepth(computed.hashes.get(index), computed.depths.get(index));
  }

  private static BocCellKind bocCellKind(final BocCell cell) {
    if (!cell.exotic) {
      return BocCellKind.ORDINARY;
    }
    if (cell.data.length == 0) {
      throw new IllegalArgumentException("TON BoC exotic cell type is unsupported");
    }
    switch (cell.data[0] & 0xff) {
      case 1:
        return BocCellKind.PRUNED_BRANCH;
      case 3:
        return BocCellKind.MERKLE_PROOF;
      case 4:
        return BocCellKind.MERKLE_UPDATE;
      default:
        throw new IllegalArgumentException("TON BoC exotic cell type is unsupported");
    }
  }

  private static BocPrunedBranch parsePrunedBranch(final BocCell cell) {
    if (!cellSerializedBitLenIsByteAligned(cell.dataDescriptor & 0xff, cell.data)
        || !cell.refs.isEmpty()
        || cell.data.length < 2
        || (cell.data[0] & 0xff) != 1) {
      throw new IllegalArgumentException("TON BoC pruned branch cell is invalid");
    }
    if (cell.data.length == 35) {
      final ArrayList<byte[]> hashes = new ArrayList<>(1);
      hashes.add(Arrays.copyOfRange(cell.data, 1, 33));
      final ArrayList<Integer> depths = new ArrayList<>(1);
      depths.add(((cell.data[33] & 0xff) << 8) | (cell.data[34] & 0xff));
      return new BocPrunedBranch(1, hashes, depths);
    }
    final int mask = levelMaskValue(cell.data[1] & 0xff);
    final int level = levelMaskLevel(mask);
    if (level < 1 || level > 3 || cell.data.length != 2 + level * 34) {
      throw new IllegalArgumentException("TON BoC pruned branch cell is invalid");
    }

    final ArrayList<byte[]> hashes = new ArrayList<>(level);
    for (int index = 0; index < level; index++) {
      final int start = 2 + index * 32;
      hashes.add(Arrays.copyOfRange(cell.data, start, start + 32));
    }

    final ArrayList<Integer> depths = new ArrayList<>(level);
    final int depthsStart = 2 + level * 32;
    for (int index = 0; index < level; index++) {
      final int start = depthsStart + index * 2;
      depths.add(((cell.data[start] & 0xff) << 8) | (cell.data[start + 1] & 0xff));
    }

    return new BocPrunedBranch(mask, hashes, depths);
  }

  private static int minSizeBytes(final int value) {
    final BigInteger numeric = BigInteger.valueOf(value);
    for (int size = 1; size <= 7; size++) {
      if (numeric.compareTo(BigInteger.ONE.shiftLeft(size * 8).subtract(BigInteger.ONE)) <= 0) {
        return size;
      }
    }
    throw new IllegalArgumentException("TON sized integer is too large");
  }

  private static byte[] sizedUInt(final int value, final int size) {
    BigInteger working = BigInteger.valueOf(value);
    final byte[] out = new byte[size];
    for (int index = size - 1; index >= 0; index--) {
      out[index] = working.and(BigInteger.valueOf(0xffL)).byteValue();
      working = working.shiftRight(8);
    }
    if (!BigInteger.ZERO.equals(working)) {
      throw new IllegalArgumentException("TON sized integer overflows");
    }
    return out;
  }

  private static String hashHex(final String prefix, final byte[] payload) {
    return "0x" + hexLower(prefixedHashBytes(prefix, payload));
  }

  private static byte[] prefixedHashBytes(final String prefix, final byte[] payload) {
    final byte[] prefixBytes = prefix.getBytes(java.nio.charset.StandardCharsets.UTF_8);
    final byte[] preimage = new byte[prefixBytes.length + payload.length];
    System.arraycopy(prefixBytes, 0, preimage, 0, prefixBytes.length);
    System.arraycopy(payload, 0, preimage, prefixBytes.length, payload.length);
    return Blake2b.digest256(preimage);
  }

  private static byte[] sha256(final byte[] input) {
    try {
      return MessageDigest.getInstance("SHA-256").digest(input);
    } catch (final java.security.NoSuchAlgorithmException ex) {
      throw new IllegalStateException("sha256 unavailable", ex);
    }
  }

  private static byte[] hex32Bytes(final String value, final String field) {
    Objects.requireNonNull(value, field);
    if (!value.trim().equals(value)) {
      throw new IllegalArgumentException(field + " must be canonical hex");
    }
    String body = value;
    if (body.regionMatches(true, 0, "0x", 0, 2)) {
      body = body.substring(2);
    }
    for (int i = 0; i < body.length(); i++) {
      if (Character.isWhitespace(body.charAt(i))) {
        throw new IllegalArgumentException(field + " must be canonical hex");
      }
    }
    if (body.length() != 64) {
      throw new IllegalArgumentException(field + " must be 32 bytes");
    }
    final byte[] out = new byte[32];
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

  private static byte[] nonZeroHex32Bytes(final String value, final String field) {
    final byte[] bytes = hex32Bytes(value, field);
    if (!containsNonZero(bytes)) {
      throw new IllegalArgumentException(field + " must not be zero");
    }
    return bytes;
  }

  private static String normalizeHex32(final String value, final String field) {
    return "0x" + hexLower(hex32Bytes(value, field));
  }

  private static String normalizeNonZeroHex32(final String value, final String field) {
    return "0x" + hexLower(nonZeroHex32Bytes(value, field));
  }

  private static String normalizeNonEmpty(final String value, final String field) {
    final String trimmed = Objects.requireNonNull(value, field).trim();
    if (trimmed.isEmpty()) {
      throw new IllegalArgumentException(field + " must be non-empty");
    }
    return trimmed;
  }

  private static String normalizePositiveDecimalText(final String value, final String field) {
    Objects.requireNonNull(value, field);
    if (!value.trim().equals(value) || value.isEmpty() || value.charAt(0) == '0') {
      throw new IllegalArgumentException(field + " must be a positive decimal");
    }
    for (int index = 0; index < value.length(); index++) {
      final char symbol = value.charAt(index);
      if (symbol < '0' || symbol > '9') {
        throw new IllegalArgumentException(field + " must be a positive decimal");
      }
    }
    return value;
  }

  private static String normalizeTonActiveAccountStatus(final String value, final String field) {
    Objects.requireNonNull(value, field);
    if (!"active".equals(value)) {
      throw new IllegalArgumentException(field + " must be active");
    }
    return value;
  }

  private static String normalizeTonRawAddress(final String value, final String field) {
    Objects.requireNonNull(value, field);
    if (!value.trim().equals(value)) {
      throw new IllegalArgumentException(field + " must not contain whitespace");
    }
    final int separator = value.indexOf(':');
    if (separator <= 0 || separator != value.lastIndexOf(':')) {
      throw new IllegalArgumentException(field + " must be workchain:account_hex");
    }
    final String workchain = value.substring(0, separator);
    final String accountHex = value.substring(separator + 1);
    final String digits = workchain.startsWith("-") ? workchain.substring(1) : workchain;
    if (digits.isEmpty()
        || workchain.startsWith("+")
        || (workchain.startsWith("-") && "0".equals(digits))
        || (digits.length() > 1 && digits.startsWith("0"))) {
      throw new IllegalArgumentException(field + " workchain must be canonical i32");
    }
    for (int index = 0; index < digits.length(); index++) {
      final char symbol = digits.charAt(index);
      if (symbol < '0' || symbol > '9') {
        throw new IllegalArgumentException(field + " workchain must be canonical i32");
      }
    }
    final int workchainId;
    try {
      workchainId = Integer.parseInt(workchain);
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException(field + " workchain must be canonical i32", ex);
    }
    if (workchainId != TON_BASECHAIN_WORKCHAIN_ID) {
      throw new IllegalArgumentException(field + " workchain must be basechain 0");
    }
    if (accountHex.length() != 64) {
      throw new IllegalArgumentException(field + " account must be 32 bytes");
    }
    for (int index = 0; index < accountHex.length(); index++) {
      final char symbol = accountHex.charAt(index);
      if (!((symbol >= '0' && symbol <= '9') || (symbol >= 'a' && symbol <= 'f'))) {
        throw new IllegalArgumentException(field + " account must be lowercase canonical hex");
      }
    }
    if (!containsNonZero(hex32Bytes(accountHex, field + " account"))) {
      throw new IllegalArgumentException(field + " account must not be zero");
    }
    return value;
  }

  private static ProofContext normalizeProofContext(
      final String statementHash, final String destinationBindingHash) {
    return new ProofContext(
        1,
        normalizeNonZeroHex32(statementHash, "statementHash"),
        normalizeNonZeroHex32(destinationBindingHash, "destinationBindingHash"));
  }

  private static void writeString(
      final ByteArrayOutputStream out, final String value, final String field) {
    final byte[] bytes =
        normalizeNonEmpty(value, field).getBytes(java.nio.charset.StandardCharsets.UTF_8);
    writeU32Le(out, bytes.length);
    write(out, bytes);
  }

  private static BigInteger normalizeU64(final String value, final String field) {
    Objects.requireNonNull(value, field);
    if (!value.trim().equals(value)
        || !value.matches("[0-9]+")
        || (value.length() > 1 && value.startsWith("0"))) {
      throw new IllegalArgumentException(field + " must be a canonical unsigned integer");
    }
    final BigInteger numeric = new BigInteger(value);
    if (numeric.compareTo(MAX_U64) > 0) {
      throw new IllegalArgumentException(field + " must fit u64");
    }
    return numeric;
  }

  private static void writeU16Be(final ByteArrayOutputStream out, final int value) {
    out.write((value >>> 8) & 0xff);
    out.write(value & 0xff);
  }

  private static void writeU16Le(final ByteArrayOutputStream out, final int value) {
    if (value < 0 || value > 0xffff) {
      throw new IllegalArgumentException("u16 value must fit u16");
    }
    out.write(value & 0xff);
    out.write((value >>> 8) & 0xff);
  }

  private static void writeU32Le(final ByteArrayOutputStream out, final int value) {
    out.write(value & 0xff);
    out.write((value >>> 8) & 0xff);
    out.write((value >>> 16) & 0xff);
    out.write((value >>> 24) & 0xff);
  }

  private static void writeI32Le(final ByteArrayOutputStream out, final int value) {
    out.write(value & 0xff);
    out.write((value >>> 8) & 0xff);
    out.write((value >>> 16) & 0xff);
    out.write((value >>> 24) & 0xff);
  }

  private static void writeU32Be(final ByteArrayOutputStream out, final long value) {
    out.write((int) ((value >>> 24) & 0xff));
    out.write((int) ((value >>> 16) & 0xff));
    out.write((int) ((value >>> 8) & 0xff));
    out.write((int) (value & 0xff));
  }

  private static byte[] currentValidatorSetConfigKey() {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeU32Be(out, CURRENT_VALIDATOR_SET_CONFIG_PARAM);
    return out.toByteArray();
  }

  private static void writeU64Le(final ByteArrayOutputStream out, final BigInteger value) {
    BigInteger working = value;
    for (int i = 0; i < 8; i++) {
      out.write(working.and(BigInteger.valueOf(0xffL)).intValue());
      working = working.shiftRight(8);
    }
  }

  private static byte[] sccpWordU32Le(final int value) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeU32Le(out, value);
    return Arrays.copyOf(out.toByteArray(), 32);
  }

  private static byte[] sccpWordU8(final int value) {
    if (value < 0 || value > 0xff) {
      throw new IllegalArgumentException("u8 value must fit u8");
    }
    final byte[] out = new byte[32];
    out[0] = (byte) value;
    return out;
  }

  private static byte[] sccpWordI32Le(final int value) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeI32Le(out, value);
    return Arrays.copyOf(out.toByteArray(), 32);
  }

  private static byte[] sccpWordU64Le(final BigInteger value) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeU64Le(out, value);
    return Arrays.copyOf(out.toByteArray(), 32);
  }

  private static void writeU64Be(final ByteArrayOutputStream out, final BigInteger value) {
    BigInteger working = value;
    final byte[] bytes = new byte[8];
    for (int index = 7; index >= 0; index--) {
      bytes[index] = working.and(BigInteger.valueOf(0xffL)).byteValue();
      working = working.shiftRight(8);
    }
    write(out, bytes);
  }

  private static void write(final ByteArrayOutputStream out, final byte[] bytes) {
    out.write(bytes, 0, bytes.length);
  }

  private static String hexLower(final byte[] bytes) {
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      builder.append(String.format("%02x", b & 0xff));
    }
    return builder.toString();
  }

  private static boolean containsNonZero(final byte[] bytes) {
    for (final byte b : bytes) {
      if (b != 0) {
        return true;
      }
    }
    return false;
  }

  private static boolean isZero(final byte[] bytes) {
    return !containsNonZero(bytes);
  }

  private static List<List<String>> copyColumns(final List<List<String>> columns) {
    final ArrayList<List<String>> copy = new ArrayList<List<String>>();
    for (final List<String> column : columns) {
      copy.add(Collections.unmodifiableList(new ArrayList<String>(column)));
    }
    return Collections.unmodifiableList(copy);
  }

  private static final class AuditRoleProfile {
    private final String name;
    private final int code;
    private final String circuitId;
    private final String verifierId;
    private final List<String> requiredInputNames;

    private AuditRoleProfile(
        final String name,
        final int code,
        final String circuitId,
        final String verifierId,
        final String[] requiredInputNames) {
      this.name = name;
      this.code = code;
      this.circuitId = circuitId;
      this.verifierId = verifierId;
      this.requiredInputNames =
          Collections.unmodifiableList(Arrays.asList(requiredInputNames.clone()));
    }
  }

  private static final class NormalizedFullLightClientAuditInput {
    private final FullLightClientAuditRole role;
    private final NormalizedShardStateSourceStateInput shardState;
    private final String sourceVerifierMaterialHash;
    private final String sourceAdapterDeploymentHash;
    private final String fullLightClientGateHash;
    private final String verifierHash;
    private final String shardStateProofPublicInputsHash;
    private final String shardStateVerificationProofHash;
    private final String validatorSetPayloadHash;
    private final String configLeafHash;
    private final String configValueHash;

    private NormalizedFullLightClientAuditInput(
        final FullLightClientAuditRole role,
        final NormalizedShardStateSourceStateInput shardState,
        final String sourceVerifierMaterialHash,
        final String sourceAdapterDeploymentHash,
        final String fullLightClientGateHash,
        final String verifierHash,
        final String shardStateProofPublicInputsHash,
        final String shardStateVerificationProofHash,
        final String validatorSetPayloadHash,
        final String configLeafHash,
        final String configValueHash) {
      this.role = role;
      this.shardState = shardState;
      this.sourceVerifierMaterialHash = sourceVerifierMaterialHash;
      this.sourceAdapterDeploymentHash = sourceAdapterDeploymentHash;
      this.fullLightClientGateHash = fullLightClientGateHash;
      this.verifierHash = verifierHash;
      this.shardStateProofPublicInputsHash = shardStateProofPublicInputsHash;
      this.shardStateVerificationProofHash = shardStateVerificationProofHash;
      this.validatorSetPayloadHash = validatorSetPayloadHash;
      this.configLeafHash = configLeafHash;
      this.configValueHash = configValueHash;
    }
  }

  private static final class BoundedBocHash {
    private final byte[] raw;
    private final String hash;

    private BoundedBocHash(final byte[] raw, final String hash) {
      this.raw = raw;
      this.hash = hash;
    }
  }

  private static final class NormalizedValidatorSetTransitionProof {
    private final int version;
    private final int sourceDomain;
    private final BigInteger fromValidatorSetSeqno;
    private final BigInteger toValidatorSetSeqno;
    private final BigInteger masterchainSeqno;
    private final int masterchainWorkchainId;
    private final BigInteger masterchainShard;
    private final String masterchainBlockHash;
    private final String masterchainFileHash;
    private final String parentValidatorSetHash;
    private final String nextValidatorSetHash;
    private final byte[] nextValidatorSetPayload;
    private final String nextValidatorSetPayloadHash;
    private final String nextValidatorSetConfigHash;
    private final String transitionMessageHash;
    private final String transitionSignatureHash;
    private final ValidatorSignatureProofInput validatorSignatureProof;

    private NormalizedValidatorSetTransitionProof(
        final int version,
        final int sourceDomain,
        final BigInteger fromValidatorSetSeqno,
        final BigInteger toValidatorSetSeqno,
        final BigInteger masterchainSeqno,
        final int masterchainWorkchainId,
        final BigInteger masterchainShard,
        final String masterchainBlockHash,
        final String masterchainFileHash,
        final String parentValidatorSetHash,
        final String nextValidatorSetHash,
        final byte[] nextValidatorSetPayload,
        final String nextValidatorSetPayloadHash,
        final String nextValidatorSetConfigHash,
        final String transitionMessageHash,
        final String transitionSignatureHash,
        final ValidatorSignatureProofInput validatorSignatureProof) {
      this.version = version;
      this.sourceDomain = sourceDomain;
      this.fromValidatorSetSeqno = fromValidatorSetSeqno;
      this.toValidatorSetSeqno = toValidatorSetSeqno;
      this.masterchainSeqno = masterchainSeqno;
      this.masterchainWorkchainId = masterchainWorkchainId;
      this.masterchainShard = masterchainShard;
      this.masterchainBlockHash = masterchainBlockHash;
      this.masterchainFileHash = masterchainFileHash;
      this.parentValidatorSetHash = parentValidatorSetHash;
      this.nextValidatorSetHash = nextValidatorSetHash;
      this.nextValidatorSetPayload = nextValidatorSetPayload;
      this.nextValidatorSetPayloadHash = nextValidatorSetPayloadHash;
      this.nextValidatorSetConfigHash = nextValidatorSetConfigHash;
      this.transitionMessageHash = transitionMessageHash;
      this.transitionSignatureHash = transitionSignatureHash;
      this.validatorSignatureProof = validatorSignatureProof;
    }
  }

  private static final class NormalizedShardStateSourceStateInput {
    private final int version;
    private final int sourceDomain;
    private final BigInteger masterchainSeqno;
    private final int masterchainWorkchainId;
    private final BigInteger masterchainShard;
    private final String masterchainBlockHash;
    private final String masterchainFileHash;
    private final String validatorSetHash;
    private final String masterchainConfigRoot;
    private final String masterchainConfigProofHash;
    private final int shardWorkchainId;
    private final BigInteger shardShard;
    private final BigInteger shardSeqno;
    private final String shardBlockHash;
    private final String shardFileHash;
    private final String shardStateRoot;
    private final String transactionRoot;
    private final BigInteger transactionLt;
    private final String shardStateDictionaryRoot;
    private final int shardStateDictionaryKeyBitLen;
    private final byte[] shardStateDictionaryKey;
    private final String masterchainSignatureHash;
    private final String shardProofHash;
    private final byte[] shardStateProofBoc;
    private final byte[] shardStateDictionaryProofBoc;
    private final byte[] configDictionaryProofBoc;
    private final String shardStateProofBocHash;
    private final String shardAccountsProofBocHash;
    private final String configProofBocHash;
    private final List<NormalizedValidatorSetTransitionProof> validatorSetTransitionProofs;
    private final String transitionChainHash;
    private final String sourceStateVerifierId;
    private final String sourceStateVerifierHash;
    private final String sourceTrustAnchorId;
    private final String sourceTrustAnchorHash;
    private final String consensusVerifierId;
    private final String consensusVerifierHash;
    private final String messageInclusionVerifierId;
    private final String messageInclusionVerifierHash;
    private final String finalityPolicyId;
    private final String finalityPolicyHash;

    private NormalizedShardStateSourceStateInput(
        final int version,
        final int sourceDomain,
        final BigInteger masterchainSeqno,
        final int masterchainWorkchainId,
        final BigInteger masterchainShard,
        final String masterchainBlockHash,
        final String masterchainFileHash,
        final String validatorSetHash,
        final String masterchainConfigRoot,
        final String masterchainConfigProofHash,
        final int shardWorkchainId,
        final BigInteger shardShard,
        final BigInteger shardSeqno,
        final String shardBlockHash,
        final String shardFileHash,
        final String shardStateRoot,
        final String transactionRoot,
        final BigInteger transactionLt,
        final String shardStateDictionaryRoot,
        final int shardStateDictionaryKeyBitLen,
        final byte[] shardStateDictionaryKey,
        final String masterchainSignatureHash,
        final String shardProofHash,
        final byte[] shardStateProofBoc,
        final byte[] shardStateDictionaryProofBoc,
        final byte[] configDictionaryProofBoc,
        final String shardStateProofBocHash,
        final String shardAccountsProofBocHash,
        final String configProofBocHash,
        final List<NormalizedValidatorSetTransitionProof> validatorSetTransitionProofs,
        final String transitionChainHash,
        final String sourceStateVerifierId,
        final String sourceStateVerifierHash,
        final String sourceTrustAnchorId,
        final String sourceTrustAnchorHash,
        final String consensusVerifierId,
        final String consensusVerifierHash,
        final String messageInclusionVerifierId,
        final String messageInclusionVerifierHash,
        final String finalityPolicyId,
        final String finalityPolicyHash) {
      this.version = version;
      this.sourceDomain = sourceDomain;
      this.masterchainSeqno = masterchainSeqno;
      this.masterchainWorkchainId = masterchainWorkchainId;
      this.masterchainShard = masterchainShard;
      this.masterchainBlockHash = masterchainBlockHash;
      this.masterchainFileHash = masterchainFileHash;
      this.validatorSetHash = validatorSetHash;
      this.masterchainConfigRoot = masterchainConfigRoot;
      this.masterchainConfigProofHash = masterchainConfigProofHash;
      this.shardWorkchainId = shardWorkchainId;
      this.shardShard = shardShard;
      this.shardSeqno = shardSeqno;
      this.shardBlockHash = shardBlockHash;
      this.shardFileHash = shardFileHash;
      this.shardStateRoot = shardStateRoot;
      this.transactionRoot = transactionRoot;
      this.transactionLt = transactionLt;
      this.shardStateDictionaryRoot = shardStateDictionaryRoot;
      this.shardStateDictionaryKeyBitLen = shardStateDictionaryKeyBitLen;
      this.shardStateDictionaryKey = shardStateDictionaryKey;
      this.masterchainSignatureHash = masterchainSignatureHash;
      this.shardProofHash = shardProofHash;
      this.shardStateProofBoc = shardStateProofBoc;
      this.shardStateDictionaryProofBoc = shardStateDictionaryProofBoc;
      this.configDictionaryProofBoc = configDictionaryProofBoc;
      this.shardStateProofBocHash = shardStateProofBocHash;
      this.shardAccountsProofBocHash = shardAccountsProofBocHash;
      this.configProofBocHash = configProofBocHash;
      this.validatorSetTransitionProofs = validatorSetTransitionProofs;
      this.transitionChainHash = transitionChainHash;
      this.sourceStateVerifierId = sourceStateVerifierId;
      this.sourceStateVerifierHash = sourceStateVerifierHash;
      this.sourceTrustAnchorId = sourceTrustAnchorId;
      this.sourceTrustAnchorHash = sourceTrustAnchorHash;
      this.consensusVerifierId = consensusVerifierId;
      this.consensusVerifierHash = consensusVerifierHash;
      this.messageInclusionVerifierId = messageInclusionVerifierId;
      this.messageInclusionVerifierHash = messageInclusionVerifierHash;
      this.finalityPolicyId = finalityPolicyId;
      this.finalityPolicyHash = finalityPolicyHash;
    }
  }

  private static final class ValidatorSetParts {
    private final List<byte[]> publicKeys;
    private final List<BigInteger> weights;

    private ValidatorSetParts(final List<byte[]> publicKeys, final List<BigInteger> weights) {
      this.publicKeys = publicKeys;
      this.weights = weights;
    }
  }

  private static final class TonValidatorDescr {
    private final int key;
    private final byte[] publicKey;
    private final BigInteger weight;

    private TonValidatorDescr(final int key, final byte[] publicKey, final BigInteger weight) {
      this.key = key;
      this.publicKey = publicKey;
      this.weight = weight;
    }
  }

  private static final class TonCell {
    private final byte[] data;
    private final ArrayList<Integer> refs;

    private TonCell(final byte[] data, final ArrayList<Integer> refs) {
      this.data = data;
      this.refs = refs;
    }
  }

  private static final class BocCell {
    private final byte descriptor;
    private final byte dataDescriptor;
    private final byte[] data;
    private final ArrayList<Integer> refs;
    private final int level;
    private final boolean exotic;

    private BocCell(
        final byte descriptor,
        final byte dataDescriptor,
        final byte[] data,
        final ArrayList<Integer> refs,
        final int level,
        final boolean exotic) {
      this.descriptor = descriptor;
      this.dataDescriptor = dataDescriptor;
      this.data = data;
      this.refs = refs;
      this.level = level;
      this.exotic = exotic;
    }
  }

  private enum BocCellKind {
    ORDINARY,
    PRUNED_BRANCH,
    MERKLE_PROOF,
    MERKLE_UPDATE
  }

  private static final class HashDepth {
    private final byte[] hash;
    private final int depth;

    private HashDepth(final byte[] hash, final int depth) {
      this.hash = hash;
      this.depth = depth;
    }
  }

  private static final class BocPrunedBranch {
    private final int mask;
    private final ArrayList<byte[]> hashes;
    private final ArrayList<Integer> depths;

    private BocPrunedBranch(
        final int mask, final ArrayList<byte[]> hashes, final ArrayList<Integer> depths) {
      this.mask = mask;
      this.hashes = hashes;
      this.depths = depths;
    }
  }

  private static final class BocComputedCell {
    private final int mask;
    private final ArrayList<byte[]> hashes;
    private final ArrayList<Integer> depths;

    private BocComputedCell(
        final int mask, final ArrayList<byte[]> hashes, final ArrayList<Integer> depths) {
      this.mask = mask;
      this.hashes = hashes;
      this.depths = depths;
    }
  }

  private static final class ShardStateAccountsOpening {
    private final String accountsRootHash;
    private final int globalId;
    private final int workchainId;
    private final int seqNo;
    private final int genUtime;
    private final long genLt;
    private final int minRefMcSeqno;
    private final int shardPfxBits;
    private final ArrayList<Boolean> shardPrefixBits;
    private final BigInteger shardId;

    private ShardStateAccountsOpening(
        final String accountsRootHash,
        final int globalId,
        final int workchainId,
        final int seqNo,
        final int genUtime,
        final long genLt,
        final int minRefMcSeqno,
        final int shardPfxBits,
        final ArrayList<Boolean> shardPrefixBits,
        final BigInteger shardId) {
      this.accountsRootHash = accountsRootHash;
      this.globalId = globalId;
      this.workchainId = workchainId;
      this.seqNo = seqNo;
      this.genUtime = genUtime;
      this.genLt = genLt;
      this.minRefMcSeqno = minRefMcSeqno;
      this.shardPfxBits = shardPfxBits;
      this.shardPrefixBits = shardPrefixBits;
      this.shardId = shardId;
    }
  }

  private static final class BocBitReader {
    private final BocCell cell;
    private final int bitLen;
    private int bitOffset;
    private int refOffset;

    private BocBitReader(final BocCell cell) {
      this.cell = cell;
      this.bitLen = cellSerializedBitLen(cell.dataDescriptor & 0xff, cell.data);
      this.bitOffset = 0;
      this.refOffset = 0;
    }

    private boolean readBit() {
      if (bitOffset >= bitLen) {
        throw new IllegalArgumentException("TON HashmapE cell bits are truncated");
      }
      final boolean bit =
          (((cell.data[bitOffset / 8] & 0xff) >>> (7 - (bitOffset % 8))) & 1) != 0;
      bitOffset++;
      return bit;
    }

    private int readUInt(final int bits) {
      int value = 0;
      for (int index = 0; index < bits; index++) {
        value = (value << 1) | (readBit() ? 1 : 0);
      }
      return value;
    }

    private long readUInt64(final int bits) {
      long value = 0L;
      for (int index = 0; index < bits; index++) {
        value = (value << 1) | (readBit() ? 1L : 0L);
      }
      return value;
    }

    private BigInteger readUIntBigInteger(final int bits) {
      BigInteger value = BigInteger.ZERO;
      for (int index = 0; index < bits; index++) {
        value = value.shiftLeft(1);
        if (readBit()) {
          value = value.or(BigInteger.ONE);
        }
      }
      return value;
    }

    private byte[] readBytes(final int byteLength) {
      final byte[] out = new byte[byteLength];
      for (int index = 0; index < byteLength; index++) {
        out[index] = (byte) readUInt(8);
      }
      return out;
    }

    private void skipBits(final int bits) {
      if (bits < 0 || bitOffset + bits > bitLen) {
        throw new IllegalArgumentException("TON BoC cell bits are truncated");
      }
      bitOffset += bits;
    }

    private int readRef() {
      if (refOffset >= cell.refs.size()) {
        throw new IllegalArgumentException("TON HashmapE cell refs are truncated");
      }
      final int ref = cell.refs.get(refOffset);
      refOffset++;
      return ref;
    }

    private int remainingBits() {
      return bitLen - bitOffset;
    }

    private int remainingRefs() {
      return cell.refs.size() - refOffset;
    }

    private boolean isExhausted() {
      return remainingBits() == 0 && remainingRefs() == 0;
    }
  }

  private static final class ParsedBoc {
    private final ArrayList<Integer> roots;
    private final ArrayList<BocCell> cells;

    private ParsedBoc(final ArrayList<Integer> roots, final ArrayList<BocCell> cells) {
      this.roots = roots;
      this.cells = cells;
    }
  }

  private static final class BocRootAndChildIndex {
    private final byte[] rootHash;
    private final int childIndex;

    private BocRootAndChildIndex(final byte[] rootHash, final int childIndex) {
      this.rootHash = rootHash;
      this.childIndex = childIndex;
    }
  }

  private static final class Cursor {
    private int offset;

    private Cursor(final int offset) {
      this.offset = offset;
    }
  }

  public interface WitnessProvider {
    ProofRequestInput resolveWitness(ProofRequestInput input);
  }

  public interface ProofEngine {
    byte[] prove(ProofRequest request);
  }

  /** Local proof engine for nested TON shard-state source-state requests. */
  public interface ShardStateProofEngine {
    byte[] prove(ShardStateProofRequest request);
  }

  /** Local proof engine for TON full-light source-state audit role requests. */
  public interface FullLightClientAuditProofEngine {
    byte[] prove(FullLightClientAuditProofRequest request);
  }

  /** Role-separated TON full-light audit proof capsules. */
  public record FullLightClientAuditProofs(
      SourceStateVerificationProof masterchainConfig,
      SourceStateVerificationProof validatorSetTransition,
      SourceStateVerificationProof shardAccountsDictionary) {}

  /** Source-state proof wrapper for Android TON proof engines. */
  public static final class SourceStateProver {
    private final ShardStateProofEngine shardStateProofEngine;
    private final FullLightClientAuditProofEngine fullLightClientAuditProofEngine;

    public SourceStateProver() {
      this(null, null);
    }

    public SourceStateProver(
        final ShardStateProofEngine shardStateProofEngine,
        final FullLightClientAuditProofEngine fullLightClientAuditProofEngine) {
      this.shardStateProofEngine = shardStateProofEngine;
      this.fullLightClientAuditProofEngine = fullLightClientAuditProofEngine;
    }

    public SourceStateVerificationProof proveShardState(final ShardStateProofRequestInput input) {
      return proveShardState(buildShardStateProofRequest(input));
    }

    public SourceStateVerificationProof proveShardState(final ShardStateProofRequest request) {
      if (shardStateProofEngine == null) {
        throw new IllegalStateException("TON SCCP source-state prover is not linked");
      }
      requireSourceStateProofRequestForProverCallback(request);
      return wrapSourceStateVerificationProof(
          shardStateProofEngine.prove(callbackRequestSnapshot(request)), request);
    }

    public FullLightClientAuditProofs proveFullLightClientAudit(
        final FullLightClientAuditProofInput input) {
      final FullLightClientAuditProofRequests requests = buildFullLightClientAuditProofRequests(input);
      return new FullLightClientAuditProofs(
          proveFullLightClientAudit(requests.masterchainConfig()),
          proveFullLightClientAudit(requests.validatorSetTransition()),
          proveFullLightClientAudit(requests.shardAccountsDictionary()));
    }

    public SourceStateVerificationProof proveFullLightClientAudit(
        final FullLightClientAuditProofRequest request) {
      if (fullLightClientAuditProofEngine == null) {
        throw new IllegalStateException("TON SCCP source-state prover is not linked");
      }
      requireSourceStateProofRequestForProverCallback(request);
      return wrapSourceStateVerificationProof(
          fullLightClientAuditProofEngine.prove(callbackRequestSnapshot(request)), request);
    }
  }

  private static ShardStateProofRequest callbackRequestSnapshot(
      final ShardStateProofRequest request) {
    Objects.requireNonNull(request, "request");
    return new ShardStateProofRequest(
        request.version(),
        request.proofFamily(),
        request.circuitId(),
        request.parameterSet(),
        request.sourceDomain(),
        request.masterchainSeqno(),
        request.shardSeqno(),
        request.sourceStateVerifierId(),
        request.sourceStateVerifierHash(),
        request.shardStateProofPublicInputsHash(),
        request.statementBytes(),
        request.witnessCommitmentBytes(),
        request.verificationContextBytes(),
        request.schemaDescriptor(),
        request.publicInputColumns(),
        request.fastpqPublicInputs(),
        request.fastpqTransitions());
  }

  private static FullLightClientAuditProofRequest callbackRequestSnapshot(
      final FullLightClientAuditProofRequest request) {
    Objects.requireNonNull(request, "request");
    return new FullLightClientAuditProofRequest(
        request.version(),
        request.proofFamily(),
        request.circuitId(),
        request.parameterSet(),
        request.role(),
        request.roleCode(),
        request.sourceDomain(),
        request.masterchainSeqno(),
        request.shardSeqno(),
        request.verifierId(),
        request.verifierHash(),
        request.sourceStateVerifierId(),
        request.sourceStateVerifierHash(),
        request.sourceVerifierMaterialHash(),
        request.sourceAdapterDeploymentHash(),
        request.fullLightClientGateHash(),
        request.shardStateProofPublicInputsHash(),
        request.shardStateVerificationProofHash(),
        request.auditStatementHash(),
        request.statementBytes(),
        request.verificationContextBytes(),
        request.schemaDescriptor(),
        request.publicInputColumns(),
        request.fastpqPublicInputs(),
        request.fastpqTransitions());
  }

  static ProofRequest callbackRequestSnapshot(final ProofRequest request) {
    Objects.requireNonNull(request, "request");
    return new ProofRequest(
        request.version(),
        request.backend(),
        request.sourceDomain(),
        request.targetDomain(),
        request.publicInputs(),
        request.publicInputsBytes(),
        request.bundleBytes(),
        request.sourceProofBytes(),
        request.proofContext(),
        request.statementHash(),
        request.destinationBindingHash(),
        request.sourceStateVerifierId(),
        request.sourceStateVerifierHash(),
        request.sourceAdapterDeploymentBindingHash(),
        request.sourceAdapterDeploymentBinding(),
        request.requestHash());
  }

  public record ValidatorSignatureProofInput(
      int version,
      String totalWeight,
      String signedWeight,
      String blockMessageHash,
      List<byte[]> validatorPublicKeys,
      List<String> validatorWeights,
      byte[] signersBitmap,
      List<byte[]> signatures) {
    public ValidatorSignatureProofInput(
        final String totalWeight,
        final String signedWeight,
        final String blockMessageHash,
        final List<byte[]> validatorPublicKeys,
        final List<String> validatorWeights,
        final byte[] signersBitmap,
        final List<byte[]> signatures) {
      this(
          1,
          totalWeight,
          signedWeight,
          blockMessageHash,
          validatorPublicKeys,
          validatorWeights,
          signersBitmap,
          signatures);
    }
  }

  public record ValidatorSetTransitionProofInput(
      int version,
      int sourceDomain,
      String fromValidatorSetSeqno,
      String toValidatorSetSeqno,
      String masterchainSeqno,
      int masterchainWorkchainId,
      String masterchainShard,
      String masterchainBlockHash,
      String masterchainFileHash,
      String parentValidatorSetHash,
      String nextValidatorSetHash,
      byte[] nextValidatorSetPayload,
      String nextValidatorSetPayloadHash,
      String nextValidatorSetConfigHash,
      String transitionMessageHash,
      String transitionSignatureHash,
      ValidatorSignatureProofInput validatorSignatureProof) {
    public ValidatorSetTransitionProofInput {
      nextValidatorSetPayload =
          Arrays.copyOf(
              Objects.requireNonNull(nextValidatorSetPayload, "nextValidatorSetPayload"),
              nextValidatorSetPayload.length);
    }

    @Override
    public byte[] nextValidatorSetPayload() {
      return Arrays.copyOf(nextValidatorSetPayload, nextValidatorSetPayload.length);
    }
  }

  public record ShardStateProofRequestInput(
      int sourceDomain,
      String masterchainSeqno,
      int masterchainWorkchainId,
      String masterchainShard,
      String masterchainBlockHash,
      String masterchainFileHash,
      String validatorSetHash,
      String masterchainConfigRoot,
      String masterchainConfigProofHash,
      int shardWorkchainId,
      String shardShard,
      String shardSeqno,
      String shardBlockHash,
      String shardFileHash,
      String shardStateRoot,
      String transactionRoot,
      String transactionLt,
      String shardStateDictionaryRoot,
      int shardStateDictionaryKeyBitLen,
      byte[] shardStateDictionaryKey,
      String masterchainSignatureHash,
      String shardProofHash,
      byte[] shardStateProofBoc,
      byte[] shardStateDictionaryProofBoc,
      byte[] configDictionaryProofBoc,
      List<ValidatorSetTransitionProofInput> validatorSetTransitionProofs,
      String sourceStateVerifierId,
      String sourceStateVerifierHash,
      String sourceTrustAnchorId,
      String sourceTrustAnchorHash,
      String consensusVerifierId,
      String consensusVerifierHash,
      String messageInclusionVerifierId,
      String messageInclusionVerifierHash,
      String finalityPolicyId,
      String finalityPolicyHash) {
    public ShardStateProofRequestInput {
      shardStateDictionaryKey =
          Arrays.copyOf(
              Objects.requireNonNull(shardStateDictionaryKey, "shardStateDictionaryKey"),
              shardStateDictionaryKey.length);
      shardStateProofBoc =
          Arrays.copyOf(Objects.requireNonNull(shardStateProofBoc, "shardStateProofBoc"), shardStateProofBoc.length);
      shardStateDictionaryProofBoc =
          Arrays.copyOf(
              Objects.requireNonNull(shardStateDictionaryProofBoc, "shardStateDictionaryProofBoc"),
              shardStateDictionaryProofBoc.length);
      configDictionaryProofBoc =
          Arrays.copyOf(
              Objects.requireNonNull(configDictionaryProofBoc, "configDictionaryProofBoc"),
              configDictionaryProofBoc.length);
      validatorSetTransitionProofs =
          Collections.unmodifiableList(new ArrayList<ValidatorSetTransitionProofInput>(
              Objects.requireNonNull(validatorSetTransitionProofs, "validatorSetTransitionProofs")));
    }

    @Override
    public byte[] shardStateDictionaryKey() {
      return Arrays.copyOf(shardStateDictionaryKey, shardStateDictionaryKey.length);
    }

    @Override
    public byte[] shardStateProofBoc() {
      return Arrays.copyOf(shardStateProofBoc, shardStateProofBoc.length);
    }

    @Override
    public byte[] shardStateDictionaryProofBoc() {
      return Arrays.copyOf(shardStateDictionaryProofBoc, shardStateDictionaryProofBoc.length);
    }

    @Override
    public byte[] configDictionaryProofBoc() {
      return Arrays.copyOf(configDictionaryProofBoc, configDictionaryProofBoc.length);
    }
  }

  public record ShardStateFastpqPublicInputs(
      String dsid, String slot, String oldRoot, String newRoot, String permRoot, String txSetHash) {}

  public record ShardStateFastpqTransition(
      String key, String operation, String oldValue, String newValue) {}

  public record ShardStateProofRequest(
      int version,
      String proofFamily,
      String circuitId,
      String parameterSet,
      int sourceDomain,
      String masterchainSeqno,
      String shardSeqno,
      String sourceStateVerifierId,
      String sourceStateVerifierHash,
      String shardStateProofPublicInputsHash,
      byte[] statementBytes,
      byte[] witnessCommitmentBytes,
      byte[] verificationContextBytes,
      byte[] schemaDescriptor,
      List<List<String>> publicInputColumns,
      ShardStateFastpqPublicInputs fastpqPublicInputs,
      List<ShardStateFastpqTransition> fastpqTransitions) {
    public ShardStateProofRequest {
      statementBytes =
          Arrays.copyOf(Objects.requireNonNull(statementBytes, "statementBytes"), statementBytes.length);
      witnessCommitmentBytes =
          Arrays.copyOf(
              Objects.requireNonNull(witnessCommitmentBytes, "witnessCommitmentBytes"),
              witnessCommitmentBytes.length);
      verificationContextBytes =
          Arrays.copyOf(
              Objects.requireNonNull(verificationContextBytes, "verificationContextBytes"),
              verificationContextBytes.length);
      schemaDescriptor =
          Arrays.copyOf(
              Objects.requireNonNull(schemaDescriptor, "schemaDescriptor"),
              schemaDescriptor.length);
      publicInputColumns = copyColumns(Objects.requireNonNull(publicInputColumns, "publicInputColumns"));
      fastpqTransitions =
          Collections.unmodifiableList(new ArrayList<ShardStateFastpqTransition>(
              Objects.requireNonNull(fastpqTransitions, "fastpqTransitions")));
    }

    @Override
    public byte[] statementBytes() {
      return Arrays.copyOf(statementBytes, statementBytes.length);
    }

    @Override
    public byte[] witnessCommitmentBytes() {
      return Arrays.copyOf(witnessCommitmentBytes, witnessCommitmentBytes.length);
    }

    @Override
    public byte[] verificationContextBytes() {
      return Arrays.copyOf(verificationContextBytes, verificationContextBytes.length);
    }

    @Override
    public byte[] schemaDescriptor() {
      return Arrays.copyOf(schemaDescriptor, schemaDescriptor.length);
    }
  }

  public record SourceStateVerificationProof(
      int version,
      String proofFamily,
      String circuitId,
      byte[] proofBytes) {
    public SourceStateVerificationProof {
      proofFamily = Objects.requireNonNull(proofFamily, "proofFamily");
      circuitId = Objects.requireNonNull(circuitId, "circuitId");
      proofBytes =
          Arrays.copyOf(Objects.requireNonNull(proofBytes, "proofBytes"), proofBytes.length);
    }

    public SourceStateVerificationProof(final byte[] proofBytes) {
      this(1, STARK_FRI_PROOF_FAMILY_V1, SHARD_STATE_OPEN_VERIFY_CIRCUIT_ID_V1, proofBytes);
    }

    public String proofBase64() {
      return Base64.getEncoder().encodeToString(proofBytes);
    }

    @Override
    public byte[] proofBytes() {
      return Arrays.copyOf(proofBytes, proofBytes.length);
    }
  }

  public enum FullLightClientAuditRole {
    MASTERCHAIN_CONFIG,
    VALIDATOR_SET_TRANSITION,
    SHARD_ACCOUNTS_DICTIONARY
  }

  public record FullLightClientAuditProofInput(
      ShardStateProofRequestInput shardState,
      SourceStateVerificationProof shardStateVerificationProof,
      String validatorSetPayloadHash,
      String configLeafHash,
      String configValueHash,
      String sourceVerifierMaterialHash,
      String sourceAdapterDeploymentHash,
      String fullLightClientGateHash,
      String tonMasterchainConfigVerifierHash,
      String tonValidatorSetTransitionVerifierHash,
      String tonShardAccountsDictionaryVerifierHash,
      String shardStateProofPublicInputsHash,
      String shardStateVerificationProofHash) {
    public FullLightClientAuditProofInput {
      Objects.requireNonNull(shardState, "shardState");
      Objects.requireNonNull(shardStateVerificationProof, "shardStateVerificationProof");
    }

    public FullLightClientAuditProofInput(
        final ShardStateProofRequestInput shardState,
        final SourceStateVerificationProof shardStateVerificationProof,
        final String validatorSetPayloadHash,
        final String configLeafHash,
        final String configValueHash,
        final String sourceVerifierMaterialHash,
        final String sourceAdapterDeploymentHash,
        final String fullLightClientGateHash,
        final String tonMasterchainConfigVerifierHash,
        final String tonValidatorSetTransitionVerifierHash,
        final String tonShardAccountsDictionaryVerifierHash) {
      this(
          shardState,
          shardStateVerificationProof,
          validatorSetPayloadHash,
          configLeafHash,
          configValueHash,
          sourceVerifierMaterialHash,
          sourceAdapterDeploymentHash,
          fullLightClientGateHash,
          tonMasterchainConfigVerifierHash,
          tonValidatorSetTransitionVerifierHash,
          tonShardAccountsDictionaryVerifierHash,
          null,
          null);
    }
  }

  public record FullLightClientAuditFastpqPublicInputs(
      String dsid,
      String slot,
      String oldRoot,
      String newRoot,
      String permRoot,
      String txSetHash) {}

  public record FullLightClientAuditFastpqTransition(
      String key,
      String operation,
      String oldValue,
      String newValue) {}

  public record FullLightClientAuditProofRequest(
      int version,
      String proofFamily,
      String circuitId,
      String parameterSet,
      String role,
      int roleCode,
      int sourceDomain,
      String masterchainSeqno,
      String shardSeqno,
      String verifierId,
      String verifierHash,
      String sourceStateVerifierId,
      String sourceStateVerifierHash,
      String sourceVerifierMaterialHash,
      String sourceAdapterDeploymentHash,
      String fullLightClientGateHash,
      String shardStateProofPublicInputsHash,
      String shardStateVerificationProofHash,
      String auditStatementHash,
      byte[] statementBytes,
      byte[] verificationContextBytes,
      byte[] schemaDescriptor,
      List<List<String>> publicInputColumns,
      FullLightClientAuditFastpqPublicInputs fastpqPublicInputs,
      List<FullLightClientAuditFastpqTransition> fastpqTransitions) {
    public FullLightClientAuditProofRequest {
      statementBytes =
          Arrays.copyOf(Objects.requireNonNull(statementBytes, "statementBytes"), statementBytes.length);
      verificationContextBytes =
          Arrays.copyOf(
              Objects.requireNonNull(verificationContextBytes, "verificationContextBytes"),
              verificationContextBytes.length);
      schemaDescriptor =
          Arrays.copyOf(
              Objects.requireNonNull(schemaDescriptor, "schemaDescriptor"),
              schemaDescriptor.length);
      publicInputColumns = copyColumns(Objects.requireNonNull(publicInputColumns, "publicInputColumns"));
      fastpqTransitions =
          Collections.unmodifiableList(new ArrayList<FullLightClientAuditFastpqTransition>(
              Objects.requireNonNull(fastpqTransitions, "fastpqTransitions")));
    }

    @Override
    public byte[] statementBytes() {
      return Arrays.copyOf(statementBytes, statementBytes.length);
    }

    @Override
    public byte[] verificationContextBytes() {
      return Arrays.copyOf(verificationContextBytes, verificationContextBytes.length);
    }

    @Override
    public byte[] schemaDescriptor() {
      return Arrays.copyOf(schemaDescriptor, schemaDescriptor.length);
    }
  }

  public record FullLightClientAuditProofRequests(
      FullLightClientAuditProofRequest masterchainConfig,
      FullLightClientAuditProofRequest validatorSetTransition,
      FullLightClientAuditProofRequest shardAccountsDictionary) {}

  public record PublicInputsInput(
      int version,
      String messageId,
      String payloadHash,
      int targetDomain,
      String commitmentRoot,
      String finalityHeight,
      String finalityBlockHash) {
    public PublicInputsInput(
        final String messageId,
        final String payloadHash,
        final String commitmentRoot,
        final String finalityHeight,
        final String finalityBlockHash) {
      this(1, messageId, payloadHash, DOMAIN_TON, commitmentRoot, finalityHeight, finalityBlockHash);
    }
  }

  public record SubmissionDestinationBindingInput(String key, String bindingHash) {}

  public record SubmissionManifestInput(
      int version,
      int localDomain,
      int counterpartyDomain,
      String securityModel,
      String anchorGovernance,
      String verifierTarget,
      String verifierBackendFamily,
      String proofFamily,
      String verifierBackendKey,
      String messageBackend,
      String registryBackend,
      String manifestSeed,
      SubmissionDestinationBindingInput destinationBinding) {}

  public record SubmissionMetadataInput(
      SubmissionManifestInput manifest,
      SubmissionDestinationBindingInput destinationBinding,
      String destinationBindingHash,
      PublicInputsInput publicInputs,
      String statementHash) {}

  public record MessageBodyInput(
      ProofResult proofResult,
      PublicInputsInput publicInputs,
      byte[] proofBytes,
      byte[] bundleBytes,
      String statementHash,
      String destinationBindingHash,
      byte[] metadataBytes,
      String queryId) {
    public MessageBodyInput {
      proofResult = requireWrappedProofResultForSubmission(proofResult);
      if (!Objects.equals(publicInputs, proofResult.publicInputs())) {
        throw new IllegalArgumentException("publicInputs must match proofResult.publicInputs");
      }
      proofBytes = Arrays.copyOf(Objects.requireNonNull(proofBytes, "proofBytes"), proofBytes.length);
      if (!Arrays.equals(proofBytes, proofResult.proofBytes())) {
        throw new IllegalArgumentException("proofBytes must match proofResult.proofBytes");
      }
      bundleBytes = Arrays.copyOf(Objects.requireNonNull(bundleBytes, "bundleBytes"), bundleBytes.length);
      if (!Arrays.equals(bundleBytes, proofResult.bundleBytes())) {
        throw new IllegalArgumentException("bundleBytes must match proofResult.bundleBytes");
      }
      if (!Objects.equals(statementHash, proofResult.proofContext().statementHash())) {
        throw new IllegalArgumentException(
            "statementHash must match proofResult.proofContext.statementHash");
      }
      if (!Objects.equals(destinationBindingHash, proofResult.proofContext().destinationBindingHash())) {
        throw new IllegalArgumentException(
            "destinationBindingHash must match proofResult.proofContext.destinationBindingHash");
      }
      metadataBytes =
          Arrays.copyOf(Objects.requireNonNull(metadataBytes, "metadataBytes"), metadataBytes.length);
    }

    public MessageBodyInput(final ProofResult proofResult, final byte[] bundleBytes) {
      this(proofResult, bundleBytes, new byte[0], null);
    }

    public MessageBodyInput(
        final ProofResult proofResult, final byte[] bundleBytes, final byte[] metadataBytes) {
      this(proofResult, bundleBytes, metadataBytes, null);
    }

    public MessageBodyInput(
        final ProofResult proofResult,
        final byte[] bundleBytes,
        final byte[] metadataBytes,
        final String queryId) {
      this(requireWrappedProofResultForSubmission(proofResult), bundleBytes, metadataBytes, queryId, true);
    }

    private MessageBodyInput(
        final ProofResult proofResult,
        final byte[] bundleBytes,
        final byte[] metadataBytes,
        final String queryId,
        final boolean checkedProofResult) {
      this(
          proofResult,
          proofResult.publicInputs(),
          proofResult.proofBytes(),
          requireBundleMatchesProofResult(bundleBytes, proofResult),
          proofResult.proofContext().statementHash(),
          proofResult.proofContext().destinationBindingHash(),
          metadataBytes,
          queryId);
    }

    private static byte[] requireBundleMatchesProofResult(
        final byte[] bundleBytes, final ProofResult proofResult) {
      if (!Arrays.equals(bundleBytes, proofResult.bundleBytes())) {
        throw new IllegalArgumentException("bundleBytes must match proofResult.bundleBytes");
      }
      return bundleBytes;
    }

    @Override
    public byte[] proofBytes() {
      return Arrays.copyOf(proofBytes, proofBytes.length);
    }

    @Override
    public byte[] bundleBytes() {
      return Arrays.copyOf(bundleBytes, bundleBytes.length);
    }

    @Override
    public byte[] metadataBytes() {
      return Arrays.copyOf(metadataBytes, metadataBytes.length);
    }
  }

  public record SubmissionArgument(String key, String encoding, String bytesHex) {}

  public record Submission(
      int version,
      String envelopeEncoding,
      String submissionKind,
      String verifierEntrypoint,
      byte[] messageBodyBoc,
      String messageBodyBocHex,
      List<SubmissionArgument> arguments,
      byte[] envelopeBytes,
      String envelopeHex) {
    public Submission {
      messageBodyBoc =
          Arrays.copyOf(Objects.requireNonNull(messageBodyBoc, "messageBodyBoc"), messageBodyBoc.length);
      arguments = Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(arguments, "arguments")));
      envelopeBytes = Arrays.copyOf(Objects.requireNonNull(envelopeBytes, "envelopeBytes"), envelopeBytes.length);
    }

    public Submission(
        final String envelopeEncoding, final byte[] messageBodyBoc, final String messageBodyBocHex) {
      this(
          1,
          envelopeEncoding,
          "internal_message",
          "op::submit_sccp_message_proof",
          messageBodyBoc,
          messageBodyBocHex,
          Collections.singletonList(
              new SubmissionArgument("message_body_boc", "ton_boc", messageBodyBocHex)),
          messageBodyBoc,
          messageBodyBocHex);
    }

    @Override
    public byte[] messageBodyBoc() {
      return Arrays.copyOf(messageBodyBoc, messageBodyBoc.length);
    }

    @Override
    public byte[] envelopeBytes() {
      return Arrays.copyOf(envelopeBytes, envelopeBytes.length);
    }
  }

  private static SolanaSccpProver.SourceAdapterDeploymentBinding
      checkedTonProofRequestDeploymentBinding(
          final SolanaSccpProver.SourceAdapterDeploymentBinding sourceAdapterDeploymentBinding,
          final int sourceDomain) {
    final SolanaSccpProver.SourceAdapterDeploymentBinding deploymentBinding =
        SolanaSccpProver.normalizeSourceAdapterDeploymentBinding(
            Objects.requireNonNull(
                    sourceAdapterDeploymentBinding, "sourceAdapterDeploymentBinding")
                .sourceDomain(),
            sourceAdapterDeploymentBinding.targetDomain(),
            sourceAdapterDeploymentBinding.sourceAdapterDeploymentHash(),
            sourceAdapterDeploymentBinding.sourceAdapterDeploymentReceiptHash());
    if (deploymentBinding.sourceDomain() != sourceDomain) {
      throw new IllegalArgumentException(
          "sourceAdapterDeploymentBinding.sourceDomain must match sourceDomain");
    }
    if (deploymentBinding.sourceDomain() != DOMAIN_TON) {
      throw new IllegalArgumentException(
          "sourceAdapterDeploymentBinding.sourceDomain must be TON");
    }
    if (deploymentBinding.targetDomain() != SolanaSccpProver.DOMAIN_SORA) {
      throw new IllegalArgumentException(
          "sourceAdapterDeploymentBinding.targetDomain must be SORA");
    }
    if (SolanaSccpProver.ZERO_HASH_V1.equals(
        deploymentBinding.sourceAdapterDeploymentHash())) {
      throw new IllegalArgumentException("sourceAdapterDeploymentBinding must be non-zero");
    }
    return deploymentBinding;
  }

  /** TON live-account evidence collected by UI code before route canary submission. */
  public record RouteCanaryEvidenceInput(
      String routeAllowlistHash,
      String destinationBindingHash,
      String expectedDestinationBindingHash,
      String sourceVerifierMaterialHash,
      String sourceAdapterEngineDeploymentHash,
      String verifierContractAddress,
      String verifierCodeHash,
      String accountStatus,
      String accountStateHash,
      String lastTransactionLt,
      String lastTransactionHash,
      String verifierCodeBocRootHash) {
    public RouteCanaryEvidenceInput {
      Objects.requireNonNull(routeAllowlistHash, "routeAllowlistHash");
      Objects.requireNonNull(destinationBindingHash, "destinationBindingHash");
      Objects.requireNonNull(sourceVerifierMaterialHash, "sourceVerifierMaterialHash");
      Objects.requireNonNull(sourceAdapterEngineDeploymentHash, "sourceAdapterEngineDeploymentHash");
      Objects.requireNonNull(verifierContractAddress, "verifierContractAddress");
      Objects.requireNonNull(verifierCodeHash, "verifierCodeHash");
      Objects.requireNonNull(accountStatus, "accountStatus");
      Objects.requireNonNull(accountStateHash, "accountStateHash");
      Objects.requireNonNull(lastTransactionLt, "lastTransactionLt");
      Objects.requireNonNull(lastTransactionHash, "lastTransactionHash");
      Objects.requireNonNull(verifierCodeBocRootHash, "verifierCodeBocRootHash");
    }
  }

  public record ProofRequestInput(
      PublicInputsInput publicInputs,
      byte[] bundleBytes,
      byte[] sourceProofBytes,
      String statementHash,
      String destinationBindingHash,
      String sourceStateVerifierId,
      String sourceStateVerifierHash,
      String sourceAdapterDeploymentHash,
      String sourceAdapterDeploymentReceiptHash,
      String backend,
      int sourceDomain) {
    public ProofRequestInput(
        final PublicInputsInput publicInputs,
        final byte[] bundleBytes,
        final byte[] sourceProofBytes,
        final String statementHash,
        final String destinationBindingHash,
        final String sourceStateVerifierId,
        final String sourceStateVerifierHash,
        final SolanaSccpProver.SourceAdapterDeploymentBinding sourceAdapterDeploymentBinding,
        final String backend,
        final int sourceDomain) {
      this(
          publicInputs,
          bundleBytes,
          sourceProofBytes,
          statementHash,
          destinationBindingHash,
          sourceStateVerifierId,
          sourceStateVerifierHash,
          checkedTonProofRequestDeploymentBinding(
                  sourceAdapterDeploymentBinding, sourceDomain)
              .sourceAdapterDeploymentHash(),
          checkedTonProofRequestDeploymentBinding(
                  sourceAdapterDeploymentBinding, sourceDomain)
              .sourceAdapterDeploymentReceiptHash(),
          backend,
          sourceDomain);
    }

    public ProofRequestInput(
        final PublicInputsInput publicInputs,
        final byte[] bundleBytes,
        final String statementHash,
        final String destinationBindingHash,
        final String sourceStateVerifierId,
        final String sourceStateVerifierHash,
        final SolanaSccpProver.SourceAdapterDeploymentBinding sourceAdapterDeploymentBinding) {
      this(
          publicInputs,
          bundleBytes,
          new byte[0],
          statementHash,
          destinationBindingHash,
          sourceStateVerifierId,
          sourceStateVerifierHash,
          sourceAdapterDeploymentBinding,
          CONTRACT_PROOF_BACKEND_V1,
          DOMAIN_TON);
    }

    public ProofRequestInput(
        final PublicInputsInput publicInputs,
        final byte[] bundleBytes,
        final String statementHash,
        final String destinationBindingHash) {
      this(
          publicInputs,
          bundleBytes,
          new byte[0],
          statementHash,
          destinationBindingHash,
          MAINNET_SHARD_STATE_VERIFIER_ID_V1,
          SolanaSccpProver.ZERO_HASH_V1,
          SolanaSccpProver.ZERO_HASH_V1,
          SolanaSccpProver.ZERO_HASH_V1,
          CONTRACT_PROOF_BACKEND_V1,
          DOMAIN_TON);
    }
  }

  public record ProofContext(int version, String statementHash, String destinationBindingHash) {}

  public record ProofRequest(
      int version,
      String backend,
      int sourceDomain,
      int targetDomain,
      PublicInputsInput publicInputs,
      byte[] publicInputsBytes,
      byte[] bundleBytes,
      byte[] sourceProofBytes,
      ProofContext proofContext,
      String statementHash,
      String destinationBindingHash,
      String sourceStateVerifierId,
      String sourceStateVerifierHash,
      String sourceAdapterDeploymentBindingHash,
      SolanaSccpProver.SourceAdapterDeploymentBinding sourceAdapterDeploymentBinding,
      String requestHash) {
    public ProofRequest {
      publicInputsBytes =
          Arrays.copyOf(
              Objects.requireNonNull(publicInputsBytes, "publicInputsBytes"),
              publicInputsBytes.length);
      bundleBytes =
          Arrays.copyOf(Objects.requireNonNull(bundleBytes, "bundleBytes"), bundleBytes.length);
      sourceProofBytes =
          Arrays.copyOf(
              Objects.requireNonNull(sourceProofBytes, "sourceProofBytes"),
              sourceProofBytes.length);
    }

    @Override
    public byte[] publicInputsBytes() {
      return Arrays.copyOf(publicInputsBytes, publicInputsBytes.length);
    }

    @Override
    public byte[] bundleBytes() {
      return Arrays.copyOf(bundleBytes, bundleBytes.length);
    }

    @Override
    public byte[] sourceProofBytes() {
      return Arrays.copyOf(sourceProofBytes, sourceProofBytes.length);
    }
  }

  public record ProofResult(
      int version,
      String backend,
      byte[] proofBytes,
      String proofBase64,
      PublicInputsInput publicInputs,
      byte[] bundleBytes,
      byte[] sourceProofBytes,
      ProofContext proofContext,
      String statementHash,
      String destinationBindingHash,
      String sourceStateVerifierId,
      String sourceStateVerifierHash,
      String sourceAdapterDeploymentBindingHash,
      SolanaSccpProver.SourceAdapterDeploymentBinding sourceAdapterDeploymentBinding,
      String requestHash,
      String envelopeHash) {
    public ProofResult {
      proofBytes = Arrays.copyOf(Objects.requireNonNull(proofBytes, "proofBytes"), proofBytes.length);
      bundleBytes = Arrays.copyOf(Objects.requireNonNull(bundleBytes, "bundleBytes"), bundleBytes.length);
      sourceProofBytes =
          Arrays.copyOf(
              Objects.requireNonNull(sourceProofBytes, "sourceProofBytes"),
              sourceProofBytes.length);
    }

    @Override
    public byte[] proofBytes() {
      return Arrays.copyOf(proofBytes, proofBytes.length);
    }

    @Override
    public byte[] bundleBytes() {
      return Arrays.copyOf(bundleBytes, bundleBytes.length);
    }

    @Override
    public byte[] sourceProofBytes() {
      return Arrays.copyOf(sourceProofBytes, sourceProofBytes.length);
    }
  }
}
