package org.hyperledger.iroha.android.offline;

import java.net.URI;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.transport.TransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.client.ZkMerklePathResponse;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;

/**
 * ABI-20 Kagemusha V4 artifact streaming and capability bridge.
 *
 * <p>This is the sole first-release offline-cash surface. It authenticates the opaque ten-file proof
 * artifact set and validates exact typed request/payment/acknowledgement and proof-bound membership
 * archives. Proof execution remains fail-closed while the native backend reports unavailable.
 * V4 lifecycle results remain opaque until dedicated ABI-20 projections are available; no V2
 * result decoder is used as a substitute.
 */
public final class KagemushaRecursiveSpendProver {
  public static final int V4_REQUIRED_NATIVE_BRIDGE_ABI_VERSION = 20;
  public static final int REQUIRED_NATIVE_BRIDGE_ABI_VERSION = V4_REQUIRED_NATIVE_BRIDGE_ABI_VERSION;
  public static final String V4_ARTIFACT_MANIFEST_SCHEMA =
      "kagemusha.offline.recursive_spend.artifact_manifest.v4";
  public static final String ARTIFACT_MANIFEST_SCHEMA = V4_ARTIFACT_MANIFEST_SCHEMA;
  public static final List<String> V4_ARTIFACT_FILES =
      Collections.unmodifiableList(Arrays.asList(
          "step-eq.parameters.krv4",
          "step-eq.circuit-params.krv4",
          "step-eq.proving-key.krv4",
          "step-eq.verifying-key.krv4",
          "step-eq.bootstrap-witness.krv4",
          "step-ep.parameters.krv4",
          "step-ep.circuit-params.krv4",
          "step-ep.proving-key.krv4",
          "step-ep.verifying-key.krv4",
          "step-ep.bootstrap-witness.krv4"));
  public static final List<String> ARTIFACT_FILES = V4_ARTIFACT_FILES;
  public static final int V4_ARTIFACT_COUNT = 10;
  public static final int ARTIFACT_COUNT = V4_ARTIFACT_COUNT;
  public static final int MAX_MANIFEST_BYTES = 1024 * 1024;
  public static final int MAX_TRUSTED_RELEASE_POLICY_BYTES = 64 * 1024;
  public static final int MAX_RELEASE_ATTESTATION_BYTES = 1024 * 1024;
  public static final int MAX_RELEASE_EVIDENCE_BYTES = 16 * 1024 * 1024;
  public static final int MAX_PEER_TEXT_ENVELOPE_BYTES = 12 * 1024;
  public static final int MAX_PEER_TEXT_ARCHIVE_BYTES = 9_211;
  public static final int MAX_PEER_ARCHIVE_BYTES = 32 * 1024;
  public static final int MAX_LOCAL_REQUEST_ARCHIVE_BYTES = 8 * 1024 * 1024;
  public static final int MAX_LOCAL_RESULT_ARCHIVE_BYTES = 64 * 1024;
  public static final int MAX_TORII_REQUEST_BYTES = 512 * 1024;
  public static final int MAX_TORII_RESPONSE_BYTES = 4 * 1024 * 1024;
  public static final int MAXIMUM_INPUTS_PER_TRANSITION = 2;
  public static final int MAXIMUM_LOCAL_APPEND_BUILDER_INPUTS = MAXIMUM_INPUTS_PER_TRANSITION;
  public static final int MAXIMUM_BRANCH_CLAIMS = 2;
  public static final int MAXIMUM_PEER_HOPS = 8;
  public static final int CONFIDENTIAL_TREE_DEPTH = 16;

  private static final int EXACT_STATE_PROJECTION_VERSION = 1;

  private static final String LIBRARY_NAME = "connect_norito_bridge";
  private static final boolean ARTIFACT_BRIDGE_AVAILABLE = loadArtifactBridge();
  private static final boolean PROOF_BACKEND_AVAILABLE = loadProofBackendCapability();

  /** Canonical ABI-20 artifact roles. Declaration order is part of the native contract. */
  public enum ArtifactRoleV4 {
    STEP_EQ_PARAMETERS("step-eq.parameters.krv4"),
    STEP_EQ_CIRCUIT_PARAMS("step-eq.circuit-params.krv4"),
    STEP_EQ_PROVING_KEY("step-eq.proving-key.krv4"),
    STEP_EQ_VERIFYING_KEY("step-eq.verifying-key.krv4"),
    STEP_EQ_BOOTSTRAP_WITNESS("step-eq.bootstrap-witness.krv4"),
    STEP_EP_PARAMETERS("step-ep.parameters.krv4"),
    STEP_EP_CIRCUIT_PARAMS("step-ep.circuit-params.krv4"),
    STEP_EP_PROVING_KEY("step-ep.proving-key.krv4"),
    STEP_EP_VERIFYING_KEY("step-ep.verifying-key.krv4"),
    STEP_EP_BOOTSTRAP_WITNESS("step-ep.bootstrap-witness.krv4");

    private final String fileName;

    ArtifactRoleV4(final String fileName) {
      this.fileName = fileName;
    }

    public String fileName() {
      return fileName;
    }
  }

  private KagemushaRecursiveSpendProver() {}

  static void requireCanonicalV4ArtifactRoleInventory(final List<ArtifactRoleV4> roles) {
    Objects.requireNonNull(roles, "roles");
    final ArtifactRoleV4[] canonical = ArtifactRoleV4.values();
    if (roles.size() != canonical.length) {
      throw new IllegalArgumentException("artifact roles must contain exactly ten entries");
    }
    for (int index = 0; index < canonical.length; index++) {
      if (roles.get(index) != canonical[index]) {
        throw new IllegalArgumentException("artifact roles are not in canonical V4 order");
      }
    }
  }

  public static boolean isArtifactStreamingAvailable() {
    return ARTIFACT_BRIDGE_AVAILABLE;
  }

  public static boolean isProofBackendAvailable() {
    return PROOF_BACKEND_AVAILABLE;
  }

  public static ArtifactIngest beginArtifactIngest(
      final byte[] manifestNorito,
      final byte[] manifestSha256,
      final byte[] expectedArtifactSha256) {
    requireArtifactBridge();
    final byte[] manifest = requireManifest(manifestNorito);
    final byte[] manifestDigest = requireDigest(manifestSha256, "manifestSha256");
    final byte[] artifactDigest =
        requireDigest(expectedArtifactSha256, "expectedArtifactSha256");
    final long handle = nativeArtifactBeginV4(manifest, manifestDigest, artifactDigest);
    if (handle <= 0) {
      throw new IllegalStateException("native Kagemusha artifact ingest returned no handle");
    }
    return new ArtifactIngest(handle);
  }

  public static ArtifactInstallSession beginArtifactInstallSession(
      final byte[] manifestNorito,
      final byte[] manifestSha256,
      final ReleaseAuthentication releaseAuthentication) {
    requireArtifactBridge();
    return new ArtifactInstallSession(
        requireManifest(manifestNorito),
        requireDigest(manifestSha256, "manifestSha256"),
        Objects.requireNonNull(releaseAuthentication, "releaseAuthentication"));
  }

  public static RecipientPaymentRequest decodeRecipientPaymentRequest(final byte[] archive) {
    return new RecipientPaymentRequest(archive);
  }

  public static PeerPayment decodePeerPayment(final byte[] archive) {
    return new PeerPayment(archive);
  }

  public static ReceiverAcknowledgement decodeReceiverAcknowledgement(final byte[] archive) {
    return new ReceiverAcknowledgement(archive);
  }

  public static NoteMembershipWitness decodeNoteMembershipWitness(final byte[] archive) {
    return new NoteMembershipWitness(archive);
  }

  /** Restores the opaque note opening retained for a finalized top-up or staged output. */
  public static NoteOpening decodeNoteOpening(final byte[] archive) {
    return new NoteOpening(archive);
  }

  public static InitRequestV4 decodeInitRequestV4(final byte[] archive) {
    return new InitRequestV4(archive);
  }

  public static AppendRequestV4 decodeAppendRequestV4(
      final byte[] archive, final NoteOpening changeOpening) {
    return new AppendRequestV4(archive, changeOpening);
  }

  public static VerifyRequestV4 decodeVerifyRequestV4(final byte[] archive) {
    return new VerifyRequestV4(archive);
  }

  public static TopUpAnchorV4 decodeTopUpAnchorV4(final byte[] archive) {
    return new TopUpAnchorV4(archive);
  }

  public static BundleV4 decodeBundleV4(final byte[] archive) {
    return new BundleV4(archive);
  }

  public static TopUpFinalityEvidenceV4 decodeTopUpFinalityEvidenceV4(final byte[] archive) {
    return new TopUpFinalityEvidenceV4(archive);
  }

  /** Restore one secret-bearing V4 branch from independently persisted typed archives. */
  public static SpendableBranchV4 restoreSpendableBranchV4(
      final BundleV4 bundle,
      final NoteMembershipWitness membershipWitness,
      final NoteOpening opening) {
    return new SpendableBranchV4(
        Objects.requireNonNull(bundle, "bundle"),
        Objects.requireNonNull(membershipWitness, "membershipWitness"),
        Objects.requireNonNull(opening, "opening"));
  }

  public static RedeemRequestV4 decodeRedeemRequestV4(
      final byte[] archive, final NoteOpening changeOpening) {
    return new RedeemRequestV4(archive, changeOpening);
  }

  public static InitResultV4 decodeInitResultV4(final byte[] archive) {
    return new InitResultV4(archive);
  }

  public static SplitResultV4 decodeSplitResultV4(
      final byte[] archive, final NoteOpening changeOpening) {
    return new SplitResultV4(archive, changeOpening);
  }

  public static VerifyResultV4 decodeVerifyResultV4(final byte[] archive) {
    return new VerifyResultV4(archive);
  }

  public static RedeemBuildResultV4 decodeRedeemBuildResultV4(
      final byte[] archive, final NoteOpening changeOpening) {
    return new RedeemBuildResultV4(archive, changeOpening);
  }

  public static TopUpFinalityRosterArtifact decodeTopUpFinalityRosterArtifact(
      final byte[] archive) {
    return new TopUpFinalityRosterArtifact(archive);
  }

  /** Restores the exact canonical Torii request retained for an idempotent top-up retry. */
  public static TopUpRequest decodeTopUpRequest(final byte[] archive) {
    return new TopUpRequest(archive);
  }

  /** Restores the exact canonical Torii request retained for an idempotent redemption retry. */
  public static RedeemSubmissionRequest decodeRedeemSubmissionRequest(final byte[] archive) {
    return new RedeemSubmissionRequest(archive);
  }

  public static OperationStatusProjection projectOperationStatus(final OperationStatus status) {
    requireArtifactBridge();
    final byte[][] fields = nativeProjectOperationStatusV2(
        Objects.requireNonNull(status, "status").noritoEncoded());
    requireFieldCount(fields, 10, "operation status projection");
    final String stateText = canonicalText(fields[0], "operationState");
    final OperationState state;
    if ("pending".equals(stateText)) state = OperationState.PENDING;
    else if ("applied".equals(stateText)) state = OperationState.APPLIED;
    else if ("rejected".equals(stateText)) state = OperationState.REJECTED;
    else throw new IllegalStateException("native Kagemusha operation state is invalid");
    final String kindText = canonicalText(fields[1], "operationKind");
    final OperationKind kind;
    if ("top_up".equals(kindText)) kind = OperationKind.TOP_UP;
    else if ("redeem".equals(kindText)) kind = OperationKind.REDEEM;
    else throw new IllegalStateException("native Kagemusha operation kind is invalid");
    final Long heightOrSubmittedAt = fields[4].length == 0
        ? null : longInteger(fields[4], "operationHeightOrSubmittedAt");
    final Long serverTime = fields[5].length == 0
        ? null : longInteger(fields[5], "serverTimeMilliseconds");
    final FinalizedTopUp finalizedTopUp;
    if (fields[6].length != 0 || fields[7].length != 0) {
      if (state != OperationState.APPLIED || kind != OperationKind.TOP_UP
          || fields[6].length == 0 || fields[7].length == 0
          || heightOrSubmittedAt == null || serverTime == null) {
        throw new IllegalStateException("native Kagemusha finalized top-up fields are invalid");
      }
      finalizedTopUp = new FinalizedTopUp(
          new TopUpAnchor(fields[6]), new TopUpFinalityProof(fields[7]),
          heightOrSubmittedAt, serverTime);
    } else {
      finalizedTopUp = null;
    }
    final OperationRejection rejection;
    if (fields[8].length != 0 || fields[9].length != 0) {
      if (state != OperationState.REJECTED || fields[8].length == 0 || fields[9].length == 0) {
        throw new IllegalStateException("native Kagemusha rejection fields are invalid");
      }
      rejection = new OperationRejection(
          canonicalText(fields[8], "rejectionCode"),
          canonicalText(fields[9], "rejectionMessage"));
    } else {
      rejection = null;
    }
    return new OperationStatusProjection(
        state, kind, requireDigest(fields[2], "operationId"),
        requireDigest(fields[3], "transactionHash"),
        state == OperationState.PENDING ? heightOrSubmittedAt : null,
        state == OperationState.APPLIED ? heightOrSubmittedAt : null,
        serverTime, finalizedTopUp, rejection);
  }

  /** Decode the authoritative, snapshot-bound Torii Kagemusha capability response. */
  public static ReadinessProjection projectReadiness(final Readiness readiness) {
    requireArtifactBridge();
    final byte[][] fields =
        nativeProjectReadinessV2(Objects.requireNonNull(readiness, "readiness").noritoEncoded());
    if (fields == null || fields.length < 15) {
      throw new IllegalStateException(
          "native Kagemusha readiness projection returned invalid fields");
    }
    for (final byte[] field : fields) {
      if (field == null) {
        throw new IllegalStateException(
            "native Kagemusha readiness projection returned a null field");
      }
    }
    final int blockerCount = integer(fields[14], "blockerCount");
    if (blockerCount < 0 || fields.length != 15 + blockerCount * 2) {
      throw new IllegalStateException(
          "native Kagemusha readiness projection returned invalid blockers");
    }
    final java.util.ArrayList<ReadinessBlocker> blockers =
        new java.util.ArrayList<>(blockerCount);
    for (int index = 0; index < blockerCount; index++) {
      blockers.add(
          new ReadinessBlocker(
              canonicalText(fields[15 + index * 2], "blockerCode"),
              canonicalText(fields[16 + index * 2], "blockerMessage")));
    }
    return new ReadinessProjection(
        integer(fields[0], "requiredBridgeAbiVersion"),
        integer(fields[1], "maximumHops"),
        canonicalText(fields[2], "assetDefinitionId"),
        fields[3].length == 0 ? null : integer(fields[3], "assetScale"),
        longInteger(fields[4], "evaluatedBlockHeight"),
        requireDigest(fields[5], "evaluatedBlockHash"),
        bool(fields[6], "proofBackendAvailable"),
        bool(fields[7], "recursiveLineageSupported"),
        bool(fields[8], "ready"),
        activeVerifier(fields[9]),
        activeVerifier(fields[10]),
        activeVerifier(fields[11]),
        activeVerifier(fields[12]),
        activeVerifier(fields[13]),
        blockers);
  }

  public static RequestAuthorizationPreparation prepareRequestAuthorization(
      final String authority,
      final String deviceId,
      final byte[] operationId,
      final long issuedAtMilliseconds,
      final long expiresAtMilliseconds,
      final byte[] nonce,
      final byte[] payloadDigest,
      final byte[] appAttestEvidence) {
    requireArtifactBridge();
    final byte[][] fields = nativePrepareAuthorizationV2(
        utf8(authority, "authority"),
        utf8(deviceId, "deviceId"),
        requireDigest(operationId, "operationId"),
        issuedAtMilliseconds,
        expiresAtMilliseconds,
        requireDigest(nonce, "nonce"),
        requireDigest(payloadDigest, "payloadDigest"),
        appAttestEvidence == null ? new byte[0] : Arrays.copyOf(appAttestEvidence, appAttestEvidence.length));
    requireFieldCount(fields, 5, "authorization preparation");
    return new RequestAuthorizationPreparation(
        new RequestAuthorizationTemplate(fields[0]), fields[1], fields[2], fields[3],
        fields[4].length == 0 ? null : fields[4]);
  }

  public static RequestAuthorization signRequestAuthorization(
      final RequestAuthorizationPreparation preparation, final byte[] signature) {
    requireArtifactBridge();
    return new RequestAuthorization(nativeCreateAuthorizationV2(
        Objects.requireNonNull(preparation, "preparation").template.noritoEncoded(),
        copyRequired(signature, "signature")));
  }

  public static TopUpRequest finalizeTopUp(
      final TopUpUnsigned unsigned, final RequestAuthorization authorization) {
    requireArtifactBridge();
    return new TopUpRequest(nativeFinalizeTopUpV2(
        Objects.requireNonNull(unsigned, "unsigned").noritoEncoded(),
        Objects.requireNonNull(authorization, "authorization").noritoEncoded()));
  }

  public static TopUpRequest finalizeTopUp(
      final TopUpPreparation preparation, final RequestAuthorization authorization) {
    return finalizeTopUp(Objects.requireNonNull(preparation, "preparation").unsigned, authorization);
  }

  public static TopUpPreparation prepareTopUp(
      final String chainId,
      final String assetDefinitionId,
      final String payerAccountId,
      final KagemushaScaledAmount amount,
      final byte[] operationId,
      final byte[] openingSpendKey,
      final byte[] openingRho,
      final byte[] openingDiversifier,
      final TopUpZeroPath zeroPath,
      final byte[] shieldVerifierCommitment,
      final ArtifactBinding artifactBinding) {
    requireArtifactBridge();
    Objects.requireNonNull(amount, "amount");
    Objects.requireNonNull(zeroPath, "zeroPath");
    Objects.requireNonNull(artifactBinding, "artifactBinding");
    final byte[] spendKeyCopy = requireDigest(openingSpendKey, "openingSpendKey");
    final byte[] rhoCopy = requireDigest(openingRho, "openingRho");
    final byte[] diversifierCopy = requireDigest(openingDiversifier, "openingDiversifier");
    final byte[][] fields;
    try {
      fields = nativePrepareTopUpV2(
          utf8(chainId, "chainId"),
          utf8(assetDefinitionId, "assetDefinitionId"),
          utf8(payerAccountId, "payerAccountId"),
          utf8(amount.atomicUnits(), "atomicUnits"),
          amount.scale(),
          requireDigest(operationId, "operationId"),
          spendKeyCopy,
          rhoCopy,
          diversifierCopy,
          zeroPath.leafIndex,
          zeroPath.flattenedSiblings(),
          zeroPath.directions(),
          zeroPath.root(),
          requireDigest(shieldVerifierCommitment, "shieldVerifierCommitment"),
          artifactBinding.noritoEncoded());
    } finally {
      Arrays.fill(spendKeyCopy, (byte) 0);
      Arrays.fill(rhoCopy, (byte) 0);
      Arrays.fill(diversifierCopy, (byte) 0);
    }
    requireFieldCount(fields, 11, "top-up preparation");
    return new TopUpPreparation(
        new TopUpUnsigned(fields[0]), fields[1], new NoteOpening(fields[2]), fields[3], fields[4],
        fields[5], fields[6], fields[7], amount(fields[8], fields[9]),
        integer(fields[10], "leafIndex"));
  }

  public static RedeemFinalization finalizeRedeemV4(
      final RedeemBuildResultV4 buildResult, final RequestAuthorization authorization) {
    Objects.requireNonNull(buildResult, "buildResult");
    Objects.requireNonNull(authorization, "authorization");
    requireV4ProofBackend();
    throw new IllegalStateException("native Kagemusha V4 redeem finalization is unavailable");
  }

  public static RecipientRequestPreparation prepareRecipientPaymentRequest(
      final String chainId,
      final String assetDefinitionId,
      final KagemushaScaledAmount amount,
      final String recipientAccountId,
      final String receiverDeviceId,
      final KagemushaDevicePublicKeyV2 receiverPublicKey,
      final byte[] requestId,
      final long issuedAtMilliseconds,
      final long expiresAtMilliseconds,
      final byte[] spendKey,
      final byte[] rho,
      final byte[] diversifier) {
    requireArtifactBridge();
    Objects.requireNonNull(amount, "amount");
    final byte[] spendKeyCopy = requireDigest(spendKey, "spendKey");
    final byte[] rhoCopy = requireDigest(rho, "rho");
    final byte[] diversifierCopy = requireDigest(diversifier, "diversifier");
    final byte[][] fields;
    try {
      fields = nativePrepareRecipientRequestV2(
          utf8(chainId, "chainId"),
          utf8(assetDefinitionId, "assetDefinitionId"),
          utf8(amount.atomicUnits(), "atomicUnits"),
          amount.scale(),
          utf8(recipientAccountId, "recipientAccountId"),
          utf8(receiverDeviceId, "receiverDeviceId"),
          Objects.requireNonNull(receiverPublicKey, "receiverPublicKey").sec1Bytes(),
          requireDigest(requestId, "requestId"),
          issuedAtMilliseconds,
          expiresAtMilliseconds,
          spendKeyCopy,
          rhoCopy,
          diversifierCopy);
    } finally {
      Arrays.fill(spendKeyCopy, (byte) 0);
      Arrays.fill(rhoCopy, (byte) 0);
      Arrays.fill(diversifierCopy, (byte) 0);
    }
    requireFieldCount(fields, 5, "recipient request preparation");
    return new RecipientRequestPreparation(
        new RecipientRequestPayload(fields[0]),
        fields[1],
        new NoteOpening(fields[2]),
        fields[3],
        fields[4],
        amount);
  }

  public static RecipientPaymentRequest signRecipientPaymentRequest(
      final RecipientRequestPreparation preparation,
      final KagemushaDeviceSignatureV2 signature) {
    requireArtifactBridge();
    Objects.requireNonNull(preparation, "preparation");
    return new RecipientPaymentRequest(
        nativeCreateRecipientRequestV2(
            preparation.payload.noritoEncoded(),
            Objects.requireNonNull(signature, "signature").rawBytes()));
  }

  /** Prepare one local-only opening for sender change or partial redemption change. */
  public static NoteOpening prepareNoteOpening(
      final byte[] spendKey, final byte[] rho, final byte[] diversifier) {
    requireArtifactBridge();
    final byte[] spendKeyCopy = requireDigest(spendKey, "spendKey");
    final byte[] rhoCopy = requireDigest(rho, "rho");
    final byte[] diversifierCopy = requireDigest(diversifier, "diversifier");
    try {
      return new NoteOpening(
          nativePrepareNoteOpeningV2(spendKeyCopy, rhoCopy, diversifierCopy));
    } finally {
      Arrays.fill(spendKeyCopy, (byte) 0);
      Arrays.fill(rhoCopy, (byte) 0);
      Arrays.fill(diversifierCopy, (byte) 0);
    }
  }

  public static VerifiedRecipientPaymentRequest verifyRecipientPaymentRequest(
      final RecipientPaymentRequest request, final long verifiedAtMilliseconds) {
    requireArtifactBridge();
    Objects.requireNonNull(request, "request");
    if (verifiedAtMilliseconds <= 0) {
      throw new IllegalArgumentException("verifiedAtMilliseconds must be positive");
    }
    final RecipientRequestProjection projection = projectRecipientPaymentRequest(request);
    return new VerifiedRecipientPaymentRequest(
        request,
        requireDigest(
            nativeVerifyRecipientRequestV2(request.noritoEncoded(), verifiedAtMilliseconds),
            "requestDigest"),
        verifiedAtMilliseconds,
        projection);
  }

  public static RecipientRequestProjection projectRecipientPaymentRequest(
      final RecipientPaymentRequest request) {
    requireArtifactBridge();
    final byte[][] fields = nativeProjectRecipientRequestV2(
        Objects.requireNonNull(request, "request").noritoEncoded());
    requireFieldCount(fields, 14, "recipient request projection");
    return new RecipientRequestProjection(
        canonicalText(fields[0], "chainId"),
        canonicalText(fields[1], "assetDefinitionId"),
        amount(fields[2], fields[3]),
        canonicalText(fields[4], "recipientAccountId"),
        canonicalText(fields[5], "receiverDeviceId"),
        fields[6],
        longInteger(fields[7], "issuedAtMilliseconds"),
        longInteger(fields[8], "expiresAtMilliseconds"),
        fields[9], fields[10], fields[11], fields[12], fields[13]);
  }

  public static InitRequestV4 buildInitRequestV4(
      final TopUpAnchorV4 topUpAnchor,
      final TopUpFinalityProof topUpFinalityProof,
      final TopUpFinalityRosterArtifact topUpFinalityRosterArtifact,
      final NoteOpening opening,
      final OutputMembershipPaths outputMembershipPaths) {
    requireArtifactBridge();
    requireV4ProofBackend();
    final OutputMembershipPaths membership =
        Objects.requireNonNull(outputMembershipPaths, "outputMembershipPaths");
    if (membership.recipient() == null || membership.change() != null) {
      throw new IllegalArgumentException(
          "initialization requires exactly one recipient output path");
    }
    final byte[] openingArchive = Objects.requireNonNull(opening, "opening").noritoEncoded();
    final byte[] membershipArchive = membership.nativeArchive();
    try {
      return new InitRequestV4(nativeBuildInitRequestV4(
          Objects.requireNonNull(topUpAnchor, "topUpAnchor").noritoEncoded(),
          Objects.requireNonNull(topUpFinalityProof, "topUpFinalityProof").noritoEncoded(),
          Objects.requireNonNull(topUpFinalityRosterArtifact, "topUpFinalityRosterArtifact")
              .noritoEncoded(),
          openingArchive,
          membershipArchive));
    } finally {
      Arrays.fill(openingArchive, (byte) 0);
      Arrays.fill(membershipArchive, (byte) 0);
    }
  }

  /** Build one canonical append request from one or two independently spendable inputs. */
  public static AppendRequestV4 buildAppendRequestV4(
      final List<SpendableBranchV4> inputs,
      final NoteOpening changeOpening,
      final OutputMembershipPaths outputMembershipPaths,
      final byte[] transferVerifierCommitment,
      final byte[] operationId,
      final long blockHeight) {
    Objects.requireNonNull(inputs, "inputs");
    if (inputs.size() < 1 || inputs.size() > MAXIMUM_LOCAL_APPEND_BUILDER_INPUTS
        || inputs.stream().anyMatch(Objects::isNull)) {
      throw new IllegalArgumentException("inputs must contain one or two spendable branches");
    }
    for (int left = 0; left < inputs.size(); left++) {
      for (int right = left + 1; right < inputs.size(); right++) {
        if (inputs.get(left).bundle().equals(inputs.get(right).bundle())) {
          throw new IllegalArgumentException("inputs must refer to distinct V4 bundles");
        }
      }
    }
    final OutputMembershipPaths membership =
        Objects.requireNonNull(outputMembershipPaths, "outputMembershipPaths");
    if (membership.recipient() == null) {
      throw new IllegalArgumentException("append requires a recipient output path");
    }
    if ((membership.change() != null) != (changeOpening != null)) {
      throw new IllegalArgumentException(
          "change output membership must be present exactly when changeOpening is present");
    }
    requireArtifactBridge();
    requireV4ProofBackend();
    final byte[][] bundles = new byte[inputs.size()][];
    final byte[][] openings = new byte[inputs.size()][];
    final byte[][] witnesses = new byte[inputs.size()][];
    for (int index = 0; index < inputs.size(); index++) {
      final SpendableBranchV4 value = inputs.get(index);
      bundles[index] = value.bundle().noritoEncoded();
      openings[index] = value.opening().noritoEncoded();
      witnesses[index] = value.membershipWitness().noritoEncoded();
    }
    final byte[] change = changeOpening == null ? new byte[0] : changeOpening.noritoEncoded();
    final byte[] outputMembership = membership.nativeArchive();
    final byte[] verifier =
        requireDigest(transferVerifierCommitment, "transferVerifierCommitment");
    final byte[] operation = requireDigest(operationId, "operationId");
    final byte[] archive;
    try {
      archive = nativeBuildAppendRequestV4(
          bundles, openings, witnesses, change, outputMembership, verifier, operation,
          blockHeight);
    } finally {
      for (final byte[] value : bundles) Arrays.fill(value, (byte) 0);
      for (final byte[] value : openings) Arrays.fill(value, (byte) 0);
      for (final byte[] value : witnesses) Arrays.fill(value, (byte) 0);
      Arrays.fill(change, (byte) 0);
      Arrays.fill(outputMembership, (byte) 0);
      Arrays.fill(verifier, (byte) 0);
      Arrays.fill(operation, (byte) 0);
    }
    return new AppendRequestV4(archive, changeOpening);
  }

  public static PeerPaymentProjection projectPeerPayment(final PeerPayment payment) {
    requireArtifactBridge();
    Objects.requireNonNull(payment, "payment");
    final byte[][] fields = nativeProjectPeerPaymentV2(payment.noritoEncoded());
    final ProjectionCursor cursor = new ProjectionCursor(fields, "peer payment projection");
    requireProjectionVersion(cursor.next("version"), "peer payment projection");
    final byte[] operationId = requireDigest(cursor.next("operationId"), "operationId");
    final byte[] requestDigest = requireDigest(cursor.next("requestDigest"), "requestDigest");
    final BranchProjection projection = branchProjection(cursor);
    cursor.finish();
    final PeerPaymentProjection result =
        new PeerPaymentProjection(projection, operationId, requestDigest);
    Arrays.fill(operationId, (byte) 0);
    Arrays.fill(requestDigest, (byte) 0);
    return result;
  }

  public static VerifyRequestV4 buildVerifyRequestV4(
      final BundleV4 bundle,
      final RecipientPaymentRequest recipientRequest,
      final TopUpFinalityRosterArtifact topUpFinalityRosterArtifact,
      final List<TopUpFinalityEvidenceV4> topUpFinalityEvidence,
      final int maximumHops,
      final long blockHeight,
      final long verifiedAtMilliseconds) {
    requireArtifactBridge();
    requireV4ProofBackend();
    Objects.requireNonNull(topUpFinalityEvidence, "topUpFinalityEvidence");
    final byte[][] evidence = new byte[topUpFinalityEvidence.size()][];
    for (int index = 0; index < topUpFinalityEvidence.size(); index++) {
      evidence[index] = Objects.requireNonNull(
          topUpFinalityEvidence.get(index), "topUpFinalityEvidence[" + index + "]")
          .noritoEncoded();
    }
    try {
      return new VerifyRequestV4(nativeBuildVerifyRequestV4(
          Objects.requireNonNull(bundle, "bundle").noritoEncoded(),
          Objects.requireNonNull(recipientRequest, "recipientRequest").noritoEncoded(),
          Objects.requireNonNull(topUpFinalityRosterArtifact, "topUpFinalityRosterArtifact")
              .noritoEncoded(),
          evidence,
          maximumHops,
          blockHeight,
          verifiedAtMilliseconds));
    } finally {
      for (final byte[] value : evidence) Arrays.fill(value, (byte) 0);
    }
  }

  public static RedeemRequestV4 buildRedeemRequestV4(
      final SpendableBranchV4 input,
      final String recipientAccountId,
      final KagemushaScaledAmount amount,
      final NoteOpening changeOpening,
      final OutputMembershipPaths changeOutputMembershipPaths,
      final byte[] unshieldVerifierCommitment,
      final byte[] operationId,
      final long blockHeight) {
    requireArtifactBridge();
    requireV4ProofBackend();
    Objects.requireNonNull(input, "input");
    Objects.requireNonNull(amount, "amount");
    if ((changeOpening != null) != (changeOutputMembershipPaths != null)) {
      throw new IllegalArgumentException(
          "change output membership must be present exactly when changeOpening is present");
    }
    if (changeOutputMembershipPaths != null
        && (changeOutputMembershipPaths.recipient() != null
            || changeOutputMembershipPaths.change() == null)) {
      throw new IllegalArgumentException(
          "redemption change requires exactly one change output path");
    }
    final byte[] change = changeOpening == null ? new byte[0] : changeOpening.noritoEncoded();
    final byte[] outputMembership = changeOutputMembershipPaths == null
        ? new byte[0] : changeOutputMembershipPaths.nativeArchive();
    final byte[] verifier =
        requireDigest(unshieldVerifierCommitment, "unshieldVerifierCommitment");
    final byte[] operation = requireDigest(operationId, "operationId");
    final byte[] bundleArchive = input.bundle().noritoEncoded();
    final byte[] openingArchive = input.opening().noritoEncoded();
    final byte[] witnessArchive = input.membershipWitness().noritoEncoded();
    final byte[] recipient = utf8(recipientAccountId, "recipientAccountId");
    final byte[] atomicUnits = utf8(amount.atomicUnits(), "atomicUnits");
    try {
      return new RedeemRequestV4(nativeBuildRedeemRequestV4(
          bundleArchive, openingArchive, witnessArchive, recipient,
          atomicUnits, amount.scale(), change,
          outputMembership, verifier, operation, blockHeight), changeOpening);
    } finally {
      Arrays.fill(change, (byte) 0);
      Arrays.fill(outputMembership, (byte) 0);
      Arrays.fill(verifier, (byte) 0);
      Arrays.fill(operation, (byte) 0);
      Arrays.fill(bundleArchive, (byte) 0);
      Arrays.fill(openingArchive, (byte) 0);
      Arrays.fill(witnessArchive, (byte) 0);
      Arrays.fill(recipient, (byte) 0);
      Arrays.fill(atomicUnits, (byte) 0);
    }
  }

  public static AcknowledgementPreparation prepareAcknowledgement(
      final RecipientPaymentRequest request,
      final PeerPayment payment,
      final long acceptedAtMilliseconds) {
    requireArtifactBridge();
    final byte[][] fields = nativePrepareAcknowledgementV2(
        Objects.requireNonNull(request, "request").noritoEncoded(),
        Objects.requireNonNull(payment, "payment").noritoEncoded(),
        acceptedAtMilliseconds);
    requireFieldCount(fields, 6, "acknowledgement preparation");
    return new AcknowledgementPreparation(
        new AcknowledgementPayload(fields[0]), fields[1], fields[2], fields[3], fields[4], fields[5]);
  }

  public static ReceiverAcknowledgement signAcknowledgement(
      final AcknowledgementPreparation preparation,
      final KagemushaDeviceSignatureV2 signature,
      final RecipientPaymentRequest request,
      final PeerPayment payment) {
    requireArtifactBridge();
    return new ReceiverAcknowledgement(nativeCreateAcknowledgementV2(
        Objects.requireNonNull(preparation, "preparation").payload.noritoEncoded(),
        Objects.requireNonNull(signature, "signature").rawBytes(),
        Objects.requireNonNull(request, "request").noritoEncoded(),
        Objects.requireNonNull(payment, "payment").noritoEncoded()));
  }

  public static AcknowledgementVerification verifyAcknowledgement(
      final ReceiverAcknowledgement acknowledgement,
      final RecipientPaymentRequest request,
      final PeerPayment payment) {
    requireArtifactBridge();
    final byte[][] fields = nativeVerifyAcknowledgementV2(
        Objects.requireNonNull(acknowledgement, "acknowledgement").noritoEncoded(),
        Objects.requireNonNull(request, "request").noritoEncoded(),
        Objects.requireNonNull(payment, "payment").noritoEncoded());
    requireFieldCount(fields, 5, "acknowledgement verification");
    return new AcknowledgementVerification(
        bool(fields[0], "valid"), fields[1], fields[2], fields[3], fields[4]);
  }

  /** Build the first spendable branch from a finalized top-up anchor. */
  public static InitResultV4 initSpendV4(final InitRequestV4 request) {
    Objects.requireNonNull(request, "request");
    requireProofBackend();
    try {
      return new InitResultV4(
          requireNativeResult(nativeInitSpendV4(request.noritoEncoded()), "init spend"));
    } catch (final UnsatisfiedLinkError failure) {
      throw new IllegalStateException("native Kagemusha init spend entrypoint is unavailable", failure);
    }
  }

  /** Prove one exact recipient output and optional independently spendable sender change. */
  public static SplitResultV4 appendSpendV4(
      final AppendRequestV4 request,
      final RecipientPaymentRequest recipientRequest,
      final long verifiedAtMilliseconds) {
    if (verifiedAtMilliseconds <= 0) {
      throw new IllegalArgumentException("verifiedAtMilliseconds must be positive");
    }
    Objects.requireNonNull(request, "request");
    Objects.requireNonNull(recipientRequest, "recipientRequest");
    requireProofBackend();
    final NoteOpening changeOpening = request.changeOpening;
    final byte[] secretArchive = request.consumeAndDestroy();
    try {
      return new SplitResultV4(
          requireNativeResult(
              nativeAppendSpendV4(
                  secretArchive,
                  recipientRequest.noritoEncoded(),
                  verifiedAtMilliseconds),
              "append spend"),
          changeOpening);
    } catch (final UnsatisfiedLinkError failure) {
      throw new IllegalStateException("native Kagemusha append spend entrypoint is unavailable", failure);
    } finally {
      Arrays.fill(secretArchive, (byte) 0);
    }
  }

  /** Verify the recursive proof, exact split bindings, membership, and hop limit. */
  public static VerifyResultV4 verifySpendV4(final VerifyRequestV4 request) {
    Objects.requireNonNull(request, "request");
    requireProofBackend();
    try {
      return new VerifyResultV4(
          requireNativeResult(
              nativeVerifySpendV4(request.noritoEncoded()),
              "verify spend"));
    } catch (final UnsatisfiedLinkError failure) {
      throw new IllegalStateException("native Kagemusha verify spend entrypoint is unavailable", failure);
    }
  }

  /** Build a full or partial redemption and its optional proof-bound offline change. */
  public static RedeemBuildResultV4 buildRedeemV4(final RedeemRequestV4 request) {
    Objects.requireNonNull(request, "request");
    requireProofBackend();
    final NoteOpening changeOpening = request.changeOpening;
    final byte[] secretArchive = request.consumeAndDestroy();
    try {
      return new RedeemBuildResultV4(
          requireNativeResult(
              nativeBuildRedeemV4(secretArchive),
              "build redeem"),
          changeOpening);
    } catch (final UnsatisfiedLinkError failure) {
      throw new IllegalStateException("native Kagemusha build redeem entrypoint is unavailable", failure);
    } finally {
      Arrays.fill(secretArchive, (byte) 0);
    }
  }

  public static ToriiClient newToriiClient(
      final URI baseUri, final TransportExecutor transport) {
    return new ToriiClient(baseUri, transport);
  }

  static boolean isExactBridgeAbi(final int abiVersion) {
    return abiVersion == REQUIRED_NATIVE_BRIDGE_ABI_VERSION;
  }

  static boolean detectExactNativeAvailability(
      final NativeProbe loadLibrary,
      final NativeAbiVersionProbe abiVersion,
      final NativeSymbolProbe symbolProbe) {
    Objects.requireNonNull(loadLibrary, "loadLibrary");
    Objects.requireNonNull(abiVersion, "abiVersion");
    Objects.requireNonNull(symbolProbe, "symbolProbe");
    try {
      loadLibrary.run();
      return isExactBridgeAbi(abiVersion.run()) && symbolProbe.run();
    } catch (final UnsatisfiedLinkError | RuntimeException error) {
      return false;
    }
  }

  private static boolean loadArtifactBridge() {
    return detectExactNativeAvailability(
        () -> System.loadLibrary(LIBRARY_NAME),
        KagemushaRecursiveSpendProver::nativeBridgeAbiVersion,
        () ->
            expectIllegalArgumentProbe(
                () -> nativeArtifactBeginV4(new byte[] {0}, new byte[32], new byte[32])));
  }

  private static boolean loadProofBackendCapability() {
    return detectExactNativeAvailability(
        () -> System.loadLibrary(LIBRARY_NAME),
        KagemushaRecursiveSpendProver::nativeBridgeAbiVersion,
        KagemushaRecursiveSpendProver::nativePastaCycleV4BackendAvailable);
  }

  private static boolean expectIllegalArgumentProbe(final NativeProbe probe) {
    try {
      probe.run();
      return false;
    } catch (final IllegalArgumentException expected) {
      return true;
    }
  }

  private static void requireArtifactBridge() {
    requireV4ArtifactBridge();
  }

  private static void requireV4ArtifactBridge() {
    if (!ARTIFACT_BRIDGE_AVAILABLE) {
      throw new IllegalStateException(
          LIBRARY_NAME + " ABI " + REQUIRED_NATIVE_BRIDGE_ABI_VERSION
              + " artifact streaming is unavailable");
    }
  }

  private static void requireProofBackend() {
    requireV4ProofBackend();
  }

  private static void requireV4ProofBackend() {
    if (!PROOF_BACKEND_AVAILABLE) {
      throw new IllegalStateException(
          LIBRARY_NAME + " ABI " + REQUIRED_NATIVE_BRIDGE_ABI_VERSION
              + " Kagemusha proof backend is unavailable");
    }
  }

  private static byte[] requireNativeResult(final byte[] result, final String label) {
    if (result == null || result.length == 0) {
      throw new IllegalStateException("native Kagemusha " + label + " returned no archive");
    }
    return result;
  }

  private static byte[] utf8(final String value, final String field) {
    if (value == null || value.isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException(field + " must be canonical non-empty text");
    }
    return value.getBytes(StandardCharsets.UTF_8);
  }

  private static byte[] copyRequired(final byte[] value, final String field) {
    if (value == null || value.length == 0) {
      throw new IllegalArgumentException(field + " must not be empty");
    }
    return Arrays.copyOf(value, value.length);
  }

  private static void requireFieldCount(
      final byte[][] fields, final int expected, final String label) {
    if (fields == null || fields.length != expected) {
      throw new IllegalStateException("native Kagemusha " + label + " returned invalid fields");
    }
    for (final byte[] field : fields) {
      if (field == null) {
        throw new IllegalStateException("native Kagemusha " + label + " returned a null field");
      }
    }
  }

  private static KagemushaScaledAmount amount(final byte[] atomic, final byte[] scale) {
    return KagemushaScaledAmount.fromAtomicUnits(
        new String(atomic, StandardCharsets.US_ASCII), integer(scale, "scale"));
  }

  private static int integer(final byte[] value, final String field) {
    try {
      return Integer.parseInt(new String(value, StandardCharsets.US_ASCII));
    } catch (final RuntimeException failure) {
      throw new IllegalStateException("native Kagemusha " + field + " is invalid", failure);
    }
  }

  private static long longInteger(final byte[] value, final String field) {
    try {
      return Long.parseLong(new String(value, StandardCharsets.US_ASCII));
    } catch (final RuntimeException failure) {
      throw new IllegalStateException("native Kagemusha " + field + " is invalid", failure);
    }
  }

  private static String canonicalText(final byte[] value, final String field) {
    final String text = new String(value, StandardCharsets.UTF_8);
    if (text.isEmpty() || !text.equals(text.trim())) {
      throw new IllegalStateException("native Kagemusha " + field + " is invalid");
    }
    for (int index = 0; index < text.length(); index++) {
      if (Character.isISOControl(text.charAt(index))) {
        throw new IllegalStateException("native Kagemusha " + field + " is invalid");
      }
    }
    return text;
  }

  private static ActiveVerifier activeVerifier(final byte[] archive) {
    if (archive.length == 0) {
      return null;
    }
    final byte[][] fields = nativeProjectActiveVerifierV2(archive);
    requireFieldCount(fields, 9, "active verifier projection");
    return new ActiveVerifier(
        canonicalText(fields[0], "verifierBackend"),
        canonicalText(fields[1], "verifierName"),
        integer(fields[2], "verifierVersion"),
        canonicalText(fields[3], "verifierCircuitId"),
        requireDigest(fields[4], "verifierCommitment"),
        requireDigest(fields[5], "publicInputsSchemaHash"),
        integer(fields[6], "maximumProofBytes"),
        longInteger(fields[7], "activationHeight"),
        fields[8].length == 0 ? null : longInteger(fields[8], "withdrawalHeight"));
  }

  private static boolean bool(final byte[] value, final String field) {
    if (value.length != 1 || (value[0] != 0 && value[0] != 1)) {
      throw new IllegalStateException("native Kagemusha " + field + " is invalid");
    }
    return value[0] == 1;
  }

  private static void requireProjectionVersion(final byte[] value, final String field) {
    if (value.length != 4
        || value[0] != 0
        || value[1] != 0
        || value[2] != 0
        || (value[3] & 0xff) != EXACT_STATE_PROJECTION_VERSION) {
      throw new IllegalStateException("native Kagemusha " + field + " version is unsupported");
    }
  }

  private static int projectionCount(final byte[] value, final String field) {
    if (value.length != 4) {
      throw new IllegalStateException("native Kagemusha " + field + " count is invalid");
    }
    long count = 0;
    for (final byte octet : value) {
      count = (count << 8) | (octet & 0xffL);
    }
    if (count < 1 || count > MAXIMUM_BRANCH_CLAIMS) {
      throw new IllegalStateException(
          "native Kagemusha " + field + " count is outside the exact-state limit");
    }
    return (int) count;
  }

  private static final class ProjectionCursor {
    private final byte[][] fields;
    private final String label;
    private int index;

    private ProjectionCursor(final byte[][] fields, final String label) {
      this.fields = Objects.requireNonNull(fields, "fields");
      this.label = label;
    }

    private byte[] next(final String field) {
      if (index >= fields.length) {
        throw new IllegalStateException("native Kagemusha " + label + " omitted " + field);
      }
      return fields[index++];
    }

    private void finish() {
      if (index != fields.length) {
        throw new IllegalStateException("native Kagemusha " + label + " has trailing fields");
      }
    }
  }

  private static BranchProjection branchProjection(final ProjectionCursor cursor) {
    final Bundle bundle = new Bundle(cursor.next("bundle"));
    final NoteMembershipWitness witness =
        new NoteMembershipWitness(cursor.next("membershipWitness"));
    final byte[] commitment = cursor.next("commitment");
    final byte[] spendNullifier = cursor.next("spendNullifier");
    final KagemushaScaledAmount amount =
        amount(cursor.next("atomicUnits"), cursor.next("scale"));
    final int hopCount = integer(cursor.next("hopCount"), "hopCount");
    final int proofStepCount = integer(cursor.next("proofStepCount"), "proofStepCount");
    final byte[] bundleDigest = cursor.next("bundleDigest");
    final ArtifactBinding artifactBinding = new ArtifactBinding(cursor.next("artifactBinding"));
    final int claimCount = projectionCount(cursor.next("branchClaimCount"), "branchClaim");
    final List<BranchClaim> claims = new ArrayList<>(claimCount);
    for (int index = 0; index < claimCount; index++) {
      claims.add(new BranchClaim(cursor.next("branchClaim[" + index + "]")));
    }
    return new BranchProjection(
        bundle, witness, commitment, spendNullifier, amount, hopCount, proofStepCount,
        bundleDigest, artifactBinding, claims);
  }

  private static byte[] requireManifest(final byte[] value) {
    if (value == null || value.length == 0 || value.length > MAX_MANIFEST_BYTES) {
      throw new IllegalArgumentException(
          "manifestNorito must contain 1.." + MAX_MANIFEST_BYTES + " bytes");
    }
    return Arrays.copyOf(value, value.length);
  }

  private static byte[] requireDigest(final byte[] value, final String name) {
    if (value == null || value.length != 32) {
      throw new IllegalArgumentException(name + " must contain exactly 32 bytes");
    }
    int accumulator = 0;
    for (final byte octet : value) {
      accumulator |= octet;
    }
    if (accumulator == 0) {
      throw new IllegalArgumentException(name + " must be non-zero");
    }
    return Arrays.copyOf(value, value.length);
  }

  private static byte[] requireBoundedBytes(
      final byte[] value, final String name, final int maximumBytes) {
    if (value == null || value.length == 0 || value.length > maximumBytes) {
      throw new IllegalArgumentException(
          name + " must contain 1.." + maximumBytes + " bytes");
    }
    return Arrays.copyOf(value, value.length);
  }

  private static byte[] requireChunk(final byte[] value) {
    if (value == null || value.length == 0) {
      throw new IllegalArgumentException("chunk must not be empty");
    }
    return Arrays.copyOf(value, value.length);
  }

  private static byte[] requireCanonicalArchive(
      final byte[] value, final String schema, final String field, final int maximumBytes) {
    if (value == null || value.length == 0 || value.length > maximumBytes) {
      throw new IllegalArgumentException(
          field + " must contain 1.." + maximumBytes + " bytes");
    }
    final byte[] archive = Arrays.copyOf(value, value.length);
    final NoritoHeader.DecodeResult decoded;
    try {
      decoded = NoritoHeader.decode(archive, SchemaHash.hash16(schema));
    } catch (final RuntimeException failure) {
      throw new IllegalArgumentException(field + " must contain canonical " + schema, failure);
    }
    final NoritoHeader header = decoded.header();
    if (header.compression() != NoritoHeader.COMPRESSION_NONE
        || header.flags() != NoritoHeader.COMPACT_LEN
        || decoded.payload().length == 0
        || archive.length != NoritoHeader.HEADER_LENGTH + decoded.payload().length
        || !Arrays.equals(
            header.encode(), Arrays.copyOfRange(archive, 0, NoritoHeader.HEADER_LENGTH))) {
      throw new IllegalArgumentException(field + " must use canonical compact Norito framing");
    }
    header.validateChecksum(decoded.payload());
    return archive;
  }

  /** Immutable canonical Norito archive; proof and accumulator bytes remain opaque. */
  public abstract static class CanonicalArchive {
    private final byte[] archive;
    private boolean destroyed;

    private CanonicalArchive(
        final byte[] archive, final String schema, final String field, final int maximumBytes) {
      this.archive = requireCanonicalArchive(archive, schema, field, maximumBytes);
    }

    public final synchronized byte[] noritoEncoded() {
      if (destroyed) {
        throw new IllegalStateException("canonical archive has been destroyed");
      }
      return Arrays.copyOf(archive, archive.length);
    }

    protected final synchronized byte[] consumeAndDestroy() {
      if (destroyed) {
        throw new IllegalStateException("canonical archive has already been consumed");
      }
      final byte[] consumed = Arrays.copyOf(archive, archive.length);
      Arrays.fill(archive, (byte) 0);
      destroyed = true;
      return consumed;
    }

    protected final synchronized void destroy() {
      Arrays.fill(archive, (byte) 0);
      destroyed = true;
    }

    public final synchronized boolean isDestroyed() {
      return destroyed;
    }

    @Override
    public final boolean equals(final Object other) {
      return other != null
          && getClass() == other.getClass()
          && Arrays.equals(archive, ((CanonicalArchive) other).archive);
    }

    @Override
    public final int hashCode() {
      return Arrays.hashCode(archive);
    }
  }

  public static final class RecipientPaymentRequest extends CanonicalArchive {
    private RecipientPaymentRequest(final byte[] archive) {
      super(
          archive,
          "KagemushaRecipientPaymentRequestV2",
          "recipientPaymentRequest",
          MAX_PEER_ARCHIVE_BYTES);
    }
  }

  public static final class PeerPayment extends CanonicalArchive {
    private PeerPayment(final byte[] archive) {
      super(
          archive,
          "KagemushaRecursiveSpendPeerPaymentV2",
          "peerPayment",
          MAX_PEER_ARCHIVE_BYTES);
    }
  }

  public static final class ReceiverAcknowledgement extends CanonicalArchive {
    private ReceiverAcknowledgement(final byte[] archive) {
      super(
          archive,
          "KagemushaReceiverAcknowledgementV2",
          "receiverAcknowledgement",
          MAX_PEER_ARCHIVE_BYTES);
    }
  }

  /** Proof-bound output membership state carried atomically with an accepted branch. */
  public static final class NoteMembershipWitness extends CanonicalArchive {
    private NoteMembershipWitness(final byte[] archive) {
      super(
          archive,
          "KagemushaNoteMembershipWitnessV2",
          "noteMembershipWitness",
          MAX_PEER_ARCHIVE_BYTES);
    }
  }

  /** Encrypted local note opening; never send this archive to Torii or a peer. */
  public static final class NoteOpening extends CanonicalArchive {
    private NoteOpening(final byte[] archive) {
      super(archive, "KagemushaNoteOpeningV2", "noteOpening", MAX_LOCAL_REQUEST_ARCHIVE_BYTES);
    }
  }

  public static final class RecipientRequestPayload extends CanonicalArchive {
    private RecipientRequestPayload(final byte[] archive) {
      super(
          archive,
          "KagemushaRecipientPaymentRequestSigningPayloadV2",
          "recipientRequestPayload",
          MAX_PEER_ARCHIVE_BYTES);
    }
  }

  public static final class Bundle extends CanonicalArchive {
    private Bundle(final byte[] archive) {
      super(
          archive,
          "KagemushaRecursiveSpendBundleV2",
          "bundle",
          MAX_LOCAL_REQUEST_ARCHIVE_BYTES);
    }
  }

  /** Opaque ABI-20 recursive state; it is never decoded as a V2/V3 bundle. */
  public static final class BundleV4 extends CanonicalArchive {
    private BundleV4(final byte[] archive) {
      super(
          archive,
          "KagemushaRecursiveSpendBundleV4",
          "bundleV4",
          MAX_LOCAL_REQUEST_ARCHIVE_BYTES);
    }
  }

  /** Opaque current lineage claim; native comparison implements all overlap rules. */
  public static final class BranchClaim extends CanonicalArchive {
    private BranchClaim(final byte[] archive) {
      super(
          archive,
          "KagemushaRecursiveSpendBranchClaimV2",
          "branchClaim",
          MAX_PEER_ARCHIVE_BYTES);
    }

    public boolean conflictsWith(final BranchClaim other) {
      requireArtifactBridge();
      return nativeBranchClaimsConflictV2(
          noritoEncoded(), Objects.requireNonNull(other, "other").noritoEncoded());
    }
  }

  public static final class ArtifactBinding extends CanonicalArchive {
    private ArtifactBinding(final byte[] archive) {
      super(archive, "KagemushaRecursiveSpendArtifactBindingV3", "artifactBinding", MAX_MANIFEST_BYTES);
    }
  }

  public static final class TopUpUnsigned extends CanonicalArchive {
    private TopUpUnsigned(final byte[] archive) {
      super(archive, "KagemushaRecursiveSpendTopUpUnsignedV2", "topUpUnsigned", MAX_TORII_REQUEST_BYTES);
    }
  }

  public static final class TopUpRequest extends CanonicalArchive {
    TopUpRequest(final byte[] archive) {
      super(archive, "iroha.torii.v1.offline.top_up.request", "topUpRequest", MAX_TORII_REQUEST_BYTES);
    }
  }

  public static final class TopUpAnchor extends CanonicalArchive {
    private TopUpAnchor(final byte[] archive) {
      super(archive, "KagemushaRecursiveSpendTopUpAnchorV2", "topUpAnchor", MAX_TORII_RESPONSE_BYTES);
    }
  }

  /** Finalized ABI-20 top-up receipt with a V4 artifact binding. */
  public static final class TopUpAnchorV4 extends CanonicalArchive {
    private TopUpAnchorV4(final byte[] archive) {
      super(
          archive,
          "KagemushaRecursiveSpendTopUpAnchorV4",
          "topUpAnchorV4",
          MAX_TORII_RESPONSE_BYTES);
    }
  }

  public static final class TopUpFinalityProof extends CanonicalArchive {
    private TopUpFinalityProof(final byte[] archive) {
      super(archive, "KagemushaTopUpFinalityProofV2", "topUpFinalityProof", MAX_TORII_RESPONSE_BYTES);
    }
  }

  public static final class TopUpFinalityRosterArtifact extends CanonicalArchive {
    private TopUpFinalityRosterArtifact(final byte[] archive) {
      super(
          archive,
          "KagemushaTopUpFinalityRosterArtifactV2",
          "topUpFinalityRosterArtifact",
          MAX_TORII_RESPONSE_BYTES);
    }
  }

  /** Complete V4 origin plus its stable compact-finality proof. */
  public static final class TopUpFinalityEvidenceV4 extends CanonicalArchive {
    private TopUpFinalityEvidenceV4(final byte[] archive) {
      super(
          archive,
          "KagemushaRecursiveSpendTopUpFinalityEvidenceV4",
          "topUpFinalityEvidenceV4",
          MAX_TORII_RESPONSE_BYTES);
    }
  }

  public static final class RedeemSubmissionRequest extends CanonicalArchive {
    RedeemSubmissionRequest(final byte[] archive) {
      super(archive, "iroha.torii.v1.offline.redeem.request", "redeemSubmissionRequest", MAX_TORII_REQUEST_BYTES);
    }
  }

  public static final class RequestAuthorizationTemplate extends CanonicalArchive {
    private RequestAuthorizationTemplate(final byte[] archive) {
      super(archive, "KagemushaRequestAuthorizationV2", "requestAuthorizationTemplate", MAX_TORII_REQUEST_BYTES);
    }
  }

  public static final class RequestAuthorization extends CanonicalArchive {
    private RequestAuthorization(final byte[] archive) {
      super(archive, "KagemushaRequestAuthorizationV2", "requestAuthorization", MAX_TORII_REQUEST_BYTES);
    }
  }

  public static final class TopUpZeroPath {
    private final int leafIndex;
    private final List<byte[]> siblings;
    private final byte[] directions;
    private final byte[] root;

    public TopUpZeroPath(
        final int leafIndex,
        final List<byte[]> siblings,
        final byte[] directions,
        final byte[] root) {
      if (leafIndex < 0 || leafIndex >= (1 << CONFIDENTIAL_TREE_DEPTH)) {
        throw new IllegalArgumentException("leafIndex is outside the confidential tree");
      }
      if (siblings == null || siblings.size() != CONFIDENTIAL_TREE_DEPTH) {
        throw new IllegalArgumentException(
            "siblings must contain exactly " + CONFIDENTIAL_TREE_DEPTH + " 32-byte nodes");
      }
      final java.util.ArrayList<byte[]> siblingCopies =
          new java.util.ArrayList<>(CONFIDENTIAL_TREE_DEPTH);
      for (final byte[] sibling : siblings) {
        if (sibling == null || sibling.length != 32) {
          throw new IllegalArgumentException(
              "siblings must contain exactly " + CONFIDENTIAL_TREE_DEPTH + " 32-byte nodes");
        }
        siblingCopies.add(Arrays.copyOf(sibling, sibling.length));
      }
      if (directions == null || directions.length != CONFIDENTIAL_TREE_DEPTH) {
        throw new IllegalArgumentException(
            "directions must contain exactly " + CONFIDENTIAL_TREE_DEPTH + " binary values");
      }
      int encodedLeaf = 0;
      for (int level = 0; level < directions.length; level++) {
        if (directions[level] != 0 && directions[level] != 1) {
          throw new IllegalArgumentException("directions must be binary values");
        }
        encodedLeaf |= directions[level] << level;
      }
      if (encodedLeaf != leafIndex) {
        throw new IllegalArgumentException("directions do not encode leafIndex");
      }
      this.leafIndex = leafIndex;
      this.siblings = Collections.unmodifiableList(siblingCopies);
      this.directions = Arrays.copyOf(directions, directions.length);
      this.root = requireDigest(root, "root");
    }

    public int leafIndex() { return leafIndex; }
    public List<byte[]> siblings() {
      final java.util.ArrayList<byte[]> copies = new java.util.ArrayList<>(siblings.size());
      for (final byte[] sibling : siblings) copies.add(Arrays.copyOf(sibling, sibling.length));
      return Collections.unmodifiableList(copies);
    }
    public byte[] directions() { return Arrays.copyOf(directions, directions.length); }
    public byte[] root() { return Arrays.copyOf(root, root.length); }
    private byte[] flattenedSiblings() {
      final byte[] flattened = new byte[CONFIDENTIAL_TREE_DEPTH * 32];
      for (int index = 0; index < siblings.size(); index++) {
        System.arraycopy(siblings.get(index), 0, flattened, index * 32, 32);
      }
      return flattened;
    }

    /** Convert only Torii's authoritative next-zero path; ordinary inclusion paths fail. */
    public static TopUpZeroPath from(final ZkMerklePathResponse response) {
      Objects.requireNonNull(response, "response");
      if (response.treeDepth() != CONFIDENTIAL_TREE_DEPTH) {
        throw new IllegalArgumentException(
            "Torii confidential tree depth does not match Kagemusha");
      }
      final ZkMerklePathResponse.Entry path = response.requireNextZeroPath();
      return new TopUpZeroPath(
          path.leafIndex(), path.siblingBytes(), path.directions(), path.rootBytes());
    }
  }

  /** One authenticated confidential-tree path used by the V4 output-update witness. */
  public static final class OutputMembershipPath {
    private final int leafIndex;
    private final List<byte[]> siblings;
    private final byte[] directions;
    private final byte[] root;

    public OutputMembershipPath(
        final int leafIndex,
        final List<byte[]> siblings,
        final byte[] directions,
        final byte[] root) {
      if (leafIndex < 0 || leafIndex >= (1 << CONFIDENTIAL_TREE_DEPTH)) {
        throw new IllegalArgumentException("leafIndex is outside the confidential tree");
      }
      if (siblings == null || siblings.size() != CONFIDENTIAL_TREE_DEPTH) {
        throw new IllegalArgumentException(
            "siblings must contain exactly " + CONFIDENTIAL_TREE_DEPTH + " 32-byte nodes");
      }
      final ArrayList<byte[]> siblingCopies = new ArrayList<>(CONFIDENTIAL_TREE_DEPTH);
      for (final byte[] sibling : siblings) {
        if (sibling == null || sibling.length != 32) {
          throw new IllegalArgumentException(
              "siblings must contain exactly " + CONFIDENTIAL_TREE_DEPTH + " 32-byte nodes");
        }
        siblingCopies.add(Arrays.copyOf(sibling, sibling.length));
      }
      if (directions == null || directions.length != CONFIDENTIAL_TREE_DEPTH) {
        throw new IllegalArgumentException(
            "directions must contain exactly " + CONFIDENTIAL_TREE_DEPTH + " binary values");
      }
      int encodedLeaf = 0;
      for (int level = 0; level < directions.length; level++) {
        if (directions[level] != 0 && directions[level] != 1) {
          throw new IllegalArgumentException("directions must be binary values");
        }
        encodedLeaf |= directions[level] << level;
      }
      if (encodedLeaf != leafIndex) {
        throw new IllegalArgumentException("directions do not encode leafIndex");
      }
      this.leafIndex = leafIndex;
      this.siblings = Collections.unmodifiableList(siblingCopies);
      this.directions = Arrays.copyOf(directions, directions.length);
      this.root = requireDigest(root, "root");
    }

    public int leafIndex() { return leafIndex; }

    public List<byte[]> siblings() {
      final ArrayList<byte[]> copies = new ArrayList<>(siblings.size());
      for (final byte[] sibling : siblings) {
        copies.add(Arrays.copyOf(sibling, sibling.length));
      }
      return Collections.unmodifiableList(copies);
    }

    public byte[] directions() { return Arrays.copyOf(directions, directions.length); }

    public byte[] root() { return Arrays.copyOf(root, root.length); }

    private byte[] flattenedSiblings() {
      final byte[] flattened = new byte[CONFIDENTIAL_TREE_DEPTH * 32];
      for (int index = 0; index < siblings.size(); index++) {
        System.arraycopy(siblings.get(index), 0, flattened, index * 32, 32);
      }
      return flattened;
    }

    /** Convert one validated Torii path entry without weakening its root/index binding. */
    public static OutputMembershipPath from(final ZkMerklePathResponse.Entry entry) {
      final ZkMerklePathResponse.Entry value = Objects.requireNonNull(entry, "entry");
      return new OutputMembershipPath(
          value.leafIndex(), value.siblingBytes(), value.directions(), value.rootBytes());
    }
  }

  /** Insertion path plus membership path for one output at the operation's final root. */
  public static final class OutputMembershipLeafPaths {
    private final OutputMembershipPath updatePath;
    private final OutputMembershipPath membershipPath;

    public OutputMembershipLeafPaths(
        final OutputMembershipPath updatePath,
        final OutputMembershipPath membershipPath) {
      this.updatePath = Objects.requireNonNull(updatePath, "updatePath");
      this.membershipPath = Objects.requireNonNull(membershipPath, "membershipPath");
      if (updatePath.leafIndex() != membershipPath.leafIndex()) {
        throw new IllegalArgumentException(
            "updatePath and membershipPath must address the same leaf");
      }
    }

    public int leafIndex() { return updatePath.leafIndex(); }

    public OutputMembershipPath updatePath() { return updatePath; }

    public OutputMembershipPath membershipPath() { return membershipPath; }

    private byte[][] nativeFields() {
      return new byte[][] {
        Integer.toString(leafIndex()).getBytes(StandardCharsets.UTF_8),
        updatePath.flattenedSiblings(),
        updatePath.directions(),
        updatePath.root(),
        membershipPath.flattenedSiblings(),
        membershipPath.directions(),
        membershipPath.root()
      };
    }
  }

  /** Complete V4 output-update witness; commitments are derived and bound by native code. */
  public static final class OutputMembershipPaths {
    private final byte[] initialRoot;
    private final byte[] finalRoot;
    private final OutputMembershipLeafPaths recipient;
    private final OutputMembershipLeafPaths change;
    private final OutputMembershipPath dummyPath;

    public OutputMembershipPaths(
        final byte[] initialRoot,
        final byte[] finalRoot,
        final OutputMembershipLeafPaths recipient,
        final OutputMembershipLeafPaths change,
        final OutputMembershipPath dummyPath) {
      this.initialRoot = requireDigest(initialRoot, "initialRoot");
      this.finalRoot = requireDigest(finalRoot, "finalRoot");
      this.recipient = recipient;
      this.change = change;
      this.dummyPath = Objects.requireNonNull(dummyPath, "dummyPath");
      if (Arrays.equals(this.initialRoot, this.finalRoot)) {
        throw new IllegalArgumentException("initialRoot and finalRoot must differ");
      }
      if (recipient == null && change == null) {
        throw new IllegalArgumentException("at least one output membership leaf is required");
      }
      if (!Arrays.equals(dummyPath.root(), this.finalRoot)) {
        throw new IllegalArgumentException("dummyPath must be rooted at finalRoot");
      }
      if (recipient != null
          && !Arrays.equals(recipient.membershipPath().root(), this.finalRoot)) {
        throw new IllegalArgumentException(
            "recipient membershipPath must be rooted at finalRoot");
      }
      if (change != null && !Arrays.equals(change.membershipPath().root(), this.finalRoot)) {
        throw new IllegalArgumentException("change membershipPath must be rooted at finalRoot");
      }
      if (recipient != null) {
        if (!Arrays.equals(recipient.updatePath().root(), this.initialRoot)) {
          throw new IllegalArgumentException("recipient updatePath must be rooted at initialRoot");
        }
      } else if (!Arrays.equals(change.updatePath().root(), this.initialRoot)) {
        throw new IllegalArgumentException("change updatePath must be rooted at initialRoot");
      }
      if (recipient != null && change != null
          && recipient.leafIndex() + 1 != change.leafIndex()) {
        throw new IllegalArgumentException(
            "change output must immediately follow the recipient output");
      }
      if ((recipient != null && recipient.leafIndex() == dummyPath.leafIndex())
          || (change != null && change.leafIndex() == dummyPath.leafIndex())
          || (recipient != null && change != null
              && recipient.leafIndex() == change.leafIndex())) {
        throw new IllegalArgumentException(
            "output and dummy paths must address distinct leaves");
      }
    }

    public byte[] initialRoot() { return Arrays.copyOf(initialRoot, initialRoot.length); }

    public byte[] finalRoot() { return Arrays.copyOf(finalRoot, finalRoot.length); }

    public OutputMembershipLeafPaths recipient() { return recipient; }

    public OutputMembershipLeafPaths change() { return change; }

    public OutputMembershipPath dummyPath() { return dummyPath; }

    private byte[] nativeArchive() {
      requireArtifactBridge();
      final byte[][] recipientFields =
          recipient == null ? new byte[0][] : recipient.nativeFields();
      final byte[][] changeFields = change == null ? new byte[0][] : change.nativeFields();
      final byte[][] dummyFields = new byte[][] {
        Integer.toString(dummyPath.leafIndex()).getBytes(StandardCharsets.UTF_8),
        dummyPath.flattenedSiblings(),
        dummyPath.directions(),
        dummyPath.root()
      };
      try {
        return nativeBuildOutputMembershipPathsV4(
            initialRoot, finalRoot, recipientFields, changeFields, dummyFields);
      } finally {
        wipeFields(recipientFields);
        wipeFields(changeFields);
        wipeFields(dummyFields);
      }
    }

    private static void wipeFields(final byte[][] fields) {
      for (final byte[] field : fields) Arrays.fill(field, (byte) 0);
    }
  }

  public static final class InitRequestV4 extends CanonicalArchive {
    private InitRequestV4(final byte[] archive) {
      super(
          archive,
          "KagemushaRecursiveSpendInitLocalRequestV4",
          "initRequest",
          MAX_LOCAL_REQUEST_ARCHIVE_BYTES);
    }
  }

  /** Local secret-bearing append input. Native code consumes and wipes its openings. */
  public static final class AppendRequestV4 extends CanonicalArchive implements AutoCloseable {
    private final NoteOpening changeOpening;

    private AppendRequestV4(final byte[] archive, final NoteOpening changeOpening) {
      super(
          archive,
          "KagemushaRecursiveSpendAppendLocalRequestV4",
          "appendRequest",
          MAX_LOCAL_REQUEST_ARCHIVE_BYTES);
      this.changeOpening = changeOpening;
    }

    @Override
    public void close() { destroy(); }
  }

  public static final class VerifyRequestV4 extends CanonicalArchive {
    private VerifyRequestV4(final byte[] archive) {
      super(
          archive,
          "KagemushaRecursiveSpendVerifyLocalRequestV4",
          "verifyRequest",
          MAX_LOCAL_REQUEST_ARCHIVE_BYTES);
    }
  }

  /** Local secret-bearing redemption input. Native code consumes and wipes its openings. */
  public static final class RedeemRequestV4 extends CanonicalArchive implements AutoCloseable {
    private final NoteOpening changeOpening;

    private RedeemRequestV4(final byte[] archive, final NoteOpening changeOpening) {
      super(
          archive,
          "KagemushaRecursiveSpendRedeemLocalRequestV4",
          "redeemRequest",
          MAX_LOCAL_REQUEST_ARCHIVE_BYTES);
      this.changeOpening = changeOpening;
    }

    @Override
    public void close() { destroy(); }
  }

  public static final class InitResultV4 extends CanonicalArchive {
    private InitResultV4(final byte[] archive) {
      super(
          archive,
          "KagemushaRecursiveSpendInitResultV4",
          "initResult",
          MAX_LOCAL_RESULT_ARCHIVE_BYTES);
    }
  }

  public static final class SplitResultV4 extends CanonicalArchive {
    private final NoteOpening changeOpening;

    private SplitResultV4(final byte[] archive, final NoteOpening changeOpening) {
      super(
          archive,
          "KagemushaRecursiveSpendSplitResultV4",
          "splitResult",
          MAX_LOCAL_RESULT_ARCHIVE_BYTES);
      this.changeOpening = changeOpening;
    }
  }

  public static final class VerifyResultV4 extends CanonicalArchive {
    private VerifyResultV4(final byte[] archive) {
      super(
          archive,
          "KagemushaRecursiveSpendVerifyResultV4",
          "verifyResult",
          MAX_LOCAL_RESULT_ARCHIVE_BYTES);
    }
  }

  public static final class RedeemBuildResultV4 extends CanonicalArchive {
    private final NoteOpening changeOpening;

    private RedeemBuildResultV4(final byte[] archive, final NoteOpening changeOpening) {
      super(
          archive,
          "KagemushaRecursiveSpendRedeemBuildResultV4",
          "redeemBuildResult",
          MAX_LOCAL_RESULT_ARCHIVE_BYTES);
      this.changeOpening = changeOpening;
    }
  }

  public static final class RecipientRequestPreparation {
    private final RecipientRequestPayload payload;
    private final byte[] signingBytes;
    private final NoteOpening opening;
    private final byte[] commitment;
    private final byte[] nullifier;
    private final KagemushaScaledAmount amount;

    private RecipientRequestPreparation(
        final RecipientRequestPayload payload,
        final byte[] signingBytes,
        final NoteOpening opening,
        final byte[] commitment,
        final byte[] nullifier,
        final KagemushaScaledAmount amount) {
      this.payload = payload;
      this.signingBytes = copyRequired(signingBytes, "signingBytes");
      this.opening = opening;
      this.commitment = requireDigest(commitment, "commitment");
      this.nullifier = requireDigest(nullifier, "nullifier");
      this.amount = amount;
    }

    public byte[] signingBytes() { return Arrays.copyOf(signingBytes, signingBytes.length); }
    public NoteOpening opening() { return opening; }
    public byte[] commitment() { return Arrays.copyOf(commitment, commitment.length); }
    public byte[] nullifier() { return Arrays.copyOf(nullifier, nullifier.length); }
    public KagemushaScaledAmount amount() { return amount; }
  }

  public static final class VerifiedRecipientPaymentRequest {
    private final RecipientPaymentRequest request;
    private final byte[] digest;
    private final long verifiedAtMilliseconds;
    private final RecipientRequestProjection projection;

    private VerifiedRecipientPaymentRequest(
        final RecipientPaymentRequest request,
        final byte[] digest,
        final long verifiedAtMilliseconds,
        final RecipientRequestProjection projection) {
      this.request = request;
      this.digest = requireDigest(digest, "requestDigest");
      this.verifiedAtMilliseconds = verifiedAtMilliseconds;
      this.projection = projection;
      if (!Arrays.equals(this.digest, projection.digest())) {
        throw new IllegalStateException("verified request digest does not match its projection");
      }
    }

    public RecipientPaymentRequest request() { return request; }
    public byte[] digest() { return Arrays.copyOf(digest, digest.length); }
    public long verifiedAtMilliseconds() { return verifiedAtMilliseconds; }
    public RecipientRequestProjection projection() { return projection; }
  }

  public static final class RecipientRequestProjection {
    private final String chainId;
    private final String assetDefinitionId;
    private final KagemushaScaledAmount amount;
    private final String recipientAccountId;
    private final String receiverDeviceId;
    private final byte[] requestId;
    private final long issuedAtMilliseconds;
    private final long expiresAtMilliseconds;
    private final byte[] outputCommitment;
    private final byte[] outputNullifier;
    private final byte[] receiverKeyReference;
    private final KagemushaDevicePublicKeyV2 receiverPublicKey;
    private final byte[] digest;

    private RecipientRequestProjection(
        final String chainId,
        final String assetDefinitionId,
        final KagemushaScaledAmount amount,
        final String recipientAccountId,
        final String receiverDeviceId,
        final byte[] requestId,
        final long issuedAtMilliseconds,
        final long expiresAtMilliseconds,
        final byte[] outputCommitment,
        final byte[] outputNullifier,
        final byte[] receiverKeyReference,
        final byte[] receiverPublicKey,
        final byte[] digest) {
      this.chainId = chainId;
      this.assetDefinitionId = assetDefinitionId;
      this.amount = amount;
      this.recipientAccountId = recipientAccountId;
      this.receiverDeviceId = receiverDeviceId;
      this.requestId = requireDigest(requestId, "requestId");
      this.issuedAtMilliseconds = issuedAtMilliseconds;
      this.expiresAtMilliseconds = expiresAtMilliseconds;
      this.outputCommitment = requireDigest(outputCommitment, "outputCommitment");
      this.outputNullifier = requireDigest(outputNullifier, "outputNullifier");
      this.receiverKeyReference = requireDigest(receiverKeyReference, "receiverKeyReference");
      this.receiverPublicKey = new KagemushaDevicePublicKeyV2(receiverPublicKey);
      this.digest = requireDigest(digest, "requestDigest");
    }

    public String chainId() { return chainId; }
    public String assetDefinitionId() { return assetDefinitionId; }
    public KagemushaScaledAmount amount() { return amount; }
    public String recipientAccountId() { return recipientAccountId; }
    public String receiverDeviceId() { return receiverDeviceId; }
    public byte[] requestId() { return Arrays.copyOf(requestId, requestId.length); }
    public long issuedAtMilliseconds() { return issuedAtMilliseconds; }
    public long expiresAtMilliseconds() { return expiresAtMilliseconds; }
    public byte[] outputCommitment() { return Arrays.copyOf(outputCommitment, outputCommitment.length); }
    public byte[] outputNullifier() { return Arrays.copyOf(outputNullifier, outputNullifier.length); }
    public byte[] receiverKeyReference() {
      return Arrays.copyOf(receiverKeyReference, receiverKeyReference.length);
    }
    public KagemushaDevicePublicKeyV2 receiverPublicKey() { return receiverPublicKey; }
    public byte[] digest() { return Arrays.copyOf(digest, digest.length); }
  }

  public static final class RequestAuthorizationPreparation {
    private final RequestAuthorizationTemplate template;
    private final byte[] signingBytes;
    private final byte[] operationId;
    private final byte[] payloadDigest;
    private final byte[] appAttestEvidenceSha256;

    private RequestAuthorizationPreparation(
        final RequestAuthorizationTemplate template,
        final byte[] signingBytes,
        final byte[] operationId,
        final byte[] payloadDigest,
        final byte[] appAttestEvidenceSha256) {
      this.template = template;
      this.signingBytes = copyRequired(signingBytes, "signingBytes");
      this.operationId = requireDigest(operationId, "operationId");
      this.payloadDigest = requireDigest(payloadDigest, "payloadDigest");
      this.appAttestEvidenceSha256 = appAttestEvidenceSha256 == null
          ? null : requireDigest(appAttestEvidenceSha256, "appAttestEvidenceSha256");
    }

    public byte[] signingBytes() { return Arrays.copyOf(signingBytes, signingBytes.length); }
    public byte[] operationId() { return Arrays.copyOf(operationId, operationId.length); }
    public byte[] payloadDigest() { return Arrays.copyOf(payloadDigest, payloadDigest.length); }
    public byte[] appAttestEvidenceSha256() {
      return appAttestEvidenceSha256 == null ? null
          : Arrays.copyOf(appAttestEvidenceSha256, appAttestEvidenceSha256.length);
    }
  }

  public static final class TopUpPreparation {
    private final TopUpUnsigned unsigned;
    private final byte[] authorizationDigest;
    private final NoteOpening opening;
    private final byte[] noteCommitment;
    private final byte[] spendNullifier;
    private final byte[] initialRoot;
    private final byte[] finalizedRoot;
    private final byte[] operationId;
    private final KagemushaScaledAmount amount;
    private final int leafIndex;

    private TopUpPreparation(
        final TopUpUnsigned unsigned,
        final byte[] authorizationDigest,
        final NoteOpening opening,
        final byte[] noteCommitment,
        final byte[] spendNullifier,
        final byte[] initialRoot,
        final byte[] finalizedRoot,
        final byte[] operationId,
        final KagemushaScaledAmount amount,
        final int leafIndex) {
      this.unsigned = unsigned;
      this.authorizationDigest = requireDigest(authorizationDigest, "authorizationDigest");
      this.opening = opening;
      this.noteCommitment = requireDigest(noteCommitment, "noteCommitment");
      this.spendNullifier = requireDigest(spendNullifier, "spendNullifier");
      this.initialRoot = requireDigest(initialRoot, "initialRoot");
      this.finalizedRoot = requireDigest(finalizedRoot, "finalizedRoot");
      this.operationId = requireDigest(operationId, "operationId");
      this.amount = amount;
      this.leafIndex = leafIndex;
    }

    public TopUpUnsigned unsigned() { return unsigned; }
    public byte[] authorizationDigest() {
      return Arrays.copyOf(authorizationDigest, authorizationDigest.length);
    }
    public NoteOpening opening() { return opening; }
    public byte[] noteCommitment() { return Arrays.copyOf(noteCommitment, noteCommitment.length); }
    public byte[] spendNullifier() { return Arrays.copyOf(spendNullifier, spendNullifier.length); }
    public byte[] initialRoot() { return Arrays.copyOf(initialRoot, initialRoot.length); }
    public byte[] finalizedRoot() { return Arrays.copyOf(finalizedRoot, finalizedRoot.length); }
    public byte[] operationId() { return Arrays.copyOf(operationId, operationId.length); }
    public KagemushaScaledAmount amount() { return amount; }
    public int leafIndex() { return leafIndex; }
  }

  public static final class RedeemFinalization {
    private final RedeemSubmissionRequest request;
    private final byte[] operationId;

    private RedeemFinalization(final RedeemSubmissionRequest request, final byte[] operationId) {
      this.request = request;
      this.operationId = requireDigest(operationId, "operationId");
    }

    public RedeemSubmissionRequest request() { return request; }
    public byte[] operationId() { return Arrays.copyOf(operationId, operationId.length); }
  }

  public static class BranchProjection {
    private final Bundle bundle;
    private final NoteMembershipWitness membershipWitness;
    private final byte[] commitment;
    private final byte[] spendNullifier;
    private final KagemushaScaledAmount amount;
    private final int hopCount;
    private final int proofStepCount;
    private final byte[] bundleDigest;
    private final ArtifactBinding artifactBinding;
    private final List<BranchClaim> branchClaims;

    private BranchProjection(
        final Bundle bundle,
        final NoteMembershipWitness membershipWitness,
        final byte[] commitment,
        final byte[] spendNullifier,
        final KagemushaScaledAmount amount,
        final int hopCount,
        final int proofStepCount,
        final byte[] bundleDigest,
        final ArtifactBinding artifactBinding,
        final List<BranchClaim> branchClaims) {
      this.bundle = bundle;
      this.membershipWitness = membershipWitness;
      this.commitment = requireDigest(commitment, "commitment");
      this.spendNullifier = requireDigest(spendNullifier, "spendNullifier");
      this.amount = amount;
      if (hopCount < 0 || hopCount > MAXIMUM_PEER_HOPS) {
        throw new IllegalStateException("native Kagemusha hop count is invalid");
      }
      this.hopCount = hopCount;
      if (proofStepCount < 1 || proofStepCount > 128) {
        throw new IllegalStateException("native Kagemusha proof-step count is invalid");
      }
      this.proofStepCount = proofStepCount;
      this.bundleDigest = requireDigest(bundleDigest, "bundleDigest");
      this.artifactBinding = Objects.requireNonNull(artifactBinding, "artifactBinding");
      if (branchClaims == null
          || branchClaims.size() < 1
          || branchClaims.size() > MAXIMUM_BRANCH_CLAIMS
          || branchClaims.stream().anyMatch(Objects::isNull)) {
        throw new IllegalStateException("native Kagemusha exact-state claims are invalid");
      }
      this.branchClaims = Collections.unmodifiableList(new ArrayList<>(branchClaims));
    }

    public Bundle bundle() { return bundle; }
    public NoteMembershipWitness membershipWitness() { return membershipWitness; }
    public byte[] commitment() { return Arrays.copyOf(commitment, commitment.length); }
    public byte[] spendNullifier() { return Arrays.copyOf(spendNullifier, spendNullifier.length); }
    public byte[] bundleDigest() { return Arrays.copyOf(bundleDigest, bundleDigest.length); }
    public ArtifactBinding artifactBinding() { return artifactBinding; }
    public List<BranchClaim> branchClaims() { return branchClaims; }
    public boolean conflictsWith(final BranchProjection other) {
      Objects.requireNonNull(other, "other");
      for (final BranchClaim left : branchClaims) {
        for (final BranchClaim right : other.branchClaims) {
          if (left.conflictsWith(right)) return true;
        }
      }
      return false;
    }
    public KagemushaScaledAmount amount() { return amount; }
    public int hopCount() { return hopCount; }
    public int proofStepCount() { return proofStepCount; }
  }

  public static final class SpendableBranch extends BranchProjection {
    private final NoteOpening opening;

    private SpendableBranch(
        final Bundle bundle,
        final NoteMembershipWitness membershipWitness,
        final NoteOpening opening,
        final byte[] commitment,
        final byte[] spendNullifier,
        final KagemushaScaledAmount amount,
        final int hopCount,
        final int proofStepCount,
        final byte[] bundleDigest,
        final ArtifactBinding artifactBinding,
        final List<BranchClaim> branchClaims) {
      super(
          bundle, membershipWitness, commitment, spendNullifier, amount, hopCount,
          proofStepCount, bundleDigest, artifactBinding, branchClaims);
      this.opening = Objects.requireNonNull(opening, "opening");
    }

    public NoteOpening opening() { return opening; }
  }

  /** Secret-bearing local state used only by the genuine ABI-20 builders. */
  public static final class SpendableBranchV4 {
    private final BundleV4 bundle;
    private final NoteMembershipWitness membershipWitness;
    private final NoteOpening opening;

    private SpendableBranchV4(
        final BundleV4 bundle,
        final NoteMembershipWitness membershipWitness,
        final NoteOpening opening) {
      this.bundle = Objects.requireNonNull(bundle, "bundle");
      this.membershipWitness = Objects.requireNonNull(membershipWitness, "membershipWitness");
      this.opening = Objects.requireNonNull(opening, "opening");
    }

    public BundleV4 bundle() { return bundle; }
    public NoteMembershipWitness membershipWitness() { return membershipWitness; }
    public NoteOpening opening() { return opening; }
  }

  public static final class PeerPaymentProjection {
    private final BranchProjection branch;
    private final byte[] operationId;
    private final byte[] requestDigest;

    private PeerPaymentProjection(
        final BranchProjection branch, final byte[] operationId, final byte[] requestDigest) {
      this.branch = Objects.requireNonNull(branch, "branch");
      this.operationId = requireDigest(operationId, "operationId");
      this.requestDigest = requireDigest(requestDigest, "requestDigest");
    }

    public BranchProjection branch() { return branch; }
    public byte[] operationId() { return Arrays.copyOf(operationId, operationId.length); }
    public byte[] requestDigest() { return Arrays.copyOf(requestDigest, requestDigest.length); }
  }

  public static final class SplitProjection {
    private final PeerPayment peerPayment;
    private final BranchProjection recipient;
    private final BranchProjection change;
    private final byte[] operationId;
    private final byte[] requestDigest;
    private final byte[] splitBindingDigest;

    private SplitProjection(
        final PeerPayment peerPayment,
        final BranchProjection recipient,
        final BranchProjection change,
        final byte[] operationId,
        final byte[] requestDigest,
        final byte[] splitBindingDigest) {
      this.peerPayment = peerPayment;
      this.recipient = recipient;
      this.change = change;
      this.operationId = requireDigest(operationId, "operationId");
      this.requestDigest = requireDigest(requestDigest, "requestDigest");
      this.splitBindingDigest = requireDigest(splitBindingDigest, "splitBindingDigest");
    }

    public PeerPayment peerPayment() { return peerPayment; }
    public BranchProjection recipient() { return recipient; }
    public BranchProjection change() { return change; }
    public byte[] operationId() { return Arrays.copyOf(operationId, operationId.length); }
    public byte[] requestDigest() { return Arrays.copyOf(requestDigest, requestDigest.length); }
    public byte[] splitBindingDigest() { return Arrays.copyOf(splitBindingDigest, splitBindingDigest.length); }
  }

  public static final class VerifyProjection {
    public final boolean valid;
    public final boolean chainAdmissible;
    public final boolean lineageRedeemable;
    public final boolean witnesslessRedemptionSupported;
    private final byte[] commitment;
    private final byte[] spendNullifier;
    private final KagemushaScaledAmount amount;
    public final int hopCount;
    public final int proofStepCount;
    private final byte[] bundleDigest;
    public final String assetDefinitionId;
    private final ArtifactBinding artifactBinding;
    private final byte[] requestDigest;
    private final byte[] outputBindingDigest;
    public final String verifierBackend;
    public final String verifierName;
    public final String verifierCircuitId;
    public final Long verifierActivationHeight;
    public final Long verifierWithdrawalHeight;
    public final long verifiedAtBlockHeight;
    public final long verifiedAtMilliseconds;
    private final List<BranchClaim> branchClaims;

    private VerifyProjection(
        final boolean valid, final boolean chainAdmissible, final boolean lineageRedeemable,
        final boolean witnesslessRedemptionSupported, final byte[] commitment,
        final byte[] spendNullifier,
        final KagemushaScaledAmount amount, final int hopCount, final int proofStepCount,
        final byte[] bundleDigest, final String assetDefinitionId,
        final ArtifactBinding artifactBinding, final byte[] requestDigest,
        final byte[] outputBindingDigest,
        final String verifierBackend, final String verifierName, final String verifierCircuitId,
        final Long verifierActivationHeight, final Long verifierWithdrawalHeight,
        final long verifiedAtBlockHeight, final long verifiedAtMilliseconds,
        final List<BranchClaim> branchClaims) {
      this.valid = valid;
      this.chainAdmissible = chainAdmissible;
      this.lineageRedeemable = lineageRedeemable;
      this.witnesslessRedemptionSupported = witnesslessRedemptionSupported;
      this.commitment = requireDigest(commitment, "commitment");
      this.spendNullifier = requireDigest(spendNullifier, "spendNullifier");
      this.amount = amount;
      this.hopCount = hopCount;
      this.proofStepCount = proofStepCount;
      this.bundleDigest = requireDigest(bundleDigest, "bundleDigest");
      this.assetDefinitionId = Objects.requireNonNull(assetDefinitionId, "assetDefinitionId");
      this.artifactBinding = Objects.requireNonNull(artifactBinding, "artifactBinding");
      this.requestDigest = requireDigest(requestDigest, "requestDigest");
      this.outputBindingDigest = requireDigest(outputBindingDigest, "outputBindingDigest");
      this.verifierBackend = Objects.requireNonNull(verifierBackend, "verifierBackend");
      this.verifierName = Objects.requireNonNull(verifierName, "verifierName");
      this.verifierCircuitId = Objects.requireNonNull(verifierCircuitId, "verifierCircuitId");
      this.verifierActivationHeight = verifierActivationHeight;
      this.verifierWithdrawalHeight = verifierWithdrawalHeight;
      this.verifiedAtBlockHeight = verifiedAtBlockHeight;
      this.verifiedAtMilliseconds = verifiedAtMilliseconds;
      if (hopCount < 0 || hopCount > MAXIMUM_PEER_HOPS
          || proofStepCount < 1 || proofStepCount > 128) {
        throw new IllegalStateException("native Kagemusha verified state counters are invalid");
      }
      if (verifiedAtBlockHeight <= 0 || verifiedAtMilliseconds <= 0) {
        throw new IllegalStateException("native Kagemusha verification snapshot is invalid");
      }
      if (branchClaims == null
          || branchClaims.isEmpty()
          || branchClaims.size() > MAXIMUM_BRANCH_CLAIMS
          || branchClaims.stream().anyMatch(Objects::isNull)) {
        throw new IllegalStateException("native Kagemusha verified branch claim vector is invalid");
      }
      this.branchClaims = Collections.unmodifiableList(new ArrayList<>(branchClaims));
    }

    public byte[] commitment() { return Arrays.copyOf(commitment, commitment.length); }
    public byte[] spendNullifier() { return Arrays.copyOf(spendNullifier, spendNullifier.length); }
    public KagemushaScaledAmount amount() { return amount; }
    public byte[] bundleDigest() { return Arrays.copyOf(bundleDigest, bundleDigest.length); }
    public ArtifactBinding artifactBinding() { return artifactBinding; }
    public byte[] requestDigest() { return Arrays.copyOf(requestDigest, requestDigest.length); }
    public byte[] outputBindingDigest() { return Arrays.copyOf(outputBindingDigest, outputBindingDigest.length); }
    public List<BranchClaim> branchClaims() { return branchClaims; }
  }

  public static final class RedeemBuildProjection {
    private final byte[] unsignedRequest;
    private final byte[] authorizationDigest;
    private final BranchProjection change;
    private final byte[] operationId;

    private RedeemBuildProjection(
        final byte[] unsignedRequest, final byte[] authorizationDigest,
        final BranchProjection change, final byte[] operationId) {
      this.unsignedRequest = copyRequired(unsignedRequest, "unsignedRequest");
      this.authorizationDigest = requireDigest(authorizationDigest, "authorizationDigest");
      this.change = change;
      this.operationId = requireDigest(operationId, "operationId");
    }

    public byte[] unsignedRequest() { return Arrays.copyOf(unsignedRequest, unsignedRequest.length); }
    public byte[] authorizationDigest() { return Arrays.copyOf(authorizationDigest, authorizationDigest.length); }
    public BranchProjection change() { return change; }
    public byte[] operationId() { return Arrays.copyOf(operationId, operationId.length); }
  }

  public static final class AcknowledgementPayload extends CanonicalArchive {
    private AcknowledgementPayload(final byte[] archive) {
      super(
          archive,
          "KagemushaReceiverAcknowledgementPayloadV2",
          "acknowledgementPayload",
          MAX_PEER_ARCHIVE_BYTES);
    }
  }

  public static final class AcknowledgementPreparation {
    private final AcknowledgementPayload payload;
    private final byte[] signingBytes;
    private final byte[] operationId;
    private final byte[] requestDigest;
    private final byte[] bundleDigest;
    private final byte[] commitment;

    private AcknowledgementPreparation(
        final AcknowledgementPayload payload, final byte[] signingBytes, final byte[] operationId,
        final byte[] requestDigest, final byte[] bundleDigest, final byte[] commitment) {
      this.payload = payload;
      this.signingBytes = copyRequired(signingBytes, "signingBytes");
      this.operationId = requireDigest(operationId, "operationId");
      this.requestDigest = requireDigest(requestDigest, "requestDigest");
      this.bundleDigest = requireDigest(bundleDigest, "bundleDigest");
      this.commitment = requireDigest(commitment, "commitment");
    }

    public byte[] signingBytes() { return Arrays.copyOf(signingBytes, signingBytes.length); }
    public byte[] operationId() { return Arrays.copyOf(operationId, operationId.length); }
    public byte[] requestDigest() { return Arrays.copyOf(requestDigest, requestDigest.length); }
    public byte[] bundleDigest() { return Arrays.copyOf(bundleDigest, bundleDigest.length); }
    public byte[] commitment() { return Arrays.copyOf(commitment, commitment.length); }
  }

  public static final class AcknowledgementVerification {
    public final boolean valid;
    private final byte[] operationId;
    private final byte[] requestDigest;
    private final byte[] bundleDigest;
    private final byte[] acknowledgementDigest;

    private AcknowledgementVerification(
        final boolean valid, final byte[] operationId, final byte[] requestDigest,
        final byte[] bundleDigest, final byte[] acknowledgementDigest) {
      this.valid = valid;
      this.operationId = requireDigest(operationId, "operationId");
      this.requestDigest = requireDigest(requestDigest, "requestDigest");
      this.bundleDigest = requireDigest(bundleDigest, "bundleDigest");
      this.acknowledgementDigest = requireDigest(acknowledgementDigest, "acknowledgementDigest");
    }

    public byte[] operationId() { return Arrays.copyOf(operationId, operationId.length); }
    public byte[] requestDigest() { return Arrays.copyOf(requestDigest, requestDigest.length); }
    public byte[] bundleDigest() { return Arrays.copyOf(bundleDigest, bundleDigest.length); }
    public byte[] acknowledgementDigest() { return Arrays.copyOf(acknowledgementDigest, acknowledgementDigest.length); }
  }

  public static final class Readiness extends CanonicalArchive {
    private Readiness(final byte[] archive) {
      super(archive, "OfflineReadiness", "readiness", MAX_TORII_RESPONSE_BYTES);
    }
  }

  public static final class ActiveVerifier {
    private final String backend;
    private final String name;
    private final int version;
    private final String circuitId;
    private final byte[] commitment;
    private final byte[] publicInputsSchemaHash;
    private final int maximumProofBytes;
    private final long activationHeight;
    private final Long withdrawalHeight;

    private ActiveVerifier(
        final String backend,
        final String name,
        final int version,
        final String circuitId,
        final byte[] commitment,
        final byte[] publicInputsSchemaHash,
        final int maximumProofBytes,
        final long activationHeight,
        final Long withdrawalHeight) {
      this.backend = backend;
      this.name = name;
      this.version = version;
      this.circuitId = circuitId;
      this.commitment = requireDigest(commitment, "verifierCommitment");
      this.publicInputsSchemaHash =
          requireDigest(publicInputsSchemaHash, "publicInputsSchemaHash");
      this.maximumProofBytes = maximumProofBytes;
      this.activationHeight = activationHeight;
      this.withdrawalHeight = withdrawalHeight;
    }

    public String backend() { return backend; }
    public String name() { return name; }
    public int version() { return version; }
    public String circuitId() { return circuitId; }
    public byte[] commitment() { return Arrays.copyOf(commitment, commitment.length); }
    public byte[] publicInputsSchemaHash() {
      return Arrays.copyOf(publicInputsSchemaHash, publicInputsSchemaHash.length);
    }
    public int maximumProofBytes() { return maximumProofBytes; }
    public long activationHeight() { return activationHeight; }
    public Long withdrawalHeight() { return withdrawalHeight; }
    public boolean isActiveAt(final long blockHeight) {
      return blockHeight >= activationHeight
          && (withdrawalHeight == null || blockHeight < withdrawalHeight);
    }
  }

  public static final class ReadinessBlocker {
    private final String code;
    private final String message;

    private ReadinessBlocker(final String code, final String message) {
      this.code = code;
      this.message = message;
    }

    public String code() { return code; }
    public String message() { return message; }
  }

  public static final class ReadinessProjection {
    private final int requiredBridgeAbiVersion;
    private final int maximumHops;
    private final String assetDefinitionId;
    private final Integer assetScale;
    private final long evaluatedBlockHeight;
    private final byte[] evaluatedBlockHash;
    private final boolean proofBackendAvailable;
    private final boolean recursiveLineageSupported;
    private final boolean ready;
    private final ActiveVerifier transferVerifier;
    private final ActiveVerifier topUpShieldVerifier;
    private final ActiveVerifier unshieldVerifier;
    private final ActiveVerifier recursiveStepEqVerifier;
    private final ActiveVerifier recursiveStepEpVerifier;
    private final List<ReadinessBlocker> blockers;

    private ReadinessProjection(
        final int requiredBridgeAbiVersion,
        final int maximumHops,
        final String assetDefinitionId,
        final Integer assetScale,
        final long evaluatedBlockHeight,
        final byte[] evaluatedBlockHash,
        final boolean proofBackendAvailable,
        final boolean recursiveLineageSupported,
        final boolean ready,
        final ActiveVerifier transferVerifier,
        final ActiveVerifier topUpShieldVerifier,
        final ActiveVerifier unshieldVerifier,
        final ActiveVerifier recursiveStepEqVerifier,
        final ActiveVerifier recursiveStepEpVerifier,
        final List<ReadinessBlocker> blockers) {
      this.requiredBridgeAbiVersion = requiredBridgeAbiVersion;
      this.maximumHops = maximumHops;
      this.assetDefinitionId = assetDefinitionId;
      this.assetScale = assetScale;
      this.evaluatedBlockHeight = evaluatedBlockHeight;
      this.evaluatedBlockHash = requireDigest(evaluatedBlockHash, "evaluatedBlockHash");
      this.proofBackendAvailable = proofBackendAvailable;
      this.recursiveLineageSupported = recursiveLineageSupported;
      this.ready = ready;
      this.transferVerifier = transferVerifier;
      this.topUpShieldVerifier = topUpShieldVerifier;
      this.unshieldVerifier = unshieldVerifier;
      this.recursiveStepEqVerifier = recursiveStepEqVerifier;
      this.recursiveStepEpVerifier = recursiveStepEpVerifier;
      this.blockers = Collections.unmodifiableList(new java.util.ArrayList<>(blockers));
    }

    public int requiredBridgeAbiVersion() { return requiredBridgeAbiVersion; }
    public int maximumHops() { return maximumHops; }
    public String assetDefinitionId() { return assetDefinitionId; }
    public Integer assetScale() { return assetScale; }
    public long evaluatedBlockHeight() { return evaluatedBlockHeight; }
    public byte[] evaluatedBlockHash() {
      return Arrays.copyOf(evaluatedBlockHash, evaluatedBlockHash.length);
    }
    public boolean proofBackendAvailable() { return proofBackendAvailable; }
    public boolean recursiveLineageSupported() { return recursiveLineageSupported; }
    public boolean ready() { return ready; }
    public ActiveVerifier transferVerifier() { return transferVerifier; }
    public ActiveVerifier topUpShieldVerifier() { return topUpShieldVerifier; }
    public ActiveVerifier unshieldVerifier() { return unshieldVerifier; }
    public ActiveVerifier recursiveStepEqVerifier() { return recursiveStepEqVerifier; }
    public ActiveVerifier recursiveStepEpVerifier() { return recursiveStepEpVerifier; }
    public List<ReadinessBlocker> blockers() { return blockers; }
    public boolean bridgeCompatible() {
      return requiredBridgeAbiVersion == REQUIRED_NATIVE_BRIDGE_ABI_VERSION;
    }
    /** Every role-specific verifier is present and active at the same committed snapshot. */
    public boolean allVerifiersActive() {
      return transferVerifier != null && transferVerifier.isActiveAt(evaluatedBlockHeight)
          && topUpShieldVerifier != null && topUpShieldVerifier.isActiveAt(evaluatedBlockHeight)
          && unshieldVerifier != null && unshieldVerifier.isActiveAt(evaluatedBlockHeight)
          && recursiveStepEqVerifier != null
          && recursiveStepEqVerifier.isActiveAt(evaluatedBlockHeight)
          && recursiveStepEpVerifier != null
          && recursiveStepEpVerifier.isActiveAt(evaluatedBlockHeight);
    }
    public boolean chainArtifactSetReady() {
      return proofBackendAvailable && recursiveLineageSupported && allVerifiersActive();
    }
    public boolean offlineReady() {
      return ready && bridgeCompatible() && chainArtifactSetReady() && assetScale != null
          && assetScale >= 0 && assetScale <= KagemushaScaledAmount.MAXIMUM_SCALE
          && evaluatedBlockHeight > 0
          && maximumHops == MAXIMUM_PEER_HOPS
          && isProofBackendAvailable() && blockers.isEmpty();
    }
  }

  public static final class OperationReference extends CanonicalArchive {
    private OperationReference(final byte[] archive) {
      super(
          archive,
          "OfflineOperationReference",
          "operationReference",
          MAX_TORII_RESPONSE_BYTES);
    }
  }

  public static final class OperationStatus extends CanonicalArchive {
    private OperationStatus(final byte[] archive) {
      super(archive, "OfflineOperationStatus", "operationStatus", MAX_TORII_RESPONSE_BYTES);
    }
  }

  public enum OperationState { PENDING, APPLIED, REJECTED }

  public enum OperationKind { TOP_UP, REDEEM }

  public static final class OperationRejection {
    private final String code;
    private final String message;

    private OperationRejection(final String code, final String message) {
      this.code = code;
      this.message = message;
    }

    public String code() { return code; }
    public String message() { return message; }
  }

  public static final class FinalizedTopUp {
    private final TopUpAnchor anchor;
    private final TopUpFinalityProof finalityProof;
    private final long finalizedBlockHeight;
    private final long serverTimeMilliseconds;

    private FinalizedTopUp(
        final TopUpAnchor anchor,
        final TopUpFinalityProof finalityProof,
        final long finalizedBlockHeight,
        final long serverTimeMilliseconds) {
      this.anchor = anchor;
      this.finalityProof = finalityProof;
      this.finalizedBlockHeight = finalizedBlockHeight;
      this.serverTimeMilliseconds = serverTimeMilliseconds;
    }

    public TopUpAnchor anchor() { return anchor; }
    public TopUpFinalityProof finalityProof() { return finalityProof; }
    public long finalizedBlockHeight() { return finalizedBlockHeight; }
    public long serverTimeMilliseconds() { return serverTimeMilliseconds; }
  }

  public static final class OperationStatusProjection {
    private final OperationState state;
    private final OperationKind kind;
    private final byte[] operationId;
    private final byte[] transactionHash;
    private final Long submittedAtMilliseconds;
    private final Long finalizedBlockHeight;
    private final Long serverTimeMilliseconds;
    private final FinalizedTopUp finalizedTopUp;
    private final OperationRejection rejection;

    private OperationStatusProjection(
        final OperationState state,
        final OperationKind kind,
        final byte[] operationId,
        final byte[] transactionHash,
        final Long submittedAtMilliseconds,
        final Long finalizedBlockHeight,
        final Long serverTimeMilliseconds,
        final FinalizedTopUp finalizedTopUp,
        final OperationRejection rejection) {
      this.state = state;
      this.kind = kind;
      this.operationId = requireDigest(operationId, "operationId");
      this.transactionHash = requireDigest(transactionHash, "transactionHash");
      this.submittedAtMilliseconds = submittedAtMilliseconds;
      this.finalizedBlockHeight = finalizedBlockHeight;
      this.serverTimeMilliseconds = serverTimeMilliseconds;
      this.finalizedTopUp = finalizedTopUp;
      this.rejection = rejection;
    }

    public OperationState state() { return state; }
    public OperationKind kind() { return kind; }
    public byte[] operationId() { return Arrays.copyOf(operationId, operationId.length); }
    public byte[] transactionHash() { return Arrays.copyOf(transactionHash, transactionHash.length); }
    public Long submittedAtMilliseconds() { return submittedAtMilliseconds; }
    public Long finalizedBlockHeight() { return finalizedBlockHeight; }
    public Long serverTimeMilliseconds() { return serverTimeMilliseconds; }
    public FinalizedTopUp finalizedTopUp() { return finalizedTopUp; }
    public OperationRejection rejection() { return rejection; }
  }

  /** Strict typed client for the four first-release Kagemusha Torii routes. */
  public static final class ToriiClient {
    public static final String READINESS_PATH = "/v1/offline/readiness";
    public static final String TOP_UP_PATH = "/v1/offline/top-up";
    public static final String REDEEM_PATH = "/v1/offline/redeem";
    public static final String OPERATIONS_PATH = "/v1/offline/operations";
    public static final String NORITO_MEDIA_TYPE = "application/x-norito";

    private final String baseUri;
    private final TransportExecutor transport;

    private ToriiClient(final URI baseUri, final TransportExecutor transport) {
      Objects.requireNonNull(baseUri, "baseUri");
      this.transport = Objects.requireNonNull(transport, "transport");
      if (!baseUri.isAbsolute()
          || baseUri.getRawQuery() != null
          || baseUri.getRawFragment() != null
          || baseUri.getRawUserInfo() != null) {
        throw new IllegalArgumentException("baseUri must be an absolute credential-free HTTP URI");
      }
      final String scheme = baseUri.getScheme();
      if (!("http".equalsIgnoreCase(scheme) || "https".equalsIgnoreCase(scheme))) {
        throw new IllegalArgumentException("baseUri must use HTTP or HTTPS");
      }
      this.baseUri = stripTrailingSlash(baseUri.toString());
    }

    public CompletableFuture<Readiness> getReadiness(final String assetDefinitionId) {
      if (assetDefinitionId == null
          || assetDefinitionId.isEmpty()
          || !assetDefinitionId.equals(assetDefinitionId.trim())) {
        throw new IllegalArgumentException("assetDefinitionId must be canonical non-empty text");
      }
      final String encoded;
      try {
        encoded = URLEncoder.encode(assetDefinitionId, StandardCharsets.UTF_8.name());
      } catch (final java.io.UnsupportedEncodingException impossible) {
        throw new IllegalStateException("UTF-8 is unavailable", impossible);
      }
      return execute(
              TransportRequest.builder()
                  .setMethod("GET")
                  .setUri(URI.create(baseUri + READINESS_PATH + "?asset_definition_id=" + encoded))
                  .addHeader("Accept", NORITO_MEDIA_TYPE)
                  .setMaximumResponseBytes((long) MAX_TORII_RESPONSE_BYTES)
                  .build(),
              200)
          .thenApply(response -> new Readiness(response.body()));
    }

    public CompletableFuture<OperationReference> submitTopUp(
        final TopUpRequest request, final String operationId) {
      return submitCommand(
          TOP_UP_PATH, Objects.requireNonNull(request, "request").noritoEncoded(), operationId);
    }

    public CompletableFuture<OperationReference> submitRedeem(
        final RedeemSubmissionRequest request, final String operationId) {
      return submitCommand(
          REDEEM_PATH, Objects.requireNonNull(request, "request").noritoEncoded(), operationId);
    }

    public CompletableFuture<OperationStatus> getOperation(final String operationId) {
      final String id = requireOperationId(operationId);
      return execute(
              TransportRequest.builder()
                  .setMethod("GET")
                  .setUri(URI.create(baseUri + OPERATIONS_PATH + "/" + id))
                  .addHeader("Accept", NORITO_MEDIA_TYPE)
                  .setMaximumResponseBytes((long) MAX_TORII_RESPONSE_BYTES)
                  .build(),
              200)
          .thenApply(response -> new OperationStatus(response.body()));
    }

    private CompletableFuture<OperationReference> submitCommand(
        final String path, final byte[] request, final String operationId) {
      final String id = requireOperationId(operationId);
      return execute(
              TransportRequest.builder()
                  .setMethod("POST")
                  .setUri(URI.create(baseUri + path))
                  .addHeader("Accept", NORITO_MEDIA_TYPE)
                  .addHeader("Content-Type", NORITO_MEDIA_TYPE)
                  .addHeader("Idempotency-Key", id)
                  .setBody(request)
                  .setMaximumResponseBytes((long) MAX_TORII_RESPONSE_BYTES)
                  .build(),
              202)
          .thenApply(response -> new OperationReference(response.body()));
    }

    private CompletableFuture<TransportResponse> execute(
        final TransportRequest request, final int expectedStatus) {
      return transport
          .execute(request)
          .thenApply(
              response -> {
                if (response.statusCode() != expectedStatus) {
                  throw new IllegalStateException(
                      "Kagemusha Torii request failed with HTTP " + response.statusCode());
                }
                final List<String> contentTypes =
                    response.headers().getOrDefault("Content-Type", Collections.emptyList());
                if (contentTypes.size() != 1
                    || !NORITO_MEDIA_TYPE.equalsIgnoreCase(contentTypes.get(0))) {
                  throw new IllegalStateException(
                      "Kagemusha Torii response must use " + NORITO_MEDIA_TYPE);
                }
                return response;
              });
    }

    private static String requireOperationId(final String value) {
      if (value == null || value.length() != 64) {
        throw new IllegalArgumentException("operationId must be non-zero lowercase 32-byte hex");
      }
      boolean nonZero = false;
      for (int index = 0; index < value.length(); index++) {
        final char character = value.charAt(index);
        if (!((character >= '0' && character <= '9')
            || (character >= 'a' && character <= 'f'))) {
          throw new IllegalArgumentException("operationId must be non-zero lowercase 32-byte hex");
        }
        nonZero |= character != '0';
      }
      if (!nonZero) {
        throw new IllegalArgumentException("operationId must be non-zero lowercase 32-byte hex");
      }
      return value;
    }

    private static String stripTrailingSlash(final String value) {
      int end = value.length();
      while (end > 0 && value.charAt(end - 1) == '/') {
        end--;
      }
      return value.substring(0, end);
    }
  }

  /** Owns one native artifact spool until installation or cancellation. */
  public static final class ArtifactIngest implements AutoCloseable {
    private long handle;
    private boolean finalized;
    private boolean installClaimed;

    private ArtifactIngest(final long handle) {
      this.handle = handle;
    }

    public synchronized void write(final byte[] chunk) {
      requireOpen(false);
      nativeArtifactWriteV4(handle, requireChunk(chunk));
    }

    public synchronized void finish() {
      requireOpen(false);
      nativeArtifactFinalizeV4(handle);
      finalized = true;
    }

    public synchronized boolean isFinalized() {
      return finalized;
    }

    @Override
    public synchronized void close() {
      if (handle == 0) {
        return;
      }
      if (installClaimed) {
        throw new IllegalStateException("artifact ingest is being installed");
      }
      nativeArtifactCancelV4(handle);
      handle = 0;
      finalized = false;
    }

    private synchronized long claimFinalizedHandle() {
      if (handle == 0 || !finalized || installClaimed) {
        throw new IllegalStateException("artifact ingest is not installable");
      }
      installClaimed = true;
      return handle;
    }

    private synchronized void releaseInstallClaim(final long expectedHandle) {
      if (handle == expectedHandle) {
        installClaimed = false;
      }
    }

    private synchronized void relinquishInstalledHandle(final long expectedHandle) {
      if (handle != expectedHandle || !finalized || !installClaimed) {
        throw new IllegalStateException("artifact install ownership mismatch");
      }
      handle = 0;
      finalized = false;
      installClaimed = false;
    }

    private void requireOpen(final boolean allowFinalized) {
      if (handle == 0) {
        throw new IllegalStateException("artifact ingest is closed");
      }
      if (finalized && !allowFinalized) {
        throw new IllegalStateException("artifact ingest is already finalized");
      }
      if (installClaimed) {
        throw new IllegalStateException("artifact ingest is being installed");
      }
    }
  }

  /**
   * Locally trusted material required to authenticate one published Kagemusha release.
   *
   * <p>The policy must be provisioned from the deployment trust root rather than copied from the
   * downloaded release. Native code verifies the signed role thresholds and hashes both evidence
   * files before any finalized artifact handle can be consumed.
   */
  public static final class ReleaseAuthentication {
    private final byte[] trustedPolicyNorito;
    private final byte[] releaseAttestationNorito;
    private final byte[] benchmarkEvidence;
    private final byte[] cryptographicReview;

    public ReleaseAuthentication(
        final byte[] trustedPolicyNorito,
        final byte[] releaseAttestationNorito,
        final byte[] benchmarkEvidence,
        final byte[] cryptographicReview) {
      this.trustedPolicyNorito = requireBoundedBytes(
          trustedPolicyNorito,
          "trustedPolicyNorito",
          MAX_TRUSTED_RELEASE_POLICY_BYTES);
      this.releaseAttestationNorito = requireBoundedBytes(
          releaseAttestationNorito,
          "releaseAttestationNorito",
          MAX_RELEASE_ATTESTATION_BYTES);
      this.benchmarkEvidence = requireBoundedBytes(
          benchmarkEvidence,
          "benchmarkEvidence",
          MAX_RELEASE_EVIDENCE_BYTES);
      this.cryptographicReview = requireBoundedBytes(
          cryptographicReview,
          "cryptographicReview",
          MAX_RELEASE_EVIDENCE_BYTES);
    }
  }

  /** Coordinates one authenticated, atomic ten-artifact generation install. */
  public static final class ArtifactInstallSession implements AutoCloseable {
    private final byte[] manifestNorito;
    private final byte[] manifestSha256;
    private final byte[] trustedPolicyNorito;
    private final byte[] releaseAttestationNorito;
    private final byte[] benchmarkEvidence;
    private final byte[] cryptographicReview;
    private final Map<ArtifactRoleV4, ArtifactIngest> artifacts = new LinkedHashMap<>();
    private final List<String> artifactDigests = new ArrayList<>();
    private boolean installed;
    private boolean closed;

    private ArtifactInstallSession(
        final byte[] manifestNorito,
        final byte[] manifestSha256,
        final ReleaseAuthentication releaseAuthentication) {
      this.manifestNorito = Arrays.copyOf(manifestNorito, manifestNorito.length);
      this.manifestSha256 = Arrays.copyOf(manifestSha256, manifestSha256.length);
      this.trustedPolicyNorito = Arrays.copyOf(
          releaseAuthentication.trustedPolicyNorito,
          releaseAuthentication.trustedPolicyNorito.length);
      this.releaseAttestationNorito = Arrays.copyOf(
          releaseAuthentication.releaseAttestationNorito,
          releaseAuthentication.releaseAttestationNorito.length);
      this.benchmarkEvidence = Arrays.copyOf(
          releaseAuthentication.benchmarkEvidence,
          releaseAuthentication.benchmarkEvidence.length);
      this.cryptographicReview = Arrays.copyOf(
          releaseAuthentication.cryptographicReview,
          releaseAuthentication.cryptographicReview.length);
    }

    public synchronized ArtifactIngest beginArtifact(
        final ArtifactRoleV4 role,
        final byte[] expectedArtifactSha256) {
      requirePending();
      if (artifacts.size() == ARTIFACT_COUNT) {
        throw new IllegalStateException("artifact set already has eight streams");
      }
      final ArtifactRoleV4 requiredRole = ArtifactRoleV4.values()[artifacts.size()];
      if (Objects.requireNonNull(role, "role") != requiredRole) {
        throw new IllegalArgumentException("artifact role is not in canonical V4 order");
      }
      final byte[] digest = requireDigest(expectedArtifactSha256, "expectedArtifactSha256");
      final String key = hex(digest);
      if (artifactDigests.contains(key)) {
        throw new IllegalArgumentException("expectedArtifactSha256 is duplicated");
      }
      final ArtifactIngest ingest =
          beginArtifactIngest(manifestNorito, manifestSha256, digest);
      artifacts.put(role, ingest);
      artifactDigests.add(key);
      return ingest;
    }

    public synchronized void install() {
      requirePending();
      if (artifacts.size() != ARTIFACT_COUNT) {
        throw new IllegalStateException("artifact set must contain exactly ten streams");
      }
      requireCanonicalV4ArtifactRoleInventory(new ArrayList<>(artifacts.keySet()));
      final ArtifactIngest[] ordered = artifacts.values().toArray(new ArtifactIngest[0]);
      final long[] handles = new long[ARTIFACT_COUNT];
      int claimed = 0;
      try {
        for (; claimed < ordered.length; claimed++) {
          handles[claimed] = ordered[claimed].claimFinalizedHandle();
        }
        nativeArtifactSetInstallV4(
            manifestNorito,
            manifestSha256,
            trustedPolicyNorito,
            releaseAttestationNorito,
            benchmarkEvidence,
            cryptographicReview,
            handles);
      } catch (final RuntimeException | UnsatisfiedLinkError failure) {
        for (int index = 0; index < claimed; index++) {
          ordered[index].releaseInstallClaim(handles[index]);
        }
        throw failure;
      }
      for (int index = 0; index < ordered.length; index++) {
        ordered[index].relinquishInstalledHandle(handles[index]);
      }
      artifacts.clear();
      artifactDigests.clear();
      installed = true;
    }

    public synchronized boolean isInstalled() {
      return !closed && nativeArtifactSetIsInstalledV4(manifestNorito, manifestSha256);
    }

    public synchronized ArtifactBinding artifactBinding() {
      if (!installed || closed || !isInstalled()) {
        throw new IllegalStateException("artifact set is not installed");
      }
      throw new IllegalStateException(
          "ABI20 V4 artifact binding projection is unavailable until the V4 registry is linked");
    }

    public synchronized void uninstall() {
      if (!installed || closed) {
        return;
      }
      nativeArtifactSetUninstallV4(manifestSha256);
      installed = false;
      closed = true;
    }

    @Override
    public synchronized void close() {
      if (closed || installed) {
        return;
      }
      RuntimeException firstFailure = null;
      for (final ArtifactIngest ingest : artifacts.values()) {
        try {
          ingest.close();
        } catch (final RuntimeException failure) {
          if (firstFailure == null) {
            firstFailure = failure;
          }
        }
      }
      artifacts.clear();
      artifactDigests.clear();
      closed = true;
      if (firstFailure != null) {
        throw firstFailure;
      }
    }

    private void requirePending() {
      if (closed || installed) {
        throw new IllegalStateException("artifact install session is not pending");
      }
    }
  }

  private static String hex(final byte[] digest) {
    final StringBuilder value = new StringBuilder(64);
    for (final byte octet : digest) {
      value.append(Character.forDigit((octet >>> 4) & 0x0f, 16));
      value.append(Character.forDigit(octet & 0x0f, 16));
    }
    return value.toString();
  }

  interface NativeProbe {
    void run();
  }

  interface NativeAbiVersionProbe {
    int run();
  }

  interface NativeSymbolProbe {
    boolean run();
  }

  private static native int nativeBridgeAbiVersion();

  private static native boolean nativePastaCycleV4BackendAvailable();

  private static native long nativeArtifactBeginV4(
      byte[] manifestNorito, byte[] manifestSha256, byte[] expectedArtifactSha256);

  private static native void nativeArtifactWriteV4(long handle, byte[] chunk);

  private static native void nativeArtifactFinalizeV4(long handle);

  private static native void nativeArtifactCancelV4(long handle);

  private static native void nativeArtifactSetInstallV4(
      byte[] manifestNorito,
      byte[] manifestSha256,
      byte[] trustedPolicyNorito,
      byte[] releaseAttestationNorito,
      byte[] benchmarkEvidence,
      byte[] cryptographicReview,
      long[] artifactHandles);

  private static native boolean nativeArtifactSetIsInstalledV4(
      byte[] manifestNorito, byte[] manifestSha256);

  private static native void nativeArtifactSetUninstallV4(byte[] manifestSha256);

  private static native byte[] nativeInitSpendV4(byte[] requestNorito);

  private static native byte[] nativeAppendSpendV4(
      byte[] requestNorito, byte[] recipientRequestNorito, long verifiedAtMilliseconds);

  private static native byte[] nativeVerifySpendV4(byte[] requestNorito);

  private static native byte[] nativeBuildRedeemV4(byte[] requestNorito);

  private static native byte[][] nativePrepareRecipientRequestV2(
      byte[] chainId, byte[] asset, byte[] atomicUnits, int scale, byte[] recipient,
      byte[] receiverDeviceId, byte[] receiverPublicKey, byte[] requestId,
      long issuedAtMilliseconds, long expiresAtMilliseconds, byte[] spendKey, byte[] rho,
      byte[] diversifier);
  private static native byte[] nativeCreateRecipientRequestV2(byte[] payload, byte[] signature);
  private static native byte[] nativeVerifyRecipientRequestV2(byte[] request, long verifiedAtMilliseconds);
  private static native byte[] nativeBuildOutputMembershipPathsV4(
      byte[] initialRoot, byte[] finalRoot, byte[][] recipientFields,
      byte[][] changeFields, byte[][] dummyFields);
  private static native byte[] nativeBuildInitRequestV2(
      byte[] anchor, byte[] proof, byte[] roster);
  private static native byte[] nativeBuildInitRequestV4(
      byte[] anchor, byte[] proof, byte[] roster, byte[] opening, byte[] outputMembership);
  private static native byte[] nativeBuildAppendRequestV2(
      byte[][] bundles, byte[][] openings, byte[][] witnesses, byte[] changeOpening,
      byte[] verifierCommitment, byte[] operationId, long blockHeight);
  private static native byte[] nativeBuildAppendRequestV4(
      byte[][] bundles, byte[][] openings, byte[][] witnesses, byte[] changeOpening,
      byte[] outputMembership, byte[] verifierCommitment, byte[] operationId, long blockHeight);
  private static native byte[][] nativeProjectPeerPaymentV2(byte[] payment);
  private static native byte[][] nativeProjectSplitResultV2(byte[] result);
  private static native byte[] nativeBuildVerifyRequestV2(
      byte[] payment, byte[] request, int maximumHops, long blockHeight, long verifiedAtMilliseconds);
  private static native byte[] nativeBuildVerifyRequestV4(
      byte[] bundle, byte[] recipientRequest, byte[] roster, byte[][] finalityEvidence,
      int maximumHops, long blockHeight, long verifiedAtMilliseconds);
  private static native byte[][] nativeProjectVerifyResultV2(byte[] result);
  private static native byte[] nativeBuildRedeemRequestV2(
      byte[] bundle, byte[] opening, byte[] witness, byte[] recipient, byte[] atomicUnits, int scale,
      byte[] changeOpening, byte[] verifierCommitment, byte[] operationId, long blockHeight);
  private static native byte[] nativeBuildRedeemRequestV4(
      byte[] bundle, byte[] opening, byte[] membershipWitness, byte[] recipient,
      byte[] atomicUnits, int scale, byte[] changeOpening, byte[] changeOutputMembership,
      byte[] verifierCommitment, byte[] operationId, long blockHeight);
  private static native byte[][] nativeProjectRedeemBuildResultV2(byte[] result);
  private static native byte[][] nativePrepareAcknowledgementV2(
      byte[] request, byte[] payment, long acceptedAtMilliseconds);
  private static native byte[] nativeCreateAcknowledgementV2(
      byte[] payload, byte[] signature, byte[] request, byte[] payment);
  private static native byte[][] nativeVerifyAcknowledgementV2(
      byte[] acknowledgement, byte[] request, byte[] payment);
  private static native byte[][] nativeProjectReadinessV2(byte[] readiness);
  private static native byte[][] nativeProjectActiveVerifierV2(byte[] verifier);
  private static native byte[] nativeArtifactBindingV3(byte[] manifest, byte[] manifestSha256);
  private static native byte[][] nativePrepareAuthorizationV2(
      byte[] authority, byte[] deviceId, byte[] operationId, long issuedAtMilliseconds,
      long expiresAtMilliseconds, byte[] nonce, byte[] payloadDigest, byte[] appAttestEvidence);
  private static native byte[] nativeCreateAuthorizationV2(byte[] template, byte[] signature);
  private static native byte[] nativeFinalizeTopUpV2(byte[] unsigned, byte[] authorization);
  private static native byte[][] nativeFinalizeRedeemV2(byte[] buildResult, byte[] authorization);
  private static native byte[][] nativePrepareTopUpV2(
      byte[] chainId, byte[] assetDefinition, byte[] payer, byte[] atomicUnits, int scale,
      byte[] operationId, byte[] spendKey, byte[] rho, byte[] diversifier, int leafIndex,
      byte[] flattenedSiblings, byte[] directions, byte[] root,
      byte[] shieldVerifierCommitment, byte[] artifactBinding);
  private static native byte[][] nativeProjectOperationStatusV2(byte[] status);
  private static native boolean nativeBranchClaimsConflictV2(byte[] left, byte[] right);
  private static native byte[] nativePrepareNoteOpeningV2(
      byte[] spendKey, byte[] rho, byte[] diversifier);
  private static native byte[][] nativeProjectRecipientRequestV2(byte[] request);
}
