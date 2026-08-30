// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.nio.ByteBuffer;
import java.nio.CharBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.SecureRandom;
import java.util.Arrays;
import java.util.Objects;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.sdk.privacy.PrivacyActionOperationViewV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyExact12ActionContractV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyLedgerEffectKindV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyOperationSchemaV1;
import org.hyperledger.iroha.sdk.privacy.PrivacyProtocolIdV1;

/** ABI-22 native codec and verifier for authenticated finalized Exact12 action receipts. */
public final class AuthenticatedPrivacyActionReceiptNativeBridge {
  public static final int REQUIRED_BRIDGE_ABI_VERSION = 22;
  public static final long RESPONSE_MAX_BYTES = 256L * 1024L;
  private static final int ACTION_INDEX_V1 = 0;
  private static final int DIGEST_BYTES = 32;
  private static final int NONCE_BYTES = 32;
  private static final int REQUEST_BINDING_BYTES = 96;
  private static final int PREPARATION_MAX_BYTES = 64 * 1024;
  private static final int SIGNATURE_MAX_BYTES = 16 * 1024;
  private static final int SIGNED_QUERY_MAX_BYTES = 64 * 1024;
  private static final SecureRandom NONCE_RANDOM = new SecureRandom();

  private AuthenticatedPrivacyActionReceiptNativeBridge() {}

  /** Native-bound signed ID105 body plus the private preparation used to verify its response. */
  public static final class SignedQueryV1 {
    private final byte[] preparation;
    private final byte[] requestBody;
    private final String networkIdHex;
    private final PrivacyProtocolIdV1 protocolId;
    private final PrivacyOperationSchemaV1 operationSchema;
    private final PrivacyLedgerEffectKindV1 ledgerEffectKind;
    private final String transactionHashHex;
    private final int actionIndex;
    private final byte[] transactionIntentDigest;
    private final byte[] statementDigest;
    private final byte[] proofEnvelopeHash;

    private SignedQueryV1(
        final byte[] preparation,
        final byte[] requestBody,
        final String networkIdHex,
        final PrivacyProtocolIdV1 protocolId,
        final PrivacyOperationSchemaV1 operationSchema,
        final PrivacyLedgerEffectKindV1 ledgerEffectKind,
        final String transactionHashHex,
        final int actionIndex,
        final byte[] transactionIntentDigest,
        final byte[] statementDigest,
        final byte[] proofEnvelopeHash) {
      this.preparation = preparation.clone();
      this.requestBody = requestBody.clone();
      this.networkIdHex = networkIdHex;
      this.protocolId = protocolId;
      this.operationSchema = operationSchema;
      this.ledgerEffectKind = ledgerEffectKind;
      this.transactionHashHex = transactionHashHex;
      this.actionIndex = actionIndex;
      this.transactionIntentDigest = transactionIntentDigest.clone();
      this.statementDigest = statementDigest.clone();
      this.proofEnvelopeHash = proofEnvelopeHash.clone();
    }

    /** Canonical versioned {@code SignedQuery} bytes for {@code POST /v1/query}. */
    public byte[] requestBody() {
      return requestBody.clone();
    }

    byte[] preparation() {
      return preparation.clone();
    }
  }

  /** Native-authenticated, finalized ID105 receipt for one Exact12 action. */
  public static final class AuthenticatedActionExecutionReceiptV1 {
    private final String networkIdHex;
    private final PrivacyProtocolIdV1 protocolId;
    private final PrivacyOperationSchemaV1 operationSchema;
    private final PrivacyLedgerEffectKindV1 ledgerEffectKind;
    private final String transactionHashHex;
    private final int actionIndex;
    private final byte[] transactionIntentDigest;
    private final byte[] statementDigest;
    private final byte[] proofEnvelopeHash;
    private final byte[] capabilityManifestDigest;
    private final BigInteger capabilityCommittedHeight;
    private final BigInteger admittedAtHeight;
    private final BigInteger finalizedHeight;
    private final byte[] finalizedBlockHash;

    private AuthenticatedActionExecutionReceiptV1(
        final String networkIdHex,
        final PrivacyProtocolIdV1 protocolId,
        final PrivacyOperationSchemaV1 operationSchema,
        final PrivacyLedgerEffectKindV1 ledgerEffectKind,
        final String transactionHashHex,
        final int actionIndex,
        final byte[] transactionIntentDigest,
        final byte[] statementDigest,
        final byte[] proofEnvelopeHash,
        final byte[] capabilityManifestDigest,
        final BigInteger capabilityCommittedHeight,
        final BigInteger admittedAtHeight,
        final BigInteger finalizedHeight,
        final byte[] finalizedBlockHash) {
      this.networkIdHex = networkIdHex;
      this.protocolId = protocolId;
      this.operationSchema = operationSchema;
      this.ledgerEffectKind = ledgerEffectKind;
      this.transactionHashHex = transactionHashHex;
      this.actionIndex = actionIndex;
      this.transactionIntentDigest = transactionIntentDigest.clone();
      this.statementDigest = statementDigest.clone();
      this.proofEnvelopeHash = proofEnvelopeHash.clone();
      this.capabilityManifestDigest = capabilityManifestDigest.clone();
      this.capabilityCommittedHeight = capabilityCommittedHeight;
      this.admittedAtHeight = admittedAtHeight;
      this.finalizedHeight = finalizedHeight;
      this.finalizedBlockHash = finalizedBlockHash.clone();
      validate();
    }

    public String networkIdHex() {
      return networkIdHex;
    }

    public PrivacyProtocolIdV1 protocolId() {
      return protocolId;
    }

    public PrivacyOperationSchemaV1 operationSchema() {
      return operationSchema;
    }

    public PrivacyLedgerEffectKindV1 ledgerEffectKind() {
      return ledgerEffectKind;
    }

    public String transactionHashHex() {
      return transactionHashHex;
    }

    public int actionIndex() {
      return actionIndex;
    }

    public byte[] transactionIntentDigest() {
      return transactionIntentDigest.clone();
    }

    public byte[] statementDigest() {
      return statementDigest.clone();
    }

    public byte[] proofEnvelopeHash() {
      return proofEnvelopeHash.clone();
    }

    public byte[] capabilityManifestDigest() {
      return capabilityManifestDigest.clone();
    }

    public BigInteger capabilityCommittedHeight() {
      return capabilityCommittedHeight;
    }

    public BigInteger admittedAtHeight() {
      return admittedAtHeight;
    }

    public BigInteger finalizedHeight() {
      return finalizedHeight;
    }

    public byte[] finalizedBlockHash() {
      return finalizedBlockHash.clone();
    }

    private void validate() {
      if (!isExactNonzeroLowerHash(networkIdHex)
          || !isExactNonzeroLowerHash(transactionHashHex)
          || protocolId != PrivacyExact12ActionContractV1.protocolId(operationSchema)
          || ledgerEffectKind != PrivacyExact12ActionContractV1.ledgerEffectKind(operationSchema)
          || actionIndex != ACTION_INDEX_V1) {
        throw new IllegalStateException("native action receipt has contradictory typed bindings");
      }
      requireNonzero32(transactionIntentDigest, "transactionIntentDigest");
      requireNonzero32(statementDigest, "statementDigest");
      requireNonzero32(proofEnvelopeHash, "proofEnvelopeHash");
      requireNonzero32(capabilityManifestDigest, "capabilityManifestDigest");
      requireNonzero32(finalizedBlockHash, "finalizedBlockHash");
      requirePositiveU64(capabilityCommittedHeight, "capabilityCommittedHeight");
      requirePositiveU64(admittedAtHeight, "admittedAtHeight");
      requirePositiveU64(finalizedHeight, "finalizedHeight");
      if (capabilityCommittedHeight.compareTo(admittedAtHeight) > 0
          || admittedAtHeight.compareTo(finalizedHeight) > 0) {
        throw new IllegalStateException(
            "native action receipt has contradictory capability, admission, or finality heights");
      }
    }
  }

  /** Builds and signs one fresh ID105 query bound to every inspected action digest. */
  public static SignedQueryV1 buildSignedPrivacyActionReceiptQueryV1(
      final PrivacyActionOperationViewV1 operation,
      final NetworkId networkId,
      final String authorityAccountId,
      final IrohaQuerySignatureProvider signer) {
    final byte[] nonce = new byte[NONCE_BYTES];
    for (int attempt = 0; attempt < 16 && allZero(nonce); attempt++) {
      NONCE_RANDOM.nextBytes(nonce);
    }
    if (allZero(nonce)) {
      throw new IllegalStateException(
          "secure receipt-query nonce generator repeatedly returned zero");
    }
    return buildSignedPrivacyActionReceiptQueryAtV1(
        operation,
        networkId,
        authorityAccountId,
        signer,
        System.currentTimeMillis(),
        nonce);
  }

  static SignedQueryV1 buildSignedPrivacyActionReceiptQueryAtV1(
      final PrivacyActionOperationViewV1 operation,
      final NetworkId networkId,
      final String authorityAccountId,
      final IrohaQuerySignatureProvider signer,
      final long creationTimeMs,
      final byte[] nonce) {
    requireNative();
    Objects.requireNonNull(operation, "operation");
    Objects.requireNonNull(networkId, "networkId");
    Objects.requireNonNull(signer, "signer");
    if (creationTimeMs <= 0) {
      throw new IllegalArgumentException("creationTimeMs must be positive");
    }
    if (nonce == null || nonce.length != NONCE_BYTES || allZero(nonce)) {
      throw new IllegalArgumentException("nonce must contain exactly 32 non-zero bytes");
    }
    final String transactionHashHex = lowerHex(operation.transactionHashBytes());
    final String networkIdHex = lowerHex(networkId.bytes());
    final byte[] binding = new byte[REQUEST_BINDING_BYTES];
    System.arraycopy(operation.transactionIntentDigestBytes(), 0, binding, 0, 32);
    System.arraycopy(operation.statementDigestBytes(), 0, binding, 32, 32);
    System.arraycopy(operation.proofEnvelopeHashBytes(), 0, binding, 64, 32);
    final byte[][] prepared =
        nativePreparePrivacyActionReceiptQueryV1(
            networkId.bytes(),
            utf8(authorityAccountId, "authorityAccountId"),
            operation.operationSchema.ordinal(),
            transactionHashHex.getBytes(StandardCharsets.US_ASCII),
            ACTION_INDEX_V1,
            binding,
            creationTimeMs,
            nonce.clone());
    if (prepared == null
        || prepared.length != 2
        || prepared[0] == null
        || prepared[0].length == 0
        || prepared[0].length > PREPARATION_MAX_BYTES
        || prepared[1] == null
        || prepared[1].length != DIGEST_BYTES) {
      throw new IllegalStateException("native receipt-query preparation returned an invalid shape");
    }
    final byte[] digest = prepared[1].clone();
    final byte[] signature;
    try {
      signature = signer.signQueryDigest(digest.clone());
    } finally {
      Arrays.fill(digest, (byte) 0);
      Arrays.fill(prepared[1], (byte) 0);
      Arrays.fill(binding, (byte) 0);
    }
    if (signature == null || signature.length == 0 || signature.length > SIGNATURE_MAX_BYTES) {
      throw new IllegalArgumentException("opaque query signer returned invalid signature bytes");
    }
    final byte[] requestBody =
        nativeFinalizePrivacyActionReceiptQueryV1(
            prepared[0].clone(), signature.clone());
    if (requestBody == null
        || requestBody.length == 0
        || requestBody.length > SIGNED_QUERY_MAX_BYTES) {
      throw new IllegalStateException("native receipt-query finalizer violated the request byte bound");
    }
    return new SignedQueryV1(
        prepared[0],
        requestBody,
        networkIdHex,
        operation.protocolId,
        operation.operationSchema,
        operation.ledgerEffectKind,
        transactionHashHex,
        ACTION_INDEX_V1,
        operation.transactionIntentDigestBytes(),
        operation.statementDigestBytes(),
        operation.proofEnvelopeHashBytes());
  }

  /** Natively verifies and projects the exact finalized receipt bound to {@code signedQuery}. */
  public static AuthenticatedActionExecutionReceiptV1 projectPrivacyActionReceiptV1(
      final SignedQueryV1 signedQuery, final byte[] responseNorito) {
    requireNative();
    final SignedQueryV1 exact = Objects.requireNonNull(signedQuery, "signedQuery");
    if (responseNorito == null
        || responseNorito.length == 0
        || (long) responseNorito.length > RESPONSE_MAX_BYTES) {
      throw new IllegalArgumentException("responseNorito violates its closed byte bound");
    }
    final byte[][] fields =
        nativeProjectPrivacyActionReceiptV1(exact.preparation(), responseNorito.clone());
    if (fields == null || fields.length != 15) {
      throw new IllegalStateException("native authenticated receipt projection has invalid shape");
    }
    if (!"1".equals(exactUtf8(fields[0], "version"))) {
      throw new IllegalStateException("native authenticated receipt version is invalid");
    }
    final String networkIdHex = exactNonzeroLowerHash(fields[1], "networkId");
    final PrivacyProtocolIdV1 protocolId =
        PrivacyProtocolIdV1.fromCanonicalLabel(exactUtf8(fields[2], "protocolId"));
    final PrivacyOperationSchemaV1 operationSchema =
        PrivacyOperationSchemaV1.fromCanonicalLabel(exactUtf8(fields[3], "operationSchema"));
    final PrivacyLedgerEffectKindV1 ledgerEffectKind =
        PrivacyLedgerEffectKindV1.fromCanonicalLabel(exactUtf8(fields[4], "ledgerEffectKind"));
    final String transactionHashHex = exactNonzeroLowerHash(fields[5], "transactionHash");
    final int actionIndex;
    try {
      actionIndex = exactUnsignedDecimal(fields[6], "actionIndex").intValueExact();
    } catch (final ArithmeticException error) {
      throw new IllegalStateException("native actionIndex does not fit an exact JVM int", error);
    }
    final byte[] transactionIntentDigest =
        exactNonzeroLowerHashBytes(fields[7], "transactionIntentDigest");
    final byte[] statementDigest = exactNonzeroLowerHashBytes(fields[8], "statementDigest");
    final byte[] proofEnvelopeHash = exactNonzeroLowerHashBytes(fields[9], "proofEnvelopeHash");
    final byte[] capabilityManifestDigest =
        exactNonzeroLowerHashBytes(fields[10], "capabilityManifestDigest");
    final BigInteger capabilityCommittedHeight =
        exactPositiveU64(fields[11], "capabilityCommittedHeight");
    final BigInteger admittedAtHeight = exactPositiveU64(fields[12], "admittedAtHeight");
    final BigInteger finalizedHeight = exactPositiveU64(fields[13], "finalizedHeight");
    final byte[] finalizedBlockHash =
        exactNonzeroLowerHashBytes(fields[14], "finalizedBlockHash");

    if (!networkIdHex.equals(exact.networkIdHex)
        || protocolId != exact.protocolId
        || operationSchema != exact.operationSchema
        || ledgerEffectKind != exact.ledgerEffectKind
        || protocolId != PrivacyExact12ActionContractV1.protocolId(operationSchema)
        || ledgerEffectKind != PrivacyExact12ActionContractV1.ledgerEffectKind(operationSchema)
        || !transactionHashHex.equals(exact.transactionHashHex)
        || actionIndex != exact.actionIndex
        || !MessageDigest.isEqual(transactionIntentDigest, exact.transactionIntentDigest)
        || !MessageDigest.isEqual(statementDigest, exact.statementDigest)
        || !MessageDigest.isEqual(proofEnvelopeHash, exact.proofEnvelopeHash)) {
      throw new IllegalStateException(
          "native authenticated receipt changed its requested action binding");
    }

    return new AuthenticatedActionExecutionReceiptV1(
        networkIdHex,
        protocolId,
        operationSchema,
        ledgerEffectKind,
        transactionHashHex,
        actionIndex,
        transactionIntentDigest,
        statementDigest,
        proofEnvelopeHash,
        capabilityManifestDigest,
        capabilityCommittedHeight,
        admittedAtHeight,
        finalizedHeight,
        finalizedBlockHash);
  }

  /** Verify one rejected Exact12 action against its exact binding, block, and QC page. */
  public static AuthenticatedFinalizedPrivacyActionRejectionV1
      projectFinalizedPrivacyActionRejectionV1(
          final AuthenticatedTransactionDetailsCarrierV2 carrier,
          final PrivacyActionOperationViewV1 operation,
          final NetworkId networkId,
          final AuthenticatedFinalityCheckpointV1 trustedCheckpoint,
          final AuthenticatedFinalityProofPageV1 finalityPage,
          final byte[] executedBlockWire) {
    requireNative();
    final AuthenticatedTransactionDetailsCarrierV2 exactCarrier =
        Objects.requireNonNull(carrier, "carrier");
    final PrivacyActionOperationViewV1 exactOperation =
        Objects.requireNonNull(operation, "operation");
    final NetworkId exactNetworkId = Objects.requireNonNull(networkId, "networkId");
    final AuthenticatedFinalityCheckpointV1 checkpoint =
        Objects.requireNonNull(trustedCheckpoint, "trustedCheckpoint");
    final AuthenticatedFinalityProofPageV1 page =
        Objects.requireNonNull(finalityPage, "finalityPage");
    if (exactCarrier.resultOkHint()) {
      throw new IllegalArgumentException(
          "finalized Exact12 rejection requires a rejected transaction-details carrier");
    }
    if (executedBlockWire == null
        || executedBlockWire.length == 0
        || (long) executedBlockWire.length
            > AuthenticatedTransactionDetailsNativeBridge.EXECUTED_BLOCK_WIRE_MAX_BYTES) {
      throw new IllegalArgumentException("executedBlockWire violates its closed byte bound");
    }
    final byte[] requestedActionBinding = new byte[REQUEST_BINDING_BYTES];
    System.arraycopy(
        exactOperation.transactionIntentDigestBytes(), 0, requestedActionBinding, 0, 32);
    System.arraycopy(exactOperation.statementDigestBytes(), 0, requestedActionBinding, 32, 32);
    System.arraycopy(exactOperation.proofEnvelopeHashBytes(), 0, requestedActionBinding, 64, 32);
    final byte[][] fields;
    try {
      fields =
          nativeProjectFinalizedPrivacyActionRejectionV1(
              exactCarrier.signedQuery().preparation(),
              exactCarrier.responseNorito(),
              exactOperation.operationSchema.ordinal(),
              ACTION_INDEX_V1,
              requestedActionBinding,
              exactNetworkId.bytes(),
              checkpoint.height(),
              checkpoint.heightContextId(),
              page.evidenceArchive(),
              executedBlockWire.clone());
    } finally {
      Arrays.fill(requestedActionBinding, (byte) 0);
    }
    if (fields == null || fields.length != 22) {
      throw new IllegalStateException(
          "native finalized Exact12 rejection projection has invalid shape");
    }
    if (!"1".equals(exactUtf8(fields[0], "version"))) {
      throw new IllegalStateException("native finalized Exact12 rejection version is invalid");
    }
    final String networkIdHex = exactNonzeroLowerHash(fields[1], "networkId");
    final PrivacyProtocolIdV1 protocolId =
        PrivacyProtocolIdV1.fromCanonicalLabel(exactUtf8(fields[2], "protocolId"));
    final PrivacyOperationSchemaV1 operationSchema =
        PrivacyOperationSchemaV1.fromCanonicalLabel(exactUtf8(fields[3], "operationSchema"));
    final PrivacyLedgerEffectKindV1 ledgerEffectKind =
        PrivacyLedgerEffectKindV1.fromCanonicalLabel(exactUtf8(fields[4], "ledgerEffectKind"));
    final String transactionHashHex = exactNonzeroLowerHash(fields[5], "transactionHash");
    final int actionIndex;
    try {
      actionIndex = exactUnsignedDecimal(fields[6], "actionIndex").intValueExact();
    } catch (final ArithmeticException error) {
      throw new IllegalStateException("native actionIndex does not fit an exact JVM int", error);
    }
    final byte[] transactionIntentDigest =
        exactNonzeroLowerHashBytes(fields[7], "transactionIntentDigest");
    final byte[] statementDigest = exactNonzeroLowerHashBytes(fields[8], "statementDigest");
    final byte[] proofEnvelopeHash =
        exactNonzeroLowerHashBytes(fields[9], "proofEnvelopeHash");
    final String queryAuthority = exactUtf8(fields[10], "queryAuthorityAccountId");
    final String transactionAuthority =
        exactUtf8(fields[11], "transactionAuthorityAccountId");
    final String blockHashHex = exactNonzeroLowerHash(fields[12], "blockHashHex");
    final String resultHashHex = exactNonzeroLowerHash(fields[13], "resultHashHex");
    final AuthenticatedPrivacyActionRejectionCodeV1 rejectionCode =
        AuthenticatedPrivacyActionRejectionCodeV1.fromCanonicalLabel(
            exactUtf8(fields[14], "rejectionCode"));
    final String rejectionMessage = exactUtf8(fields[15], "rejectionMessage");
    final long committedBlockHeight;
    try {
      committedBlockHeight =
          exactPositiveU64(fields[16], "committedBlockHeight").longValueExact();
    } catch (final ArithmeticException error) {
      throw new IllegalStateException(
          "native finalized Exact12 rejection height exceeds the mobile u63 range", error);
    }
    final AuthenticatedFinalizedPrivacyActionRejectionV1 rejection =
        new AuthenticatedFinalizedPrivacyActionRejectionV1(
            networkIdHex,
            protocolId,
            operationSchema,
            ledgerEffectKind,
            transactionHashHex,
            actionIndex,
            transactionIntentDigest,
            statementDigest,
            proofEnvelopeHash,
            queryAuthority,
            transactionAuthority,
            blockHashHex,
            resultHashHex,
            rejectionCode,
            rejectionMessage,
            committedBlockHeight,
            AuthenticatedFinalityCheckpointV1.fromProjection(fields[17]),
            exactNonzeroLowerHash(fields[18], "executedBlockWireHashHex"),
            exactNonzeroLowerHash(fields[19], "evidenceIdHex"),
            exactNonzeroLowerHash(fields[20], "transactionDetailsHashHex"),
            exactNonzeroLowerHash(fields[21], "finalityPageHashHex"));
    if (!rejection.networkIdHex().equals(lowerHex(exactNetworkId.bytes()))
        || rejection.protocolId() != exactOperation.protocolId
        || rejection.operationSchema() != exactOperation.operationSchema
        || rejection.ledgerEffectKind() != exactOperation.ledgerEffectKind
        || !rejection.transactionHashHex().equals(
            lowerHex(exactOperation.transactionHashBytes()))
        || rejection.actionIndex() != ACTION_INDEX_V1
        || !MessageDigest.isEqual(
            rejection.transactionIntentDigest(),
            exactOperation.transactionIntentDigestBytes())
        || !MessageDigest.isEqual(rejection.statementDigest(), exactOperation.statementDigestBytes())
        || !MessageDigest.isEqual(
            rejection.proofEnvelopeHash(), exactOperation.proofEnvelopeHashBytes())
        || rejection.committedBlockHeight() != exactCarrier.committedBlockHeightHint()
        || rejection.finalizedCheckpoint().height() <= checkpoint.height()
        || !rejection.finalityPageHashHex().equals(page.hashHex())
        || !rejection.finalityPageHashHex().equals(
            lowerHex(IrohaHash.prehash(page.evidenceArchive())))
        || !rejection.transactionDetailsHashHex().equals(
            lowerHex(IrohaHash.prehash(exactCarrier.responseNorito())))
        || !rejection.executedBlockWireHashHex().equals(
            lowerHex(IrohaHash.prehash(executedBlockWire)))) {
      throw new IllegalStateException(
          "native finalized Exact12 rejection changed its requested evidence binding");
    }
    return rejection;
  }

  /** Convenience overload which first creates the canonical QC page. */
  public static AuthenticatedFinalizedPrivacyActionRejectionV1
      projectFinalizedPrivacyActionRejectionV1(
          final AuthenticatedTransactionDetailsCarrierV2 carrier,
          final PrivacyActionOperationViewV1 operation,
          final NetworkId networkId,
          final AuthenticatedFinalityCheckpointV1 trustedCheckpoint,
          final byte[][] finalityProofsNorito,
          final byte[] executedBlockWire) {
    return projectFinalizedPrivacyActionRejectionV1(
        carrier,
        operation,
        networkId,
        trustedCheckpoint,
        AuthenticatedTransactionDetailsNativeBridge.bindFinalityProofPageV1(
            finalityProofsNorito),
        executedBlockWire);
  }

  private static byte[] utf8(final String value, final String field) {
    return Objects.requireNonNull(value, field).getBytes(StandardCharsets.UTF_8);
  }

  private static String exactUtf8(final byte[] value, final String field) {
    if (value == null || value.length == 0) {
      throw new IllegalStateException("native " + field + " is empty");
    }
    try {
      final CharBuffer decoded =
          StandardCharsets.UTF_8
              .newDecoder()
              .onMalformedInput(CodingErrorAction.REPORT)
              .onUnmappableCharacter(CodingErrorAction.REPORT)
              .decode(ByteBuffer.wrap(value));
      return decoded.toString();
    } catch (final CharacterCodingException error) {
      throw new IllegalStateException("native " + field + " is not exact UTF-8", error);
    }
  }

  private static BigInteger exactUnsignedDecimal(final byte[] value, final String field) {
    final String text = exactUtf8(value, field);
    if (text.isEmpty() || (text.length() > 1 && text.charAt(0) == '0')) {
      throw new IllegalStateException("native " + field + " is not canonical decimal");
    }
    for (int index = 0; index < text.length(); index++) {
      if (text.charAt(index) < '0' || text.charAt(index) > '9') {
        throw new IllegalStateException("native " + field + " is not canonical decimal");
      }
    }
    return new BigInteger(text);
  }

  private static BigInteger exactPositiveU64(final byte[] value, final String field) {
    final BigInteger exact = exactUnsignedDecimal(value, field);
    requirePositiveU64(exact, field);
    return exact;
  }

  private static void requirePositiveU64(final BigInteger value, final String field) {
    if (value.signum() <= 0 || value.bitLength() > 64) {
      throw new IllegalStateException("native " + field + " is not a positive u64");
    }
  }

  private static String exactNonzeroLowerHash(final byte[] value, final String field) {
    final String text = exactUtf8(value, field);
    if (!isExactNonzeroLowerHash(text)) {
      throw new IllegalStateException(
          "native " + field + " is not an exact non-zero lowercase 32-byte hash");
    }
    return text;
  }

  private static byte[] exactNonzeroLowerHashBytes(final byte[] value, final String field) {
    final String text = exactNonzeroLowerHash(value, field);
    final byte[] decoded = new byte[32];
    for (int index = 0; index < decoded.length; index++) {
      decoded[index] =
          (byte)
              ((Character.digit(text.charAt(index * 2), 16) << 4)
                  | Character.digit(text.charAt(index * 2 + 1), 16));
    }
    return decoded;
  }

  private static boolean isExactNonzeroLowerHash(final String value) {
    if (value == null || value.length() != 64) {
      return false;
    }
    boolean nonzero = false;
    for (int index = 0; index < value.length(); index++) {
      final char item = value.charAt(index);
      if (!((item >= '0' && item <= '9') || (item >= 'a' && item <= 'f'))) {
        return false;
      }
      nonzero |= item != '0';
    }
    return nonzero;
  }

  private static void requireNonzero32(final byte[] value, final String field) {
    if (value == null || value.length != 32 || allZero(value)) {
      throw new IllegalStateException(field + " must contain exactly 32 non-zero bytes");
    }
  }

  private static String lowerHex(final byte[] value) {
    final char[] encoded = new char[value.length * 2];
    final char[] alphabet = "0123456789abcdef".toCharArray();
    for (int index = 0; index < value.length; index++) {
      final int item = value[index] & 0xff;
      encoded[index * 2] = alphabet[item >>> 4];
      encoded[index * 2 + 1] = alphabet[item & 0x0f];
    }
    return new String(encoded);
  }

  private static boolean allZero(final byte[] value) {
    for (final byte item : value) {
      if (item != 0) {
        return false;
      }
    }
    return true;
  }

  private static void requireNative() {
    NativeHolder.requireAvailable();
  }

  private static final class NativeHolder {
    private static final Throwable LOAD_ERROR;

    static {
      Throwable failure = null;
      try {
        System.loadLibrary("connect_norito_bridge");
        final int actual = nativeBridgeAbiVersion();
        if (actual != REQUIRED_BRIDGE_ABI_VERSION) {
          failure =
              new IllegalStateException(
                  "native authenticated receipt ABI mismatch: expected "
                      + REQUIRED_BRIDGE_ABI_VERSION
                      + ", found "
                      + actual);
        }
      } catch (final RuntimeException | LinkageError error) {
        failure = error;
      }
      LOAD_ERROR = failure;
    }

    private NativeHolder() {}

    private static void requireAvailable() {
      if (LOAD_ERROR != null) {
        throw new IllegalStateException(
            "ABI-22 native authenticated Exact12 receipt bridge is unavailable", LOAD_ERROR);
      }
    }
  }

  private static native int nativeBridgeAbiVersion();

  private static native byte[][] nativePreparePrivacyActionReceiptQueryV1(
      byte[] networkId,
      byte[] authorityAccountId,
      int operationIndex,
      byte[] transactionHashHex,
      int actionIndex,
      byte[] requestedActionBinding,
      long creationTimeMs,
      byte[] nonce);

  private static native byte[] nativeFinalizePrivacyActionReceiptQueryV1(
      byte[] preparation, byte[] signature);

  private static native byte[][] nativeProjectPrivacyActionReceiptV1(
      byte[] preparation, byte[] responseNorito);

  private static native byte[][] nativeProjectFinalizedPrivacyActionRejectionV1(
      byte[] preparation,
      byte[] responseNorito,
      int operationIndex,
      int actionIndex,
      byte[] requestedActionBinding,
      byte[] networkId,
      long trustedCheckpointHeight,
      byte[] trustedCheckpointContextId,
      byte[] finalityPageArchive,
      byte[] executedBlockWire);
}
