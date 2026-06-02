package org.hyperledger.iroha.android.sccp;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.Map;

public final class EvmSccpProverTests {
  private EvmSccpProverTests() {}

  public static void main(final String[] args) {
    proofRequestBindsPublicSignalsAndRelayContext();
    proverRequiresLinkedProofEngine();
    proverWrapsExternalProofBytes();
    proverResolvesWitnessProviderBeforeBuildingRequest();
    rejectsMalformedGroth16ProofTuple();
    buildsContractCallSubmission();
    bscMainnetFacadeRequiresChainId56AndBscTarget();
    bscMainnetFacadeBuildsLocalAdmissionSubmission();
    ethereumMainnetFacadeRequiresChainId1AndEthTarget();
    ethereumMainnetFacadeBuildsLocalAdmissionSubmission();
    bscMainnetInboundFacadeUsesMainnetRpcAndRejectsDrift();
    mainnetFacadesSnapshotWitnessProviderInputs();
    System.out.println("[IrohaAndroid] EVM-family SCCP prover tests passed.");
  }

  private static void proofRequestBindsPublicSignalsAndRelayContext() {
    final EvmSccpProver.ProofRequest request =
        EvmSccpProver.buildProofRequest(
            sampleProofRequestInput(samplePublicInputs(EvmSccpProver.DOMAIN_ETH), new byte[] {9, 10}, repeat("56", 32)));
    assert EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1.equals(request.backend())
        : "backend must be EVM-family Groth16";
    assert request.sourceDomain() == SolanaSccpProver.DOMAIN_SORA : "source domain must be SORA";
    assert request.targetDomain() == EvmSccpProver.DOMAIN_ETH : "target domain must be Ethereum";
    assert request.publicSignalWords().size() == 9 : "request must expose nine public signals";
    assert "0x2eb6b5dbab56255a979f433862429637ba1e8251106271606f0a279f593d7a39"
        .equals(request.publicSignalWords().get(2)) : "target-domain signal must bind Ethereum";
    assert ("0x" + repeat("56", 32)).equals(request.statementHash())
        : "statement hash must be normalized";
    assert ("0x" + repeat("78", 32)).equals(request.destinationBindingHash())
        : "destination binding hash must be normalized";
    assert "0xfb990c2ffdf826c9beb0e74105b060af467570720a1382b48abc42d32850f5ea"
        .equals(request.requestHash()) : "request hash must bind EVM proof material";
    final EvmSccpProver.ProofRequest callbackSnapshot =
        EvmSccpProver.callbackRequestSnapshot(request);
    assert callbackSnapshot != request : "EVM proof engine must receive a request snapshot";
    assert callbackSnapshot.version() == request.version() : "snapshot version must match";
    assert callbackSnapshot.backend().equals(request.backend()) : "snapshot backend must match";
    assert callbackSnapshot.sourceDomain() == request.sourceDomain() : "snapshot source domain must match";
    assert callbackSnapshot.targetDomain() == request.targetDomain() : "snapshot target domain must match";
    assert callbackSnapshot.publicSignalWords().equals(request.publicSignalWords())
        : "snapshot public signals must match";
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

    final SourceSccpProofs.EvmDestinationBinding destinationBinding =
        sampleDestinationBinding(samplePublicInputs(EvmSccpProver.DOMAIN_ETH));
    final EvmSccpProver.ProofRequest boundRequest =
        EvmSccpProver.buildProofRequest(
            new EvmSccpProver.ProofRequestInput(
                samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
                new byte[] {5, 6, 7},
                new byte[] {9, 10},
                repeat("56", 32),
                destinationBinding));
    assert destinationBinding.hash.equals(boundRequest.destinationBindingHash())
        : "destination binding object constructor must thread the derived hash";
    assert destinationBinding == boundRequest.destinationBinding()
        : "bound request must carry destination binding deployment material";
    assert !request.requestHash().equals(boundRequest.requestHash())
        : "request hash must bind the derived destination binding";

    final EvmSccpProver.ProofRequest bscRequest =
        EvmSccpProver.buildProofRequest(
            sampleProofRequestInput(samplePublicInputs(EvmSccpProver.DOMAIN_BSC), new byte[] {9, 10}, repeat("56", 32)));
    assert bscRequest.targetDomain() == EvmSccpProver.DOMAIN_BSC : "target domain must support BSC";
    assert !request.publicSignalWords().get(2).equals(bscRequest.publicSignalWords().get(2))
        : "target-domain signal must distinguish ETH and BSC";
    assert !request.requestHash().equals(bscRequest.requestHash())
        : "request hash must distinguish ETH and BSC targets";
    final EvmSccpProver.ProofRequest shiftedSplitRequest =
        EvmSccpProver.buildProofRequest(
            new EvmSccpProver.ProofRequestInput(
                samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
                new byte[] {5, 6, 7, 9},
                new byte[] {10},
                repeat("56", 32),
                repeat("78", 32),
                EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
                SolanaSccpProver.DOMAIN_SORA));
    assert !request.requestHash().equals(shiftedSplitRequest.requestHash())
        : "request hash must distinguish shifted EVM bundle/proof splits";

    boolean threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          sampleProofRequestInput(samplePublicInputs(EvmSccpProver.DOMAIN_ETH), new byte[0], ""));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("statementHash");
    }
    assert threw : "missing statement hash must be rejected";

    threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH, "0"),
              new byte[0],
              repeat("56", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("finalityHeight");
    }
    assert threw : "zero finality height must be rejected";

    threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          sampleProofRequestInput(
              new EvmSccpProver.PublicInputsInput(
                  1,
                  repeat("11", 32),
                  " " + repeat("22", 32),
                  EvmSccpProver.DOMAIN_ETH,
                  repeat("33", 32),
                  "19",
                  repeat("44", 32)),
              new byte[0],
              repeat("56", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("payloadHash") && ex.getMessage().contains("canonical hex");
    }
    assert threw : "padded EVM payload hash must be rejected";

    threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
              new byte[0],
              repeat("56", 32) + " "));
    } catch (final IllegalArgumentException ex) {
      threw =
          ex.getMessage().contains("statementHash") && ex.getMessage().contains("canonical hex");
    }
    assert threw : "padded EVM statement hash must be rejected";

    for (final String finalityHeight : new String[] {"019", "0x13", "+19", " 19", "19 "}) {
      threw = false;
      try {
        EvmSccpProver.buildProofRequest(
            sampleProofRequestInput(
                samplePublicInputs(EvmSccpProver.DOMAIN_ETH, finalityHeight),
                new byte[0],
                repeat("56", 32)));
      } catch (final IllegalArgumentException ex) {
        threw = ex.getMessage().contains("finalityHeight");
      }
      assert threw : "noncanonical EVM finality height must be rejected";
    }

    threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
              new byte[0],
              repeat("56", 32),
              repeat("78", 32),
              EvmSccpProver.DOMAIN_ETH));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceDomain must be SORA");
    }
    assert threw : "non-SORA source domains must be rejected";

    threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
              new byte[0],
              repeat("56", 32),
              repeat("78", 32),
              TonSccpProver.DOMAIN_TON));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceDomain must be SORA");
    }
    assert threw : "non-SORA source domains must be rejected";

    threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(TonSccpProver.DOMAIN_TON),
              new byte[0],
              repeat("56", 32),
              repeat("78", 32),
              SolanaSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("publicInputs.targetDomain must be ETH or BSC");
    }
    assert threw : "non-EVM-family target domains must be rejected";

    threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
              new byte[0],
              repeat("56", 32),
              repeat("00", 32),
              SolanaSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("destinationBindingHash");
    }
    assert threw : "zero destination binding hash must be rejected";

    threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          new EvmSccpProver.ProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
              new byte[0],
              new byte[0],
              repeat("56", 32),
              repeat("78", 32),
              EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
              SolanaSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("bundleBytes");
    }
    assert threw : "empty EVM bundle bytes must be rejected";

    threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
              new byte[] {0, 0},
              repeat("56", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceProofBytes must not be all zero");
    }
    assert threw : "all-zero EVM source proof bytes must be rejected";

    final byte[] oversizedSourceProof =
        new byte[EvmSccpProver.SOURCE_STATE_MAX_PROOF_BYTES + 1];
    Arrays.fill(oversizedSourceProof, (byte) 1);
    threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
              oversizedSourceProof,
              repeat("56", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceProofBytes must be at most");
    }
    assert threw : "oversized EVM source proof bytes must be rejected";
    assert EvmSccpProver.buildProofRequest(
            sampleProofRequestInput(
                samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
                new byte[0],
                repeat("56", 32)))
        .sourceProofBytes()
        .length == 0 : "empty optional EVM source proof bytes must remain valid";

    threw = false;
    try {
      EvmSccpProver.buildProofRequest(
          sampleProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
              new byte[0],
              repeat("56", 32),
              repeat("78", 32),
              "debug-evm-backend",
              SolanaSccpProver.DOMAIN_SORA));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("evm-groth16-bn254-v1");
    }
    assert threw : "wrong EVM proof backend must be rejected";

    final SourceSccpProofs.EvmDestinationBinding bscDestinationBinding =
        SourceSccpProofs.evmDestinationBinding(
            EvmSccpProver.DOMAIN_SORA,
            EvmSccpProver.DOMAIN_BSC,
            "0x" + repeat("33", 32),
            "0x" + repeat("11", 20),
            "0x" + repeat("22", 20),
            "0x" + repeat("bb", 32),
            "0x" + repeat("cc", 32));
    threw = false;
    try {
      new EvmSccpProver.ProofRequestInput(
          samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
          new byte[] {5, 6, 7},
          repeat("56", 32),
          bscDestinationBinding);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("destinationBinding.targetDomain");
    }
    assert threw : "destination binding target must match EVM public inputs";
  }

  private static void proverRequiresLinkedProofEngine() {
    boolean threw = false;
    try {
      new EvmSccpProver()
          .prove(sampleProofRequestInput(samplePublicInputs(EvmSccpProver.DOMAIN_ETH), new byte[0], repeat("56", 32)));
    } catch (final IllegalStateException ex) {
      threw = ex.getMessage().contains("not linked");
    }
    assert threw : "expected missing local prover to throw";
  }

  private static void proverWrapsExternalProofBytes() {
    final byte[] proofBytes = sampleGroth16ProofBytes();
    final EvmSccpProver.ProofRequest[] seenRequests = new EvmSccpProver.ProofRequest[2];
    final int[] seenRequestCount = new int[] {0};
    final EvmSccpProver prover =
        new EvmSccpProver(
            null,
            request -> {
              seenRequests[seenRequestCount[0]++] = request;
              assert EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1.equals(request.backend())
                  : "backend must be EVM-family Groth16";
              assert request.targetDomain() == EvmSccpProver.DOMAIN_ETH
                  : "target domain must be Ethereum";
              assert request.publicSignalWords().size() == 9 : "request must carry public signals";
              return proofBytes;
            });

    final EvmSccpProver.ProofResult result =
        prover.prove(
            sampleProductionProofRequestInput(
                samplePublicInputs(EvmSccpProver.DOMAIN_ETH), new byte[] {9, 10}, repeat("56", 32)));
    final EvmSccpProver.ProofResult omittedSourceResult =
        prover.prove(
            sampleProductionProofRequestInput(
                samplePublicInputs(EvmSccpProver.DOMAIN_ETH), new byte[0], repeat("56", 32)));
    assert Arrays.equals(proofBytes, result.proofBytes())
        : "proof bytes must be preserved";
    assert Arrays.equals(new byte[0], omittedSourceResult.sourceProofBytes())
        : "EVM production proofs may omit source proof bytes";
    assert !result.proofBase64().isEmpty() : "proof base64 must be exposed";
    assert ("0x" + repeat("56", 32)).equals(result.statementHash())
        : "result must expose statement hash";
    final EvmSccpProver.ProofRequest request =
        EvmSccpProver.buildProofRequest(
            sampleProductionProofRequestInput(
                samplePublicInputs(EvmSccpProver.DOMAIN_ETH), new byte[] {9, 10}, repeat("56", 32)));
    final EvmSccpProver.ProofRequest omittedSourceRequest =
        EvmSccpProver.buildProofRequest(
            sampleProductionProofRequestInput(
                samplePublicInputs(EvmSccpProver.DOMAIN_ETH), new byte[0], repeat("56", 32)));
    assert request.destinationBindingHash().equals(result.destinationBindingHash())
        : "result must expose destination binding hash";
    assert request.destinationBinding().hash.equals(result.destinationBinding().hash)
        : "result must carry destination binding deployment material";
    assert request.requestHash().equals(result.requestHash()) : "result must expose the request hash";
    assert result.envelopeHash().matches("0x[0-9a-f]{64}")
        : "result must bind proof bytes to the EVM request";

    assert seenRequestCount[0] == 2 : "proof engine must receive both EVM callback requests";
    assert seenRequests[0] != request : "EVM proof engine must receive a request snapshot";
    assert seenRequests[0].requestHash().equals(request.requestHash())
        : "EVM callback snapshot must match the canonical request hash";
    assert Arrays.equals(seenRequests[0].bundleBytes(), request.bundleBytes())
        : "EVM callback snapshot must copy bundle bytes";
    assert Arrays.equals(seenRequests[0].sourceProofBytes(), request.sourceProofBytes())
        : "EVM callback snapshot must copy source proof bytes";
    assert seenRequests[1] != omittedSourceRequest
        : "EVM proof engine must receive an omitted-source request snapshot";
    assert seenRequests[1].requestHash().equals(omittedSourceRequest.requestHash())
        : "EVM omitted-source callback snapshot must match canonical request";
    boolean threw = false;
    try {
      EvmSccpProver.wrapProofResult(new byte[] {0, 0}, request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("all zero");
    }
    assert threw : "EVM proof result wrapper must reject all-zero proof bytes";

    threw = false;
    try {
      EvmSccpProver.wrapProofResult(new byte[] {1, 2, 3, 4}, request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("384 bytes");
    }
    assert threw : "EVM proof result wrapper must reject non-canonical proof lengths";

    threw = false;
    try {
      EvmSccpProver.wrapProofResult(
          new byte[] {1}, evmRequestWithBackend(request, "debug-evm-backend"));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("evm-groth16-bn254-v1");
    }
    assert threw : "EVM proof result wrapper must reject wrong backends";

    boolean canonicalThrew = false;
    try {
      EvmSccpProver.wrapProofResult(
          proofBytes, evmRequestWithRequestHash(request, "0x" + repeat("99", 32)));
    } catch (final IllegalArgumentException ex) {
      canonicalThrew = ex.getMessage().contains("canonical");
    }
    assert canonicalThrew : "EVM proof result wrapper must reject non-canonical requests";

    boolean missingBindingThrew = false;
    try {
      EvmSccpProver.wrapProofResult(
          proofBytes,
          EvmSccpProver.buildProofRequest(
              sampleProofRequestInput(
                  samplePublicInputs(EvmSccpProver.DOMAIN_ETH), new byte[] {9, 10}, repeat("56", 32))));
    } catch (final IllegalArgumentException ex) {
      missingBindingThrew = ex.getMessage().contains("destinationBinding");
    }
    assert missingBindingThrew : "EVM proof result wrapper must reject hash-only destination bindings";

    final byte[] exposedProof = result.proofBytes();
    exposedProof[0] = 9;
    assert Arrays.equals(proofBytes, result.proofBytes())
        : "EVM proof result bytes must be defensive copies";

    final ArrayList<String> mutableSignals = new ArrayList<>(result.publicSignalWords());
    final EvmSccpProver.ProofResult manualResult =
        new EvmSccpProver.ProofResult(
            result.version(),
            result.backend(),
            result.proofBytes(),
            result.proofBase64(),
            result.publicInputs(),
            mutableSignals,
            result.bundleBytes(),
            result.sourceProofBytes(),
            result.proofContext(),
            result.statementHash(),
            result.destinationBindingHash(),
            result.requestHash(),
            result.envelopeHash());
    mutableSignals.set(0, "0x" + repeat("99", 32));
    assert manualResult.publicSignalWords().equals(result.publicSignalWords())
        : "EVM proof result public signals must be construction snapshots";
    boolean immutableSignals = false;
    try {
      manualResult.publicSignalWords().set(0, "0x" + repeat("88", 32));
    } catch (final UnsupportedOperationException ex) {
      immutableSignals = true;
    }
    assert immutableSignals : "EVM proof result public signals must be immutable";
  }

  private static void proverResolvesWitnessProviderBeforeBuildingRequest() {
    final boolean[] resolved = new boolean[] {false};
    final byte[] proofBytes = sampleGroth16ProofBytes();
    final byte[] bundleBytes = new byte[] {5, 6, 7};
    final EvmSccpProver.ProofRequestInput userInput =
        sampleProductionProofRequestInput(
            samplePublicInputs(EvmSccpProver.DOMAIN_ETH), bundleBytes, new byte[0], repeat("56", 32));
    final EvmSccpProver prover =
        new EvmSccpProver(
            input -> {
              assert Arrays.equals(new byte[0], input.sourceProofBytes())
                  : "UI witness provider should receive unresolved request input";
              assert input.bundleBytes() != bundleBytes
                  : "UI witness provider must receive a byte snapshot";
              input.bundleBytes()[0] = 0x7f;
              resolved[0] = true;
              return sampleProductionProofRequestInput(
                  input.publicInputs(),
                  new byte[] {9, 10},
                  input.statementHash());
            },
            request -> {
              assert resolved[0] : "witness provider must run before proof engine";
              assert Arrays.equals(new byte[] {9, 10}, request.sourceProofBytes())
                  : "proof engine must receive provider-resolved source proof bytes";
              return proofBytes;
            });

    final EvmSccpProver.ProofResult result = prover.prove(userInput);

    assert Arrays.equals(new byte[] {9, 10}, result.sourceProofBytes())
        : "wrapped result must preserve provider-resolved source proof bytes";
    assert Arrays.equals(new byte[] {5, 6, 7}, userInput.bundleBytes())
        : "UI-owned EVM bundle bytes must not be mutated by witness provider";
    assert Arrays.equals(new byte[] {5, 6, 7}, bundleBytes)
        : "UI-owned EVM bundle array must not be mutated by witness provider";
  }

  private static void rejectsMalformedGroth16ProofTuple() {
    final EvmSccpProver.ProofRequest request =
        EvmSccpProver.buildProofRequest(
            sampleProductionProofRequestInput(
                samplePublicInputs(EvmSccpProver.DOMAIN_ETH), new byte[] {9, 10}, repeat("56", 32)));

    boolean threw = false;
    try {
      EvmSccpProver.wrapProofResult(sampleGroth16ProofBytes(0, abiWord(2)), request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes.version");
    }
    assert threw : "EVM proof wrapper must reject non-v1 Groth16 ABI tuples";

    threw = false;
    try {
      EvmSccpProver.wrapProofResult(sampleGroth16ProofBytes(4, repeatedWord(0xff)), request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("BN254 base-field");
    }
    assert threw : "EVM proof wrapper must reject out-of-range BN254 coordinates";

    threw = false;
    try {
      EvmSccpProver.wrapProofResult(sampleGroth16ProofBytesWithZeroB(), request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes.b");
    }
    assert threw : "EVM proof wrapper must reject zero Groth16 B points";

    threw = false;
    try {
      EvmSccpProver.wrapProofResult(sampleGroth16ProofBytes(11, abiWord(3)), request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes.c");
    }
    assert threw : "EVM proof wrapper must reject off-curve Groth16 C points";

    threw = false;
    try {
      EvmSccpProver.wrapProofResult(sampleGroth16ProofBytesWithNonSubgroupB(), request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes.b");
    }
    assert threw : "EVM proof wrapper must reject non-subgroup Groth16 B points";

    threw = false;
    try {
      EvmSccpProver.wrapProofResult(sampleGroth16ProofBytes(1, repeatedWord(0x22)), request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("messageId must match");
    }
    assert threw : "EVM proof wrapper must reject message-id mismatches";

    threw = false;
    try {
      EvmSccpProver.wrapProofResult(sampleGroth16ProofBytes(2, abiWord(999)), request);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceDomain must match");
    }
    assert threw : "EVM proof wrapper must reject source-domain mismatches";

    threw = false;
    try {
      EvmSccpProver.submitMessageProofCallData(
          sampleGroth16ProofBytes(2, abiWord(EvmSccpProver.DOMAIN_ETH)),
          samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
          repeat("56", 32));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceDomain must match");
    }
    assert threw : "EVM direct calldata helper must reject source-domain mismatches";

    threw = false;
    try {
      EvmSccpProver.submitMessageProofCallData(
          sampleGroth16ProofBytes(2, abiWord(EvmSccpProver.DOMAIN_ETH)),
          samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
          repeat("56", 32),
          EvmSccpProver.DOMAIN_ETH);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceDomain must be SORA");
    }
    assert threw : "EVM direct calldata helper must reject non-SORA source domains";

    threw = false;
    try {
      EvmSccpProver.buildSubmission(
          new EvmSccpProver.SubmissionInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
              sampleGroth16ProofBytes(3, repeatedWord(0x44)),
              repeat("56", 32),
              repeat("78", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("commitmentRoot must match");
    }
    assert threw : "EVM submission builder must reject commitment-root mismatches";
  }

  private static void buildsContractCallSubmission() {
    final byte[] proofBytes = sampleGroth16ProofBytes();
    final EvmSccpProver.ProofRequest request =
        EvmSccpProver.buildProofRequest(
            sampleProductionProofRequestInput(
                samplePublicInputs(EvmSccpProver.DOMAIN_ETH), new byte[] {9, 10}, repeat("56", 32)));
    final EvmSccpProver.ProofResult proofResult =
        EvmSccpProver.wrapProofResult(proofBytes, request);
    final EvmSccpProver.Submission submission =
        EvmSccpProver.buildSubmission(new EvmSccpProver.SubmissionInput(proofResult));

    assert "contract_call".equals(submission.submissionKind())
        : "EVM submission kind must be contract_call";
    assert "evm_groth16_contract_call".equals(submission.platformPayload())
        : "EVM submission platform payload must identify contract calls";
    assert EvmSccpProver.CONTRACT_CALL_ABI_TUPLE_V1.equals(submission.envelopeEncoding())
        : "EVM envelope encoding must be ABI tuple v1";
    assert EvmSccpProver.SUBMIT_MESSAGE_PROOF_SELECTOR_V1.equals(submission.functionSelector())
        : "EVM function selector must be exposed";
    assert submission.callDataHex().startsWith(EvmSccpProver.SUBMIT_MESSAGE_PROOF_SELECTOR_V1)
        : "call data must start with the selector";
    assert submission.callData().length == 676 : "EVM call data length must be ABI canonical";
    assert ("0x" + repeat("00", 30) + "0100")
        .equals("0x" + hexLower(Arrays.copyOfRange(submission.callData(), 4, 36)))
        : "dynamic proof offset must be 0x100";
    assert ("0x" + repeat("00", 30) + "0180")
        .equals("0x" + hexLower(Arrays.copyOfRange(submission.callData(), 260, 292)))
        : "proof length must be 384 bytes";
    assert EvmSccpProver.messageTransparentPublicInputAbiWords(
            samplePublicInputs(EvmSccpProver.DOMAIN_ETH))
        .equals(submission.publicInputWords()) : "public input ABI words must be exposed";
    assert proofResult.publicSignalWords().equals(submission.publicSignalWords())
        : "public signal words must be carried";
    assert Arrays.equals(new byte[] {5, 6, 7}, proofResult.bundleBytes())
        : "proof results must retain request bundle bytes";
    assert Arrays.equals(new byte[] {9, 10}, proofResult.sourceProofBytes())
        : "proof results must retain source proof bytes";
    assert Arrays.equals(proofBytes, submission.proofBytes()) : "proof bytes must be preserved";
    assert Arrays.equals(submission.callData(), submission.envelopeBytes())
        : "EVM envelope bytes must equal call data";
    assert Arrays.equals(
            submission.callData(),
            EvmSccpProver.submitMessageProofCallData(
                proofBytes, proofResult.publicInputs(), proofResult.statementHash()))
        : "direct calldata helper must match submission call data";
    final SourceSccpProofs.EvmDestinationBinding destinationBinding =
        sampleDestinationBinding(proofResult.publicInputs());
    final EvmSccpProver.Submission bindingSubmission =
        EvmSccpProver.buildSubmission(
            new EvmSccpProver.SubmissionInput(
                proofResult.publicInputs(),
                proofBytes,
                proofResult.statementHash(),
                destinationBinding));
    assert destinationBinding.hash.equals(bindingSubmission.destinationBindingHash())
        : "EVM submission input must accept a derived destination binding";

    final EvmSccpProver.ProofResult omittedSourceProofResult =
        EvmSccpProver.wrapProofResult(
            proofBytes,
            EvmSccpProver.buildProofRequest(
                sampleProductionProofRequestInput(
                    samplePublicInputs(EvmSccpProver.DOMAIN_ETH), new byte[0], repeat("56", 32))));
    final EvmSccpProver.Submission omittedSourceSubmission =
        EvmSccpProver.buildSubmission(
            new EvmSccpProver.SubmissionInput(omittedSourceProofResult));
    assert Arrays.equals(new byte[0], omittedSourceProofResult.sourceProofBytes())
        : "EVM submit-ready proof results may omit source proof bytes";
    assert Arrays.equals(proofBytes, omittedSourceSubmission.proofBytes())
        : "EVM omitted-source submission must preserve proof bytes";

    final byte[] exposedCallData = submission.callData();
    exposedCallData[0] = 0;
    assert submission.callData()[0] != 0 : "submission call data must be a defensive copy";

    final byte[] proofMismatch = Arrays.copyOf(proofBytes, proofBytes.length);
    proofMismatch[4 * 32 + 31] = 9;
    boolean threw = false;
    try {
      EvmSccpProver.buildSubmission(
          new EvmSccpProver.SubmissionInput(
              proofResult.publicInputs(),
              proofMismatch,
              proofResult.statementHash(),
              proofResult.destinationBindingHash(),
              SolanaSccpProver.DOMAIN_SORA,
              proofResult,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes");
    }
    assert threw : "submission must reject proof bytes that differ from the wrapped result";

    final SourceSccpProofs.EvmDestinationBinding bscDestinationBinding =
        SourceSccpProofs.evmDestinationBinding(
            EvmSccpProver.DOMAIN_SORA,
            EvmSccpProver.DOMAIN_BSC,
            "0x" + repeat("33", 32),
            "0x" + repeat("11", 20),
            "0x" + repeat("22", 20),
            "0x" + repeat("bb", 32),
            "0x" + repeat("cc", 32));
    threw = false;
    try {
      new EvmSccpProver.SubmissionInput(
          proofResult.publicInputs(),
          proofBytes,
          proofResult.statementHash(),
          bscDestinationBinding);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("destinationBinding.targetDomain");
    }
    assert threw : "submission destination binding target must match EVM public inputs";

    threw = false;
    try {
      EvmSccpProver.buildSubmission(
          new EvmSccpProver.SubmissionInput(
              evmResultWithEnvelopeHash(proofResult, "0x" + repeat("aa", 32))));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("wrapped proof bytes");
    }
    assert threw : "submission must reject tampered wrapped proof-result envelope hashes";

    threw = false;
    try {
      EvmSccpProver.buildSubmission(
          new EvmSccpProver.SubmissionInput(evmResultWithProofBase64(proofResult, "AAAA")));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBase64");
    }
    assert threw : "submission must reject tampered wrapped proof-result proofBase64";

    threw = false;
    try {
      EvmSccpProver.buildSubmission(
          new EvmSccpProver.SubmissionInput(
              evmResultWithBundleBytes(proofResult, new byte[] {5, 6, 8})));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("requestHash");
    }
    assert threw : "submission must reject stale wrapped proof-result request context";

    final ArrayList<String> mismatchedSignals = new ArrayList<>(proofResult.publicSignalWords());
    mismatchedSignals.set(0, "0x" + repeat("99", 32));
    threw = false;
    try {
      EvmSccpProver.buildSubmission(
          new EvmSccpProver.SubmissionInput(
              proofResult.publicInputs(),
              proofBytes,
              proofResult.statementHash(),
              proofResult.destinationBindingHash(),
              SolanaSccpProver.DOMAIN_SORA,
              null,
              mismatchedSignals));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("publicSignalWords");
    }
    assert threw : "submission must reject public-signal mismatches";

    threw = false;
    try {
      EvmSccpProver.buildSubmission(
          new EvmSccpProver.SubmissionInput(
              samplePublicInputs(TonSccpProver.DOMAIN_TON),
              proofBytes,
              proofResult.statementHash(),
              proofResult.destinationBindingHash()));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("ETH or BSC");
    }
    assert threw : "submission must reject non-EVM-family target domains";
  }

  private static void bscMainnetFacadeRequiresChainId56AndBscTarget() {
    final byte[] proofBytes = sampleGroth16ProofBytes();
    final SourceSccpProofs.EvmDestinationBinding binding =
        BscSccpProver.destinationBinding(
            "0x" + repeat("11", 20),
            "0x" + repeat("22", 20),
            "0x" + repeat("bb", 32),
            "0x" + repeat("cc", 32));
    assert SourceSccpProofs.BSC_MAINNET_NETWORK_ID.equals(binding.networkId)
        : "BSC binding must default to chain id 56";
    assert binding.targetDomain == EvmSccpProver.DOMAIN_BSC
        : "BSC binding must target BSC";
    assert binding.hash.equals(
            BscSccpProver.destinationBindingHash(
                "0x" + repeat("11", 20),
                "0x" + repeat("22", 20),
                "0x" + repeat("bb", 32),
                "0x" + repeat("cc", 32)))
        : "BSC binding hash helper must match binding";

    final EvmSccpProver.ProofRequest request =
        BscSccpProver.buildProofRequest(
            new EvmSccpProver.ProofRequestInput(
                samplePublicInputs(EvmSccpProver.DOMAIN_BSC),
                new byte[] {5, 6, 7},
                new byte[] {9, 10},
                repeat("56", 32),
                binding));
    assert request.targetDomain() == EvmSccpProver.DOMAIN_BSC
        : "BSC request must target BSC";
    assert binding.hash.equals(request.destinationBindingHash())
        : "BSC request must bind the BSC destination binding";

    final EvmSccpProver.ProofResult result =
        BscSccpProver.wrapProofResult(proofBytes, request);
    final EvmSccpProver.Submission submission =
        BscSccpProver.buildSubmission(new EvmSccpProver.SubmissionInput(result));
    assert submission.targetDomain() == EvmSccpProver.DOMAIN_BSC
        : "BSC submission must target BSC";
    assert Arrays.equals(proofBytes, submission.proofBytes())
        : "BSC submission must preserve proof bytes";
    final Object submitted =
        new BscMainnetSccp(
                null,
                null,
                null,
                null,
                null,
                null,
                outboundSubmission -> {
                  assert outboundSubmission.targetDomain() == EvmSccpProver.DOMAIN_BSC
                      : "BSC outbound submitter must receive BSC calldata";
                  assert Arrays.equals(proofBytes, outboundSubmission.proofBytes())
                      : "BSC outbound submitter must receive proof bytes";
                  assert binding.hash.equals(outboundSubmission.destinationBindingHash())
                      : "BSC outbound submitter must receive the bound destination hash";
                  return "bsc-submitted";
                })
            .submitOutboundToBsc(new EvmSccpProver.SubmissionInput(result));
    assert "bsc-submitted".equals(submitted)
        : "BSC outbound submitter must return app-owned submission result";
    boolean threw = false;
    try {
      new BscMainnetSccp().submitOutboundToBsc(new EvmSccpProver.SubmissionInput(result));
    } catch (final IllegalStateException ex) {
      threw = ex.getMessage().contains("outbound submitter");
    }
    assert threw : "BSC outbound submission requires an app-owned submitter";

    threw = false;
    try {
      BscSccpProver.destinationBinding(
          "0x" + repeat("11", 20),
          "0x" + repeat("22", 20),
          "0x" + repeat("bb", 32),
          "0x" + repeat("cc", 32),
          "0x" + repeat("33", 32));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("chain id 56");
    }
    assert threw : "BSC destination helper must reject non-mainnet network ids";

    final SourceSccpProofs.EvmDestinationBinding wrongNetworkBinding =
        SourceSccpProofs.evmDestinationBinding(
            EvmSccpProver.DOMAIN_SORA,
            EvmSccpProver.DOMAIN_BSC,
            "0x" + repeat("33", 32),
            "0x" + repeat("11", 20),
            "0x" + repeat("22", 20),
            "0x" + repeat("bb", 32),
            "0x" + repeat("cc", 32));
    threw = false;
    try {
      BscSccpProver.buildProofRequest(
          new EvmSccpProver.ProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_BSC),
              new byte[] {5, 6, 7},
              new byte[0],
              repeat("56", 32),
              wrongNetworkBinding));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("chain id 56");
    }
    assert threw : "BSC request helper must reject non-mainnet destination bindings";

    threw = false;
    try {
      BscSccpProver.buildProofRequest(
          sampleProductionProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH), new byte[0], repeat("56", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("target BSC");
    }
    assert threw : "BSC request helper must reject ETH public inputs";
  }

  private static void bscMainnetFacadeBuildsLocalAdmissionSubmission() {
    final BscMainnetSccp.LocalAdmissionSubmissionInput input =
        new BscMainnetSccp.LocalAdmissionSubmissionInput(
            new byte[] {1, 2, 3},
            new byte[] {4, 5, 6},
            new byte[] {7, 8, 9},
            new byte[] {10, 11, 12},
            "0x" + repeat("66", 32),
            "0x" + repeat("77", 32),
            "0x" + repeat("88", 32));
    final BscMainnetSccp.LocalAdmissionSubmission submission =
        BscMainnetSccp.buildLocalAdmissionSubmission(input);
    final BscMainnetSccp.LocalAdmissionSubmission facadeSubmission =
        new BscMainnetSccp().buildLocalAdmission(input);

    assert BscMainnetSccp.LOCAL_ADMISSION_SUBMISSION_KIND_V1.equals(submission.platformPayload())
        : "BSC local admission platform payload must be local_admission";
    assert BscMainnetSccp.LOCAL_ADMISSION_ENVELOPE_ENCODING_V1.equals(submission.envelopeEncoding())
        : "BSC local admission must use the Norito envelope";
    assert BscMainnetSccp.LOCAL_ADMISSION_ENTRYPOINT_V1.equals(submission.verifierEntrypoint())
        : "BSC local admission must target SubmitBridgeProof";
    assert submission.sourceDomain() == EvmSccpProver.DOMAIN_BSC
        : "BSC local admission source must be BSC";
    assert submission.targetDomain() == EvmSccpProver.DOMAIN_SORA
        : "BSC local admission target must be SORA";
    assert submission.arguments().isEmpty() : "BSC local admission must not add call arguments";
    assert Arrays.equals(new byte[] {1, 2, 3}, submission.proofBytes())
        : "BSC local admission must copy proof bytes";
    assert Arrays.equals(new byte[] {4, 5, 6}, submission.publicInputsBytes())
        : "BSC local admission must copy public input bytes";
    assert Arrays.equals(new byte[] {7, 8, 9}, submission.bundleBytes())
        : "BSC local admission must copy bundle bytes";
    assert Arrays.equals(new byte[] {10, 11, 12}, submission.envelopeBytes())
        : "BSC local admission must copy envelope bytes";
    assert Arrays.equals(new byte[] {1, 2, 3}, submission.localAdmission().proofBytes())
        : "BSC local admission payload must carry proof bytes";
    assert submission.envelopeHex().equals(facadeSubmission.envelopeHex())
        : "facade local admission helper must match static helper";

    input.proofBytes()[0] = 99;
    assert Arrays.equals(new byte[] {1, 2, 3}, submission.proofBytes())
        : "BSC local admission must not expose mutable proof storage";

    boolean threw = false;
    try {
      BscMainnetSccp.buildLocalAdmissionSubmission(
          new BscMainnetSccp.LocalAdmissionSubmissionInput(
              new byte[] {1, 2, 3},
              new byte[] {4, 5, 6},
              new byte[] {7, 8, 9},
              new byte[] {10, 11, 12},
              "0x" + repeat("66", 32),
              "0x" + repeat("77", 32),
              "0x" + repeat("88", 32),
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              BscMainnetSccp.STARK_FRI_PROOF_FAMILY_V1,
              EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
              BscMainnetSccp.LOCAL_ADMISSION_ENVELOPE_ENCODING_V1,
              BscMainnetSccp.LOCAL_ADMISSION_SUBMISSION_KIND_V1,
              BscMainnetSccp.LOCAL_ADMISSION_ENTRYPOINT_V1));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("BSC -> SORA");
    }
    assert threw : "BSC local admission must reject wrong source domains";

    threw = false;
    try {
      BscMainnetSccp.buildLocalAdmissionSubmission(
          new BscMainnetSccp.LocalAdmissionSubmissionInput(
              new byte[] {0, 0},
              new byte[] {4, 5, 6},
              new byte[] {7, 8, 9},
              new byte[] {10, 11, 12},
              "0x" + repeat("66", 32),
              "0x" + repeat("77", 32),
              "0x" + repeat("88", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes must not be all zero");
    }
    assert threw : "BSC local admission must reject all-zero proof bytes";

    threw = false;
    try {
      BscMainnetSccp.buildLocalAdmissionSubmission(
          new BscMainnetSccp.LocalAdmissionSubmissionInput(
              new byte[] {1, 2, 3},
              new byte[] {4, 5, 6},
              new byte[] {7, 8, 9},
              new byte[0],
              "0x" + repeat("66", 32),
              "0x" + repeat("77", 32),
              "0x" + repeat("88", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("envelopeBytes must not be empty");
    }
    assert threw : "BSC local admission must reject empty envelope bytes";

    threw = false;
    try {
      BscMainnetSccp.buildLocalAdmissionSubmission(
          new BscMainnetSccp.LocalAdmissionSubmissionInput(
              new byte[] {1, 2, 3},
              new byte[] {4, 5, 6},
              new byte[] {7, 8, 9},
              new byte[] {10, 11, 12},
              "0x" + repeat("66", 32),
              "0x" + repeat("77", 32),
              "0x" + repeat("88", 32),
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              BscMainnetSccp.STARK_FRI_PROOF_FAMILY_V1,
              EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
              "abi_tuple_v1",
              BscMainnetSccp.LOCAL_ADMISSION_SUBMISSION_KIND_V1,
              BscMainnetSccp.LOCAL_ADMISSION_ENTRYPOINT_V1));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("envelopeEncoding");
    }
    assert threw : "BSC local admission must reject stale metadata";

    threw = false;
    try {
      BscMainnetSccp.buildLocalAdmissionSubmission(
          new BscMainnetSccp.LocalAdmissionSubmissionInput(
              new byte[] {1, 2, 3},
              new byte[] {4, 5, 6},
              new byte[] {7, 8, 9},
              new byte[] {10, 11, 12},
              "0x" + repeat("66", 32),
              "0x" + repeat("77", 32),
              "0x" + repeat("88", 32),
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              "debug-proof-family",
              EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
              BscMainnetSccp.LOCAL_ADMISSION_ENVELOPE_ENCODING_V1,
              BscMainnetSccp.LOCAL_ADMISSION_SUBMISSION_KIND_V1,
              BscMainnetSccp.LOCAL_ADMISSION_ENTRYPOINT_V1));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofFamily");
    }
    assert threw : "BSC local admission must reject stale proof families";
  }

  private static void ethereumMainnetFacadeRequiresChainId1AndEthTarget() {
    final byte[] proofBytes = sampleGroth16ProofBytes();
    EthereumMainnetSccp.requireMainnetChainId(1L);
    assert "0x577b41c65ffbce226de59f224b464797257063747891b88ebec1bcd57af82727"
            .equals(EthereumMainnetSccp.sourceEventTopic())
        : "Ethereum source-event topic must bind SccpSourceEvent(bytes32)";
    boolean threw = false;
    try {
      EthereumMainnetSccp.requireMainnetChainId(56L);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("eth_chainId == 1");
    }
    assert threw : "Ethereum mainnet chain guard must reject BSC chain id";

    final SourceSccpProofs.EvmDestinationBinding binding =
        EthereumMainnetSccp.destinationBinding(
            "0x" + repeat("11", 20),
            "0x" + repeat("22", 20),
            "0x" + repeat("bb", 32),
            "0x" + repeat("cc", 32));
    assert SourceSccpProofs.ETH_MAINNET_NETWORK_ID.equals(binding.networkId)
        : "Ethereum binding must default to chain id 1";
    assert binding.sourceDomain == EvmSccpProver.DOMAIN_SORA
        : "Ethereum binding must start from SORA";
    assert binding.targetDomain == EvmSccpProver.DOMAIN_ETH
        : "Ethereum binding must target ETH";
    assert binding.hash.equals(
            EthereumMainnetSccp.destinationBindingHash(
                "0x" + repeat("11", 20),
                "0x" + repeat("22", 20),
                "0x" + repeat("bb", 32),
                "0x" + repeat("cc", 32)))
        : "Ethereum binding hash helper must match binding";

    final EvmSccpProver.ProofRequestInput input =
        new EvmSccpProver.ProofRequestInput(
            samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
            new byte[] {5, 6, 7},
            new byte[] {9, 10},
            repeat("56", 32),
            binding);
    final EvmSccpProver.ProofRequest request = EthereumMainnetSccp.buildProofRequest(input);
    assert request.targetDomain() == EvmSccpProver.DOMAIN_ETH
        : "Ethereum request must target ETH";
    assert binding.hash.equals(request.destinationBindingHash())
        : "Ethereum request must bind the Ethereum destination binding";
    final EvmSccpProver.ProofRequest instanceRequest =
        new EthereumMainnetSccp().buildOutboundProofRequest(input);
    assert request.requestHash().equals(instanceRequest.requestHash())
        : "Ethereum facade request builder must use the static helper request hash";
    assert Arrays.equals(request.bundleBytes(), instanceRequest.bundleBytes())
        : "Ethereum facade request builder must preserve bundle bytes";
    assert Arrays.equals(request.sourceProofBytes(), instanceRequest.sourceProofBytes())
        : "Ethereum facade request builder must preserve source proof bytes";

    final EvmSccpProver.ProofResult result =
        EthereumMainnetSccp.wrapProofResult(proofBytes, request);
    final EvmSccpProver.Submission submission =
        new EthereumMainnetSccp().buildEthereumCalldata(new EvmSccpProver.SubmissionInput(result));
    assert submission.targetDomain() == EvmSccpProver.DOMAIN_ETH
        : "Ethereum submission must target ETH";
    assert Arrays.equals(proofBytes, submission.proofBytes())
        : "Ethereum submission must preserve proof bytes";
    final Object submitted =
        new EthereumMainnetSccp(
                null,
                null,
                null,
                null,
                null,
                null,
                outboundSubmission -> {
                  assert outboundSubmission.targetDomain() == EvmSccpProver.DOMAIN_ETH
                      : "Ethereum outbound submitter must receive ETH calldata";
                  assert Arrays.equals(proofBytes, outboundSubmission.proofBytes())
                      : "Ethereum outbound submitter must receive proof bytes";
                  return "eth-submitted";
                })
            .submitOutboundToEthereum(new EvmSccpProver.SubmissionInput(result));
    assert "eth-submitted".equals(submitted)
        : "Ethereum outbound submitter must return app-owned submission result";
    threw = false;
    try {
      new EthereumMainnetSccp().submitOutboundToEthereum(new EvmSccpProver.SubmissionInput(result));
    } catch (final IllegalStateException ex) {
      threw = ex.getMessage().contains("outbound submitter");
    }
    assert threw : "Ethereum outbound submission requires an app-owned submitter";

    threw = false;
    try {
      new EthereumMainnetSccp()
          .buildEthereumCalldata(
              new EvmSccpProver.SubmissionInput(
                  samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
                  proofBytes,
                  repeat("56", 32),
                  binding.hash));
    } catch (final NullPointerException ex) {
      threw = ex.getMessage().contains("wrapped proofResult");
    }
    assert threw : "Ethereum mainnet calldata helper must require a wrapped proof result";

    threw = false;
    try {
      SourceSccpProofs.ethereumMainnetDestinationBinding(
          "0x" + repeat("11", 20),
          "0x" + repeat("22", 20),
          "0x" + repeat("bb", 32),
          "0x" + repeat("cc", 32),
          "0x" + repeat("33", 32));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("chain id 1");
    }
    assert threw : "Ethereum destination helper must reject non-mainnet network ids";

    threw = false;
    final SourceSccpProofs.EvmDestinationBinding bscBinding =
        BscSccpProver.destinationBinding(
            "0x" + repeat("11", 20),
            "0x" + repeat("22", 20),
            "0x" + repeat("bb", 32),
            "0x" + repeat("cc", 32));
    try {
      EthereumMainnetSccp.buildProofRequest(
          new EvmSccpProver.ProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_BSC),
              new byte[] {5, 6, 7},
              new byte[0],
              repeat("56", 32),
              bscBinding));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("target ETH");
    }
    assert threw : "Ethereum request helper must reject BSC public inputs";

    threw = false;
    try {
      EthereumMainnetSccp.buildProofRequest(
          new EvmSccpProver.ProofRequestInput(
              samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
              new byte[] {5, 6, 7},
              new byte[0],
              repeat("56", 32),
              binding.hash,
              EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
              EvmSccpProver.DOMAIN_BSC,
              binding));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("SORA -> ETH");
    }
    assert threw : "Ethereum request helper must reject non-SORA outbound source domains";

    final String txHash = "0x" + repeat("aa", 32);
    final String blockHash = "0x" + repeat("bb", 32);
    final String sourceEventDigest = "0x" + repeat("ee", 32);
    final String sourceBridgeEmitterAddress = "0x" + repeat("12", 20);
    final Map<String, Object> unrelatedLog = linkedMap(
        "address", "0x" + repeat("00", 20),
        "topics", Arrays.asList("0x" + repeat("00", 32)),
        "data", "0x1234");
    final Map<String, Object> sourceEventLog = linkedMap(
        "address", sourceBridgeEmitterAddress,
        "transactionHash", txHash,
        "blockHash", blockHash,
        "blockNumber", "0x1234",
        "topics", Arrays.asList(EthereumMainnetSccp.sourceEventTopic(), sourceEventDigest),
        "data", "0x");
    final Map<String, Object> receipt = linkedMap(
        "transactionHash", txHash,
        "blockHash", blockHash,
        "blockNumber", "0x1234",
        "status", "0x1");
    final Map<String, Object> receiptWithSourceEvent = new LinkedHashMap<>(receipt);
    receiptWithSourceEvent.put("logs", Arrays.asList(unrelatedLog, sourceEventLog));
    final Map<String, Object> block = linkedMap(
        "hash", blockHash,
        "number", "0x1234",
        "receiptsRoot", "0x" + repeat("cc", 32));
    final EthereumMainnetSccp.BeaconFinalityEvidence beaconFinalityEvidence =
        new EthereumMainnetSccp.BeaconFinalityEvidence(
            "0x1234",
            blockHash,
            "0x" + repeat("cc", 32),
            linkedMap("finalizedHeaderRoot", "0x" + repeat("dd", 32)));
    final Map<String, Object> beaconFinality = beaconFinalityEvidence.toMap();
    final EthereumMainnetSccp.ReceiptProof receiptProof =
        new EthereumMainnetSccp.ReceiptProof(
            sourceEventDigest,
            "32",
            "4660",
            blockHash,
            "0x" + repeat("cc", 32),
            "0x" + repeat("dd", 32),
            "0x" + repeat("aa", 32),
            "3",
            Arrays.asList(new byte[] {1}, new byte[] {2, 3}),
            Arrays.asList(hexWord(repeat("11", 32))));
    final String receiptProofHash =
        SourceSccpProofs.evmReceiptProofHash(
            receiptProof.sourceEventDigest(),
            receiptProof.beaconSlot(),
            receiptProof.executionBlockNumber(),
            receiptProof.executionBlockHash(),
            receiptProof.executionReceiptsRoot(),
            receiptProof.beaconFinalizedRoot(),
            receiptProof.syncCommitteeRoot(),
            receiptProof.receiptRootIndex(),
            receiptProof.receiptTrieProofNodes(),
            receiptProof.inclusionBranch());
    assert "0x39f014e3f5f8d38b44d59f1afdf72ceb71d10d6d937f268c404b046f092b38f0"
            .equals(receiptProofHash)
        : "Ethereum receipt-proof vector must match the shared native hash";
    final ArrayList<String> calls = new ArrayList<>();
    final int[] consensusCalls = new int[1];
    final EthereumMainnetSccp sdk =
        new EthereumMainnetSccp(
            null,
            null,
            (method, params) -> {
              calls.add(method);
              if ("eth_chainId".equals(method)) {
                return "0x1";
              }
              if ("eth_getTransactionReceipt".equals(method)) {
                assert params.size() == 1 && txHash.equals(params.get(0))
                    : "receipt request must use the requested tx hash";
                return receipt;
              }
              if ("eth_getBlockByHash".equals(method)) {
                assert params.size() == 2 && blockHash.equals(params.get(0))
                    : "block request must use receipt block hash";
                return block;
              }
              throw new IllegalArgumentException("unexpected method " + method);
            },
            (collectedReceipt, collectedBlock, collectedTransactionHash) -> {
              consensusCalls[0]++;
              assert receipt.equals(collectedReceipt)
                  : "consensus provider must receive collected receipt";
              assert block.equals(collectedBlock)
                  : "consensus provider must receive collected block";
              assert txHash.equals(collectedTransactionHash)
                  : "consensus provider must receive collected tx hash";
              return beaconFinality;
            },
            evidence -> {
              assert evidence.sourceDomain() == EvmSccpProver.DOMAIN_ETH
                  : "inbound evidence must be ETH sourced";
              assert evidence.targetDomain() == EvmSccpProver.DOMAIN_SORA
                  : "inbound evidence must target SORA";
              assert txHash.equals(evidence.transactionHash())
                  : "inbound evidence must carry normalized tx hash";
              assert "4660".equals(evidence.beaconFinality().get("executionBlockNumber"))
                  : "inbound evidence must carry normalized finality block number";
              assert blockHash.equals(evidence.beaconFinality().get("executionBlockHash"))
                  : "inbound evidence must carry finality block hash";
              assert receiptProofHash.equals(evidence.receiptProofHash())
                  : "inbound evidence must carry receipt proof hash";
              assert receiptProof.sourceEventDigest().equals(evidence.receiptProof().sourceEventDigest())
                  : "inbound evidence must carry receipt proof material";
              assert sourceEventDigest.equals(evidence.sourceEventDigest())
                  : "inbound evidence must carry validated source event digest";
              return new byte[] {1, 2, 3};
            },
            proof -> {
              assert Arrays.equals(new byte[] {1, 2, 3}, proof)
                  : "inbound submitter must receive proof bytes";
              return "submitted";
            });
    final EthereumMainnetSccp.InboundEvidence evidence =
        sdk.collectInboundEvidenceFromReceipt(
            new EthereumMainnetSccp.InboundEvidence(
                EvmSccpProver.DOMAIN_ETH,
                EvmSccpProver.DOMAIN_SORA,
                txHash,
                null,
                null,
                null,
                null));
    assert txHash.equals(evidence.transactionHash()) : "inbound evidence must retain tx hash";
    assert receipt.equals(evidence.receipt()) : "inbound evidence must carry receipt";
    assert block.equals(evidence.block()) : "inbound evidence must carry block";
    assert "4660".equals(evidence.beaconFinality().get("executionBlockNumber"))
        : "inbound evidence must normalize beacon finality block number";
    assert ("0x" + repeat("cc", 32)).equals(evidence.beaconFinality().get("executionReceiptsRoot"))
        : "inbound evidence must carry beacon finality receipt root";
    assert consensusCalls[0] == 1 : "consensus provider must be called once";
    assert calls.equals(Arrays.asList("eth_chainId", "eth_getTransactionReceipt", "eth_getBlockByHash"))
        : "inbound collection must validate mainnet and fetch receipt/block";
    assert "submitted".equals(sdk.submitInboundToIroha(new byte[] {1, 2, 3}))
        : "inbound submitter must return caller result";

    final EthereumMainnetSccp.InboundEvidence sourceEventEvidence =
        sdk.collectInboundEvidenceFromReceipt(
            new EthereumMainnetSccp.InboundEvidence(
                EvmSccpProver.DOMAIN_ETH,
                EvmSccpProver.DOMAIN_SORA,
                null,
                receiptWithSourceEvent,
                block,
                beaconFinality,
                null,
                null,
                sourceBridgeEmitterAddress));
    assert sourceEventDigest.equals(sourceEventEvidence.sourceEventDigest())
        : "source-event validation must derive the receipt event digest";
    assert sourceBridgeEmitterAddress.equals(sourceEventEvidence.sourceBridgeEmitterAddress())
        : "source-event validation must retain the normalized bridge emitter";
    final EthereumMainnetSccp.InboundEvidence explicitSourceEventEvidence =
        sdk.collectInboundEvidenceFromReceipt(
            new EthereumMainnetSccp.InboundEvidence(
                EvmSccpProver.DOMAIN_ETH,
                EvmSccpProver.DOMAIN_SORA,
                null,
                receiptWithSourceEvent,
                block,
                beaconFinality,
                null,
                sourceEventDigest,
                sourceBridgeEmitterAddress));
    assert sourceEventDigest.equals(explicitSourceEventEvidence.sourceEventDigest())
        : "source-event validation must accept the expected digest";

    final EthereumMainnetSccp.InboundEvidence receiptProofEvidence =
        new EthereumMainnetSccp()
            .collectInboundEvidenceFromReceipt(
                new EthereumMainnetSccp.InboundEvidence(
                    EvmSccpProver.DOMAIN_ETH,
                    EvmSccpProver.DOMAIN_SORA,
                    null,
                    null,
                    null,
                    null,
                    receiptProof,
                    receiptProofHash,
                    null,
                    null));
    assert receiptProofHash.equals(receiptProofEvidence.receiptProofHash())
        : "Ethereum inbound collection must derive receiptProofHash from receiptProof";
    assert receiptProof == receiptProofEvidence.receiptProof()
        : "Ethereum inbound collection must retain app-collected receiptProof";
    final EthereumMainnetSccp.InboundEvidence unanchoredProofEvidence =
        new EthereumMainnetSccp.InboundEvidence(
            EvmSccpProver.DOMAIN_ETH,
            EvmSccpProver.DOMAIN_SORA,
            evidence.transactionHash(),
            evidence.receipt(),
            evidence.block(),
            evidence.beaconFinality(),
            receiptProof,
            receiptProofHash,
            null,
            null);
    threw = false;
    try {
      sdk.proveInboundToSora(unanchoredProofEvidence);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receipt source event validation");
    }
    assert threw : "Ethereum inbound proving must reject receipt proofs without source event validation";
    final EthereumMainnetSccp.InboundEvidence proofReadyEvidence =
        new EthereumMainnetSccp.InboundEvidence(
            EvmSccpProver.DOMAIN_ETH,
            EvmSccpProver.DOMAIN_SORA,
            sourceEventEvidence.transactionHash(),
            sourceEventEvidence.receipt(),
            sourceEventEvidence.block(),
            sourceEventEvidence.beaconFinality(),
            receiptProof,
            receiptProofHash,
            sourceEventEvidence.sourceEventDigest(),
            sourceEventEvidence.sourceBridgeEmitterAddress());
    assert Arrays.equals(
            new byte[] {1, 2, 3}, sdk.proveInboundToSora(proofReadyEvidence))
        : "inbound prover must receive receipt-proof-backed validated evidence";
    threw = false;
    try {
      new EthereumMainnetSccp(
              null, null, null, null, emptyProofEvidence -> new byte[0], null)
          .proveInboundToSora(proofReadyEvidence);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes must not be empty");
    }
    assert threw : "Ethereum inbound prover output must reject empty proof bytes";
    threw = false;
    try {
      new EthereumMainnetSccp(
              null, null, null, null, zeroProofEvidence -> new byte[] {0, 0}, null)
          .proveInboundToSora(proofReadyEvidence);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes must not be all zero");
    }
    assert threw : "Ethereum inbound prover output must reject all-zero proof bytes";
    threw = false;
    try {
      new EthereumMainnetSccp()
          .collectInboundEvidenceFromReceipt(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  null,
                  null,
                  null,
                  receiptProof,
                  "0x" + repeat("99", 32),
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receiptProofHash must match receiptProof");
    }
    assert threw : "Ethereum inbound collection must reject conflicting receiptProofHash";
    threw = false;
    try {
      new EthereumMainnetSccp()
          .collectInboundEvidenceFromReceipt(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  null,
                  null,
                  null,
                  new EthereumMainnetSccp.ReceiptProof(
                      EvmSccpProver.DOMAIN_BSC,
                      receiptProof.sourceEventDigest(),
                      receiptProof.beaconSlot(),
                      receiptProof.executionBlockNumber(),
                      receiptProof.executionBlockHash(),
                      receiptProof.executionReceiptsRoot(),
                      receiptProof.beaconFinalizedRoot(),
                      receiptProof.syncCommitteeRoot(),
                      receiptProof.receiptRootIndex(),
                      receiptProof.receiptTrieProofNodes(),
                      receiptProof.inclusionBranch()),
                  null,
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receiptProof.sourceDomain");
    }
    assert threw : "Ethereum inbound collection must reject cross-lane receiptProof transcripts";

    assert Arrays.equals(
            new byte[] {7, 8, 9},
            new EthereumMainnetSccp(
                    null,
                    null,
                    null,
                    null,
                    typedEvidence -> {
                      assert txHash.equals(typedEvidence.transactionHash())
                          : "typed beacon finality evidence must preserve receipt tx hash";
                      assert blockHash.equals(
                              typedEvidence.beaconFinality().get("executionBlockHash"))
                          : "typed beacon finality evidence must preserve block hash";
                      return new byte[] {7, 8, 9};
                    },
                    null)
                .proveInboundToSora(
                    new EthereumMainnetSccp.InboundEvidence(
                        EvmSccpProver.DOMAIN_ETH,
                        EvmSccpProver.DOMAIN_SORA,
                        null,
                        receiptWithSourceEvent,
                        block,
                        beaconFinalityEvidence.toMap(),
                        receiptProof,
                        receiptProofHash,
                        null,
                        sourceBridgeEmitterAddress)))
        : "typed beacon finality evidence must feed inbound proof collection";

    final ArrayList<String> perCallProviderCalls = new ArrayList<>();
    final int[] perCallConsensusCalls = new int[1];
    final EthereumMainnetSccp perCallSdk =
        new EthereumMainnetSccp(
            null,
            null,
            null,
            null,
            perCallEvidence -> {
              assert txHash.equals(perCallEvidence.transactionHash())
                  : "per-call provider must collect receipt evidence";
              assert blockHash.equals(perCallEvidence.beaconFinality().get("executionBlockHash"))
                  : "per-call consensus provider must attach finality";
              return new byte[] {4, 5, 6};
            },
            null);
    assert Arrays.equals(
            new byte[] {4, 5, 6},
            perCallSdk.proveInboundToSora(
                new EthereumMainnetSccp.InboundEvidence(
                    EvmSccpProver.DOMAIN_ETH,
                    EvmSccpProver.DOMAIN_SORA,
                    txHash,
                    null,
                    null,
                    null,
                    receiptProof,
                    receiptProofHash,
                    null,
                    sourceBridgeEmitterAddress),
                (method, params) -> {
                  perCallProviderCalls.add(method);
                  if ("eth_chainId".equals(method)) {
                    return "0x1";
                  }
                  if ("eth_getTransactionReceipt".equals(method)) {
                    return receiptWithSourceEvent;
                  }
                  if ("eth_getBlockByHash".equals(method)) {
                    return block;
                  }
                  throw new IllegalArgumentException("unexpected method " + method);
                },
                (collectedReceipt, collectedBlock, collectedTransactionHash) -> {
                  perCallConsensusCalls[0]++;
                  return beaconFinality;
                }))
        : "per-call providers must be usable for inbound proving";
    assert perCallProviderCalls.equals(
            Arrays.asList("eth_chainId", "eth_getTransactionReceipt", "eth_getBlockByHash"))
        : "per-call execution provider must collect mainnet receipt and block";
    assert perCallConsensusCalls[0] == 1
        : "per-call consensus provider must be called once";

    final int[] missingFinalityProverCalls = new int[1];
    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              null,
              null,
              null,
              evidenceWithoutFinality -> {
                missingFinalityProverCalls[0]++;
                return new byte[] {1, 2, 3};
              },
              null)
          .proveInboundToSora(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receipt,
                  block,
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("beaconFinality");
    }
    assert threw : "Ethereum inbound proving must reject missing beacon finality";
    assert missingFinalityProverCalls[0] == 0
        : "Ethereum inbound prover must not run without beacon finality";

    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              null,
              null,
              null,
              evidenceWithoutReceiptProof -> {
                missingFinalityProverCalls[0]++;
                return new byte[] {1, 2, 3};
              },
              null)
          .proveInboundToSora(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receipt,
                  block,
                  beaconFinality,
                  receiptProofHash));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receiptProof");
    }
    assert threw : "Ethereum inbound proving must reject hash-only receipt proof evidence";
    assert missingFinalityProverCalls[0] == 0
        : "Ethereum inbound prover must not run without receipt proof material";

    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              null,
              null,
              null,
              driftedReceiptProofEvidence -> {
                missingFinalityProverCalls[0]++;
                return new byte[] {1, 2, 3};
              },
              null)
          .proveInboundToSora(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receipt,
                  block,
                  beaconFinality,
                  new EthereumMainnetSccp.ReceiptProof(
                      receiptProof.sourceEventDigest(),
                      receiptProof.beaconSlot(),
                      receiptProof.executionBlockNumber(),
                      receiptProof.executionBlockHash(),
                      "0x" + repeat("99", 32),
                      receiptProof.beaconFinalizedRoot(),
                      receiptProof.syncCommitteeRoot(),
                      receiptProof.receiptRootIndex(),
                      receiptProof.receiptTrieProofNodes(),
                      receiptProof.inclusionBranch()),
                  null,
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receiptProof.executionReceiptsRoot");
    }
    assert threw : "Ethereum inbound proving must reject drifted receipt proof transcripts";
    assert missingFinalityProverCalls[0] == 0
        : "Ethereum inbound prover must not run with drifted receipt proof material";

    threw = false;
    try {
      new EthereumMainnetSccp()
          .collectInboundEvidenceFromReceipt(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  txHash,
                  null,
                  null,
                  null,
                  null));
    } catch (final IllegalStateException ex) {
      threw = ex.getMessage().contains("execution provider");
    }
    assert threw : "Ethereum inbound collection by transaction hash must require a provider";

    threw = false;
    try {
      new EthereumMainnetSccp(null, null, (method, params) -> "0x38", null, null)
          .collectInboundEvidenceFromReceipt(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receipt,
                  null,
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("eth_chainId == 1");
    }
    assert threw : "Ethereum inbound collection must reject non-mainnet RPC";

    threw = false;
    try {
      new EthereumMainnetSccp(null, null, (method, params) -> "1", null, null)
          .collectInboundEvidenceFromReceipt(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receipt,
                  null,
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("canonical JSON-RPC quantity");
    }
    assert threw : "Ethereum inbound collection must reject decimal eth_chainId RPC";

    threw = false;
    try {
      new EthereumMainnetSccp(null, null, (method, params) -> Long.valueOf(1L), null, null)
          .collectInboundEvidenceFromReceipt(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receipt,
                  null,
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("canonical JSON-RPC quantity");
    }
    assert threw : "Ethereum inbound collection must reject numeric eth_chainId RPC";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              linkedMap(
                  "transactionHash", txHash,
                  "blockHash", blockHash,
                  "blockNumber", "0x1234",
                  "status", "0x0"),
              null,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("status must be 0x1");
    }
    assert threw : "Ethereum inbound collection must reject failed receipts";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              linkedMap(
                  "transactionHash", txHash,
                  "blockHash", blockHash,
                  "status", "0x1"),
              block,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receipt.blockNumber");
    }
    assert threw : "Ethereum inbound collection must reject receipts without block numbers";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              linkedMap(
                  "transactionHash", txHash,
                  "blockHash", blockHash,
                  "blockNumber", "0x0",
                  "status", "0x1"),
              block,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receipt.blockNumber");
    }
    assert threw : "Ethereum inbound collection must reject zero receipt block numbers";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              txHash,
              linkedMap(
                  "transactionHash", "0x" + repeat("ab", 32),
                  "blockHash", blockHash,
                  "blockNumber", "0x1234",
                  "status", "0x1"),
              null,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("transactionHash must match");
    }
    assert threw : "Ethereum inbound collection must reject receipt tx drift";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              linkedMap(
                  "hash", "0x" + repeat("bc", 32),
                  "number", "0x1234",
                  "receiptsRoot", "0x" + repeat("cc", 32)),
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("block.hash must match");
    }
    assert threw : "Ethereum inbound collection must reject block hash drift";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              linkedMap(
                  "hash", blockHash,
                  "receiptsRoot", "0x" + repeat("cc", 32)),
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("block.number");
    }
    assert threw : "Ethereum inbound collection must reject blocks without numbers";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              linkedMap(
                  "hash", blockHash,
                  "number", "0x0",
                  "receiptsRoot", "0x" + repeat("cc", 32)),
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("block.number");
    }
    assert threw : "Ethereum inbound collection must reject zero block numbers";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              linkedMap(
                  "transactionHash", txHash.toUpperCase(),
                  "blockHash", blockHash,
                  "blockNumber", "0x1234",
                  "status", "0x1"),
              null,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("canonical lowercase");
    }
    assert threw : "Ethereum inbound collection must reject uppercase RPC hashes";

    final Map<String, Object> driftedFinalityHash = new LinkedHashMap<>(beaconFinality);
    driftedFinalityHash.put("executionBlockHash", "0x" + repeat("bc", 32));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              block,
              driftedFinalityHash,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("beaconFinality.executionBlockHash");
    }
    assert threw : "Ethereum inbound collection must reject finality block-hash drift";

    final Map<String, Object> driftedFinalityNumber = new LinkedHashMap<>(beaconFinality);
    driftedFinalityNumber.put("executionBlockNumber", "0x1235");
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              block,
              driftedFinalityNumber,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("beaconFinality.executionBlockNumber");
    }
    assert threw : "Ethereum inbound collection must reject finality block-number drift";

    final Map<String, Object> driftedFinalityReceiptsRoot = new LinkedHashMap<>(beaconFinality);
    driftedFinalityReceiptsRoot.put("executionReceiptsRoot", "0x" + repeat("cd", 32));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              block,
              driftedFinalityReceiptsRoot,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("beaconFinality.executionReceiptsRoot");
    }
    assert threw : "Ethereum inbound collection must reject finality receipts-root drift";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receiptWithSourceEvent,
              block,
              beaconFinality,
              null,
              sourceEventDigest,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceBridgeEmitterAddress");
    }
    assert threw : "Ethereum source-event validation must require the source bridge emitter";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              block,
              beaconFinality,
              null,
              null,
              sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receipt.logs");
    }
    assert threw : "Ethereum source-event validation must require receipt logs";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receiptWithSourceEvent,
              block,
              beaconFinality,
              null,
              null,
              "0x" + repeat("13", 20)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("expected SCCP source event");
    }
    assert threw : "Ethereum source-event validation must reject a wrong bridge emitter";

    final Map<String, Object> wrongTopicLog = new LinkedHashMap<>(sourceEventLog);
    wrongTopicLog.put("topics", Arrays.asList("0x" + repeat("ab", 32), sourceEventDigest));
    final Map<String, Object> wrongTopicReceipt = new LinkedHashMap<>(receipt);
    wrongTopicReceipt.put("logs", Arrays.asList(wrongTopicLog));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              wrongTopicReceipt,
              block,
              beaconFinality,
              null,
              null,
              sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("expected SCCP source event");
    }
    assert threw : "Ethereum source-event validation must reject a wrong event topic";

    final Map<String, Object> duplicateReceipt = new LinkedHashMap<>(receipt);
    duplicateReceipt.put("logs", Arrays.asList(sourceEventLog, sourceEventLog));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              duplicateReceipt,
              block,
              beaconFinality,
              null,
              null,
              sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("exactly one");
    }
    assert threw : "Ethereum source-event validation must reject duplicate matching events";

    final Map<String, Object> removedLog = new LinkedHashMap<>(sourceEventLog);
    removedLog.put("removed", Boolean.TRUE);
    final Map<String, Object> removedReceipt = new LinkedHashMap<>(receipt);
    removedReceipt.put("logs", Arrays.asList(removedLog));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              removedReceipt,
              block,
              beaconFinality,
              null,
              null,
              sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("removed logs");
    }
    assert threw : "Ethereum source-event validation must reject removed logs";

    final Map<String, Object> nonObjectLogReceipt = new LinkedHashMap<>(receipt);
    nonObjectLogReceipt.put("logs", Arrays.asList("not-a-log"));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              nonObjectLogReceipt,
              block,
              beaconFinality,
              null,
              null,
              sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receipt.logs[0] must be an object");
    }
    assert threw : "Ethereum source-event validation must reject non-object logs";

    final Map<String, Object> missingDataLog = new LinkedHashMap<>(sourceEventLog);
    missingDataLog.remove("data");
    final Map<String, Object> missingDataReceipt = new LinkedHashMap<>(receipt);
    missingDataReceipt.put("logs", Arrays.asList(missingDataLog));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              missingDataReceipt,
              block,
              beaconFinality,
              null,
              null,
              sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receipt.logs[0].data");
    }
    assert threw : "Ethereum source-event validation must reject source logs without data";

    final Map<String, Object> driftedLogTransaction = new LinkedHashMap<>(sourceEventLog);
    driftedLogTransaction.put("transactionHash", "0x" + repeat("ab", 32));
    final Map<String, Object> driftedLogTransactionReceipt = new LinkedHashMap<>(receipt);
    driftedLogTransactionReceipt.put("logs", Arrays.asList(driftedLogTransaction));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              driftedLogTransactionReceipt,
              block,
              beaconFinality,
              null,
              null,
              sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("transactionHash must match");
    }
    assert threw : "Ethereum source-event validation must reject log transaction hash drift";

    final Map<String, Object> driftedLogBlockHash = new LinkedHashMap<>(sourceEventLog);
    driftedLogBlockHash.put("blockHash", "0x" + repeat("ab", 32));
    final Map<String, Object> driftedLogBlockHashReceipt = new LinkedHashMap<>(receipt);
    driftedLogBlockHashReceipt.put("logs", Arrays.asList(driftedLogBlockHash));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              driftedLogBlockHashReceipt,
              block,
              beaconFinality,
              null,
              null,
              sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("blockHash must match");
    }
    assert threw : "Ethereum source-event validation must reject log block hash drift";

    final Map<String, Object> driftedLogBlockNumber = new LinkedHashMap<>(sourceEventLog);
    driftedLogBlockNumber.put("blockNumber", "0x1235");
    final Map<String, Object> driftedLogBlockNumberReceipt = new LinkedHashMap<>(receipt);
    driftedLogBlockNumberReceipt.put("logs", Arrays.asList(driftedLogBlockNumber));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              driftedLogBlockNumberReceipt,
              block,
              beaconFinality,
              null,
              null,
              sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("blockNumber must match");
    }
    assert threw : "Ethereum source-event validation must reject log block number drift";

    threw = false;
    try {
      sdk.submitInboundToIroha(new byte[] {0, 0});
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("all zero");
    }
    assert threw : "Ethereum inbound submitter must reject zero proof bytes";
  }

  private static void ethereumMainnetFacadeBuildsLocalAdmissionSubmission() {
    final EthereumMainnetSccp.LocalAdmissionSubmissionInput input =
        new EthereumMainnetSccp.LocalAdmissionSubmissionInput(
            new byte[] {1, 2, 3},
            new byte[] {4, 5, 6},
            new byte[] {7, 8, 9},
            new byte[] {10, 11, 12},
            "0x" + repeat("66", 32),
            "0x" + repeat("77", 32),
            "0x" + repeat("88", 32));
    final EthereumMainnetSccp.LocalAdmissionSubmission submission =
        EthereumMainnetSccp.buildLocalAdmissionSubmission(input);
    final EthereumMainnetSccp.LocalAdmissionSubmission facadeSubmission =
        new EthereumMainnetSccp().buildLocalAdmission(input);

    assert EthereumMainnetSccp.LOCAL_ADMISSION_SUBMISSION_KIND_V1.equals(
            submission.platformPayload())
        : "Ethereum local admission platform payload must be local_admission";
    assert EthereumMainnetSccp.LOCAL_ADMISSION_ENVELOPE_ENCODING_V1.equals(
            submission.envelopeEncoding())
        : "Ethereum local admission must use the Norito envelope";
    assert EthereumMainnetSccp.LOCAL_ADMISSION_ENTRYPOINT_V1.equals(
            submission.verifierEntrypoint())
        : "Ethereum local admission must target SubmitBridgeProof";
    assert submission.sourceDomain() == EvmSccpProver.DOMAIN_ETH
        : "Ethereum local admission source must be ETH";
    assert submission.targetDomain() == EvmSccpProver.DOMAIN_SORA
        : "Ethereum local admission target must be SORA";
    assert submission.arguments().isEmpty() : "Ethereum local admission must not add call arguments";
    assert Arrays.equals(new byte[] {1, 2, 3}, submission.proofBytes())
        : "Ethereum local admission must copy proof bytes";
    assert Arrays.equals(new byte[] {4, 5, 6}, submission.publicInputsBytes())
        : "Ethereum local admission must copy public input bytes";
    assert Arrays.equals(new byte[] {7, 8, 9}, submission.bundleBytes())
        : "Ethereum local admission must copy bundle bytes";
    assert Arrays.equals(new byte[] {10, 11, 12}, submission.envelopeBytes())
        : "Ethereum local admission must copy envelope bytes";
    assert Arrays.equals(new byte[] {1, 2, 3}, submission.localAdmission().proofBytes())
        : "Ethereum local admission payload must carry proof bytes";
    assert submission.envelopeHex().equals(facadeSubmission.envelopeHex())
        : "facade local admission helper must match static helper";

    input.proofBytes()[0] = 99;
    assert Arrays.equals(new byte[] {1, 2, 3}, submission.proofBytes())
        : "Ethereum local admission must not expose mutable proof storage";

    boolean threw = false;
    try {
      EthereumMainnetSccp.buildLocalAdmissionSubmission(
          new EthereumMainnetSccp.LocalAdmissionSubmissionInput(
              new byte[] {1, 2, 3},
              new byte[] {4, 5, 6},
              new byte[] {7, 8, 9},
              new byte[] {10, 11, 12},
              "0x" + repeat("66", 32),
              "0x" + repeat("77", 32),
              "0x" + repeat("88", 32),
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              EthereumMainnetSccp.STARK_FRI_PROOF_FAMILY_V1,
              EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
              EthereumMainnetSccp.LOCAL_ADMISSION_ENVELOPE_ENCODING_V1,
              EthereumMainnetSccp.LOCAL_ADMISSION_SUBMISSION_KIND_V1,
              EthereumMainnetSccp.LOCAL_ADMISSION_ENTRYPOINT_V1));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("ETH -> SORA");
    }
    assert threw : "Ethereum local admission must reject wrong source domains";

    threw = false;
    try {
      EthereumMainnetSccp.buildLocalAdmissionSubmission(
          new EthereumMainnetSccp.LocalAdmissionSubmissionInput(
              new byte[] {0, 0},
              new byte[] {4, 5, 6},
              new byte[] {7, 8, 9},
              new byte[] {10, 11, 12},
              "0x" + repeat("66", 32),
              "0x" + repeat("77", 32),
              "0x" + repeat("88", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofBytes must not be all zero");
    }
    assert threw : "Ethereum local admission must reject all-zero proof bytes";

    assertEthereumLocalAdmissionRejected(
        new EthereumMainnetSccp.LocalAdmissionSubmissionInput(
            new byte[] {1, 2, 3},
            new byte[] {0, 0},
            new byte[] {7, 8, 9},
            new byte[] {10, 11, 12},
            "0x" + repeat("66", 32),
            "0x" + repeat("77", 32),
            "0x" + repeat("88", 32)),
        "publicInputsBytes must not be all zero",
        "Ethereum local admission must reject all-zero public input bytes");

    assertEthereumLocalAdmissionRejected(
        new EthereumMainnetSccp.LocalAdmissionSubmissionInput(
            new byte[] {1, 2, 3},
            new byte[] {4, 5, 6},
            new byte[] {0, 0},
            new byte[] {10, 11, 12},
            "0x" + repeat("66", 32),
            "0x" + repeat("77", 32),
            "0x" + repeat("88", 32)),
        "bundleBytes must not be all zero",
        "Ethereum local admission must reject all-zero bundle bytes");

    threw = false;
    try {
      EthereumMainnetSccp.buildLocalAdmissionSubmission(
          new EthereumMainnetSccp.LocalAdmissionSubmissionInput(
              new byte[] {1, 2, 3},
              new byte[] {4, 5, 6},
              new byte[] {7, 8, 9},
              new byte[0],
              "0x" + repeat("66", 32),
              "0x" + repeat("77", 32),
              "0x" + repeat("88", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("envelopeBytes must not be empty");
    }
    assert threw : "Ethereum local admission must reject empty envelope bytes";

    assertEthereumLocalAdmissionRejected(
        new EthereumMainnetSccp.LocalAdmissionSubmissionInput(
            new byte[] {1, 2, 3},
            new byte[] {4, 5, 6},
            new byte[] {7, 8, 9},
            new byte[] {0, 0},
            "0x" + repeat("66", 32),
            "0x" + repeat("77", 32),
            "0x" + repeat("88", 32)),
        "envelopeBytes must not be all zero",
        "Ethereum local admission must reject all-zero envelope bytes");

    assertEthereumLocalAdmissionRejected(
        new EthereumMainnetSccp.LocalAdmissionSubmissionInput(
            new byte[] {1, 2, 3},
            new byte[] {4, 5, 6},
            new byte[] {7, 8, 9},
            new byte[] {10, 11, 12},
            "0x" + repeat("00", 32),
            "0x" + repeat("77", 32),
            "0x" + repeat("88", 32)),
        "statementHash must not be zero",
        "Ethereum local admission must reject zero statement hashes");

    assertEthereumLocalAdmissionRejected(
        new EthereumMainnetSccp.LocalAdmissionSubmissionInput(
            new byte[] {1, 2, 3},
            new byte[] {4, 5, 6},
            new byte[] {7, 8, 9},
            new byte[] {10, 11, 12},
            "0x" + repeat("66", 32),
            "0x" + repeat("00", 32),
            "0x" + repeat("88", 32)),
        "sourceVerifierMaterialHash must not be zero",
        "Ethereum local admission must reject zero source material hashes");

    assertEthereumLocalAdmissionRejected(
        new EthereumMainnetSccp.LocalAdmissionSubmissionInput(
            new byte[] {1, 2, 3},
            new byte[] {4, 5, 6},
            new byte[] {7, 8, 9},
            new byte[] {10, 11, 12},
            "0x" + repeat("66", 32),
            "0x" + repeat("77", 32),
            "0x" + repeat("00", 32)),
        "sourceAdapterEngineDeploymentHash must not be zero",
        "Ethereum local admission must reject zero source adapter deployment hashes");

    threw = false;
    try {
      EthereumMainnetSccp.buildLocalAdmissionSubmission(
          new EthereumMainnetSccp.LocalAdmissionSubmissionInput(
              new byte[] {1, 2, 3},
              new byte[] {4, 5, 6},
              new byte[] {7, 8, 9},
              new byte[] {10, 11, 12},
              "0x" + repeat("66", 32),
              "0x" + repeat("77", 32),
              "0x" + repeat("88", 32),
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              EthereumMainnetSccp.STARK_FRI_PROOF_FAMILY_V1,
              EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
              "abi_tuple_v1",
              EthereumMainnetSccp.LOCAL_ADMISSION_SUBMISSION_KIND_V1,
              EthereumMainnetSccp.LOCAL_ADMISSION_ENTRYPOINT_V1));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("envelopeEncoding");
    }
    assert threw : "Ethereum local admission must reject stale metadata";

    threw = false;
    try {
      EthereumMainnetSccp.buildLocalAdmissionSubmission(
          new EthereumMainnetSccp.LocalAdmissionSubmissionInput(
              new byte[] {1, 2, 3},
              new byte[] {4, 5, 6},
              new byte[] {7, 8, 9},
              new byte[] {10, 11, 12},
              "0x" + repeat("66", 32),
              "0x" + repeat("77", 32),
              "0x" + repeat("88", 32),
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              "debug-proof-family",
              EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
              EthereumMainnetSccp.LOCAL_ADMISSION_ENVELOPE_ENCODING_V1,
              EthereumMainnetSccp.LOCAL_ADMISSION_SUBMISSION_KIND_V1,
              EthereumMainnetSccp.LOCAL_ADMISSION_ENTRYPOINT_V1));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("proofFamily");
    }
    assert threw : "Ethereum local admission must reject stale proof families";
  }

  private static void assertEthereumLocalAdmissionRejected(
      final EthereumMainnetSccp.LocalAdmissionSubmissionInput input,
      final String expectedMessagePart,
      final String assertionMessage) {
    boolean threw = false;
    try {
      EthereumMainnetSccp.buildLocalAdmissionSubmission(input);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains(expectedMessagePart);
    }
    assert threw : assertionMessage;
  }

  private static void bscMainnetInboundFacadeUsesMainnetRpcAndRejectsDrift() {
    final String txHash = "0x" + repeat("aa", 32);
    final String blockHash = "0x" + repeat("bb", 32);
    final Map<String, Object> receipt = linkedMap(
        "transactionHash", txHash,
        "blockHash", blockHash,
        "blockNumber", "0x1234",
        "status", "0x1");
    final Map<String, Object> block = linkedMap(
        "hash", blockHash,
        "number", "0x1234",
        "receiptsRoot", "0x" + repeat("cc", 32));
    final BscMainnetSccp.ParliaFinalityEvidence parliaFinalityEvidence =
        new BscMainnetSccp.ParliaFinalityEvidence(
            "0x1234",
            blockHash,
            "0x" + repeat("cc", 32),
            linkedMap(
                "validatorEpoch", "0x24",
                "commitSealHash", "0x" + repeat("dd", 32)));
    final Map<String, Object> parliaFinality = parliaFinalityEvidence.toMap();
    final ArrayList<String> calls = new ArrayList<>();
    final BscMainnetSccp sdk =
        new BscMainnetSccp(
            null,
            null,
            (method, params) -> {
              calls.add(method);
              if ("eth_chainId".equals(method)) {
                return "0x38";
              }
              if ("eth_getTransactionReceipt".equals(method)) {
                assert params.size() == 1 && txHash.equals(params.get(0))
                    : "receipt request must use the requested BSC tx hash";
                return receipt;
              }
              if ("eth_getBlockByHash".equals(method)) {
                assert params.size() == 2 && blockHash.equals(params.get(0))
                    : "block request must use BSC receipt block hash";
                return block;
              }
              throw new IllegalArgumentException("unexpected method " + method);
            },
            evidence -> {
              assert evidence.sourceDomain() == EvmSccpProver.DOMAIN_BSC
                  : "BSC inbound evidence must be BSC sourced";
              assert evidence.targetDomain() == EvmSccpProver.DOMAIN_SORA
                  : "BSC inbound evidence must target SORA";
              assert txHash.equals(evidence.transactionHash())
                  : "BSC inbound evidence must carry normalized tx hash";
              assert blockHash.equals(evidence.parliaFinality().get("executionBlockHash"))
                  : "BSC inbound evidence must carry bound finality block hash";
              return new byte[] {1, 2, 3};
            },
            proof -> {
              assert Arrays.equals(new byte[] {1, 2, 3}, proof)
                  : "BSC inbound submitter must receive proof bytes";
              return "submitted";
            });
    final BscMainnetSccp.InboundEvidence evidence =
        sdk.collectInboundEvidenceFromReceipt(
            BscMainnetSccp.InboundEvidence.withParliaFinalityEvidence(
                EvmSccpProver.DOMAIN_BSC,
                EvmSccpProver.DOMAIN_SORA,
                txHash,
                null,
                null,
                parliaFinalityEvidence,
                null));
    assert txHash.equals(evidence.transactionHash()) : "BSC evidence must retain tx hash";
    assert receipt.equals(evidence.receipt()) : "BSC evidence must carry receipt";
    assert block.equals(evidence.block()) : "BSC evidence must carry block";
    assert "4660".equals(evidence.parliaFinality().get("executionBlockNumber"))
        : "BSC evidence must normalize Parlia execution block number";
    assert blockHash.equals(evidence.parliaFinality().get("executionBlockHash"))
        : "BSC evidence must retain Parlia execution block hash";
    assert calls.equals(Arrays.asList("eth_chainId", "eth_getTransactionReceipt", "eth_getBlockByHash"))
        : "BSC inbound collection must validate mainnet and fetch receipt/block";
    final BscMainnetSccp.InboundEvidence providerFinalityEvidence =
        new BscMainnetSccp(
                null,
                null,
                (method, params) -> {
                  if ("eth_chainId".equals(method)) {
                    return "0x38";
                  }
                  if ("eth_getTransactionReceipt".equals(method)) {
                    return receipt;
                  }
                  if ("eth_getBlockByHash".equals(method)) {
                    return block;
                  }
                  throw new IllegalArgumentException("unexpected method " + method);
                },
                (receiptInput, blockInput, transactionHashInput) -> parliaFinality,
                null,
                null)
            .collectInboundEvidenceFromReceipt(
                new BscMainnetSccp.InboundEvidence(
                    EvmSccpProver.DOMAIN_BSC,
                    EvmSccpProver.DOMAIN_SORA,
                    txHash,
                    null,
                    null,
                    null,
                    null));
    assert blockHash.equals(providerFinalityEvidence.parliaFinality().get("executionBlockHash"))
        : "BSC consensus provider finality must be normalized";
    assert Arrays.equals(new byte[] {1, 2, 3}, sdk.proveInboundToSora(evidence))
        : "BSC inbound prover must receive validated evidence";
    assert "submitted".equals(sdk.submitInboundToIroha(new byte[] {1, 2, 3}))
        : "BSC inbound submitter must return caller result";

    boolean threw = false;
    try {
      new BscMainnetSccp(null, null, (method, params) -> "0x1", null, null)
          .collectInboundEvidenceFromReceipt(
              new BscMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_BSC,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receipt,
                  null,
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("eth_chainId == 56");
    }
    assert threw : "BSC inbound collection must reject non-mainnet RPC";

    threw = false;
    try {
      new BscMainnetSccp(null, null, (method, params) -> "56", null, null)
          .collectInboundEvidenceFromReceipt(
              new BscMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_BSC,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receipt,
                  null,
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("canonical JSON-RPC quantity");
    }
    assert threw : "BSC inbound collection must reject decimal eth_chainId RPC";

    threw = false;
    try {
      new BscMainnetSccp(null, null, (method, params) -> Long.valueOf(56L), null, null)
          .collectInboundEvidenceFromReceipt(
              new BscMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_BSC,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receipt,
                  null,
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("canonical JSON-RPC quantity");
    }
    assert threw : "BSC inbound collection must reject numeric eth_chainId RPC";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              null,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceDomain must be BSC");
    }
    assert threw : "BSC inbound collection must reject foreign source domains";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              linkedMap(
                  "transactionHash", txHash,
                  "blockHash", blockHash,
                  "blockNumber", "0x1234",
                  "status", "0x0"),
              null,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("status must be 0x1");
    }
    assert threw : "BSC inbound collection must reject failed receipts";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              linkedMap(
                  "transactionHash", txHash,
                  "blockHash", blockHash,
                  "status", "0x1"),
              block,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receipt.blockNumber");
    }
    assert threw : "BSC inbound collection must reject receipts without block numbers";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              linkedMap(
                  "transactionHash", txHash,
                  "blockHash", blockHash,
                  "blockNumber", "0x0",
                  "status", "0x1"),
              block,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receipt.blockNumber");
    }
    assert threw : "BSC inbound collection must reject zero receipt block numbers";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              txHash,
              linkedMap(
                  "transactionHash", "0x" + repeat("ab", 32),
                  "blockHash", blockHash,
                  "blockNumber", "0x1234",
                  "status", "0x1"),
              null,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("transactionHash must match");
    }
    assert threw : "BSC inbound collection must reject receipt tx drift";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              linkedMap(
                  "hash", "0x" + repeat("bc", 32),
                  "number", "0x1234",
                  "receiptsRoot", "0x" + repeat("cc", 32)),
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("block.hash must match");
    }
    assert threw : "BSC inbound collection must reject block hash drift";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              linkedMap(
                  "hash", blockHash,
                  "receiptsRoot", "0x" + repeat("cc", 32)),
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("block.number");
    }
    assert threw : "BSC inbound collection must reject blocks without numbers";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              linkedMap(
                  "hash", blockHash,
                  "number", "0x0",
                  "receiptsRoot", "0x" + repeat("cc", 32)),
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("block.number");
    }
    assert threw : "BSC inbound collection must reject zero block numbers";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              linkedMap(
                  "hash", blockHash,
                  "number", "0x1235",
                  "receiptsRoot", "0x" + repeat("cc", 32)),
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("block.number must match");
    }
    assert threw : "BSC inbound collection must reject block number drift";

    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              linkedMap(
                  "transactionHash", txHash.toUpperCase(),
                  "blockHash", blockHash,
                  "blockNumber", "0x1234",
                  "status", "0x1"),
              null,
              null,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("canonical lowercase");
    }
    assert threw : "BSC inbound collection must reject uppercase RPC hashes";

    threw = false;
    try {
      sdk.proveInboundToSora(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              null,
              null,
              null,
              "0x" + repeat("ee", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("parliaFinality");
    }
    assert threw : "BSC inbound proving must reject missing Parlia finality";

    final Map<String, Object> driftedFinalityHash = new LinkedHashMap<>(parliaFinality);
    driftedFinalityHash.put("executionBlockHash", "0x" + repeat("bc", 32));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              block,
              driftedFinalityHash,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("parliaFinality.executionBlockHash");
    }
    assert threw : "BSC inbound collection must reject Parlia block-hash drift";

    final Map<String, Object> driftedFinalityNumber = new LinkedHashMap<>(parliaFinality);
    driftedFinalityNumber.put("executionBlockNumber", "0x1235");
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              block,
              driftedFinalityNumber,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("parliaFinality.executionBlockNumber");
    }
    assert threw : "BSC inbound collection must reject Parlia block-number drift";

    final Map<String, Object> driftedFinalityReceiptsRoot = new LinkedHashMap<>(parliaFinality);
    driftedFinalityReceiptsRoot.put("executionReceiptsRoot", "0x" + repeat("cd", 32));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new BscMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_BSC,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              block,
              driftedFinalityReceiptsRoot,
              null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("parliaFinality.executionReceiptsRoot");
    }
    assert threw : "BSC inbound collection must reject Parlia receipts-root drift";

    threw = false;
    try {
      sdk.submitInboundToIroha(new byte[] {0, 0});
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("all zero");
    }
    assert threw : "BSC inbound submitter must reject zero proof bytes";
  }

  private static void mainnetFacadesSnapshotWitnessProviderInputs() {
    final byte[] ethBundleBytes = new byte[] {5, 6, 7};
    final byte[] ethSourceProofBytes = new byte[] {9, 10};
    final SourceSccpProofs.EvmDestinationBinding ethBinding =
        EthereumMainnetSccp.destinationBinding(
            "0x" + repeat("11", 20),
            "0x" + repeat("22", 20),
            "0x" + repeat("bb", 32),
            "0x" + repeat("cc", 32));
    final EvmSccpProver.ProofRequestInput ethInput =
        new EvmSccpProver.ProofRequestInput(
            samplePublicInputs(EvmSccpProver.DOMAIN_ETH),
            ethBundleBytes,
            ethSourceProofBytes,
            repeat("56", 32),
            ethBinding);
    final EvmSccpProver.ProofRequest ethRequest =
        new EthereumMainnetSccp(
                input -> {
                  assert input.bundleBytes() != ethBundleBytes
                      : "Ethereum witness provider must receive bundle byte snapshot";
                  assert input.sourceProofBytes() != ethSourceProofBytes
                      : "Ethereum witness provider must receive source-proof byte snapshot";
                  input.bundleBytes()[0] = 0x7f;
                  input.sourceProofBytes()[0] = 0x7e;
                  return new EvmSccpProver.ProofRequestInput(
                      input.publicInputs(),
                      new byte[] {8, 8, 8},
                      new byte[] {9, 9},
                      input.statementHash(),
                      input.destinationBinding());
                },
                null)
            .buildOutboundProofRequest(ethInput);
    assert Arrays.equals(new byte[] {5, 6, 7}, ethBundleBytes)
        : "Ethereum facade must not let witness providers mutate app-owned bundle bytes";
    assert Arrays.equals(new byte[] {9, 10}, ethSourceProofBytes)
        : "Ethereum facade must not let witness providers mutate app-owned source proof bytes";
    assert Arrays.equals(new byte[] {8, 8, 8}, ethRequest.bundleBytes())
        : "Ethereum facade must use witness-resolved bundle bytes";
    assert Arrays.equals(new byte[] {9, 9}, ethRequest.sourceProofBytes())
        : "Ethereum facade must use witness-resolved source proof bytes";

    final byte[] bscBundleBytes = new byte[] {3, 4, 5};
    final byte[] bscSourceProofBytes = new byte[] {6, 7};
    final SourceSccpProofs.EvmDestinationBinding bscBinding =
        BscSccpProver.destinationBinding(
            "0x" + repeat("11", 20),
            "0x" + repeat("22", 20),
            "0x" + repeat("bb", 32),
            "0x" + repeat("cc", 32));
    final EvmSccpProver.ProofRequestInput bscInput =
        new EvmSccpProver.ProofRequestInput(
            samplePublicInputs(EvmSccpProver.DOMAIN_BSC),
            bscBundleBytes,
            bscSourceProofBytes,
            repeat("56", 32),
            bscBinding);
    final EvmSccpProver.ProofRequest bscRequest =
        new BscSccpProver(
                input -> {
                  assert input.bundleBytes() != bscBundleBytes
                      : "BSC witness provider must receive bundle byte snapshot";
                  assert input.sourceProofBytes() != bscSourceProofBytes
                      : "BSC witness provider must receive source-proof byte snapshot";
                  input.bundleBytes()[0] = 0x7f;
                  input.sourceProofBytes()[0] = 0x7e;
                  return new EvmSccpProver.ProofRequestInput(
                      input.publicInputs(),
                      new byte[] {4, 4, 4},
                      new byte[] {5, 5},
                      input.statementHash(),
                      input.destinationBinding());
                },
                null)
            .buildRequest(bscInput);
    assert Arrays.equals(new byte[] {3, 4, 5}, bscBundleBytes)
        : "BSC facade must not let witness providers mutate app-owned bundle bytes";
    assert Arrays.equals(new byte[] {6, 7}, bscSourceProofBytes)
        : "BSC facade must not let witness providers mutate app-owned source proof bytes";
    assert Arrays.equals(new byte[] {4, 4, 4}, bscRequest.bundleBytes())
        : "BSC facade must use witness-resolved bundle bytes";
    assert Arrays.equals(new byte[] {5, 5}, bscRequest.sourceProofBytes())
        : "BSC facade must use witness-resolved source proof bytes";
  }

  private static byte[] sampleGroth16ProofBytes() {
    return flattenGroth16ProofWords(sampleGroth16ProofWords());
  }

  private static byte[] sampleGroth16ProofBytes(final int wordIndex, final byte[] word) {
    final byte[][] words = sampleGroth16ProofWords();
    words[wordIndex] = Arrays.copyOf(word, word.length);
    return flattenGroth16ProofWords(words);
  }

  private static byte[] sampleGroth16ProofBytesWithZeroB() {
    final byte[][] words = sampleGroth16ProofWords();
    words[6] = new byte[32];
    words[7] = new byte[32];
    words[8] = new byte[32];
    words[9] = new byte[32];
    return flattenGroth16ProofWords(words);
  }

  private static byte[] sampleGroth16ProofBytesWithNonSubgroupB() {
    final byte[][] words = sampleGroth16ProofWords();
    words[6] = abiWord(0);
    words[7] = abiWord(1);
    words[8] = hexWord("0cf32d3c49a2cb8a092f24ec3201e68dc299b6216e6321ee60573e3a7f596ea8");
    words[9] = hexWord("07bca656753ef8cbee60335acbffe3def91636952d4ab9eb0b839c7f3566c0e2");
    return flattenGroth16ProofWords(words);
  }

  private static byte[][] sampleGroth16ProofWords() {
    return new byte[][] {
      abiWord(1),
      repeatedWord(0x11),
      abiWord(SolanaSccpProver.DOMAIN_SORA),
      repeatedWord(0x33),
      abiWord(1),
      abiWord(2),
      hexWord("1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed"),
      hexWord("198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2"),
      hexWord("12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa"),
      hexWord("090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b"),
      abiWord(1),
      abiWord(2)
    };
  }

  private static byte[] flattenGroth16ProofWords(final byte[][] words) {
    final byte[] out = new byte[EvmSccpProver.GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1];
    for (int index = 0; index < words.length; index++) {
      System.arraycopy(words[index], 0, out, index * 32, 32);
    }
    return out;
  }

  private static byte[] abiWord(final long value) {
    final byte[] out = new byte[32];
    long working = value;
    for (int index = 31; index >= 0; index--) {
      out[index] = (byte) (working & 0xffL);
      working >>>= 8;
      if (working == 0L) {
        break;
      }
    }
    return out;
  }

  private static byte[] repeatedWord(final int value) {
    final byte[] out = new byte[32];
    Arrays.fill(out, (byte) value);
    return out;
  }

  private static byte[] hexWord(final String hex) {
    if (hex.length() != 64) {
      throw new IllegalArgumentException("hex word must be 32 bytes");
    }
    final byte[] out = new byte[32];
    for (int index = 0; index < out.length; index++) {
      out[index] = (byte) Integer.parseInt(hex.substring(index * 2, index * 2 + 2), 16);
    }
    return out;
  }

  private static String hexLower(final byte[] bytes) {
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      builder.append(String.format("%02x", b & 0xff));
    }
    return builder.toString();
  }

  private static EvmSccpProver.ProofRequest evmRequestWithBackend(
      final EvmSccpProver.ProofRequest request, final String backend) {
    return new EvmSccpProver.ProofRequest(
        request.version(),
        backend,
        request.sourceDomain(),
        request.targetDomain(),
        request.publicInputs(),
        request.publicInputsBytes(),
        request.publicSignalWords(),
        request.bundleBytes(),
        request.sourceProofBytes(),
        request.proofContext(),
        request.statementHash(),
        request.destinationBindingHash(),
        request.requestHash(),
        request.destinationBinding());
  }

  private static EvmSccpProver.ProofResult evmResultWithEnvelopeHash(
      final EvmSccpProver.ProofResult result, final String envelopeHash) {
    return new EvmSccpProver.ProofResult(
        result.version(),
        result.backend(),
        result.proofBytes(),
        result.proofBase64(),
        result.publicInputs(),
        result.publicSignalWords(),
        result.bundleBytes(),
        result.sourceProofBytes(),
        result.proofContext(),
        result.statementHash(),
        result.destinationBindingHash(),
        result.requestHash(),
        envelopeHash,
        result.destinationBinding());
  }

  private static EvmSccpProver.ProofResult evmResultWithBundleBytes(
      final EvmSccpProver.ProofResult result, final byte[] bundleBytes) {
    return new EvmSccpProver.ProofResult(
        result.version(),
        result.backend(),
        result.proofBytes(),
        result.proofBase64(),
        result.publicInputs(),
        result.publicSignalWords(),
        bundleBytes,
        result.sourceProofBytes(),
        result.proofContext(),
        result.statementHash(),
        result.destinationBindingHash(),
        result.requestHash(),
        result.envelopeHash(),
        result.destinationBinding());
  }

  private static EvmSccpProver.ProofResult evmResultWithProofBase64(
      final EvmSccpProver.ProofResult result, final String proofBase64) {
    return new EvmSccpProver.ProofResult(
        result.version(),
        result.backend(),
        result.proofBytes(),
        proofBase64,
        result.publicInputs(),
        result.publicSignalWords(),
        result.bundleBytes(),
        result.sourceProofBytes(),
        result.proofContext(),
        result.statementHash(),
        result.destinationBindingHash(),
        result.requestHash(),
        result.envelopeHash(),
        result.destinationBinding());
  }

  private static EvmSccpProver.ProofRequest evmRequestWithRequestHash(
      final EvmSccpProver.ProofRequest request, final String requestHash) {
    return new EvmSccpProver.ProofRequest(
        request.version(),
        request.backend(),
        request.sourceDomain(),
        request.targetDomain(),
        request.publicInputs(),
        request.publicInputsBytes(),
        request.publicSignalWords(),
        request.bundleBytes(),
        request.sourceProofBytes(),
        request.proofContext(),
        request.statementHash(),
        request.destinationBindingHash(),
        requestHash,
        request.destinationBinding());
  }

  private static EvmSccpProver.ProofRequestInput sampleProofRequestInput(
      final EvmSccpProver.PublicInputsInput publicInputs,
      final byte[] sourceProofBytes,
      final String statementHash) {
    return sampleProofRequestInput(
        publicInputs, sourceProofBytes, statementHash, repeat("78", 32), SolanaSccpProver.DOMAIN_SORA);
  }

  private static EvmSccpProver.ProofRequestInput sampleProofRequestInput(
      final EvmSccpProver.PublicInputsInput publicInputs,
      final byte[] sourceProofBytes,
      final String statementHash,
      final String destinationBindingHash,
      final int sourceDomain) {
    return sampleProofRequestInput(
        publicInputs,
        sourceProofBytes,
        statementHash,
        destinationBindingHash,
        EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
        sourceDomain);
  }

  private static EvmSccpProver.ProofRequestInput sampleProofRequestInput(
      final EvmSccpProver.PublicInputsInput publicInputs,
      final byte[] sourceProofBytes,
      final String statementHash,
      final String destinationBindingHash,
      final String backend,
      final int sourceDomain) {
    return new EvmSccpProver.ProofRequestInput(
        publicInputs,
        new byte[] {5, 6, 7},
        sourceProofBytes,
        statementHash,
        destinationBindingHash,
        backend,
        sourceDomain);
  }

  private static EvmSccpProver.ProofRequestInput sampleProductionProofRequestInput(
      final EvmSccpProver.PublicInputsInput publicInputs,
      final byte[] sourceProofBytes,
      final String statementHash) {
    return sampleProductionProofRequestInput(
        publicInputs, new byte[] {5, 6, 7}, sourceProofBytes, statementHash);
  }

  private static EvmSccpProver.ProofRequestInput sampleProductionProofRequestInput(
      final EvmSccpProver.PublicInputsInput publicInputs,
      final byte[] bundleBytes,
      final byte[] sourceProofBytes,
      final String statementHash) {
    return new EvmSccpProver.ProofRequestInput(
        publicInputs,
        bundleBytes,
        sourceProofBytes,
        statementHash,
        sampleDestinationBinding(publicInputs),
        EvmSccpProver.GROTH16_BN254_PROOF_BACKEND_V1,
        SolanaSccpProver.DOMAIN_SORA);
  }

  private static SourceSccpProofs.EvmDestinationBinding sampleDestinationBinding(
      final EvmSccpProver.PublicInputsInput publicInputs) {
    return SourceSccpProofs.evmDestinationBinding(
        EvmSccpProver.DOMAIN_SORA,
        publicInputs.targetDomain(),
        "0x" + repeat("33", 32),
        "0x" + repeat("11", 20),
        "0x" + repeat("22", 20),
        "0x" + repeat("bb", 32),
        "0x" + repeat("cc", 32));
  }

  private static EvmSccpProver.PublicInputsInput samplePublicInputs(final int targetDomain) {
    return samplePublicInputs(targetDomain, "19");
  }

  private static EvmSccpProver.PublicInputsInput samplePublicInputs(
      final int targetDomain, final String finalityHeight) {
    return new EvmSccpProver.PublicInputsInput(
        1,
        repeat("11", 32),
        repeat("22", 32),
        targetDomain,
        repeat("33", 32),
        finalityHeight,
        repeat("44", 32));
  }

  private static Map<String, Object> linkedMap(final Object... entries) {
    if (entries.length % 2 != 0) {
      throw new IllegalArgumentException("linkedMap requires key/value pairs");
    }
    final LinkedHashMap<String, Object> out = new LinkedHashMap<>();
    for (int index = 0; index < entries.length; index += 2) {
      out.put((String) entries[index], entries[index + 1]);
    }
    return out;
  }

  private static String repeat(final String value, final int count) {
    final StringBuilder out = new StringBuilder(value.length() * count);
    for (int i = 0; i < count; i++) {
      out.append(value);
    }
    return out.toString();
  }
}
