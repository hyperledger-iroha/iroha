package org.hyperledger.iroha.android.sccp;

import java.util.Arrays;
import java.util.Objects;

/** BSC mainnet SCCP Groth16 proof request helpers for local-first Android proof generation. */
public final class BscSccpProver {
  public static final int DOMAIN_SORA = EvmSccpProver.DOMAIN_SORA;
  public static final int DOMAIN_BSC = EvmSccpProver.DOMAIN_BSC;
  public static final long MAINNET_CHAIN_ID = SourceSccpProofs.BSC_MAINNET_CHAIN_ID;
  public static final String MAINNET_NETWORK_ID = SourceSccpProofs.BSC_MAINNET_NETWORK_ID;

  private final EvmSccpProver.WitnessProvider witnessProvider;
  private final EvmSccpProver.ProofEngine proofEngine;

  public BscSccpProver() {
    this(null, null);
  }

  public BscSccpProver(
      final EvmSccpProver.WitnessProvider witnessProvider,
      final EvmSccpProver.ProofEngine proofEngine) {
    this.witnessProvider = witnessProvider;
    this.proofEngine = proofEngine;
  }

  public static SourceSccpProofs.EvmDestinationBinding destinationBinding(
      final String verifierAddress,
      final String bridgeAddress,
      final String verifierCodeHash,
      final String verifierKeyHash) {
    return SourceSccpProofs.bscMainnetDestinationBinding(
        verifierAddress, bridgeAddress, verifierCodeHash, verifierKeyHash);
  }

  public static SourceSccpProofs.EvmDestinationBinding destinationBinding(
      final String verifierAddress,
      final String bridgeAddress,
      final String verifierCodeHash,
      final String verifierKeyHash,
      final String networkId) {
    return SourceSccpProofs.bscMainnetDestinationBinding(
        verifierAddress, bridgeAddress, verifierCodeHash, verifierKeyHash, networkId);
  }

  public static String destinationBindingHash(
      final String verifierAddress,
      final String bridgeAddress,
      final String verifierCodeHash,
      final String verifierKeyHash) {
    return destinationBinding(verifierAddress, bridgeAddress, verifierCodeHash, verifierKeyHash)
        .hash;
  }

  public static EvmSccpProver.ProofRequest buildProofRequest(
      final EvmSccpProver.ProofRequestInput input) {
    requireBscRequestInput(input);
    final EvmSccpProver.ProofRequest request = EvmSccpProver.buildProofRequest(input);
    requireBscProofRequest(request);
    return request;
  }

  public static EvmSccpProver.ProofResult wrapProofResult(
      final byte[] proofBytes, final EvmSccpProver.ProofRequest request) {
    requireBscProofRequest(request);
    return EvmSccpProver.wrapProofResult(proofBytes, request);
  }

  public static EvmSccpProver.Submission buildSubmission(
      final EvmSccpProver.SubmissionInput input) {
    Objects.requireNonNull(input, "input");
    if (input.publicInputs().targetDomain() != DOMAIN_BSC) {
      throw new IllegalArgumentException("BSC mainnet submissions must target BSC");
    }
    final EvmSccpProver.ProofResult proofResult = input.proofResult();
    if (proofResult == null) {
      throw new IllegalArgumentException(
          "BSC mainnet submissions require a wrapped proofResult with destinationBinding");
    }
    if (proofResult.publicInputs().targetDomain() != DOMAIN_BSC) {
      throw new IllegalArgumentException("BSC mainnet proofResult must target BSC");
    }
    final SourceSccpProofs.EvmDestinationBinding destinationBinding =
        requireBscDestinationBinding(proofResult.destinationBinding());
    if (!destinationBinding.hash.equals(proofResult.destinationBindingHash())) {
      throw new IllegalArgumentException(
          "BSC mainnet proofResult destinationBindingHash must match destinationBinding");
    }
    return EvmSccpProver.buildSubmission(input);
  }

  public EvmSccpProver.ProofRequest buildRequest(final EvmSccpProver.ProofRequestInput input) {
    final EvmSccpProver.ProofRequestInput resolved =
        witnessProvider == null ? input : witnessProvider.resolveWitness(inputSnapshot(input));
    return buildProofRequest(resolved);
  }

  public EvmSccpProver.ProofResult prove(final EvmSccpProver.ProofRequestInput input) {
    final EvmSccpProver.ProofRequest request = buildRequest(input);
    if (proofEngine == null) {
      throw new IllegalStateException("BSC mainnet SCCP Groth16 prover is not linked");
    }
    return wrapProofResult(proofEngine.prove(EvmSccpProver.callbackRequestSnapshot(request)), request);
  }

  private static void requireBscRequestInput(final EvmSccpProver.ProofRequestInput input) {
    Objects.requireNonNull(input, "input");
    if (input.publicInputs().targetDomain() != DOMAIN_BSC) {
      throw new IllegalArgumentException("BSC mainnet proof requests must target BSC");
    }
    final SourceSccpProofs.EvmDestinationBinding binding =
        requireBscDestinationBinding(input.destinationBinding());
    if (!binding.hash.equals(input.destinationBindingHash())) {
      throw new IllegalArgumentException(
          "destinationBindingHash must match BSC mainnet destinationBinding");
    }
  }

  private static void requireBscProofRequest(final EvmSccpProver.ProofRequest request) {
    Objects.requireNonNull(request, "request");
    if (request.targetDomain() != DOMAIN_BSC || request.publicInputs().targetDomain() != DOMAIN_BSC) {
      throw new IllegalArgumentException("BSC mainnet proof requests must target BSC");
    }
    final SourceSccpProofs.EvmDestinationBinding binding =
        requireBscDestinationBinding(request.destinationBinding());
    if (!binding.hash.equals(request.destinationBindingHash())) {
      throw new IllegalArgumentException(
          "destinationBindingHash must match BSC mainnet destinationBinding");
    }
  }

  private static SourceSccpProofs.EvmDestinationBinding requireBscDestinationBinding(
      final SourceSccpProofs.EvmDestinationBinding destinationBinding) {
    final SourceSccpProofs.EvmDestinationBinding binding =
        Objects.requireNonNull(destinationBinding, "destinationBinding");
    if (binding.targetDomain != DOMAIN_BSC) {
      throw new IllegalArgumentException("BSC mainnet destinationBinding must target BSC");
    }
    if (!MAINNET_NETWORK_ID.equals(binding.networkId)) {
      throw new IllegalArgumentException(
          "BSC mainnet destinationBinding.networkId must be chain id 56");
    }
    return binding;
  }

  private static EvmSccpProver.ProofRequestInput inputSnapshot(
      final EvmSccpProver.ProofRequestInput input) {
    final byte[] bundleBytes = Objects.requireNonNull(input.bundleBytes(), "bundleBytes");
    final byte[] sourceProofBytes =
        Objects.requireNonNull(input.sourceProofBytes(), "sourceProofBytes");
    return new EvmSccpProver.ProofRequestInput(
        input.publicInputs(),
        Arrays.copyOf(bundleBytes, bundleBytes.length),
        Arrays.copyOf(sourceProofBytes, sourceProofBytes.length),
        input.statementHash(),
        input.destinationBindingHash(),
        input.backend(),
        input.sourceDomain(),
        input.destinationBinding());
  }
}
