package org.hyperledger.iroha.android.offline;

import java.net.URI;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.LocalSigningContext;
import org.hyperledger.iroha.android.client.transport.TransportExecutor;

/** Public Java facade for exactly the four first-release Offline Cash V1 Torii routes. */
public final class OfflineCashToriiV1 {
  private OfflineCashToriiV1() {}

  /** Canonical signed top-up request retained for exact-byte idempotent retries. */
  public static final class TopUpRequestV1 {
    public static final int MAX_CANONICAL_BYTES =
        KagemushaRecursiveSpendProver.MAX_TORII_TOP_UP_REQUEST_BYTES_V4;

    private final byte[] canonical;

    public TopUpRequestV1(final byte[] canonicalNorito) {
      final KagemushaRecursiveSpendProver.TopUpRequest request =
          KagemushaRecursiveSpendProver.decodeTopUpRequest(canonicalNorito);
      KagemushaRecursiveSpendProver.projectTopUpSubmissionRequest(request);
      this.canonical = request.noritoEncoded();
    }

    public static TopUpRequestV1 decodeCanonical(final byte[] canonicalNorito) {
      return new TopUpRequestV1(canonicalNorito);
    }

    public byte[] encodeCanonical() {
      return Arrays.copyOf(canonical, canonical.length);
    }

    @Override
    public boolean equals(final Object other) {
      return this == other
          || other instanceof TopUpRequestV1
              && Arrays.equals(canonical, ((TopUpRequestV1) other).canonical);
    }

    @Override
    public int hashCode() {
      return Arrays.hashCode(canonical);
    }
  }

  /** Canonical signed redemption request retained for exact-byte idempotent retries. */
  public static final class RedeemRequestV1 {
    public static final int MAX_CANONICAL_BYTES =
        KagemushaRecursiveSpendProver.MAX_TORII_REDEEM_REQUEST_BYTES_V4;

    private final byte[] canonical;

    public RedeemRequestV1(final byte[] canonicalNorito) {
      final KagemushaRecursiveSpendProver.RedeemSubmissionRequest request =
          KagemushaRecursiveSpendProver.decodeRedeemSubmissionRequest(canonicalNorito);
      KagemushaRecursiveSpendProver.projectRedeemSubmissionRequest(request);
      this.canonical = request.noritoEncoded();
    }

    public static RedeemRequestV1 decodeCanonical(final byte[] canonicalNorito) {
      return new RedeemRequestV1(canonicalNorito);
    }

    public byte[] encodeCanonical() {
      return Arrays.copyOf(canonical, canonical.length);
    }

    @Override
    public boolean equals(final Object other) {
      return this == other
          || other instanceof RedeemRequestV1
              && Arrays.equals(canonical, ((RedeemRequestV1) other).canonical);
    }

    @Override
    public int hashCode() {
      return Arrays.hashCode(canonical);
    }
  }

  /** One canonical universal-readiness blocker. */
  public static final class ReadinessBlockerV1 {
    private final String code;
    private final String message;

    private ReadinessBlockerV1(final String code, final String message) {
      this.code = Objects.requireNonNull(code, "code");
      this.message = Objects.requireNonNull(message, "message");
    }

    public String code() {
      return code;
    }

    public String message() {
      return message;
    }

    @Override
    public boolean equals(final Object other) {
      return this == other
          || other instanceof ReadinessBlockerV1
              && code.equals(((ReadinessBlockerV1) other).code)
              && message.equals(((ReadinessBlockerV1) other).message);
    }

    @Override
    public int hashCode() {
      return 31 * code.hashCode() + message.hashCode();
    }
  }

  /** Strict asset-neutral Offline Cash V1 readiness response. */
  public static final class ReadinessV1 {
    private final boolean mandatory;
    private final String cashHandoffCapability;
    private final int requiredBridgeAbiVersion;
    private final int maximumHops;
    private final boolean ready;
    private final List<ReadinessBlockerV1> blockers;

    private ReadinessV1(final KagemushaRecursiveSpendProver.OfflineStatus status) {
      this.mandatory = status.mandatory();
      this.cashHandoffCapability = status.cashHandoffCapability();
      this.requiredBridgeAbiVersion = status.requiredBridgeAbiVersion();
      this.maximumHops = status.maximumHops();
      this.ready = status.ready();
      final List<ReadinessBlockerV1> projected = new ArrayList<>();
      for (final KagemushaRecursiveSpendProver.ReadinessBlocker blocker : status.blockers()) {
        projected.add(new ReadinessBlockerV1(blocker.code(), blocker.message()));
      }
      this.blockers = Collections.unmodifiableList(projected);
    }

    public boolean mandatory() {
      return mandatory;
    }

    public String cashHandoffCapability() {
      return cashHandoffCapability;
    }

    public int requiredBridgeAbiVersion() {
      return requiredBridgeAbiVersion;
    }

    public int maximumHops() {
      return maximumHops;
    }

    public boolean ready() {
      return ready;
    }

    public List<Object> assets() {
      return Collections.emptyList();
    }

    public List<ReadinessBlockerV1> blockers() {
      return blockers;
    }
  }

  /** Canonical accepted-operation reference. */
  public static final class OperationReferenceV1 {
    public static final int MAX_CANONICAL_BYTES =
        KagemushaRecursiveSpendProver.MAX_TORII_RESPONSE_BYTES;

    private final byte[] canonical;

    public OperationReferenceV1(final byte[] canonicalNorito) {
      final KagemushaRecursiveSpendProver.OperationReference reference =
          KagemushaRecursiveSpendProver.decodeOperationReference(canonicalNorito);
      KagemushaRecursiveSpendProver.projectOperationReference(reference);
      this.canonical = reference.noritoEncoded();
    }

    public static OperationReferenceV1 decodeCanonical(final byte[] canonicalNorito) {
      return new OperationReferenceV1(canonicalNorito);
    }

    public byte[] encodeCanonical() {
      return Arrays.copyOf(canonical, canonical.length);
    }

    @Override
    public boolean equals(final Object other) {
      return this == other
          || other instanceof OperationReferenceV1
              && Arrays.equals(canonical, ((OperationReferenceV1) other).canonical);
    }

    @Override
    public int hashCode() {
      return Arrays.hashCode(canonical);
    }
  }

  /** Pollable operation state. */
  public enum OperationStateV1 {
    PENDING,
    APPLIED,
    REJECTED
  }

  /** Offline operation kind. */
  public enum OperationKindV1 {
    TOP_UP,
    REDEEM
  }

  /** Stable terminal rejection. */
  public static final class OperationRejectionV1 {
    private static final String REJECTION_CODE = "offline_operation_rejected";
    private static final int MAX_MESSAGE_CODE_POINTS = 1024;

    private final String code;
    private final String message;

    private OperationRejectionV1(final String code, final String message) {
      if (!REJECTION_CODE.equals(code)) {
        throw new IllegalArgumentException(
            "rejectionCode must equal " + REJECTION_CODE);
      }
      this.code = code;
      this.message = requireCanonicalRejectionMessage(message);
    }

    public String code() {
      return code;
    }

    public String message() {
      return message;
    }
  }

  /** Safe applied-top-up view; anchor and proof internals remain opaque canonical bytes. */
  public static final class FinalizedTopUpV1 {
    private final byte[] anchorCanonical;
    private final byte[] finalityProofCanonical;
    private final long finalizedBlockHeight;
    private final long serverTimeMilliseconds;

    private FinalizedTopUpV1(
        final byte[] anchorCanonical,
        final byte[] finalityProofCanonical,
        final long finalizedBlockHeight,
        final long serverTimeMilliseconds) {
      this.anchorCanonical = Arrays.copyOf(anchorCanonical, anchorCanonical.length);
      this.finalityProofCanonical =
          Arrays.copyOf(finalityProofCanonical, finalityProofCanonical.length);
      this.finalizedBlockHeight = finalizedBlockHeight;
      this.serverTimeMilliseconds = serverTimeMilliseconds;
    }

    public byte[] anchorCanonical() {
      return Arrays.copyOf(anchorCanonical, anchorCanonical.length);
    }

    public byte[] finalityProofCanonical() {
      return Arrays.copyOf(finalityProofCanonical, finalityProofCanonical.length);
    }

    public long finalizedBlockHeight() {
      return finalizedBlockHeight;
    }

    public long serverTimeMilliseconds() {
      return serverTimeMilliseconds;
    }
  }

  /** Strict public projection of a decoded operation status. */
  public static final class OperationStatusProjectionV1 {
    private final OperationStateV1 state;
    private final OperationKindV1 kind;
    private final byte[] operationId;
    private final byte[] transactionHash;
    private final Long submittedAtMilliseconds;
    private final Long finalizedBlockHeight;
    private final Long serverTimeMilliseconds;
    private final FinalizedTopUpV1 finalizedTopUp;
    private final OperationRejectionV1 rejection;

    private OperationStatusProjectionV1(
        final OperationStateV1 state,
        final OperationKindV1 kind,
        final byte[] operationId,
        final byte[] transactionHash,
        final Long submittedAtMilliseconds,
        final Long finalizedBlockHeight,
        final Long serverTimeMilliseconds,
        final FinalizedTopUpV1 finalizedTopUp,
        final OperationRejectionV1 rejection) {
      this.state = Objects.requireNonNull(state, "state");
      this.kind = Objects.requireNonNull(kind, "kind");
      this.operationId = requireDigest(operationId, "operationId");
      this.transactionHash = requireDigest(transactionHash, "transactionHash");
      this.submittedAtMilliseconds = submittedAtMilliseconds;
      this.finalizedBlockHeight = finalizedBlockHeight;
      this.serverTimeMilliseconds = serverTimeMilliseconds;
      this.finalizedTopUp = finalizedTopUp;
      this.rejection = rejection;
      validateStateFields();
    }

    private void validateStateFields() {
      switch (state) {
        case PENDING:
          if (submittedAtMilliseconds == null
              || submittedAtMilliseconds.longValue() <= 0
              || finalizedBlockHeight != null
              || serverTimeMilliseconds != null
              || finalizedTopUp != null
              || rejection != null) {
            throw new IllegalArgumentException(
                "pending operation projection requires a positive submission time and no terminal fields");
          }
          break;
        case APPLIED:
          if (submittedAtMilliseconds != null
              || finalizedBlockHeight == null
              || finalizedBlockHeight.longValue() <= 0
              || serverTimeMilliseconds == null
              || serverTimeMilliseconds.longValue() <= 0
              || rejection != null
              || (kind == OperationKindV1.TOP_UP) != (finalizedTopUp != null)) {
            throw new IllegalArgumentException("applied operation projection is incomplete");
          }
          if (finalizedTopUp != null
              && (finalizedTopUp.finalizedBlockHeight() != finalizedBlockHeight.longValue()
                  || finalizedTopUp.serverTimeMilliseconds()
                      != serverTimeMilliseconds.longValue())) {
            throw new IllegalArgumentException(
                "finalized top-up projection changed its terminal height or time");
          }
          break;
        case REJECTED:
          if (submittedAtMilliseconds != null
              || finalizedBlockHeight != null
              || serverTimeMilliseconds != null
              || finalizedTopUp != null
              || rejection == null) {
            throw new IllegalArgumentException(
                "rejected operation projection must contain only its rejection");
          }
          break;
        default:
          throw new IllegalStateException("unknown Offline Cash operation state");
      }
    }

    public OperationStateV1 state() {
      return state;
    }

    public OperationKindV1 kind() {
      return kind;
    }

    public byte[] operationId() {
      return Arrays.copyOf(operationId, operationId.length);
    }

    public byte[] transactionHash() {
      return Arrays.copyOf(transactionHash, transactionHash.length);
    }

    public Long submittedAtMilliseconds() {
      return submittedAtMilliseconds;
    }

    public Long finalizedBlockHeight() {
      return finalizedBlockHeight;
    }

    public Long serverTimeMilliseconds() {
      return serverTimeMilliseconds;
    }

    public FinalizedTopUpV1 finalizedTopUp() {
      return finalizedTopUp;
    }

    public OperationRejectionV1 rejection() {
      return rejection;
    }
  }

  /** Canonical poll response with a native-backed public projection. */
  public static final class OperationStatusV1 {
    public static final int MAX_CANONICAL_BYTES =
        KagemushaRecursiveSpendProver.MAX_TORII_RESPONSE_BYTES;

    private final byte[] canonical;

    public OperationStatusV1(final byte[] canonicalNorito) {
      final KagemushaRecursiveSpendProver.OperationStatus status =
          KagemushaRecursiveSpendProver.decodeOperationStatus(canonicalNorito);
      KagemushaRecursiveSpendProver.projectOperationStatus(status);
      this.canonical = status.noritoEncoded();
    }

    public static OperationStatusV1 decodeCanonical(final byte[] canonicalNorito) {
      return new OperationStatusV1(canonicalNorito);
    }

    public byte[] encodeCanonical() {
      return Arrays.copyOf(canonical, canonical.length);
    }

    public OperationStatusProjectionV1 project() {
      return mapOperationProjection(
          KagemushaRecursiveSpendProver.projectOperationStatus(
              KagemushaRecursiveSpendProver.decodeOperationStatus(canonical)));
    }

    @Override
    public boolean equals(final Object other) {
      return this == other
          || other instanceof OperationStatusV1
              && Arrays.equals(canonical, ((OperationStatusV1) other).canonical);
    }

    @Override
    public int hashCode() {
      return Arrays.hashCode(canonical);
    }
  }

  /** Typed client bound to the caller-selected genesis network identity. */
  public static final class ClientV1 {
    public static final String READINESS_PATH =
        KagemushaRecursiveSpendProver.ToriiClient.READINESS_PATH;
    public static final String TOP_UP_PATH = KagemushaRecursiveSpendProver.ToriiClient.TOP_UP_PATH;
    public static final String REDEEM_PATH = KagemushaRecursiveSpendProver.ToriiClient.REDEEM_PATH;
    public static final String OPERATIONS_PATH =
        KagemushaRecursiveSpendProver.ToriiClient.OPERATIONS_PATH;
    public static final String JSON_MEDIA_TYPE =
        KagemushaRecursiveSpendProver.ToriiClient.JSON_MEDIA_TYPE;
    public static final String NORITO_MEDIA_TYPE =
        KagemushaRecursiveSpendProver.ToriiClient.NORITO_MEDIA_TYPE;

    private final KagemushaRecursiveSpendProver.ToriiClient delegate;

    private ClientV1(
        final URI baseUri,
        final TransportExecutor transport,
        final LocalSigningContext localSigningContext) {
      this.delegate =
          KagemushaRecursiveSpendProver.newToriiClient(
              baseUri,
              Objects.requireNonNull(transport, "transport"),
              Objects.requireNonNull(localSigningContext, "localSigningContext"));
    }

    /**
     * Creates a client that validates opaque signed request public bindings locally. Registered
     * device signature authenticity and authoritative time remain Torii admission decisions.
     */
    public static ClientV1 create(
        final URI baseUri,
        final TransportExecutor transport,
        final LocalSigningContext localSigningContext) {
      return new ClientV1(baseUri, transport, localSigningContext);
    }

    public CompletableFuture<ReadinessV1> getReadiness() {
      return delegate.getOfflineCapability().thenApply(ReadinessV1::new);
    }

    public CompletableFuture<OperationReferenceV1> submitTopUp(
        final TopUpRequestV1 request, final String operationId) {
      final TopUpRequestV1 requiredRequest = Objects.requireNonNull(request, "request");
      return delegate
          .submitTopUp(
              KagemushaRecursiveSpendProver.decodeTopUpRequest(
                  requiredRequest.encodeCanonical()),
              operationId)
          .thenApply(reference -> new OperationReferenceV1(reference.noritoEncoded()));
    }

    public CompletableFuture<OperationReferenceV1> submitRedeem(
        final RedeemRequestV1 request, final String operationId) {
      final RedeemRequestV1 requiredRequest = Objects.requireNonNull(request, "request");
      return delegate
          .submitRedeem(
              KagemushaRecursiveSpendProver.decodeRedeemSubmissionRequest(
                  requiredRequest.encodeCanonical()),
              operationId)
          .thenApply(reference -> new OperationReferenceV1(reference.noritoEncoded()));
    }

    public CompletableFuture<OperationStatusV1> getOperation(final String operationId) {
      return delegate
          .getOperation(operationId)
          .thenApply(status -> new OperationStatusV1(status.noritoEncoded()));
    }
  }

  private static OperationStatusProjectionV1 mapOperationProjection(
      final KagemushaRecursiveSpendProver.OperationStatusProjection projection) {
    final OperationStateV1 state;
    switch (projection.state()) {
      case PENDING:
        state = OperationStateV1.PENDING;
        break;
      case APPLIED:
        state = OperationStateV1.APPLIED;
        break;
      case REJECTED:
        state = OperationStateV1.REJECTED;
        break;
      default:
        throw new IllegalStateException("unknown native Offline Cash operation state");
    }
    final OperationKindV1 kind =
        projection.kind() == KagemushaRecursiveSpendProver.OperationKind.TOP_UP
            ? OperationKindV1.TOP_UP
            : OperationKindV1.REDEEM;
    final KagemushaRecursiveSpendProver.FinalizedTopUp nativeTopUp =
        projection.finalizedTopUp();
    final FinalizedTopUpV1 finalizedTopUp =
        nativeTopUp == null
            ? null
            : new FinalizedTopUpV1(
                nativeTopUp.anchor().noritoEncoded(),
                nativeTopUp.finalityProof().noritoEncoded(),
                nativeTopUp.finalizedBlockHeight(),
                nativeTopUp.serverTimeMilliseconds());
    final KagemushaRecursiveSpendProver.OperationRejection nativeRejection =
        projection.rejection();
    final OperationRejectionV1 rejection =
        nativeRejection == null
            ? null
            : new OperationRejectionV1(nativeRejection.code(), nativeRejection.message());
    return new OperationStatusProjectionV1(
        state,
        kind,
        projection.operationId(),
        projection.transactionHash(),
        projection.submittedAtMilliseconds(),
        projection.finalizedBlockHeight(),
        projection.serverTimeMilliseconds(),
        finalizedTopUp,
        rejection);
  }

  private static byte[] requireDigest(final byte[] value, final String field) {
    if (value == null || value.length != 32) {
      throw new IllegalArgumentException(field + " must contain exactly 32 bytes");
    }
    int accumulator = 0;
    for (final byte item : value) accumulator |= item;
    if (accumulator == 0) {
      throw new IllegalArgumentException(field + " must be non-zero");
    }
    return Arrays.copyOf(value, value.length);
  }

  private static String requireCanonicalRejectionMessage(final String value) {
    if (value == null || value.isEmpty() || hasBoundaryWhitespace(value)) {
      throw new IllegalArgumentException(
          "rejectionMessage must contain 1..1024 canonical Unicode scalars");
    }
    for (int offset = 0; offset < value.length(); ) {
      final int codePoint = value.codePointAt(offset);
      if (Character.isISOControl(codePoint)) {
        throw new IllegalArgumentException(
            "rejectionMessage must contain 1..1024 canonical Unicode scalars");
      }
      offset += Character.charCount(codePoint);
    }
    if (value.codePointCount(0, value.length()) > OperationRejectionV1.MAX_MESSAGE_CODE_POINTS) {
      throw new IllegalArgumentException(
          "rejectionMessage must contain 1..1024 canonical Unicode scalars");
    }
    return value;
  }

  private static boolean hasBoundaryWhitespace(final String value) {
    final int first = value.codePointAt(0);
    final int last = value.codePointBefore(value.length());
    return isUnicodeWhitespace(first) || isUnicodeWhitespace(last);
  }

  private static boolean isUnicodeWhitespace(final int codePoint) {
    return Character.isWhitespace(codePoint) || Character.isSpaceChar(codePoint);
  }
}
