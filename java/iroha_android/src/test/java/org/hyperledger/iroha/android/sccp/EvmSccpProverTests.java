package org.hyperledger.iroha.android.sccp;

import com.sun.net.httpserver.HttpServer;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

public final class EvmSccpProverTests {
  private EvmSccpProverTests() {}

  public static void main(final String[] args) throws Exception {
    proofRequestBindsPublicSignalsAndRelayContext();
    proverRequiresLinkedProofEngine();
    proverWrapsExternalProofBytes();
    proverResolvesWitnessProviderBeforeBuildingRequest();
    rejectsMalformedGroth16ProofTuple();
    buildsContractCallSubmission();
    bscMainnetFacadeRequiresChainId56AndBscTarget();
    bscMainnetFacadeBuildsLocalAdmissionSubmission();
    ethereumMainnetFacadeRequiresChainId1AndEthTarget();
    ethereumReceiptTrieProofBuilderUsesRlpTransactionIndexKeys();
    ethereumInboundCollectionBuildsReceiptProofFromBlockReceipts();
    ethereumMainnetFacadeBuildsLocalAdmissionSubmission();
    ethereumMainnetBeaconRestConsensusProviderCollectsFinalizedEvidence();
    ethereumMainnetBeaconRestHttpTransportRejectsOversizedBodies();
    ethereumMainnetBeaconRestConsensusProviderRejectsUnsafeFinality();
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
    threw = false;
    try {
      BscSccpProver.wrapProofResult(
          proofBytes, evmRequestWithDestinationBindingHash(request, "0x" + repeat("99", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("destinationBindingHash");
    }
    assert threw : "BSC wrapProofResult must reject forged destinationBindingHash";
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
    threw = false;
    try {
      EthereumMainnetSccp.wrapProofResult(
          proofBytes, evmRequestWithDestinationBindingHash(request, "0x" + repeat("99", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("destinationBindingHash");
    }
    assert threw : "Ethereum wrapProofResult must reject forged destinationBindingHash";
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
    final boolean[] guardedSubmitterCalled = new boolean[] {false};
    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              null,
              (method, params) -> {
                assert "eth_chainId".equals(method)
                    : "Ethereum outbound submit must validate the configured execution provider";
                return "0x38";
              },
              null,
              null,
              null,
              outboundSubmission -> {
                guardedSubmitterCalled[0] = true;
                return "wrong-chain";
              })
          .submitOutboundToEthereum(new EvmSccpProver.SubmissionInput(result));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("eth_chainId == 1");
    }
    assert threw : "Ethereum outbound submitter must reject configured non-mainnet execution RPC";
    assert !guardedSubmitterCalled[0] : "Ethereum outbound submitter must not run after chain-id failure";
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
    final boolean[] outboundProverCalled = new boolean[] {false};
    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              proofRequest -> {
                outboundProverCalled[0] = true;
                return proofBytes;
              })
          .proveOutboundToEthereum(
              new EvmSccpProver.ProofRequestInput(
                  samplePublicInputs(EvmSccpProver.DOMAIN_BSC),
                  new byte[] {5, 6, 7},
                  new byte[0],
                  repeat("56", 32),
                  binding));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("target ETH");
    }
    assert threw : "Ethereum outbound prove facade must reject BSC requests";
    assert !outboundProverCalled[0]
        : "Ethereum outbound prover callback must not see BSC requests";

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
            linkedMap(
                "finalizedHeaderRoot", "0x" + repeat("dd", 32),
                "syncCommitteeRoot", "0x" + repeat("aa", 32),
                "beaconSlot", "0x20"));
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
    threw = false;
    try {
      SourceSccpProofs.canonicalEvmReceiptProofBytes(
          receiptProof.sourceEventDigest(),
          receiptProof.beaconSlot(),
          receiptProof.executionBlockNumber(),
          receiptProof.executionBlockHash(),
          receiptProof.executionReceiptsRoot(),
          receiptProof.beaconFinalizedRoot(),
          receiptProof.syncCommitteeRoot(),
          receiptProof.receiptRootIndex(),
          new ArrayList<byte[]>(),
          receiptProof.inclusionBranch());
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receiptTrieProofNodes");
    }
    assert threw : "Ethereum receipt-proof transcript must reject empty receiptTrieProofNodes";
    threw = false;
    try {
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
          new ArrayList<byte[]>());
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("inclusionBranch must not be empty");
    }
    assert threw : "Ethereum receipt-proof transcript must reject empty inclusionBranch";
    threw = false;
    try {
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
          receiptProof.inclusionBranch(),
          EvmSccpProver.DOMAIN_BSC);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceDomain must be ETH");
    }
    assert threw : "Ethereum receipt-proof transcript must reject BSC sourceDomain";
    threw = false;
    try {
      SourceSccpProofs.canonicalBscReceiptProofBytes(
          receiptProof.sourceEventDigest(),
          "21",
          "22",
          receiptProof.executionBlockHash(),
          receiptProof.executionReceiptsRoot(),
          receiptProof.beaconFinalizedRoot(),
          receiptProof.syncCommitteeRoot(),
          receiptProof.receiptRootIndex(),
          receiptProof.receiptTrieProofNodes(),
          new ArrayList<byte[]>());
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("inclusionBranch must not be empty");
    }
    assert threw : "BSC receipt-proof transcript must reject empty inclusionBranch";
    threw = false;
    try {
      SourceSccpProofs.canonicalBscReceiptProofBytes(
          receiptProof.sourceEventDigest(),
          "21",
          "22",
          receiptProof.executionBlockHash(),
          receiptProof.executionReceiptsRoot(),
          receiptProof.beaconFinalizedRoot(),
          receiptProof.syncCommitteeRoot(),
          receiptProof.receiptRootIndex(),
          receiptProof.receiptTrieProofNodes(),
          receiptProof.inclusionBranch(),
          EvmSccpProver.DOMAIN_ETH);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("sourceDomain must be BSC");
    }
    assert threw : "BSC receipt-proof transcript must reject ETH sourceDomain";
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
              assert ("0x" + repeat("dd", 32)).equals(
                      evidence.beaconFinality().get("finalizedHeaderRoot"))
                  : "inbound evidence must carry finalized beacon root";
              assert ("0x" + repeat("aa", 32)).equals(
                      evidence.beaconFinality().get("syncCommitteeRoot"))
                  : "inbound evidence must carry sync committee root";
              assert "32".equals(evidence.beaconFinality().get("beaconSlot"))
                  : "inbound evidence must carry normalized beacon slot";
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
    assert ("0x" + repeat("dd", 32)).equals(evidence.beaconFinality().get("finalizedHeaderRoot"))
        : "inbound evidence must carry finalized beacon root";
    assert ("0x" + repeat("aa", 32)).equals(evidence.beaconFinality().get("syncCommitteeRoot"))
        : "inbound evidence must carry sync committee root";
    assert "32".equals(evidence.beaconFinality().get("beaconSlot"))
        : "inbound evidence must carry normalized beacon slot";
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
    final EthereumMainnetSccp configuredSourceBridgeSdk =
        new EthereumMainnetSccp(
            null, null, null, null, null, null, null, sourceBridgeEmitterAddress);
    final EthereumMainnetSccp.InboundEvidence configuredSourceEventEvidence =
        configuredSourceBridgeSdk.collectInboundEvidenceFromReceipt(
            new EthereumMainnetSccp.InboundEvidence(
                EvmSccpProver.DOMAIN_ETH,
                EvmSccpProver.DOMAIN_SORA,
                null,
                receiptWithSourceEvent,
                block,
                beaconFinality,
                null,
                null,
                null));
    assert sourceEventDigest.equals(configuredSourceEventEvidence.sourceEventDigest())
        : "configured Ethereum source bridge emitter must derive the receipt event digest";
    assert sourceBridgeEmitterAddress.equals(configuredSourceEventEvidence.sourceBridgeEmitterAddress())
        : "configured Ethereum source bridge emitter must be retained on evidence";
    threw = false;
    try {
      configuredSourceBridgeSdk.collectInboundEvidenceFromReceipt(
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
      threw = ex.getMessage().contains("sourceBridgeEmitterAddress");
    }
    assert threw : "Ethereum source-event validation must reject configured/input bridge drift";

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
    final EthereumMainnetSccp.InboundEvidence receiptProofHashOnlyEvidence =
        new EthereumMainnetSccp()
            .collectInboundEvidenceFromReceipt(
                new EthereumMainnetSccp.InboundEvidence(
                    EvmSccpProver.DOMAIN_ETH,
                    EvmSccpProver.DOMAIN_SORA,
                    null,
                    null,
                    null,
                    null,
                    receiptProofHash));
    assert receiptProofHash.equals(receiptProofHashOnlyEvidence.receiptProofHash())
        : "Ethereum inbound collection must accept hash-only receiptProofHash evidence";
    assert receiptProofHashOnlyEvidence.receiptProof() == null
        : "hash-only Ethereum evidence must not synthesize a receiptProof";
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
                  "0x" + repeat("00", 32)));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receiptProofHash must not be zero");
    }
    assert threw : "Ethereum inbound collection must reject zero hash-only receiptProofHash";
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
                  receiptProofHash + " "));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receiptProofHash must be canonical");
    }
    assert threw : "Ethereum inbound collection must reject noncanonical hash-only receiptProofHash";
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

    final Map<String, Object> missingFinalizedRootFinality = linkedMap(
        "syncCommitteeRoot", "0x" + repeat("aa", 32),
        "beaconSlot", "0x20",
        "executionBlockNumber", "0x1234",
        "executionBlockHash", blockHash,
        "executionReceiptsRoot", "0x" + repeat("cc", 32));
    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              null,
              null,
              null,
              missingFinalizedRootEvidence -> {
                missingFinalityProverCalls[0]++;
                return new byte[] {1, 2, 3};
              },
              null)
          .proveInboundToSora(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receiptWithSourceEvent,
                  block,
                  missingFinalizedRootFinality,
                  receiptProof,
                  null,
                  null,
                  sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("beaconFinality.finalizedHeaderRoot");
    }
    assert threw : "Ethereum inbound proving must require finalized beacon root evidence";
    assert missingFinalityProverCalls[0] == 0
        : "Ethereum inbound prover must not run without finalized beacon root evidence";

    final Map<String, Object> missingSyncRootFinality = linkedMap(
        "finalizedHeaderRoot", "0x" + repeat("dd", 32),
        "beaconSlot", "0x20",
        "executionBlockNumber", "0x1234",
        "executionBlockHash", blockHash,
        "executionReceiptsRoot", "0x" + repeat("cc", 32));
    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              null,
              null,
              null,
              missingSyncRootEvidence -> {
                missingFinalityProverCalls[0]++;
                return new byte[] {1, 2, 3};
              },
              null)
          .proveInboundToSora(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receiptWithSourceEvent,
                  block,
                  missingSyncRootFinality,
                  receiptProof,
                  null,
                  null,
                  sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("beaconFinality.syncCommitteeRoot");
    }
    assert threw : "Ethereum inbound proving must require sync committee root evidence";
    assert missingFinalityProverCalls[0] == 0
        : "Ethereum inbound prover must not run without sync committee root evidence";

    final Map<String, Object> missingBeaconSlotFinality = linkedMap(
        "finalizedHeaderRoot", "0x" + repeat("dd", 32),
        "syncCommitteeRoot", "0x" + repeat("aa", 32),
        "executionBlockNumber", "0x1234",
        "executionBlockHash", blockHash,
        "executionReceiptsRoot", "0x" + repeat("cc", 32));
    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              null,
              null,
              null,
              missingBeaconSlotEvidence -> {
                missingFinalityProverCalls[0]++;
                return new byte[] {1, 2, 3};
              },
              null)
          .proveInboundToSora(
              new EthereumMainnetSccp.InboundEvidence(
                  EvmSccpProver.DOMAIN_ETH,
                  EvmSccpProver.DOMAIN_SORA,
                  null,
                  receiptWithSourceEvent,
                  block,
                  missingBeaconSlotFinality,
                  receiptProof,
                  null,
                  null,
                  sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("beaconFinality.beaconSlot");
    }
    assert threw : "Ethereum inbound proving must require finalized beacon slot evidence";
    assert missingFinalityProverCalls[0] == 0
        : "Ethereum inbound prover must not run without finalized beacon slot evidence";

    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              null,
              null,
              null,
              driftedFinalizedRootEvidence -> {
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
                      receiptProof.executionReceiptsRoot(),
                      "0x" + repeat("99", 32),
                      receiptProof.syncCommitteeRoot(),
                      receiptProof.receiptRootIndex(),
                      receiptProof.receiptTrieProofNodes(),
                      receiptProof.inclusionBranch()),
                  null,
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receiptProof.beaconFinalizedRoot");
    }
    assert threw : "Ethereum inbound proving must reject drifted finalized beacon roots";
    assert missingFinalityProverCalls[0] == 0
        : "Ethereum inbound prover must not run with drifted finalized beacon root";

    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              null,
              null,
              null,
              driftedSyncRootEvidence -> {
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
                      receiptProof.executionReceiptsRoot(),
                      receiptProof.beaconFinalizedRoot(),
                      "0x" + repeat("99", 32),
                      receiptProof.receiptRootIndex(),
                      receiptProof.receiptTrieProofNodes(),
                      receiptProof.inclusionBranch()),
                  null,
                  null,
                  null));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receiptProof.syncCommitteeRoot");
    }
    assert threw : "Ethereum inbound proving must reject drifted sync committee roots";
    assert missingFinalityProverCalls[0] == 0
        : "Ethereum inbound prover must not run with drifted sync committee root";

    threw = false;
    try {
      new EthereumMainnetSccp(
              null,
              null,
              null,
              null,
              driftedBeaconSlotEvidence -> {
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
                      "33",
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
      threw = ex.getMessage().contains("receiptProof.beaconSlot");
    }
    assert threw : "Ethereum inbound proving must reject drifted finalized beacon slots";
    assert missingFinalityProverCalls[0] == 0
        : "Ethereum inbound prover must not run with drifted finalized beacon slot";

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
      new EthereumMainnetSccp(null, null, (method, params) -> "0x01", null, null)
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
    assert threw : "Ethereum inbound collection must reject leading-zero eth_chainId RPC";

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

    final Map<String, Object> extraTopicLog = new LinkedHashMap<>(sourceEventLog);
    extraTopicLog.put(
        "topics",
        Arrays.asList(
            EthereumMainnetSccp.sourceEventTopic(), sourceEventDigest, "0x" + repeat("66", 32)));
    final Map<String, Object> extraTopicReceipt = new LinkedHashMap<>(receipt);
    extraTopicReceipt.put("logs", Arrays.asList(extraTopicLog));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              extraTopicReceipt,
              block,
              beaconFinality,
              null,
              null,
              sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("exactly 2 topics");
    }
    assert threw : "Ethereum source-event validation must reject extra source-event topics";

    final Map<String, Object> nonEmptyDataLog = new LinkedHashMap<>(sourceEventLog);
    nonEmptyDataLog.put("data", "0x01");
    final Map<String, Object> nonEmptyDataReceipt = new LinkedHashMap<>(receipt);
    nonEmptyDataReceipt.put("logs", Arrays.asList(nonEmptyDataLog));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              nonEmptyDataReceipt,
              block,
              beaconFinality,
              null,
              null,
              sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("data must be 0x");
    }
    assert threw : "Ethereum source-event validation must reject non-empty source-event data";

    final Map<String, Object> zeroDigestLog = new LinkedHashMap<>(sourceEventLog);
    zeroDigestLog.put(
        "topics",
        Arrays.asList(EthereumMainnetSccp.sourceEventTopic(), "0x" + repeat("00", 32)));
    final Map<String, Object> zeroDigestReceipt = new LinkedHashMap<>(receipt);
    zeroDigestReceipt.put("logs", Arrays.asList(zeroDigestLog));
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              zeroDigestReceipt,
              block,
              beaconFinality,
              null,
              null,
              sourceBridgeEmitterAddress));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("digest must not be zero");
    }
    assert threw : "Ethereum source-event validation must reject zero source-event digest";

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

    for (final String missingField :
        Arrays.asList("transactionHash", "blockHash", "blockNumber")) {
      final Map<String, Object> missingContextLog = new LinkedHashMap<>(sourceEventLog);
      missingContextLog.remove(missingField);
      final Map<String, Object> missingContextReceipt = new LinkedHashMap<>(receipt);
      missingContextReceipt.put("logs", Arrays.asList(missingContextLog));
      threw = false;
      try {
        sdk.collectInboundEvidenceFromReceipt(
            new EthereumMainnetSccp.InboundEvidence(
                EvmSccpProver.DOMAIN_ETH,
                EvmSccpProver.DOMAIN_SORA,
                null,
                missingContextReceipt,
                block,
                beaconFinality,
                null,
                null,
                sourceBridgeEmitterAddress));
      } catch (final IllegalArgumentException ex) {
        threw = ex.getMessage().contains("receipt.logs[0]." + missingField);
      }
      assert threw : "Ethereum source-event validation must reject logs without " + missingField;
    }

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

  private static void ethereumReceiptTrieProofBuilderUsesRlpTransactionIndexKeys() {
    final Map<String, Object> receipt =
        sampleEvmReceipt(0, "0x" + repeat("aa", 32), "0x" + repeat("bb", 32), "0x1234");
    final SourceSccpProofs.EvmReceiptTrieProof proof =
        SourceSccpProofs.buildEvmReceiptTrieProofFromReceipts(Arrays.asList(receipt), "0x0");

    assert "0x80".equals(SourceSccpProofs.evmReceiptTrieKey("0x0"))
        : "receipt trie key for index zero must be raw RLP 0x80";
    assert "0x01".equals(SourceSccpProofs.evmReceiptTrieKey("0x1"))
        : "single-byte RLP keys below 0x80 must encode as the byte itself";
    assert "0x8180".equals(SourceSccpProofs.evmReceiptTrieKey("0x80"))
        : "receipt trie keys must use RLP integer encoding";
    boolean threw = false;
    try {
      SourceSccpProofs.evmReceiptTrieKey("0x01");
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("canonical JSON-RPC quantity");
    }
    assert threw : "receipt trie keys must reject noncanonical JSON-RPC quantities";
    assert "0x80".equals(proof.receiptTrieKey) : "proof must expose the selected RLP key";
    assert proof.receiptRlp.equals("0x" + hexLower(SourceSccpProofs.canonicalEvmReceiptRlp(receipt)))
        : "proof must expose the canonical encoded target receipt";
    assert proof.receiptsRoot.matches("0x[0-9a-f]{64}") : "proof must derive a receiptsRoot";
    assert !proof.receiptTrieProofNodes().isEmpty() : "proof must include MPT nodes";

    final Map<String, Object> zeroTopicReceipt =
        sampleEvmReceipt(1, "0x" + repeat("ab", 32), "0x" + repeat("bb", 32), "0x1234");
    zeroTopicReceipt.put(
        "logs",
        Arrays.asList(
            linkedMap(
                "address", "0x" + repeat("12", 20),
                "topics", Arrays.asList("0x" + repeat("00", 32)),
                "data", "0x")));
    final SourceSccpProofs.EvmReceiptTrieProof zeroTopicProof =
        SourceSccpProofs.buildEvmReceiptTrieProofFromReceipts(
            Arrays.asList(receipt, zeroTopicReceipt), "0x0");
    assert proof.receiptRlp.equals(zeroTopicProof.receiptRlp)
        : "generic Ethereum receipt RLP must allow zero log topics";
    final Map<String, Object> zeroAddressReceipt =
        sampleEvmReceipt(1, "0x" + repeat("ac", 32), "0x" + repeat("bb", 32), "0x1234");
    zeroAddressReceipt.put(
        "logs",
        Arrays.asList(
            linkedMap(
                "address", "0x" + repeat("00", 20),
                "topics", Arrays.asList("0x" + repeat("44", 32)),
                "data", "0x")));
    final SourceSccpProofs.EvmReceiptTrieProof zeroAddressProof =
        SourceSccpProofs.buildEvmReceiptTrieProofFromReceipts(
            Arrays.asList(receipt, zeroAddressReceipt), "0x0");
    assert proof.receiptRlp.equals(zeroAddressProof.receiptRlp)
        : "generic Ethereum receipt RLP must allow zero log addresses";

    final Map<String, Object> wrongIndex = new LinkedHashMap<>(receipt);
    wrongIndex.put("transactionIndex", "0x1");
    threw = false;
    try {
      SourceSccpProofs.buildEvmReceiptTrieProofFromReceipts(Arrays.asList(wrongIndex), "0x0");
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("transactionIndex");
    }
    assert threw : "receipt proof builder must reject out-of-order block receipts";

    final Map<String, Object> duplicateHashReceipt =
        sampleEvmReceipt(1, "0x" + repeat("aa", 32), "0x" + repeat("bb", 32), "0x1234");
    threw = false;
    try {
      SourceSccpProofs.buildEvmReceiptTrieProofFromReceipts(
          Arrays.asList(receipt, duplicateHashReceipt), "0x0");
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("transactionHash values must be unique");
    }
    assert threw : "receipt proof builder must reject duplicate block receipt transaction hashes";

    threw = false;
    try {
      SourceSccpProofs.buildEvmReceiptTrieProofFromReceipts(Arrays.asList(receipt), "0x1");
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("block receipt index");
    }
    assert threw : "receipt proof builder must reject out-of-range target indexes";

    threw = false;
    try {
      SourceSccpProofs.buildEvmReceiptTrieProofFromReceipts(new ArrayList<Map<String, Object>>(), "0x0");
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("non-empty");
    }
    assert threw : "receipt proof builder must reject empty block receipt lists";

    final List<Map<String, Object>> oversizedReceipts = new ArrayList<>();
    for (int index = 0; index < 4_097; index++) {
      oversizedReceipts.add(receipt);
    }
    threw = false;
    try {
      SourceSccpProofs.buildEvmReceiptTrieProofFromReceipts(oversizedReceipts, "0x0");
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("at most");
    }
    assert threw : "receipt proof builder must reject oversized block receipt lists";

    threw = false;
    final Map<String, Object> uppercaseBloomReceipt = new LinkedHashMap<>(receipt);
    uppercaseBloomReceipt.put("logsBloom", "0x" + repeat("AA", 256));
    try {
      SourceSccpProofs.canonicalEvmReceiptRlp(uppercaseBloomReceipt);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("lowercase");
    }
    assert threw : "receipt RLP must reject noncanonical uppercase hex";

    threw = false;
    final Map<String, Object> badType = new LinkedHashMap<>(receipt);
    badType.put("type", "0x80");
    try {
      SourceSccpProofs.canonicalEvmReceiptRlp(badType);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("typed receipt type");
    }
    assert threw : "typed receipt prefixes must fit one byte below 0x80";

    threw = false;
    final Map<String, Object> unsupportedType = new LinkedHashMap<>(receipt);
    unsupportedType.put("type", "0x7f");
    try {
      SourceSccpProofs.canonicalEvmReceiptRlp(unsupportedType);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("not supported");
    }
    assert threw : "unknown typed receipt prefixes must stay unsupported until their payload layout is admitted";

    final Map<String, Object> validReceiptLog =
        linkedMap(
            "address", "0x" + repeat("11", 20),
            "topics", Arrays.asList("0x" + repeat("22", 32)),
            "data", "0x");
    final Map<String, Object> removedLog = new LinkedHashMap<>(validReceiptLog);
    removedLog.put("removed", Boolean.TRUE);
    final Map<String, Object> removedLogReceipt = new LinkedHashMap<>(receipt);
    removedLogReceipt.put("logs", Arrays.asList(removedLog));
    threw = false;
    try {
      SourceSccpProofs.canonicalEvmReceiptRlp(removedLogReceipt);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("removed");
    }
    assert threw : "receipt RLP must reject removed logs";

    final Map<String, Object> tooManyTopicsLog = new LinkedHashMap<>(validReceiptLog);
    tooManyTopicsLog.put(
        "topics",
        Arrays.asList(
            "0x" + repeat("22", 32),
            "0x" + repeat("22", 32),
            "0x" + repeat("22", 32),
            "0x" + repeat("22", 32),
            "0x" + repeat("22", 32)));
    final Map<String, Object> tooManyTopicsReceipt = new LinkedHashMap<>(receipt);
    tooManyTopicsReceipt.put("logs", Arrays.asList(tooManyTopicsLog));
    threw = false;
    try {
      SourceSccpProofs.canonicalEvmReceiptRlp(tooManyTopicsReceipt);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("topics");
    }
    assert threw : "receipt RLP must reject logs with too many topics";
  }

  private static void ethereumInboundCollectionBuildsReceiptProofFromBlockReceipts() {
    final String txHash = "0x" + repeat("aa", 32);
    final String otherTxHash = "0x" + repeat("ab", 32);
    final String blockHash = "0x" + repeat("bb", 32);
    final String sourceBridgeEmitterAddress = "0x" + repeat("12", 20);
    final String sourceEventDigest = "0x" + repeat("ee", 32);
    final Map<String, Object> sourceEventLog =
        linkedMap(
            "address", sourceBridgeEmitterAddress,
            "transactionHash", txHash,
            "blockHash", blockHash,
            "blockNumber", "0x1234",
            "topics", Arrays.asList(EthereumMainnetSccp.sourceEventTopic(), sourceEventDigest),
            "data", "0x");
    final Map<String, Object> receipt =
        sampleEvmReceipt(0, txHash, blockHash, "0x1234", Arrays.asList(sourceEventLog));
    final Map<String, Object> otherReceipt =
        sampleEvmReceipt(1, otherTxHash, blockHash, "0x1234");
    final List<Map<String, Object>> blockReceipts = Arrays.asList(receipt, otherReceipt);
    final SourceSccpProofs.EvmReceiptTrieProof trieProof =
        SourceSccpProofs.buildEvmReceiptTrieProofFromReceipts(blockReceipts, "0x0");
    final Map<String, Object> block =
        linkedMap("hash", blockHash, "number", "0x1234", "receiptsRoot", trieProof.receiptsRoot);
    final Map<String, Object> beaconFinality =
        linkedMap(
            "executionBlockNumber", "0x1234",
            "executionBlockHash", blockHash,
            "executionReceiptsRoot", trieProof.receiptsRoot,
            "finalizedHeaderRoot", "0x" + repeat("dd", 32),
            "syncCommitteeRoot", "0x" + repeat("cc", 32),
            "beaconSlot", "0x20");
    final List<byte[]> inclusionBranch = Arrays.asList(repeatedWord(0x44));
    final List<String> calls = new ArrayList<>();
    final EthereumMainnetSccp sdk =
        new EthereumMainnetSccp(
            null,
            null,
            (method, params) -> {
              calls.add(method);
              if ("eth_chainId".equals(method)) {
                return "0x1";
              }
              if ("eth_getBlockReceipts".equals(method)) {
                assert params.equals(Arrays.<Object>asList("0x1234"))
                    : "block receipt fetch must use the receipt block number";
                return blockReceipts;
              }
              throw new IllegalArgumentException("unexpected method " + method);
            },
            null,
            null);

    final EthereumMainnetSccp.InboundEvidence evidence =
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
                null,
                sourceBridgeEmitterAddress,
                null,
                inclusionBranch));

    assert calls.equals(Arrays.asList("eth_chainId", "eth_getBlockReceipts"))
        : "collection must validate mainnet and fetch block receipts";
    assert sourceEventDigest.equals(evidence.sourceEventDigest())
        : "collection must validate the SCCP source event";
    assert evidence.blockReceipts().equals(blockReceipts)
        : "collection must retain block receipt evidence";
    final EthereumMainnetSccp.ReceiptProof receiptProof = evidence.receiptProof();
    assert receiptProof != null : "collection must auto-build receiptProof";
    assert receiptProof.sourceDomain() == EvmSccpProver.DOMAIN_ETH
        : "auto-built receiptProof must stay on ETH";
    assert "0".equals(receiptProof.receiptRootIndex())
        : "receiptRootIndex must be the transaction index";
    assert "32".equals(receiptProof.beaconSlot()) : "beacon slot must be normalized decimal";
    assert "4660".equals(receiptProof.executionBlockNumber())
        : "execution block number must be normalized decimal";
    assert trieProof.receiptsRoot.equals(receiptProof.executionReceiptsRoot())
        : "receipt proof must bind the computed receipt root";
    assert Arrays.equals(
        trieProof.receiptTrieProofNodes().get(0), receiptProof.receiptTrieProofNodes().get(0))
        : "receipt proof must carry generated MPT nodes";
    assert Arrays.equals(inclusionBranch.get(0), receiptProof.inclusionBranch().get(0))
        : "receipt proof must carry the app-supplied SSZ inclusion branch";
    final String expectedHash =
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
    assert expectedHash.equals(evidence.receiptProofHash())
        : "collection must derive receiptProofHash from the generated receiptProof";

    boolean threw = false;
    final Map<String, Object> wrongRootBlock = new LinkedHashMap<>(block);
    wrongRootBlock.put("receiptsRoot", "0x" + repeat("99", 32));
    final Map<String, Object> wrongRootFinality = new LinkedHashMap<>(beaconFinality);
    wrongRootFinality.put("executionReceiptsRoot", "0x" + repeat("99", 32));
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              wrongRootBlock,
              wrongRootFinality,
              null,
              null,
              null,
              sourceBridgeEmitterAddress,
              blockReceipts,
              inclusionBranch));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("computed receipt trie root");
    }
    assert threw : "collection must reject receipt roots that do not match reconstructed block receipts";

    final Map<String, Object> mismatchedIndexedReceipt = new LinkedHashMap<>(receipt);
    mismatchedIndexedReceipt.put("logs", new ArrayList<Map<String, Object>>());
    final List<Map<String, Object>> mismatchedBlockReceipts =
        Arrays.asList(mismatchedIndexedReceipt, otherReceipt);
    final SourceSccpProofs.EvmReceiptTrieProof mismatchedReceiptProof =
        SourceSccpProofs.buildEvmReceiptTrieProofFromReceipts(mismatchedBlockReceipts, "0x0");
    final Map<String, Object> mismatchedBlock = new LinkedHashMap<>(block);
    mismatchedBlock.put("receiptsRoot", mismatchedReceiptProof.receiptsRoot);
    final Map<String, Object> mismatchedFinality = new LinkedHashMap<>(beaconFinality);
    mismatchedFinality.put("executionReceiptsRoot", mismatchedReceiptProof.receiptsRoot);
    threw = false;
    try {
      sdk.collectInboundEvidenceFromReceipt(
          new EthereumMainnetSccp.InboundEvidence(
              EvmSccpProver.DOMAIN_ETH,
              EvmSccpProver.DOMAIN_SORA,
              null,
              receipt,
              mismatchedBlock,
              mismatchedFinality,
              null,
              null,
              null,
              sourceBridgeEmitterAddress,
              mismatchedBlockReceipts,
              inclusionBranch));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("receipt RLP");
    }
    assert threw : "collection must reject block receipts whose RLP differs from the source receipt";

    final Map<String, Object> blockHashDriftReceipt = new LinkedHashMap<>(receipt);
    blockHashDriftReceipt.put("blockHash", "0x" + repeat("99", 32));
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
              null,
              sourceBridgeEmitterAddress,
              Arrays.asList(blockHashDriftReceipt, otherReceipt),
              inclusionBranch));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("blockHash");
    }
    assert threw : "collection must reject block receipt blockHash drift";

    final Map<String, Object> blockNumberDriftReceipt = new LinkedHashMap<>(receipt);
    blockNumberDriftReceipt.put("blockNumber", "0x1235");
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
              null,
              sourceBridgeEmitterAddress,
              Arrays.asList(blockNumberDriftReceipt, otherReceipt),
              inclusionBranch));
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("blockNumber");
    }
    assert threw : "collection must reject block receipt blockNumber drift";
  }

  private static void ethereumMainnetBeaconRestConsensusProviderCollectsFinalizedEvidence() {
    final String txHash = "0x" + repeat("aa", 32);
    final String blockHash = "0x" + repeat("bb", 32);
    final Map<String, Object> receipt =
        linkedMap(
            "transactionHash", txHash,
            "blockHash", blockHash,
            "blockNumber", "0x1234",
            "status", "0x1");
    final Map<String, Object> block =
        linkedMap(
            "hash", blockHash,
            "number", "0x1234",
            "receiptsRoot", "0x" + repeat("cc", 32));
    final List<String> calls = new ArrayList<>();
    final List<Map<String, String>> headerCalls = new ArrayList<>();
    final EthereumMainnetSccp.BeaconRestTransport transport =
        (url, headers) -> {
          calls.add(url);
          headerCalls.add(headers);
          if ("https://beacon.example/eth/v1/beacon/headers/finalized".equals(url)) {
            return beaconResponse(beaconHeaderJson(false, true));
          }
          if ("https://beacon.example/eth/v1/beacon/blocks/finalized/root".equals(url)) {
            return beaconResponse(beaconBlockRootJson("dd"));
          }
          if ("https://beacon.example/eth/v2/beacon/blocks/finalized".equals(url)) {
            return beaconResponse(beaconBlockJson("64", blockHash, "4660", "0x" + repeat("cc", 32)));
          }
          if ("https://beacon.example/eth/v1/beacon/states/finalized/finality_checkpoints"
              .equals(url)) {
            return beaconResponse(beaconCheckpointJson("dd"));
          }
          throw new IllegalArgumentException("unexpected Beacon REST URL " + url);
        };
    final EthereumMainnetSccp.BeaconRestConsensusProvider provider =
        new EthereumMainnetSccp.BeaconRestConsensusProvider(
            "https://beacon.example/eth/v1",
            "0x" + repeat("ee", 32),
            null,
            linkedStringMap("Authorization", "Bearer local"),
            true,
            transport);
    final EthereumMainnetSccp.InboundEvidence evidence =
        new EthereumMainnetSccp(null, null, null, provider, null, null)
            .collectInboundEvidenceFromReceipt(
                new EthereumMainnetSccp.InboundEvidence(
                    EthereumMainnetSccp.DOMAIN_ETH,
                    EthereumMainnetSccp.DOMAIN_SORA,
                    null,
                    receipt,
                    block,
                    null,
                    null));

    assert "4660".equals(evidence.beaconFinality().get("executionBlockNumber"));
    assert blockHash.equals(evidence.beaconFinality().get("executionBlockHash"));
    assert ("0x" + repeat("cc", 32)).equals(evidence.beaconFinality().get("executionReceiptsRoot"));
    assert ("0x" + repeat("dd", 32)).equals(evidence.beaconFinality().get("finalizedHeaderRoot"));
    assert ("0x" + repeat("ee", 32)).equals(evidence.beaconFinality().get("syncCommitteeRoot"));
    assert "64".equals(evidence.beaconFinality().get("beaconSlot"));
    assert calls.equals(
        Arrays.asList(
            "https://beacon.example/eth/v1/beacon/headers/finalized",
            "https://beacon.example/eth/v1/beacon/blocks/finalized/root",
            "https://beacon.example/eth/v2/beacon/blocks/finalized",
            "https://beacon.example/eth/v1/beacon/states/finalized/finality_checkpoints"));
    assert "Bearer local".equals(headerCalls.get(0).get("Authorization"));
  }

  private static void ethereumMainnetBeaconRestHttpTransportRejectsOversizedBodies()
      throws Exception {
    final HttpServer server = HttpServer.create(new InetSocketAddress("127.0.0.1", 0), 0);
    server.createContext(
        "/oversized",
        exchange -> {
          final byte[] body = new byte[1024 * 1024 + 1];
          Arrays.fill(body, (byte) 0x7b);
          exchange.sendResponseHeaders(200, body.length);
          try (OutputStream output = exchange.getResponseBody()) {
            output.write(body);
          }
        });
    server.start();
    try {
      boolean threw = false;
      try {
        new EthereumMainnetSccp.BeaconRestHttpTransport()
            .get(
                "http://127.0.0.1:" + server.getAddress().getPort() + "/oversized",
                java.util.Collections.emptyMap());
      } catch (final IllegalArgumentException ex) {
        threw = ex.getMessage().contains("response body must be at most");
      }
      assert threw : "Beacon REST HTTP transport must reject oversized response bodies";
    } finally {
      server.stop(0);
    }
  }

  private static void ethereumMainnetBeaconRestConsensusProviderRejectsUnsafeFinality() {
    final Map<String, Object> block =
        linkedMap(
            "hash", "0x" + repeat("bb", 32),
            "number", "0x1234",
            "receiptsRoot", "0x" + repeat("cc", 32));

    boolean threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(false, true)),
              beaconResponse(beaconCheckpointJson("dd")),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, null, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("requires block");
    }
    assert threw : "Beacon REST provider must require an execution block";

    threw = false;
    try {
      beaconRestProvider(
              new EthereumMainnetSccp.BeaconRestResponse(
                  503, "{}".getBytes(StandardCharsets.UTF_8), "Unavailable"),
              beaconResponse(beaconCheckpointJson("dd")),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("request failed 503 Unavailable");
    }
    assert threw : "Beacon REST provider must reject non-2xx header responses";

    threw = false;
    try {
      beaconRestProvider(
              new EthereumMainnetSccp.BeaconRestResponse(200, new byte[1024 * 1024 + 1]),
              beaconResponse(beaconCheckpointJson("dd")),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("response body must be at most");
    }
    assert threw : "Beacon REST provider must reject oversized header responses";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(true, true)),
              beaconResponse(beaconCheckpointJson("dd")),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("must not be execution optimistic");
    }
    assert threw : "Beacon REST provider must reject optimistic headers";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(
                  beaconHeaderJson(false, true)
                      .replace("\"execution_optimistic\":false", "\"execution_optimistic\":\"false\"")),
              beaconResponse(beaconCheckpointJson("dd")),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("execution_optimistic must be a boolean");
    }
    assert threw : "Beacon REST provider must reject malformed optimistic flags";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(
                  beaconHeaderJson(false, true).replace("\"finalized\":true", "\"finalized\":\"true\"")),
              beaconResponse(beaconCheckpointJson("dd")),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("finalized must be a boolean");
    }
    assert threw : "Beacon REST provider must reject malformed finalized flags";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(
                  beaconHeaderJson(false, true).replace("\"canonical\":true", "\"canonical\":\"true\"")),
              beaconResponse(beaconCheckpointJson("dd")),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("canonical must be a boolean");
    }
    assert threw : "Beacon REST provider must reject malformed canonical flags";

    for (final String[] rootCase :
        new String[][] {
          {"parent_root", "01"},
          {"state_root", "02"},
          {"body_root", "03"},
        }) {
      threw = false;
      try {
        beaconRestProvider(
                beaconResponse(
                    beaconHeaderJson(false, true)
                        .replace(
                            "\"" + rootCase[0] + "\":\"0x" + repeat(rootCase[1], 32) + "\"",
                            "\"" + rootCase[0] + "\":\"0x\"")),
                beaconResponse(beaconCheckpointJson("dd")),
                "0x" + repeat("ee", 32),
                null)
            .collectFinalityEvidence(null, block, null);
      } catch (final IllegalArgumentException ex) {
        threw = ex.getMessage().contains(rootCase[0]);
      }
      assert threw : "Beacon REST provider must reject malformed " + rootCase[0];
    }

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(
                  beaconHeaderJson(false, true)
                      .replace(
                          "\"signature\":\"0x" + repeat("12", 96) + "\"",
                          "\"signature\":\"0x" + repeat("12", 95) + "\"")),
              beaconResponse(beaconCheckpointJson("dd")),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("signature");
    }
    assert threw : "Beacon REST provider must reject malformed finalized header signatures";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(false, true)),
              beaconResponse(beaconBlockRootJson("99")),
              beaconResponse(beaconBlockJson("64", "0x" + repeat("bb", 32), "4660", "0x" + repeat("cc", 32))),
              beaconResponse(beaconCheckpointJson("dd")),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("finalized block root must match finalized header root");
    }
    assert threw : "Beacon REST provider must reject finalized block root/header drift";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(false, true)),
              beaconResponse(beaconBlockJson("65", "0x" + repeat("bb", 32), "4660", "0x" + repeat("cc", 32))),
              beaconResponse(beaconCheckpointJson("dd")),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("finalized block slot must match finalized header slot");
    }
    assert threw : "Beacon REST provider must reject finalized block slot drift";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(false, true)),
              beaconResponse(beaconBlockJson("64", "0x" + repeat("99", 32), "4660", "0x" + repeat("cc", 32))),
              beaconResponse(beaconCheckpointJson("dd")),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("execution payload block_hash must match block.hash");
    }
    assert threw : "Beacon REST provider must reject execution payload block-hash drift";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(false, true)),
              beaconResponse(beaconBlockJson("64", "0x" + repeat("bb", 32), "4661", "0x" + repeat("cc", 32))),
              beaconResponse(beaconCheckpointJson("dd")),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("execution payload block_number must match block.number");
    }
    assert threw : "Beacon REST provider must reject execution payload block-number drift";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(false, true)),
              beaconResponse(beaconBlockJson("64", "0x" + repeat("bb", 32), "4660", "0x" + repeat("99", 32))),
              beaconResponse(beaconCheckpointJson("dd")),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("execution payload receipts_root must match block.receiptsRoot");
    }
    assert threw : "Beacon REST provider must reject execution payload receipts-root drift";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(false, false)),
              beaconResponse(beaconCheckpointJson("dd")),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("must be finalized");
    }
    assert threw : "Beacon REST provider must reject unfinalized headers";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(false, true)),
              beaconResponse(beaconCheckpointJson("99")),
              "0x" + repeat("ee", 32),
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("checkpoint root must match");
    }
    assert threw : "Beacon REST provider must reject checkpoint/header mismatches";

    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(false, true)),
              beaconResponse(beaconCheckpointJson("dd")),
              null,
              null)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("requires syncCommitteeRoot or syncCommitteePayload");
    }
    assert threw : "Beacon REST provider must require local sync committee material";

    final byte[] publicKey = new byte[48];
    Arrays.fill(publicKey, (byte) 0x11);
    final byte[] pop = new byte[96];
    Arrays.fill(pop, (byte) 0x22);
    final byte[] syncCommitteePayload =
        SourceSccpProofs.canonicalEthSyncCommitteePayloadBytes(
            Arrays.asList(publicKey), Arrays.asList("1"), Arrays.asList(pop));
    threw = false;
    try {
      beaconRestProvider(
              beaconResponse(beaconHeaderJson(false, true)),
              beaconResponse(beaconCheckpointJson("dd")),
              "0x" + repeat("ee", 32),
              syncCommitteePayload)
          .collectFinalityEvidence(null, block, null);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage().contains("syncCommitteeRoot must match syncCommitteePayload");
    }
    assert threw : "Beacon REST provider must reject sync committee root/payload mismatches";
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

  private static Map<String, Object> sampleEvmReceipt(
      final int transactionIndex,
      final String transactionHash,
      final String blockHash,
      final String blockNumber) {
    return sampleEvmReceipt(
        transactionIndex, transactionHash, blockHash, blockNumber, java.util.Collections.emptyList());
  }

  private static Map<String, Object> sampleEvmReceipt(
      final int transactionIndex,
      final String transactionHash,
      final String blockHash,
      final String blockNumber,
      final List<Map<String, Object>> logs) {
    return linkedMap(
        "transactionHash", transactionHash,
        "transactionIndex", "0x" + Integer.toString(transactionIndex, 16),
        "blockHash", blockHash,
        "blockNumber", blockNumber,
        "status", "0x1",
        "cumulativeGasUsed", "0x" + Long.toString(21_000L * (transactionIndex + 1L), 16),
        "logsBloom", "0x" + repeat("00", 256),
        "logs", logs);
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

  private static EvmSccpProver.ProofRequest evmRequestWithDestinationBindingHash(
      final EvmSccpProver.ProofRequest request, final String destinationBindingHash) {
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
        destinationBindingHash,
        request.requestHash(),
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

  private static EthereumMainnetSccp.BeaconRestResponse beaconResponse(final String json) {
    return new EthereumMainnetSccp.BeaconRestResponse(200, json.getBytes(StandardCharsets.UTF_8));
  }

  private static EthereumMainnetSccp.BeaconRestConsensusProvider beaconRestProvider(
      final EthereumMainnetSccp.BeaconRestResponse header,
      final EthereumMainnetSccp.BeaconRestResponse checkpoint,
      final String syncCommitteeRoot,
      final byte[] syncCommitteePayload) {
    return beaconRestProvider(
        header,
        beaconResponse(beaconBlockRootJson("dd")),
        beaconResponse(beaconBlockJson("64", "0x" + repeat("bb", 32), "4660", "0x" + repeat("cc", 32))),
        checkpoint,
        syncCommitteeRoot,
        syncCommitteePayload);
  }

  private static EthereumMainnetSccp.BeaconRestConsensusProvider beaconRestProvider(
      final EthereumMainnetSccp.BeaconRestResponse header,
      final EthereumMainnetSccp.BeaconRestResponse finalizedBlockRoot,
      final EthereumMainnetSccp.BeaconRestResponse finalizedBlock,
      final EthereumMainnetSccp.BeaconRestResponse checkpoint,
      final String syncCommitteeRoot,
      final byte[] syncCommitteePayload) {
    return new EthereumMainnetSccp.BeaconRestConsensusProvider(
        "https://beacon.example",
        syncCommitteeRoot,
        syncCommitteePayload,
        java.util.Collections.emptyMap(),
        true,
        (url, headers) -> {
          if (url.endsWith("/eth/v1/beacon/headers/finalized")) {
            return header;
          }
          if (url.endsWith("/eth/v1/beacon/blocks/finalized/root")) {
            return finalizedBlockRoot;
          }
          if (url.endsWith("/eth/v2/beacon/blocks/finalized")) {
            return finalizedBlock;
          }
          if (url.endsWith("/eth/v1/beacon/states/finalized/finality_checkpoints")) {
            return checkpoint;
          }
          throw new IllegalArgumentException("unexpected Beacon REST URL " + url);
        });
  }

  private static EthereumMainnetSccp.BeaconRestConsensusProvider beaconRestProvider(
      final EthereumMainnetSccp.BeaconRestResponse header,
      final EthereumMainnetSccp.BeaconRestResponse finalizedBlock,
      final EthereumMainnetSccp.BeaconRestResponse checkpoint,
      final String syncCommitteeRoot,
      final byte[] syncCommitteePayload) {
    return beaconRestProvider(
        header,
        beaconResponse(beaconBlockRootJson("dd")),
        finalizedBlock,
        checkpoint,
        syncCommitteeRoot,
        syncCommitteePayload);
  }

  private static String beaconHeaderJson(
      final boolean executionOptimistic, final boolean finalized) {
    return "{"
        + "\"execution_optimistic\":"
        + executionOptimistic
        + ",\"finalized\":"
        + finalized
        + ",\"data\":{"
        + "\"root\":\"0x"
        + repeat("dd", 32)
        + "\",\"canonical\":true,"
        + "\"header\":{\"message\":{"
        + "\"slot\":\"64\","
        + "\"proposer_index\":\"1\","
        + "\"parent_root\":\"0x"
        + repeat("01", 32)
        + "\",\"state_root\":\"0x"
        + repeat("02", 32)
        + "\",\"body_root\":\"0x"
        + repeat("03", 32)
        + "\"},\"signature\":\"0x"
        + repeat("12", 96)
        + "\"}}}";
  }

  private static String beaconCheckpointJson(final String rootByte) {
    return "{"
        + "\"execution_optimistic\":false,"
        + "\"finalized\":true,"
        + "\"data\":{\"finalized\":{\"root\":\"0x"
        + repeat(rootByte, 32)
        + "\",\"epoch\":\"2\"}}}";
  }

  private static String beaconBlockRootJson(final String rootByte) {
    return "{"
        + "\"execution_optimistic\":false,"
        + "\"finalized\":true,"
        + "\"data\":{\"root\":\"0x"
        + repeat(rootByte, 32)
        + "\"}}";
  }

  private static String beaconBlockJson(
      final String slot,
      final String blockHash,
      final String blockNumber,
      final String receiptsRoot) {
    return "{"
        + "\"execution_optimistic\":false,"
        + "\"finalized\":true,"
        + "\"data\":{\"message\":{"
        + "\"slot\":\""
        + slot
        + "\",\"body\":{\"execution_payload\":{"
        + "\"block_hash\":\""
        + blockHash
        + "\",\"block_number\":\""
        + blockNumber
        + "\",\"receipts_root\":\""
        + receiptsRoot
        + "\"}}}}}";
  }

  private static Map<String, String> linkedStringMap(final String key, final String value) {
    final LinkedHashMap<String, String> out = new LinkedHashMap<>();
    out.put(key, value);
    return out;
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
