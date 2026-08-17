package org.hyperledger.iroha.android.offline;

import java.lang.reflect.Constructor;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.PrivateKey;
import java.security.Signature;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import org.hyperledger.iroha.android.client.CanonicalRequestSigner;
import org.hyperledger.iroha.android.client.LocalSigningContext;
import org.hyperledger.iroha.android.client.ToriiCanonicalRequestAuth;
import org.hyperledger.iroha.android.client.transport.RequestReplayPolicy;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.norito.CRC64;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;

/** Source-level checks for the ABI-22 bridge carrying the Kagemusha ABI-21/V4 lifecycle. */
public final class KagemushaRecursiveSpendProverTest {
  public static void main(final String[] args) {
    heavyProofPermitIsReentrantButRejectsAnotherThreadWithoutWaiting();
    exactAbiIsRequired();
    appAttestNativeProjectionRejectsCorruptAuxiliaryFields();
    artifactContractIsFixed();
    frontierContractIsPersistableAndProofBound();
    outputMembershipPathsRejectNonconsecutiveDummyFrontier();
    redemptionChangePreparationTransfersOpeningOwnershipExactlyOnce();
    redemptionChangePreparationDestroysOnlyAnUntransferredOpening();
    redemptionChangeRequestFailureDestroysTransferredOpening();
    redemptionChangeCarriersCloseOrTransferOneOwnerExactlyOnce();
    noteOpeningAndInitRequestCloseIdempotently();
    temporarySecretArchivesAreWipedAfterPartialConstructionAndOwnerCopy();
    preparationConstructorFailuresDestroyStagedOpenings();
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
    offlineCapabilityRejectsBackendReadinessClaims();
    publicSurfaceIsKagemushaOnly();
  }

  @org.junit.Test
  public void scaledAmountsAreExactAndNeverRoundUnderJUnit() {
    scaledAmountsAreExactAndNeverRound();
  }

  @org.junit.Test
  public void toriiLifecycleRoutesAndHeadersAreExactUnderJUnit() {
    toriiLifecycleRoutesAndHeadersAreExact();
  }

  @org.junit.Test
  public void offlineCapabilityRejectsBackendReadinessClaimsUnderJUnit() {
    offlineCapabilityRejectsBackendReadinessClaims();
  }

  @org.junit.Test
  public void secretLifecycleTransfersAndWipesOwnershipUnderJUnit() {
    redemptionChangePreparationTransfersOpeningOwnershipExactlyOnce();
    redemptionChangePreparationDestroysOnlyAnUntransferredOpening();
    redemptionChangeRequestFailureDestroysTransferredOpening();
    redemptionChangeCarriersCloseOrTransferOneOwnerExactlyOnce();
    temporarySecretArchivesAreWipedAfterPartialConstructionAndOwnerCopy();
    preparationConstructorFailuresDestroyStagedOpenings();
  }

  private static void heavyProofPermitIsReentrantButRejectsAnotherThreadWithoutWaiting() {
    final CountDownLatch entered = new CountDownLatch(1);
    final CountDownLatch release = new CountDownLatch(1);
    final AtomicReference<Throwable> failure = new AtomicReference<>();
    final Thread worker = new Thread(() -> {
      try {
        KagemushaRecursiveSpendProver.withHeavyProofPermitForTest(() -> {
          entered.countDown();
          try {
            if (!release.await(5, TimeUnit.SECONDS)) {
              throw new AssertionError("timed out waiting to release proof permit");
            }
          } catch (final InterruptedException error) {
            Thread.currentThread().interrupt();
            throw new AssertionError(error);
          }
        });
      } catch (final Throwable error) {
        failure.set(error);
      }
    });
    worker.start();
    try {
      assert entered.await(5, TimeUnit.SECONDS);
    } catch (final InterruptedException error) {
      Thread.currentThread().interrupt();
      throw new AssertionError(error);
    }
    try {
      KagemushaRecursiveSpendProver.withHeavyProofPermitForTest(() -> {});
      throw new AssertionError("contending proof permit must fail without waiting");
    } catch (final KagemushaRecursiveSpendProver.ProofWorkerBusyException expected) {
      assert expected.getMessage().contains("retry");
    } finally {
      release.countDown();
    }
    try {
      worker.join(5_000);
    } catch (final InterruptedException error) {
      Thread.currentThread().interrupt();
      throw new AssertionError(error);
    }
    assert !worker.isAlive();
    if (failure.get() != null) throw new AssertionError(failure.get());
    KagemushaRecursiveSpendProver.withHeavyProofPermitForTest(() ->
        KagemushaRecursiveSpendProver.withHeavyProofPermitForTest(() -> {}));
  }

  private static void exactAbiIsRequired() {
    assert KagemushaRecursiveSpendProver.isExactBridgeAbi(22);
    assert !KagemushaRecursiveSpendProver.isExactBridgeAbi(20);
    assert KagemushaRecursiveSpendProver.detectExactNativeAvailability(
        () -> {}, () -> 22, () -> true);
    assert !KagemushaRecursiveSpendProver.detectExactNativeAvailability(
        () -> {}, () -> 22, () -> false);
    assert !KagemushaRecursiveSpendProver.detectExactNativeAvailability(
        () -> { throw new UnsatisfiedLinkError("missing"); }, () -> 22, () -> true);
    assert KagemushaRecursiveSpendProver.detectProductionProofBackendCompilation(
        () -> { throw new IllegalArgumentException("production artifact validation"); });
    assert !KagemushaRecursiveSpendProver.detectProductionProofBackendCompilation(
        () -> { throw new IllegalStateException("default build"); });
    assert !KagemushaRecursiveSpendProver.detectProductionProofBackendCompilation(
        () -> { throw new UnsatisfiedLinkError("missing"); });
  }

  private static void appAttestNativeProjectionRejectsCorruptAuxiliaryFields() {
    final byte[] authorizationArchive = archive("KagemushaRequestAuthorizationV2");
    final byte[] rawLowSSignature = new byte[64];
    rawLowSSignature[31] = 1;
    rawLowSSignature[63] = 1;
    final byte[] extensionAuthenticatorData = new byte[38];
    extensionAuthenticatorData[32] = (byte) 0x80;
    extensionAuthenticatorData[36] = 1;
    extensionAuthenticatorData[37] = (byte) 0xa0;
    final KagemushaRecursiveSpendProver.RequestAuthorization accepted =
        KagemushaRecursiveSpendProver.requestAuthorizationFromIosAppAttestNativeProjection(
            new byte[][] {authorizationArchive, rawLowSSignature, extensionAuthenticatorData});
    assert Arrays.equals(authorizationArchive, accepted.noritoEncoded());

    final List<byte[][]> invalidProjections = Arrays.asList(
        new byte[][] {authorizationArchive, rawLowSSignature},
        new byte[][] {authorizationArchive, new byte[0], extensionAuthenticatorData},
        new byte[][] {
          authorizationArchive, Arrays.copyOf(rawLowSSignature, 63), extensionAuthenticatorData
        },
        new byte[][] {authorizationArchive, new byte[64], extensionAuthenticatorData},
        new byte[][] {authorizationArchive, rawLowSSignature, new byte[36]},
        new byte[][] {authorizationArchive, rawLowSSignature, new byte[38]},
        new byte[][] {
          authorizationArchive, rawLowSSignature, authenticatorData(37, 0)
        },
        new byte[][] {
          authorizationArchive, rawLowSSignature, authenticatorData(37, 0x80)
        },
        new byte[][] {
          authorizationArchive, rawLowSSignature, authenticatorData(38, 0x01)
        },
        new byte[][] {
          authorizationArchive, rawLowSSignature, authenticatorData(4 * 1024 + 1, 0x80)
        },
        new byte[][] {authorizationArchive, null, extensionAuthenticatorData});
    for (final byte[][] projection : invalidProjections) {
      assertThrowsIllegalState(() ->
          KagemushaRecursiveSpendProver
              .requestAuthorizationFromIosAppAttestNativeProjection(projection));
    }
  }

  private static byte[] authenticatorData(final int length, final int flags) {
    final byte[] value = new byte[length];
    if (length > 32) value[32] = (byte) flags;
    return value;
  }

  private static void artifactContractIsFixed() {
    assert KagemushaRecursiveSpendProver.REQUIRED_NATIVE_BRIDGE_ABI_VERSION == 22;
    assert KagemushaRecursiveSpendProver.ARTIFACT_COUNT == 8;
    assert KagemushaRecursiveSpendProver.MAX_ARTIFACT_CHUNK_BYTES == 1024 * 1024;
    try {
      KagemushaRecursiveSpendProver.requireChunk(
          new byte[KagemushaRecursiveSpendProver.MAX_ARTIFACT_CHUNK_BYTES + 1]);
      throw new AssertionError("oversized artifact chunk must be rejected before JNI copying");
    } catch (final IllegalArgumentException expected) {
      assert expected.getMessage().contains("1..1048576 bytes");
    }
    assert KagemushaRecursiveSpendProver.MAXIMUM_INPUTS_PER_TRANSITION == 2;
    assert KagemushaRecursiveSpendProver.MAXIMUM_LOCAL_APPEND_BUILDER_INPUTS == 2;
    assert KagemushaRecursiveSpendProver.MAXIMUM_BRANCH_CLAIMS == 2;
    assert KagemushaRecursiveSpendProver.MAXIMUM_PEER_HOPS == 8;
    assert KagemushaRecursiveSpendProver.MAXIMUM_RECURSIVE_PROOF_PAIR_BYTES_V4
        == 384 * 1024;
    assert KagemushaRecursiveSpendProver.MAX_PEER_ARCHIVE_BYTES_V2 == 32 * 1024;
    assert KagemushaRecursiveSpendProver.MAX_PEER_ARCHIVE_BYTES_V4 == 32 * 1024 * 1024;
    assert KagemushaRecursiveSpendProver.MAX_TOP_UP_PROVENANCE_ARCHIVE_BYTES_V4 == 6_488_064;
    assert KagemushaRecursiveSpendProver.MAX_PEER_ARCHIVE_BYTES
        == KagemushaRecursiveSpendProver.MAX_PEER_ARCHIVE_BYTES_V4;
    assert KagemushaPeerTransport.MAXIMUM_ARCHIVE_BYTES_V2 == 32 * 1024;
    assert KagemushaPeerTransport.MAXIMUM_ARCHIVE_BYTES_V4 == 32 * 1024 * 1024;
    assert KagemushaPeerTransport.MAXIMUM_ARCHIVE_BYTES
        == KagemushaPeerTransport.MAXIMUM_ARCHIVE_BYTES_V4;
    assert KagemushaRecursiveSpendProver.MAX_PEER_TEXT_ARCHIVE_BYTES == 24_576;
    assert KagemushaRecursiveSpendProver.MAX_RECIPIENT_RECEIVE_OFFER_BYTES_V2 == 24_576;
    assert KagemushaRecursiveSpendProver.MAX_PUBLISHER_CHECKPOINT_ENVELOPE_BYTES_V1 == 2_048;
    assert KagemushaRecursiveSpendProver.PROMOTED_FINALITY_CHECKPOINT_BYTES_V2 == 40;
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
    final Method[] recipientRequestMethods = Arrays.stream(methods)
        .filter(method -> Modifier.isPublic(method.getModifiers()))
        .filter(method -> method.getName().equals("prepareRecipientPaymentRequest"))
        .toArray(Method[]::new);
    assert recipientRequestMethods.length == 1;
    assert recipientRequestMethods[0].getParameterCount() == 13;
    assert recipientRequestMethods[0].getParameterTypes()[1] == int.class;
    final Method nativeRecipientRequest = Arrays.stream(methods)
        .filter(method -> method.getName().equals("nativePrepareRecipientRequestV2"))
        .findFirst()
        .orElseThrow();
    assert nativeRecipientRequest.getParameterCount() == 14;
    assert nativeRecipientRequest.getParameterTypes()[1] == int.class;
    final Method[] lineageQueryMethods = Arrays.stream(methods)
        .filter(method -> Modifier.isPublic(method.getModifiers()))
        .filter(method -> method.getName().equals("createRecipientLineageQueryV2"))
        .toArray(Method[]::new);
    assert lineageQueryMethods.length == 1;
    assert lineageQueryMethods[0].getParameterCount() == 6;
    assert lineageQueryMethods[0].getParameterTypes()[1] == int.class;
    final Method nativeLineageQuery = Arrays.stream(methods)
        .filter(method -> method.getName().equals("nativeCreateRecipientLineageQueryV2"))
        .findFirst()
        .orElseThrow();
    assert nativeLineageQuery.getParameterCount() == 6;
    assert nativeLineageQuery.getParameterTypes()[1] == int.class;
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
    final Method authorizationPrepareNative = Arrays.stream(methods)
        .filter(method -> method.getName().equals("nativePrepareAuthorizationV2"))
        .findFirst()
        .orElseThrow();
    assert authorizationPrepareNative.getReturnType() == byte[][].class;
    assert Arrays.equals(
        authorizationPrepareNative.getParameterTypes(),
        new Class<?>[] {
          byte[].class,
          int.class,
          byte[].class,
          byte[].class,
          byte[].class,
          long.class,
          long.class,
          byte[].class,
          byte[].class,
          byte[].class,
          byte[].class
        });
    final String retiredAuthorizationFinalizer =
        String.join("", "native", "Create", "Authorization", "V2");
    assert Arrays.stream(methods)
        .noneMatch(method -> method.getName().equals(retiredAuthorizationFinalizer));
    final Method authorizationFinalizeNative = Arrays.stream(methods)
        .filter(method -> method.getName().equals("nativeFinalizeHardwareAuthorizationV2"))
        .findFirst()
        .orElseThrow();
    assert authorizationFinalizeNative.getReturnType() == byte[][].class;
    assert Arrays.equals(
        authorizationFinalizeNative.getParameterTypes(),
        new Class<?>[] {byte[].class, byte[].class, byte[].class});
    final Method iosAuthorizationFinalizeNative = Arrays.stream(methods)
        .filter(method -> method.getName().equals("nativeFinalizeIosAppAttestAuthorizationV2"))
        .findFirst()
        .orElseThrow();
    assert iosAuthorizationFinalizeNative.getReturnType() == byte[][].class;
    assert Arrays.equals(
        iosAuthorizationFinalizeNative.getParameterTypes(),
        new Class<?>[] {byte[].class, byte[].class});
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

  private static void redemptionChangePreparationTransfersOpeningOwnershipExactlyOnce() {
    final KagemushaRecursiveSpendProver.NoteOpening opening =
        KagemushaRecursiveSpendProver.decodeNoteOpening(
            archive("KagemushaNoteOpeningV2"));
    final KagemushaRecursiveSpendProver.RedemptionChangePreparationV4 preparation =
        new KagemushaRecursiveSpendProver.RedemptionChangePreparationV4(
            opening,
            filled(0x21),
            filled(0x22),
            filled(0x23),
            filled(0x24),
            KagemushaScaledAmount.fromAtomicUnits("375", 2));
    final byte[] rho = preparation.rho();
    Arrays.fill(rho, (byte) 0);
    assert Arrays.equals(preparation.rho(), filled(0x21));
    assert "375".equals(preparation.amount().atomicUnits());

    final KagemushaRecursiveSpendProver.NoteOpening transferred = preparation.takeOpening();
    try {
      assert transferred == opening;
      assertThrowsIllegalState(preparation::takeOpening);

      preparation.close();
      preparation.close();
      assert !transferred.isDestroyed();
      assert transferred.noritoEncoded().length > 0;
      assertThrowsIllegalState(preparation::takeOpening);
      assertThrowsIllegalState(preparation::rho);
      assertThrowsIllegalState(preparation::commitment);
      assertThrowsIllegalState(preparation::amount);
    } finally {
      transferred.destroy();
    }
  }

  private static void redemptionChangePreparationDestroysOnlyAnUntransferredOpening() {
    final KagemushaRecursiveSpendProver.NoteOpening opening =
        KagemushaRecursiveSpendProver.decodeNoteOpening(
            archive("KagemushaNoteOpeningV2"));
    final KagemushaRecursiveSpendProver.RedemptionChangePreparationV4 preparation =
        new KagemushaRecursiveSpendProver.RedemptionChangePreparationV4(
            opening,
            filled(0x31),
            filled(0x32),
            filled(0x33),
            filled(0x34),
            KagemushaScaledAmount.fromAtomicUnits("125", 2));

    preparation.close();
    preparation.close();
    assert opening.isDestroyed();
    assertThrowsIllegalState(preparation::takeOpening);
    assertThrowsIllegalState(preparation::diversifier);
    assertThrowsIllegalState(preparation::amount);

    final KagemushaRecursiveSpendProver.NoteOpening rejectedOpening =
        KagemushaRecursiveSpendProver.decodeNoteOpening(
            archive("KagemushaNoteOpeningV2"));
    assertThrowsIllegalState(() ->
        new KagemushaRecursiveSpendProver.RedemptionChangePreparationV4(
            rejectedOpening,
            filled(0x41),
            filled(0x41),
            filled(0x43),
            filled(0x44),
            KagemushaScaledAmount.fromAtomicUnits("125", 2)));
    assert rejectedOpening.isDestroyed();
  }

  private static void redemptionChangeRequestFailureDestroysTransferredOpening() {
    final KagemushaRecursiveSpendProver.NoteOpening opening =
        KagemushaRecursiveSpendProver.decodeNoteOpening(
            archive("KagemushaNoteOpeningV2", 0x45));
    assertThrowsIllegalArgument(() ->
        KagemushaRecursiveSpendProver.decodeRedeemRequestV4(new byte[0], opening));
    assert opening.isDestroyed();

    final KagemushaRecursiveSpendProver.SpendableBranchV4 redeemInput = spendableBranch(0x46);
    final KagemushaRecursiveSpendProver.NoteOpening redeemChange =
        KagemushaRecursiveSpendProver.decodeNoteOpening(
            archive("KagemushaNoteOpeningV2", 0x47));
    try {
      assertThrowsNativeFailure(() -> KagemushaRecursiveSpendProver.buildRedeemRequestV4(
          redeemInput,
          "alice@wonderland",
          org.hyperledger.iroha.android.address.AccountAddress.DEFAULT_I105_DISCRIMINANT,
          KagemushaScaledAmount.fromAtomicUnits("125", 2),
          redeemChange,
          redemptionChangeOutputMembershipPaths(),
          filled(0x48),
          filled(0x49),
          1));
      assert redeemChange.isDestroyed();
    } finally {
      redeemInput.close();
    }

    final KagemushaRecursiveSpendProver.SpendableBranchV4 appendInput = spendableBranch(0x50);
    final KagemushaRecursiveSpendProver.NoteOpening appendChange =
        KagemushaRecursiveSpendProver.decodeNoteOpening(
            archive("KagemushaNoteOpeningV2", 0x51));
    try {
      assertThrowsNativeFailure(() -> KagemushaRecursiveSpendProver.buildAppendRequestV4(
          Collections.singletonList(appendInput),
          appendChange,
          appendChangeOutputMembershipPaths(),
          filled(0x52),
          filled(0x53),
          1));
      assert appendChange.isDestroyed();
    } finally {
      appendInput.close();
    }

    final KagemushaRecursiveSpendProver.NoteOpening restoreOpening =
        KagemushaRecursiveSpendProver.decodeNoteOpening(
            archive("KagemushaNoteOpeningV2", 0x54));
    assertThrowsIllegalArgument(() ->
        KagemushaRecursiveSpendProver.restoreSpendableBranchV4(
            KagemushaRecursiveSpendProver.decodeBundleV4(
                archive("KagemushaRecursiveSpendBundleV4", 0x55)),
            KagemushaRecursiveSpendProver.decodeNoteMembershipWitness(
                archive("KagemushaNoteMembershipWitnessV2", 0x56)),
            restoreOpening,
            KagemushaRecursiveSpendProver.decodeTopUpProvenanceV4(
                archive("KagemushaRecursiveSpendTopUpProvenanceV4", 0x57)),
            0));
    assert restoreOpening.isDestroyed();
  }

  private static void redemptionChangeCarriersCloseOrTransferOneOwnerExactlyOnce() {
    assert AutoCloseable.class.isAssignableFrom(
        KagemushaRecursiveSpendProver.SpendableBranchV4.class);
    final KagemushaRecursiveSpendProver.NoteOpening closeOwnedOpening =
        KagemushaRecursiveSpendProver.decodeNoteOpening(
            archive("KagemushaNoteOpeningV2", 0x4a));
    final KagemushaRecursiveSpendProver.RedeemRequestV4 closeOwnedRequest =
        KagemushaRecursiveSpendProver.decodeRedeemRequestV4(
            archive("KagemushaRecursiveSpendRedeemLocalRequestV4", 0x4b),
            closeOwnedOpening);
    closeOwnedRequest.close();
    closeOwnedRequest.close();
    assert closeOwnedOpening.isDestroyed();
    assertThrowsIllegalState(closeOwnedRequest::takeChangeOpening);

    final KagemushaRecursiveSpendProver.NoteOpening handedOffOpening =
        KagemushaRecursiveSpendProver.decodeNoteOpening(
            archive("KagemushaNoteOpeningV2", 0x4c));
    final KagemushaRecursiveSpendProver.RedeemRequestV4 request =
        KagemushaRecursiveSpendProver.decodeRedeemRequestV4(
            archive("KagemushaRecursiveSpendRedeemLocalRequestV4", 0x4d),
            handedOffOpening);
    final KagemushaRecursiveSpendProver.NoteOpening requestHandoff =
        request.takeChangeOpening();
    assert requestHandoff == handedOffOpening;
    request.close();
    request.close();
    assert !handedOffOpening.isDestroyed();
    assertThrowsIllegalState(request::takeChangeOpening);

    final KagemushaRecursiveSpendProver.RedeemBuildResultV4 result =
        KagemushaRecursiveSpendProver.decodeRedeemBuildResultV4(
            archive("KagemushaRecursiveSpendRedeemBuildResultV4", 0x4e),
            requestHandoff);
    final KagemushaRecursiveSpendProver.NoteOpening resultHandoff =
        result.takeChangeOpening();
    assert resultHandoff == handedOffOpening;
    result.close();
    result.close();
    assert !handedOffOpening.isDestroyed();
    assertThrowsIllegalState(result::takeChangeOpening);
    resultHandoff.destroy();
    assert handedOffOpening.isDestroyed();

    final KagemushaRecursiveSpendProver.SpendableBranchV4 spendableBranch =
        spendableBranch(0x4f);
    final KagemushaRecursiveSpendProver.NoteOpening spendableOpening = spendableBranch.opening();
    spendableBranch.close();
    spendableBranch.close();
    assert spendableOpening.isDestroyed();
  }

  private static void temporarySecretArchivesAreWipedAfterPartialConstructionAndOwnerCopy() {
    final byte[] first = filled(0x61);
    final byte[] second = filled(0x62);
    final byte[][] partiallyConstructed = new byte[][] {first, null, second};
    SecretArchiveWiper.wipeAll(partiallyConstructed);
    assert allZero(first);
    assert allZero(second);
    SecretArchiveWiper.wipeAll(null);

    final byte[] rawNativeArchive =
        archive("KagemushaRecursiveSpendRedeemLocalRequestV4", 0x63);
    final KagemushaRecursiveSpendProver.RedeemRequestV4 owner =
        KagemushaRecursiveSpendProver.decodeRedeemRequestV4(rawNativeArchive, null);
    SecretArchiveWiper.wipe(rawNativeArchive);
    assert allZero(rawNativeArchive);
    assert owner.noritoEncoded().length > NoritoHeader.HEADER_LENGTH;
    owner.close();
    SecretArchiveWiper.wipe(null);

    final List<byte[]> firstCopyObserved = new ArrayList<>();
    assertThrowsIllegalArgument(() ->
        SecretArchiveWiper.withOpeningDigests(
            filled(0x64),
            "spendKey",
            new byte[31],
            "rho",
            filled(0x66),
            "diversifier",
            firstCopyObserved::add,
            (spendKey, rho, diversifier) -> null));
    assert firstCopyObserved.size() == 1;
    assert allZero(firstCopyObserved.get(0));

    final List<byte[]> firstAndSecondCopiesObserved = new ArrayList<>();
    assertThrowsIllegalArgument(() ->
        SecretArchiveWiper.withOpeningDigests(
            filled(0x67),
            "spendKey",
            filled(0x68),
            "rho",
            new byte[31],
            "diversifier",
            firstAndSecondCopiesObserved::add,
            (spendKey, rho, diversifier) -> null));
    assert firstAndSecondCopiesObserved.size() == 2;
    assert allZero(firstAndSecondCopiesObserved.get(0));
    assert allZero(firstAndSecondCopiesObserved.get(1));

    final byte[] rawOpeningArchive = archive("KagemushaNoteOpeningV2", 0x69);
    final KagemushaRecursiveSpendProver.NoteOpening openingOwner =
        KagemushaRecursiveSpendProver.decodeNoteOpening(rawOpeningArchive);
    SecretArchiveWiper.wipe(rawOpeningArchive);
    assert allZero(rawOpeningArchive);
    assert openingOwner.noritoEncoded().length > NoritoHeader.HEADER_LENGTH;
    openingOwner.close();

    final byte[] rawInitArchive =
        archive("KagemushaRecursiveSpendInitLocalRequestV4", 0x6a);
    final KagemushaRecursiveSpendProver.InitRequestV4 initOwner =
        KagemushaRecursiveSpendProver.decodeInitRequestV4(rawInitArchive);
    SecretArchiveWiper.wipe(rawInitArchive);
    assert allZero(rawInitArchive);
    assert initOwner.noritoEncoded().length > NoritoHeader.HEADER_LENGTH;
    initOwner.close();
  }

  private static void noteOpeningAndInitRequestCloseIdempotently() {
    assert AutoCloseable.class.isAssignableFrom(
        KagemushaRecursiveSpendProver.NoteOpening.class);
    final KagemushaRecursiveSpendProver.NoteOpening opening =
        KagemushaRecursiveSpendProver.decodeNoteOpening(
            archive("KagemushaNoteOpeningV2", 0x5c));
    opening.close();
    opening.close();
    assert opening.isDestroyed();
    assertThrowsIllegalState(opening::noritoEncoded);

    assert AutoCloseable.class.isAssignableFrom(
        KagemushaRecursiveSpendProver.InitRequestV4.class);
    final KagemushaRecursiveSpendProver.InitRequestV4 init =
        KagemushaRecursiveSpendProver.decodeInitRequestV4(
            archive("KagemushaRecursiveSpendInitLocalRequestV4", 0x5d));
    init.close();
    init.close();
    assert init.isDestroyed();
    assertThrowsIllegalState(init::noritoEncoded);
  }

  private static void preparationConstructorFailuresDestroyStagedOpenings() {
    final KagemushaRecursiveSpendProver.NoteOpening recipientOpening =
        KagemushaRecursiveSpendProver.decodeNoteOpening(
            archive("KagemushaNoteOpeningV2", 0x5e));
    assertThrowsIllegalArgument(() ->
        construct(
            KagemushaRecursiveSpendProver.RecipientRequestPreparation.class,
            new Class<?>[] {
              KagemushaRecursiveSpendProver.RecipientRequestPayload.class,
              byte[].class,
              KagemushaRecursiveSpendProver.NoteOpening.class,
              byte[].class,
              byte[].class,
              KagemushaScaledAmount.class
            },
            construct(
                KagemushaRecursiveSpendProver.RecipientRequestPayload.class,
                new Class<?>[] {byte[].class},
                archive("KagemushaRecipientPaymentRequestSigningPayloadV2", 0x5f)),
            new byte[] {1},
            recipientOpening,
            filled(0x60),
            new byte[31],
            KagemushaScaledAmount.fromAtomicUnits("125", 2)));
    assert recipientOpening.isDestroyed();

    final KagemushaRecursiveSpendProver.NoteOpening topUpOpening =
        KagemushaRecursiveSpendProver.decodeNoteOpening(
            archive("KagemushaNoteOpeningV2", 0x70));
    assertThrowsIllegalArgument(() ->
        construct(
            KagemushaRecursiveSpendProver.TopUpPreparation.class,
            new Class<?>[] {
              KagemushaRecursiveSpendProver.TopUpUnsigned.class,
              byte[].class,
              KagemushaRecursiveSpendProver.NoteOpening.class,
              byte[].class,
              byte[].class,
              byte[].class,
              byte[].class,
              byte[].class,
              KagemushaScaledAmount.class,
              int.class
            },
            construct(
                KagemushaRecursiveSpendProver.TopUpUnsigned.class,
                new Class<?>[] {byte[].class},
                archive("KagemushaRecursiveSpendTopUpUnsignedV4", 0x71)),
            filled(0x72),
            topUpOpening,
            filled(0x73),
            filled(0x74),
            filled(0x75),
            filled(0x76),
            new byte[31],
            KagemushaScaledAmount.fromAtomicUnits("125", 2),
            0));
    assert topUpOpening.isDestroyed();
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
        "kagemusha-recursive-spend-step-eq-compact-layout-v5",
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
                "kagemusha-recursive-spend-step-eq-compact-layout-v5",
                4,
                20L),
            artifactSet,
            true)
        .chainArtifactSetReady();
    assert !readinessProjection(transfer, unshield, stepEq, null, true)
        .chainArtifactSetReady();
    assert !readinessProjection(transfer, unshield, stepEq, artifactSet, false)
        .chainArtifactSetReady();
    assertThrowsIllegalArgument(() -> readinessProjection(
        "cash_handoff_v2", transfer, unshield, stepEq, artifactSet, true));
    assertThrowsIllegalArgument(() -> readinessProjection(
        null, transfer, unshield, stepEq, artifactSet, true));

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
    return readinessProjection(
        KagemushaRecursiveSpendProver.CASH_HANDOFF_CAPABILITY_V1,
        transfer,
        unshield,
        stepEq,
        artifactSet,
        proofBackendAvailable);
  }

  private static KagemushaRecursiveSpendProver.ReadinessProjection readinessProjection(
      final String cashHandoffCapability,
      final KagemushaRecursiveSpendProver.ActiveVerifier transfer,
      final KagemushaRecursiveSpendProver.ActiveVerifier unshield,
      final KagemushaRecursiveSpendProver.ActiveVerifier stepEq,
      final KagemushaRecursiveSpendProver.AuthenticatedArtifactSet artifactSet,
      final boolean proofBackendAvailable) {
    return new KagemushaRecursiveSpendProver.ReadinessProjection(
        cashHandoffCapability,
        21,
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
            "kagemusha-recursive-spend-step-ep-compact-lineage-v5",
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
              archive("iroha_data_model::offline::model::KagemushaRecipientPaymentRequestV2")),
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
      assert init.isDestroyed();
      assertThrowsIllegalState(() -> KagemushaRecursiveSpendProver.initSpendV4(init));
    }
    init.close();
    init.close();
    assert init.isDestroyed();
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
            archive("iroha_data_model::offline::model::KagemushaRecursiveSpendPeerPaymentV4"));
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
    assert amount.fixedScaleDecimal().equals("10.750000000");
    assert amount.displayDecimal().equals("10.75");
    assert KagemushaScaledAmount.sum(
            Arrays.asList(
                KagemushaScaledAmount.fromDecimal("4.50", 9),
                KagemushaScaledAmount.fromDecimal("6.25", 9)))
        .atomicUnits().equals("10750000000");
    assert KagemushaScaledAmount.fromAtomicUnits("1", 9)
        .fixedScaleDecimal().equals("0.000000001");
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
    requireNativeArtifactStreaming();
    final byte[] requestArchive = archive(
        "iroha_data_model::offline::model::KagemushaRecipientPaymentRequestV2");
    final KagemushaRecursiveSpendProver.RecipientPaymentRequest request =
        KagemushaRecursiveSpendProver.decodeRecipientPaymentRequest(requestArchive);
    requestArchive[requestArchive.length - 1] ^= 1;
    assert request.noritoEncoded()[request.noritoEncoded().length - 1] == 0x51;

    final byte[] offerBytes = portableOfferFixture("offline_recipient_receive_offer_v2.hex");
    final KagemushaRecursiveSpendProver.RecipientReceiveOfferV2 offer =
        KagemushaRecursiveSpendProver.decodeRecipientReceiveOfferV2(offerBytes);
    final KagemushaRecursiveSpendProver.RecipientReceiveOfferProjectionV2 offerProjection =
        KagemushaRecursiveSpendProver.projectRecipientReceiveOfferV2(offer);
    assert Arrays.equals(
        portableOfferFixture("offline_recipient_payment_request_v2.hex"),
        offerProjection.request().noritoEncoded());
    assert Arrays.equals(
        portableOfferFixture("offline_recipient_registration_lineage_v2.hex"),
        offerProjection.lineage().noritoEncoded());
    assert Arrays.equals(
        portableOfferFixture("offline_recipient_checkpoint_envelope.hex"),
        offerProjection.publisherCheckpointEnvelope());

    assert KagemushaRecursiveSpendProver.decodePeerPayment(
            archive("iroha_data_model::offline::model::KagemushaRecursiveSpendPeerPaymentV4"))
        .noritoEncoded().length > NoritoHeader.HEADER_LENGTH;
    assert KagemushaRecursiveSpendProver.decodeReceiverAcknowledgement(
            archive("iroha_data_model::offline::model::KagemushaReceiverAcknowledgementV2"))
        .noritoEncoded().length > NoritoHeader.HEADER_LENGTH;
    assert KagemushaRecursiveSpendProver.decodeNoteMembershipWitness(
            archive("KagemushaNoteMembershipWitnessV2"))
        .noritoEncoded().length > NoritoHeader.HEADER_LENGTH;

    boolean rejected = false;
    try {
      KagemushaRecursiveSpendProver.decodePeerPayment(
          archive("iroha_data_model::offline::model::KagemushaRecipientPaymentRequestV2"));
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

    final byte[] corrupted = archive(
        "iroha_data_model::offline::model::KagemushaRecursiveSpendPeerPaymentV4");
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
    final int padding = schema.equals(
            "iroha_data_model::offline::model::KagemushaRecipientPaymentRequestV2")
        || schema.equals(
            "iroha_data_model::offline::model::KagemushaRecursiveSpendPeerPaymentV4")
        ? 8 : 0;
    final byte[] archive = new byte[NoritoHeader.HEADER_LENGTH + padding + payload.length];
    System.arraycopy(header.encode(), 0, archive, 0, NoritoHeader.HEADER_LENGTH);
    System.arraycopy(payload, 0, archive, NoritoHeader.HEADER_LENGTH + padding, payload.length);
    return archive;
  }

  private static void peerTransportGoldenVectorsAreExact() {
    requireNativeArtifactStreaming();
    final byte[] offerArchive = portableOfferFixture(
        "offline_recipient_receive_offer_v2.hex");
    final KagemushaPeerTransport.Payload request =
        KagemushaPeerTransport.Payload.decode(
            offerArchive,
            KagemushaPeerTransport.Kind.RECEIVE_REQUEST);
    final String text = KagemushaPeerTransport.encode(request);
    assert text.startsWith("PKK2R.");
    assert Arrays.equals(offerArchive, KagemushaPeerTransport.decode(text).archive());
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
    requireNativeArtifactStreaming();
    final byte[] offerArchive = portableOfferFixture(
        "offline_recipient_receive_offer_v2.hex");
    final KagemushaPeerTransport.Payload request =
        KagemushaPeerTransport.Payload.decode(
            offerArchive,
            KagemushaPeerTransport.Kind.RECEIVE_REQUEST);
    final List<String> frames = KagemushaQrStream.encode(
        request, KagemushaQrStream.Options.STANDARD);
    assert frames.size() > 3;
    assert frames.stream().allMatch(value -> value.startsWith("PKKQ1."));
    final KagemushaQrStream.Decoder decoder = new KagemushaQrStream.Decoder();
    KagemushaQrStream.DecodeResult recovered = null;
    for (final String frame : frames) recovered = decoder.ingest(frame);
    assert recovered != null;
    assert recovered.isComplete();
    assert Arrays.equals(offerArchive, recovered.payload().archive());

    final byte[] rawArchive = request.archive();
    final List<byte[]> apdus = KagemushaNfcProtocol.writePayloadApdus(
        KagemushaNfcProtocol.PayloadKind.RECEIVE_REQUEST, rawArchive, 220);
    assert apdus.size() > 3;
    assert hex(apdus.get(apdus.size() - 1)).equals("8022040000");
    assert KagemushaNfcProtocol.AID_HEX.equals("F0504B45504B524E464301");
    assert KagemushaNfcProtocol.AID_HEX.equals(IrohaPeerNfcV1.APPLICATION_IDENTIFIER_HEX);
    assert KagemushaPeerTransport.NFC_APPLICATION_IDENTIFIER_HEX
        .equals(IrohaPeerNfcV1.APPLICATION_IDENTIFIER_HEX);
    assert KagemushaNfcProtocol.SAFE_CHUNK_BYTES == 220;
    assert KagemushaNfcProtocol.RAW_TRANSPORT_VERSION == 4;
    assert KagemushaNfcProtocol.parseCommand(apdus.get(0)).type()
        == KagemushaNfcProtocol.Type.WRITE_META;

    final byte[] nearby = KagemushaNearby.encode(request, KagemushaNearby.PairingSymbol.STARS);
    assert nearby.length
        == offerArchive.length + IrohaPeerWireMessageV1.HEADER_LENGTH + KagemushaNearby.HEADER_LENGTH;
    assert hex(Arrays.copyOfRange(nearby, 0, 8)).equals("504b4e4231010100");
    assert readU32Be(Arrays.copyOfRange(nearby, 8, 12)) == nearby.length - 12;
    assert KagemushaNearby.decode(nearby).payload().kind()
        == KagemushaPeerTransport.Kind.RECEIVE_REQUEST;
    assert Arrays.equals(offerArchive, KagemushaNearby.decode(nearby).payload().archive());
    assert hex(KagemushaNearby.encodeRejection()).equals("504b4e423104000000000000");
    assert KagemushaNearby.decode(KagemushaNearby.encodeRejection()).messageKind()
        == KagemushaNearby.MessageKind.REJECTED;
    assert !KagemushaNearby.IS_AVAILABLE;
    Arrays.fill(rawArchive, (byte) 0);
    Arrays.fill(nearby, (byte) 0);
  }

  private static void requireNativeArtifactStreaming() {
    if (!KagemushaRecursiveSpendProver.isArtifactStreamingAvailable()) {
      throw new AssertionError(
          "A freshly built connect_norito_bridge ABI 22 artifact-streaming library is required");
    }
  }

  private static byte[] portableOfferFixture(final String name) {
    Path current = Paths.get(System.getProperty("user.dir")).toAbsolutePath();
    while (current != null) {
      final Path candidate = current.resolve("crates/connect_norito_bridge/tests/fixtures")
          .resolve(name);
      if (Files.isRegularFile(candidate)) {
        try {
          final String hex = new String(Files.readAllBytes(candidate), StandardCharsets.US_ASCII)
              .replaceAll("\\s+", "");
          final byte[] bytes = new byte[hex.length() / 2];
          for (int index = 0; index < bytes.length; index++) {
            bytes[index] = (byte) Integer.parseInt(hex.substring(index * 2, index * 2 + 2), 16);
          }
          return bytes;
        } catch (final java.io.IOException failure) {
          throw new AssertionError("unable to load portable Kagemusha fixture", failure);
        }
      }
      current = current.getParent();
    }
    throw new AssertionError("portable Kagemusha fixture is missing: " + name);
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

  private static int readU32Be(final byte[] bytes) {
    assert bytes.length == 4;
    return ((bytes[0] & 0xff) << 24)
        | ((bytes[1] & 0xff) << 16)
        | ((bytes[2] & 0xff) << 8)
        | (bytes[3] & 0xff);
  }

  private static void toriiLifecycleRoutesAndHeadersAreExact() {
    final NetworkId networkId =
        NetworkId.parse(
            "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0");
    final NetworkId otherNetworkId =
        NetworkId.parse(
            "hash:0E5751C026E543B2E8AB2EB06099DAA1D1E5DF47778F7787FAAB45CDF12FE3A9#6A22");
    final KeyPair keyPair = generateEd25519KeyPair();
    final long timestampMs = 1_700_000_000_500L;
    final String nonce = "offline-lineage-1";
    final ToriiCanonicalRequestAuth canonicalAuth =
        new ToriiCanonicalRequestAuth(
            "alice@universal",
            message -> signEd25519(keyPair.getPrivate(), message),
            timestampMs,
            nonce);
    final AtomicReference<TransportRequest> captured = new AtomicReference<>();
    final KagemushaRecursiveSpendProver.ToriiClient client =
        KagemushaRecursiveSpendProver.newToriiClient(
            URI.create("https://torii.example/api/"),
            request -> {
              captured.set(request);
              final boolean capability = request.uri().getPath().endsWith("/readiness");
              final boolean lineage = request.uri().getPath().endsWith("/receiver-lineage");
              final boolean command = "POST".equals(request.method()) && !lineage;
              return CompletableFuture.completedFuture(
                  TransportResponse.builder()
                      .setStatusCode(command ? 202 : 200)
                      .addHeader(
                          "Content-Type",
                          capability ? "application/json" : "application/x-norito")
                      .setBody(
                          capability
                              ? universalOfflineCapabilityJson().getBytes(StandardCharsets.UTF_8)
                              : archive(
                                  command
                                      ? "OfflineOperationReference"
                                      : lineage
                                          ? "iroha_torii_shared::offline_api::OfflineRecipientRegistrationLineage"
                                      : request.uri().getPath().contains("/operations/")
                                          ? "OfflineOperationStatus"
                                          : unexpectedToriiRoute(request)))
                      .build());
            },
            new LocalSigningContext(networkId));

    final KagemushaRecursiveSpendProver.OfflineStatus status =
        client.getOfflineCapability().join();
    assert Arrays.stream(KagemushaRecursiveSpendProver.ToriiClient.class.getDeclaredMethods())
        .noneMatch(method -> method.getName().equals("getReadiness"))
        : "selector-taking offline readiness alias must remain absent";
    assert !status.mandatory();
    assert status.cashHandoffCapability().equals("cash_handoff_v1");
    assert status.requiredBridgeAbiVersion() == 22;
    assert status.maximumHops() == 8;
    assert status.ready();
    assert status.assets().isEmpty();
    assert status.blockers().isEmpty();
    assert captured.get().uri().toString()
        .equals("https://torii.example/api/v1/offline/readiness");
    assert captured.get().headers().get("Accept").equals(Arrays.asList("application/json"));

    final KagemushaRecursiveSpendProver.RecipientLineageQueryV2 query = construct(
        KagemushaRecursiveSpendProver.RecipientLineageQueryV2.class,
        new Class<?>[] {byte[].class},
        archive("iroha_torii_shared::offline_api::OfflineRecipientLineageRequest"));
    client.getRecipientRegistrationLineage(query, canonicalAuth).join();
    final TransportRequest lineageRequest = captured.get();
    assert lineageRequest.uri().getPath().equals("/api/v1/offline/receiver-lineage");
    assert lineageRequest.headers().get("Content-Type")
        .equals(Arrays.asList("application/x-norito"));
    assert lineageRequest.headers().get(CanonicalRequestSigner.HEADER_ACCOUNT)
        .equals(Arrays.asList("alice@universal"));
    assert lineageRequest.headers().get(CanonicalRequestSigner.HEADER_TIMESTAMP_MS)
        .equals(Arrays.asList(Long.toString(timestampMs)));
    assert lineageRequest.headers().get(CanonicalRequestSigner.HEADER_NONCE)
        .equals(Arrays.asList(nonce));
    assert lineageRequest.replayPolicy() == RequestReplayPolicy.ONE_SHOT;
    final byte[] signature =
        Base64.getDecoder()
            .decode(
                lineageRequest.headers().get(CanonicalRequestSigner.HEADER_SIGNATURE).get(0));
    assert verifyEd25519(
        keyPair,
        CanonicalRequestSigner.canonicalRequestSignatureMessage(
            networkId,
            lineageRequest.method(),
            lineageRequest.uri(),
            lineageRequest.body(),
            timestampMs,
            nonce),
        signature);
    assert !verifyEd25519(
        keyPair,
        CanonicalRequestSigner.canonicalRequestSignatureMessage(
            otherNetworkId,
            lineageRequest.method(),
            lineageRequest.uri(),
            lineageRequest.body(),
            timestampMs,
            nonce),
        signature);
    assert !verifyEd25519(
        keyPair,
        CanonicalRequestSigner.canonicalRequestSignatureMessage(
            networkId,
            "GET",
            lineageRequest.uri(),
            lineageRequest.body(),
            timestampMs,
            nonce),
        signature);
    assert !verifyEd25519(
        keyPair,
        CanonicalRequestSigner.canonicalRequestSignatureMessage(
            networkId,
            lineageRequest.method(),
            URI.create("https://torii.example/api/v1/offline/readiness"),
            lineageRequest.body(),
            timestampMs,
            nonce),
        signature);
    final byte[] substitutedBody =
        Arrays.copyOf(lineageRequest.body(), lineageRequest.body().length + 1);
    assert !verifyEd25519(
        keyPair,
        CanonicalRequestSigner.canonicalRequestSignatureMessage(
            networkId,
            lineageRequest.method(),
            lineageRequest.uri(),
            substitutedBody,
            timestampMs,
            nonce),
        signature);

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

  private static String universalOfflineCapabilityJson() {
    return "{\"mandatory\":false,\"cash_handoff_capability\":\"cash_handoff_v1\","
        + "\"required_bridge_abi_version\":22,\"max_hops\":8,\"ready\":true,"
        + "\"assets\":[],\"blockers\":[]}";
  }

  private static String unexpectedToriiRoute(final TransportRequest request) {
    throw new AssertionError("unexpected Torii route " + request.uri());
  }

  private static void offlineCapabilityRejectsBackendReadinessClaims() {
    final List<String> invalidPayloads = Arrays.asList(
        "{\"mandatory\":true,\"cash_handoff_capability\":\"cash_handoff_v1\",\"required_bridge_abi_version\":22,\"max_hops\":8,\"ready\":true,\"assets\":[],\"blockers\":[]}",
        "{\"mandatory\":false,\"cash_handoff_capability\":\"cash_handoff_v2\",\"required_bridge_abi_version\":22,\"max_hops\":8,\"ready\":true,\"assets\":[],\"blockers\":[]}",
        "{\"mandatory\":false,\"cash_handoff_capability\":\"cash_handoff_v1\",\"required_bridge_abi_version\":21,\"max_hops\":8,\"ready\":true,\"assets\":[],\"blockers\":[]}",
        "{\"mandatory\":false,\"cash_handoff_capability\":\"cash_handoff_v1\",\"required_bridge_abi_version\":22,\"max_hops\":9,\"ready\":true,\"assets\":[],\"blockers\":[]}",
        "{\"mandatory\":false,\"cash_handoff_capability\":\"cash_handoff_v1\",\"required_bridge_abi_version\":22,\"max_hops\":8,\"ready\":false,\"assets\":[],\"blockers\":[]}",
        "{\"mandatory\":false,\"cash_handoff_capability\":\"cash_handoff_v1\",\"required_bridge_abi_version\":22,\"max_hops\":8,\"ready\":true,\"assets\":[{}],\"blockers\":[]}",
        "{\"mandatory\":false,\"cash_handoff_capability\":\"cash_handoff_v1\",\"required_bridge_abi_version\":22,\"max_hops\":8,\"ready\":true,\"assets\":[],\"blockers\":[{\"code\":\"unexpected\",\"message\":\"unexpected\"}]}",
        "{\"mandatory\":false,\"cash_handoff_capability\":\"cash_handoff_v1\",\"required_bridge_abi_version\":22,\"max_hops\":8,\"ready\":true,\"assets\":[],\"blockers\":[],\"future\":true}");
    for (final String payload : invalidPayloads) {
      final KagemushaRecursiveSpendProver.ToriiClient client =
          KagemushaRecursiveSpendProver.newToriiClient(
              URI.create("https://torii.example"),
              request -> CompletableFuture.completedFuture(
                  TransportResponse.builder()
                      .setStatusCode(200)
                      .addHeader("Content-Type", "application/json")
                      .setBody(payload.getBytes(StandardCharsets.UTF_8))
                      .build()),
              new LocalSigningContext(
                  NetworkId.parse(
                      "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0")));
      boolean rejected = false;
      try {
        client.getOfflineCapability().join();
      } catch (final RuntimeException expected) {
        rejected = true;
      }
      assert rejected : "accepted non-universal offline capability: " + payload;
    }
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
            "createRecipientLineageQueryV2",
            "createRecipientReceiveOfferV2",
            "decodeAppendRequestV4",
            "decodeBundleV4",
            "decodeInitRequestV4",
            "decodeInitResultV4",
            "decodeNoteMembershipWitness",
            "decodeNoteOpening",
            "decodeOutputMembershipFrontierV4",
            "decodePeerPayment",
            "decodeReadiness",
            "decodeRedeemRequestV4",
            "decodeReceiverAcknowledgement",
            "decodeRecipientPaymentRequest",
            "decodeRecipientReceiveOfferV2",
            "decodeRecipientRegistrationLineageV2",
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
            "finalizeIosAppAttest",
            "finalizeRequestAuthorization",
            "finalizeRedeemV4",
            "finalizeTopUp",
            "deriveOutputMembershipPathsV4",
            "initSpendV4",
            "installedArtifactManifestSha256V4",
            "isArtifactStreamingAvailable",
            "isProductionProofBackendCompiled",
            "isProofBackendAvailable",
            "newToriiClient",
            "prepareAcknowledgement",
            "prepareNoteOpening",
            "preparePeerSplitChangeV4",
            "prepareRedemptionChangeV4",
            "prepareRecipientPaymentRequest",
            "prepareRequestAuthorization",
            "prepareTopUp",
            "projectInitResultV4",
            "projectOperationStatus",
            "projectPeerPayment",
            "projectRecipientPaymentRequest",
            "projectRecipientReceiveOfferV2",
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
            "verifyAcknowledgement",
            "verifyRecipientPaymentRequest",
            "verifyRecipientReceiveOfferV2",
            "verifyRecipientRegistrationLineageV2",
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

  private static KagemushaRecursiveSpendProver.OutputMembershipPaths
      appendChangeOutputMembershipPaths() {
    final byte[] initialRoot = filled(0x31);
    final byte[] afterRecipientRoot = filled(0x32);
    final byte[] finalRoot = filled(0x33);
    return new KagemushaRecursiveSpendProver.OutputMembershipPaths(
        initialRoot,
        finalRoot,
        new KagemushaRecursiveSpendProver.OutputMembershipLeafPaths(
            outputMembershipPath(initialRoot, 0), outputMembershipPath(finalRoot, 0)),
        new KagemushaRecursiveSpendProver.OutputMembershipLeafPaths(
            outputMembershipPath(afterRecipientRoot, 1), outputMembershipPath(finalRoot, 1)),
        outputMembershipPath(finalRoot, 2));
  }

  private static KagemushaRecursiveSpendProver.OutputMembershipPaths
      redemptionChangeOutputMembershipPaths() {
    final byte[] initialRoot = filled(0x41);
    final byte[] finalRoot = filled(0x42);
    return new KagemushaRecursiveSpendProver.OutputMembershipPaths(
        initialRoot,
        finalRoot,
        null,
        new KagemushaRecursiveSpendProver.OutputMembershipLeafPaths(
            outputMembershipPath(initialRoot, 0), outputMembershipPath(finalRoot, 0)),
        outputMembershipPath(finalRoot, 1));
  }

  private static KagemushaRecursiveSpendProver.SpendableBranchV4 spendableBranch(
      final int seed) {
    try {
      final java.lang.reflect.Constructor<KagemushaRecursiveSpendProver.SpendableBranchV4>
          constructor = KagemushaRecursiveSpendProver.SpendableBranchV4.class
              .getDeclaredConstructor(
                  KagemushaRecursiveSpendProver.BundleV4.class,
                  KagemushaRecursiveSpendProver.NoteMembershipWitness.class,
                  KagemushaRecursiveSpendProver.NoteOpening.class,
                  KagemushaRecursiveSpendProver.TopUpProvenanceV4.class,
                  KagemushaRecursiveSpendProver.OutputMembershipFrontierV4.class);
      constructor.setAccessible(true);
      return constructor.newInstance(
          KagemushaRecursiveSpendProver.decodeBundleV4(
              archive("KagemushaRecursiveSpendBundleV4", seed)),
          KagemushaRecursiveSpendProver.decodeNoteMembershipWitness(
              archive("KagemushaNoteMembershipWitnessV2", seed + 1)),
          KagemushaRecursiveSpendProver.decodeNoteOpening(
              archive("KagemushaNoteOpeningV2", seed + 2)),
          KagemushaRecursiveSpendProver.decodeTopUpProvenanceV4(
              archive("KagemushaRecursiveSpendTopUpProvenanceV4", seed + 3)),
          KagemushaRecursiveSpendProver.decodeOutputMembershipFrontierV4(
              archive(
                  "connect_norito_bridge::KagemushaOutputMembershipFrontierV4",
                  seed + 4)));
    } catch (final ReflectiveOperationException failure) {
      throw new AssertionError("failed to construct a spendable test branch", failure);
    }
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

  private static KeyPair generateEd25519KeyPair() {
    try {
      return KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
    } catch (final Exception error) {
      throw new AssertionError(error);
    }
  }

  private static byte[] signEd25519(final PrivateKey privateKey, final byte[] message) {
    try {
      final Signature signer = Signature.getInstance("Ed25519");
      signer.initSign(privateKey);
      signer.update(message);
      return signer.sign();
    } catch (final Exception error) {
      throw new AssertionError(error);
    }
  }

  private static boolean verifyEd25519(
      final KeyPair keyPair, final byte[] message, final byte[] signature) {
    try {
      final Signature verifier = Signature.getInstance("Ed25519");
      verifier.initVerify(keyPair.getPublic());
      verifier.update(message);
      return verifier.verify(signature);
    } catch (final Exception error) {
      throw new AssertionError(error);
    }
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

  private static void assertThrowsNativeFailure(final Runnable action) {
    try {
      action.run();
      assert false : "expected fail-closed native builder failure";
    } catch (final RuntimeException | UnsatisfiedLinkError expected) {
      // Expected when no exact native proof backend is installed for source-level tests.
    }
  }

  private static boolean allZero(final byte[] value) {
    for (final byte byteValue : value) {
      if (byteValue != 0) return false;
    }
    return true;
  }

  private static Method declaredMethod(final String name, final Class<?>... parameterTypes) {
    try {
      return KagemushaRecursiveSpendProver.class.getDeclaredMethod(name, parameterTypes);
    } catch (final ReflectiveOperationException failure) {
      throw new AssertionError("missing method " + name, failure);
    }
  }

  private static <T> T construct(
      final Class<T> type, final Class<?>[] parameterTypes, final Object... arguments) {
    try {
      final Constructor<T> constructor = type.getDeclaredConstructor(parameterTypes);
      constructor.setAccessible(true);
      return constructor.newInstance(arguments);
    } catch (final InvocationTargetException failure) {
      final Throwable cause = failure.getCause();
      if (cause instanceof RuntimeException) throw (RuntimeException) cause;
      if (cause instanceof Error) throw (Error) cause;
      throw new AssertionError("constructor failed", cause);
    } catch (final ReflectiveOperationException failure) {
      throw new AssertionError("missing constructor for " + type.getName(), failure);
    }
  }

  private static String repeat(final String value, final int count) {
    final StringBuilder repeated = new StringBuilder(value.length() * count);
    for (int index = 0; index < count; index++) repeated.append(value);
    return repeated.toString();
  }
}
