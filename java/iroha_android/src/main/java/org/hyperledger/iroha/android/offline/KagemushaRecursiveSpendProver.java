package org.hyperledger.iroha.android.offline;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.locks.ReentrantLock;
import java.util.function.Supplier;
import org.hyperledger.iroha.android.client.JsonNumbers;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.client.LocalSigningContext;
import org.hyperledger.iroha.android.client.ToriiCanonicalRequestAuth;
import org.hyperledger.iroha.android.client.transport.TransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.client.ZkMerklePathResponse;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;

/**
 * Native bridge ABI 23 for Kagemusha ABI-21/V4 artifact streaming and capabilities.
 *
 * <p>This is the sole first-release offline-cash surface. It authenticates the opaque eight-file proof
 * artifact set and validates exact typed request/payment/acknowledgement and proof-bound membership
 * archives. Proof execution remains fail-closed while the native backend reports unavailable.
 * Every recursive lifecycle result is projected only through an ABI-21/V4 native decoder.
 */
public final class KagemushaRecursiveSpendProver {
  /** Retryable contention signal raised before a second proof request is copied. */
  public static final class ProofWorkerBusyException extends IllegalStateException {
    private ProofWorkerBusyException(final String message) {
      super(message);
    }

    private ProofWorkerBusyException(final String message, final Throwable cause) {
      super(message, cause);
    }
  }

  /** Closed first-release hardware assertion profiles for online operations. */
  public enum OnlineHardwareAssertionPlatform {
    ANDROID_KEYMINT(DeviceAttestationRegistration.ANDROID_KEYMINT_PLATFORM),
    IOS_APP_ATTEST(DeviceAttestationRegistration.IOS_APP_ATTEST_PLATFORM);

    private final String wireName;

    OnlineHardwareAssertionPlatform(final String wireName) {
      this.wireName = wireName;
    }

    public String wireName() {
      return wireName;
    }
  }

  public static final int V4_REQUIRED_NATIVE_BRIDGE_ABI_VERSION = 23;
  public static final int REQUIRED_NATIVE_BRIDGE_ABI_VERSION = V4_REQUIRED_NATIVE_BRIDGE_ABI_VERSION;
  /** Mandatory sender-final peer-cash handoff/finality contract. */
  public static final String CASH_HANDOFF_CAPABILITY_V1 = "cash_handoff_v1";
  public static final String V4_ARTIFACT_MANIFEST_SCHEMA =
      "kagemusha.offline.recursive_spend.artifact_manifest.v4";
  public static final String ARTIFACT_MANIFEST_SCHEMA = V4_ARTIFACT_MANIFEST_SCHEMA;
  public static final List<String> V4_ARTIFACT_FILES =
      Collections.unmodifiableList(Arrays.asList(
          "step-eq.params-ipa.krv4",
          "step-eq.proving-key.krv4",
          "step-eq.verifying-key.krv4",
          "step-eq.bootstrap-witness.krv4",
          "step-ep.params-ipa.krv4",
          "step-ep.proving-key.krv4",
          "step-ep.verifying-key.krv4",
          "step-ep.bootstrap-witness.krv4"));
  public static final List<String> ARTIFACT_FILES = V4_ARTIFACT_FILES;
  public static final int V4_ARTIFACT_COUNT = 8;
  public static final int ARTIFACT_COUNT = V4_ARTIFACT_COUNT;
  public static final int MAX_MANIFEST_BYTES = 1024 * 1024;
  public static final int MAX_ARTIFACT_CHUNK_BYTES = 1024 * 1024;
  public static final int MAX_TRUSTED_RELEASE_POLICY_BYTES = 64 * 1024;
  public static final int MAX_RELEASE_ATTESTATION_BYTES = 1024 * 1024;
  public static final int MAX_INTERNAL_VALIDATION_RECEIPT_BYTES = 1024 * 1024;
  public static final int MAX_RELEASE_EVIDENCE_BYTES = 16 * 1024 * 1024;
  public static final int MAX_CRYPTOGRAPHIC_REVIEW_BYTES = 1024 * 1024;
  public static final int MAX_PROMOTION_RECORD_BYTES = 1024 * 1024;
  public static final int MAX_PEER_TEXT_ENVELOPE_BYTES = 12 * 1024;
  public static final int MAX_PEER_TEXT_ARCHIVE_BYTES =
      (MAX_PEER_TEXT_ENVELOPE_BYTES - 6) * 3 / 4;
  public static final int MAX_PEER_ARCHIVE_BYTES_V2 = 32 * 1024;
  public static final int MAX_RECIPIENT_RECEIVE_OFFER_BYTES_V2 = 24_576;
  public static final int MAX_PUBLISHER_CHECKPOINT_ENVELOPE_BYTES_V1 = 2 * 1024;
  public static final int PROMOTED_FINALITY_CHECKPOINT_BYTES_V2 = 40;
  /** Consensus ceiling for one canonical recipient-only ABI-21 peer archive. */
  public static final int MAX_PEER_ARCHIVE_BYTES_V4 = 32 * 1024 * 1024;
  public static final int MAX_PEER_ARCHIVE_BYTES = MAX_PEER_ARCHIVE_BYTES_V4;
  /** Consensus-derived ceiling for one canonical ABI-21 top-up provenance archive. */
  public static final int MAX_TOP_UP_PROVENANCE_ARCHIVE_BYTES_V4 = 6_488_064;
  /** Largest V4 local verify carrier accepted by the native bridge, plus framing headroom. */
  public static final int MAX_LOCAL_REQUEST_ARCHIVE_BYTES_V4 = 64 * 1024 * 1024 + 64;
  public static final int MAX_LOCAL_RESULT_ARCHIVE_BYTES_V4 = 64 * 1024 * 1024 + 64;
  public static final int MAX_LOCAL_REQUEST_ARCHIVE_BYTES = MAX_LOCAL_REQUEST_ARCHIVE_BYTES_V4;
  public static final int MAX_LOCAL_RESULT_ARCHIVE_BYTES = MAX_LOCAL_RESULT_ARCHIVE_BYTES_V4;
  /** Exact Torii body ceiling for the ABI-21/V4 top-up route. */
  public static final int MAX_TORII_TOP_UP_REQUEST_BYTES_V4 = 512 * 1024;

  /** Exact Torii body ceiling for the ABI-21/V4 redemption route. */
  public static final int MAX_TORII_REDEEM_REQUEST_BYTES_V4 = 48 * 1024 * 1024;

  private static final int MAX_REQUEST_AUTHORIZATION_BYTES = 512 * 1024;
  private static final int IOS_APP_ATTEST_ASSERTION_OBJECT_MAX_BYTES = 8 * 1024;
  private static final int IOS_APP_ATTEST_AUTHENTICATOR_DATA_FIXED_HEADER_BYTES = 37;
  private static final int IOS_APP_ATTEST_AUTHENTICATOR_DATA_MIN_BYTES =
      IOS_APP_ATTEST_AUTHENTICATOR_DATA_FIXED_HEADER_BYTES + 1;
  private static final int IOS_APP_ATTEST_AUTHENTICATOR_DATA_MAX_BYTES = 4 * 1024;
  private static final int IOS_APP_ATTEST_EXTENSION_DATA_FLAG = 0x80;
  public static final int MAX_TORII_RESPONSE_BYTES = 4 * 1024 * 1024;
  public static final int MAXIMUM_INPUTS_PER_TRANSITION = 2;
  public static final int MAXIMUM_LOCAL_APPEND_BUILDER_INPUTS = MAXIMUM_INPUTS_PER_TRANSITION;
  public static final int MAXIMUM_BRANCH_CLAIMS = 2;
  public static final int MAXIMUM_PEER_HOPS = 8;
  public static final int MAXIMUM_RECURSIVE_PROOF_PAIR_BYTES_V4 = 384 * 1024;
  public static final int CONFIDENTIAL_TREE_DEPTH = 16;
  public static final int MAX_OUTPUT_MEMBERSHIP_FRONTIER_ARCHIVE_BYTES_V4 = 4 * 1024;
  public static final int MAX_OUTPUT_MEMBERSHIP_PATHS_ARCHIVE_BYTES_V4 = 16 * 1024;

  private static final int EXACT_STATE_PROJECTION_VERSION = 1;

  private static final String LIBRARY_NAME = "connect_norito_bridge";
  private static final String NATIVE_BUSY_MESSAGE =
      " is busy; retry after the active proof completes";
  private static final ReentrantLock HEAVY_PROOF_PERMIT = new ReentrantLock();
  private static final boolean ARTIFACT_BRIDGE_AVAILABLE = loadArtifactBridge();

  private static <T> T withHeavyProofPermit(
      final String label, final Supplier<T> action) {
    if (!HEAVY_PROOF_PERMIT.tryLock()) {
      throw new ProofWorkerBusyException(
          "Kagemusha " + label + " is busy; retry after the active proof completes");
    }
    try {
      return Objects.requireNonNull(action, "action").get();
    } catch (final ProofWorkerBusyException failure) {
      throw failure;
    } catch (final IllegalStateException failure) {
      if (failure.getMessage() != null
          && failure.getMessage().contains(NATIVE_BUSY_MESSAGE)) {
        throw new ProofWorkerBusyException(
            "Kagemusha " + label + " is busy; retry after the active proof completes",
            failure);
      }
      throw failure;
    } finally {
      HEAVY_PROOF_PERMIT.unlock();
    }
  }

  static void withHeavyProofPermitForTest(final Runnable action) {
    withHeavyProofPermit("test", () -> {
      Objects.requireNonNull(action, "action").run();
      return Boolean.TRUE;
    });
  }

  /** Canonical ABI-21 artifact roles. Declaration order is part of the native contract. */
  public enum ArtifactRoleV4 {
    STEP_EQ_PARAMS_IPA("step-eq.params-ipa.krv4"),
    STEP_EQ_PROVING_KEY("step-eq.proving-key.krv4"),
    STEP_EQ_VERIFYING_KEY("step-eq.verifying-key.krv4"),
    STEP_EQ_BOOTSTRAP_WITNESS("step-eq.bootstrap-witness.krv4"),
    STEP_EP_PARAMS_IPA("step-ep.params-ipa.krv4"),
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
      throw new IllegalArgumentException("artifact roles must contain exactly eight entries");
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

  /**
   * Returns whether the linked bridge was compiled with the non-default production Kagemusha
   * capability, independently of whether an authenticated artifact set has been installed yet.
   */
  public static boolean isProductionProofBackendCompiled() {
    if (!ARTIFACT_BRIDGE_AVAILABLE) return false;
    return detectProductionProofBackendCompilation(
        () -> nativeArtifactBeginV4(new byte[] {0}, new byte[32], new byte[32]));
  }

  public static boolean isProofBackendAvailable() {
    if (!ARTIFACT_BRIDGE_AVAILABLE) return false;
    try {
      return nativePastaCycleV4BackendAvailable();
    } catch (final UnsatisfiedLinkError | RuntimeException failure) {
      return false;
    }
  }

  /** Returns the exact authenticated manifest installed in native, or null when absent. */
  public static byte[] installedArtifactManifestSha256V4() {
    if (!ARTIFACT_BRIDGE_AVAILABLE) return null;
    try {
      return requireDigest(nativeInstalledManifestSha256V4(), "installedManifestSha256");
    } catch (final UnsatisfiedLinkError | RuntimeException failure) {
      return null;
    }
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

  public static RecipientRegistrationLineage decodeRecipientRegistrationLineageV2(
      final byte[] archive) {
    return new RecipientRegistrationLineage(archive);
  }

  public static RecipientReceiveOfferV2 decodeRecipientReceiveOfferV2(final byte[] archive) {
    final RecipientReceiveOfferV2 offer = new RecipientReceiveOfferV2(archive);
    projectRecipientReceiveOfferV2(offer);
    return offer;
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
    return SecretArchiveWiper.transferChangeOpeningOwnership(
        changeOpening, opening -> new AppendRequestV4(archive, opening));
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

  public static TopUpProvenanceV4 decodeTopUpProvenanceV4(final byte[] archive) {
    return new TopUpProvenanceV4(archive);
  }

  /** Restores canonical persisted frontier bytes without making a branch spendable. */
  public static OutputMembershipFrontierV4 decodeOutputMembershipFrontierV4(
      final byte[] archive) {
    return new OutputMembershipFrontierV4(archive);
  }

  /** Builds the canonical next-zero frontier that must be persisted atomically with a branch. */
  public static OutputMembershipFrontierV4 buildOutputMembershipFrontierV4(
      final OutputMembershipPath zeroPath) {
    requireArtifactBridge();
    final OutputMembershipPath path = Objects.requireNonNull(zeroPath, "zeroPath");
    final byte[] siblings = path.flattenedSiblings();
    final byte[] directions = path.directions();
    final byte[] root = path.root();
    try {
      return new OutputMembershipFrontierV4(nativeBuildOutputMembershipFrontierV4(
          path.leafIndex(), siblings, directions, root));
    } finally {
      Arrays.fill(siblings, (byte) 0);
      Arrays.fill(directions, (byte) 0);
      Arrays.fill(root, (byte) 0);
    }
  }

  /** Derives the only valid consecutive output paths from one authenticated frontier. */
  public static OutputMembershipPaths deriveOutputMembershipPathsV4(
      final OutputMembershipFrontierV4 frontier,
      final byte[] recipientCommitment,
      final byte[] changeCommitment) {
    if (recipientCommitment == null && changeCommitment == null) {
      throw new IllegalArgumentException(
          "recipientCommitment or changeCommitment must be present");
    }
    requireArtifactBridge();
    final byte[] frontierArchive = Objects.requireNonNull(frontier, "frontier").noritoEncoded();
    final byte[] recipient = recipientCommitment == null
        ? new byte[0] : requireDigest(recipientCommitment, "recipientCommitment");
    final byte[] change = changeCommitment == null
        ? new byte[0] : requireDigest(changeCommitment, "changeCommitment");
    try {
      return outputMembershipPathsFromNativeProjection(
          nativeDeriveOutputMembershipPathsV4(frontierArchive, recipient, change));
    } finally {
      Arrays.fill(frontierArchive, (byte) 0);
      Arrays.fill(recipient, (byte) 0);
      Arrays.fill(change, (byte) 0);
    }
  }

  /**
   * Restore one secret-bearing V4 branch only after native revalidates its provenance against the
   * bundle and the release installed at the current block height. Ownership of {@code opening}
   * transfers at call entry: failure destroys it, while success transfers it to the returned
   * closeable branch.
   */
  public static SpendableBranchV4 restoreSpendableBranchV4(
      final BundleV4 bundle,
      final NoteMembershipWitness membershipWitness,
      final NoteOpening opening,
      final TopUpProvenanceV4 topUpProvenance,
      final long blockHeight) {
    return SecretArchiveWiper.transferChangeOpeningOwnership(
        opening,
        ownedOpening -> restoreSpendableBranchV4Owned(
            bundle,
            membershipWitness,
            Objects.requireNonNull(ownedOpening, "opening"),
            topUpProvenance,
            blockHeight));
  }

  private static SpendableBranchV4 restoreSpendableBranchV4Owned(
      final BundleV4 bundle,
      final NoteMembershipWitness membershipWitness,
      final NoteOpening opening,
      final TopUpProvenanceV4 topUpProvenance,
      final long blockHeight) {
    if (blockHeight <= 0) {
      throw new IllegalArgumentException("blockHeight must be positive");
    }
    requireArtifactBridge();
    requireV4ProofBackend();
    final BundleV4 requiredBundle = Objects.requireNonNull(bundle, "bundle");
    final NoteMembershipWitness requiredWitness =
        Objects.requireNonNull(membershipWitness, "membershipWitness");
    final NoteOpening requiredOpening = Objects.requireNonNull(opening, "opening");
    final TopUpProvenanceV4 requiredProvenance =
        Objects.requireNonNull(topUpProvenance, "topUpProvenance");
    final byte[] bundleArchive = requiredBundle.noritoEncoded();
    final byte[] provenanceArchive = requiredProvenance.noritoEncoded();
    final byte[] witnessArchive = requiredWitness.noritoEncoded();
    final byte[] openingArchive = requiredOpening.noritoEncoded();
    try {
      final OutputMembershipFrontierV4 frontier = new OutputMembershipFrontierV4(
          nativeValidateSpendableBranchV4(
              bundleArchive,
              provenanceArchive,
              witnessArchive,
              openingArchive,
              blockHeight));
      return new SpendableBranchV4(
          requiredBundle,
          requiredWitness,
          requiredOpening,
          requiredProvenance,
          frontier);
    } finally {
      Arrays.fill(bundleArchive, (byte) 0);
      Arrays.fill(provenanceArchive, (byte) 0);
      Arrays.fill(witnessArchive, (byte) 0);
      Arrays.fill(openingArchive, (byte) 0);
    }
  }

  /** Restore finalized top-up state with its caller-retained, local-only note opening. */
  public static SpendableBranchV4 restoreInitBranchV4(
      final InitResultV4 result,
      final NoteOpening opening,
      final long blockHeight) {
    return SecretArchiveWiper.transferChangeOpeningOwnership(opening, ownedOpening -> {
      if (blockHeight <= 0) {
        throw new IllegalArgumentException("blockHeight must be positive");
      }
      final InitProjectionV4 projection = projectInitResultV4(
          Objects.requireNonNull(result, "result"));
      final BranchProjection branch = projection.branch();
      return restoreSpendableBranchV4Owned(
          branch.bundle(),
          branch.membershipWitness(),
          Objects.requireNonNull(ownedOpening, "opening"),
          projection.topUpProvenance(),
          blockHeight);
    });
  }

  /** Restore a received offline payment with the receiver's local-only note opening. */
  public static SpendableBranchV4 restorePeerPaymentBranchV4(
      final PeerPayment payment,
      final NoteOpening opening,
      final long blockHeight) {
    return SecretArchiveWiper.transferChangeOpeningOwnership(opening, ownedOpening -> {
      if (blockHeight <= 0) {
        throw new IllegalArgumentException("blockHeight must be positive");
      }
      final PeerPaymentProjection projection = projectPeerPayment(
          Objects.requireNonNull(payment, "payment"));
      final BranchProjection branch = projection.branch();
      return restoreSpendableBranchV4Owned(
          branch.bundle(),
          branch.membershipWitness(),
          Objects.requireNonNull(ownedOpening, "opening"),
          projection.topUpProvenance(),
          blockHeight);
    });
  }

  /** Restore sender change retained locally after a successful offline split. */
  public static SpendableBranchV4 restoreSplitChangeBranchV4(
      final SplitResultV4 result,
      final long blockHeight) {
    if (blockHeight <= 0) {
      throw new IllegalArgumentException("blockHeight must be positive");
    }
    final SplitResultV4 requiredResult = Objects.requireNonNull(result, "result");
    final NoteOpening opening = requiredResult.takeChangeOpening();
    if (opening == null) {
      throw new IllegalStateException("split result has no local change opening");
    }
    return SecretArchiveWiper.transferChangeOpeningOwnership(
        opening,
        ownedOpening -> {
          final SplitProjection projection = projectSplitResultV4(requiredResult);
          final BranchProjection change = projection.change();
          final TopUpProvenanceV4 provenance = projection.changeTopUpProvenance();
          if (change == null || provenance == null) {
            throw new IllegalStateException("split result has no spendable change branch");
          }
          return restoreSpendableBranchV4Owned(
              change.bundle(), change.membershipWitness(), ownedOpening, provenance, blockHeight);
        });
  }

  /** Restore offline change retained locally after building a partial redemption. */
  public static SpendableBranchV4 restoreRedeemChangeBranchV4(
      final RedeemBuildResultV4 result,
      final long blockHeight) {
    if (blockHeight <= 0) {
      throw new IllegalArgumentException("blockHeight must be positive");
    }
    final RedeemBuildResultV4 requiredResult = Objects.requireNonNull(result, "result");
    final NoteOpening opening = requiredResult.takeChangeOpening();
    if (opening == null) {
      throw new IllegalStateException("redeem result has no local change opening");
    }
    return SecretArchiveWiper.transferChangeOpeningOwnership(
        opening,
        ownedOpening -> {
          final RedeemBuildProjection projection = projectRedeemBuildResultV4(requiredResult);
          final BranchProjection change = projection.change();
          final TopUpProvenanceV4 provenance = projection.changeTopUpProvenance();
          if (change == null || provenance == null) {
            throw new IllegalStateException("redeem result has no spendable change branch");
          }
          return restoreSpendableBranchV4Owned(
              change.bundle(), change.membershipWitness(), ownedOpening, provenance, blockHeight);
        });
  }

  public static RedeemRequestV4 decodeRedeemRequestV4(
      final byte[] archive, final NoteOpening changeOpening) {
    return SecretArchiveWiper.transferChangeOpeningOwnership(
        changeOpening, opening -> new RedeemRequestV4(archive, opening));
  }

  public static InitResultV4 decodeInitResultV4(final byte[] archive) {
    return new InitResultV4(archive);
  }

  public static SplitResultV4 decodeSplitResultV4(
      final byte[] archive, final NoteOpening changeOpening) {
    return SecretArchiveWiper.transferChangeOpeningOwnership(
        changeOpening, opening -> new SplitResultV4(archive, opening));
  }

  public static VerifyResultV4 decodeVerifyResultV4(final byte[] archive) {
    return new VerifyResultV4(archive);
  }

  public static RedeemBuildResultV4 decodeRedeemBuildResultV4(
      final byte[] archive, final NoteOpening changeOpening) {
    return SecretArchiveWiper.transferChangeOpeningOwnership(
        changeOpening, opening -> new RedeemBuildResultV4(archive, opening));
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
    final byte[][] fields = nativeProjectOperationStatusV4(
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
          new TopUpAnchorV4(fields[6]), new TopUpFinalityProof(fields[7]),
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

  public static RequestAuthorizationPreparation prepareRequestAuthorization(
      final String authority,
      final int chainDiscriminant,
      final String deviceId,
      final String assetDefinitionId,
      final byte[] operationId,
      final long issuedAtMilliseconds,
      final long expiresAtMilliseconds,
      final byte[] nonce,
      final byte[] payloadDigest,
      final byte[] registrationHash,
      final OnlineHardwareAssertionPlatform platform) {
    requireArtifactBridge();
    final byte[][] fields = nativePrepareAuthorizationV2(
        utf8(authority, "authority"),
        requireChainDiscriminant(chainDiscriminant),
        utf8(deviceId, "deviceId"),
        utf8(assetDefinitionId, "assetDefinitionId"),
        requireDigest(operationId, "operationId"),
        issuedAtMilliseconds,
        expiresAtMilliseconds,
        requireDigest(nonce, "nonce"),
        requireDigest(payloadDigest, "payloadDigest"),
        requireDigest(registrationHash, "registrationHash"),
        utf8(Objects.requireNonNull(platform, "platform").wireName(), "hardwareAssertionPlatform"));
    requireFieldCount(fields, 5, "authorization preparation");
    return new RequestAuthorizationPreparation(
        new RequestAuthorizationPreparationArchive(fields[0]),
        fields[1], fields[2], fields[3], fields[4]);
  }

  public static RequestAuthorization finalizeRequestAuthorization(
      final RequestAuthorizationPreparation preparation, final byte[] platformSignatureDer) {
    return finalizeRequestAuthorization(preparation, platformSignatureDer, new byte[0]);
  }

  public static RequestAuthorization finalizeRequestAuthorization(
      final RequestAuthorizationPreparation preparation,
      final byte[] platformSignatureDer,
      final byte[] authenticatorData) {
    requireArtifactBridge();
    final byte[] der = copyRequired(platformSignatureDer, "platformSignatureDer");
    final byte[] expectedRaw = KagemushaP256Codec.rawLowSFromStrictDer(der);
    final byte[][] fields = nativeFinalizeHardwareAuthorizationV2(
        Objects.requireNonNull(preparation, "preparation").archive.noritoEncoded(),
        authenticatorData == null
            ? new byte[0]
            : Arrays.copyOf(authenticatorData, authenticatorData.length),
        der);
    requireFieldCount(fields, 2, "authorization finalization");
    if (!Arrays.equals(fields[1], expectedRaw)) {
      throw new IllegalStateException(
          "native authorization signature normalization drifted from the SDK");
    }
    return new RequestAuthorization(fields[0]);
  }

  /** Finalize directly from the CBOR returned by DCAppAttestService.generateAssertion. */
  public static RequestAuthorization finalizeIosAppAttest(
      final RequestAuthorizationPreparation preparation, final byte[] assertionObject) {
    requireArtifactBridge();
    final byte[] boundedAssertionObject = copyRequired(assertionObject, "assertionObject");
    if (boundedAssertionObject.length > IOS_APP_ATTEST_ASSERTION_OBJECT_MAX_BYTES) {
      throw new IllegalArgumentException("assertionObject exceeds the App Attest response bound");
    }
    final byte[][] fields = nativeFinalizeIosAppAttestAuthorizationV2(
        Objects.requireNonNull(preparation, "preparation").archive.noritoEncoded(),
        boundedAssertionObject);
    return requestAuthorizationFromIosAppAttestNativeProjection(fields);
  }

  static RequestAuthorization requestAuthorizationFromIosAppAttestNativeProjection(
      final byte[][] fields) {
    requireFieldCount(fields, 3, "App Attest authorization finalization");
    try {
      KagemushaP256Codec.requireRawLowSSignature(fields[1]);
    } catch (final IllegalArgumentException failure) {
      throw new IllegalStateException(
          "native Kagemusha App Attest finalization returned an invalid raw signature", failure);
    }
    requireIosAppAttestAuthenticatorDataProjection(fields[2]);
    return new RequestAuthorization(fields[0]);
  }

  public static TopUpRequest finalizeTopUp(
      final TopUpUnsigned unsigned, final RequestAuthorization authorization) {
    requireArtifactBridge();
    return new TopUpRequest(nativeFinalizeTopUpV4(
        Objects.requireNonNull(unsigned, "unsigned").noritoEncoded(),
        Objects.requireNonNull(authorization, "authorization").noritoEncoded()));
  }

  public static TopUpRequest finalizeTopUp(
      final TopUpPreparation preparation, final RequestAuthorization authorization) {
    return finalizeTopUp(Objects.requireNonNull(preparation, "preparation").unsigned, authorization);
  }

  public static TopUpPreparation prepareTopUp(
      final NetworkId networkId,
      final int chainDiscriminant,
      final String assetDefinitionId,
      final String payerAccountId,
      final KagemushaScaledAmount amount,
      final byte[] operationId,
      final byte[] openingSpendKey,
      final byte[] openingRho,
      final byte[] openingDiversifier,
      final TopUpZeroPath zeroPath,
      final byte[] shieldVerifierCommitment,
      final ArtifactBindingV4 artifactBinding) {
    requireArtifactBridge();
    Objects.requireNonNull(amount, "amount");
    Objects.requireNonNull(zeroPath, "zeroPath");
    Objects.requireNonNull(artifactBinding, "artifactBinding");
    return SecretArchiveWiper.withOpeningDigests(
        openingSpendKey,
        "openingSpendKey",
        openingRho,
        "openingRho",
        openingDiversifier,
        "openingDiversifier",
        (spendKeyCopy, rhoCopy, diversifierCopy) -> {
          byte[][] fields = null;
          NoteOpening locallyOwnedOpening = null;
          try {
            fields = nativePrepareTopUpV4(
                Objects.requireNonNull(networkId, "networkId").bytes(),
                requireChainDiscriminant(chainDiscriminant),
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
            requireFieldCount(fields, 11, "top-up preparation");
            locallyOwnedOpening = new NoteOpening(fields[2]);
            final TopUpPreparation preparation = new TopUpPreparation(
                new TopUpUnsigned(fields[0]),
                fields[1],
                locallyOwnedOpening,
                fields[3],
                fields[4],
                fields[5],
                fields[6],
                fields[7],
                amount(fields[8], fields[9]),
                integer(fields[10], "leafIndex"));
            locallyOwnedOpening = null;
            return preparation;
          } finally {
            if (locallyOwnedOpening != null) locallyOwnedOpening.close();
            SecretArchiveWiper.wipeAll(fields);
          }
        });
  }

  public static RedeemFinalization finalizeRedeemV4(
      final RedeemBuildResultV4 buildResult, final RequestAuthorization authorization) {
    Objects.requireNonNull(buildResult, "buildResult");
    Objects.requireNonNull(authorization, "authorization");
    requireArtifactBridge();
    final byte[][] fields = nativeFinalizeRedeemV4(
        buildResult.noritoEncoded(), authorization.noritoEncoded());
    requireFieldCount(fields, 2, "V4 redeem finalization");
    return new RedeemFinalization(
        new RedeemSubmissionRequest(fields[0]),
        requireDigest(fields[1], "operationId"));
  }

  public static RecipientRequestPreparation prepareRecipientPaymentRequest(
      final NetworkId networkId,
      final int chainDiscriminant,
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
    return SecretArchiveWiper.withOpeningDigests(
        spendKey,
        "spendKey",
        rho,
        "rho",
        diversifier,
        "diversifier",
        (spendKeyCopy, rhoCopy, diversifierCopy) -> {
          byte[][] fields = null;
          NoteOpening locallyOwnedOpening = null;
          try {
            fields = nativePrepareRecipientRequestV2(
                Objects.requireNonNull(networkId, "networkId").bytes(),
                requireChainDiscriminant(chainDiscriminant),
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
            requireFieldCount(fields, 5, "recipient request preparation");
            locallyOwnedOpening = new NoteOpening(fields[2]);
            final RecipientRequestPreparation preparation = new RecipientRequestPreparation(
                new RecipientRequestPayload(fields[0]),
                fields[1],
                locallyOwnedOpening,
                fields[3],
                fields[4],
                amount);
            locallyOwnedOpening = null;
            return preparation;
          } finally {
            if (locallyOwnedOpening != null) locallyOwnedOpening.close();
            SecretArchiveWiper.wipeAll(fields);
          }
        });
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
    return SecretArchiveWiper.withOpeningDigests(
        spendKey,
        "spendKey",
        rho,
        "rho",
        diversifier,
        "diversifier",
        (spendKeyCopy, rhoCopy, diversifierCopy) -> {
          byte[] nativeArchive = null;
          try {
            nativeArchive = nativePrepareNoteOpeningV2(
                spendKeyCopy, rhoCopy, diversifierCopy);
            return new NoteOpening(nativeArchive);
          } finally {
            SecretArchiveWiper.wipe(nativeArchive);
          }
        });
  }

  /**
   * Prepares partial-redemption change inside the native secret boundary.
   *
   * <p>Native revalidates the exact input note/opening and derives a fresh opening from a
   * domain-separated binding over that input, the change amount, operation id, and caller entropy.
   * The authoritative confidential diversifier is selected natively.</p>
   */
  public static RedemptionChangePreparationV4 prepareRedemptionChangeV4(
      final SpendableBranchV4 input,
      final KagemushaScaledAmount changeAmount,
      final byte[] operationId,
      final byte[] entropy) {
    requireArtifactBridge();
    Objects.requireNonNull(input, "input");
    Objects.requireNonNull(changeAmount, "changeAmount");
    byte[] operation = null;
    byte[] freshEntropy = null;
    byte[] bundleArchive = null;
    byte[] openingArchive = null;
    byte[] atomicUnits = null;
    byte[][] fields = null;
    NoteOpening opening = null;
    try {
      operation = requireDigest(operationId, "operationId");
      freshEntropy = requireDigest(entropy, "entropy");
      if (Arrays.equals(operation, freshEntropy)) {
        throw new IllegalArgumentException("entropy must be distinct from operationId");
      }
      bundleArchive = input.bundle().noritoEncoded();
      openingArchive = input.opening().noritoEncoded();
      atomicUnits = utf8(changeAmount.atomicUnits(), "atomicUnits");
      fields = nativePrepareRedemptionChangeV4(
          bundleArchive,
          openingArchive,
          atomicUnits,
          changeAmount.scale(),
          operation,
          freshEntropy);
      requireFieldCount(fields, 7, "V4 redemption change preparation");
      final String[] digestNames = {"rho", "diversifier", "commitment", "spendNullifier"};
      for (int index = 1; index <= 4; index++) {
        final byte[] checked = requireDigest(fields[index], digestNames[index - 1]);
        Arrays.fill(checked, (byte) 0);
      }
      if (Arrays.equals(fields[1], fields[2])) {
        throw new IllegalStateException(
            "native Kagemusha redemption opening coordinates collide");
      }
      final KagemushaScaledAmount projectedAmount = amount(fields[5], fields[6]);
      if (!projectedAmount.equals(changeAmount)) {
        throw new IllegalStateException("native Kagemusha redemption change amount changed");
      }
      opening = new NoteOpening(fields[0]);
      final NoteOpening preparedOpening = opening;
      opening = null;
      final RedemptionChangePreparationV4 preparation = new RedemptionChangePreparationV4(
          preparedOpening,
          fields[1],
          fields[2],
          fields[3],
          fields[4],
          projectedAmount);
      return preparation;
    } finally {
      if (opening != null) opening.destroy();
      if (fields != null) {
        for (final byte[] field : fields) {
          if (field != null) Arrays.fill(field, (byte) 0);
        }
      }
      if (atomicUnits != null) Arrays.fill(atomicUnits, (byte) 0);
      if (openingArchive != null) Arrays.fill(openingArchive, (byte) 0);
      if (bundleArchive != null) Arrays.fill(bundleArchive, (byte) 0);
      if (freshEntropy != null) Arrays.fill(freshEntropy, (byte) 0);
      if (operation != null) Arrays.fill(operation, (byte) 0);
    }
  }

  /**
   * Prepares sender change for an ordinary one- or two-input peer split.
   * Native reauthenticates the ordered inputs, shared context, receiver request, and exact value
   * conservation before deriving an owned opening under a peer-split-only domain.
   */
  public static PeerSplitChangePreparationV4 preparePeerSplitChangeV4(
      final List<SpendableBranchV4> inputs,
      final VerifiedRecipientPaymentRequest recipientRequest,
      final KagemushaScaledAmount changeAmount,
      final byte[] operationId,
      final byte[] entropy) {
    requireArtifactBridge();
    Objects.requireNonNull(inputs, "inputs");
    Objects.requireNonNull(recipientRequest, "recipientRequest");
    Objects.requireNonNull(changeAmount, "changeAmount");
    if (inputs.isEmpty() || inputs.size() > MAXIMUM_INPUTS_PER_TRANSITION) {
      throw new IllegalArgumentException("inputs must contain one or two spendable branches");
    }
    final byte[] operation = requireDigest(operationId, "operationId");
    final byte[] freshEntropy = requireDigest(entropy, "entropy");
    if (Arrays.equals(operation, freshEntropy)) {
      Arrays.fill(operation, (byte) 0);
      Arrays.fill(freshEntropy, (byte) 0);
      throw new IllegalArgumentException("entropy must be distinct from operationId");
    }
    final byte[][] bundles = new byte[inputs.size()][];
    final byte[][] openings = new byte[inputs.size()][];
    byte[] signedRequest = null;
    byte[] atomicUnits = null;
    byte[][] fields = null;
    NoteOpening opening = null;
    try {
      for (int index = 0; index < inputs.size(); index++) {
        final SpendableBranchV4 input = Objects.requireNonNull(inputs.get(index), "inputs entry");
        bundles[index] = input.bundle().noritoEncoded();
        openings[index] = input.opening().noritoEncoded();
      }
      signedRequest = recipientRequest.request().noritoEncoded();
      atomicUnits = utf8(changeAmount.atomicUnits(), "atomicUnits");
      fields = nativePreparePeerSplitChangeV4(
          bundles,
          openings,
          signedRequest,
          atomicUnits,
          changeAmount.scale(),
          operation,
          freshEntropy);
      requireFieldCount(fields, 7, "V4 peer-split change preparation");
      for (int index = 1; index <= 4; index++) {
        requireDigest(fields[index], "peerSplitChangeField" + index);
      }
      final KagemushaScaledAmount projectedAmount = amount(fields[5], fields[6]);
      if (!projectedAmount.equals(changeAmount)) {
        throw new IllegalStateException("native Kagemusha peer-split change amount changed");
      }
      opening = new NoteOpening(fields[0]);
      final NoteOpening preparedOpening = opening;
      opening = null;
      return new PeerSplitChangePreparationV4(
          preparedOpening,
          fields[1],
          fields[2],
          fields[3],
          fields[4],
          projectedAmount);
    } finally {
      if (opening != null) opening.destroy();
      SecretArchiveWiper.wipeAll(fields);
      if (atomicUnits != null) Arrays.fill(atomicUnits, (byte) 0);
      if (signedRequest != null) Arrays.fill(signedRequest, (byte) 0);
      for (final byte[] value : openings) if (value != null) Arrays.fill(value, (byte) 0);
      for (final byte[] value : bundles) if (value != null) Arrays.fill(value, (byte) 0);
      Arrays.fill(freshEntropy, (byte) 0);
      Arrays.fill(operation, (byte) 0);
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

  /** Create the request-independent selector used to prefetch portable receiver lineage. */
  public static RecipientLineageQueryV2 createRecipientLineageQueryV2(
      final NetworkId networkId,
      final int chainDiscriminant,
      final String recipientAccountId,
      final String receiverDeviceId,
      final String assetDefinitionId,
      final long trustedCheckpointHeight) {
    requireArtifactBridge();
    if (trustedCheckpointHeight <= 0) {
      throw new IllegalArgumentException("trustedCheckpointHeight must be positive");
    }
    return new RecipientLineageQueryV2(
        nativeCreateRecipientLineageQueryV2(
            Objects.requireNonNull(networkId, "networkId").bytes(),
            requireChainDiscriminant(chainDiscriminant),
            utf8(recipientAccountId, "recipientAccountId"),
            utf8(receiverDeviceId, "receiverDeviceId"),
            utf8(assetDefinitionId, "assetDefinitionId"),
            trustedCheckpointHeight));
  }

  /** Verify signed request, active-state lineage and a bounded finality suffix locally. */
  public static VerifiedRecipientRegistrationLineageV2 verifyRecipientRegistrationLineageV2(
      final RecipientPaymentRequest request,
      final RecipientRegistrationLineage lineage,
      final long verifiedAtMilliseconds,
      final long trustedCheckpointHeight,
      final byte[] trustedCheckpointContextId) {
    requireArtifactBridge();
    if (verifiedAtMilliseconds <= 0) {
      throw new IllegalArgumentException("verifiedAtMilliseconds must be positive");
    }
    if (trustedCheckpointHeight <= 0) {
      throw new IllegalArgumentException("trustedCheckpointHeight must be positive");
    }
    final byte[] trustedContext =
        requireFinalityCheckpointContext(
            trustedCheckpointContextId, "trustedCheckpointContextId");
    try {
      final byte[][] fields = nativeVerifyRecipientRegistrationLineageV2(
              Objects.requireNonNull(request, "request").noritoEncoded(),
              Objects.requireNonNull(lineage, "lineage").noritoEncoded(),
              verifiedAtMilliseconds,
              trustedCheckpointHeight,
              trustedContext);
      requireFieldCount(fields, 2, "verified recipient lineage");
      return new VerifiedRecipientRegistrationLineageV2(
          new RecipientRegistrationLineage(fields[0]),
          new FinalityCheckpointPromotionV2(fields[1]));
    } finally {
      Arrays.fill(trustedContext, (byte) 0);
    }
  }

  /** Build one canonical receive offer carrying request, lineage and publisher envelope. */
  public static RecipientReceiveOfferV2 createRecipientReceiveOfferV2(
      final RecipientPaymentRequest request,
      final RecipientRegistrationLineage lineage,
      final byte[] publisherCheckpointEnvelope) {
    requireArtifactBridge();
    final byte[] envelope = requireBoundedBytes(
        publisherCheckpointEnvelope,
        "publisherCheckpointEnvelope",
        MAX_PUBLISHER_CHECKPOINT_ENVELOPE_BYTES_V1);
    try {
      return new RecipientReceiveOfferV2(
          nativeCreateRecipientReceiveOfferV2(
              Objects.requireNonNull(request, "request").noritoEncoded(),
              Objects.requireNonNull(lineage, "lineage").noritoEncoded(),
              envelope));
    } finally {
      Arrays.fill(envelope, (byte) 0);
    }
  }

  public static RecipientReceiveOfferProjectionV2 projectRecipientReceiveOfferV2(
      final RecipientReceiveOfferV2 offer) {
    requireArtifactBridge();
    final byte[][] fields = nativeProjectRecipientReceiveOfferV2(
        Objects.requireNonNull(offer, "offer").noritoEncoded());
    requireFieldCount(fields, 3, "recipient receive offer projection");
    return new RecipientReceiveOfferProjectionV2(
        new RecipientPaymentRequest(fields[0]),
        new RecipientRegistrationLineage(fields[1]),
        fields[2]);
  }

  /** Verify the exact whole offer locally against one durable trusted checkpoint. */
  public static VerifiedRecipientReceiveOfferV2 verifyRecipientReceiveOfferV2(
      final RecipientReceiveOfferV2 offer,
      final long verifiedAtMilliseconds,
      final long trustedCheckpointHeight,
      final byte[] trustedCheckpointContextId) {
    requireArtifactBridge();
    if (verifiedAtMilliseconds <= 0) {
      throw new IllegalArgumentException("verifiedAtMilliseconds must be positive");
    }
    if (trustedCheckpointHeight <= 0) {
      throw new IllegalArgumentException("trustedCheckpointHeight must be positive");
    }
    final byte[] trustedContext = requireFinalityCheckpointContext(
        trustedCheckpointContextId, "trustedCheckpointContextId");
    try {
      final byte[][] fields = nativeVerifyRecipientReceiveOfferV2(
          Objects.requireNonNull(offer, "offer").noritoEncoded(),
          verifiedAtMilliseconds,
          trustedCheckpointHeight,
          trustedContext);
      requireFieldCount(fields, 4, "verified recipient receive offer");
      return new VerifiedRecipientReceiveOfferV2(
          new RecipientPaymentRequest(fields[0]),
          new RecipientRegistrationLineage(fields[1]),
          fields[2],
          new FinalityCheckpointPromotionV2(fields[3]),
          verifiedAtMilliseconds);
    } finally {
      Arrays.fill(trustedContext, (byte) 0);
    }
  }

  public static RecipientRequestProjection projectRecipientPaymentRequest(
      final RecipientPaymentRequest request) {
    requireArtifactBridge();
    final byte[][] fields = nativeProjectRecipientRequestV2(
        Objects.requireNonNull(request, "request").noritoEncoded());
    requireFieldCount(fields, 14, "recipient request projection");
    return new RecipientRequestProjection(
        NetworkId.fromBytes(requireDigest(fields[0], "networkId")),
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
    byte[] openingArchive = null;
    byte[] membershipArchive = null;
    byte[] nativeArchive = null;
    try {
      openingArchive = Objects.requireNonNull(opening, "opening").noritoEncoded();
      membershipArchive = membership.nativeArchive();
      nativeArchive = nativeBuildInitRequestV4(
          Objects.requireNonNull(topUpAnchor, "topUpAnchor").noritoEncoded(),
          Objects.requireNonNull(topUpFinalityProof, "topUpFinalityProof").noritoEncoded(),
          Objects.requireNonNull(topUpFinalityRosterArtifact, "topUpFinalityRosterArtifact")
              .noritoEncoded(),
          openingArchive,
          membershipArchive);
      return new InitRequestV4(nativeArchive);
    } finally {
      SecretArchiveWiper.wipe(nativeArchive);
      SecretArchiveWiper.wipe(membershipArchive);
      SecretArchiveWiper.wipe(openingArchive);
    }
  }

  /** Build and validate the complete origin-finality inventory for one V4 bundle. */
  public static TopUpProvenanceV4 buildTopUpProvenanceV4(
      final BundleV4 bundle,
      final TopUpFinalityRosterArtifact topUpFinalityRosterArtifact,
      final List<TopUpAnchorV4> topUpAnchors,
      final List<TopUpFinalityProof> topUpFinalityProofs,
      final long blockHeight) {
    Objects.requireNonNull(topUpAnchors, "topUpAnchors");
    Objects.requireNonNull(topUpFinalityProofs, "topUpFinalityProofs");
    if (topUpAnchors.size() < 1
        || topUpAnchors.size() > MAXIMUM_INPUTS_PER_TRANSITION
        || topUpFinalityProofs.size() != topUpAnchors.size()) {
      throw new IllegalArgumentException(
          "topUpAnchors and topUpFinalityProofs must have the same 1..2 count");
    }
    requireArtifactBridge();
    final byte[][] anchors = new byte[topUpAnchors.size()][];
    final byte[][] proofs = new byte[topUpFinalityProofs.size()][];
    for (int index = 0; index < anchors.length; index++) {
      anchors[index] = Objects.requireNonNull(
          topUpAnchors.get(index), "topUpAnchors[" + index + "]").noritoEncoded();
      proofs[index] = Objects.requireNonNull(
          topUpFinalityProofs.get(index), "topUpFinalityProofs[" + index + "]").noritoEncoded();
    }
    try {
      return new TopUpProvenanceV4(nativeBuildTopUpProvenanceV4(
          Objects.requireNonNull(bundle, "bundle").noritoEncoded(),
          Objects.requireNonNull(
              topUpFinalityRosterArtifact, "topUpFinalityRosterArtifact").noritoEncoded(),
          anchors,
          proofs,
          blockHeight));
    } finally {
      for (final byte[] value : anchors) Arrays.fill(value, (byte) 0);
      for (final byte[] value : proofs) Arrays.fill(value, (byte) 0);
    }
  }

  /** Revalidate persisted provenance against the bundle and current installed release. */
  public static TopUpProvenanceV4 validateTopUpProvenanceV4(
      final BundleV4 bundle,
      final TopUpProvenanceV4 topUpProvenance,
      final long blockHeight) {
    requireArtifactBridge();
    return new TopUpProvenanceV4(nativeValidateTopUpProvenanceV4(
        Objects.requireNonNull(bundle, "bundle").noritoEncoded(),
        Objects.requireNonNull(topUpProvenance, "topUpProvenance").noritoEncoded(),
        blockHeight));
  }

  /** Build one canonical append request from one or two independently spendable inputs. */
  public static AppendRequestV4 buildAppendRequestV4(
      final List<SpendableBranchV4> inputs,
      final NoteOpening changeOpening,
      final OutputMembershipPaths outputMembershipPaths,
      final byte[] transferVerifierCommitment,
      final byte[] operationId,
      final long blockHeight) {
    return SecretArchiveWiper.transferChangeOpeningOwnership(
        changeOpening,
        ownedChangeOpening -> buildAppendRequestV4Owned(
            inputs,
            ownedChangeOpening,
            outputMembershipPaths,
            transferVerifierCommitment,
            operationId,
            blockHeight));
  }

  private static AppendRequestV4 buildAppendRequestV4Owned(
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
    byte[][] bundles = null;
    byte[][] topUpProvenances = null;
    byte[][] openings = null;
    byte[][] witnesses = null;
    byte[] change = null;
    byte[] outputMembership = null;
    byte[] verifier = null;
    byte[] operation = null;
    byte[] archive = null;
    try {
      bundles = new byte[inputs.size()][];
      topUpProvenances = new byte[inputs.size()][];
      openings = new byte[inputs.size()][];
      witnesses = new byte[inputs.size()][];
      for (int index = 0; index < inputs.size(); index++) {
        final SpendableBranchV4 value = inputs.get(index);
        bundles[index] = value.bundle().noritoEncoded();
        topUpProvenances[index] = value.topUpProvenance().noritoEncoded();
        openings[index] = value.opening().noritoEncoded();
        witnesses[index] = value.membershipWitness().noritoEncoded();
      }
      change = changeOpening == null ? new byte[0] : changeOpening.noritoEncoded();
      outputMembership = membership.nativeArchive();
      verifier = requireDigest(transferVerifierCommitment, "transferVerifierCommitment");
      operation = requireDigest(operationId, "operationId");
      archive = nativeBuildAppendRequestV4(
          bundles, topUpProvenances, openings, witnesses, change, outputMembership, verifier, operation,
          blockHeight);
      return new AppendRequestV4(archive, changeOpening);
    } finally {
      SecretArchiveWiper.wipeAll(bundles);
      SecretArchiveWiper.wipeAll(topUpProvenances);
      SecretArchiveWiper.wipeAll(openings);
      SecretArchiveWiper.wipeAll(witnesses);
      SecretArchiveWiper.wipe(change);
      SecretArchiveWiper.wipe(outputMembership);
      SecretArchiveWiper.wipe(verifier);
      SecretArchiveWiper.wipe(operation);
      SecretArchiveWiper.wipe(archive);
    }
  }

  public static PeerPaymentProjection projectPeerPayment(final PeerPayment payment) {
    requireArtifactBridge();
    Objects.requireNonNull(payment, "payment");
    final byte[][] fields = nativeProjectPeerPaymentV4(payment.noritoEncoded());
    final ProjectionCursor cursor = new ProjectionCursor(fields, "peer payment projection");
    requireProjectionVersion(cursor.next("version"), "peer payment projection");
    final byte[] operationId = requireDigest(cursor.next("operationId"), "operationId");
    final byte[] requestDigest = requireDigest(cursor.next("requestDigest"), "requestDigest");
    final TopUpProvenanceV4 topUpProvenance =
        new TopUpProvenanceV4(cursor.next("topUpProvenance"));
    final BranchProjection projection = branchProjection(cursor);
    cursor.finish();
    final PeerPaymentProjection result =
        new PeerPaymentProjection(projection, topUpProvenance, operationId, requestDigest);
    Arrays.fill(operationId, (byte) 0);
    Arrays.fill(requestDigest, (byte) 0);
    return result;
  }

  public static InitProjectionV4 projectInitResultV4(final InitResultV4 result) {
    requireArtifactBridge();
    final ProjectionCursor cursor = new ProjectionCursor(
        nativeProjectInitResultV4(Objects.requireNonNull(result, "result").noritoEncoded()),
        "V4 init result projection");
    requireProjectionVersion(cursor.next("version"), "V4 init result projection");
    final TopUpProvenanceV4 topUpProvenance =
        new TopUpProvenanceV4(cursor.next("topUpProvenance"));
    final BranchProjection branch = branchProjection(cursor);
    final byte[] publicStatementDigest =
        requireDigest(cursor.next("publicStatementDigest"), "publicStatementDigest");
    cursor.finish();
    return new InitProjectionV4(branch, topUpProvenance, publicStatementDigest);
  }

  public static SplitProjection projectSplitResultV4(final SplitResultV4 result) {
    requireArtifactBridge();
    final ProjectionCursor cursor = new ProjectionCursor(
        nativeProjectSplitResultV4(Objects.requireNonNull(result, "result").noritoEncoded()),
        "V4 split result projection");
    requireProjectionVersion(cursor.next("version"), "V4 split result projection");
    final PeerPayment payment = new PeerPayment(cursor.next("peerPayment"));
    final byte[] operationId = requireDigest(cursor.next("operationId"), "operationId");
    final byte[] requestDigest = requireDigest(cursor.next("requestDigest"), "requestDigest");
    final byte[] splitBindingDigest =
        requireDigest(cursor.next("splitBindingDigest"), "splitBindingDigest");
    final TopUpProvenanceV4 recipientTopUpProvenance =
        new TopUpProvenanceV4(cursor.next("recipientTopUpProvenance"));
    final BranchProjection recipient = branchProjection(cursor);
    final boolean changePresent = bool(cursor.next("changePresent"), "changePresent");
    final TopUpProvenanceV4 changeTopUpProvenance = changePresent
        ? new TopUpProvenanceV4(cursor.next("changeTopUpProvenance")) : null;
    final BranchProjection change = changePresent ? branchProjection(cursor) : null;
    cursor.finish();
    return new SplitProjection(
        payment, recipient, change, recipientTopUpProvenance, changeTopUpProvenance,
        operationId, requestDigest, splitBindingDigest);
  }

  public static VerifyProjection projectVerifyResultV4(final VerifyResultV4 result) {
    requireArtifactBridge();
    final ProjectionCursor cursor = new ProjectionCursor(
        nativeProjectVerifyResultV4(Objects.requireNonNull(result, "result").noritoEncoded()),
        "V4 verify result projection");
    requireProjectionVersion(cursor.next("version"), "V4 verify result projection");
    final boolean valid = bool(cursor.next("valid"), "valid");
    final boolean chainAdmissible = bool(cursor.next("chainAdmissible"), "chainAdmissible");
    final boolean lineageRedeemable =
        bool(cursor.next("lineageRedeemable"), "lineageRedeemable");
    final boolean witnesslessRedemptionSupported = bool(
        cursor.next("witnesslessRedemptionSupported"), "witnesslessRedemptionSupported");
    final byte[] commitment = cursor.next("commitment");
    final byte[] spendNullifier = cursor.next("spendNullifier");
    final KagemushaScaledAmount amount =
        amount(cursor.next("atomicUnits"), cursor.next("scale"));
    final int hopCount = integer(cursor.next("hopCount"), "hopCount");
    final int proofStepCount = integer(cursor.next("proofStepCount"), "proofStepCount");
    final byte[] bundleDigest = cursor.next("bundleDigest");
    final String assetDefinitionId =
        canonicalText(cursor.next("assetDefinitionId"), "assetDefinitionId");
    final ArtifactBindingV4 artifactBinding =
        new ArtifactBindingV4(cursor.next("artifactBinding"));
    final byte[] requestDigest = cursor.next("requestDigest");
    final byte[] outputBindingDigest = cursor.next("outputBindingDigest");
    final String verifierBackend =
        canonicalText(cursor.next("verifierBackend"), "verifierBackend");
    final String verifierName = canonicalText(cursor.next("verifierName"), "verifierName");
    final String verifierCircuitId =
        canonicalText(cursor.next("verifierCircuitId"), "verifierCircuitId");
    final byte[] activationBytes = cursor.next("verifierActivationHeight");
    final Long activation = activationBytes.length == 0
        ? null : longInteger(activationBytes, "verifierActivationHeight");
    final byte[] withdrawalBytes = cursor.next("verifierWithdrawalHeight");
    final Long withdrawal = withdrawalBytes.length == 0
        ? null : longInteger(withdrawalBytes, "verifierWithdrawalHeight");
    final long verifiedAtBlockHeight =
        longInteger(cursor.next("verifiedAtBlockHeight"), "verifiedAtBlockHeight");
    final long verifiedAtMilliseconds =
        longInteger(cursor.next("verifiedAtMilliseconds"), "verifiedAtMilliseconds");
    final int claimCount = projectionCount(cursor.next("branchClaimCount"), "branchClaim");
    final List<BranchClaim> claims = new ArrayList<>(claimCount);
    for (int index = 0; index < claimCount; index++) {
      claims.add(new BranchClaim(cursor.next("branchClaim[" + index + "]")));
    }
    cursor.finish();
    return new VerifyProjection(
        valid, chainAdmissible, lineageRedeemable, witnesslessRedemptionSupported,
        commitment, spendNullifier, amount, hopCount, proofStepCount, bundleDigest,
        assetDefinitionId, artifactBinding, requestDigest, outputBindingDigest,
        verifierBackend, verifierName, verifierCircuitId, activation, withdrawal,
        verifiedAtBlockHeight, verifiedAtMilliseconds, claims);
  }

  public static RedeemBuildProjection projectRedeemBuildResultV4(
      final RedeemBuildResultV4 result) {
    requireArtifactBridge();
    final ProjectionCursor cursor = new ProjectionCursor(
        nativeProjectRedeemBuildResultV4(
            Objects.requireNonNull(result, "result").noritoEncoded()),
        "V4 redeem build projection");
    requireProjectionVersion(cursor.next("version"), "V4 redeem build projection");
    final RedeemUnsignedV4 unsigned = new RedeemUnsignedV4(cursor.next("unsigned"));
    final byte[] authorizationDigest = cursor.next("authorizationDigest");
    final byte[] operationId = cursor.next("operationId");
    final boolean changePresent = bool(cursor.next("changePresent"), "changePresent");
    final TopUpProvenanceV4 changeTopUpProvenance = changePresent
        ? new TopUpProvenanceV4(cursor.next("changeTopUpProvenance")) : null;
    final BranchProjection change = changePresent ? branchProjection(cursor) : null;
    cursor.finish();
    return new RedeemBuildProjection(
        unsigned, authorizationDigest, change, changeTopUpProvenance, operationId);
  }

  public static VerifyRequestV4 buildVerifyRequestV4(
      final BundleV4 bundle,
      final RecipientPaymentRequest recipientRequest,
      final TopUpProvenanceV4 topUpProvenance,
      final int maximumHops,
      final long blockHeight,
      final long verifiedAtMilliseconds) {
    requireArtifactBridge();
    requireV4ProofBackend();
    return new VerifyRequestV4(nativeBuildVerifyRequestV4(
        Objects.requireNonNull(bundle, "bundle").noritoEncoded(),
        Objects.requireNonNull(recipientRequest, "recipientRequest").noritoEncoded(),
        Objects.requireNonNull(topUpProvenance, "topUpProvenance").noritoEncoded(),
        maximumHops,
        blockHeight,
        verifiedAtMilliseconds));
  }

  public static RedeemRequestV4 buildRedeemRequestV4(
      final SpendableBranchV4 input,
      final String recipientAccountId,
      final int chainDiscriminant,
      final KagemushaScaledAmount amount,
      final NoteOpening changeOpening,
      final OutputMembershipPaths changeOutputMembershipPaths,
      final byte[] unshieldVerifierCommitment,
      final byte[] operationId,
      final long blockHeight) {
    return SecretArchiveWiper.transferChangeOpeningOwnership(
        changeOpening,
        ownedChangeOpening -> buildRedeemRequestV4Owned(
            input,
            recipientAccountId,
            chainDiscriminant,
            amount,
            ownedChangeOpening,
            changeOutputMembershipPaths,
            unshieldVerifierCommitment,
            operationId,
            blockHeight));
  }

  private static RedeemRequestV4 buildRedeemRequestV4Owned(
      final SpendableBranchV4 input,
      final String recipientAccountId,
      final int chainDiscriminant,
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
    byte[] change = null;
    byte[] outputMembership = null;
    byte[] verifier = null;
    byte[] operation = null;
    byte[] bundleArchive = null;
    byte[] topUpProvenanceArchive = null;
    byte[] openingArchive = null;
    byte[] witnessArchive = null;
    byte[] recipient = null;
    byte[] atomicUnits = null;
    byte[] archive = null;
    try {
      change = changeOpening == null ? new byte[0] : changeOpening.noritoEncoded();
      outputMembership = changeOutputMembershipPaths == null
          ? new byte[0] : changeOutputMembershipPaths.nativeArchive();
      verifier = requireDigest(unshieldVerifierCommitment, "unshieldVerifierCommitment");
      operation = requireDigest(operationId, "operationId");
      bundleArchive = input.bundle().noritoEncoded();
      topUpProvenanceArchive = input.topUpProvenance().noritoEncoded();
      openingArchive = input.opening().noritoEncoded();
      witnessArchive = input.membershipWitness().noritoEncoded();
      recipient = utf8(recipientAccountId, "recipientAccountId");
      atomicUnits = utf8(amount.atomicUnits(), "atomicUnits");
      archive = nativeBuildRedeemRequestV4(
          bundleArchive, topUpProvenanceArchive, openingArchive, witnessArchive, recipient,
          requireChainDiscriminant(chainDiscriminant), atomicUnits, amount.scale(), change,
          outputMembership, verifier, operation, blockHeight);
      return new RedeemRequestV4(archive, changeOpening);
    } finally {
      SecretArchiveWiper.wipe(change);
      SecretArchiveWiper.wipe(outputMembership);
      SecretArchiveWiper.wipe(verifier);
      SecretArchiveWiper.wipe(operation);
      SecretArchiveWiper.wipe(bundleArchive);
      SecretArchiveWiper.wipe(topUpProvenanceArchive);
      SecretArchiveWiper.wipe(openingArchive);
      SecretArchiveWiper.wipe(witnessArchive);
      SecretArchiveWiper.wipe(recipient);
      SecretArchiveWiper.wipe(atomicUnits);
      SecretArchiveWiper.wipe(archive);
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

  /**
   * Verifies a receiver-signed delivery receipt. Under cash_handoff_v1 this
   * is never a sender commit, acceptance, rollback, or clawback gate.
   */
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
    final InitRequestV4 requiredRequest = Objects.requireNonNull(request, "request");
    return withHeavyProofPermit("init spend", () -> {
      byte[] secretArchive = null;
      boolean terminal = true;
      try {
        requireProofBackend();
        final byte[] borrowed = requiredRequest.borrowForNative();
        secretArchive = borrowed;
        return new InitResultV4(
            callNativeLifecycle("init spend", () -> nativeInitSpendV4(borrowed)));
      } catch (final ProofWorkerBusyException failure) {
        terminal = false;
        throw failure;
      } finally {
        SecretArchiveWiper.wipe(secretArchive);
        if (terminal) requiredRequest.close();
      }
    });
  }

  /** Prove one exact recipient output and optional independently spendable sender change. */
  public static SplitResultV4 appendSpendV4(
      final AppendRequestV4 request,
      final RecipientPaymentRequest recipientRequest,
      final long verifiedAtMilliseconds) {
    final AppendRequestV4 requiredRequest = Objects.requireNonNull(request, "request");
    return withHeavyProofPermit("append spend", () -> {
      byte[] secretArchive = null;
      boolean terminal = true;
      try {
        if (verifiedAtMilliseconds <= 0) {
          throw new IllegalArgumentException("verifiedAtMilliseconds must be positive");
        }
        final RecipientPaymentRequest requiredRecipient =
            Objects.requireNonNull(recipientRequest, "recipientRequest");
        requireProofBackend();
        final byte[] borrowed = requiredRequest.borrowForNative();
        secretArchive = borrowed;
        final byte[] resultArchive = callNativeLifecycle(
            "append spend",
            () -> nativeAppendSpendV4(
                borrowed,
                requiredRecipient.noritoEncoded(),
                verifiedAtMilliseconds));
        return SecretArchiveWiper.transferChangeOpeningOwnership(
            requiredRequest.takeChangeOpening(),
            changeOpening -> new SplitResultV4(resultArchive, changeOpening));
      } catch (final ProofWorkerBusyException failure) {
        terminal = false;
        throw failure;
      } finally {
        SecretArchiveWiper.wipe(secretArchive);
        if (terminal) requiredRequest.close();
      }
    });
  }

  /** Verify the recursive proof, exact split bindings, membership, and hop limit. */
  public static VerifyResultV4 verifySpendV4(final VerifyRequestV4 request) {
    final VerifyRequestV4 requiredRequest = Objects.requireNonNull(request, "request");
    return withHeavyProofPermit("verify spend", () -> {
      requireProofBackend();
      return new VerifyResultV4(callNativeLifecycle(
          "verify spend", () -> nativeVerifySpendV4(requiredRequest.borrowForNative())));
    });
  }

  /** Build a full or partial redemption and its optional proof-bound offline change. */
  public static RedeemBuildResultV4 buildRedeemV4(final RedeemRequestV4 request) {
    final RedeemRequestV4 requiredRequest = Objects.requireNonNull(request, "request");
    return withHeavyProofPermit("build redeem", () -> {
      byte[] secretArchive = null;
      boolean terminal = true;
      try {
        requireProofBackend();
        final byte[] borrowed = requiredRequest.borrowForNative();
        secretArchive = borrowed;
        final byte[] resultArchive = callNativeLifecycle(
            "build redeem", () -> nativeBuildRedeemV4(borrowed));
        return SecretArchiveWiper.transferChangeOpeningOwnership(
            requiredRequest.takeChangeOpening(),
            changeOpening -> new RedeemBuildResultV4(resultArchive, changeOpening));
      } catch (final ProofWorkerBusyException failure) {
        terminal = false;
        throw failure;
      } finally {
        SecretArchiveWiper.wipe(secretArchive);
        if (terminal) requiredRequest.close();
      }
    });
  }

  public static ToriiClient newToriiClient(
      final URI baseUri, final TransportExecutor transport,
      final LocalSigningContext localSigningContext) {
    return new ToriiClient(baseUri, transport, localSigningContext);
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
            expectRejectedSymbolProbe(
                () -> nativeArtifactBeginV4(new byte[] {0}, new byte[32], new byte[32])));
  }

  /* Native returns unavailable before promotion and malformed-artifact after promotion. */
  private static boolean expectRejectedSymbolProbe(final NativeProbe probe) {
    try {
      probe.run();
      return false;
    } catch (final IllegalArgumentException | IllegalStateException expected) {
      return true;
    }
  }

  static boolean detectProductionProofBackendCompilation(final NativeProbe probe) {
    Objects.requireNonNull(probe, "probe");
    try {
      probe.run();
      return false;
    } catch (final IllegalArgumentException productionMalformedArtifact) {
      return true;
    } catch (final IllegalStateException defaultOrCandidateBuild) {
      return false;
    } catch (final UnsatisfiedLinkError | SecurityException absentBridge) {
      return false;
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
    if (!isProofBackendAvailable()) {
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

  @FunctionalInterface
  private interface NativeLifecycleCall {
    byte[] call();
  }

  private static byte[] callNativeLifecycle(
      final String label, final NativeLifecycleCall call) {
    try {
      return requireNativeResult(Objects.requireNonNull(call, "call").call(), label);
    } catch (final IllegalStateException failure) {
      if (failure.getMessage() != null
          && failure.getMessage().contains(NATIVE_BUSY_MESSAGE)) {
        throw new ProofWorkerBusyException(
            "Kagemusha " + label + " is busy; retry after the active proof completes",
            failure);
      }
      throw failure;
    } catch (final UnsatisfiedLinkError failure) {
      throw new IllegalStateException(
          "native Kagemusha " + label + " entrypoint is unavailable", failure);
    }
  }

  private static byte[] utf8(final String value, final String field) {
    if (value == null || value.isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException(field + " must be canonical non-empty text");
    }
    return value.getBytes(StandardCharsets.UTF_8);
  }

  private static int requireChainDiscriminant(final int value) {
    if (value < 0 || value > 0xffff) {
      throw new IllegalArgumentException("chainDiscriminant must fit in u16");
    }
    return value;
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

  private static void requireIosAppAttestAuthenticatorDataProjection(
      final byte[] authenticatorData) {
    if (authenticatorData.length < IOS_APP_ATTEST_AUTHENTICATOR_DATA_MIN_BYTES
        || authenticatorData.length > IOS_APP_ATTEST_AUTHENTICATOR_DATA_MAX_BYTES) {
      throw new IllegalStateException(
          "native Kagemusha App Attest finalization returned invalid authenticator data");
    }
    final int flags = authenticatorData[32] & 0xff;
    if (flags != IOS_APP_ATTEST_EXTENSION_DATA_FLAG) {
      throw new IllegalStateException(
          "native Kagemusha App Attest finalization must return extension-bearing authenticator data");
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

  private static List<byte[]> outputMembershipSiblings(
      final byte[] flattened, final String field) {
    if (flattened.length != CONFIDENTIAL_TREE_DEPTH * 32) {
      throw new IllegalStateException(
          "native Kagemusha " + field + " has an invalid sibling count");
    }
    final ArrayList<byte[]> siblings = new ArrayList<>(CONFIDENTIAL_TREE_DEPTH);
    for (int index = 0; index < CONFIDENTIAL_TREE_DEPTH; index++) {
      siblings.add(Arrays.copyOfRange(flattened, index * 32, (index + 1) * 32));
    }
    return siblings;
  }

  private static OutputMembershipPath outputMembershipPathFromNativeProjection(
      final byte[][] fields,
      final int leafIndex,
      final int siblingsIndex,
      final int directionsIndex,
      final int rootIndex,
      final String field) {
    try {
      return new OutputMembershipPath(
          leafIndex,
          outputMembershipSiblings(fields[siblingsIndex], field + ".siblings"),
          fields[directionsIndex],
          fields[rootIndex]);
    } catch (final IllegalArgumentException failure) {
      throw new IllegalStateException(
          "native Kagemusha " + field + " is invalid", failure);
    }
  }

  private static OutputMembershipLeafPaths outputMembershipLeafFromNativeProjection(
      final byte[][] fields, final int offset, final String field) {
    boolean anyPresent = false;
    boolean allPresent = true;
    for (int index = offset; index < offset + 7; index++) {
      anyPresent |= fields[index].length != 0;
      allPresent &= fields[index].length != 0;
    }
    if (!anyPresent) return null;
    if (!allPresent) {
      throw new IllegalStateException(
          "native Kagemusha " + field + " is only partially present");
    }
    final int leafIndex = integer(fields[offset], field + ".leafIndex");
    return new OutputMembershipLeafPaths(
        outputMembershipPathFromNativeProjection(
            fields, leafIndex, offset + 1, offset + 2, offset + 3, field + ".updatePath"),
        outputMembershipPathFromNativeProjection(
            fields, leafIndex, offset + 4, offset + 5, offset + 6,
            field + ".membershipPath"));
  }

  private static OutputMembershipPaths outputMembershipPathsFromNativeProjection(
      final byte[][] fields) {
    requireFieldCount(fields, 21, "V4 output membership derivation");
    final OutputMembershipLeafPaths recipient =
        outputMembershipLeafFromNativeProjection(fields, 3, "recipient");
    final OutputMembershipLeafPaths change =
        outputMembershipLeafFromNativeProjection(fields, 10, "change");
    if (fields[17].length == 0
        || fields[18].length == 0
        || fields[19].length == 0
        || fields[20].length == 0) {
      throw new IllegalStateException(
          "native Kagemusha dummy output membership path is absent");
    }
    final int dummyLeafIndex = integer(fields[17], "dummy.leafIndex");
    final OutputMembershipPath dummy = outputMembershipPathFromNativeProjection(
        fields, dummyLeafIndex, 18, 19, 20, "dummy.path");
    try {
      return new OutputMembershipPaths(
          fields[1], fields[2], recipient, change, dummy, fields[0]);
    } catch (final IllegalArgumentException failure) {
      throw new IllegalStateException(
          "native Kagemusha V4 output membership derivation is invalid", failure);
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

  private static String requireStableOperationCode(final String value) {
    if (value == null || value.isEmpty() || value.length() > 64) {
      throw new IllegalArgumentException("rejection code must use the stable lowercase code grammar");
    }
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      final boolean alphanumeric = (character >= 'a' && character <= 'z')
          || (character >= '0' && character <= '9');
      if (!alphanumeric && (index == 0 || character != '_')) {
        throw new IllegalArgumentException(
            "rejection code must use the stable lowercase code grammar");
      }
    }
    return value;
  }

  private static String requireCanonicalOperationMessage(final String value) {
    if (value == null || value.isEmpty()
        || isUnicodeWhitespace(value.codePointAt(0))
        || isUnicodeWhitespace(value.codePointBefore(value.length()))
        || value.codePointCount(0, value.length()) > 1024
        || value.getBytes(StandardCharsets.UTF_8).length > 4096) {
      throw new IllegalArgumentException("rejection message must be bounded canonical text");
    }
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (Character.isISOControl(character)) {
        throw new IllegalArgumentException("rejection message must be bounded canonical text");
      }
      if (Character.isHighSurrogate(character)) {
        if (index + 1 >= value.length() || !Character.isLowSurrogate(value.charAt(index + 1))) {
          throw new IllegalArgumentException("rejection message must be bounded canonical text");
        }
        index++;
      } else if (Character.isLowSurrogate(character)) {
        throw new IllegalArgumentException("rejection message must be bounded canonical text");
      }
    }
    return value;
  }

  private static boolean isUnicodeWhitespace(final int codePoint) {
    return Character.isWhitespace(codePoint) || Character.isSpaceChar(codePoint);
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
    final BundleV4 bundle = new BundleV4(cursor.next("bundle"));
    final NoteMembershipWitness witness =
        new NoteMembershipWitness(cursor.next("membershipWitness"));
    final byte[] commitment = cursor.next("commitment");
    final byte[] spendNullifier = cursor.next("spendNullifier");
    final KagemushaScaledAmount amount =
        amount(cursor.next("atomicUnits"), cursor.next("scale"));
    final int hopCount = integer(cursor.next("hopCount"), "hopCount");
    final int proofStepCount = integer(cursor.next("proofStepCount"), "proofStepCount");
    final byte[] bundleDigest = cursor.next("bundleDigest");
    final ArtifactBindingV4 artifactBinding =
        new ArtifactBindingV4(cursor.next("artifactBinding"));
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

  private static byte[] requireFinalityCheckpointContext(
      final byte[] value, final String name) {
    final byte[] context = requireDigest(value, name);
    if ((context[context.length - 1] & 1) != 1) {
      Arrays.fill(context, (byte) 0);
      throw new IllegalArgumentException(name + " must preserve the Iroha hash marker");
    }
    return context;
  }

  private static byte[] requireBoundedBytes(
      final byte[] value, final String name, final int maximumBytes) {
    if (value == null || value.length == 0 || value.length > maximumBytes) {
      throw new IllegalArgumentException(
          name + " must contain 1.." + maximumBytes + " bytes");
    }
    return Arrays.copyOf(value, value.length);
  }

  static byte[] requireChunk(final byte[] value) {
    if (value == null || value.length == 0 || value.length > MAX_ARTIFACT_CHUNK_BYTES) {
      throw new IllegalArgumentException(
          "chunk must contain 1.." + MAX_ARTIFACT_CHUNK_BYTES + " bytes");
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
    try {
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
          || archive.length
              != NoritoHeader.HEADER_LENGTH + peerArchivePadding(schema) + decoded.payload().length
          || !Arrays.equals(
              header.encode(), Arrays.copyOfRange(archive, 0, NoritoHeader.HEADER_LENGTH))) {
        throw new IllegalArgumentException(field + " must use canonical compact Norito framing");
      }
      header.validateChecksum(decoded.payload());
      return archive;
    } catch (final RuntimeException | Error failure) {
      Arrays.fill(archive, (byte) 0);
      throw failure;
    }
  }

  private static int peerArchivePadding(final String schema) {
    return switch (schema) {
      case "iroha_data_model::offline::model::KagemushaRecipientPaymentRequestV2",
          "iroha_torii_shared::offline_api::OfflineRecipientReceiveOfferV2",
          "iroha_data_model::offline::model::KagemushaRecursiveSpendPeerPaymentV4",
          "iroha.torii.v1.offline.top_up.request",
          "iroha.torii.v1.offline.redeem.request" -> 8;
      case "iroha_data_model::offline::model::KagemushaReceiverAcknowledgementV2" -> 0;
      default -> 0;
    };
  }

  /** Immutable canonical Norito archive; proof and accumulator bytes remain opaque. */
  public abstract static class CanonicalArchive {
    private static final Object EQUALITY_TIE_LOCK = new Object();
    private final byte[] archive;
    private final int equalityHashCode;
    private boolean destroyed;

    private CanonicalArchive(
        final byte[] archive, final String schema, final String field, final int maximumBytes) {
      this.archive = requireCanonicalArchive(archive, schema, field, maximumBytes);
      // Retain only the 32-bit collection bucket after zeroization, never a secret digest.
      this.equalityHashCode = Arrays.hashCode(this.archive);
    }

    public final synchronized byte[] noritoEncoded() {
      if (destroyed) {
        throw new IllegalStateException("canonical archive has been destroyed");
      }
      return Arrays.copyOf(archive, archive.length);
    }

    /** Borrows one synchronized native-call copy without changing ownership. */
    final synchronized byte[] borrowForNative() {
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
      if (this == other) {
        return true;
      }
      if (other == null || getClass() != other.getClass()) {
        return false;
      }
      final CanonicalArchive that = (CanonicalArchive) other;
      final int identity = System.identityHashCode(this);
      final int otherIdentity = System.identityHashCode(that);
      if (identity < otherIdentity) {
        synchronized (this) {
          synchronized (that) {
            return liveContentEquals(that);
          }
        }
      }
      if (identity > otherIdentity) {
        synchronized (that) {
          synchronized (this) {
            return liveContentEquals(that);
          }
        }
      }
      synchronized (EQUALITY_TIE_LOCK) {
        synchronized (this) {
          synchronized (that) {
            return liveContentEquals(that);
          }
        }
      }
    }

    @Override
    public final int hashCode() {
      return equalityHashCode;
    }

    private boolean liveContentEquals(final CanonicalArchive other) {
      return !destroyed && !other.destroyed && Arrays.equals(archive, other.archive);
    }
  }

  public static final class RecipientPaymentRequest extends CanonicalArchive {
    private RecipientPaymentRequest(final byte[] archive) {
      super(
          archive,
          "iroha_data_model::offline::model::KagemushaRecipientPaymentRequestV2",
          "recipientPaymentRequest",
          MAX_PEER_ARCHIVE_BYTES_V2);
    }
  }

  public static final class RecipientLineageQueryV2 extends CanonicalArchive {
    private RecipientLineageQueryV2(final byte[] archive) {
      super(
          archive,
          "iroha_torii_shared::offline_api::OfflineRecipientLineageRequest",
          "recipientLineageQuery",
          MAX_PEER_ARCHIVE_BYTES_V2);
    }
  }

  /** Portable proof material; it becomes trusted only through a V2 native verifier result. */
  public static final class RecipientRegistrationLineage extends CanonicalArchive {
    private RecipientRegistrationLineage(final byte[] archive) {
      super(
          archive,
          "iroha_torii_shared::offline_api::OfflineRecipientRegistrationLineage",
          "recipientRegistrationLineage",
          MAX_TORII_RESPONSE_BYTES);
    }
  }

  public static final class RecipientReceiveOfferV2 extends CanonicalArchive {
    private RecipientReceiveOfferV2(final byte[] archive) {
      super(
          archive,
          "iroha_torii_shared::offline_api::OfflineRecipientReceiveOfferV2",
          "recipientReceiveOffer",
          MAX_RECIPIENT_RECEIVE_OFFER_BYTES_V2);
    }
  }

  public static final class PeerPayment extends CanonicalArchive {
    private PeerPayment(final byte[] archive) {
      super(
          archive,
          "iroha_data_model::offline::model::KagemushaRecursiveSpendPeerPaymentV4",
          "peerPayment",
          MAX_PEER_ARCHIVE_BYTES_V4);
    }
  }

  public static final class ReceiverAcknowledgement extends CanonicalArchive {
    private ReceiverAcknowledgement(final byte[] archive) {
      super(
          archive,
          "iroha_data_model::offline::model::KagemushaReceiverAcknowledgementV2",
          "receiverAcknowledgement",
          MAX_PEER_ARCHIVE_BYTES_V2);
    }
  }

  /** Proof-bound output membership state carried atomically with an accepted branch. */
  public static final class NoteMembershipWitness extends CanonicalArchive {
    private NoteMembershipWitness(final byte[] archive) {
      super(
          archive,
          "KagemushaNoteMembershipWitnessV2",
          "noteMembershipWitness",
          MAX_PEER_ARCHIVE_BYTES_V2);
    }
  }

  /**
   * Encrypted local note opening; never send this archive to Torii or a peer.
   *
   * <p>Close this value, preferably with try-with-resources, as soon as ownership ends so its
   * secret archive is zeroized deterministically.</p>
   */
  public static final class NoteOpening extends CanonicalArchive implements AutoCloseable {
    private NoteOpening(final byte[] archive) {
      super(archive, "KagemushaNoteOpeningV2", "noteOpening", MAX_LOCAL_REQUEST_ARCHIVE_BYTES);
    }

    /** Zeroize this opening. Repeated closes are harmless. */
    @Override
    public void close() {
      destroy();
    }
  }

  /** Owns native-derived redemption change secrets until {@link #takeOpening()} moves the opening. */
  public static final class RedemptionChangePreparationV4 implements AutoCloseable {
    private NoteOpening opening;
    private final byte[] rho;
    private final byte[] diversifier;
    private final byte[] commitment;
    private final byte[] spendNullifier;
    private final KagemushaScaledAmount amount;
    private boolean closed;

    RedemptionChangePreparationV4(
        final NoteOpening opening,
        final byte[] rho,
        final byte[] diversifier,
        final byte[] commitment,
        final byte[] spendNullifier,
        final KagemushaScaledAmount amount) {
      final NoteOpening ownedOpening = Objects.requireNonNull(opening, "opening");
      byte[] rhoCopy = null;
      byte[] diversifierCopy = null;
      byte[] commitmentCopy = null;
      byte[] spendNullifierCopy = null;
      try {
        if (ownedOpening.isDestroyed()) {
          throw new IllegalStateException("opening has already been destroyed");
        }
        final KagemushaScaledAmount requiredAmount = Objects.requireNonNull(amount, "amount");
        rhoCopy = requireDigest(rho, "rho");
        diversifierCopy = requireDigest(diversifier, "diversifier");
        commitmentCopy = requireDigest(commitment, "commitment");
        spendNullifierCopy = requireDigest(spendNullifier, "spendNullifier");
        if (Arrays.equals(rhoCopy, diversifierCopy)) {
          throw new IllegalStateException(
              "native Kagemusha redemption opening coordinates collide");
        }
        this.opening = ownedOpening;
        this.rho = rhoCopy;
        this.diversifier = diversifierCopy;
        this.commitment = commitmentCopy;
        this.spendNullifier = spendNullifierCopy;
        this.amount = requiredAmount;
      } catch (final RuntimeException | Error failure) {
        if (spendNullifierCopy != null) Arrays.fill(spendNullifierCopy, (byte) 0);
        if (commitmentCopy != null) Arrays.fill(commitmentCopy, (byte) 0);
        if (diversifierCopy != null) Arrays.fill(diversifierCopy, (byte) 0);
        if (rhoCopy != null) Arrays.fill(rhoCopy, (byte) 0);
        ownedOpening.destroy();
        throw failure;
      }
    }

    /** Move the opening to a request/result owner. This succeeds exactly once. */
    public synchronized NoteOpening takeOpening() {
      requireOpen();
      if (opening == null) {
        throw new IllegalStateException(
            "redemption change opening has already been transferred");
      }
      final NoteOpening ownedOpening = opening;
      opening = null;
      return ownedOpening;
    }
    public synchronized byte[] rho() { requireOpen(); return Arrays.copyOf(rho, rho.length); }
    public synchronized byte[] diversifier() {
      requireOpen();
      return Arrays.copyOf(diversifier, diversifier.length);
    }
    public synchronized byte[] commitment() {
      requireOpen();
      return Arrays.copyOf(commitment, commitment.length);
    }
    public synchronized byte[] spendNullifier() {
      requireOpen();
      return Arrays.copyOf(spendNullifier, spendNullifier.length);
    }
    public synchronized KagemushaScaledAmount amount() {
      requireOpen();
      return amount;
    }

    @Override
    public synchronized void close() {
      if (closed) return;
      if (opening != null) {
        opening.destroy();
        opening = null;
      }
      Arrays.fill(rho, (byte) 0);
      Arrays.fill(diversifier, (byte) 0);
      Arrays.fill(commitment, (byte) 0);
      Arrays.fill(spendNullifier, (byte) 0);
      closed = true;
    }

    private void requireOpen() {
      if (closed) throw new IllegalStateException("redemption change preparation has been destroyed");
    }
  }

  /** Owns native-derived ordinary peer-split change until {@link #takeOpening()} transfers it. */
  public static final class PeerSplitChangePreparationV4 implements AutoCloseable {
    private NoteOpening opening;
    private final byte[] rho;
    private final byte[] diversifier;
    private final byte[] commitment;
    private final byte[] spendNullifier;
    private final KagemushaScaledAmount amount;
    private boolean closed;

    PeerSplitChangePreparationV4(
        final NoteOpening opening,
        final byte[] rho,
        final byte[] diversifier,
        final byte[] commitment,
        final byte[] spendNullifier,
        final KagemushaScaledAmount amount) {
      final NoteOpening ownedOpening = Objects.requireNonNull(opening, "opening");
      byte[] rhoCopy = null;
      byte[] diversifierCopy = null;
      byte[] commitmentCopy = null;
      byte[] nullifierCopy = null;
      try {
        if (ownedOpening.isDestroyed()) {
          throw new IllegalStateException("opening has already been destroyed");
        }
        rhoCopy = requireDigest(rho, "rho");
        diversifierCopy = requireDigest(diversifier, "diversifier");
        commitmentCopy = requireDigest(commitment, "commitment");
        nullifierCopy = requireDigest(spendNullifier, "spendNullifier");
        if (Arrays.equals(rhoCopy, diversifierCopy)) {
          throw new IllegalStateException("native Kagemusha peer-split opening coordinates collide");
        }
        this.opening = ownedOpening;
        this.rho = rhoCopy;
        this.diversifier = diversifierCopy;
        this.commitment = commitmentCopy;
        this.spendNullifier = nullifierCopy;
        this.amount = Objects.requireNonNull(amount, "amount");
      } catch (final Throwable failure) {
        if (nullifierCopy != null) Arrays.fill(nullifierCopy, (byte) 0);
        if (commitmentCopy != null) Arrays.fill(commitmentCopy, (byte) 0);
        if (diversifierCopy != null) Arrays.fill(diversifierCopy, (byte) 0);
        if (rhoCopy != null) Arrays.fill(rhoCopy, (byte) 0);
        ownedOpening.destroy();
        throw failure;
      }
    }

    public synchronized NoteOpening takeOpening() {
      requireOpen();
      if (opening == null) {
        throw new IllegalStateException("peer-split change opening has already been transferred");
      }
      final NoteOpening owned = opening;
      opening = null;
      return owned;
    }

    public synchronized byte[] rho() { requireOpen(); return Arrays.copyOf(rho, rho.length); }
    public synchronized byte[] diversifier() {
      requireOpen(); return Arrays.copyOf(diversifier, diversifier.length);
    }
    public synchronized byte[] commitment() {
      requireOpen(); return Arrays.copyOf(commitment, commitment.length);
    }
    public synchronized byte[] spendNullifier() {
      requireOpen(); return Arrays.copyOf(spendNullifier, spendNullifier.length);
    }
    public synchronized KagemushaScaledAmount amount() { requireOpen(); return amount; }

    @Override
    public synchronized void close() {
      if (closed) return;
      if (opening != null) opening.destroy();
      opening = null;
      Arrays.fill(rho, (byte) 0);
      Arrays.fill(diversifier, (byte) 0);
      Arrays.fill(commitment, (byte) 0);
      Arrays.fill(spendNullifier, (byte) 0);
      closed = true;
    }

    private void requireOpen() {
      if (closed) throw new IllegalStateException("peer-split change preparation has been destroyed");
    }
  }

  public static final class RecipientRequestPayload extends CanonicalArchive {
    private RecipientRequestPayload(final byte[] archive) {
      super(
          archive,
          "KagemushaRecipientPaymentRequestSigningPayloadV2",
          "recipientRequestPayload",
          MAX_PEER_ARCHIVE_BYTES_V2);
    }
  }

  /** Opaque ABI-21 recursive state. */
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
          MAX_PEER_ARCHIVE_BYTES_V2);
    }

    public boolean conflictsWith(final BranchClaim other) {
      requireArtifactBridge();
      return nativeBranchClaimsConflictV2(
          noritoEncoded(), Objects.requireNonNull(other, "other").noritoEncoded());
    }
  }

  public static final class ArtifactBindingV4 extends CanonicalArchive {
    private ArtifactBindingV4(final byte[] archive) {
      super(archive, "KagemushaRecursiveSpendArtifactBindingV4", "artifactBinding", MAX_MANIFEST_BYTES);
    }
  }

  public static final class TopUpUnsigned extends CanonicalArchive {
    private TopUpUnsigned(final byte[] archive) {
      super(
          archive,
          "KagemushaRecursiveSpendTopUpUnsignedV4",
          "topUpUnsigned",
          MAX_TORII_TOP_UP_REQUEST_BYTES_V4);
    }
  }

  public static final class TopUpRequest extends CanonicalArchive {
    TopUpRequest(final byte[] archive) {
      super(
          archive,
          "iroha.torii.v1.offline.top_up.request",
          "topUpRequest",
          MAX_TORII_TOP_UP_REQUEST_BYTES_V4);
    }
  }

  /** Finalized ABI-21 top-up receipt with a V4 artifact binding. */
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

  /** Complete bounded origin-finality inventory required to spend or verify one V4 bundle. */
  public static final class TopUpProvenanceV4 extends CanonicalArchive {
    private TopUpProvenanceV4(final byte[] archive) {
      super(
          archive,
          "KagemushaRecursiveSpendTopUpProvenanceV4",
          "topUpProvenanceV4",
          MAX_TOP_UP_PROVENANCE_ARCHIVE_BYTES_V4);
    }
  }

  public static final class RedeemSubmissionRequest extends CanonicalArchive {
    RedeemSubmissionRequest(final byte[] archive) {
      super(
          archive,
          "iroha.torii.v1.offline.redeem.request",
          "redeemSubmissionRequest",
          MAX_TORII_REDEEM_REQUEST_BYTES_V4);
    }
  }

  public static final class RedeemUnsignedV4 extends CanonicalArchive {
    private RedeemUnsignedV4(final byte[] archive) {
      super(
          archive,
          "KagemushaRecursiveSpendRedeemUnsignedV4",
          "redeemUnsignedV4",
          MAX_TORII_REDEEM_REQUEST_BYTES_V4);
    }
  }

  public static final class RequestAuthorizationPreparationArchive extends CanonicalArchive {
    private RequestAuthorizationPreparationArchive(final byte[] archive) {
      super(
          archive,
          "KagemushaRequestAuthorizationPreparationV2",
          "requestAuthorizationPreparation",
          MAX_REQUEST_AUTHORIZATION_BYTES);
    }
  }

  public static final class RequestAuthorization extends CanonicalArchive {
    private RequestAuthorization(final byte[] archive) {
      super(
          archive,
          "KagemushaRequestAuthorizationV2",
          "requestAuthorization",
          MAX_REQUEST_AUTHORIZATION_BYTES);
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

  /** Canonical next-zero cursor persisted atomically with every restored ABI-21 branch. */
  public static final class OutputMembershipFrontierV4 extends CanonicalArchive {
    private OutputMembershipFrontierV4(final byte[] archive) {
      super(
          archive,
          "connect_norito_bridge::KagemushaOutputMembershipFrontierV4",
          "outputMembershipFrontierV4",
          MAX_OUTPUT_MEMBERSHIP_FRONTIER_ARCHIVE_BYTES_V4);
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
    private final byte[] canonicalArchive;

    public OutputMembershipPaths(
        final byte[] initialRoot,
        final byte[] finalRoot,
        final OutputMembershipLeafPaths recipient,
        final OutputMembershipLeafPaths change,
        final OutputMembershipPath dummyPath) {
      this(initialRoot, finalRoot, recipient, change, dummyPath, null);
    }

    private OutputMembershipPaths(
        final byte[] initialRoot,
        final byte[] finalRoot,
        final OutputMembershipLeafPaths recipient,
        final OutputMembershipLeafPaths change,
        final OutputMembershipPath dummyPath,
        final byte[] canonicalArchive) {
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
      final int lastOutputLeafIndex = change != null
          ? change.leafIndex()
          : recipient.leafIndex();
      if (dummyPath.leafIndex() != lastOutputLeafIndex + 1) {
        throw new IllegalArgumentException(
            "dummyPath must immediately follow the final output");
      }
      if ((recipient != null && recipient.leafIndex() == dummyPath.leafIndex())
          || (change != null && change.leafIndex() == dummyPath.leafIndex())
          || (recipient != null && change != null
              && recipient.leafIndex() == change.leafIndex())) {
        throw new IllegalArgumentException(
            "output and dummy paths must address distinct leaves");
      }
      this.canonicalArchive = canonicalArchive == null ? null : requireCanonicalArchive(
          canonicalArchive,
          "connect_norito_bridge::KagemushaOutputMembershipPathsV4",
          "outputMembershipPathsV4",
          MAX_OUTPUT_MEMBERSHIP_PATHS_ARCHIVE_BYTES_V4);
    }

    public byte[] initialRoot() { return Arrays.copyOf(initialRoot, initialRoot.length); }

    public byte[] finalRoot() { return Arrays.copyOf(finalRoot, finalRoot.length); }

    public OutputMembershipLeafPaths recipient() { return recipient; }

    public OutputMembershipLeafPaths change() { return change; }

    public OutputMembershipPath dummyPath() { return dummyPath; }

    private byte[] nativeArchive() {
      if (canonicalArchive != null) {
        return Arrays.copyOf(canonicalArchive, canonicalArchive.length);
      }
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

  /** Local secret-bearing initialization input. Close it if it is not submitted. */
  public static final class InitRequestV4 extends CanonicalArchive implements AutoCloseable {
    private InitRequestV4(final byte[] archive) {
      super(
          archive,
          "KagemushaRecursiveSpendInitLocalRequestV4",
          "initRequest",
          MAX_LOCAL_REQUEST_ARCHIVE_BYTES);
    }

    /** Zeroize an unconsumed initialization request. Repeated closes are harmless. */
    @Override
    public void close() {
      destroy();
    }
  }

  /** Local secret-bearing append input. Native code consumes and wipes its openings. */
  public static final class AppendRequestV4 extends CanonicalArchive implements AutoCloseable {
    private final SecretArchiveWiper.ChangeOpeningOwner changeOpeningOwner;

    private AppendRequestV4(final byte[] archive, final NoteOpening changeOpening) {
      super(
          archive,
          "KagemushaRecursiveSpendAppendLocalRequestV4",
          "appendRequest",
          MAX_LOCAL_REQUEST_ARCHIVE_BYTES);
      this.changeOpeningOwner = new SecretArchiveWiper.ChangeOpeningOwner(changeOpening);
    }

    synchronized NoteOpening takeChangeOpening() {
      if (isDestroyed()) throw new IllegalStateException("append request has been closed");
      return changeOpeningOwner.take();
    }

    @Override
    public synchronized void close() {
      destroy();
      changeOpeningOwner.close();
    }
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
    private final SecretArchiveWiper.ChangeOpeningOwner changeOpeningOwner;

    private RedeemRequestV4(final byte[] archive, final NoteOpening changeOpening) {
      super(
          archive,
          "KagemushaRecursiveSpendRedeemLocalRequestV4",
          "redeemRequest",
          MAX_LOCAL_REQUEST_ARCHIVE_BYTES);
      this.changeOpeningOwner = new SecretArchiveWiper.ChangeOpeningOwner(changeOpening);
    }

    synchronized NoteOpening takeChangeOpening() {
      if (isDestroyed()) throw new IllegalStateException("redeem request has been closed");
      return changeOpeningOwner.take();
    }

    @Override
    public synchronized void close() {
      destroy();
      changeOpeningOwner.close();
    }
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

  public static final class SplitResultV4 extends CanonicalArchive implements AutoCloseable {
    private final SecretArchiveWiper.ChangeOpeningOwner changeOpeningOwner;

    private SplitResultV4(final byte[] archive, final NoteOpening changeOpening) {
      super(
          archive,
          "KagemushaRecursiveSpendSplitResultV4",
          "splitResult",
          MAX_LOCAL_RESULT_ARCHIVE_BYTES);
      this.changeOpeningOwner = new SecretArchiveWiper.ChangeOpeningOwner(changeOpening);
    }

    synchronized NoteOpening takeChangeOpening() {
      if (isDestroyed()) throw new IllegalStateException("split result has been closed");
      return changeOpeningOwner.take();
    }

    @Override
    public synchronized void close() {
      destroy();
      changeOpeningOwner.close();
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

  public static final class RedeemBuildResultV4 extends CanonicalArchive implements AutoCloseable {
    private final SecretArchiveWiper.ChangeOpeningOwner changeOpeningOwner;

    private RedeemBuildResultV4(final byte[] archive, final NoteOpening changeOpening) {
      super(
          archive,
          "KagemushaRecursiveSpendRedeemBuildResultV4",
          "redeemBuildResult",
          MAX_LOCAL_RESULT_ARCHIVE_BYTES);
      this.changeOpeningOwner = new SecretArchiveWiper.ChangeOpeningOwner(changeOpening);
    }

    synchronized NoteOpening takeChangeOpening() {
      if (isDestroyed()) throw new IllegalStateException("redeem result has been closed");
      return changeOpeningOwner.take();
    }

    @Override
    public synchronized void close() {
      destroy();
      changeOpeningOwner.close();
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
      final NoteOpening ownedOpening = Objects.requireNonNull(opening, "opening");
      byte[] signingBytesCopy = null;
      byte[] commitmentCopy = null;
      byte[] nullifierCopy = null;
      try {
        final RecipientRequestPayload requiredPayload =
            Objects.requireNonNull(payload, "payload");
        final KagemushaScaledAmount requiredAmount = Objects.requireNonNull(amount, "amount");
        signingBytesCopy = copyRequired(signingBytes, "signingBytes");
        commitmentCopy = requireDigest(commitment, "commitment");
        nullifierCopy = requireDigest(nullifier, "nullifier");
        this.payload = requiredPayload;
        this.signingBytes = signingBytesCopy;
        this.opening = ownedOpening;
        this.commitment = commitmentCopy;
        this.nullifier = nullifierCopy;
        this.amount = requiredAmount;
      } catch (final RuntimeException | Error failure) {
        SecretArchiveWiper.wipe(nullifierCopy);
        SecretArchiveWiper.wipe(commitmentCopy);
        SecretArchiveWiper.wipe(signingBytesCopy);
        ownedOpening.close();
        throw failure;
      }
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

  public static final class FinalityCheckpointPromotionV2 {
    private final byte[] encoded;
    private final long height;

    private FinalityCheckpointPromotionV2(final byte[] value) {
      if (value == null || value.length != PROMOTED_FINALITY_CHECKPOINT_BYTES_V2) {
        throw new IllegalArgumentException("promoted checkpoint must contain exactly 40 bytes");
      }
      if ((value[0] & 0x80) != 0) {
        throw new IllegalArgumentException(
            "promoted checkpoint height exceeds the signed-64-bit client bound");
      }
      long parsedHeight = 0;
      for (int index = 0; index < 8; index++) {
        parsedHeight = (parsedHeight << 8) | (value[index] & 0xffL);
      }
      if (parsedHeight <= 0) {
        throw new IllegalArgumentException("promoted checkpoint height must be positive");
      }
      final byte[] context = Arrays.copyOfRange(value, 8, value.length);
      try {
        final byte[] checked = requireFinalityCheckpointContext(
            context, "promotedCheckpointContextId");
        Arrays.fill(checked, (byte) 0);
      } finally {
        Arrays.fill(context, (byte) 0);
      }
      this.encoded = Arrays.copyOf(value, value.length);
      this.height = parsedHeight;
    }

    public byte[] encoded() { return Arrays.copyOf(encoded, encoded.length); }
    public long height() { return height; }
    public byte[] contextId() { return Arrays.copyOfRange(encoded, 8, encoded.length); }
  }

  public static final class VerifiedRecipientRegistrationLineageV2 {
    private final RecipientRegistrationLineage lineage;
    private final FinalityCheckpointPromotionV2 promotedCheckpoint;

    private VerifiedRecipientRegistrationLineageV2(
        final RecipientRegistrationLineage lineage,
        final FinalityCheckpointPromotionV2 promotedCheckpoint) {
      this.lineage = Objects.requireNonNull(lineage, "lineage");
      this.promotedCheckpoint = Objects.requireNonNull(promotedCheckpoint, "promotedCheckpoint");
    }

    public RecipientRegistrationLineage lineage() { return lineage; }
    public FinalityCheckpointPromotionV2 promotedCheckpoint() { return promotedCheckpoint; }
  }

  public static final class RecipientReceiveOfferProjectionV2 {
    private final RecipientPaymentRequest request;
    private final RecipientRegistrationLineage lineage;
    private final byte[] publisherCheckpointEnvelope;

    private RecipientReceiveOfferProjectionV2(
        final RecipientPaymentRequest request,
        final RecipientRegistrationLineage lineage,
        final byte[] publisherCheckpointEnvelope) {
      this.request = Objects.requireNonNull(request, "request");
      this.lineage = Objects.requireNonNull(lineage, "lineage");
      this.publisherCheckpointEnvelope = requireBoundedBytes(
          publisherCheckpointEnvelope,
          "publisherCheckpointEnvelope",
          MAX_PUBLISHER_CHECKPOINT_ENVELOPE_BYTES_V1);
    }

    public RecipientPaymentRequest request() { return request; }
    public RecipientRegistrationLineage lineage() { return lineage; }
    public byte[] publisherCheckpointEnvelope() {
      return Arrays.copyOf(publisherCheckpointEnvelope, publisherCheckpointEnvelope.length);
    }
  }

  public static final class VerifiedRecipientReceiveOfferV2 {
    private final RecipientPaymentRequest request;
    private final RecipientRegistrationLineage lineage;
    private final byte[] publisherCheckpointEnvelope;
    private final FinalityCheckpointPromotionV2 promotedCheckpoint;
    private final long verifiedAtMilliseconds;

    private VerifiedRecipientReceiveOfferV2(
        final RecipientPaymentRequest request,
        final RecipientRegistrationLineage lineage,
        final byte[] publisherCheckpointEnvelope,
        final FinalityCheckpointPromotionV2 promotedCheckpoint,
        final long verifiedAtMilliseconds) {
      if (verifiedAtMilliseconds <= 0) {
        throw new IllegalArgumentException("verifiedAtMilliseconds must be positive");
      }
      this.request = Objects.requireNonNull(request, "request");
      this.lineage = Objects.requireNonNull(lineage, "lineage");
      this.publisherCheckpointEnvelope = requireBoundedBytes(
          publisherCheckpointEnvelope,
          "publisherCheckpointEnvelope",
          MAX_PUBLISHER_CHECKPOINT_ENVELOPE_BYTES_V1);
      this.promotedCheckpoint = Objects.requireNonNull(promotedCheckpoint, "promotedCheckpoint");
      this.verifiedAtMilliseconds = verifiedAtMilliseconds;
    }

    public RecipientPaymentRequest request() { return request; }
    public RecipientRegistrationLineage lineage() { return lineage; }
    public byte[] publisherCheckpointEnvelope() {
      return Arrays.copyOf(publisherCheckpointEnvelope, publisherCheckpointEnvelope.length);
    }
    public FinalityCheckpointPromotionV2 promotedCheckpoint() { return promotedCheckpoint; }
    public long verifiedAtMilliseconds() { return verifiedAtMilliseconds; }
  }

  public static final class RecipientRequestProjection {
    private final NetworkId networkId;
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
        final NetworkId networkId,
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
      this.networkId = Objects.requireNonNull(networkId, "networkId");
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

    public NetworkId networkId() { return networkId; }
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
    private final RequestAuthorizationPreparationArchive archive;
    private final byte[] signingBytes;
    private final byte[] operationId;
    private final byte[] payloadDigest;
    private final byte[] registrationHash;

    private RequestAuthorizationPreparation(
        final RequestAuthorizationPreparationArchive archive,
        final byte[] signingBytes,
        final byte[] operationId,
        final byte[] payloadDigest,
        final byte[] registrationHash) {
      this.archive = archive;
      this.signingBytes = copyRequired(signingBytes, "signingBytes");
      this.operationId = requireDigest(operationId, "operationId");
      this.payloadDigest = requireDigest(payloadDigest, "payloadDigest");
      this.registrationHash = requireDigest(registrationHash, "registrationHash");
    }

    public byte[] signingBytes() { return Arrays.copyOf(signingBytes, signingBytes.length); }
    public byte[] operationId() { return Arrays.copyOf(operationId, operationId.length); }
    public byte[] payloadDigest() { return Arrays.copyOf(payloadDigest, payloadDigest.length); }
    public byte[] registrationHash() {
      return Arrays.copyOf(registrationHash, registrationHash.length);
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
      final NoteOpening ownedOpening = Objects.requireNonNull(opening, "opening");
      byte[] authorizationDigestCopy = null;
      byte[] noteCommitmentCopy = null;
      byte[] spendNullifierCopy = null;
      byte[] initialRootCopy = null;
      byte[] finalizedRootCopy = null;
      byte[] operationIdCopy = null;
      try {
        final TopUpUnsigned requiredUnsigned = Objects.requireNonNull(unsigned, "unsigned");
        final KagemushaScaledAmount requiredAmount = Objects.requireNonNull(amount, "amount");
        authorizationDigestCopy = requireDigest(authorizationDigest, "authorizationDigest");
        noteCommitmentCopy = requireDigest(noteCommitment, "noteCommitment");
        spendNullifierCopy = requireDigest(spendNullifier, "spendNullifier");
        initialRootCopy = requireDigest(initialRoot, "initialRoot");
        finalizedRootCopy = requireDigest(finalizedRoot, "finalizedRoot");
        operationIdCopy = requireDigest(operationId, "operationId");
        this.unsigned = requiredUnsigned;
        this.authorizationDigest = authorizationDigestCopy;
        this.opening = ownedOpening;
        this.noteCommitment = noteCommitmentCopy;
        this.spendNullifier = spendNullifierCopy;
        this.initialRoot = initialRootCopy;
        this.finalizedRoot = finalizedRootCopy;
        this.operationId = operationIdCopy;
        this.amount = requiredAmount;
        this.leafIndex = leafIndex;
      } catch (final RuntimeException | Error failure) {
        SecretArchiveWiper.wipe(operationIdCopy);
        SecretArchiveWiper.wipe(finalizedRootCopy);
        SecretArchiveWiper.wipe(initialRootCopy);
        SecretArchiveWiper.wipe(spendNullifierCopy);
        SecretArchiveWiper.wipe(noteCommitmentCopy);
        SecretArchiveWiper.wipe(authorizationDigestCopy);
        ownedOpening.close();
        throw failure;
      }
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
    private final BundleV4 bundle;
    private final NoteMembershipWitness membershipWitness;
    private final byte[] commitment;
    private final byte[] spendNullifier;
    private final KagemushaScaledAmount amount;
    private final int hopCount;
    private final int proofStepCount;
    private final byte[] bundleDigest;
    private final ArtifactBindingV4 artifactBinding;
    private final List<BranchClaim> branchClaims;

    private BranchProjection(
        final BundleV4 bundle,
        final NoteMembershipWitness membershipWitness,
        final byte[] commitment,
        final byte[] spendNullifier,
        final KagemushaScaledAmount amount,
        final int hopCount,
        final int proofStepCount,
        final byte[] bundleDigest,
        final ArtifactBindingV4 artifactBinding,
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

    public BundleV4 bundle() { return bundle; }
    public NoteMembershipWitness membershipWitness() { return membershipWitness; }
    public byte[] commitment() { return Arrays.copyOf(commitment, commitment.length); }
    public byte[] spendNullifier() { return Arrays.copyOf(spendNullifier, spendNullifier.length); }
    public byte[] bundleDigest() { return Arrays.copyOf(bundleDigest, bundleDigest.length); }
    public ArtifactBindingV4 artifactBinding() { return artifactBinding; }
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

  /** Secret-bearing local state used only by the genuine ABI-21 builders. */
  public static final class SpendableBranchV4 implements AutoCloseable {
    private final BundleV4 bundle;
    private final NoteMembershipWitness membershipWitness;
    private final NoteOpening opening;
    private final TopUpProvenanceV4 topUpProvenance;
    private final OutputMembershipFrontierV4 frontier;

    private SpendableBranchV4(
        final BundleV4 bundle,
        final NoteMembershipWitness membershipWitness,
        final NoteOpening opening,
        final TopUpProvenanceV4 topUpProvenance,
        final OutputMembershipFrontierV4 frontier) {
      this.bundle = Objects.requireNonNull(bundle, "bundle");
      this.membershipWitness = Objects.requireNonNull(membershipWitness, "membershipWitness");
      this.opening = Objects.requireNonNull(opening, "opening");
      this.topUpProvenance = Objects.requireNonNull(topUpProvenance, "topUpProvenance");
      this.frontier = Objects.requireNonNull(frontier, "frontier");
    }

    public BundleV4 bundle() { return bundle; }
    public NoteMembershipWitness membershipWitness() { return membershipWitness; }
    public NoteOpening opening() { return opening; }
    public TopUpProvenanceV4 topUpProvenance() { return topUpProvenance; }
    public OutputMembershipFrontierV4 frontier() { return frontier; }

    /** Destroy the locally held secret opening; public proof artifacts remain immutable. */
    @Override
    public void close() {
      opening.destroy();
    }
  }

  public static final class PeerPaymentProjection {
    private final BranchProjection branch;
    private final TopUpProvenanceV4 topUpProvenance;
    private final byte[] operationId;
    private final byte[] requestDigest;

    private PeerPaymentProjection(
        final BranchProjection branch,
        final TopUpProvenanceV4 topUpProvenance,
        final byte[] operationId,
        final byte[] requestDigest) {
      this.branch = Objects.requireNonNull(branch, "branch");
      this.topUpProvenance = Objects.requireNonNull(topUpProvenance, "topUpProvenance");
      this.operationId = requireDigest(operationId, "operationId");
      this.requestDigest = requireDigest(requestDigest, "requestDigest");
    }

    public BranchProjection branch() { return branch; }
    public TopUpProvenanceV4 topUpProvenance() { return topUpProvenance; }
    public byte[] operationId() { return Arrays.copyOf(operationId, operationId.length); }
    public byte[] requestDigest() { return Arrays.copyOf(requestDigest, requestDigest.length); }
  }

  public static final class InitProjectionV4 {
    private final BranchProjection branch;
    private final TopUpProvenanceV4 topUpProvenance;
    private final byte[] publicStatementDigest;

    private InitProjectionV4(
        final BranchProjection branch,
        final TopUpProvenanceV4 topUpProvenance,
        final byte[] publicStatementDigest) {
      this.branch = Objects.requireNonNull(branch, "branch");
      this.topUpProvenance = Objects.requireNonNull(topUpProvenance, "topUpProvenance");
      this.publicStatementDigest =
          requireDigest(publicStatementDigest, "publicStatementDigest");
    }

    public BranchProjection branch() { return branch; }
    public BundleV4 bundle() { return branch.bundle(); }
    public TopUpProvenanceV4 topUpProvenance() { return topUpProvenance; }
    public byte[] publicStatementDigest() {
      return Arrays.copyOf(publicStatementDigest, publicStatementDigest.length);
    }
  }

  public static final class SplitProjection {
    private final PeerPayment peerPayment;
    private final BranchProjection recipient;
    private final BranchProjection change;
    private final TopUpProvenanceV4 recipientTopUpProvenance;
    private final TopUpProvenanceV4 changeTopUpProvenance;
    private final byte[] operationId;
    private final byte[] requestDigest;
    private final byte[] splitBindingDigest;

    private SplitProjection(
        final PeerPayment peerPayment,
        final BranchProjection recipient,
        final BranchProjection change,
        final TopUpProvenanceV4 recipientTopUpProvenance,
        final TopUpProvenanceV4 changeTopUpProvenance,
        final byte[] operationId,
        final byte[] requestDigest,
        final byte[] splitBindingDigest) {
      this.peerPayment = peerPayment;
      this.recipient = recipient;
      this.change = change;
      this.recipientTopUpProvenance =
          Objects.requireNonNull(recipientTopUpProvenance, "recipientTopUpProvenance");
      if ((change == null) != (changeTopUpProvenance == null)) {
        throw new IllegalStateException(
            "native Kagemusha change provenance does not match change projection");
      }
      this.changeTopUpProvenance = changeTopUpProvenance;
      this.operationId = requireDigest(operationId, "operationId");
      this.requestDigest = requireDigest(requestDigest, "requestDigest");
      this.splitBindingDigest = requireDigest(splitBindingDigest, "splitBindingDigest");
    }

    public PeerPayment peerPayment() { return peerPayment; }
    public BranchProjection recipient() { return recipient; }
    public BranchProjection change() { return change; }
    public TopUpProvenanceV4 recipientTopUpProvenance() { return recipientTopUpProvenance; }
    public TopUpProvenanceV4 changeTopUpProvenance() { return changeTopUpProvenance; }
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
    private final ArtifactBindingV4 artifactBinding;
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
        final ArtifactBindingV4 artifactBinding, final byte[] requestDigest,
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
    public ArtifactBindingV4 artifactBinding() { return artifactBinding; }
    public byte[] requestDigest() { return Arrays.copyOf(requestDigest, requestDigest.length); }
    public byte[] outputBindingDigest() { return Arrays.copyOf(outputBindingDigest, outputBindingDigest.length); }
    public List<BranchClaim> branchClaims() { return branchClaims; }
  }

  public static final class RedeemBuildProjection {
    private final RedeemUnsignedV4 unsigned;
    private final byte[] authorizationDigest;
    private final BranchProjection change;
    private final TopUpProvenanceV4 changeTopUpProvenance;
    private final byte[] operationId;

    private RedeemBuildProjection(
        final RedeemUnsignedV4 unsigned, final byte[] authorizationDigest,
        final BranchProjection change, final TopUpProvenanceV4 changeTopUpProvenance,
        final byte[] operationId) {
      this.unsigned = Objects.requireNonNull(unsigned, "unsigned");
      this.authorizationDigest = requireDigest(authorizationDigest, "authorizationDigest");
      if ((change == null) != (changeTopUpProvenance == null)) {
        throw new IllegalStateException(
            "native Kagemusha redemption change provenance does not match change projection");
      }
      this.change = change;
      this.changeTopUpProvenance = changeTopUpProvenance;
      this.operationId = requireDigest(operationId, "operationId");
    }

    public RedeemUnsignedV4 unsigned() { return unsigned; }
    public byte[] authorizationDigest() { return Arrays.copyOf(authorizationDigest, authorizationDigest.length); }
    public BranchProjection change() { return change; }
    public TopUpProvenanceV4 changeTopUpProvenance() { return changeTopUpProvenance; }
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

  /** Delivery-receipt evidence for an already-final sender cash handoff. */
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

  /** Asset-neutral offline protocol capability implemented by every app-api node. */
  public static final class OfflineStatus {
    private static final List<String> FIELDS = Collections.unmodifiableList(
        Arrays.asList(
            "cash_handoff_capability",
            "required_bridge_abi_version",
            "max_hops",
            "ready"));

    private final String cashHandoffCapability;
    private final int requiredBridgeAbiVersion;
    private final int maximumHops;
    private final boolean ready;

    private OfflineStatus(
        final String cashHandoffCapability,
        final int requiredBridgeAbiVersion,
        final int maximumHops,
        final boolean ready) {
      if (!CASH_HANDOFF_CAPABILITY_V1.equals(cashHandoffCapability)) {
        throw new IllegalArgumentException(
            "cashHandoffCapability must be the exact cash_handoff_v1 contract");
      }
      if (requiredBridgeAbiVersion != REQUIRED_NATIVE_BRIDGE_ABI_VERSION) {
        throw new IllegalArgumentException("requiredBridgeAbiVersion must be 23");
      }
      if (maximumHops != MAXIMUM_PEER_HOPS) {
        throw new IllegalArgumentException(
            "maximumHops must match the cash_handoff_v1 bound");
      }
      if (!ready) {
        throw new IllegalArgumentException(
            "ready must be true for universal offline capability");
      }
      this.cashHandoffCapability = cashHandoffCapability;
      this.requiredBridgeAbiVersion = requiredBridgeAbiVersion;
      this.maximumHops = maximumHops;
      this.ready = ready;
    }

    private static OfflineStatus decode(final byte[] payload) {
      final Object parsed = JsonParser.parse(new String(payload, StandardCharsets.UTF_8));
      if (!(parsed instanceof Map<?, ?> root)
          || root.size() != FIELDS.size()
          || !root.keySet().containsAll(FIELDS)) {
        throw new IllegalStateException(
            "offline capability response must contain exactly the universal fields");
      }
      final Object capabilityValue = root.get("cash_handoff_capability");
      final Object readyValue = root.get("ready");
      if (!(capabilityValue instanceof String capability)
          || !(readyValue instanceof Boolean ready)) {
        throw new IllegalStateException(
            "offline capability response has an invalid universal field type");
      }
      return new OfflineStatus(
          capability,
          JsonNumbers.asInt(
              root.get("required_bridge_abi_version"),
              "offline capability required_bridge_abi_version"),
          JsonNumbers.asInt(root.get("max_hops"), "offline capability max_hops"),
          ready);
    }

    public String cashHandoffCapability() { return cashHandoffCapability; }
    public int requiredBridgeAbiVersion() { return requiredBridgeAbiVersion; }
    public int maximumHops() { return maximumHops; }
    public boolean ready() { return ready; }
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
      this.code = requireStableOperationCode(code);
      this.message = requireCanonicalOperationMessage(message);
    }

    public String code() { return code; }
    public String message() { return message; }
  }

  public static final class FinalizedTopUp {
    private final TopUpAnchorV4 anchor;
    private final TopUpFinalityProof finalityProof;
    private final long finalizedBlockHeight;
    private final long serverTimeMilliseconds;

    private FinalizedTopUp(
        final TopUpAnchorV4 anchor,
        final TopUpFinalityProof finalityProof,
        final long finalizedBlockHeight,
        final long serverTimeMilliseconds) {
      this.anchor = anchor;
      this.finalityProof = finalityProof;
      if (finalizedBlockHeight <= 0 || serverTimeMilliseconds <= 0) {
        throw new IllegalArgumentException("finalized top-up times must be positive");
      }
      this.finalizedBlockHeight = finalizedBlockHeight;
      this.serverTimeMilliseconds = serverTimeMilliseconds;
    }

    public TopUpAnchorV4 anchor() { return anchor; }
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
      this.operationId = requireDigest(operationId, "operationId");
      this.transactionHash = requireDigest(transactionHash, "transactionHash");
      if (state == null || kind == null) {
        throw new IllegalArgumentException("operation state and kind must be present");
      }
      final boolean valid = switch (state) {
        case PENDING -> submittedAtMilliseconds != null && submittedAtMilliseconds > 0
            && finalizedBlockHeight == null && serverTimeMilliseconds == null
            && finalizedTopUp == null && rejection == null;
        case APPLIED -> submittedAtMilliseconds == null
            && finalizedBlockHeight != null && finalizedBlockHeight > 0
            && serverTimeMilliseconds != null && serverTimeMilliseconds > 0
            && rejection == null
            && ((kind == OperationKind.TOP_UP && finalizedTopUp != null
                    && finalizedTopUp.finalizedBlockHeight == finalizedBlockHeight
                    && finalizedTopUp.serverTimeMilliseconds == serverTimeMilliseconds)
                || (kind == OperationKind.REDEEM && finalizedTopUp == null));
        case REJECTED -> submittedAtMilliseconds == null && finalizedBlockHeight == null
            && serverTimeMilliseconds == null && finalizedTopUp == null && rejection != null;
      };
      if (!valid) {
        throw new IllegalArgumentException("operation status fields are inconsistent");
      }
      this.state = state;
      this.kind = kind;
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

  /** Strict typed client for the five first-release Kagemusha Torii routes. */
  public static final class ToriiClient {
    public static final String CAPABILITY_PATH = "/v1/offline/readiness";
    public static final String TOP_UP_PATH = "/v1/offline/top-up";
    public static final String REDEEM_PATH = "/v1/offline/redeem";
    public static final String OPERATIONS_PATH = "/v1/offline/operations";
    public static final String RECEIVER_LINEAGE_PATH = "/v1/offline/receiver-lineage";
    public static final String JSON_MEDIA_TYPE = "application/json";
    public static final String NORITO_MEDIA_TYPE = "application/x-norito";
    private static final String UNSIGNED_LONG_MAX = "18446744073709551615";

    private final String baseUri;
    private final TransportExecutor transport;
    private final LocalSigningContext localSigningContext;

    private ToriiClient(final URI baseUri, final TransportExecutor transport,
        final LocalSigningContext localSigningContext) {
      Objects.requireNonNull(baseUri, "baseUri");
      this.transport = Objects.requireNonNull(transport, "transport");
      this.localSigningContext = Objects.requireNonNull(localSigningContext, "localSigningContext");
      if (!baseUri.isAbsolute()
          || baseUri.isOpaque()
          || baseUri.getHost() == null
          || baseUri.getHost().isEmpty()
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

    public CompletableFuture<OfflineStatus> getOfflineCapability() {
      return execute(
              TransportRequest.builder()
                  .setMethod("GET")
                  .setUri(URI.create(baseUri + CAPABILITY_PATH))
                  .addHeader("Accept", JSON_MEDIA_TYPE)
                  .setMaximumResponseBytes((long) MAX_TORII_RESPONSE_BYTES)
                  .build(),
              200,
              JSON_MEDIA_TYPE)
          .thenApply(response -> OfflineStatus.decode(response.body()));
    }

    public CompletableFuture<RecipientRegistrationLineage> getRecipientRegistrationLineage(
        final RecipientLineageQueryV2 query, final ToriiCanonicalRequestAuth canonicalAuth) {
      return execute(
              KagemushaToriiLineageRequest.build(
                  baseUri, query, localSigningContext, canonicalAuth),
              200)
          .thenApply(response -> new RecipientRegistrationLineage(response.body()));
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
          .thenApply(
              response -> {
                requireCommandResponseHeaders(response, id);
                return new OperationReference(response.body());
              });
    }

    private static void requireCommandResponseHeaders(
        final TransportResponse response, final String operationId) {
      final String expectedLocation = OPERATIONS_PATH + "/" + operationId;
      final List<String> locations =
          response.headers().getOrDefault("Location", Collections.emptyList());
      if (locations.size() != 1 || !expectedLocation.equals(locations.get(0))) {
        throw new IllegalStateException(
            "Kagemusha Torii Location must match the canonical operation resource");
      }
      final List<String> retryAfterValues =
          response.headers().getOrDefault("Retry-After", Collections.emptyList());
      if (retryAfterValues.size() != 1) {
        throw new IllegalStateException(
            "Kagemusha Torii Retry-After must occur exactly once");
      }
      final String retryAfter = retryAfterValues.get(0);
      if (retryAfter.isEmpty() || retryAfter.length() > 20) {
        throw new IllegalStateException(
            "Kagemusha Torii Retry-After must be a positive u64 delay");
      }
      int firstSignificant = 0;
      for (int index = 0; index < retryAfter.length(); index++) {
        final char character = retryAfter.charAt(index);
        if (character < '0' || character > '9') {
          throw new IllegalStateException(
              "Kagemusha Torii Retry-After must be a positive u64 delay");
        }
        if (firstSignificant == index && character == '0') firstSignificant++;
      }
      final String significant = retryAfter.substring(firstSignificant);
      if (significant.isEmpty()
          || significant.length() > UNSIGNED_LONG_MAX.length()
          || (significant.length() == UNSIGNED_LONG_MAX.length()
              && significant.compareTo(UNSIGNED_LONG_MAX) > 0)) {
        throw new IllegalStateException(
            "Kagemusha Torii Retry-After must be a positive u64 delay");
      }
    }

    private CompletableFuture<TransportResponse> execute(
        final TransportRequest request, final int expectedStatus) {
      return execute(request, expectedStatus, NORITO_MEDIA_TYPE);
    }

    private CompletableFuture<TransportResponse> execute(
        final TransportRequest request,
        final int expectedStatus,
        final String expectedMediaType) {
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
                    || !expectedMediaType.equalsIgnoreCase(contentTypes.get(0))) {
                  throw new IllegalStateException(
                      "Kagemusha Torii response must use " + expectedMediaType);
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

    public void finish() {
      withHeavyProofPermit("artifact finalization", () -> {
        synchronized (this) {
          requireOpen(false);
          nativeArtifactFinalizeV4(handle);
          finalized = true;
        }
        return Boolean.TRUE;
      });
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
   * downloaded release. Native code authenticates the runner-signed internal-validation receipt,
   * verifies the signed role thresholds, and hashes both external evidence files before validating
   * the candidate-bound promotion record and consuming any finalized artifact handle.
   */
  public static final class ReleaseAuthentication {
    private final byte[] trustedPolicyNorito;
    private final byte[] releaseAttestationNorito;
    private final byte[] internalValidationReceiptNorito;
    private final byte[] benchmarkEvidence;
    private final byte[] cryptographicReview;
    private final byte[] promotionRecordNorito;

    public ReleaseAuthentication(
        final byte[] trustedPolicyNorito,
        final byte[] releaseAttestationNorito,
        final byte[] internalValidationReceiptNorito,
        final byte[] benchmarkEvidence,
        final byte[] cryptographicReview,
        final byte[] promotionRecordNorito) {
      this.trustedPolicyNorito = requireBoundedBytes(
          trustedPolicyNorito,
          "trustedPolicyNorito",
          MAX_TRUSTED_RELEASE_POLICY_BYTES);
      this.releaseAttestationNorito = requireBoundedBytes(
          releaseAttestationNorito,
          "releaseAttestationNorito",
          MAX_RELEASE_ATTESTATION_BYTES);
      this.internalValidationReceiptNorito = requireBoundedBytes(
          internalValidationReceiptNorito,
          "internalValidationReceiptNorito",
          MAX_INTERNAL_VALIDATION_RECEIPT_BYTES);
      this.benchmarkEvidence = requireBoundedBytes(
          benchmarkEvidence,
          "benchmarkEvidence",
          MAX_RELEASE_EVIDENCE_BYTES);
      this.cryptographicReview = requireBoundedBytes(
          cryptographicReview,
          "cryptographicReview",
          MAX_CRYPTOGRAPHIC_REVIEW_BYTES);
      this.promotionRecordNorito = requireBoundedBytes(
          promotionRecordNorito,
          "promotionRecordNorito",
          MAX_PROMOTION_RECORD_BYTES);
    }
  }

  /** Coordinates one authenticated, atomic eight-artifact generation install. */
  public static final class ArtifactInstallSession implements AutoCloseable {
    private final byte[] manifestNorito;
    private final byte[] manifestSha256;
    private final byte[] trustedPolicyNorito;
    private final byte[] releaseAttestationNorito;
    private final byte[] internalValidationReceiptNorito;
    private final byte[] benchmarkEvidence;
    private final byte[] cryptographicReview;
    private final byte[] promotionRecordNorito;
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
      this.internalValidationReceiptNorito = Arrays.copyOf(
          releaseAuthentication.internalValidationReceiptNorito,
          releaseAuthentication.internalValidationReceiptNorito.length);
      this.benchmarkEvidence = Arrays.copyOf(
          releaseAuthentication.benchmarkEvidence,
          releaseAuthentication.benchmarkEvidence.length);
      this.cryptographicReview = Arrays.copyOf(
          releaseAuthentication.cryptographicReview,
          releaseAuthentication.cryptographicReview.length);
      this.promotionRecordNorito = Arrays.copyOf(
          releaseAuthentication.promotionRecordNorito,
          releaseAuthentication.promotionRecordNorito.length);
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

    public void install() {
      withHeavyProofPermit("artifact install", () -> {
        synchronized (this) {
          requirePending();
          if (artifacts.size() != ARTIFACT_COUNT) {
            throw new IllegalStateException("artifact set must contain exactly eight streams");
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
                internalValidationReceiptNorito,
                benchmarkEvidence,
                cryptographicReview,
                promotionRecordNorito,
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
        return Boolean.TRUE;
      });
    }

    public synchronized boolean isInstalled() {
      return !closed && nativeArtifactSetIsInstalledV4(manifestNorito, manifestSha256);
    }

    public synchronized ArtifactBindingV4 artifactBinding() {
      if (!installed || closed || !isInstalled()) {
        throw new IllegalStateException("artifact set is not installed");
      }
      return new ArtifactBindingV4(
          nativeBuildArtifactBindingV4(manifestNorito, manifestSha256));
    }

    public void uninstall() {
      withHeavyProofPermit("artifact uninstall", () -> {
        synchronized (this) {
          if (installed && !closed) {
            nativeArtifactSetUninstallV4(manifestSha256);
            installed = false;
            closed = true;
          }
        }
        return Boolean.TRUE;
      });
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
      byte[] internalValidationReceiptNorito,
      byte[] benchmarkEvidence,
      byte[] cryptographicReview,
      byte[] promotionRecordNorito,
      long[] artifactHandles);

  private static native boolean nativeArtifactSetIsInstalledV4(
      byte[] manifestNorito, byte[] manifestSha256);
  private static native byte[] nativeInstalledManifestSha256V4();
  private static native byte[] nativeBuildArtifactBindingV4(
      byte[] manifestNorito, byte[] manifestSha256);

  private static native void nativeArtifactSetUninstallV4(byte[] manifestSha256);

  private static native byte[] nativeInitSpendV4(byte[] requestNorito);

  private static native byte[] nativeAppendSpendV4(
      byte[] requestNorito, byte[] recipientRequestNorito, long verifiedAtMilliseconds);

  private static native byte[] nativeVerifySpendV4(byte[] requestNorito);

  private static native byte[] nativeBuildRedeemV4(byte[] requestNorito);

  private static native byte[][] nativePrepareRecipientRequestV2(
      byte[] networkId, int chainDiscriminant, byte[] asset, byte[] atomicUnits, int scale, byte[] recipient,
      byte[] receiverDeviceId, byte[] receiverPublicKey, byte[] requestId,
      long issuedAtMilliseconds, long expiresAtMilliseconds, byte[] spendKey, byte[] rho,
      byte[] diversifier);
  private static native byte[] nativeCreateRecipientRequestV2(byte[] payload, byte[] signature);
  private static native byte[] nativeVerifyRecipientRequestV2(byte[] request, long verifiedAtMilliseconds);
  private static native byte[] nativeCreateRecipientLineageQueryV2(
      byte[] networkId,
      int chainDiscriminant,
      byte[] recipient,
      byte[] receiverDeviceId,
      byte[] asset,
      long trustedCheckpointHeight);
  private static native byte[][] nativeVerifyRecipientRegistrationLineageV2(
      byte[] request,
      byte[] lineage,
      long verifiedAtMilliseconds,
      long trustedCheckpointHeight,
      byte[] trustedCheckpointContextId);
  private static native byte[] nativeCreateRecipientReceiveOfferV2(
      byte[] request, byte[] lineage, byte[] publisherCheckpointEnvelope);
  private static native byte[][] nativeProjectRecipientReceiveOfferV2(byte[] offer);
  private static native byte[][] nativeVerifyRecipientReceiveOfferV2(
      byte[] offer,
      long verifiedAtMilliseconds,
      long trustedCheckpointHeight,
      byte[] trustedCheckpointContextId);
  private static native byte[] nativeBuildOutputMembershipFrontierV4(
      int leafIndex, byte[] flattenedSiblings, byte[] directions, byte[] root);
  private static native byte[][] nativeDeriveOutputMembershipPathsV4(
      byte[] frontier, byte[] recipientCommitment, byte[] changeCommitment);
  private static native byte[] nativeValidateSpendableBranchV4(
      byte[] bundle, byte[] provenance, byte[] membershipWitness, byte[] opening,
      long blockHeight);
  private static native byte[] nativeBuildOutputMembershipPathsV4(
      byte[] initialRoot, byte[] finalRoot, byte[][] recipientFields,
      byte[][] changeFields, byte[][] dummyFields);
  private static native byte[] nativeBuildInitRequestV4(
      byte[] anchor, byte[] proof, byte[] roster, byte[] opening, byte[] outputMembership);
  private static native byte[] nativeBuildTopUpProvenanceV4(
      byte[] bundle, byte[] roster, byte[][] anchors, byte[][] finalityProofs, long blockHeight);
  private static native byte[] nativeValidateTopUpProvenanceV4(
      byte[] bundle, byte[] provenance, long blockHeight);
  private static native byte[] nativeBuildAppendRequestV4(
      byte[][] bundles, byte[][] topUpProvenances, byte[][] openings, byte[][] witnesses,
      byte[] changeOpening, byte[] outputMembership, byte[] verifierCommitment,
      byte[] operationId, long blockHeight);
  private static native byte[][] nativeProjectPeerPaymentV4(byte[] payment);
  private static native byte[][] nativeProjectInitResultV4(byte[] result);
  private static native byte[][] nativeProjectSplitResultV4(byte[] result);
  private static native byte[] nativeBuildVerifyRequestV4(
      byte[] bundle, byte[] recipientRequest, byte[] topUpProvenance,
      int maximumHops, long blockHeight, long verifiedAtMilliseconds);
  private static native byte[][] nativeProjectVerifyResultV4(byte[] result);
  private static native byte[] nativeBuildRedeemRequestV4(
      byte[] bundle, byte[] topUpProvenance, byte[] opening, byte[] membershipWitness,
      byte[] recipient, int chainDiscriminant,
      byte[] atomicUnits, int scale, byte[] changeOpening, byte[] changeOutputMembership,
      byte[] verifierCommitment, byte[] operationId, long blockHeight);
  private static native byte[][] nativeProjectRedeemBuildResultV4(byte[] result);
  private static native byte[][] nativePrepareAcknowledgementV2(
      byte[] request, byte[] payment, long acceptedAtMilliseconds);
  private static native byte[] nativeCreateAcknowledgementV2(
      byte[] payload, byte[] signature, byte[] request, byte[] payment);
  private static native byte[][] nativeVerifyAcknowledgementV2(
      byte[] acknowledgement, byte[] request, byte[] payment);
  private static native byte[][] nativePrepareAuthorizationV2(
      byte[] authority, int chainDiscriminant, byte[] deviceId, byte[] assetDefinitionId, byte[] operationId,
      long issuedAtMilliseconds, long expiresAtMilliseconds, byte[] nonce, byte[] payloadDigest,
      byte[] registrationHash, byte[] hardwareAssertionPlatform);
  private static native byte[][] nativeFinalizeHardwareAuthorizationV2(
      byte[] preparation, byte[] authenticatorData, byte[] signatureDer);
  private static native byte[][] nativeFinalizeIosAppAttestAuthorizationV2(
      byte[] preparation, byte[] assertionObject);
  private static native byte[] nativeFinalizeTopUpV4(byte[] unsigned, byte[] authorization);
  private static native byte[][] nativeFinalizeRedeemV4(byte[] buildResult, byte[] authorization);
  private static native byte[][] nativePrepareTopUpV4(
      byte[] networkId, int chainDiscriminant, byte[] assetDefinition, byte[] payer, byte[] atomicUnits, int scale,
      byte[] operationId, byte[] spendKey, byte[] rho, byte[] diversifier, int leafIndex,
      byte[] flattenedSiblings, byte[] directions, byte[] root,
      byte[] shieldVerifierCommitment, byte[] artifactBinding);
  private static native byte[][] nativeProjectOperationStatusV4(byte[] status);
  private static native boolean nativeBranchClaimsConflictV2(byte[] left, byte[] right);
  private static native byte[][] nativePrepareRedemptionChangeV4(
      byte[] bundle, byte[] inputOpening, byte[] atomicUnits, int scale,
      byte[] operationId, byte[] entropy);
  private static native byte[][] nativePreparePeerSplitChangeV4(
      byte[][] bundles,
      byte[][] inputOpenings,
      byte[] recipientRequest,
      byte[] atomicUnits,
      int scale,
      byte[] operationId,
      byte[] entropy);
  private static native byte[] nativePrepareNoteOpeningV2(
      byte[] spendKey, byte[] rho, byte[] diversifier);
  private static native byte[][] nativeProjectRecipientRequestV2(byte[] request);
}
