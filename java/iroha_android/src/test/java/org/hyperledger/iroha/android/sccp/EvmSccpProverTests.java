package org.hyperledger.iroha.android.sccp;

import java.util.ArrayList;
import java.util.Arrays;

public final class EvmSccpProverTests {
  private EvmSccpProverTests() {}

  public static void main(final String[] args) {
    proofRequestBindsPublicSignalsAndRelayContext();
    proverRequiresLinkedProofEngine();
    proverWrapsExternalProofBytes();
    proverResolvesWitnessProviderBeforeBuildingRequest();
    rejectsMalformedGroth16ProofTuple();
    buildsContractCallSubmission();
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
    snapshotBundle[0] = 77;
    assert Arrays.equals(new byte[] {5, 6, 7}, callbackSnapshot.bundleBytes())
        : "snapshot bundle bytes must be defensive copies";

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

  private static String repeat(final String value, final int count) {
    final StringBuilder out = new StringBuilder(value.length() * count);
    for (int i = 0; i < count; i++) {
      out.append(value);
    }
    return out.toString();
  }
}
