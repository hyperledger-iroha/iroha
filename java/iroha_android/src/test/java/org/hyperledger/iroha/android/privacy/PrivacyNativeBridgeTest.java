package org.hyperledger.iroha.android.privacy;

import java.util.Arrays;
import java.util.List;

public final class PrivacyNativeBridgeTest {

  private PrivacyNativeBridgeTest() {}

  public static void main(final String[] args) {
    exposesStableFailClosedErrorCodes();
    reportsFailClosedPrivacyCapabilities();
    rejectsEmptyRequestsBeforeNativeDispatch();
    nativeAvailabilityProbeArchiveIsStableAndDefensive();
    nativeProbeRequiresAbiAndAllPrivacySymbols();
    rejectsNullAndEmptyNativeOutputs();
    rejectsInvalidNoritoNativeOutputs();
    rejectsWrongOperationSchemaNativeOutputs();
    rejectsInvalidNoritoRequestsBeforeNativeDispatch();
    rejectsWrongSchemaRequestsBeforeNativeDispatch();
    nativeDispatchReturnsDefensiveOutputCopy();
    acceptsCompleteFieldBitsetNoritoFlags();
    nativeExceptionsAreSanitizedBeforeExposingRequestBytes();
    nativeDispatchClearsTemporaryRequestCopyWithoutMutatingCallerArchive();
    hostileNativeRequestMutationCannotMutateCallerArchive();
    System.out.println("[IrohaAndroid] PrivacyNativeBridgeTest passed.");
  }

  private static void exposesStableFailClosedErrorCodes() {
    assert PrivacyNativeBridge.REQUIRED_BRIDGE_ABI_VERSION == 6;
    assert PrivacyNativeBridge.PRIVACY_FFI_VERSION_V1 == 1;
    assert PrivacyNativeBridge.STATUS_ERROR == 1;
    assert PrivacyNativeBridge.ERROR_NULL_POINTER == 1;
    assert PrivacyNativeBridge.ERROR_MALFORMED_NORITO == 2;
    assert PrivacyNativeBridge.ERROR_UNSUPPORTED_ALGORITHM == 3;
    assert PrivacyNativeBridge.ERROR_PRODUCTION_DISABLED == 4;
    assert PrivacyNativeBridge.ERROR_INVALID_REQUEST == 5;
    assert PrivacyNativeBridge.PRIVACY_NATIVE_ARCHIVE_MAX_BYTES == 64 * 1024 * 1024;
  }

  private static void reportsFailClosedPrivacyCapabilities() {
    final PrivacyNativeBridge.PrivacyCapabilities current =
        PrivacyNativeBridge.privacyCapabilities();
    assert current.isAndroidSdkAvailable();
    assert current.isBridgeAvailable() == PrivacyNativeBridge.isNativeAvailable();
    assertFailClosedProductionGate(current);

    final PrivacyNativeBridge.PrivacyCapabilities bridgeAvailable =
        PrivacyNativeBridge.privacyCapabilities(true);
    assert bridgeAvailable.isAndroidSdkAvailable();
    assert bridgeAvailable.isBridgeAvailable();
    assertFailClosedProductionGate(bridgeAvailable);

    final PrivacyNativeBridge.PrivacyCapabilities bridgeUnavailable =
        PrivacyNativeBridge.privacyCapabilities(false);
    assert bridgeUnavailable.isAndroidSdkAvailable();
    assert !bridgeUnavailable.isBridgeAvailable();
    assertFailClosedProductionGate(bridgeUnavailable);

    final PrivacyNativeBridge.PrivacyCapabilities fresh =
        PrivacyNativeBridge.privacyCapabilities(true);
    assert !fresh.missingProductionGates().contains("tampered");
    assert !fresh.auditReferences().contains("https://audit.example/forged-signoff");
    assert fresh.missingProductionGates().equals(bridgeAvailable.missingProductionGates());
    assert fresh.auditReferences().equals(bridgeAvailable.auditReferences());
  }

  private static void assertFailClosedProductionGate(
      final PrivacyNativeBridge.PrivacyCapabilities capabilities) {
    assert PrivacyNativeBridge.PRODUCTION_GATE_VERSION.equals(
        capabilities.productionGateVersion());
    assert !capabilities.isProductionReady();
    assert !capabilities.hasRealProving();
    assert !capabilities.hasRealVerification();
    assert !capabilities.hasChainAdmission();
    assert !capabilities.hasSdkParity();
    assert !capabilities.hasWalletState();
    assert !capabilities.hasDeterministicTests();
    assert !capabilities.hasFuzzing();
    assert !capabilities.hasPerformanceGates();
    assert !capabilities.hasExternalAudit();
    assert capabilities.auditReferences().isEmpty();
    assert capabilities.missingProductionGates().equals(expectedProductionGateMissingReasons());
    assert capabilities.missingProductionGates().contains(
        "real proving engine is not registered");
    assert capabilities.missingProductionGates().contains(
        "chain admission path is not enabled");
    assert capabilities.missingProductionGates().contains(
        "external audit signoff is missing");
    assert capabilities.missingProductionGates().contains(
        "implementation stage is not production-hardened");
    assert capabilities.missingProductionGates().contains(
        "planned SDK entrypoints remain");
    assert capabilities.missingProductionGates().contains(
        "dev fixture entrypoints are not production entrypoints");
    assert capabilities.missingProductionGates().contains(
        "Iroha production allowlist is not enabled for this audited row");
    assertUnsupportedOperation(
        () -> capabilities.missingProductionGates().add("tampered"));
    assertUnsupportedOperation(
        () -> capabilities.auditReferences().add("https://audit.example/forged-signoff"));
  }

  private static List<String> expectedProductionGateMissingReasons() {
    return Arrays.asList(
        "real proving engine is not registered",
        "real verifier is not registered",
        "chain admission path is not enabled",
        "cross-SDK parity is incomplete",
        "wallet/state support is incomplete",
        "deterministic tests are incomplete",
        "fuzzing gate is incomplete",
        "performance gate is incomplete",
        "external audit signoff is missing",
        "implementation stage is not production-hardened",
        "planned SDK entrypoints remain",
        "dev fixture entrypoints are not production entrypoints",
        "Iroha production allowlist is not enabled for this audited row");
  }

  private static void rejectsEmptyRequestsBeforeNativeDispatch() {
    assertThrows(() -> PrivacyNativeBridge.buildProof(new byte[0]));
    assertThrows(() -> PrivacyNativeBridge.verifyProof(new byte[0]));
    assertThrows(() -> PrivacyNativeBridge.buildProof(null));
    assertThrows(() -> PrivacyNativeBridge.verifyProof(null));
    final byte[] oversized = new byte[PrivacyNativeBridge.PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1];
    assertThrows(() -> PrivacyNativeBridge.buildProof(oversized));
    assertThrows(() -> PrivacyNativeBridge.verifyProof(oversized));
    assertThrows(() -> PrivacyNativeBridge.buildProof(privacyNoritoFrame(0x52)));
    assertThrows(() -> PrivacyNativeBridge.verifyProof(privacyNoritoFrame(0x52)));
  }

  private static void nativeAvailabilityProbeArchiveIsStableAndDefensive() {
    final byte[] first = PrivacyNativeBridge.privacyNativeAvailabilityProbeArchive();
    final byte[] second = PrivacyNativeBridge.privacyNativeAvailabilityProbeArchive();

    assert first != second;
    assert Arrays.equals(first, privacyNoritoFrame(0x52));
    assert PrivacyNativeBridge.isValidPrivacyNoritoArchive(first);
    assert !Arrays.equals(
        first,
        "iroha-privacy-native-availability-probe-v1"
            .getBytes(java.nio.charset.StandardCharsets.UTF_8));
    first[0] = 0x7f;
    assert Arrays.equals(second, privacyNoritoFrame(0x52));
  }

  private static void nativeProbeRequiresAbiAndAllPrivacySymbols() {
    assert !PrivacyNativeBridge.returnsOutputProbe(() -> privacyNoritoFrame(0x50));
    assert PrivacyNativeBridge.returnsOutputProbe(() -> privacyNoritoFrameWithPayload(0x51));
    assert PrivacyNativeBridge.returnsOutputProbe(0x50, () -> privacyNoritoFrameWithPadding(0x50, 64));
    assert !PrivacyNativeBridge.returnsOutputProbe(0x50, () -> privacyNoritoFrame(0x50));
    assert PrivacyNativeBridge.returnsOutputProbe(0x42, () -> privacyNoritoFrameWithPayload(0x42));
    assert PrivacyNativeBridge.returnsOutputProbe(0x56, () -> privacyNoritoFrameWithPayload(0x56));
    assert PrivacyNativeBridge.returnsOutputProbe(0x42, () -> privacyNoritoFrameWithFlags(0x42, 0x26));
    assert !PrivacyNativeBridge.returnsOutputProbe(0x50, () -> privacyNoritoFrameWithPayload(0x42));
    assert !PrivacyNativeBridge.returnsOutputProbe(0x42, () -> privacyNoritoFrameWithPayload(0x56));
    assert !PrivacyNativeBridge.returnsOutputProbe(0x56, () -> privacyNoritoFrameWithPayload(0x50));
    assert !PrivacyNativeBridge.returnsOutputProbe(() -> new byte[] {1});
    assert !PrivacyNativeBridge.returnsOutputProbe(() -> invalidPrivacyNoritoFrame(0, 'X'));
    assert !PrivacyNativeBridge.returnsOutputProbe(() -> invalidPrivacyNoritoFrame(4, 1));
    assert !PrivacyNativeBridge.returnsOutputProbe(() -> invalidPrivacyNoritoFrame(5, 1));
    assert !PrivacyNativeBridge.returnsOutputProbe(() -> invalidPrivacyNoritoFrame(22, 1));
    assert !PrivacyNativeBridge.returnsOutputProbe(() -> invalidPrivacyNoritoDeclaredPayloadLength(0x50));
    assert !PrivacyNativeBridge.returnsOutputProbe(() -> invalidPrivacyNoritoOversizedPayloadLength(0x50));
    assert !PrivacyNativeBridge.returnsOutputProbe(() -> invalidPrivacyNoritoFrame(39, 0x40));
    assert !PrivacyNativeBridge.returnsOutputProbe(() -> invalidPrivacyNoritoFrame(39, 0x20));
    assert !PrivacyNativeBridge.returnsOutputProbe(() -> invalidPrivacyNoritoWithNonzeroPadding());
    assert !PrivacyNativeBridge.returnsOutputProbe(() -> invalidPrivacyNoritoWithExcessivePadding());
    assert !PrivacyNativeBridge.returnsOutputProbe(() -> invalidPrivacyNoritoFrame(31, 1));
    assert !PrivacyNativeBridge.returnsOutputProbe(() -> invalidPrivacyNoritoPayloadTamper());
    assert !PrivacyNativeBridge.returnsOutputProbe(() -> new byte[0]);
    assert !PrivacyNativeBridge.returnsOutputProbe(
        () -> new byte[PrivacyNativeBridge.PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1]);
    assert !PrivacyNativeBridge.returnsOutputProbe(() -> null);
    assert !PrivacyNativeBridge.returnsOutputProbe(
        () -> {
          throw new UnsatisfiedLinkError("missing symbol");
        });
    assert !PrivacyNativeBridge.returnsOutputProbe(
        () -> {
          throw new IllegalArgumentException("bad probe");
        });
    assert !PrivacyNativeBridge.returnsOutputProbe(
        () -> {
          throw new SecurityException("blocked probe");
        });
    assert !PrivacyNativeBridge.returnsOutputProbe(
        () -> {
          throw new RuntimeException("unexpected probe failure");
        });
    assert !PrivacyNativeBridge.returnsOutputProbe(
        () -> {
          throw new LinkageError("bad linked bridge");
        });

    assert PrivacyNativeBridge.detectNativeAvailability(() -> {}, () -> 6, () -> true);
    assert !PrivacyNativeBridge.detectNativeAvailability(() -> {}, () -> 5, () -> true);
    assert !PrivacyNativeBridge.detectNativeAvailability(() -> {}, () -> 6, () -> false);
    assert !PrivacyNativeBridge.detectNativeAvailability(
        () -> {
          throw new UnsatisfiedLinkError("missing bridge");
        },
        () -> 6,
        () -> true);
    assert !PrivacyNativeBridge.detectNativeAvailability(
        () -> {
          throw new IllegalArgumentException("bad library name");
        },
        () -> 6,
        () -> true);
    assert !PrivacyNativeBridge.detectNativeAvailability(
        () -> {
          throw new SecurityException("blocked library");
        },
        () -> 6,
        () -> true);
    assert !PrivacyNativeBridge.detectNativeAvailability(
        () -> {
          throw new RuntimeException("unexpected library failure");
        },
        () -> 6,
        () -> true);
    assert !PrivacyNativeBridge.detectNativeAvailability(
        () -> {
          throw new LinkageError("bad linked bridge");
        },
        () -> 6,
        () -> true);
    assert !PrivacyNativeBridge.detectNativeAvailability(
        () -> {},
        () -> {
          throw new UnsatisfiedLinkError("missing ABI symbol");
        },
        () -> true);
    assert !PrivacyNativeBridge.detectNativeAvailability(
        () -> {},
        () -> {
          throw new IllegalArgumentException("bad ABI");
        },
        () -> true);
    assert !PrivacyNativeBridge.detectNativeAvailability(
        () -> {},
        () -> {
          throw new SecurityException("blocked ABI");
        },
        () -> true);
    assert !PrivacyNativeBridge.detectNativeAvailability(
        () -> {},
        () -> {
          throw new RuntimeException("unexpected ABI failure");
        },
        () -> true);
    assert !PrivacyNativeBridge.detectNativeAvailability(
        () -> {},
        () -> {
          throw new LinkageError("bad ABI bridge");
        },
        () -> true);
    assert !PrivacyNativeBridge.detectNativeAvailability(
        () -> {},
        () -> 6,
        () -> {
          throw new UnsatisfiedLinkError("missing privacy symbol");
        });
    assert !PrivacyNativeBridge.detectNativeAvailability(
        () -> {},
        () -> 6,
        () -> {
          throw new IllegalArgumentException("bad privacy probe");
        });
    assert !PrivacyNativeBridge.detectNativeAvailability(
        () -> {},
        () -> 6,
        () -> {
          throw new SecurityException("blocked privacy probe");
        });
    assert !PrivacyNativeBridge.detectNativeAvailability(
        () -> {},
        () -> 6,
        () -> {
          throw new RuntimeException("unexpected privacy probe");
        });
    assert !PrivacyNativeBridge.detectNativeAvailability(
        () -> {},
        () -> 6,
        () -> {
          throw new LinkageError("bad privacy bridge");
        });
  }

  private static void rejectsNullAndEmptyNativeOutputs() {
    assertIllegalState(
        () -> PrivacyNativeBridge.requireNativeOutput(null, "privacy build proof"),
        "returned no output");
    assertIllegalState(
        () -> PrivacyNativeBridge.requireNativeOutput(new byte[0], "privacy verify proof"),
        "returned empty output");
    assertIllegalState(
        () -> PrivacyNativeBridge.requireNativeOutput(privacyNoritoFrame(0x50), "privacy capabilities"),
        "empty privacy result payload");
    assertIllegalState(
        () ->
            PrivacyNativeBridge.requireNativeOutput(
                new byte[PrivacyNativeBridge.PRIVACY_NATIVE_ARCHIVE_MAX_BYTES + 1],
                "privacy capabilities"),
        "returned oversized output");
    final byte[] output = privacyNoritoFrameWithPayload(0x50);
    final byte[] archive = PrivacyNativeBridge.requireNativeOutput(output, "privacy capabilities");
    assert archive != output;
    assert Arrays.equals(output, archive);
    archive[0] = 9;
    assert output[0] == 'N';
  }

  private static void rejectsInvalidNoritoNativeOutputs() {
    assertIllegalState(
        () -> PrivacyNativeBridge.requireNativeOutput(new byte[] {1}, "privacy capabilities"),
        "invalid Norito V1 archive");
    assertIllegalState(
        () ->
            PrivacyNativeBridge.requireNativeOutput(
                invalidPrivacyNoritoFrame(0, 'X'),
                "privacy build proof"),
        "invalid Norito V1 archive");
    assertIllegalState(
        () ->
            PrivacyNativeBridge.requireNativeOutput(
                invalidPrivacyNoritoFrame(4, 1),
                "privacy build proof"),
        "invalid Norito V1 archive");
    assertIllegalState(
        () ->
            PrivacyNativeBridge.requireNativeOutput(
                invalidPrivacyNoritoFrame(5, 1),
                "privacy build proof"),
        "invalid Norito V1 archive");
    assertIllegalState(
        () ->
            PrivacyNativeBridge.requireNativeOutput(
                invalidPrivacyNoritoFrame(22, 1),
                "privacy build proof"),
        "invalid Norito V1 archive");
    assertIllegalState(
        () ->
            PrivacyNativeBridge.requireNativeOutput(
                invalidPrivacyNoritoDeclaredPayloadLength(0x42),
                "privacy build proof"),
        "invalid Norito V1 archive");
    assertIllegalState(
        () ->
            PrivacyNativeBridge.requireNativeOutput(
                invalidPrivacyNoritoOversizedPayloadLength(0x42),
                "privacy build proof"),
        "invalid Norito V1 archive");
    assertIllegalState(
        () ->
            PrivacyNativeBridge.requireNativeOutput(
                invalidPrivacyNoritoFrame(39, 0x40),
                "privacy verify proof"),
        "invalid Norito V1 archive");
    assertIllegalState(
        () ->
            PrivacyNativeBridge.requireNativeOutput(
                invalidPrivacyNoritoFrame(39, 0x20),
                "privacy verify proof"),
        "invalid Norito V1 archive");
    assertIllegalState(
        () ->
            PrivacyNativeBridge.requireNativeOutput(
                invalidPrivacyNoritoWithNonzeroPadding(),
                "privacy verify proof"),
        "invalid Norito V1 archive");
    assertIllegalState(
        () ->
            PrivacyNativeBridge.requireNativeOutput(
                invalidPrivacyNoritoWithExcessivePadding(),
                "privacy verify proof"),
        "invalid Norito V1 archive");
    assertIllegalState(
        () ->
            PrivacyNativeBridge.requireNativeOutput(
                invalidPrivacyNoritoFrame(31, 1),
                "privacy capabilities"),
        "invalid Norito V1 archive");
    assertIllegalState(
        () ->
            PrivacyNativeBridge.requireNativeOutput(
                invalidPrivacyNoritoPayloadTamper(),
                "privacy capabilities"),
        "invalid Norito V1 archive");
  }

  private static void rejectsWrongOperationSchemaNativeOutputs() {
    assertAcceptsOnlySchema("privacy capabilities", 0x50, new int[] {0x42, 0x56, 0x52});
    assertAcceptsOnlySchema("privacy build proof", 0x42, new int[] {0x50, 0x56, 0x52});
    assertAcceptsOnlySchema("privacy verify proof", 0x56, new int[] {0x50, 0x42, 0x52});
  }

  private static void assertAcceptsOnlySchema(
      final String label, final int expectedSchema, final int[] wrongSchemas) {
    assert Arrays.equals(
        PrivacyNativeBridge.requireNativeOutput(
            privacyNoritoFrameWithPayload(expectedSchema),
            label),
        privacyNoritoFrameWithPayload(expectedSchema));
    for (final byte[] mixedSchema :
        new byte[][] {
          privacyNoritoFrameWithSchemaOverride(expectedSchema, 6, wrongSchemas[0]),
          privacyNoritoFrameWithSchemaOverride(expectedSchema, 21, wrongSchemas[0])
        }) {
      assertIllegalState(
          () -> PrivacyNativeBridge.requireNativeOutput(mixedSchema, label),
          "unexpected privacy result schema");
    }

    for (final int wrongSchema : wrongSchemas) {
      assertIllegalState(
          () ->
              PrivacyNativeBridge.requireNativeOutput(
                  privacyNoritoFrameWithPayload(wrongSchema),
                  label),
          "unexpected privacy result schema");
    }
  }

  private static void nativeDispatchReturnsDefensiveOutputCopy() {
    final byte[] nativeOutput = privacyNoritoFrameWithPayload(0x42);
    final byte[] expectedOutput = privacyNoritoFrameWithPayload(0x42);

    final byte[] archive =
        PrivacyNativeBridge.call(
            "build proof",
            privacyNoritoFrameWithPadding(0x52, 64),
            request -> nativeOutput,
            true);

    assert archive != nativeOutput;
    assert Arrays.equals(archive, expectedOutput);

    nativeOutput[6] = 0x7f;
    assert Arrays.equals(archive, expectedOutput);

    archive[0] = 0x7f;
    assert nativeOutput[0] == 'N';
  }

  private static void acceptsCompleteFieldBitsetNoritoFlags() {
    final byte[] requestArchive = privacyNoritoFrameWithFlags(0x52, 0x26);
    final byte[] nativeOutput = privacyNoritoFrameWithFlags(0x42, 0x26);

    final byte[] archive =
        PrivacyNativeBridge.call(
            "build proof",
            requestArchive,
            request -> {
              assert Arrays.equals(request, requestArchive);
              return nativeOutput;
            },
            true);

    assert Arrays.equals(archive, nativeOutput);
  }

  private static void nativeExceptionsAreSanitizedBeforeExposingRequestBytes() {
    final String witness = "android-sdk-private-witness-never-echo-921b";
    final byte[] requestArchive = privacyNoritoFrameWithPayload(0x52);
    final byte[][] capturedRequests = new byte[2][];

    final IllegalStateException capabilitiesError =
        assertIllegalState(
            () ->
                PrivacyNativeBridge.invokeNativeOutput(
                    "privacy capabilities",
                    () -> {
                      throw new RuntimeException("native panic included " + witness);
                    }),
            "privacy capabilities failed");
    assertSanitized(capabilitiesError, witness);

    final IllegalStateException buildError =
        assertIllegalState(
            () ->
                PrivacyNativeBridge.call(
                    "build proof",
                    requestArchive,
                    request -> {
                      capturedRequests[0] = request;
                      assert request != requestArchive;
                      assert Arrays.equals(request, requestArchive);
                      throw new RuntimeException("native panic included " + witness);
                    },
                    true),
            "privacy build proof failed");
    assertSanitized(buildError, witness);

    final IllegalStateException verifyError =
        assertIllegalState(
            () ->
                PrivacyNativeBridge.call(
                    "verify proof",
                    requestArchive,
                    request -> {
                      capturedRequests[1] = request;
                      assert request != requestArchive;
                      assert Arrays.equals(request, requestArchive);
                      throw new UnsatisfiedLinkError("native panic included " + witness);
                    },
                    true),
            "privacy verify proof failed");
    assertSanitized(verifyError, witness);
    assertAllZero(capturedRequests[0]);
    assertAllZero(capturedRequests[1]);
    assert Arrays.equals(requestArchive, privacyNoritoFrameWithPayload(0x52));
  }

  private static void nativeDispatchClearsTemporaryRequestCopyWithoutMutatingCallerArchive() {
    final byte[] requestArchive = privacyNoritoFrameWithPayload(0x52);
    final byte[] originalArchive = Arrays.copyOf(requestArchive, requestArchive.length);
    final byte[][] capturedRequests = new byte[2][];

    final byte[] buildOutput =
        PrivacyNativeBridge.call(
            "build proof",
            requestArchive,
            request -> {
              capturedRequests[0] = request;
              assert request != requestArchive;
              assert Arrays.equals(originalArchive, request);
              return privacyNoritoFrameWithPayload(0x42);
            },
            true);
    assert Arrays.equals(buildOutput, privacyNoritoFrameWithPayload(0x42));

    final byte[] verifyOutput =
        PrivacyNativeBridge.call(
            "verify proof",
            requestArchive,
            request -> {
              capturedRequests[1] = request;
              assert request != requestArchive;
              assert Arrays.equals(originalArchive, request);
              return privacyNoritoFrameWithPayload(0x56);
            },
            true);
    assert Arrays.equals(verifyOutput, privacyNoritoFrameWithPayload(0x56));

    assert Arrays.equals(requestArchive, originalArchive);
    assertAllZero(capturedRequests[0]);
    assertAllZero(capturedRequests[1]);
  }

  private static void hostileNativeRequestMutationCannotMutateCallerArchive() {
    final byte[] requestArchive = privacyNoritoFrameWithPayload(0x52);
    final byte[] originalArchive = Arrays.copyOf(requestArchive, requestArchive.length);
    final byte[][] capturedRequests = new byte[2][];

    final byte[] buildOutput =
        PrivacyNativeBridge.call(
            "build proof",
            requestArchive,
            request -> {
              capturedRequests[0] = request;
              request[0] = 0x00;
              request[6] = 0x7f;
              return privacyNoritoFrameWithPayload(0x42);
            },
            true);
    assert Arrays.equals(buildOutput, privacyNoritoFrameWithPayload(0x42));

    final byte[] verifyOutput =
        PrivacyNativeBridge.call(
            "verify proof",
            requestArchive,
            request -> {
              capturedRequests[1] = request;
              request[0] = 0x00;
              request[6] = 0x7f;
              return privacyNoritoFrameWithPayload(0x56);
            },
            true);
    assert Arrays.equals(verifyOutput, privacyNoritoFrameWithPayload(0x56));

    assert Arrays.equals(requestArchive, originalArchive);
    assertAllZero(capturedRequests[0]);
    assertAllZero(capturedRequests[1]);
  }

  private static void rejectsInvalidNoritoRequestsBeforeNativeDispatch() {
    for (final byte[] malformedArchive : invalidPrivacyRequestArchives()) {
      assertIllegalArgument(
          () ->
              PrivacyNativeBridge.call(
                  "build proof",
                  Arrays.copyOf(malformedArchive, malformedArchive.length),
                  request -> {
                    throw new AssertionError("invalid build request must not reach native dispatch");
                  },
                  true),
          "requestArchive must be a valid Norito V1 archive");
      assertIllegalArgument(
          () ->
              PrivacyNativeBridge.call(
                  "verify proof",
                  Arrays.copyOf(malformedArchive, malformedArchive.length),
                  request -> {
                    throw new AssertionError("invalid verify request must not reach native dispatch");
                  },
                  true),
          "requestArchive must be a valid Norito V1 archive");
    }
  }

  private static void rejectsWrongSchemaRequestsBeforeNativeDispatch() {
    for (final byte[] forgedRequest : wrongSchemaPrivacyRequestArchives()) {
      assertIllegalArgument(
          () ->
              PrivacyNativeBridge.call(
                  "build proof",
                  Arrays.copyOf(forgedRequest, forgedRequest.length),
                  request -> {
                    throw new AssertionError("wrong-schema build request must not reach native dispatch");
                  },
                  true),
          "requestArchive must use the privacy request schema");
      assertIllegalArgument(
          () ->
              PrivacyNativeBridge.call(
                  "verify proof",
                  Arrays.copyOf(forgedRequest, forgedRequest.length),
                  request -> {
                    throw new AssertionError("wrong-schema verify request must not reach native dispatch");
                  },
                  true),
          "requestArchive must use the privacy request schema");
    }
  }

  private static void assertThrows(final Runnable runnable) {
    try {
      runnable.run();
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      // Expected.
    }
  }

  private static IllegalStateException assertIllegalState(
      final Runnable runnable, final String message) {
    try {
      runnable.run();
      throw new AssertionError("expected IllegalStateException");
    } catch (final IllegalStateException expected) {
      assert expected.getMessage().contains(message);
      return expected;
    }
  }

  private static IllegalArgumentException assertIllegalArgument(
      final Runnable runnable, final String message) {
    try {
      runnable.run();
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      assert expected.getMessage().contains(message);
      return expected;
    }
  }

  private static void assertSanitized(final IllegalStateException error, final String witness) {
    assert error.getCause() == null;
    assert !error.getMessage().contains(witness);
    assert !error.toString().contains(witness);
  }

  private static void assertAllZero(final byte[] bytes) {
    assert bytes != null;
    for (final byte value : bytes) {
      assert value == 0;
    }
  }

  private static void assertUnsupportedOperation(final Runnable runnable) {
    try {
      runnable.run();
      throw new AssertionError("expected UnsupportedOperationException");
    } catch (final UnsupportedOperationException expected) {
      // Expected.
    }
  }

  private static byte[] privacyNoritoFrame(final int schemaByte) {
    final byte[] frame = new byte[40];
    frame[0] = 'N';
    frame[1] = 'R';
    frame[2] = 'T';
    frame[3] = '0';
    Arrays.fill(frame, 6, 22, (byte) schemaByte);
    return frame;
  }

  private static byte[] privacyNoritoFrameWithPayload(final int schemaByte) {
    final byte[] frame = Arrays.copyOf(privacyNoritoFrame(schemaByte), 45);
    frame[23] = 3;
    final byte[] checksum =
        new byte[] {
          (byte) 0xb9,
          (byte) 0xd3,
          (byte) 0xa8,
          0x0c,
          (byte) 0xcd,
          0x5d,
          0x13,
          0x24
        };
    System.arraycopy(checksum, 0, frame, 31, checksum.length);
    frame[42] = (byte) 0xa5;
    frame[43] = 0x5a;
    frame[44] = 0x11;
    return frame;
  }

  private static byte[] privacyNoritoFrameWithPadding(
      final int schemaByte, final int paddingLength) {
    final byte[] frame = Arrays.copyOf(privacyNoritoFrame(schemaByte), 43 + paddingLength);
    frame[23] = 3;
    final byte[] checksum =
        new byte[] {
          (byte) 0xb9,
          (byte) 0xd3,
          (byte) 0xa8,
          0x0c,
          (byte) 0xcd,
          0x5d,
          0x13,
          0x24
        };
    System.arraycopy(checksum, 0, frame, 31, checksum.length);
    frame[40 + paddingLength] = (byte) 0xa5;
    frame[41 + paddingLength] = 0x5a;
    frame[42 + paddingLength] = 0x11;
    return frame;
  }

  private static byte[] privacyNoritoFrameWithSchemaOverride(
      final int schemaByte, final int offset, final int value) {
    final byte[] frame = privacyNoritoFrameWithPayload(schemaByte);
    frame[offset] = (byte) value;
    return frame;
  }

  private static byte[] privacyNoritoFrameWithDeclaredPayloadLength(
      final int schemaByte, final long payloadLength) {
    final byte[] frame = privacyNoritoFrameWithPayload(schemaByte);
    for (int index = 0; index < 8; index++) {
      frame[23 + index] = (byte) ((payloadLength >>> (8 * index)) & 0xffL);
    }
    return frame;
  }

  private static byte[] privacyNoritoFrameWithFlags(final int schemaByte, final int flags) {
    final byte[] frame = privacyNoritoFrameWithPayload(schemaByte);
    frame[39] = (byte) flags;
    return frame;
  }

  private static byte[] invalidPrivacyNoritoFrame(final int offset, final int value) {
    final byte[] frame = privacyNoritoFrame(0x50);
    frame[offset] = (byte) value;
    return frame;
  }

  private static byte[] invalidPrivacyNoritoDeclaredPayloadLength(final int schemaByte) {
    return privacyNoritoFrameWithDeclaredPayloadLength(schemaByte, 6L);
  }

  private static byte[] invalidPrivacyNoritoOversizedPayloadLength(final int schemaByte) {
    return privacyNoritoFrameWithDeclaredPayloadLength(schemaByte, Long.MIN_VALUE);
  }

  private static byte[] invalidPrivacyNoritoWithNonzeroPadding() {
    final byte[] frame = Arrays.copyOf(privacyNoritoFrame(0x50), 41);
    frame[40] = 1;
    return frame;
  }

  private static byte[] invalidPrivacyNoritoWithExcessivePadding() {
    return privacyNoritoFrameWithPadding(0x50, 65);
  }

  private static byte[] invalidPrivacyNoritoPayloadTamper() {
    final byte[] frame = privacyNoritoFrameWithPayload(0x50);
    frame[44] ^= 0x7f;
    return frame;
  }

  private static byte[][] invalidPrivacyRequestArchives() {
    return new byte[][] {
      new byte[] {1},
      invalidPrivacyNoritoFrame(0, 'X'),
      invalidPrivacyNoritoFrame(4, 1),
      invalidPrivacyNoritoFrame(5, 1),
      invalidPrivacyNoritoFrame(22, 1),
      invalidPrivacyNoritoDeclaredPayloadLength(0x52),
      invalidPrivacyNoritoOversizedPayloadLength(0x52),
      invalidPrivacyNoritoFrame(39, 0x40),
      invalidPrivacyNoritoFrame(39, 0x20),
      invalidPrivacyNoritoWithNonzeroPadding(),
      invalidPrivacyNoritoWithExcessivePadding(),
      invalidPrivacyNoritoFrame(31, 1),
      invalidPrivacyNoritoPayloadTamper()
    };
  }

  private static byte[][] wrongSchemaPrivacyRequestArchives() {
    return new byte[][] {
      privacyNoritoFrameWithPayload(0x50),
      privacyNoritoFrameWithPayload(0x42),
      privacyNoritoFrameWithPayload(0x56),
      privacyNoritoFrameWithSchemaOverride(0x52, 6, 0x42),
      privacyNoritoFrameWithSchemaOverride(0x52, 21, 0x56)
    };
  }
}
