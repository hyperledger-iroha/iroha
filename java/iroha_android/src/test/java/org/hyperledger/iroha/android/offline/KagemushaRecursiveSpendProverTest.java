package org.hyperledger.iroha.android.offline;

import java.net.URI;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.atomic.AtomicReference;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.norito.CRC64;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;

/** Source-level contract checks for the typed ABI-20 Kagemusha V4 lifecycle bridge. */
public final class KagemushaRecursiveSpendProverTest {
  public static void main(final String[] args) {
    exactAbiIsRequired();
    artifactContractIsFixed();
    frontierContractIsPersistableAndProofBound();
    outputMembershipPathsRejectNonconsecutiveDummyFrontier();
    artifactRoleInventoryRejectsCountsDuplicatesAndReordering();
    releaseAuthenticationIsMandatoryAndBounded();
    readinessPreservesExactReleaseCapabilitiesIndependently();
    canonicalPeerCodecsAreTypedAndDefensive();
    lifecycleArchivesAreTypedDefensiveAndFailClosed();
    branchRestoreRejectsMissingHeightAndLocalChangeOpeningsBeforeNativeDispatch();
    topUpProvenanceArchiveIsCanonicalBoundedAndDefensive();
    appendJoinRejectsZeroAndThreeInputsBeforeNativeDispatch();
    scaledAmountsAreExactAndNeverRound();
    peerTransportGoldenVectorsAreExact();
    qrNfcAndNearbyGoldenVectorsAreExact();
    nfcV4StreamsBeyondLegacyLimitAndRejectsDowngrade();
    toriiLifecycleRoutesAndHeadersAreExact();
    publicSurfaceIsKagemushaOnly();
  }

  private static void exactAbiIsRequired() {
    assert KagemushaRecursiveSpendProver.isExactBridgeAbi(20);
    assert !KagemushaRecursiveSpendProver.isExactBridgeAbi(19);
    assert KagemushaRecursiveSpendProver.detectExactNativeAvailability(
        () -> {}, () -> 20, () -> true);
    assert !KagemushaRecursiveSpendProver.detectExactNativeAvailability(
        () -> {}, () -> 20, () -> false);
    assert !KagemushaRecursiveSpendProver.detectExactNativeAvailability(
        () -> { throw new UnsatisfiedLinkError("missing"); }, () -> 20, () -> true);
  }

  private static void artifactContractIsFixed() {
    assert KagemushaRecursiveSpendProver.REQUIRED_NATIVE_BRIDGE_ABI_VERSION == 20;
    assert KagemushaRecursiveSpendProver.ARTIFACT_COUNT == 8;
    assert KagemushaRecursiveSpendProver.MAXIMUM_INPUTS_PER_TRANSITION == 2;
    assert KagemushaRecursiveSpendProver.MAXIMUM_LOCAL_APPEND_BUILDER_INPUTS == 2;
    assert KagemushaRecursiveSpendProver.MAXIMUM_BRANCH_CLAIMS == 2;
    assert KagemushaRecursiveSpendProver.MAXIMUM_PEER_HOPS == 8;
    assert KagemushaRecursiveSpendProver.MAXIMUM_RECURSIVE_PROOF_PAIR_BYTES_V4
        == 16 * 1024 * 1024;
    assert KagemushaRecursiveSpendProver.MAX_PEER_ARCHIVE_BYTES_V2 == 32 * 1024;
    assert KagemushaRecursiveSpendProver.MAX_PEER_ARCHIVE_BYTES_V4 == 32 * 1024 * 1024;
    assert KagemushaRecursiveSpendProver.MAX_TOP_UP_PROVENANCE_ARCHIVE_BYTES_V4 == 6_488_064;
    assert KagemushaRecursiveSpendProver.MAX_PEER_ARCHIVE_BYTES
        == KagemushaRecursiveSpendProver.MAX_PEER_ARCHIVE_BYTES_V4;
    assert KagemushaPeerTransport.MAXIMUM_ARCHIVE_BYTES_V2 == 32 * 1024;
    assert KagemushaPeerTransport.MAXIMUM_ARCHIVE_BYTES_V4 == 32 * 1024 * 1024;
    assert KagemushaPeerTransport.MAXIMUM_ARCHIVE_BYTES
        == KagemushaPeerTransport.MAXIMUM_ARCHIVE_BYTES_V4;
    assert KagemushaRecursiveSpendProver.MAX_PEER_TEXT_ARCHIVE_BYTES == 9_211;
    assert KagemushaRecursiveSpendProver.MAX_TORII_TOP_UP_REQUEST_BYTES_V4 == 512 * 1024;
    assert KagemushaRecursiveSpendProver.MAX_TORII_REDEEM_REQUEST_BYTES_V4
        == 48 * 1024 * 1024;
    assert KagemushaRecursiveSpendProver.CONFIDENTIAL_TREE_DEPTH == 16;
    assert KagemushaRecursiveSpendProver.MAX_OUTPUT_MEMBERSHIP_FRONTIER_ARCHIVE_BYTES_V4
        == 4 * 1024;
    assert KagemushaRecursiveSpendProver.MAX_OUTPUT_MEMBERSHIP_PATHS_ARCHIVE_BYTES_V4
        == 16 * 1024;
    assert "kagemusha.offline.recursive_spend.artifact_manifest.v4"
        .equals(KagemushaRecursiveSpendProver.ARTIFACT_MANIFEST_SCHEMA);
    assert KagemushaRecursiveSpendProver.ARTIFACT_FILES.equals(
        Arrays.asList(
            "step-eq.params-ipa.krv4",
            "step-eq.proving-key.krv4",
            "step-eq.verifying-key.krv4",
            "step-eq.bootstrap-witness.krv4",
            "step-ep.params-ipa.krv4",
            "step-ep.proving-key.krv4",
            "step-ep.verifying-key.krv4",
            "step-ep.bootstrap-witness.krv4"));
    final Method[] methods = KagemushaRecursiveSpendProver.class.getDeclaredMethods();
    final Method appendNative = Arrays.stream(methods)
        .filter(method -> method.getName().equals("nativeBuildAppendRequestV4"))
        .findFirst()
        .orElseThrow();
    assert appendNative.getParameterTypes()[0] == byte[][].class;
    assert appendNative.getParameterTypes()[1] == byte[][].class;
    assert appendNative.getParameterTypes()[2] == byte[][].class;
    assert appendNative.getParameterTypes()[3] == byte[][].class;
    assert Arrays.stream(methods)
        .filter(method -> method.getName().equals("buildAppendRequestV4"))
        .count() == 1;
    final Method installNative = Arrays.stream(methods)
        .filter(method -> method.getName().equals("nativeArtifactSetInstallV4"))
        .findFirst()
        .orElseThrow();
    assert Arrays.equals(
        installNative.getParameterTypes(),
        new Class<?>[] {
          byte[].class,
          byte[].class,
          byte[].class,
          byte[].class,
          byte[].class,
          byte[].class,
          byte[].class,
          long[].class
        });
    final Method[] publicInstallFactories = Arrays.stream(methods)
        .filter(method -> Modifier.isPublic(method.getModifiers()))
        .filter(method -> method.getName().equals("beginArtifactInstallSession"))
        .toArray(Method[]::new);
    assert publicInstallFactories.length == 1;
    assert Arrays.equals(
        publicInstallFactories[0].getParameterTypes(),
        new Class<?>[] {
          byte[].class,
          byte[].class,
          KagemushaRecursiveSpendProver.ReleaseAuthentication.class
        });

    final Set<String> branchMethods = new TreeSet<>();
    for (final Method method : KagemushaRecursiveSpendProver.BranchProjection.class.getMethods()) {
      branchMethods.add(method.getName());
    }
    assert branchMethods.containsAll(Arrays.asList(
        "artifactBinding", "branchClaims", "bundleDigest", "proofStepCount"));
    assert !branchMethods.contains("parentBranchClaimDigest");
    assert !branchMethods.contains("branchClaimDigest");
    assert Arrays.stream(KagemushaRecursiveSpendProver.InitProjectionV4.class.getMethods())
        .map(Method::getName)
        .anyMatch("topUpProvenance"::equals);
    assert Arrays.stream(KagemushaRecursiveSpendProver.InitProjectionV4.class.getMethods())
        .map(Method::getName)
        .anyMatch("branch"::equals);
    assert Arrays.stream(KagemushaRecursiveSpendProver.RedeemBuildProjection.class.getMethods())
        .map(Method::getName)
        .anyMatch("changeTopUpProvenance"::equals);
  }

  private static void frontierContractIsPersistableAndProofBound() {
    final byte[] archive = archive(
        "connect_norito_bridge::KagemushaOutputMembershipFrontierV4");
    final KagemushaRecursiveSpendProver.OutputMembershipFrontierV4 frontier =
        KagemushaRecursiveSpendProver.decodeOutputMembershipFrontierV4(archive);
    archive[archive.length - 1] ^= 0x5a;
    final byte[] first = frontier.noritoEncoded();
    first[first.length - 1] ^= 0x33;
    assert !Arrays.equals(first, frontier.noritoEncoded());

    final Method build = declaredMethod(
        "nativeBuildOutputMembershipFrontierV4",
        int.class,
        byte[].class,
        byte[].class,
        byte[].class);
    assert build.getReturnType() == byte[].class;
    final Method derive = declaredMethod(
        "nativeDeriveOutputMembershipPathsV4",
        byte[].class,
        byte[].class,
        byte[].class);
    assert derive.getReturnType() == byte[][].class;
    final Method validate = declaredMethod(
        "nativeValidateSpendableBranchV4",
        byte[].class,
        byte[].class,
        byte[].class,
        byte[].class,
        long.class);
    assert validate.getReturnType() == byte[].class;
    assert Arrays.stream(KagemushaRecursiveSpendProver.SpendableBranchV4.class.getMethods())
        .anyMatch(method -> method.getName().equals("frontier")
            && method.getReturnType()
                == KagemushaRecursiveSpendProver.OutputMembershipFrontierV4.class);
  }

  private static void outputMembershipPathsRejectNonconsecutiveDummyFrontier() {
    final byte[] initialRoot = filled(0x11);
    final byte[] finalRoot = filled(0x22);
    final byte[] afterRecipientRoot = filled(0x33);
    final KagemushaRecursiveSpendProver.OutputMembershipLeafPaths recipient =
        new KagemushaRecursiveSpendProver.OutputMembershipLeafPaths(
            outputMembershipPath(initialRoot, 0), outputMembershipPath(finalRoot, 0));
    assertThrowsIllegalArgument(() ->
        new KagemushaRecursiveSpendProver.OutputMembershipPaths(
            initialRoot,
            finalRoot,
            recipient,
            null,
            outputMembershipPath(finalRoot, 2)));
    final KagemushaRecursiveSpendProver.OutputMembershipLeafPaths change =
        new KagemushaRecursiveSpendProver.OutputMembershipLeafPaths(
            outputMembershipPath(afterRecipientRoot, 1),
            outputMembershipPath(finalRoot, 1));
    assertThrowsIllegalArgument(() ->
        new KagemushaRecursiveSpendProver.OutputMembershipPaths(
            initialRoot,
            finalRoot,
            recipient,
            change,
            outputMembershipPath(finalRoot, 3)));
    final KagemushaRecursiveSpendProver.OutputMembershipLeafPaths redemptionChange =
        new KagemushaRecursiveSpendProver.OutputMembershipLeafPaths(
            outputMembershipPath(initialRoot, 5),
            outputMembershipPath(finalRoot, 5));
    assertThrowsIllegalArgument(() ->
        new KagemushaRecursiveSpendProver.OutputMembershipPaths(
            initialRoot,
            finalRoot,
            null,
            redemptionChange,
            outputMembershipPath(finalRoot, 7)));
  }

  private static void artifactRoleInventoryRejectsCountsDuplicatesAndReordering() {
    final List<KagemushaRecursiveSpendProver.ArtifactRoleV4> canonical =
        Arrays.asList(KagemushaRecursiveSpendProver.ArtifactRoleV4.values());
    KagemushaRecursiveSpendProver.requireCanonicalV4ArtifactRoleInventory(canonical);

    for (final int count : new int[] {6, 7, 9}) {
      final List<KagemushaRecursiveSpendProver.ArtifactRoleV4> invalid =
          new ArrayList<>();
      for (int index = 0; index < count; index++) {
        invalid.add(canonical.get(index % canonical.size()));
      }
      assertThrowsIllegalArgument(() ->
          KagemushaRecursiveSpendProver.requireCanonicalV4ArtifactRoleInventory(invalid));
    }

    final List<KagemushaRecursiveSpendProver.ArtifactRoleV4> duplicate =
        new ArrayList<>(canonical);
    duplicate.set(1, duplicate.get(0));
    assertThrowsIllegalArgument(() ->
        KagemushaRecursiveSpendProver.requireCanonicalV4ArtifactRoleInventory(duplicate));

    final List<KagemushaRecursiveSpendProver.ArtifactRoleV4> reordered =
        new ArrayList<>(canonical);
    Collections.swap(reordered, 0, 1);
    assertThrowsIllegalArgument(() ->
        KagemushaRecursiveSpendProver.requireCanonicalV4ArtifactRoleInventory(reordered));
  }

  private static void releaseAuthenticationIsMandatoryAndBounded() {
    final byte[] one = new byte[] {1};
    new KagemushaRecursiveSpendProver.ReleaseAuthentication(one, one, one, one, one);
    assertThrowsIllegalArgument(() ->
        new KagemushaRecursiveSpendProver.ReleaseAuthentication(
            new byte[0], one, one, one, one));
    assertThrowsIllegalArgument(() ->
        new KagemushaRecursiveSpendProver.ReleaseAuthentication(
            one, new byte[0], one, one, one));
    assertThrowsIllegalArgument(() ->
        new KagemushaRecursiveSpendProver.ReleaseAuthentication(
            one, one, new byte[0], one, one));
    assertThrowsIllegalArgument(() ->
        new KagemushaRecursiveSpendProver.ReleaseAuthentication(
            one, one, one, new byte[0], one));
    assertThrowsIllegalArgument(() ->
        new KagemushaRecursiveSpendProver.ReleaseAuthentication(
            one, one, one, one, new byte[0]));
    assertThrowsIllegalArgument(() ->
        new KagemushaRecursiveSpendProver.ReleaseAuthentication(
            new byte[KagemushaRecursiveSpendProver.MAX_TRUSTED_RELEASE_POLICY_BYTES + 1],
            one,
            one,
            one,
            one));
    assertThrowsIllegalArgument(() ->
        new KagemushaRecursiveSpendProver.ReleaseAuthentication(
            one,
            one,
            one,
            one,
            new byte[KagemushaRecursiveSpendProver.MAX_PROMOTION_RECORD_BYTES + 1]));
  }

  private static void readinessPreservesExactReleaseCapabilitiesIndependently() {
    final KagemushaRecursiveSpendProver.ActiveVerifier transfer = readinessVerifier(
        "confidential_transfer_v2_verifier_record",
        "halo2/pasta/ipa/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
        1,
        null);
    final KagemushaRecursiveSpendProver.ActiveVerifier unshield = readinessVerifier(
        "confidential_unshield_v3_verifier_record",
        "halo2/pasta/ipa/confidential-unshield-change-merkle16-axiom-poseidon-v4",
        3,
        null);
    final KagemushaRecursiveSpendProver.ActiveVerifier stepEq = readinessVerifier(
        "kagemusha_recursive_step_eq_v4_verifier_record",
        "kagemusha-recursive-spend-step-eq-authenticated-layout-v4",
        4,
        30L);
    final KagemushaRecursiveSpendProver.AuthenticatedArtifactSet artifactSet =
        authenticatedArtifactSet();
    final KagemushaRecursiveSpendProver.ReadinessProjection readiness = readinessProjection(
        transfer,
        unshield,
        stepEq,
        artifactSet,
        true);

    assert readiness.allVerifiersActive();
    assert readiness.chainArtifactSetReady();
    assert !readiness.offlineReady();
    assert !readinessProjection(null, unshield, stepEq, artifactSet, true)
        .allVerifiersActive();
    assert readinessProjection(null, unshield, stepEq, artifactSet, true)
        .chainArtifactSetReady();
    assert !readinessProjection(
            transfer,
            readinessVerifier(
                "confidential_unshield_v3_verifier_record",
                "halo2/pasta/ipa/confidential-unshield-change-merkle16-axiom-poseidon-v4",
                3,
                20L),
            stepEq,
            artifactSet,
            true)
        .allVerifiersActive();
    assert !readinessProjection(
            transfer,
            unshield,
            readinessVerifier(
                "kagemusha_recursive_step_eq_v4_verifier_record",
                "kagemusha-recursive-spend-step-eq-authenticated-layout-v4",
                4,
                20L),
            artifactSet,
            true)
        .chainArtifactSetReady();
    assert !readinessProjection(transfer, unshield, stepEq, null, true)
        .chainArtifactSetReady();
    assert !readinessProjection(transfer, unshield, stepEq, artifactSet, false)
        .chainArtifactSetReady();

    final byte[] exposedManifestDigest = artifactSet.manifestSha256();
    Arrays.fill(exposedManifestDigest, (byte) 0);
    assert artifactSet.manifestSha256()[0] == (byte) 0x31;
    assertThrowsIllegalArgument(() ->
        new KagemushaRecursiveSpendProver.AuthenticatedArtifactSet(
            "release-v4",
            filled(0x31),
            filled(0x31),
            filled(0x33),
            10,
            30,
            12 * 1024,
            9));
  }

  private static KagemushaRecursiveSpendProver.ActiveVerifier readinessVerifier(
      final String name,
      final String circuitId,
      final int seed,
      final Long withdrawalHeight) {
    return new KagemushaRecursiveSpendProver.ActiveVerifier(
        "halo2/ipa",
        name,
        1,
        circuitId,
        filled(seed),
        filled(seed + 16),
        12 * 1024,
        10,
        withdrawalHeight);
  }

  private static KagemushaRecursiveSpendProver.AuthenticatedArtifactSet
      authenticatedArtifactSet() {
    return new KagemushaRecursiveSpendProver.AuthenticatedArtifactSet(
        "release-v4",
        filled(0x31),
        filled(0x32),
        filled(0x33),
        10,
        30,
        12 * 1024,
        9);
  }

  private static KagemushaRecursiveSpendProver.ReadinessProjection readinessProjection(
      final KagemushaRecursiveSpendProver.ActiveVerifier transfer,
      final KagemushaRecursiveSpendProver.ActiveVerifier unshield,
      final KagemushaRecursiveSpendProver.ActiveVerifier stepEq,
      final KagemushaRecursiveSpendProver.AuthenticatedArtifactSet artifactSet,
      final boolean proofBackendAvailable) {
    return new KagemushaRecursiveSpendProver.ReadinessProjection(
        20,
        8,
        "xor#sora",
        9,
        20,
        filled(0x41),
        proofBackendAvailable,
        true,
        true,
        transfer,
        readinessVerifier(
            "kagemusha_topup_shield_v2_verifier_record",
            "halo2/pasta/ipa/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
            2,
            null),
        unshield,
        stepEq,
        readinessVerifier(
            "kagemusha_recursive_step_ep_v4_verifier_record",
            "kagemusha-recursive-spend-step-ep-authenticated-layout-v4",
            5,
            30L),
        artifactSet,
        Collections.emptyList());
  }

  private static void appendJoinRejectsZeroAndThreeInputsBeforeNativeDispatch() {
    final byte[] verifier = filled(0x61);
    final byte[] operation = filled(0x62);
    final KagemushaRecursiveSpendProver.OutputMembershipPaths outputMembershipPaths =
        outputMembershipPaths();
    boolean zeroRejected = false;
    try {
      KagemushaRecursiveSpendProver.buildAppendRequestV4(
          new ArrayList<>(), null, outputMembershipPaths, verifier, operation, 1);
    } catch (final IllegalArgumentException expected) {
      zeroRejected = true;
    }
    assert zeroRejected;

    boolean threeRejected = false;
    try {
      KagemushaRecursiveSpendProver.buildAppendRequestV4(
          Arrays.<KagemushaRecursiveSpendProver.SpendableBranchV4>asList(null, null, null),
          null, outputMembershipPaths, verifier, operation, 1);
    } catch (final IllegalArgumentException expected) {
      threeRejected = true;
    }
    assert threeRejected;
  }

  private static void lifecycleArchivesAreTypedDefensiveAndFailClosed() {
    final byte[] initBytes = archive("KagemushaRecursiveSpendInitLocalRequestV4");
    final KagemushaRecursiveSpendProver.InitRequestV4 init =
        KagemushaRecursiveSpendProver.decodeInitRequestV4(initBytes);
    initBytes[initBytes.length - 1] = 0;
    assert init.noritoEncoded()[init.noritoEncoded().length - 1] == 0x51;

    final KagemushaRecursiveSpendProver.AppendRequestV4 append =
        KagemushaRecursiveSpendProver.decodeAppendRequestV4(
            archive("KagemushaRecursiveSpendAppendLocalRequestV4"), null);
    assert append.noritoEncoded().length > NoritoHeader.HEADER_LENGTH;
    assert KagemushaRecursiveSpendProver.decodeVerifyRequestV4(
            archive("KagemushaRecursiveSpendVerifyLocalRequestV4"))
        .noritoEncoded().length > NoritoHeader.HEADER_LENGTH;
    assert KagemushaRecursiveSpendProver.decodeRedeemRequestV4(
            archive("KagemushaRecursiveSpendRedeemLocalRequestV4"), null)
        .noritoEncoded().length > NoritoHeader.HEADER_LENGTH;
    assert KagemushaRecursiveSpendProver.decodeInitResultV4(
            archive("KagemushaRecursiveSpendInitResultV4"))
        .noritoEncoded().length > NoritoHeader.HEADER_LENGTH;

    boolean wrongSchemaRejected = false;
    try {
      KagemushaRecursiveSpendProver.decodeVerifyRequestV4(
          archive("KagemushaRecursiveSpendInitLocalRequestV4"));
    } catch (final IllegalArgumentException expected) {
      wrongSchemaRejected = true;
    }
    assert wrongSchemaRejected;

    boolean invalidTimestampRejected = false;
    try {
      KagemushaRecursiveSpendProver.appendSpendV4(
          append,
          KagemushaRecursiveSpendProver.decodeRecipientPaymentRequest(
              archive("KagemushaRecipientPaymentRequestV2")),
          0);
    } catch (final IllegalArgumentException expected) {
      invalidTimestampRejected = true;
    }
    assert invalidTimestampRejected;

    if (!KagemushaRecursiveSpendProver.isProofBackendAvailable()) {
      boolean unavailableRejected = false;
      try {
        KagemushaRecursiveSpendProver.initSpendV4(init);
      } catch (final IllegalStateException expected) {
        unavailableRejected = true;
      }
      assert unavailableRejected;
    }
    append.close();
    assert append.isDestroyed();
    boolean destroyedRejected = false;
    try {
      append.noritoEncoded();
    } catch (final IllegalStateException expected) {
      destroyedRejected = true;
    }
    assert destroyedRejected;
  }

  private static void topUpProvenanceArchiveIsCanonicalBoundedAndDefensive() {
    final byte[] bytes = archive("KagemushaRecursiveSpendTopUpProvenanceV4");
    final KagemushaRecursiveSpendProver.TopUpProvenanceV4 provenance =
        KagemushaRecursiveSpendProver.decodeTopUpProvenanceV4(bytes);
    bytes[bytes.length - 1] = 0;
    assert provenance.noritoEncoded()[provenance.noritoEncoded().length - 1] == 0x51;

    assertThrowsIllegalArgument(() -> KagemushaRecursiveSpendProver.decodeTopUpProvenanceV4(
        archive("KagemushaRecursiveSpendTopUpFinalityEvidenceV4")));
    final byte[] corrupted = archive("KagemushaRecursiveSpendTopUpProvenanceV4");
    corrupted[corrupted.length - 1] ^= 1;
    assertThrowsIllegalArgument(() ->
        KagemushaRecursiveSpendProver.decodeTopUpProvenanceV4(corrupted));
    assertThrowsIllegalArgument(() -> KagemushaRecursiveSpendProver.decodeTopUpProvenanceV4(
        new byte[KagemushaRecursiveSpendProver.MAX_TOP_UP_PROVENANCE_ARCHIVE_BYTES_V4 + 1]));

    final KagemushaRecursiveSpendProver.BundleV4 bundle =
        KagemushaRecursiveSpendProver.decodeBundleV4(
            archive("KagemushaRecursiveSpendBundleV4"));
    final KagemushaRecursiveSpendProver.TopUpFinalityRosterArtifact roster =
        KagemushaRecursiveSpendProver.decodeTopUpFinalityRosterArtifact(
            archive("KagemushaTopUpFinalityRosterArtifactV2"));
    assertThrowsIllegalArgument(() -> KagemushaRecursiveSpendProver.buildTopUpProvenanceV4(
        bundle, roster, Collections.emptyList(), Collections.emptyList(), 1));
    assertThrowsIllegalArgument(() -> KagemushaRecursiveSpendProver.buildTopUpProvenanceV4(
        bundle,
        roster,
        Collections.singletonList(KagemushaRecursiveSpendProver.decodeTopUpAnchorV4(
            archive("KagemushaRecursiveSpendTopUpAnchorV4"))),
        Collections.emptyList(),
        1));
  }

  private static void branchRestoreRejectsMissingHeightAndLocalChangeOpeningsBeforeNativeDispatch() {
    final KagemushaRecursiveSpendProver.NoteOpening opening =
        KagemushaRecursiveSpendProver.decodeNoteOpening(archive("KagemushaNoteOpeningV2"));
    final KagemushaRecursiveSpendProver.InitResultV4 init =
        KagemushaRecursiveSpendProver.decodeInitResultV4(
            archive("KagemushaRecursiveSpendInitResultV4"));
    assertThrowsIllegalArgument(() ->
        KagemushaRecursiveSpendProver.restoreInitBranchV4(init, opening, 0));
    final KagemushaRecursiveSpendProver.PeerPayment payment =
        KagemushaRecursiveSpendProver.decodePeerPayment(
            archive("KagemushaRecursiveSpendPeerPaymentV4"));
    assertThrowsIllegalArgument(() ->
        KagemushaRecursiveSpendProver.restorePeerPaymentBranchV4(payment, opening, 0));

    final KagemushaRecursiveSpendProver.SplitResultV4 split =
        KagemushaRecursiveSpendProver.decodeSplitResultV4(
            archive("KagemushaRecursiveSpendSplitResultV4"), null);
    assertThrowsIllegalState(() ->
        KagemushaRecursiveSpendProver.restoreSplitChangeBranchV4(split, 1));
    final KagemushaRecursiveSpendProver.RedeemBuildResultV4 redeem =
        KagemushaRecursiveSpendProver.decodeRedeemBuildResultV4(
            archive("KagemushaRecursiveSpendRedeemBuildResultV4"), null);
    assertThrowsIllegalState(() ->
        KagemushaRecursiveSpendProver.restoreRedeemChangeBranchV4(redeem, 1));
  }

  private static void scaledAmountsAreExactAndNeverRound() {
    final KagemushaScaledAmount amount = KagemushaScaledAmount.fromDecimal("10.75", 9);
    assert amount.atomicUnits().equals("10750000000");
    assert amount.scaledNumericDecimal().equals("10.750000000");
    assert amount.displayDecimal().equals("10.75");
    assert KagemushaScaledAmount.sum(
            Arrays.asList(
                KagemushaScaledAmount.fromDecimal("4.50", 9),
                KagemushaScaledAmount.fromDecimal("6.25", 9)))
        .atomicUnits().equals("10750000000");
    assert KagemushaScaledAmount.fromAtomicUnits("1", 9)
        .scaledNumericDecimal().equals("0.000000001");
    assert KagemushaScaledAmount.fromAtomicUnits(
            KagemushaScaledAmount.MAXIMUM_ATOMIC_UNITS, 28)
        .atomicUnits().equals(KagemushaScaledAmount.MAXIMUM_ATOMIC_UNITS);

    boolean precisionRejected = false;
    try {
      KagemushaScaledAmount.fromDecimal("1.001", 2);
    } catch (final IllegalArgumentException expected) {
      precisionRejected = true;
    }
    assert precisionRejected;

    boolean overflowRejected = false;
    try {
      KagemushaScaledAmount.fromAtomicUnits(
          "340282366920938463463374607431768211456", 9);
    } catch (final IllegalArgumentException expected) {
      overflowRejected = true;
    }
    assert overflowRejected;
  }

  private static void canonicalPeerCodecsAreTypedAndDefensive() {
    final byte[] requestArchive = archive("KagemushaRecipientPaymentRequestV2");
    final KagemushaRecursiveSpendProver.RecipientPaymentRequest request =
        KagemushaRecursiveSpendProver.decodeRecipientPaymentRequest(requestArchive);
    requestArchive[requestArchive.length - 1] ^= 1;
    assert request.noritoEncoded()[request.noritoEncoded().length - 1] == 0x51;

    assert KagemushaRecursiveSpendProver.decodePeerPayment(
            archive("KagemushaRecursiveSpendPeerPaymentV4"))
        .noritoEncoded().length > NoritoHeader.HEADER_LENGTH;
    assert KagemushaRecursiveSpendProver.decodeReceiverAcknowledgement(
            archive("KagemushaReceiverAcknowledgementV2"))
        .noritoEncoded().length > NoritoHeader.HEADER_LENGTH;
    assert KagemushaRecursiveSpendProver.decodeNoteMembershipWitness(
            archive("KagemushaNoteMembershipWitnessV2"))
        .noritoEncoded().length > NoritoHeader.HEADER_LENGTH;

    boolean rejected = false;
    try {
      KagemushaRecursiveSpendProver.decodePeerPayment(
          archive("KagemushaRecipientPaymentRequestV2"));
    } catch (final IllegalArgumentException expected) {
      rejected = true;
    }
    assert rejected;

    boolean malformedRejected = false;
    try {
      KagemushaRecursiveSpendProver.decodePeerPayment(new byte[] {1, 2, 3});
    } catch (final IllegalArgumentException expected) {
      malformedRejected = true;
    }
    assert malformedRejected;

    final byte[] corrupted = archive("KagemushaRecursiveSpendPeerPaymentV4");
    corrupted[corrupted.length - 1] = 0;
    boolean checksumRejected = false;
    try {
      KagemushaRecursiveSpendProver.decodePeerPayment(corrupted);
    } catch (final IllegalArgumentException expected) {
      checksumRejected = true;
    }
    assert checksumRejected;
  }

  private static byte[] archive(final String schema) {
    return archive(schema, 1);
  }

  private static byte[] archive(final String schema, final int payloadLength) {
    final byte[] payload = new byte[payloadLength];
    for (int index = 0; index < payload.length; index++) {
      payload[index] = (byte) (0x51 + index * 17);
    }
    final NoritoHeader header =
        new NoritoHeader(
            SchemaHash.hash16(schema),
            payload.length,
            CRC64.compute(payload),
            NoritoHeader.COMPACT_LEN,
            NoritoHeader.COMPRESSION_NONE);
    final byte[] archive = new byte[NoritoHeader.HEADER_LENGTH + payload.length];
    System.arraycopy(header.encode(), 0, archive, 0, NoritoHeader.HEADER_LENGTH);
    System.arraycopy(payload, 0, archive, NoritoHeader.HEADER_LENGTH, payload.length);
    return archive;
  }

  private static void peerTransportGoldenVectorsAreExact() {
    final KagemushaPeerTransport.Payload request =
        KagemushaPeerTransport.Payload.decode(
            archive("KagemushaRecipientPaymentRequestV2"),
            KagemushaPeerTransport.Kind.RECEIVE_REQUEST);
    final String text = KagemushaPeerTransport.encode(request);
    assert text.equals(
        "PKK2R.TlJUMAAA27ZYXi51qDW87RkAOqt6zQABAAAAAAAAAN6BMN0_Z661AlE");
    assert KagemushaPeerTransport.decode(text).kind()
        == KagemushaPeerTransport.Kind.RECEIVE_REQUEST;
    assert KagemushaPeerTransport.decodeUserPresented(" \n" + text + "\t",
        KagemushaPeerTransport.Kind.RECEIVE_REQUEST).kind()
        == KagemushaPeerTransport.Kind.RECEIVE_REQUEST;
    assert KagemushaPeerTransport.RECEIVE_REQUEST_TEXT_PREFIX.equals("PKK2R.");
    assert KagemushaPeerTransport.PAYMENT_TEXT_PREFIX.equals("PKK2P.");
    assert KagemushaPeerTransport.ACKNOWLEDGEMENT_TEXT_PREFIX.equals("PKK2A.");
    assert KagemushaPeerTransport.QR_STREAM_TEXT_PREFIX.equals("PKKQ1.");
  }

  private static void qrNfcAndNearbyGoldenVectorsAreExact() {
    final KagemushaPeerTransport.Payload request =
        KagemushaPeerTransport.Payload.decode(
            archive("KagemushaRecipientPaymentRequestV2"),
            KagemushaPeerTransport.Kind.RECEIVE_REQUEST);
    final List<String> frames = KagemushaQrStream.encode(
        request, KagemushaQrStream.Options.STANDARD);
    assert frames.equals(Arrays.asList(
        "PKKQ1.S1EBALu6J7gkvW_mKvRoE04Tc9IAAAABAC4BAQQAAQAAAQABAAAAKbu6J7gkvW_mKvRoE04Tc9L3Ile8Baahf0wb7ZGckATmMK4Faw",
        "PKKQ1.S1EBAbu6J7gkvW_mKvRoE04Tc9IAAAABAClOUlQwAADbtlheLnWoNbztGQA6q3rNAAEAAAAAAAAA3oEw3T9nrrUCUZiX9lk",
        "PKKQ1.S1EBAru6J7gkvW_mKvRoE04Tc9IAAAABAQBOUlQwAADbtlheLnWoNbztGQA6q3rNAAEAAAAAAAAA3oEw3T9nrrUCUQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA4vsCHg"));
    final KagemushaQrStream.Decoder decoder = new KagemushaQrStream.Decoder();
    assert !decoder.ingest(frames.get(0)).isComplete();
    final KagemushaQrStream.DecodeResult recovered = decoder.ingest(frames.get(2));
    assert recovered.isComplete();
    assert recovered.recoveredDataFrames() == 1;

    final byte[] rawArchive = request.archive();
    final List<byte[]> apdus = KagemushaNfcProtocol.writePayloadApdus(
        KagemushaNfcProtocol.PayloadKind.RECEIVE_REQUEST, rawArchive, 220);
    assert hex(apdus.get(0)).equals(
        "8020040026040100000029bbba27b824bd6fe62af468134e1373d2f72257bc05a6a17f4c1bed919c9004e6");
    assert hex(apdus.get(1)).equals(
        "802104002d000000004e5254300000dbb6585e2e75a835bced19003aab7acd000100000000000000de8130dd3f67aeb50251");
    assert hex(apdus.get(2)).equals("8022040000");
    assert KagemushaNfcProtocol.AID_HEX.equals("F0504B45504B524E464301");
    assert KagemushaNfcProtocol.SAFE_CHUNK_BYTES == 220;
    assert KagemushaNfcProtocol.RAW_TRANSPORT_VERSION == 4;
    assert KagemushaNfcProtocol.parseCommand(apdus.get(0)).type()
        == KagemushaNfcProtocol.Type.WRITE_META;

    final byte[] nearby = KagemushaNearby.encode(request, KagemushaNearby.PairingSymbol.STARS);
    assert new String(nearby, StandardCharsets.UTF_8).equals(
        "{\"contentType\":\"text/vnd.pk.kagemusha-v2.receive-request\",\"kind\":\"receive_request\",\"pairingChallenge\":\"nearby_pairing_stars\",\"payload\":\"UEtLMlIuVGxKVU1BQUEyN1pZWGk1MXFEVzg3UmtBT3F0NnpRQUJBQUFBQUFBQUFONkJNTjBfWjY2MUFsRQ\"}");
    assert KagemushaNearby.decode(nearby).payload().kind()
        == KagemushaPeerTransport.Kind.RECEIVE_REQUEST;
    assert !KagemushaNearby.IS_AVAILABLE;
    Arrays.fill(rawArchive, (byte) 0);
    Arrays.fill(nearby, (byte) 0);
  }

  private static void nfcV4StreamsBeyondLegacyLimitAndRejectsDowngrade() {
    final byte[] payload = new byte[70_003];
    for (int index = 0; index < payload.length; index++) {
      payload[index] = (byte) (index * 29 + 7);
    }
    final List<byte[]> commands = KagemushaNfcProtocol.writePayloadApdus(
        KagemushaNfcProtocol.PayloadKind.PAYMENT,
        payload,
        KagemushaNfcProtocol.MAX_EXTENDED_WRITE_CHUNK_BYTES);
    final KagemushaNfcProtocol.Command metadata =
        KagemushaNfcProtocol.parseCommand(commands.get(0));
    assert metadata.type() == KagemushaNfcProtocol.Type.WRITE_META;
    assert metadata.payloadLength() == payload.length;
    final KagemushaNfcProtocol.PayloadAssembler assembler =
        new KagemushaNfcProtocol.PayloadAssembler(
            metadata.kind(), metadata.payloadLength(), metadata.sha256());
    for (int index = commands.size() - 2; index >= 1; index--) {
      final KagemushaNfcProtocol.Command chunk =
          KagemushaNfcProtocol.parseCommand(commands.get(index));
      assert chunk.type() == KagemushaNfcProtocol.Type.WRITE_CHUNK;
      assert assembler.write(chunk.offset(), chunk.bytes());
    }
    assert assembler.isComplete();
    assert Arrays.equals(payload, assembler.commit());

    final byte[] atFFFF = KagemushaNfcProtocol.writeChunkApdu(0xFFFF, new byte[] {0x5A});
    final byte[] at10000 = KagemushaNfcProtocol.writeChunkApdu(0x1_0000, new byte[] {0x5A});
    assert hex(atFFFF).equals("80210400050000ffff5a");
    assert hex(at10000).equals("8021040005000100005a");
    assert KagemushaNfcProtocol.parseCommand(atFFFF).offset() == 0xFFFF;
    assert KagemushaNfcProtocol.parseCommand(at10000).offset() == 0x1_0000;
    assert hex(KagemushaNfcProtocol.readChunkApdu(0xFFFF, 1024))
        .equals("80110400060000ffff0400");

    // V2 encoded offsets in P1/P2, including offset zero, are not V4 commands.
    assert KagemushaNfcProtocol.parseCommand(
        new byte[] {(byte) 0x80, 0x21, 0, 0, 1, 0x5A}).type()
        == KagemushaNfcProtocol.Type.INVALID;
    assert KagemushaNfcProtocol.parseCommand(
        new byte[] {(byte) 0x80, 0x21, (byte) 0xFF, (byte) 0xFF, 1, 0x5A}).type()
        == KagemushaNfcProtocol.Type.INVALID;
    assert KagemushaNfcProtocol.parseCommand(
        new byte[] {(byte) 0x80, 0x11, 0, 0, 0}).type()
        == KagemushaNfcProtocol.Type.INVALID;
    final byte[] truncated = Arrays.copyOf(at10000, at10000.length - 1);
    assert KagemushaNfcProtocol.parseCommand(truncated).type()
        == KagemushaNfcProtocol.Type.INVALID;

    final byte[] maximumInfo = new byte[40];
    maximumInfo[0] = (byte) KagemushaNfcProtocol.RAW_TRANSPORT_VERSION;
    maximumInfo[1] = (byte) KagemushaNfcProtocol.PayloadKind.PAYMENT.code();
    maximumInfo[2] = 0x02;
    maximumInfo[6] = 0x00;
    maximumInfo[7] = (byte) KagemushaNfcProtocol.SAFE_CHUNK_BYTES;
    maximumInfo[8] = 1;
    assert KagemushaNfcProtocol.decodeInfo(maximumInfo).payloadLength()
        == 32 * 1024 * 1024;
    maximumInfo[5] = 1;
    assert KagemushaNfcProtocol.decodeInfo(maximumInfo) == null;
    assertThrowsIllegalArgument(() -> KagemushaNfcProtocol.writeChunkApdu(
        KagemushaNfcProtocol.MAXIMUM_PAYLOAD_BYTES, new byte[] {1}));

    assembler.clear();
    Arrays.fill(payload, (byte) 0);
  }

  private static String hex(final byte[] bytes) {
    final StringBuilder out = new StringBuilder(bytes.length * 2);
    for (final byte value : bytes) out.append(String.format("%02x", value & 0xff));
    return out.toString();
  }

  private static void toriiLifecycleRoutesAndHeadersAreExact() {
    final AtomicReference<TransportRequest> captured = new AtomicReference<>();
    final KagemushaRecursiveSpendProver.ToriiClient client =
        KagemushaRecursiveSpendProver.newToriiClient(
            URI.create("https://torii.example/api/"),
            request -> {
              captured.set(request);
              final boolean command = "POST".equals(request.method());
              return CompletableFuture.completedFuture(
                  TransportResponse.builder()
                      .setStatusCode(command ? 202 : 200)
                      .addHeader("Content-Type", "application/x-norito")
                      .setBody(
                          archive(
                              command
                                  ? "OfflineOperationReference"
                                  : request.uri().getPath().contains("/operations/")
                                      ? "OfflineOperationStatus"
                                      : "OfflineReadiness"))
                      .build());
            });

    client.getReadiness("pkr#sbp").join();
    assert captured.get().uri().toString()
        .equals("https://torii.example/api/v1/offline/readiness?asset_definition_id=pkr%23sbp");
    assert captured.get().headers().get("Accept").equals(Arrays.asList("application/x-norito"));

    final String operationId = repeat("11", 32);
    client
        .submitTopUp(
            new KagemushaRecursiveSpendProver.TopUpRequest(
                archive("iroha.torii.v1.offline.top_up.request")),
            operationId)
        .join();
    assert captured.get().method().equals("POST");
    assert captured.get().uri().getPath().equals("/api/v1/offline/top-up");
    assert captured.get().headers().get("Content-Type")
        .equals(Arrays.asList("application/x-norito"));
    assert captured.get().headers().get("Idempotency-Key").equals(Arrays.asList(operationId));

    client
        .submitRedeem(
            new KagemushaRecursiveSpendProver.RedeemSubmissionRequest(
                archive("iroha.torii.v1.offline.redeem.request")),
            operationId)
        .join();
    assert captured.get().uri().getPath().equals("/api/v1/offline/redeem");

    client.getOperation(operationId).join();
    assert captured.get().uri().getPath().equals("/api/v1/offline/operations/" + operationId);
  }

  private static void publicSurfaceIsKagemushaOnly() {
    final Set<String> methods = new TreeSet<>();
    for (final Method method : KagemushaRecursiveSpendProver.class.getDeclaredMethods()) {
      if (Modifier.isPublic(method.getModifiers())) {
        methods.add(method.getName());
      }
    }
    assert methods.equals(
        new TreeSet<>(Arrays.asList(
            "beginArtifactIngest",
            "beginArtifactInstallSession",
            "appendSpendV4",
            "buildAppendRequestV4",
            "buildInitRequestV4",
            "buildOutputMembershipFrontierV4",
            "buildTopUpProvenanceV4",
            "buildRedeemV4",
            "buildRedeemRequestV4",
            "buildVerifyRequestV4",
            "decodeAppendRequestV4",
            "decodeBundleV4",
            "decodeInitRequestV4",
            "decodeInitResultV4",
            "decodeNoteMembershipWitness",
            "decodeNoteOpening",
            "decodeOutputMembershipFrontierV4",
            "decodePeerPayment",
            "decodeRedeemRequestV4",
            "decodeReceiverAcknowledgement",
            "decodeRecipientPaymentRequest",
            "decodeRedeemBuildResultV4",
            "decodeRedeemSubmissionRequest",
            "decodeSplitResultV4",
            "decodeTopUpAnchorV4",
            "decodeTopUpFinalityEvidenceV4",
            "decodeTopUpProvenanceV4",
            "decodeTopUpRequest",
            "decodeVerifyRequestV4",
            "decodeVerifyResultV4",
            "decodeTopUpFinalityRosterArtifact",
            "finalizeRedeemV4",
            "finalizeTopUp",
            "deriveOutputMembershipPathsV4",
            "initSpendV4",
            "installedArtifactManifestSha256V4",
            "isArtifactStreamingAvailable",
            "isProofBackendAvailable",
            "newToriiClient",
            "prepareAcknowledgement",
            "prepareNoteOpening",
            "prepareRecipientPaymentRequest",
            "prepareRequestAuthorization",
            "prepareTopUp",
            "projectInitResultV4",
            "projectOperationStatus",
            "projectPeerPayment",
            "projectRecipientPaymentRequest",
            "projectRedeemBuildResultV4",
            "projectReadiness",
            "projectSplitResultV4",
            "projectVerifyResultV4",
            "restoreInitBranchV4",
            "restorePeerPaymentBranchV4",
            "restoreRedeemChangeBranchV4",
            "restoreSpendableBranchV4",
            "restoreSplitChangeBranchV4",
            "signAcknowledgement",
            "signRecipientPaymentRequest",
            "signRequestAuthorization",
            "verifyAcknowledgement",
            "verifyRecipientPaymentRequest",
            "verifySpendV4",
            "validateTopUpProvenanceV4"))) : methods;
    final Set<String> declaredNames = new TreeSet<>();
    for (final Method method : KagemushaRecursiveSpendProver.class.getDeclaredMethods()) {
      declaredNames.add(method.getName());
    }
    for (final String retired : Arrays.asList(
        "projectInitResult",
        "restoreSpendableBranch",
        "buildAppendRequest",
        "buildInitRequest",
        "buildRedeemRequest",
        "buildVerifyRequest",
        "nativeProjectInitResultV2",
        "nativeRestoreSpendableBranchV2")) {
      assert !declaredNames.contains(retired) : retired;
    }
    Method appendBuilder = null;
    for (final Method method : KagemushaRecursiveSpendProver.class.getDeclaredMethods()) {
      if (Modifier.isPublic(method.getModifiers())
          && method.getName().equals("buildAppendRequestV4")) {
        assert appendBuilder == null : "duplicate append builder";
        appendBuilder = method;
      }
    }
    assert appendBuilder != null;
    assert appendBuilder.getParameterTypes()[0].equals(List.class);
    final Method verifyBuilder = Arrays.stream(
            KagemushaRecursiveSpendProver.class.getDeclaredMethods())
        .filter(method -> Modifier.isPublic(method.getModifiers()))
        .filter(method -> method.getName().equals("buildVerifyRequestV4"))
        .findFirst()
        .orElseThrow();
    assert verifyBuilder.getParameterTypes()[2]
        == KagemushaRecursiveSpendProver.TopUpProvenanceV4.class;
    final Set<String> branchMethods = new TreeSet<>();
    for (final Method method : KagemushaRecursiveSpendProver.BranchProjection.class
        .getDeclaredMethods()) {
      branchMethods.add(method.getName());
    }
    assert branchMethods.contains("branchClaims");
    assert branchMethods.contains("bundleDigest");
    assert !branchMethods.contains("branchClaim");
    assert !branchMethods.contains("branchClaimDigest");
    assert !branchMethods.contains("parentBranchClaimDigest");
    for (final String name : Arrays.asList(
        "decodeAppendRequestV4",
        "decodeSplitResultV4",
        "decodeRedeemRequestV4",
        "decodeRedeemBuildResultV4")) {
      final List<Method> candidates = new ArrayList<>();
      for (final Method method : KagemushaRecursiveSpendProver.class.getDeclaredMethods()) {
        if (Modifier.isPublic(method.getModifiers()) && method.getName().equals(name)) {
          candidates.add(method);
        }
      }
      assert candidates.size() == 1 : candidates;
      assert Arrays.equals(
          candidates.get(0).getParameterTypes(),
          new Class<?>[] {byte[].class, KagemushaRecursiveSpendProver.NoteOpening.class}) : name;
    }
  }

  private static byte[] filled(final int value) {
    final byte[] bytes = new byte[32];
    Arrays.fill(bytes, (byte) value);
    return bytes;
  }

  private static KagemushaRecursiveSpendProver.OutputMembershipPaths outputMembershipPaths() {
    final byte[] initialRoot = filled(0x11);
    final byte[] finalRoot = filled(0x22);
    return new KagemushaRecursiveSpendProver.OutputMembershipPaths(
        initialRoot,
        finalRoot,
        new KagemushaRecursiveSpendProver.OutputMembershipLeafPaths(
            outputMembershipPath(initialRoot, 0), outputMembershipPath(finalRoot, 0)),
        null,
        outputMembershipPath(finalRoot, 1));
  }

  private static KagemushaRecursiveSpendProver.OutputMembershipPath outputMembershipPath(
      final byte[] root,
      final int leafIndex) {
    final List<byte[]> siblings = new ArrayList<>();
    for (int index = 0;
        index < KagemushaRecursiveSpendProver.CONFIDENTIAL_TREE_DEPTH;
        index++) {
      siblings.add(new byte[32]);
    }
    final byte[] directions =
        new byte[KagemushaRecursiveSpendProver.CONFIDENTIAL_TREE_DEPTH];
    for (int level = 0; level < directions.length; level++) {
      directions[level] = (byte) ((leafIndex >>> level) & 1);
    }
    return new KagemushaRecursiveSpendProver.OutputMembershipPath(
        leafIndex,
        siblings,
        directions,
        root);
  }

  private static void assertThrowsIllegalArgument(final Runnable action) {
    try {
      action.run();
      assert false : "expected IllegalArgumentException";
    } catch (final IllegalArgumentException expected) {
      // Expected fail-closed validation.
    }
  }

  private static void assertThrowsIllegalState(final Runnable action) {
    try {
      action.run();
      assert false : "expected IllegalStateException";
    } catch (final IllegalStateException expected) {
      // Expected.
    }
  }

  private static Method declaredMethod(final String name, final Class<?>... parameterTypes) {
    try {
      return KagemushaRecursiveSpendProver.class.getDeclaredMethod(name, parameterTypes);
    } catch (final ReflectiveOperationException failure) {
      throw new AssertionError("missing method " + name, failure);
    }
  }

  private static String repeat(final String value, final int count) {
    final StringBuilder repeated = new StringBuilder(value.length() * count);
    for (int index = 0; index < count; index++) repeated.append(value);
    return repeated.toString();
  }
}
