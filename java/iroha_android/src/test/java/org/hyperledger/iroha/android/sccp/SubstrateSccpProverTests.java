package org.hyperledger.iroha.android.sccp;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;

public final class SubstrateSccpProverTests {
  private SubstrateSccpProverTests() {}

  public static void main(final String[] args) {
    proofRequestBindsRelayContext();
    runtimeCallSubmissionPackagesWrappedProofResult();
    proverRequiresLinkedProofEngine();
    proverWrapsExternalProofBytes();
    proverResolvesWitnessProviderBeforeBuildingRequest();
    System.out.println("[IrohaAndroid] Substrate SCCP prover tests passed.");
  }

  private static void proofRequestBindsRelayContext() {
    final SubstrateSccpProver.ProofRequest request =
        SubstrateSccpProver.buildProofRequest(
            sampleProofRequestInput(samplePublicInputs(), new byte[] {9, 10}));
    assert SubstrateSccpProver.RUNTIME_PROOF_BACKEND_V1.equals(request.backend())
        : "backend must be Substrate runtime";
    assert request.sourceDomain() == SubstrateSccpProver.DOMAIN_SORA
        : "source domain must be SORA";
    assert request.targetDomain() == SubstrateSccpProver.DOMAIN_SORA2
        : "target domain must default to SORA2";
    assert request.publicInputsBytes().length == 141
        : "request must expose canonical Substrate public-input bytes";
    assert ("0x" + repeat("56", 32)).equals(request.statementHash())
        : "statement hash must be normalized";
    assert ("0x" + repeat("78", 32)).equals(request.destinationBindingHash())
        : "destination binding hash must be normalized";
    assert request.requestHash().matches("0x[0-9a-f]{64}") : "request hash must be hex";
    final SubstrateSccpProver.ProofRequest callbackSnapshot =
        SubstrateSccpProver.callbackRequestSnapshot(request);
    assert callbackSnapshot != request : "Substrate proof engine must receive a request snapshot";
    assert callbackSnapshot.version() == request.version() : "snapshot version must match";
    assert callbackSnapshot.backend().equals(request.backend()) : "snapshot backend must match";
    assert callbackSnapshot.sourceDomain() == request.sourceDomain() : "snapshot source domain must match";
    assert callbackSnapshot.targetDomain() == request.targetDomain() : "snapshot target domain must match";
    assert Arrays.equals(callbackSnapshot.publicInputsBytes(), request.publicInputsBytes())
        : "snapshot public inputs must match";
    assert Arrays.equals(callbackSnapshot.bundleBytes(), request.bundleBytes())
        : "snapshot bundle bytes must match";
    assert Arrays.equals(callbackSnapshot.sourceProofBytes(), request.sourceProofBytes())
        : "snapshot source proof bytes must match";
    final byte[] snapshotBundle = callbackSnapshot.bundleBytes();
    final byte[] snapshotSourceProof = callbackSnapshot.sourceProofBytes();
    snapshotBundle[0] = 77;
    snapshotSourceProof[0] = 77;
    assert Arrays.equals(new byte[] {5, 6, 7}, callbackSnapshot.bundleBytes())
        : "snapshot bundle bytes must be defensive copies";
    assert Arrays.equals(new byte[] {9, 10}, callbackSnapshot.sourceProofBytes())
        : "snapshot source proof bytes must be defensive copies";

    final SubstrateSccpProver.ProofRequest kusamaRequest =
        SubstrateSccpProver.buildProofRequest(
            sampleProofRequestInput(
                samplePublicInputs(SubstrateSccpProver.DOMAIN_SORA_KUSAMA, "42"),
                new byte[] {9, 10}));
    assert kusamaRequest.targetDomain() == SubstrateSccpProver.DOMAIN_SORA_KUSAMA
        : "target domain must support SORA Kusama";
    assert !request.requestHash().equals(kusamaRequest.requestHash())
        : "request hash must distinguish Substrate target domains";
    boolean wrongSourceThrew = false;
    try {
      SubstrateSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(), new byte[] {9, 10}, TronSccpProver.DOMAIN_TRON));
    } catch (final IllegalArgumentException ex) {
      wrongSourceThrew = ex.getMessage().contains("sourceDomain must be SORA");
    }
    assert wrongSourceThrew : "non-SORA source domains must be rejected";
    assert !request
        .requestHash()
        .equals(
            SubstrateSccpProver.buildProofRequest(
                    sampleProofRequestInput(
                        samplePublicInputs(),
                        new byte[] {5, 6, 7, 9},
                        new byte[] {10},
                        SubstrateSccpProver.DOMAIN_SORA))
                .requestHash())
        : "request hash must bind the bundle/source-proof split";

    boolean threw = false;
    try {
      SubstrateSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(TonSccpProver.DOMAIN_TON, "42"),
              new byte[0],
              SubstrateSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("Substrate-family");
    }
    assert threw : "non-Substrate target domains must be rejected";

    threw = false;
    try {
      SubstrateSccpProver.buildProofRequest(
          sampleProofRequestInput(
              new SubstrateSccpProver.PublicInputsInput(
                  1,
                  repeat("21", 32),
                  " " + repeat("22", 32),
                  SubstrateSccpProver.DOMAIN_SORA2,
                  repeat("23", 32),
                  "42",
                  repeat("24", 32)),
              new byte[0],
              SubstrateSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("payloadHash") && ex.getMessage().contains("canonical hex");
    }
    assert threw : "padded Substrate payload hash must be rejected";

    threw = false;
    try {
      SubstrateSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(),
              new byte[0],
              repeat("56", 32) + " ",
              repeat("78", 32),
              SubstrateSccpProver.RUNTIME_PROOF_BACKEND_V1,
              SubstrateSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw =
          ex.getMessage().contains("statementHash") && ex.getMessage().contains("canonical hex");
    }
    assert threw : "padded Substrate statement hash must be rejected";

    for (final String finalityHeight : new String[] {"042", "0x2a", "+42", " 42", "42 "}) {
      threw = false;
      try {
        SubstrateSccpProver.buildProofRequest(
            sampleProofRequestInput(
                samplePublicInputs(SubstrateSccpProver.DOMAIN_SORA2, finalityHeight),
                new byte[0],
                SubstrateSccpProver.DOMAIN_SORA));
      } catch (final IllegalArgumentException ex) {
        threw = ex.getMessage().contains("finalityHeight");
      }
      assert threw : "noncanonical Substrate finality height must be rejected";
    }

    threw = false;
    try {
      SubstrateSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(), new byte[0], SubstrateSccpProver.DOMAIN_SORA2));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceDomain must be SORA");
    }
    assert threw : "non-SORA source domains must be rejected";

    threw = false;
    try {
      SubstrateSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(),
              new byte[0],
              repeat("56", 32),
              repeat("00", 32),
              SubstrateSccpProver.RUNTIME_PROOF_BACKEND_V1,
              SubstrateSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("destinationBindingHash");
    }
    assert threw : "zero destination binding hash must be rejected";

    threw = false;
    try {
      SubstrateSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(),
              new byte[0],
              new byte[0],
              SubstrateSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("bundleBytes");
    }
    assert threw : "empty Substrate bundle bytes must be rejected";

    threw = false;
    try {
      SubstrateSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(),
              new byte[] {0, 0},
              new byte[0],
              SubstrateSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("all zero");
    }
    assert threw : "all-zero Substrate bundle bytes must be rejected";

    final byte[] oversizedBundle =
        new byte[SubstrateSccpProver.NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1];
    Arrays.fill(oversizedBundle, (byte) 1);
    threw = false;
    try {
      SubstrateSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(), oversizedBundle, new byte[0], SubstrateSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("at most");
    }
    assert threw : "oversized Substrate bundle bytes must be rejected";

    threw = false;
    try {
      SubstrateSccpProver.buildProofRequest(
          sampleProofRequestInput(samplePublicInputs(), new byte[] {0, 0}));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceProofBytes must not be all zero");
    }
    assert threw : "all-zero Substrate source proof bytes must be rejected";

    final byte[] oversizedSourceProof =
        new byte[SubstrateSccpProver.SOURCE_STATE_MAX_PROOF_BYTES + 1];
    Arrays.fill(oversizedSourceProof, (byte) 1);
    threw = false;
    try {
      SubstrateSccpProver.buildProofRequest(
          sampleProofRequestInput(samplePublicInputs(), oversizedSourceProof));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceProofBytes must be at most");
    }
    assert threw : "oversized Substrate source proof bytes must be rejected";
    assert SubstrateSccpProver.buildProofRequest(
            sampleProofRequestInput(samplePublicInputs(), new byte[0]))
        .sourceProofBytes()
        .length == 0 : "empty optional Substrate source proof bytes must remain valid";

    threw = false;
    try {
      SubstrateSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(),
              new byte[0],
              repeat("56", 32),
              repeat("78", 32),
              "debug-substrate-backend",
              SubstrateSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("substrate-runtime-v1");
    }
    assert threw : "wrong Substrate proof backend must be rejected";
  }

  private static void runtimeCallSubmissionPackagesWrappedProofResult() {
    final SubstrateSccpProver.ProofRequest request =
        SubstrateSccpProver.buildProofRequest(
            sampleProofRequestInput(samplePublicInputs(), new byte[] {9, 10}));
    final SubstrateSccpProver.ProofResult proofResult =
        SubstrateSccpProver.wrapProofResult(new byte[] {1, 2, 3, 4}, request);
    final SubstrateSccpProver.Submission submission =
        SubstrateSccpProver.buildSubmission(new SubstrateSccpProver.SubmissionInput(proofResult));

    assert SubstrateSccpProver.STARK_FRI_PROOF_FAMILY_V1.equals(submission.proofFamily())
        : "proof family must be STARK/Fri";
    assert SubstrateSccpProver.RUNTIME_PROOF_BACKEND_V1.equals(submission.verifierBackend())
        : "verifier backend must be Substrate runtime";
    assert "substrate_runtime_call".equals(submission.platformPayload())
        : "platform payload must be Substrate runtime call";
    assert SubstrateSccpProver.RUNTIME_CALL_SCALE_V1.equals(submission.envelopeEncoding())
        : "envelope must be SCALE call";
    assert "runtime_call".equals(submission.submissionKind()) : "submission kind must match";
    assert SubstrateSccpProver.SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1.equals(
            submission.verifierEntrypoint())
        : "runtime entrypoint must match";
    assert submission.sourceDomain() == SubstrateSccpProver.DOMAIN_SORA
        : "source domain must be SORA";
    assert submission.targetDomain() == SubstrateSccpProver.DOMAIN_SORA2
        : "target domain must be SORA2";
    assert request.requestHash().equals(submission.requestHash()) : "request hash must match";
    assert "proof_bytes".equals(submission.arguments().get(0).key())
        : "first argument must be proof bytes";
    assert "public_inputs".equals(submission.arguments().get(1).key())
        : "second argument must be public inputs";
    assert "bundle_bytes".equals(submission.arguments().get(2).key())
        : "third argument must be bundle bytes";
    assert Arrays.equals(submission.runtimeCall(), submission.envelopeBytes())
        : "runtime call and envelope bytes must match";
    assert submission.runtimeCallHex().equals(submission.envelopeHex())
        : "runtime call and envelope hex must match";
    final byte[] expectedPrefix =
        concat(
            new byte[] {0x7c},
            SubstrateSccpProver.SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1.getBytes(
                StandardCharsets.UTF_8),
            new byte[] {0x10});
    assert Arrays.equals(
            expectedPrefix,
            Arrays.copyOfRange(submission.runtimeCall(), 0, expectedPrefix.length))
        : "runtime call must start with SCALE entrypoint and proof vector length";
    assert Arrays.equals(new byte[] {1, 2, 3, 4}, submission.proofBytes())
        : "proof bytes must be preserved";
    assert Arrays.equals(request.publicInputsBytes(), submission.publicInputsBytes())
        : "public inputs bytes must match request";
    assert Arrays.equals(new byte[] {5, 6, 7}, submission.bundleBytes())
        : "bundle bytes must be preserved";

    final SubstrateSccpProver.Submission explicitSubmission =
        SubstrateSccpProver.buildSubmission(
            new SubstrateSccpProver.SubmissionInput(
                samplePublicInputs(),
                new byte[] {1, 2, 3, 4},
                new byte[] {5, 6, 7},
                new byte[0],
                repeat("56", 32),
                repeat("78", 32),
                SubstrateSccpProver.DOMAIN_SORA,
                null));
    assert Arrays.equals(submission.runtimeCall(), explicitSubmission.runtimeCall())
        : "explicit and wrapped submissions must encode the same runtime call";

    boolean threw = false;
    try {
      SubstrateSccpProver.buildSubmission(
          new SubstrateSccpProver.SubmissionInput(
              samplePublicInputs(),
              new byte[] {1, 2, 3, 4},
              new byte[] {5, 6, 7},
              new byte[] {9, 10},
              repeat("56", 32),
              repeat("78", 32),
              SubstrateSccpProver.DOMAIN_SORA,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceProofBytes requires proofResult");
    }
    assert threw : "raw Substrate source proof bytes must require a wrapped proof result";

    threw = false;
    try {
      SubstrateSccpProver.buildSubmission(
          new SubstrateSccpProver.SubmissionInput(
              proofResult.publicInputs(),
              proofResult.proofBytes(),
              new byte[] {5, 6, 8},
              proofResult.sourceProofBytes(),
              proofResult.statementHash(),
              proofResult.destinationBindingHash(),
              SubstrateSccpProver.DOMAIN_SORA,
              proofResult));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("bundleBytes");
    }
    assert threw : "mismatched wrapped bundle bytes must be rejected";

    threw = false;
    try {
      SubstrateSccpProver.buildSubmission(
          new SubstrateSccpProver.SubmissionInput(
              samplePublicInputs(),
              new byte[] {1, 2, 3, 4},
              new byte[] {0, 0},
              new byte[] {9, 10},
              repeat("56", 32),
              repeat("78", 32),
              SubstrateSccpProver.DOMAIN_SORA,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("all zero");
    }
    assert threw : "all-zero explicit Substrate submission bundle bytes must be rejected";

    threw = false;
    try {
      SubstrateSccpProver.buildSubmission(
          new SubstrateSccpProver.SubmissionInput(
              substrateProofResultWithEnvelopeHash(proofResult, "0x" + repeat("aa", 32))));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofResult");
    }
    assert threw : "mismatched wrapped envelope hash must be rejected";
  }

  private static void proverRequiresLinkedProofEngine() {
    boolean threw = false;
    try {
      new SubstrateSccpProver().prove(sampleProofRequestInput(samplePublicInputs(), new byte[0]));
    } catch (final IllegalStateException ex) {
      threw = ex.getMessage().contains("not linked");
    }
    assert threw : "expected missing local prover to throw";
  }

  private static void proverWrapsExternalProofBytes() {
    final SubstrateSccpProver.ProofRequest[] seenRequests =
        new SubstrateSccpProver.ProofRequest[2];
    final int[] seenRequestCount = new int[] {0};
    final SubstrateSccpProver prover =
        new SubstrateSccpProver(
            null,
            request -> {
              seenRequests[seenRequestCount[0]++] = request;
              assert SubstrateSccpProver.RUNTIME_PROOF_BACKEND_V1.equals(request.backend())
                  : "backend must be Substrate runtime";
              assert request.targetDomain() == SubstrateSccpProver.DOMAIN_SORA2
                  : "target domain must be SORA2";
              return new byte[] {1, 2, 3, 4};
            });

    final SubstrateSccpProver.ProofResult result =
        prover.prove(sampleProofRequestInput(samplePublicInputs(), new byte[] {9, 10}));
    final SubstrateSccpProver.ProofResult omittedSourceResult =
        prover.prove(sampleProofRequestInput(samplePublicInputs(), new byte[0]));
    assert Arrays.equals(new byte[] {1, 2, 3, 4}, result.proofBytes())
        : "proof bytes must be preserved";
    assert Arrays.equals(new byte[0], omittedSourceResult.sourceProofBytes())
        : "Substrate production proofs may omit source proof bytes";
    assert "AQIDBA==".equals(result.proofBase64()) : "proof base64 must be exposed";
    assert ("0x" + repeat("56", 32)).equals(result.statementHash())
        : "result must expose statement hash";
    assert ("0x" + repeat("78", 32)).equals(result.destinationBindingHash())
        : "result must expose destination binding hash";
    assert result.requestHash().matches("0x[0-9a-f]{64}") : "request hash must be hex";
    assert result.envelopeHash().matches("0x[0-9a-f]{64}") : "envelope hash must be hex";
    final SubstrateSccpProver.ProofRequest request =
        SubstrateSccpProver.buildProofRequest(
            sampleProofRequestInput(samplePublicInputs(), new byte[] {9, 10}));
    final SubstrateSccpProver.ProofRequest omittedSourceRequest =
        SubstrateSccpProver.buildProofRequest(
            sampleProofRequestInput(samplePublicInputs(), new byte[0]));
    assert seenRequestCount[0] == 2 : "proof engine must receive both Substrate callback requests";
    assert seenRequests[0] != request : "Substrate proof engine must receive a request snapshot";
    assert seenRequests[0].requestHash().equals(request.requestHash())
        : "Substrate callback snapshot must match the canonical request hash";
    assert Arrays.equals(seenRequests[0].publicInputsBytes(), request.publicInputsBytes())
        : "Substrate callback snapshot must copy public inputs";
    assert Arrays.equals(seenRequests[0].bundleBytes(), request.bundleBytes())
        : "Substrate callback snapshot must copy bundle bytes";
    assert Arrays.equals(seenRequests[0].sourceProofBytes(), request.sourceProofBytes())
        : "Substrate callback snapshot must copy source proof bytes";
    assert seenRequests[1] != omittedSourceRequest
        : "Substrate proof engine must receive an omitted-source request snapshot";
    assert seenRequests[1].requestHash().equals(omittedSourceRequest.requestHash())
        : "Substrate omitted-source callback snapshot must match canonical request";
    boolean threw = false;
    try {
      SubstrateSccpProver.wrapProofResult(
          new byte[] {1}, substrateRequestWithBackend(request, "debug-substrate-backend"));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("substrate-runtime-v1");
    }
    assert threw : "Substrate proof result wrapper must reject wrong backends";

    boolean zeroProofThrew = false;
    try {
      SubstrateSccpProver.wrapProofResult(new byte[] {0, 0}, request);
    } catch (final IllegalArgumentException ex) {
      zeroProofThrew = ex.getMessage().contains("all zero");
    }
    assert zeroProofThrew : "Substrate proof result wrapper must reject all-zero proof bytes";

    final byte[] oversizedProof =
        new byte[SubstrateSccpProver.NATIVE_RECURSIVE_MAX_PROOF_BYTES + 1];
    Arrays.fill(oversizedProof, (byte) 1);
    boolean oversizedProofThrew = false;
    try {
      SubstrateSccpProver.wrapProofResult(oversizedProof, request);
    } catch (final IllegalArgumentException ex) {
      oversizedProofThrew = ex.getMessage().contains("at most");
    }
    assert oversizedProofThrew
        : "Substrate proof result wrapper must reject oversized proof bytes";

    boolean canonicalThrew = false;
    try {
      SubstrateSccpProver.wrapProofResult(
          new byte[] {1}, substrateRequestWithRequestHash(request, "0x" + repeat("99", 32)));
    } catch (final IllegalArgumentException ex) {
      canonicalThrew = ex.getMessage().contains("canonical");
    }
    assert canonicalThrew : "Substrate proof result wrapper must reject non-canonical requests";

    final byte[] exposedProof = result.proofBytes();
    exposedProof[0] = 9;
    assert Arrays.equals(new byte[] {1, 2, 3, 4}, result.proofBytes())
        : "Substrate proof result bytes must be defensive copies";
  }

  private static void proverResolvesWitnessProviderBeforeBuildingRequest() {
    final boolean[] resolved = new boolean[] {false};
    final byte[] bundleBytes = new byte[] {5, 6, 7};
    final SubstrateSccpProver.ProofRequestInput userInput =
        sampleProofRequestInput(
            samplePublicInputs(), bundleBytes, new byte[0], SubstrateSccpProver.DOMAIN_SORA);
    final SubstrateSccpProver prover =
        new SubstrateSccpProver(
            input -> {
              assert Arrays.equals(new byte[0], input.sourceProofBytes())
                  : "UI witness provider should receive unresolved request input";
              assert input.bundleBytes() != bundleBytes
                  : "UI witness provider must receive a byte snapshot";
              input.bundleBytes()[0] = 0x7f;
              resolved[0] = true;
              return new SubstrateSccpProver.ProofRequestInput(
                  input.publicInputs(),
                  input.bundleBytes(),
                  new byte[] {9, 10},
                  input.statementHash(),
                  input.destinationBindingHash(),
                  input.backend(),
                  input.sourceDomain());
            },
            request -> {
              assert resolved[0] : "witness provider must run before proof engine";
              assert Arrays.equals(new byte[] {9, 10}, request.sourceProofBytes())
                  : "proof engine must receive provider-resolved source proof bytes";
              return new byte[] {1, 2, 3, 4};
            });

    final SubstrateSccpProver.ProofResult result = prover.prove(userInput);

    assert Arrays.equals(new byte[] {9, 10}, result.sourceProofBytes())
        : "wrapped result must preserve provider-resolved source proof bytes";
    assert Arrays.equals(new byte[] {5, 6, 7}, userInput.bundleBytes())
        : "UI-owned Substrate bundle bytes must not be mutated by witness provider";
    assert Arrays.equals(new byte[] {5, 6, 7}, bundleBytes)
        : "UI-owned Substrate bundle array must not be mutated by witness provider";
  }

  private static SubstrateSccpProver.ProofRequest substrateRequestWithBackend(
      final SubstrateSccpProver.ProofRequest request, final String backend) {
    return new SubstrateSccpProver.ProofRequest(
        request.version(),
        backend,
        request.sourceDomain(),
        request.targetDomain(),
        request.publicInputs(),
        request.publicInputsBytes(),
        request.bundleBytes(),
        request.sourceProofBytes(),
        request.proofContext(),
        request.statementHash(),
        request.destinationBindingHash(),
        request.requestHash());
  }

  private static SubstrateSccpProver.ProofRequest substrateRequestWithRequestHash(
      final SubstrateSccpProver.ProofRequest request, final String requestHash) {
    return new SubstrateSccpProver.ProofRequest(
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
        requestHash);
  }

  private static SubstrateSccpProver.ProofResult substrateProofResultWithEnvelopeHash(
      final SubstrateSccpProver.ProofResult result, final String envelopeHash) {
    return new SubstrateSccpProver.ProofResult(
        result.version(),
        result.backend(),
        result.proofBytes(),
        result.proofBase64(),
        result.publicInputs(),
        result.bundleBytes(),
        result.sourceProofBytes(),
        result.proofContext(),
        result.statementHash(),
        result.destinationBindingHash(),
        result.requestHash(),
        envelopeHash);
  }

  private static byte[] concat(final byte[]... parts) {
    int length = 0;
    for (final byte[] part : parts) {
      length += part.length;
    }
    final byte[] out = new byte[length];
    int offset = 0;
    for (final byte[] part : parts) {
      System.arraycopy(part, 0, out, offset, part.length);
      offset += part.length;
    }
    return out;
  }

  private static SubstrateSccpProver.ProofRequestInput sampleProofRequestInput(
      final SubstrateSccpProver.PublicInputsInput publicInputs, final byte[] sourceProofBytes) {
    return sampleProofRequestInput(publicInputs, sourceProofBytes, SubstrateSccpProver.DOMAIN_SORA);
  }

  private static SubstrateSccpProver.ProofRequestInput sampleProofRequestInput(
      final SubstrateSccpProver.PublicInputsInput publicInputs,
      final byte[] sourceProofBytes,
      final int sourceDomain) {
    return sampleProofRequestInput(
        publicInputs,
        new byte[] {5, 6, 7},
        sourceProofBytes,
        sourceDomain);
  }

  private static SubstrateSccpProver.ProofRequestInput sampleProofRequestInput(
      final SubstrateSccpProver.PublicInputsInput publicInputs,
      final byte[] bundleBytes,
      final byte[] sourceProofBytes,
      final int sourceDomain) {
    return sampleProofRequestInput(
        publicInputs,
        bundleBytes,
        sourceProofBytes,
        repeat("56", 32),
        repeat("78", 32),
        SubstrateSccpProver.RUNTIME_PROOF_BACKEND_V1,
        sourceDomain);
  }

  private static SubstrateSccpProver.ProofRequestInput sampleProofRequestInput(
      final SubstrateSccpProver.PublicInputsInput publicInputs,
      final byte[] sourceProofBytes,
      final String statementHash,
      final String destinationBindingHash,
      final String backend,
      final int sourceDomain) {
    return sampleProofRequestInput(
        publicInputs,
        new byte[] {5, 6, 7},
        sourceProofBytes,
        statementHash,
        destinationBindingHash,
        backend,
        sourceDomain);
  }

  private static SubstrateSccpProver.ProofRequestInput sampleProofRequestInput(
      final SubstrateSccpProver.PublicInputsInput publicInputs,
      final byte[] bundleBytes,
      final byte[] sourceProofBytes,
      final String statementHash,
      final String destinationBindingHash,
      final String backend,
      final int sourceDomain) {
    return new SubstrateSccpProver.ProofRequestInput(
        publicInputs,
        bundleBytes,
        sourceProofBytes,
        statementHash,
        destinationBindingHash,
        backend,
        sourceDomain);
  }

  private static SubstrateSccpProver.PublicInputsInput samplePublicInputs() {
    return samplePublicInputs(SubstrateSccpProver.DOMAIN_SORA2, "42");
  }

  private static SubstrateSccpProver.PublicInputsInput samplePublicInputs(
      final int targetDomain, final String finalityHeight) {
    return new SubstrateSccpProver.PublicInputsInput(
        1,
        repeat("21", 32),
        repeat("22", 32),
        targetDomain,
        repeat("23", 32),
        finalityHeight,
        repeat("24", 32));
  }

  private static String repeat(final String value, final int count) {
    final StringBuilder out = new StringBuilder(value.length() * count);
    for (int i = 0; i < count; i++) {
      out.append(value);
    }
    return out.toString();
  }
}
