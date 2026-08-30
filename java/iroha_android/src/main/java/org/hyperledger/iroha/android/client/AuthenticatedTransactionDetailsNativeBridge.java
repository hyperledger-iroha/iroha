package org.hyperledger.iroha.android.client;

import java.nio.ByteBuffer;
import java.nio.CharBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.security.SecureRandom;
import java.util.Arrays;
import java.util.Objects;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProver;

/** ABI-22 native codec and verifier for authenticated committed-transaction lookup. */
public final class AuthenticatedTransactionDetailsNativeBridge {
  public static final int REQUIRED_BRIDGE_ABI_VERSION = 22;
  public static final long RESPONSE_MAX_BYTES = 64L * 1024L * 1024L;
  public static final int FINALITY_PAGE_MAX_PROOFS = 64;
  public static final long FINALITY_PROOF_MAX_BYTES = 9L * 1024L * 1024L;
  public static final long FINALITY_PAGE_MAX_BYTES = 64L * 1024L * 1024L;
  public static final long EXECUTED_BLOCK_WIRE_MAX_BYTES = 32L * 1024L * 1024L;
  private static final int DIGEST_BYTES = 32;
  private static final int NONCE_BYTES = 32;
  private static final int PREPARATION_MAX_BYTES = 64 * 1024;
  private static final int SIGNATURE_MAX_BYTES = 16 * 1024;
  private static final int SIGNED_QUERY_MAX_BYTES = 64 * 1024;
  private static final SecureRandom NONCE_RANDOM = new SecureRandom();

  private AuthenticatedTransactionDetailsNativeBridge() {}

  /** Native-bound signed request plus the private native preparation used to verify its response. */
  public static final class SignedQueryV1 {
    private final byte[] preparation;
    private final byte[] requestBody;

    private SignedQueryV1(final byte[] preparation, final byte[] requestBody) {
      this.preparation = preparation.clone();
      this.requestBody = requestBody.clone();
    }

    /** Canonical versioned `SignedQuery` bytes for `/v1/pipeline/transactions/details`. */
    public byte[] requestBody() {
      return requestBody.clone();
    }

    byte[] preparation() {
      return preparation.clone();
    }
  }

  /** Native-bound authority-split signed request and response-verification preparation. */
  public static final class SignedQueryV2 {
    private final byte[] preparation;
    private final byte[] requestBody;

    private SignedQueryV2(final byte[] preparation, final byte[] requestBody) {
      this.preparation = preparation.clone();
      this.requestBody = requestBody.clone();
    }

    public byte[] requestBody() { return requestBody.clone(); }
    byte[] preparation() { return preparation.clone(); }
  }

  /**
   * Builds and signs one fresh exact-hash `FindTransactions` request.
   *
   * <p>The callback sees only a defensive copy of a 32-byte digest. The native finalizer checks
   * the returned signature against the public key embedded in {@code authorityAccountId}.
   */
  public static SignedQueryV1 buildSignedRejectedTransactionQueryV1(
      final String transactionHashHex,
      final NetworkId networkId,
      final String authorityAccountId,
      final IrohaQuerySignatureProvider signer) {
    final byte[] nonce = new byte[NONCE_BYTES];
    for (int attempt = 0; attempt < 16 && allZero(nonce); attempt++) {
      NONCE_RANDOM.nextBytes(nonce);
    }
    if (allZero(nonce)) {
      throw new IllegalStateException("secure query nonce generator repeatedly returned zero");
    }
    return buildSignedRejectedTransactionQueryAtV1(
        transactionHashHex,
        networkId,
        authorityAccountId,
        signer,
        System.currentTimeMillis(),
        nonce);
  }

  /** Builds the same exact-hash query without assuming its committed result. */
  public static SignedQueryV1 buildSignedTransactionDetailsQueryV1(
      final String transactionHashHex,
      final NetworkId networkId,
      final String authorityAccountId,
      final IrohaQuerySignatureProvider signer) {
    return buildSignedRejectedTransactionQueryV1(
        transactionHashHex, networkId, authorityAccountId, signer);
  }

  static SignedQueryV1 buildSignedRejectedTransactionQueryAtV1(
      final String transactionHashHex,
      final NetworkId networkId,
      final String authorityAccountId,
      final IrohaQuerySignatureProvider signer,
      final long creationTimeMs,
      final byte[] nonce) {
    requireNative();
    Objects.requireNonNull(networkId, "networkId");
    Objects.requireNonNull(signer, "signer");
    if (nonce == null || nonce.length != NONCE_BYTES || allZero(nonce)) {
      throw new IllegalArgumentException("nonce must contain exactly 32 nonzero bytes");
    }
    final byte[][] prepared =
        nativePrepareExactRejectedTransactionQueryV1(
            networkId.bytes(),
            utf8(authorityAccountId, "authorityAccountId"),
            utf8(transactionHashHex, "transactionHashHex"),
            creationTimeMs,
            nonce.clone());
    if (prepared == null
        || prepared.length != 2
        || prepared[0] == null
        || prepared[0].length == 0
        || prepared[0].length > PREPARATION_MAX_BYTES
        || prepared[1] == null
        || prepared[1].length != DIGEST_BYTES) {
      throw new IllegalStateException("native query preparation returned an invalid shape");
    }
    final byte[] digest = prepared[1].clone();
    final byte[] signature;
    try {
      signature = signer.signQueryDigest(digest.clone());
    } finally {
      Arrays.fill(digest, (byte) 0);
      Arrays.fill(prepared[1], (byte) 0);
    }
    if (signature == null || signature.length == 0 || signature.length > SIGNATURE_MAX_BYTES) {
      throw new IllegalArgumentException("opaque query signer returned invalid signature bytes");
    }
    final byte[] requestBody =
        nativeFinalizeExactRejectedTransactionQueryV1(
            prepared[0].clone(), signature.clone());
    if (requestBody == null
        || requestBody.length == 0
        || requestBody.length > SIGNED_QUERY_MAX_BYTES) {
      throw new IllegalStateException("native query finalizer violated the request byte bound");
    }
    return new SignedQueryV1(prepared[0], requestBody);
  }

  /** Build one exact-hash query with independent query and expected transaction authorities. */
  public static SignedQueryV2 buildSignedTransactionDetailsQueryV2(
      final String transactionHashHex,
      final NetworkId networkId,
      final String queryAuthorityAccountId,
      final String expectedTransactionAuthorityAccountId,
      final IrohaQuerySignatureProvider signer) {
    final byte[] nonce = new byte[NONCE_BYTES];
    for (int attempt = 0; attempt < 16 && allZero(nonce); attempt++) {
      NONCE_RANDOM.nextBytes(nonce);
    }
    if (allZero(nonce)) {
      throw new IllegalStateException("secure query nonce generator repeatedly returned zero");
    }
    return buildSignedTransactionDetailsQueryAtV2(
        transactionHashHex,
        networkId,
        queryAuthorityAccountId,
        expectedTransactionAuthorityAccountId,
        signer,
        System.currentTimeMillis(),
        nonce);
  }

  static SignedQueryV2 buildSignedTransactionDetailsQueryAtV2(
      final String transactionHashHex,
      final NetworkId networkId,
      final String queryAuthorityAccountId,
      final String expectedTransactionAuthorityAccountId,
      final IrohaQuerySignatureProvider signer,
      final long creationTimeMs,
      final byte[] nonce) {
    requireNative();
    Objects.requireNonNull(networkId, "networkId");
    Objects.requireNonNull(signer, "signer");
    if (nonce == null || nonce.length != NONCE_BYTES || allZero(nonce)) {
      throw new IllegalArgumentException("nonce must contain exactly 32 nonzero bytes");
    }
    final byte[][] prepared =
        nativePrepareExactTransactionQueryV2(
            networkId.bytes(),
            utf8(queryAuthorityAccountId, "queryAuthorityAccountId"),
            utf8(expectedTransactionAuthorityAccountId,
                "expectedTransactionAuthorityAccountId"),
            utf8(transactionHashHex, "transactionHashHex"),
            creationTimeMs,
            nonce.clone());
    if (prepared == null
        || prepared.length != 2
        || prepared[0] == null
        || prepared[0].length == 0
        || prepared[0].length > PREPARATION_MAX_BYTES
        || prepared[1] == null
        || prepared[1].length != DIGEST_BYTES) {
      throw new IllegalStateException("native V2 query preparation returned an invalid shape");
    }
    final byte[] digest = prepared[1].clone();
    final byte[] signature;
    try {
      signature = signer.signQueryDigest(digest.clone());
    } finally {
      Arrays.fill(digest, (byte) 0);
      Arrays.fill(prepared[1], (byte) 0);
    }
    if (signature == null || signature.length == 0 || signature.length > SIGNATURE_MAX_BYTES) {
      throw new IllegalArgumentException("opaque query signer returned invalid signature bytes");
    }
    final byte[] requestBody =
        nativeFinalizeExactTransactionQueryV2(prepared[0].clone(), signature.clone());
    if (requestBody == null
        || requestBody.length == 0
        || requestBody.length > SIGNED_QUERY_MAX_BYTES) {
      throw new IllegalStateException("native V2 query finalizer violated the request byte bound");
    }
    return new SignedQueryV2(prepared[0], requestBody);
  }

  public static AuthenticatedCommittedRejectionV2 projectCommittedRejectionV2(
      final SignedQueryV2 signedQuery, final byte[] responseNorito) {
    final SignedQueryV2 exact = Objects.requireNonNull(signedQuery, "signedQuery");
    return projectCommittedRejectionFieldsV2(
        nativeProjectExactCommittedRejectionV2(
            exact.preparation(), boundedResponseV2(responseNorito)));
  }

  public static AuthenticatedCommittedRejectionV2 projectKagemushaCommittedRejectionV2(
      final SignedQueryV2 signedQuery,
      final byte[] responseNorito,
      final byte[] expectedOperationId,
      final String expectedKind,
      final byte[] expectedRequestNorito) {
    final SignedQueryV2 exact = Objects.requireNonNull(signedQuery, "signedQuery");
    return projectCommittedRejectionFieldsV2(
        nativeProjectExactKagemushaCommittedRejectionV2(
            exact.preparation(),
            boundedResponseV2(responseNorito),
            Objects.requireNonNull(expectedOperationId, "expectedOperationId").clone(),
            utf8(expectedKind, "expectedKind"),
            Objects.requireNonNull(expectedRequestNorito, "expectedRequestNorito").clone()));
  }

  /** Natively verifies either authority-split committed success or rejection. */
  public static AuthenticatedCommittedTransactionResultV2 projectCommittedTransactionResultV2(
      final SignedQueryV2 signedQuery, final byte[] responseNorito) {
    final SignedQueryV2 exact = Objects.requireNonNull(signedQuery, "signedQuery");
    final byte[][] fields =
        nativeProjectExactCommittedTransactionResultV2(
            exact.preparation(), boundedResponseV2(responseNorito));
    if (fields == null || fields.length != 8) {
      throw new IllegalStateException(
          "native committed transaction-result V2 projection has invalid shape");
    }
    final String resultText = exactUtf8(fields[5], "resultOk");
    final boolean resultOk;
    if ("true".equals(resultText)) {
      resultOk = true;
    } else if ("false".equals(resultText)) {
      resultOk = false;
    } else {
      throw new IllegalStateException("native committed result flag is invalid");
    }
    final String reason = exactUtf8AllowEmpty(fields[6], "rejectionMessage");
    final java.math.BigInteger height;
    try {
      height = new java.math.BigInteger(
          exactPositiveDecimal(fields[7], "committedBlockHeight"));
    } catch (final NumberFormatException error) {
      throw new IllegalStateException("native committed block height is invalid", error);
    }
    return new AuthenticatedCommittedTransactionResultV2(
        exactUtf8(fields[0], "transactionHashHex"),
        exactUtf8(fields[1], "queryAuthorityAccountId"),
        exactUtf8(fields[2], "transactionAuthorityAccountId"),
        exactUtf8(fields[3], "blockHashHex"),
        exactUtf8(fields[4], "resultHashHex"),
        resultOk,
        reason.isEmpty() ? null : reason,
        height);
  }

  /** Bind exact individual Torii proof bodies into one canonical content-addressed page archive. */
  public static AuthenticatedFinalityProofPageV1 bindFinalityProofPageV1(
      final byte[][] finalityProofsNorito) {
    requireNative();
    if (finalityProofsNorito == null
        || finalityProofsNorito.length == 0
        || finalityProofsNorito.length > FINALITY_PAGE_MAX_PROOFS) {
      throw new IllegalArgumentException("finalityProofsNorito must contain 1..64 proofs");
    }
    final byte[][] copies = new byte[finalityProofsNorito.length][];
    long aggregate = 0L;
    for (int index = 0; index < finalityProofsNorito.length; index++) {
      final byte[] proof = finalityProofsNorito[index];
      if (proof == null || proof.length == 0 || (long) proof.length > FINALITY_PROOF_MAX_BYTES) {
        throw new IllegalArgumentException(
            "finalityProofsNorito[" + index + "] violates its closed byte bound");
      }
      aggregate += proof.length;
      if (aggregate > FINALITY_PAGE_MAX_BYTES) {
        throw new IllegalArgumentException("finalityProofsNorito exceeds its aggregate byte bound");
      }
      copies[index] = proof.clone();
    }
    final byte[][] fields = nativeBindFinalityProofPageV1(copies);
    if (fields == null || fields.length != 2) {
      throw new IllegalStateException("native finality page binding returned an invalid shape");
    }
    final String hashHex = exactUtf8(fields[1], "finalityPageHashHex");
    final AuthenticatedFinalityProofPageV1 page =
        new AuthenticatedFinalityProofPageV1(fields[0], hashHex);
    if (!hashHex.equals(lowerHex(IrohaHash.prehash(page.evidenceArchive())))) {
      throw new IllegalStateException("native finality page hash differs from its exact archive");
    }
    return page;
  }

  /** Verify one bounded contiguous finality page from an application-persisted checkpoint. */
  public static AuthenticatedFinalityCheckpointV1 verifyFinalityPageV1(
      final NetworkId networkId,
      final AuthenticatedFinalityCheckpointV1 trustedCheckpoint,
      final AuthenticatedFinalityProofPageV1 page) {
    requireNative();
    final AuthenticatedFinalityCheckpointV1 checkpoint =
        Objects.requireNonNull(trustedCheckpoint, "trustedCheckpoint");
    final byte[] projection =
        nativeVerifyFinalityPageV1(
            Objects.requireNonNull(networkId, "networkId").bytes(),
            checkpoint.height(),
            checkpoint.heightContextId(),
            Objects.requireNonNull(page, "page").evidenceArchive());
    return AuthenticatedFinalityCheckpointV1.fromProjection(projection);
  }

  /** Convenience overload which first creates the canonical content-addressed page. */
  public static AuthenticatedFinalityCheckpointV1 verifyFinalityPageV1(
      final NetworkId networkId,
      final AuthenticatedFinalityCheckpointV1 trustedCheckpoint,
      final byte[][] finalityProofsNorito) {
    return verifyFinalityPageV1(
        networkId, trustedCheckpoint, bindFinalityProofPageV1(finalityProofsNorito));
  }

  /**
   * Bind a structurally verified response to its private signed-query preparation.
   *
   * <p>The exposed result and height remain routing hints until finalized outcome verification.
   */
  public static AuthenticatedTransactionDetailsCarrierV2 bindTransactionDetailsCarrierV2(
      final SignedQueryV2 signedQuery, final byte[] responseNorito) {
    final byte[] exactResponse = boundedResponseV2(responseNorito);
    final AuthenticatedCommittedTransactionResultV2 hint =
        projectCommittedTransactionResultV2(
            Objects.requireNonNull(signedQuery, "signedQuery"), exactResponse);
    final long height;
    try {
      height = hint.committedBlockHeight().longValueExact();
    } catch (final ArithmeticException error) {
      throw new IllegalStateException("committedBlockHeightHint exceeds the mobile u63 range", error);
    }
    return new AuthenticatedTransactionDetailsCarrierV2(
        signedQuery, exactResponse, height, hint.resultOk());
  }

  /**
   * Verify one exact Kagemusha issuer outcome against validator finality and executed-block evidence.
   */
  public static AuthenticatedFinalizedKagemushaOutcomeV1 projectFinalizedKagemushaOutcomeV1(
      final AuthenticatedTransactionDetailsCarrierV2 carrier,
      final byte[] expectedOperationId,
      final String expectedKind,
      final byte[] expectedRequestNorito,
      final NetworkId networkId,
      final AuthenticatedFinalityCheckpointV1 trustedCheckpoint,
      final AuthenticatedFinalityProofPageV1 finalityPage,
      final byte[] executedBlockWire) {
    requireNative();
    final AuthenticatedTransactionDetailsCarrierV2 exactCarrier =
        Objects.requireNonNull(carrier, "carrier");
    final AuthenticatedFinalityCheckpointV1 checkpoint =
        Objects.requireNonNull(trustedCheckpoint, "trustedCheckpoint");
    final AuthenticatedFinalityProofPageV1 page =
        Objects.requireNonNull(finalityPage, "finalityPage");
    if (executedBlockWire == null
        || executedBlockWire.length == 0
        || (long) executedBlockWire.length > EXECUTED_BLOCK_WIRE_MAX_BYTES) {
      throw new IllegalArgumentException("executedBlockWire violates its closed byte bound");
    }
    final byte[][] fields =
        nativeProjectFinalizedKagemushaOutcomeV1(
            exactCarrier.signedQuery().preparation(),
            exactCarrier.responseNorito(),
            Objects.requireNonNull(expectedOperationId, "expectedOperationId").clone(),
            utf8(expectedKind, "expectedKind"),
            Objects.requireNonNull(expectedRequestNorito, "expectedRequestNorito").clone(),
            Objects.requireNonNull(networkId, "networkId").bytes(),
            checkpoint.height(),
            checkpoint.heightContextId(),
            page.evidenceArchive(),
            executedBlockWire.clone());
    if (fields == null || fields.length != 16) {
      throw new IllegalStateException("native finalized Kagemusha outcome has invalid shape");
    }
    final String terminal = exactUtf8(fields[0], "terminalState");
    final AuthenticatedFinalizedKagemushaOutcomeV1.TerminalState terminalState;
    if ("applied".equals(terminal)) {
      terminalState = AuthenticatedFinalizedKagemushaOutcomeV1.TerminalState.APPLIED;
    } else if ("rejected".equals(terminal)) {
      terminalState = AuthenticatedFinalizedKagemushaOutcomeV1.TerminalState.REJECTED;
    } else {
      throw new IllegalStateException("native finalized terminal state is invalid");
    }
    final long height;
    try {
      height = Long.parseLong(exactPositiveDecimal(fields[8], "committedBlockHeight"));
    } catch (final NumberFormatException error) {
      throw new IllegalStateException("native finalized block height is invalid", error);
    }
    final String rejectionCode = exactUtf8AllowEmpty(fields[11], "rejectionCode");
    final String rejectionMessage = exactUtf8AllowEmpty(fields[12], "rejectionMessage");
    final AuthenticatedFinalizedKagemushaOutcomeV1 outcome =
        new AuthenticatedFinalizedKagemushaOutcomeV1(
            terminalState,
            fields[1],
            exactUtf8(fields[2], "operationKind"),
            exactUtf8(fields[3], "transactionHashHex"),
            exactUtf8(fields[4], "queryAuthorityAccountId"),
            exactUtf8(fields[5], "transactionAuthorityAccountId"),
            exactUtf8(fields[6], "blockHashHex"),
            exactUtf8(fields[7], "resultHashHex"),
            height,
            AuthenticatedFinalityCheckpointV1.fromProjection(fields[9]),
            exactUtf8(fields[10], "executedBlockWireHashHex"),
            rejectionCode.isEmpty() ? null : rejectionCode,
            rejectionMessage.isEmpty() ? null : rejectionMessage,
            exactUtf8(fields[13], "evidenceIdHex"),
            exactUtf8(fields[14], "transactionDetailsHashHex"),
            exactUtf8(fields[15], "finalityPageHashHex"));
    requireCarrierRoutingHintsAgreeV1(
        exactCarrier.committedBlockHeightHint(), exactCarrier.resultOkHint(), outcome);
    if (!outcome.finalityPageHashHex().equals(page.hashHex())
        || !outcome.transactionDetailsHashHex().equals(
            lowerHex(IrohaHash.prehash(exactCarrier.responseNorito())))
        || !outcome.executedBlockWireHashHex().equals(
            lowerHex(IrohaHash.prehash(executedBlockWire)))) {
      throw new IllegalStateException("native finalized evidence content hashes are inconsistent");
    }
    return outcome;
  }

  /** Convenience overload which first creates the canonical content-addressed page. */
  public static AuthenticatedFinalizedKagemushaOutcomeV1 projectFinalizedKagemushaOutcomeV1(
      final AuthenticatedTransactionDetailsCarrierV2 carrier,
      final byte[] expectedOperationId,
      final String expectedKind,
      final byte[] expectedRequestNorito,
      final NetworkId networkId,
      final AuthenticatedFinalityCheckpointV1 trustedCheckpoint,
      final byte[][] finalityProofsNorito,
      final byte[] executedBlockWire) {
    return projectFinalizedKagemushaOutcomeV1(
        carrier,
        expectedOperationId,
        expectedKind,
        expectedRequestNorito,
        networkId,
        trustedCheckpoint,
        bindFinalityProofPageV1(finalityProofsNorito),
        executedBlockWire);
  }

  /**
   * Require the uniform finalized issuer outcome and specialized top-up proof to authenticate the
   * same successful top-up, transaction, block, height, and height context.
   */
  public static void requireKagemushaTopUpFinalityAgreementV1(
      final AuthenticatedFinalizedKagemushaOutcomeV1 outcome,
      final KagemushaRecursiveSpendProver.VerifiedTopUpFinalityV4 specialized) {
    final AuthenticatedFinalizedKagemushaOutcomeV1 exactOutcome =
        Objects.requireNonNull(outcome, "outcome");
    final KagemushaRecursiveSpendProver.VerifiedTopUpFinalityV4 exactSpecialized =
        Objects.requireNonNull(specialized, "specialized");
    requireKagemushaTopUpFinalityAgreementFieldsV1(
        exactOutcome,
        exactSpecialized.operationId(),
        exactSpecialized.transactionHashHex(),
        exactSpecialized.height(),
        exactSpecialized.blockHashHex(),
        exactSpecialized.heightContextId());
  }

  static void requireCarrierRoutingHintsAgreeV1(
      final long committedBlockHeightHint,
      final boolean resultOkHint,
      final AuthenticatedFinalizedKagemushaOutcomeV1 outcome) {
    final AuthenticatedFinalizedKagemushaOutcomeV1 exact =
        Objects.requireNonNull(outcome, "outcome");
    if (committedBlockHeightHint != exact.committedBlockHeight()
        || resultOkHint
            != (exact.terminalState()
                == AuthenticatedFinalizedKagemushaOutcomeV1.TerminalState.APPLIED)) {
      throw new IllegalArgumentException(
          "transaction-details routing hints disagree with finalized native evidence");
    }
  }

  static void requireKagemushaTopUpFinalityAgreementFieldsV1(
      final AuthenticatedFinalizedKagemushaOutcomeV1 outcome,
      final byte[] operationId,
      final String transactionHashHex,
      final long height,
      final String blockHashHex,
      final byte[] heightContextId) {
    final AuthenticatedFinalizedKagemushaOutcomeV1 exact =
        Objects.requireNonNull(outcome, "outcome");
    if (exact.terminalState()
            != AuthenticatedFinalizedKagemushaOutcomeV1.TerminalState.APPLIED
        || !"top_up".equals(exact.operationKind())
        || !Arrays.equals(exact.operationId(), operationId)
        || !exact.transactionHashHex().equals(transactionHashHex)
        || exact.committedBlockHeight() != height
        || !exact.blockHashHex().equals(blockHashHex)
        || !Arrays.equals(exact.finalizedCheckpoint().heightContextId(), heightContextId)) {
      throw new IllegalArgumentException(
          "uniform and specialized Kagemusha top-up finality evidence disagree");
    }
  }

  private static byte[] boundedResponseV2(final byte[] responseNorito) {
    requireNative();
    if (responseNorito == null
        || responseNorito.length == 0
        || (long) responseNorito.length > RESPONSE_MAX_BYTES) {
      throw new IllegalArgumentException("responseNorito violates its closed byte bound");
    }
    return responseNorito.clone();
  }

  private static AuthenticatedCommittedRejectionV2 projectCommittedRejectionFieldsV2(
      final byte[][] fields) {
    if (fields == null || fields.length != 8) {
      throw new IllegalStateException("native committed rejection V2 projection has invalid shape");
    }
    final long height;
    try {
      height = Long.parseLong(exactPositiveDecimal(fields[7], "committedBlockHeight"));
    } catch (final NumberFormatException error) {
      throw new IllegalStateException("native committed block height is invalid", error);
    }
    return new AuthenticatedCommittedRejectionV2(
        exactUtf8(fields[0], "transactionHashHex"),
        exactUtf8(fields[1], "queryAuthorityAccountId"),
        exactUtf8(fields[2], "transactionAuthorityAccountId"),
        exactUtf8(fields[3], "blockHashHex"),
        exactUtf8(fields[4], "resultHashHex"),
        exactUtf8(fields[5], "rejectionCode"),
        exactUtf8(fields[6], "rejectionMessage"),
        height);
  }

  /** Natively verifies and projects the exact committed rejection bound to {@code signedQuery}. */
  public static AuthenticatedCommittedRejectionV1 projectCommittedRejectionV1(
      final SignedQueryV1 signedQuery, final byte[] responseNorito) {
    requireNative();
    final SignedQueryV1 exact = Objects.requireNonNull(signedQuery, "signedQuery");
    if (responseNorito == null
        || responseNorito.length == 0
        || (long) responseNorito.length > RESPONSE_MAX_BYTES) {
      throw new IllegalArgumentException("responseNorito violates its closed byte bound");
    }
    final byte[][] fields =
        nativeProjectExactCommittedRejectionV1(
            exact.preparation(), responseNorito.clone());
    if (fields == null || fields.length != 7) {
      throw new IllegalStateException("native committed rejection projection has invalid shape");
    }
    final String heightText = exactPositiveDecimal(fields[6], "committedBlockHeight");
    final long height;
    try {
      height = Long.parseLong(heightText);
    } catch (final NumberFormatException error) {
      throw new IllegalStateException("native committed block height is invalid", error);
    }
    return new AuthenticatedCommittedRejectionV1(
        exactUtf8(fields[0], "transactionHashHex"),
        exactUtf8(fields[1], "transactionAuthorityAccountId"),
        exactUtf8(fields[2], "blockHashHex"),
        exactUtf8(fields[3], "resultHashHex"),
        exactUtf8(fields[4], "rejectionCode"),
        exactUtf8(fields[5], "rejectionMessage"),
        height);
  }

  /** Natively verifies and projects either committed success or rejection. */
  public static AuthenticatedCommittedTransactionResultV1 projectCommittedTransactionResultV1(
      final SignedQueryV1 signedQuery, final byte[] responseNorito) {
    requireNative();
    final SignedQueryV1 exact = Objects.requireNonNull(signedQuery, "signedQuery");
    if (responseNorito == null
        || responseNorito.length == 0
        || (long) responseNorito.length > RESPONSE_MAX_BYTES) {
      throw new IllegalArgumentException("responseNorito violates its closed byte bound");
    }
    final byte[][] fields =
        nativeProjectExactCommittedTransactionResultV1(
            exact.preparation(), responseNorito.clone());
    if (fields == null || fields.length != 7) {
      throw new IllegalStateException(
          "native committed transaction-result projection has invalid shape");
    }
    final String resultText = exactUtf8(fields[4], "resultOk");
    final boolean resultOk;
    if ("true".equals(resultText)) {
      resultOk = true;
    } else if ("false".equals(resultText)) {
      resultOk = false;
    } else {
      throw new IllegalStateException("native committed result flag is invalid");
    }
    final String reason = exactUtf8AllowEmpty(fields[5], "rejectionMessage");
    final java.math.BigInteger height;
    try {
      height = new java.math.BigInteger(
          exactPositiveDecimal(fields[6], "committedBlockHeight"));
    } catch (final NumberFormatException error) {
      throw new IllegalStateException("native committed block height is invalid", error);
    }
    return new AuthenticatedCommittedTransactionResultV1(
        exactUtf8(fields[0], "transactionHashHex"),
        exactUtf8(fields[1], "transactionAuthorityAccountId"),
        exactUtf8(fields[2], "blockHashHex"),
        exactUtf8(fields[3], "resultHashHex"),
        resultOk,
        reason.isEmpty() ? null : reason,
        height);
  }

  /** Natively verifies and projects exactly one committed offline-device registration result. */
  public static AuthenticatedOfflineDeviceRegistrationResultV1
      projectCommittedOfflineDeviceRegistrationResultV1(
          final SignedQueryV1 signedQuery, final byte[] responseNorito) {
    requireNative();
    final SignedQueryV1 exact = Objects.requireNonNull(signedQuery, "signedQuery");
    if (responseNorito == null
        || responseNorito.length == 0
        || (long) responseNorito.length > RESPONSE_MAX_BYTES) {
      throw new IllegalArgumentException("responseNorito violates its closed byte bound");
    }
    final byte[] json =
        nativeProjectExactOfflineDeviceRegistrationResultV1(
            exact.preparation(), responseNorito.clone());
    return AuthenticatedOfflineDeviceRegistrationResultV1.parseNativeJson(json);
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

  private static String exactUtf8AllowEmpty(final byte[] value, final String field) {
    if (value == null) {
      throw new IllegalStateException("native " + field + " is null");
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

  private static String exactPositiveDecimal(final byte[] value, final String field) {
    final String decoded = exactUtf8(value, field);
    if (decoded.charAt(0) == '0') {
      throw new IllegalStateException("native " + field + " is not a positive canonical decimal");
    }
    for (int index = 0; index < decoded.length(); index++) {
      final char character = decoded.charAt(index);
      if (character < '0' || character > '9') {
        throw new IllegalStateException(
            "native " + field + " is not a positive canonical decimal");
      }
    }
    return decoded;
  }

  private static String lowerHex(final byte[] value) {
    final char[] digits = "0123456789abcdef".toCharArray();
    final char[] output = new char[value.length * 2];
    for (int index = 0; index < value.length; index++) {
      final int current = value[index] & 0xff;
      output[index * 2] = digits[current >>> 4];
      output[index * 2 + 1] = digits[current & 0x0f];
    }
    return new String(output);
  }

  private static boolean allZero(final byte[] value) {
    int aggregate = 0;
    for (final byte current : value) {
      aggregate |= current;
    }
    return aggregate == 0;
  }

  private static void requireNative() {
    NativeLoad.RESULT.getOrThrow();
  }

  private static final class NativeLoad {
    private static final NativeLoadResult RESULT = load();

    private static NativeLoadResult load() {
      try {
        System.loadLibrary("connect_norito_bridge");
        final int actual = nativeBridgeAbiVersion();
        if (actual != REQUIRED_BRIDGE_ABI_VERSION) {
          throw new IllegalStateException(
              "native authenticated query ABI mismatch: expected "
                  + REQUIRED_BRIDGE_ABI_VERSION
                  + ", found "
                  + actual);
        }
        return new NativeLoadResult(null);
      } catch (final UnsatisfiedLinkError | RuntimeException error) {
        return new NativeLoadResult(error);
      }
    }
  }

  private static final class NativeLoadResult {
    private final Throwable failure;

    private NativeLoadResult(final Throwable failure) {
      this.failure = failure;
    }

    private void getOrThrow() {
      if (failure != null) {
        throw new IllegalStateException(
            "native authenticated transaction-details bridge is unavailable", failure);
      }
    }
  }

  private static native int nativeBridgeAbiVersion();

  private static native byte[][] nativePrepareExactRejectedTransactionQueryV1(
      byte[] networkId,
      byte[] authorityAccountId,
      byte[] transactionHashHex,
      long creationTimeMs,
      byte[] nonce);

  private static native byte[] nativeFinalizeExactRejectedTransactionQueryV1(
      byte[] preparation, byte[] signature);

  private static native byte[][] nativeProjectExactCommittedRejectionV1(
      byte[] preparation, byte[] responseNorito);

  private static native byte[][] nativeProjectExactCommittedTransactionResultV1(
      byte[] preparation, byte[] responseNorito);

  private static native byte[] nativeProjectExactOfflineDeviceRegistrationResultV1(
      byte[] preparation, byte[] responseNorito);

  private static native byte[][] nativePrepareExactTransactionQueryV2(
      byte[] networkId,
      byte[] queryAuthorityAccountId,
      byte[] expectedTransactionAuthorityAccountId,
      byte[] transactionHashHex,
      long creationTimeMs,
      byte[] nonce);

  private static native byte[] nativeFinalizeExactTransactionQueryV2(
      byte[] preparation, byte[] signature);

  private static native byte[][] nativeProjectExactCommittedRejectionV2(
      byte[] preparation, byte[] responseNorito);

  private static native byte[][] nativeProjectExactKagemushaCommittedRejectionV2(
      byte[] preparation,
      byte[] responseNorito,
      byte[] expectedOperationId,
      byte[] expectedKind,
      byte[] expectedRequestNorito);

  private static native byte[][] nativeProjectExactCommittedTransactionResultV2(
      byte[] preparation, byte[] responseNorito);

  private static native byte[][] nativeBindFinalityProofPageV1(byte[][] finalityProofsNorito);

  private static native byte[] nativeVerifyFinalityPageV1(
      byte[] networkId,
      long trustedCheckpointHeight,
      byte[] trustedCheckpointContextId,
      byte[] finalityPageArchive);

  private static native byte[][] nativeProjectFinalizedKagemushaOutcomeV1(
      byte[] preparation,
      byte[] responseNorito,
      byte[] expectedOperationId,
      byte[] expectedKind,
      byte[] expectedRequestNorito,
      byte[] networkId,
      long trustedCheckpointHeight,
      byte[] trustedCheckpointContextId,
      byte[] finalityPageArchive,
      byte[] executedBlockWire);
}
