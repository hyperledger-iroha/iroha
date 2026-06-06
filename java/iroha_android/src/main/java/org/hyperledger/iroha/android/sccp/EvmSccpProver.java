package org.hyperledger.iroha.android.sccp;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import org.bouncycastle.crypto.digests.KeccakDigest;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.crypto.Blake2b;

/** EVM-family SCCP Groth16 proof request helpers for local-first Android proof generation. */
public final class EvmSccpProver {
  public static final int DOMAIN_SORA = SourceSccpProofs.DOMAIN_SORA;
  public static final int DOMAIN_ETH = SourceSccpProofs.DOMAIN_ETH;
  public static final int DOMAIN_BSC = SourceSccpProofs.DOMAIN_BSC;
  public static final String GROTH16_BN254_PROOF_BACKEND_V1 = "evm-groth16-bn254-v1";
  public static final String NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1 =
      "sccp-native-evm-groth16-prover-bundle-v1";
  public static final String ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1 =
      "sccp-ethereum-mainnet-native-evm-cross-sdk-fixture-parity-v1";
  public static final String ETH_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1 =
      "sccp-ethereum-mainnet-native-evm-prover-self-test-v1";
  public static final String ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1 =
      "sccp:eth:native-evm-groth16-prover:ethereum-mainnet:v1";
  public static final Map<String, String> ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1 =
      Collections.unmodifiableMap(
          new LinkedHashMap<String, String>() {
            {
              put("javascript", "pure-typescript");
              put("swift", "native-swift");
              put("kotlin", "native-kotlin");
              put("java-android", "native-java");
              put("dotnet", "native-csharp");
            }
          });
  public static final List<String> ETH_NATIVE_EVM_PROVER_REQUIRED_AUDIT_HASHES_V1 =
      Collections.unmodifiableList(
          Arrays.asList(
              "circuit_security_audit",
              "native_implementation_audit",
              "reproducible_build_attestation",
              "cross_sdk_fixture_parity",
              "native_prover_self_test",
              "no_wasm_no_remote_scan"));
  public static final String NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1 = "sha256";
  private static final int NATIVE_EVM_PROVER_MIN_ARTIFACT_BYTES_V1 = 256;
  public static final int GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1 = 384;
  public static final int SOURCE_STATE_MAX_PROOF_BYTES = 2 * 1024 * 1024;
  public static final String CONTRACT_CALL_ABI_TUPLE_V1 = "abi_tuple_v1";
  public static final String SUBMIT_MESSAGE_PROOF_ABI_V1 =
      "submitSccpMessageProof(bytes,bytes32[6],bytes32)";
  public static final String SUBMIT_MESSAGE_PROOF_SELECTOR_V1 = "0xbd57826c";

  private static final String PROOF_REQUEST_PREFIX_V1 = "sccp:evm:groth16-proof-request:v1";
  private static final String PROOF_ENVELOPE_PREFIX_V1 = "sccp:evm:groth16-proof-envelope:v1";
  private static final String STARK_FRI_PROOF_FAMILY_V1 = "stark-fri-v1";
  private static final String SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1 =
      "submitSccpMessageProof(bytes proof_bytes, bytes32[6] public_inputs, bytes32 statement_hash)";
  private static final byte[] SUBMIT_MESSAGE_PROOF_SELECTOR_BYTES_V1 =
      new byte[] {(byte) 0xbd, 0x57, (byte) 0x82, 0x6c};
  private static final BigInteger MAX_U64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);
  private static final BigInteger BN254_BASE_FIELD_MODULUS =
      new BigInteger(
          "30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47", 16);
  private static final BigInteger BN254_SCALAR_FIELD_MODULUS =
      new BigInteger(
          "30644e72e131a029b85045b68181585d2833e84879b9709143e1f593f0000001", 16);
  private static final BigInteger BN254_G2_B_C0 =
      new BigInteger(
          "2b149d40ceb8aaae81be18991be06ac3b5b4c5e559dbefa33267e6dc24a138e5", 16);
  private static final BigInteger BN254_G2_B_C1 =
      new BigInteger(
          "009713b03af0fed4cd2cafadeed8fdf4a74fa084e52d1852e4a2bd0685c315d2", 16);
  private static final Set<String> NATIVE_EVM_PROVER_BUNDLE_MANIFEST_KEYS =
      Collections.unmodifiableSet(
          new LinkedHashSet<>(
              Arrays.asList(
                  "schema",
                  "bundleId",
                  "bundle_id",
                  "domain",
                  "chain",
                  "proofBackend",
                  "proof_backend",
                  "backend",
                  "proofArtifact",
                  "proof_artifact",
                  "proverArtifact",
                  "prover_artifact",
                  "circuitArtifact",
                  "circuit_artifact",
                  "proofArtifactHash",
                  "proof_artifact_hash",
                  "proverArtifactHash",
                  "prover_artifact_hash",
                  "circuitArtifactHash",
                  "circuit_artifact_hash",
                  "provingKey",
                  "proving_key",
                  "provingKeyHash",
                  "proving_key_hash",
                  "verifierKey",
                  "verifier_key",
                  "verifierKeyHash",
                  "verifier_key_hash",
                  "destinationBindingHash",
                  "destination_binding_hash",
                  "noWasm",
                  "no_wasm",
                  "remoteProverRequired",
                  "remote_prover_required",
                  "browserImplementation",
                  "browser_implementation",
                  "nativeSdkArtifacts",
                  "native_sdk_artifacts",
                  "sdkArtifacts",
                  "sdk_artifacts",
                  "crossSdkFixtureParityArtifact",
                  "cross_sdk_fixture_parity_artifact",
                  "nativeProverSelfTestArtifact",
                  "native_prover_self_test_artifact",
                  "selfTestArtifact",
                  "self_test_artifact",
                  "auditHashes",
                  "audit_hashes")));
  private static final Set<String> NATIVE_EVM_PROVER_BUNDLE_SDK_ARTIFACT_KEYS =
      Collections.unmodifiableSet(
          new LinkedHashSet<>(
              Arrays.asList(
                  "sdk",
                  "implementation",
                  "proofArtifactHash",
                  "proof_artifact_hash",
                  "proverArtifactHash",
                  "prover_artifact_hash",
                  "provingKeyHash",
                  "proving_key_hash",
                  "implementationArtifact",
                  "implementation_artifact",
                  "implementationPath",
                  "implementation_path",
                  "implementationHash",
                  "implementation_hash")));
  private static final Set<String> NATIVE_EVM_PROVER_PARITY_FIXTURE_KEYS =
      Collections.unmodifiableSet(
          new LinkedHashSet<>(
              Arrays.asList(
                  "schema",
                  "domain",
                  "chain",
                  "proofBackend",
                  "proof_backend",
                  "backend",
                  "proofArtifactHash",
                  "proof_artifact_hash",
                  "proverArtifactHash",
                  "prover_artifact_hash",
                  "circuitArtifactHash",
                  "circuit_artifact_hash",
                  "provingKeyHash",
                  "proving_key_hash",
                  "verifierKeyHash",
                  "verifier_key_hash",
                  "destinationBindingHash",
                  "destination_binding_hash",
                  "receiptProofHash",
                  "receipt_proof_hash",
                  "sourceProofHash",
                  "source_proof_hash",
                  "publicSignalWords",
                  "public_signal_words",
                  "calldataHash",
                  "calldata_hash",
                  "toriiSubmitPayloadHash",
                  "torii_submit_payload_hash",
                  "sdkResults",
                  "sdk_results")));
  private static final Set<String> NATIVE_EVM_PROVER_PARITY_SDK_RESULT_KEYS =
      Collections.unmodifiableSet(
          new LinkedHashSet<>(
              Arrays.asList(
                  "receiptProofHash",
                  "receipt_proof_hash",
                  "sourceProofHash",
                  "source_proof_hash",
                  "destinationBindingHash",
                  "destination_binding_hash",
                  "publicSignalWords",
                  "public_signal_words",
                  "calldataHash",
                  "calldata_hash",
                  "toriiSubmitPayloadHash",
                  "torii_submit_payload_hash")));
  private static final Set<String> NATIVE_EVM_PROVER_SELF_TEST_FIXTURE_KEYS =
      Collections.unmodifiableSet(
          new LinkedHashSet<>(
              Arrays.asList(
                  "schema",
                  "domain",
                  "chain",
                  "proofBackend",
                  "proof_backend",
                  "backend",
                  "proofArtifactHash",
                  "proof_artifact_hash",
                  "proverArtifactHash",
                  "prover_artifact_hash",
                  "circuitArtifactHash",
                  "circuit_artifact_hash",
                  "provingKeyHash",
                  "proving_key_hash",
                  "verifierKeyHash",
                  "verifier_key_hash",
                  "destinationBindingHash",
                  "destination_binding_hash",
                  "requestHash",
                  "request_hash",
                  "witnessHash",
                  "witness_hash",
                  "sourceProofHash",
                  "source_proof_hash",
                  "proofHash",
                  "proof_hash",
                  "publicSignalWords",
                  "public_signal_words",
                  "calldataHash",
                  "calldata_hash",
                  "toriiSubmitPayloadHash",
                  "torii_submit_payload_hash",
                  "sdkResults",
                  "sdk_results")));
  private static final Set<String> NATIVE_EVM_PROVER_SELF_TEST_SDK_RESULT_KEYS =
      Collections.unmodifiableSet(
          new LinkedHashSet<>(
              Arrays.asList(
                  "requestHash",
                  "request_hash",
                  "witnessHash",
                  "witness_hash",
                  "sourceProofHash",
                  "source_proof_hash",
                  "proofHash",
                  "proof_hash",
                  "publicSignalWords",
                  "public_signal_words",
                  "calldataHash",
                  "calldata_hash",
                  "toriiSubmitPayloadHash",
                  "torii_submit_payload_hash")));
  private static final String[] SIGNAL_LABELS =
      new String[] {
        "sccp:groth16-bn254:signal:message-id:v1",
        "sccp:groth16-bn254:signal:payload-hash:v1",
        "sccp:groth16-bn254:signal:target-domain:v1",
        "sccp:groth16-bn254:signal:commitment-root:v1",
        "sccp:groth16-bn254:signal:finality-height:v1",
        "sccp:groth16-bn254:signal:finality-block-hash:v1",
        "sccp:groth16-bn254:signal:source-domain:v1",
        "sccp:groth16-bn254:signal:statement-hash:v1",
        "sccp:groth16-bn254:signal:destination-binding-hash:v1",
      };

  private final WitnessProvider witnessProvider;
  private final ProofEngine proofEngine;

  public EvmSccpProver() {
    this(null, null);
  }

  public EvmSccpProver(final WitnessProvider witnessProvider, final ProofEngine proofEngine) {
    this.witnessProvider = witnessProvider;
    this.proofEngine = proofEngine;
  }

  public ProofRequest buildRequest(final ProofRequestInput input) {
    final ProofRequestInput resolved =
        witnessProvider == null
            ? input
            : witnessProvider.resolveWitness(witnessProviderInputSnapshot(input));
    return buildProofRequest(resolved);
  }

  public ProofResult prove(final ProofRequestInput input) {
    final ProofRequest request = buildRequest(input);
    if (proofEngine == null) {
      throw new IllegalStateException("EVM-family SCCP Groth16 prover is not linked");
    }
    requireProductionProofRequest(request);
    return wrapProofResult(proofEngine.prove(callbackRequestSnapshot(request)), request);
  }

  private static ProofRequestInput witnessProviderInputSnapshot(final ProofRequestInput input) {
    final byte[] bundleBytes = Objects.requireNonNull(input.bundleBytes(), "bundleBytes");
    final byte[] sourceProofBytes =
        Objects.requireNonNull(input.sourceProofBytes(), "sourceProofBytes");
    return new ProofRequestInput(
        input.publicInputs(),
        Arrays.copyOf(bundleBytes, bundleBytes.length),
        Arrays.copyOf(sourceProofBytes, sourceProofBytes.length),
        input.statementHash(),
        input.destinationBindingHash(),
        input.backend(),
        input.sourceDomain(),
        input.destinationBinding(),
        input.proofArtifactHash(),
        input.provingKeyHash());
  }

  static ProofRequest callbackRequestSnapshot(final ProofRequest request) {
    return new ProofRequest(
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
        request.requestHash(),
        request.destinationBinding(),
        request.proofArtifactHash(),
        request.provingKeyHash());
  }

  public static byte[] canonicalPublicInputsBytes(final PublicInputsInput input) {
    if (input.version() != 1) {
      throw new IllegalArgumentException("publicInputs.version must be 1");
    }
    if (input.targetDomain() == 0) {
      throw new IllegalArgumentException("publicInputs.targetDomain must not be zero");
    }
    if (input.targetDomain() != DOMAIN_ETH && input.targetDomain() != DOMAIN_BSC) {
      throw new IllegalArgumentException("publicInputs.targetDomain must be ETH or BSC");
    }
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(input.version());
    write(out, nonZeroHex32Bytes(input.messageId(), "messageId"));
    write(out, nonZeroHex32Bytes(input.payloadHash(), "payloadHash"));
    writeU32Le(out, input.targetDomain());
    write(out, nonZeroHex32Bytes(input.commitmentRoot(), "commitmentRoot"));
    writeU64Le(out, normalizeU64(input.finalityHeight(), "finalityHeight"));
    write(out, nonZeroHex32Bytes(input.finalityBlockHash(), "finalityBlockHash"));
    return out.toByteArray();
  }

  public static List<String> groth16Bn254PublicSignalWords(
      final PublicInputsInput publicInputs,
      final int sourceDomain,
      final String statementHash,
      final String destinationBindingHash) {
    if (publicInputs.version() != 1) {
      throw new IllegalArgumentException("publicInputs.version must be 1");
    }
    if (publicInputs.targetDomain() == 0) {
      throw new IllegalArgumentException("publicInputs.targetDomain must not be zero");
    }
    if (publicInputs.targetDomain() != DOMAIN_ETH && publicInputs.targetDomain() != DOMAIN_BSC) {
      throw new IllegalArgumentException("publicInputs.targetDomain must be ETH or BSC");
    }
    if (sourceDomain != DOMAIN_SORA) {
      throw new IllegalArgumentException("sourceDomain must be SORA");
    }
    if (sourceDomain == publicInputs.targetDomain()) {
      throw new IllegalArgumentException("sourceDomain and publicInputs.targetDomain must differ");
    }
    final byte[][] values =
        new byte[][] {
          nonZeroHex32Bytes(publicInputs.messageId(), "messageId"),
          nonZeroHex32Bytes(publicInputs.payloadHash(), "payloadHash"),
          abiWordU32(publicInputs.targetDomain()),
          nonZeroHex32Bytes(publicInputs.commitmentRoot(), "commitmentRoot"),
          abiWordU64(normalizeU64(publicInputs.finalityHeight(), "finalityHeight")),
          nonZeroHex32Bytes(publicInputs.finalityBlockHash(), "finalityBlockHash"),
          abiWordU32(sourceDomain),
          nonZeroHex32Bytes(statementHash, "statementHash"),
          nonZeroHex32Bytes(destinationBindingHash, "destinationBindingHash"),
        };
    final String[] words = new String[values.length];
    for (int index = 0; index < values.length; index++) {
      words[index] = groth16SignalWord(SIGNAL_LABELS[index], values[index]);
    }
    return Arrays.asList(words);
  }

  public static ProofRequest buildProofRequest(final ProofRequestInput input) {
    Objects.requireNonNull(input, "input");
    if (!GROTH16_BN254_PROOF_BACKEND_V1.equals(input.backend())) {
      throw new IllegalArgumentException("backend must be evm-groth16-bn254-v1");
    }
    final byte[] bundleBytes = Arrays.copyOf(input.bundleBytes(), input.bundleBytes().length);
    final byte[] sourceProofBytes =
        requireOptionalSourceProofBytes(input.sourceProofBytes(), "sourceProofBytes");
    if (bundleBytes.length == 0) {
      throw new IllegalArgumentException("bundleBytes must not be empty");
    }
    final byte[] publicInputsBytes = canonicalPublicInputsBytes(input.publicInputs());
    final ProofContext proofContext =
        normalizeProofContext(input.statementHash(), input.destinationBindingHash());
    final Groth16ProverArtifacts proverArtifacts =
        normalizeOptionalGroth16ProverArtifacts(input.proofArtifactHash(), input.provingKeyHash());
    final List<String> publicSignalWords =
        groth16Bn254PublicSignalWords(
            input.publicInputs(),
            input.sourceDomain(),
            proofContext.statementHash(),
            proofContext.destinationBindingHash());
    final ByteArrayOutputStream preimage = new ByteArrayOutputStream();
    write(preimage, publicInputsBytes);
    writeU32Le(preimage, bundleBytes.length);
    write(preimage, bundleBytes);
    writeU32Le(preimage, sourceProofBytes.length);
    write(preimage, sourceProofBytes);
    write(preimage, hex32Bytes(proofContext.statementHash(), "statementHash"));
    write(preimage, hex32Bytes(proofContext.destinationBindingHash(), "destinationBindingHash"));
    if (proverArtifacts != null) {
      write(preimage, hex32Bytes(proverArtifacts.proofArtifactHash(), "proofArtifactHash"));
      write(preimage, hex32Bytes(proverArtifacts.provingKeyHash(), "provingKeyHash"));
    }
    for (final String signal : publicSignalWords) {
      write(preimage, hex32Bytes(signal, "publicSignalWords"));
    }
    return new ProofRequest(
        1,
        input.backend(),
        input.sourceDomain(),
        input.publicInputs().targetDomain(),
        input.publicInputs(),
        publicInputsBytes,
        publicSignalWords,
        bundleBytes,
        sourceProofBytes,
        proofContext,
        proofContext.statementHash(),
        proofContext.destinationBindingHash(),
        hashHex(PROOF_REQUEST_PREFIX_V1, preimage.toByteArray()),
        input.destinationBinding(),
        proverArtifacts == null ? null : proverArtifacts.proofArtifactHash(),
        proverArtifacts == null ? null : proverArtifacts.provingKeyHash());
  }

  public static ProofResult wrapProofResult(final byte[] proofBytes, final ProofRequest request) {
    Objects.requireNonNull(request, "request");
    if (!GROTH16_BN254_PROOF_BACKEND_V1.equals(request.backend())) {
      throw new IllegalArgumentException("backend must be evm-groth16-bn254-v1");
    }
    requireProductionProofRequest(request);
    requireProofBytesForContext(proofBytes, request.publicInputs(), request.sourceDomain());
    final ByteArrayOutputStream envelopePayload = new ByteArrayOutputStream();
    write(envelopePayload, hex32Bytes(request.requestHash(), "requestHash"));
    write(envelopePayload, proofBytes);
    return new ProofResult(
        1,
        request.backend(),
        Arrays.copyOf(proofBytes, proofBytes.length),
        Base64.getEncoder().encodeToString(proofBytes),
        request.publicInputs(),
        request.publicSignalWords(),
        request.bundleBytes(),
        request.sourceProofBytes(),
        request.proofContext(),
        request.statementHash(),
        request.destinationBindingHash(),
        request.requestHash(),
        hashHex(PROOF_ENVELOPE_PREFIX_V1, envelopePayload.toByteArray()),
        request.destinationBinding(),
        request.proofArtifactHash(),
        request.provingKeyHash());
  }

  private static ProofResult requireWrappedProofResultForSubmission(final ProofResult proofResult) {
    Objects.requireNonNull(proofResult, "proofResult");
    if (!GROTH16_BN254_PROOF_BACKEND_V1.equals(proofResult.backend())) {
      throw new IllegalArgumentException("proofResult.backend must be evm-groth16-bn254-v1");
    }
    final ProofContext proofContext =
        Objects.requireNonNull(proofResult.proofContext(), "proofResult.proofContext");
    final ProofContext expectedProofContext =
        normalizeProofContext(proofResult.statementHash(), proofResult.destinationBindingHash());
    if (!proofContext.equals(expectedProofContext)) {
      throw new IllegalArgumentException(
          "proofResult.proofContext must match statementHash and destinationBindingHash");
    }
    final byte[] proofBytes = proofResult.proofBytes();
    requireProofBytesForPublicInputs(proofBytes, proofResult.publicInputs());
    if (!Base64.getEncoder().encodeToString(proofBytes).equals(proofResult.proofBase64())) {
      throw new IllegalArgumentException(
          "proofResult.proofBase64 must match proofResult.proofBytes");
    }
    final String requestHash = normalizeHex32(proofResult.requestHash(), "proofResult.requestHash");
    final String envelopeHash =
        normalizeHex32(proofResult.envelopeHash(), "proofResult.envelopeHash");
    final ByteArrayOutputStream envelopePayload = new ByteArrayOutputStream();
    write(envelopePayload, hex32Bytes(requestHash, "proofResult.requestHash"));
    write(envelopePayload, proofBytes);
    if (!envelopeHash.equals(hashHex(PROOF_ENVELOPE_PREFIX_V1, envelopePayload.toByteArray()))) {
      throw new IllegalArgumentException(
          "proofResult.envelopeHash must match wrapped proof bytes");
    }
    final byte[] sourceProofBytes =
        requireOptionalSourceProofBytes(
            proofResult.sourceProofBytes(), "proofResult.sourceProofBytes");
    final ProofRequest expectedRequest =
        buildProofRequest(
            new ProofRequestInput(
                proofResult.publicInputs(),
                proofResult.bundleBytes(),
                sourceProofBytes,
                proofResult.statementHash(),
                proofResult.destinationBindingHash(),
                proofResult.backend(),
                DOMAIN_SORA,
                proofResult.destinationBinding(),
                proofResult.proofArtifactHash(),
                proofResult.provingKeyHash()));
    if (!Objects.equals(expectedRequest.requestHash(), requestHash)) {
      throw new IllegalArgumentException(
          "proofResult.requestHash must match bundleBytes and sourceProofBytes");
    }
    return proofResult;
  }

  public static List<String> messageTransparentPublicInputAbiWords(
      final PublicInputsInput publicInputs) {
    final byte[][] words = messageTransparentPublicInputAbiWordBytes(publicInputs);
    final List<String> out = new ArrayList<>(words.length);
    for (final byte[] word : words) {
      out.add("0x" + hexLower(word));
    }
    return Collections.unmodifiableList(out);
  }

  public static byte[] submitMessageProofCallData(
      final byte[] proofBytes, final PublicInputsInput publicInputs, final String statementHash) {
    return submitMessageProofCallData(proofBytes, publicInputs, statementHash, DOMAIN_SORA);
  }

  public static byte[] submitMessageProofCallData(
      final byte[] proofBytes,
      final PublicInputsInput publicInputs,
      final String statementHash,
      final int sourceDomain) {
    if (sourceDomain != DOMAIN_SORA) {
      throw new IllegalArgumentException("sourceDomain must be SORA");
    }
    requireProofBytesForContext(proofBytes, publicInputs, sourceDomain);
    final byte[][] publicInputWords = messageTransparentPublicInputAbiWordBytes(publicInputs);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    write(out, SUBMIT_MESSAGE_PROOF_SELECTOR_BYTES_V1);
    write(out, abiWordU256(32 * 8));
    for (final byte[] word : publicInputWords) {
      write(out, word);
    }
    write(out, nonZeroHex32Bytes(statementHash, "statementHash"));
    write(out, abiWordU256(proofBytes.length));
    write(out, proofBytes);
    final int padding = (32 - (proofBytes.length % 32)) % 32;
    if (padding > 0) {
      write(out, new byte[padding]);
    }
    return out.toByteArray();
  }

  public static Submission buildSubmission(final SubmissionInput input) {
    Objects.requireNonNull(input, "input");
    if (input.sourceDomain() != DOMAIN_SORA) {
      throw new IllegalArgumentException("sourceDomain must be SORA");
    }
    final PublicInputsInput publicInputs = input.publicInputs();
    canonicalPublicInputsBytes(publicInputs);
    final byte[] proofBytes = input.proofBytes();
    requireProofBytesForContext(proofBytes, publicInputs, input.sourceDomain());
    final String statementHash = normalizeHex32(input.statementHash(), "statementHash");
    final String destinationBindingHash =
        normalizeHex32(input.destinationBindingHash(), "destinationBindingHash");
    ProofResult proofResult = input.proofResult();
    if (proofResult != null) {
      proofResult = requireWrappedProofResultForSubmission(proofResult);
    }
    if (proofResult != null) {
      if (!GROTH16_BN254_PROOF_BACKEND_V1.equals(proofResult.backend())) {
        throw new IllegalArgumentException("proofResult.backend must be evm-groth16-bn254-v1");
      }
      if (!Objects.equals(proofResult.publicInputs(), publicInputs)) {
        throw new IllegalArgumentException("publicInputs must match proofResult.publicInputs");
      }
      if (!Arrays.equals(proofResult.proofBytes(), proofBytes)) {
        throw new IllegalArgumentException("proofBytes must match proofResult.proofBytes");
      }
      if (!Objects.equals(proofResult.statementHash(), statementHash)) {
        throw new IllegalArgumentException("statementHash must match proofResult.statementHash");
      }
      if (!Objects.equals(proofResult.destinationBindingHash(), destinationBindingHash)) {
        throw new IllegalArgumentException(
            "destinationBindingHash must match proofResult.destinationBindingHash");
      }
    }
    final List<String> publicSignalWords =
        groth16Bn254PublicSignalWords(
            publicInputs, input.sourceDomain(), statementHash, destinationBindingHash);
    final List<String> suppliedPublicSignalWords =
        input.publicSignalWords() != null
            ? input.publicSignalWords()
            : (proofResult == null ? null : proofResult.publicSignalWords());
    if (suppliedPublicSignalWords != null) {
      if (suppliedPublicSignalWords.size() != 9) {
        throw new IllegalArgumentException("publicSignalWords must contain 9 words");
      }
      final List<String> normalizedSignals = new ArrayList<>(suppliedPublicSignalWords.size());
      for (int index = 0; index < suppliedPublicSignalWords.size(); index++) {
        normalizedSignals.add(
            normalizeOptionalHex32(suppliedPublicSignalWords.get(index), "publicSignalWords[" + index + "]"));
      }
      if (!normalizedSignals.equals(publicSignalWords)) {
        throw new IllegalArgumentException(
            "publicSignalWords must match publicInputs and proof context");
      }
    }
    final List<String> publicInputWords = messageTransparentPublicInputAbiWords(publicInputs);
    final ByteArrayOutputStream publicInputWordsBytesOut = new ByteArrayOutputStream();
    for (final byte[] word : messageTransparentPublicInputAbiWordBytes(publicInputs)) {
      write(publicInputWordsBytesOut, word);
    }
    final byte[] publicInputWordsBytes = publicInputWordsBytesOut.toByteArray();
    final byte[] callData =
        submitMessageProofCallData(proofBytes, publicInputs, statementHash, input.sourceDomain());
    final List<SubmissionArgument> arguments =
        Arrays.asList(
            new SubmissionArgument("proof_bytes", "raw_bytes", "0x" + hexLower(proofBytes)),
            new SubmissionArgument(
                "public_inputs", "abi_bytes32x6", "0x" + hexLower(publicInputWordsBytes)),
            new SubmissionArgument("statement_hash", "abi_bytes32", statementHash));
    return new Submission(
        1,
        STARK_FRI_PROOF_FAMILY_V1,
        GROTH16_BN254_PROOF_BACKEND_V1,
        "evm_groth16_contract_call",
        CONTRACT_CALL_ABI_TUPLE_V1,
        "contract_call",
        SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1,
        SUBMIT_MESSAGE_PROOF_ABI_V1,
        SUBMIT_MESSAGE_PROOF_SELECTOR_V1,
        input.sourceDomain(),
        publicInputs.targetDomain(),
        publicInputs,
        publicInputWords,
        publicSignalWords,
        statementHash,
        destinationBindingHash,
        arguments,
        "0x" + hexLower(callData),
        "0x" + hexLower(callData),
        proofBytes,
        publicInputWordsBytes,
        callData);
  }

  private static void requireNonZeroProofBytes(final byte[] proofBytes) {
    if (proofBytes == null || proofBytes.length == 0) {
      throw new IllegalArgumentException("proofBytes must not be empty");
    }
    for (final byte value : proofBytes) {
      if (value != 0) {
        if (proofBytes.length != GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1) {
          throw new IllegalArgumentException(
              "proofBytes must be " + GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1 + " bytes");
        }
        requireGroth16ProofTuple(proofBytes, "proofBytes");
        return;
      }
    }
    throw new IllegalArgumentException("proofBytes must not be all zero");
  }

  private static void requireOptionalNonZeroBytes(final byte[] bytes, final String label) {
    if (bytes.length == 0) {
      return;
    }
    for (final byte value : bytes) {
      if (value != 0) {
        return;
      }
    }
    throw new IllegalArgumentException(label + " must not be all zero");
  }

  private static byte[] requireOptionalSourceProofBytes(final byte[] bytes, final String label) {
    final byte[] copy = Arrays.copyOf(Objects.requireNonNull(bytes, label), bytes.length);
    if (copy.length > SOURCE_STATE_MAX_PROOF_BYTES) {
      throw new IllegalArgumentException(
          label + " must be at most " + SOURCE_STATE_MAX_PROOF_BYTES + " bytes");
    }
    requireOptionalNonZeroBytes(copy, label);
    return copy;
  }

  private static void requireNonEmptyNonZeroBytes(final byte[] bytes, final String label) {
    if (bytes.length == 0) {
      throw new IllegalArgumentException(label + " must not be empty");
    }
    requireOptionalNonZeroBytes(bytes, label);
  }

  private static void requireProofBytesForPublicInputs(
      final byte[] proofBytes, final PublicInputsInput publicInputs) {
    requireNonZeroProofBytes(proofBytes);
    if (!Arrays.equals(
        proofWord(proofBytes, 1),
        nonZeroHex32Bytes(publicInputs.messageId(), "publicInputs.messageId"))) {
      throw new IllegalArgumentException("proofBytes.messageId must match publicInputs.messageId");
    }
    if (!Arrays.equals(
        proofWord(proofBytes, 3),
        nonZeroHex32Bytes(publicInputs.commitmentRoot(), "publicInputs.commitmentRoot"))) {
      throw new IllegalArgumentException(
          "proofBytes.commitmentRoot must match publicInputs.commitmentRoot");
    }
  }

  private static void requireProofBytesForContext(
      final byte[] proofBytes, final PublicInputsInput publicInputs, final int sourceDomain) {
    requireProofBytesForPublicInputs(proofBytes, publicInputs);
    if (!proofWordValue(proofBytes, 2).equals(BigInteger.valueOf(sourceDomain))) {
      throw new IllegalArgumentException("proofBytes.sourceDomain must match sourceDomain");
    }
  }

  private static byte[] proofWord(final byte[] proofBytes, final int index) {
    return Arrays.copyOfRange(proofBytes, index * 32, (index + 1) * 32);
  }

  private static BigInteger proofWordValue(final byte[] proofBytes, final int index) {
    return new BigInteger(1, proofWord(proofBytes, index));
  }

  private static boolean proofWordIsZero(final byte[] proofBytes, final int index) {
    for (final byte value : proofWord(proofBytes, index)) {
      if (value != 0) {
        return false;
      }
    }
    return true;
  }

  private static void requireBaseFieldWord(
      final byte[] proofBytes, final int index, final String label) {
    if (proofWordValue(proofBytes, index).compareTo(BN254_BASE_FIELD_MODULUS) >= 0) {
      throw new IllegalArgumentException(label + " must be a BN254 base-field element");
    }
  }

  private static void requireNonZeroPoint(
      final byte[] proofBytes, final int[] indexes, final String label) {
    for (final int index : indexes) {
      if (!proofWordIsZero(proofBytes, index)) {
        return;
      }
    }
    throw new IllegalArgumentException(label + " must not be zero");
  }

  private static BigInteger fq(final BigInteger value) {
    return value.mod(BN254_BASE_FIELD_MODULUS);
  }

  private static final class G2ProjectivePoint {
    private final BigInteger[] x;
    private final BigInteger[] y;
    private final BigInteger[] z;
    private final boolean infinity;

    private G2ProjectivePoint(
        final BigInteger[] x,
        final BigInteger[] y,
        final BigInteger[] z,
        final boolean infinity) {
      this.x = x;
      this.y = y;
      this.z = z;
      this.infinity = infinity;
    }
  }

  private static BigInteger[] fq2Add(final BigInteger[] left, final BigInteger[] right) {
    return new BigInteger[] {fq(left[0].add(right[0])), fq(left[1].add(right[1]))};
  }

  private static BigInteger[] fq2Sub(final BigInteger[] left, final BigInteger[] right) {
    return new BigInteger[] {fq(left[0].subtract(right[0])), fq(left[1].subtract(right[1]))};
  }

  private static BigInteger[] fq2Scale(final BigInteger[] left, final long scalar) {
    final BigInteger value = BigInteger.valueOf(scalar);
    return new BigInteger[] {fq(left[0].multiply(value)), fq(left[1].multiply(value))};
  }

  private static BigInteger[] fq2Mul(final BigInteger[] left, final BigInteger[] right) {
    return new BigInteger[] {
      fq(left[0].multiply(right[0]).subtract(left[1].multiply(right[1]))),
      fq(left[0].multiply(right[1]).add(left[1].multiply(right[0])))
    };
  }

  private static boolean fq2IsZero(final BigInteger[] value) {
    return BigInteger.ZERO.equals(value[0]) && BigInteger.ZERO.equals(value[1]);
  }

  private static G2ProjectivePoint g2Infinity() {
    return new G2ProjectivePoint(
        new BigInteger[] {BigInteger.ZERO, BigInteger.ZERO},
        new BigInteger[] {BigInteger.ONE, BigInteger.ZERO},
        new BigInteger[] {BigInteger.ZERO, BigInteger.ZERO},
        true);
  }

  private static G2ProjectivePoint g2AffineProjective(
      final BigInteger[] x, final BigInteger[] y) {
    return new G2ProjectivePoint(
        x, y, new BigInteger[] {BigInteger.ONE, BigInteger.ZERO}, false);
  }

  private static boolean g2ProjectiveIsInfinity(final G2ProjectivePoint point) {
    return point.infinity || fq2IsZero(point.z);
  }

  private static G2ProjectivePoint g2ProjectiveDouble(final G2ProjectivePoint point) {
    if (g2ProjectiveIsInfinity(point) || fq2IsZero(point.y)) {
      return g2Infinity();
    }
    final BigInteger[] xx = fq2Mul(point.x, point.x);
    final BigInteger[] yy = fq2Mul(point.y, point.y);
    final BigInteger[] yyyy = fq2Mul(yy, yy);
    final BigInteger[] s =
        fq2Scale(
            fq2Sub(fq2Sub(fq2Mul(fq2Add(point.x, yy), fq2Add(point.x, yy)), xx), yyyy),
            2);
    final BigInteger[] m = fq2Scale(xx, 3);
    final BigInteger[] x3 = fq2Sub(fq2Mul(m, m), fq2Scale(s, 2));
    final BigInteger[] y3 =
        fq2Sub(fq2Mul(m, fq2Sub(s, x3)), fq2Scale(yyyy, 8));
    final BigInteger[] z3 = fq2Scale(fq2Mul(point.y, point.z), 2);
    return new G2ProjectivePoint(x3, y3, z3, false);
  }

  private static G2ProjectivePoint g2ProjectiveAddAffine(
      final G2ProjectivePoint point, final BigInteger[] affineX, final BigInteger[] affineY) {
    if (g2ProjectiveIsInfinity(point)) {
      return g2AffineProjective(affineX, affineY);
    }
    final BigInteger[] z1z1 = fq2Mul(point.z, point.z);
    final BigInteger[] u2 = fq2Mul(affineX, z1z1);
    final BigInteger[] s2 = fq2Mul(affineY, fq2Mul(point.z, z1z1));
    final BigInteger[] h = fq2Sub(u2, point.x);
    if (fq2IsZero(h)) {
      return Arrays.equals(s2, point.y) ? g2ProjectiveDouble(point) : g2Infinity();
    }
    final BigInteger[] hh = fq2Mul(h, h);
    final BigInteger[] i = fq2Scale(hh, 4);
    final BigInteger[] j = fq2Mul(h, i);
    final BigInteger[] r = fq2Scale(fq2Sub(s2, point.y), 2);
    final BigInteger[] v = fq2Mul(point.x, i);
    final BigInteger[] x3 = fq2Sub(fq2Sub(fq2Mul(r, r), j), fq2Scale(v, 2));
    final BigInteger[] y3 =
        fq2Sub(fq2Mul(r, fq2Sub(v, x3)), fq2Scale(fq2Mul(point.y, j), 2));
    final BigInteger[] z3 =
        fq2Sub(fq2Sub(fq2Mul(fq2Add(point.z, h), fq2Add(point.z, h)), z1z1), hh);
    return new G2ProjectivePoint(x3, y3, z3, false);
  }

  private static boolean g2PointIsInPrimeSubgroup(
      final BigInteger[] x, final BigInteger[] y) {
    G2ProjectivePoint acc = g2Infinity();
    for (int index = BN254_SCALAR_FIELD_MODULUS.bitLength() - 1; index >= 0; index--) {
      acc = g2ProjectiveDouble(acc);
      if (BN254_SCALAR_FIELD_MODULUS.testBit(index)) {
        acc = g2ProjectiveAddAffine(acc, x, y);
      }
    }
    return g2ProjectiveIsInfinity(acc);
  }

  private static void requireG1Point(
      final byte[] proofBytes, final int xIndex, final int yIndex, final String label) {
    requireNonZeroPoint(proofBytes, new int[] {xIndex, yIndex}, label);
    final BigInteger x = proofWordValue(proofBytes, xIndex);
    final BigInteger y = proofWordValue(proofBytes, yIndex);
    final BigInteger left = fq(y.multiply(y));
    final BigInteger right = fq(x.multiply(x).multiply(x).add(BigInteger.valueOf(3)));
    if (!left.equals(right)) {
      throw new IllegalArgumentException(label + " must be a BN254 G1 point");
    }
  }

  private static void requireG2Point(
      final byte[] proofBytes,
      final int x0Index,
      final int x1Index,
      final int y0Index,
      final int y1Index,
      final String label) {
    requireNonZeroPoint(proofBytes, new int[] {x0Index, x1Index, y0Index, y1Index}, label);
    final BigInteger[] x =
        new BigInteger[] {proofWordValue(proofBytes, x0Index), proofWordValue(proofBytes, x1Index)};
    final BigInteger[] y =
        new BigInteger[] {proofWordValue(proofBytes, y0Index), proofWordValue(proofBytes, y1Index)};
    final BigInteger[] left = fq2Mul(y, y);
    final BigInteger[] x2 = fq2Mul(x, x);
    final BigInteger[] x3 = fq2Mul(x2, x);
    final BigInteger[] right =
        new BigInteger[] {fq(x3[0].add(BN254_G2_B_C0)), fq(x3[1].add(BN254_G2_B_C1))};
    if (!left[0].equals(right[0]) || !left[1].equals(right[1]) || !g2PointIsInPrimeSubgroup(x, y)) {
      throw new IllegalArgumentException(label + " must be a BN254 G2 point");
    }
  }

  private static void requireGroth16ProofTuple(final byte[] proofBytes, final String label) {
    if (!BigInteger.ONE.equals(proofWordValue(proofBytes, 0))) {
      throw new IllegalArgumentException(label + ".version must be 1");
    }
    if (proofWordIsZero(proofBytes, 1)) {
      throw new IllegalArgumentException(label + ".messageId must not be zero");
    }
    if (proofWordValue(proofBytes, 2).compareTo(BigInteger.valueOf(0xffff_ffffL)) > 0) {
      throw new IllegalArgumentException(label + ".sourceDomain must fit u32");
    }
    if (proofWordIsZero(proofBytes, 3)) {
      throw new IllegalArgumentException(label + ".commitmentRoot must not be zero");
    }
    final String[] fields = {"a.x", "a.y", "b.x0", "b.x1", "b.y0", "b.y1", "c.x", "c.y"};
    for (int offset = 0; offset < fields.length; offset++) {
      requireBaseFieldWord(proofBytes, 4 + offset, label + "." + fields[offset]);
    }
    requireG1Point(proofBytes, 4, 5, label + ".a");
    requireG2Point(proofBytes, 6, 7, 8, 9, label + ".b");
    requireG1Point(proofBytes, 10, 11, label + ".c");
  }

  private static void requireCanonicalProofRequest(final ProofRequest request) {
    final ProofRequest expected =
        buildProofRequest(
            new ProofRequestInput(
                request.publicInputs(),
                request.bundleBytes(),
                request.sourceProofBytes(),
                request.statementHash(),
                request.destinationBindingHash(),
                request.backend(),
                request.sourceDomain(),
                request.destinationBinding(),
                request.proofArtifactHash(),
                request.provingKeyHash()));
    if (request.version() != expected.version()
        || !Objects.equals(request.backend(), expected.backend())
        || request.sourceDomain() != expected.sourceDomain()
        || request.targetDomain() != expected.targetDomain()
        || !Objects.equals(request.publicInputs(), expected.publicInputs())
        || !Arrays.equals(request.publicInputsBytes(), expected.publicInputsBytes())
        || !Objects.equals(request.publicSignalWords(), expected.publicSignalWords())
        || !Arrays.equals(request.bundleBytes(), expected.bundleBytes())
        || !Arrays.equals(request.sourceProofBytes(), expected.sourceProofBytes())
        || !Objects.equals(request.proofContext(), expected.proofContext())
        || !Objects.equals(request.statementHash(), expected.statementHash())
        || !Objects.equals(request.destinationBindingHash(), expected.destinationBindingHash())
        || !Objects.equals(request.proofArtifactHash(), expected.proofArtifactHash())
        || !Objects.equals(request.provingKeyHash(), expected.provingKeyHash())
        || !Objects.equals(request.requestHash(), expected.requestHash())
        || !Objects.equals(request.destinationBinding(), expected.destinationBinding())) {
      throw new IllegalArgumentException("proof request must be canonical");
    }
  }

  private static void requireProductionProofRequest(final ProofRequest request) {
    requireCanonicalProofRequest(request);
    if (request.version() != 1) {
      throw new IllegalArgumentException("proof request version must be 1");
    }
    if (!GROTH16_BN254_PROOF_BACKEND_V1.equals(request.backend())) {
      throw new IllegalArgumentException(
          "EVM-family SCCP proof request backend must be evm-groth16-bn254-v1");
    }
    if (request.sourceDomain() != DOMAIN_SORA) {
      throw new IllegalArgumentException("EVM-family SCCP production proofs must start from SORA");
    }
    if (request.targetDomain() != request.publicInputs().targetDomain()
        || (request.targetDomain() != DOMAIN_ETH && request.targetDomain() != DOMAIN_BSC)) {
      throw new IllegalArgumentException("EVM-family SCCP production proofs must target ETH or BSC");
    }
    if (request.bundleBytes().length == 0) {
      throw new IllegalArgumentException(
          "EVM-family SCCP proof request bundleBytes must not be empty");
    }
    requireOptionalSourceProofBytes(
        request.sourceProofBytes(), "EVM-family SCCP proof request sourceProofBytes");
    requireProductionDestinationBinding(request);
  }

  private static void requireProductionDestinationBinding(final ProofRequest request) {
    final SourceSccpProofs.EvmDestinationBinding destinationBinding = request.destinationBinding();
    if (destinationBinding == null) {
      throw new IllegalArgumentException(
          "EVM-family SCCP production proof request destinationBinding must include deployment material");
    }
    final String destinationBindingHash =
        requireDestinationBindingHash(
            request.publicInputs(), destinationBinding, request.backend(), request.sourceDomain());
    if (!Objects.equals(request.destinationBindingHash(), destinationBindingHash)) {
      throw new IllegalArgumentException(
          "destinationBindingHash must match destinationBinding deployment material");
    }
    if (!Objects.equals(request.proofContext().destinationBindingHash(), destinationBindingHash)) {
      throw new IllegalArgumentException(
          "proofContext.destinationBindingHash must match destinationBinding deployment material");
    }
  }

  private static String requireDestinationBindingHash(
      final PublicInputsInput publicInputs,
      final SourceSccpProofs.EvmDestinationBinding destinationBinding,
      final String backend,
      final int sourceDomain) {
    final SourceSccpProofs.EvmDestinationBinding binding =
        Objects.requireNonNull(destinationBinding, "destinationBinding");
    if (binding.version != 1) {
      throw new IllegalArgumentException("destinationBinding.version must be 1");
    }
    if (binding.sourceDomain != sourceDomain) {
      throw new IllegalArgumentException("destinationBinding.sourceDomain must match sourceDomain");
    }
    if (binding.targetDomain != publicInputs.targetDomain()) {
      throw new IllegalArgumentException(
          "destinationBinding.targetDomain must match publicInputs.targetDomain");
    }
    if (!binding.verifierBackend.equals(backend)) {
      throw new IllegalArgumentException("destinationBinding.verifierBackend must match backend");
    }
    if (!STARK_FRI_PROOF_FAMILY_V1.equals(binding.proofFamily)) {
      throw new IllegalArgumentException("destinationBinding.proofFamily must be stark-fri-v1");
    }
    final SourceSccpProofs.EvmDestinationBinding expected =
        SourceSccpProofs.evmDestinationBinding(
            sourceDomain,
            publicInputs.targetDomain(),
            binding.networkId,
            binding.verifierAddress,
            binding.bridgeAddress,
            binding.verifierCodeHash,
            binding.verifierKeyHash);
    if (!evmDestinationBindingsEqual(binding, expected)) {
      throw new IllegalArgumentException("destinationBinding must match deployment material");
    }
    return expected.hash;
  }

  private static boolean evmDestinationBindingsEqual(
      final SourceSccpProofs.EvmDestinationBinding left,
      final SourceSccpProofs.EvmDestinationBinding right) {
    return left.version == right.version
        && left.sourceDomain == right.sourceDomain
        && left.targetDomain == right.targetDomain
        && Objects.equals(left.networkId, right.networkId)
        && Objects.equals(left.verifierAddress, right.verifierAddress)
        && Objects.equals(left.bridgeAddress, right.bridgeAddress)
        && Objects.equals(left.verifierCodeHash, right.verifierCodeHash)
        && Objects.equals(left.verifierKeyHash, right.verifierKeyHash)
        && Objects.equals(left.verifierBackend, right.verifierBackend)
        && Objects.equals(left.proofFamily, right.proofFamily)
        && Objects.equals(left.key, right.key)
        && Objects.equals(left.hash, right.hash);
  }

  private static String hashHex(final String prefix, final byte[] payload) {
    final byte[] prefixBytes = prefix.getBytes(StandardCharsets.UTF_8);
    final byte[] preimage = new byte[prefixBytes.length + payload.length];
    System.arraycopy(prefixBytes, 0, preimage, 0, prefixBytes.length);
    System.arraycopy(payload, 0, preimage, prefixBytes.length, payload.length);
    return "0x" + hexLower(Blake2b.digest256(preimage));
  }

  private static String sha256Hex(final byte[] payload) {
    try {
      return "0x" + hexLower(MessageDigest.getInstance("SHA-256").digest(payload));
    } catch (final NoSuchAlgorithmException ex) {
      throw new IllegalStateException("SHA-256 digest is unavailable", ex);
    }
  }

  private static final byte[][] NATIVE_EVM_PROVER_FORBIDDEN_ARTIFACT_MARKERS = {
    {0x77, 0x65, 0x62, 0x61, 0x73, 0x73, 0x65, 0x6d, 0x62, 0x6c, 0x79},
    {0x77, 0x61, 0x73, 0x6d},
    {0x73, 0x6e, 0x61, 0x72, 0x6b, 0x6a, 0x73},
    {0x72, 0x65, 0x6d, 0x6f, 0x74, 0x65, 0x70, 0x72, 0x6f, 0x76, 0x65, 0x72},
    {0x72, 0x65, 0x6d, 0x6f, 0x74, 0x65, 0x20, 0x70, 0x72, 0x6f, 0x76, 0x65, 0x72},
    {0x72, 0x65, 0x6d, 0x6f, 0x74, 0x65, 0x5f, 0x70, 0x72, 0x6f, 0x76, 0x65, 0x72},
    {0x70, 0x72, 0x6f, 0x76, 0x65, 0x72, 0x5f, 0x75, 0x72, 0x6c},
    {0x70, 0x72, 0x6f, 0x76, 0x65, 0x72, 0x2d, 0x75, 0x72, 0x6c},
    {0x70, 0x72, 0x6f, 0x76, 0x65, 0x72, 0x65, 0x6e, 0x64, 0x70, 0x6f, 0x69, 0x6e, 0x74},
    {0x70, 0x72, 0x6f, 0x76, 0x65, 0x72, 0x20, 0x65, 0x6e, 0x64, 0x70, 0x6f, 0x69, 0x6e, 0x74}
  };

  private static int lowerAsciiByte(final byte value) {
    final int unsigned = value & 0xff;
    return unsigned >= 0x41 && unsigned <= 0x5a ? unsigned + 0x20 : unsigned;
  }

  private static boolean containsNativeEvmProverMarker(
      final byte[] bytes, final byte[] marker) {
    if (marker.length > bytes.length) {
      return false;
    }
    for (int offset = 0; offset <= bytes.length - marker.length; offset++) {
      boolean matched = true;
      for (int index = 0; index < marker.length; index++) {
        if (lowerAsciiByte(bytes[offset + index]) != (marker[index] & 0xff)) {
          matched = false;
          break;
        }
      }
      if (matched) {
        return true;
      }
    }
    return false;
  }

  private static void rejectNativeEvmProverForbiddenArtifactMarkers(
      final byte[] bytes, final String field) {
    for (final byte[] marker : NATIVE_EVM_PROVER_FORBIDDEN_ARTIFACT_MARKERS) {
      if (containsNativeEvmProverMarker(bytes, marker)) {
        throw new IllegalArgumentException(
            field + " contains forbidden prover dependency marker");
      }
    }
  }

  private static void requireNativeEvmProverProductionArtifactSize(
      final byte[] bytes, final String field) {
    if (bytes.length < NATIVE_EVM_PROVER_MIN_ARTIFACT_BYTES_V1) {
      throw new IllegalArgumentException(
          field + " must be at least " + NATIVE_EVM_PROVER_MIN_ARTIFACT_BYTES_V1 + " bytes");
    }
  }

  private static byte[] hex32Bytes(final String value, final String field) {
    String body = Objects.requireNonNull(value, field);
    if (!body.trim().equals(body)) {
      throw new IllegalArgumentException(field + " must be canonical hex");
    }
    if (body.regionMatches(true, 0, "0x", 0, 2)) {
      body = body.substring(2);
    }
    if (body.length() != 64) {
      throw new IllegalArgumentException(field + " must be 32 bytes");
    }
    final byte[] out = new byte[32];
    for (int i = 0; i < out.length; i++) {
      final int hi = Character.digit(body.charAt(i * 2), 16);
      final int lo = Character.digit(body.charAt(i * 2 + 1), 16);
      if (hi < 0 || lo < 0) {
        throw new IllegalArgumentException(field + " must be canonical hex");
      }
      out[i] = (byte) ((hi << 4) | lo);
    }
    return out;
  }

  private static byte[] nonZeroHex32Bytes(final String value, final String field) {
    final byte[] out = hex32Bytes(value, field);
    for (final byte b : out) {
      if (b != 0) {
        return out;
      }
    }
    throw new IllegalArgumentException(field + " must not be zero");
  }

  private static String normalizeHex32(final String value, final String field) {
    return "0x" + hexLower(nonZeroHex32Bytes(value, field));
  }

  private static String normalizeNativeEvmProverBundleHex32(
      final String value, final String field) {
    final String normalized = normalizeHex32(value, field);
    if (!normalized.equals(value)) {
      throw new IllegalArgumentException(
          field + " must be canonical lowercase 0x-prefixed 32-byte hex");
    }
    return normalized;
  }

  private static void requireNativeEvmProverBundleHashRoleSeparation(
      final LinkedHashMap<String, String> roles) {
    final LinkedHashMap<String, String> seen = new LinkedHashMap<>();
    for (final Map.Entry<String, String> role : roles.entrySet()) {
      final String previous = seen.putIfAbsent(role.getValue(), role.getKey());
      if (previous != null) {
        throw new IllegalArgumentException(
            "nativeProverBundle hashes must be role-separated: "
                + role.getKey()
                + " matches "
                + previous);
      }
    }
  }

  private static String normalizeNativeEvmProverArtifactPath(
      final String value, final String field) {
    if (value == null || value.isEmpty()) {
      throw new IllegalArgumentException(field + " must be a non-empty relative POSIX path");
    }
    for (int index = 0; index < value.length(); index++) {
      final char ch = value.charAt(index);
      if (ch < 0x20 || ch == 0x7f) {
        throw new IllegalArgumentException(field + " must not contain control characters");
      }
    }
    if (value.startsWith("/") || value.indexOf('\\') >= 0) {
      throw new IllegalArgumentException(field + " must be a relative POSIX path");
    }
    final String[] segments = value.split("/", -1);
    for (final String segment : segments) {
      if (segment.isEmpty() || ".".equals(segment) || "..".equals(segment)) {
        throw new IllegalArgumentException(field + " must stay under the manifest directory");
      }
    }
    return value;
  }

  private static String normalizeOptionalHex32(final String value, final String field) {
    return "0x" + hexLower(hex32Bytes(value, field));
  }

  private static String normalizeNativeEvmProverParityHex32(
      final String value, final String field) {
    final String normalized = normalizeOptionalHex32(value, field);
    if (!normalized.equals(value)) {
      throw new IllegalArgumentException(
          field + " must be canonical lowercase 0x-prefixed 32-byte hex");
    }
    return normalized;
  }

  private static Groth16ProverArtifacts normalizeOptionalGroth16ProverArtifacts(
      final String proofArtifactHash, final String provingKeyHash) {
    if ((proofArtifactHash == null) != (provingKeyHash == null)) {
      throw new IllegalArgumentException(
          "proofArtifactHash and provingKeyHash must be supplied together");
    }
    if (proofArtifactHash == null) {
      return null;
    }
    return new Groth16ProverArtifacts(
        normalizeHex32(proofArtifactHash, "proofArtifactHash"),
        normalizeHex32(provingKeyHash, "provingKeyHash"));
  }

  private static ProofContext normalizeProofContext(
      final String statementHash, final String destinationBindingHash) {
    return new ProofContext(
        1,
        normalizeHex32(statementHash, "statementHash"),
        normalizeHex32(destinationBindingHash, "destinationBindingHash"));
  }

  private static String groth16SignalWord(final String label, final byte[] value) {
    final byte[] labelHash = keccak256(label.getBytes(StandardCharsets.UTF_8));
    final byte[] payload = new byte[labelHash.length + value.length];
    System.arraycopy(labelHash, 0, payload, 0, labelHash.length);
    System.arraycopy(value, 0, payload, labelHash.length, value.length);
    final BigInteger reduced = new BigInteger(1, keccak256(payload)).mod(BN254_SCALAR_FIELD_MODULUS);
    return "0x" + hexLower(toFixedBytes(reduced, 32));
  }

  private static byte[] keccak256(final byte[] input) {
    final KeccakDigest digest = new KeccakDigest(256);
    digest.update(input, 0, input.length);
    final byte[] out = new byte[32];
    digest.doFinal(out, 0);
    return out;
  }

  private static byte[] abiWordU32(final int value) {
    if (value < 0) {
      throw new IllegalArgumentException("domain id must not be negative");
    }
    final byte[] out = new byte[32];
    out[28] = (byte) ((value >>> 24) & 0xff);
    out[29] = (byte) ((value >>> 16) & 0xff);
    out[30] = (byte) ((value >>> 8) & 0xff);
    out[31] = (byte) (value & 0xff);
    return out;
  }

  private static byte[] abiWordU64(final BigInteger value) {
    BigInteger working = value;
    final byte[] out = new byte[32];
    for (int index = 31; index >= 24; index--) {
      out[index] = working.and(BigInteger.valueOf(0xffL)).byteValue();
      working = working.shiftRight(8);
    }
    return out;
  }

  private static byte[] abiWordU256(final int value) {
    if (value < 0) {
      throw new IllegalArgumentException("u256 value must not be negative");
    }
    int working = value;
    final byte[] out = new byte[32];
    for (int index = 31; index >= 0; index--) {
      out[index] = (byte) (working & 0xff);
      working >>>= 8;
      if (working == 0) {
        break;
      }
    }
    return out;
  }

  private static byte[][] messageTransparentPublicInputAbiWordBytes(
      final PublicInputsInput publicInputs) {
    canonicalPublicInputsBytes(publicInputs);
    return new byte[][] {
      nonZeroHex32Bytes(publicInputs.messageId(), "messageId"),
      nonZeroHex32Bytes(publicInputs.payloadHash(), "payloadHash"),
      abiWordU32(publicInputs.targetDomain()),
      nonZeroHex32Bytes(publicInputs.commitmentRoot(), "commitmentRoot"),
      abiWordU64(normalizeU64(publicInputs.finalityHeight(), "finalityHeight")),
      nonZeroHex32Bytes(publicInputs.finalityBlockHash(), "finalityBlockHash")
    };
  }

  private static BigInteger normalizeU64(final String value, final String field) {
    final String text = Objects.requireNonNull(value, field);
    if (!isCanonicalDecimalText(text)) {
      throw new IllegalArgumentException(field + " must be an unsigned integer");
    }
    final BigInteger numeric = new BigInteger(text);
    if (numeric.compareTo(MAX_U64) > 0) {
      throw new IllegalArgumentException(field + " must fit u64");
    }
    if (numeric.signum() == 0) {
      throw new IllegalArgumentException(field + " must not be zero");
    }
    return numeric;
  }

  private static boolean isCanonicalDecimalText(final String value) {
    if ("0".equals(value)) {
      return true;
    }
    if (value.isEmpty() || value.charAt(0) < '1' || value.charAt(0) > '9') {
      return false;
    }
    for (int i = 0; i < value.length(); i++) {
      final char ch = value.charAt(i);
      if (ch < '0' || ch > '9') {
        return false;
      }
    }
    return true;
  }

  private static void writeU32Le(final ByteArrayOutputStream out, final int value) {
    if (value < 0) {
      throw new IllegalArgumentException("u32 value must not be negative");
    }
    out.write(value & 0xff);
    out.write((value >>> 8) & 0xff);
    out.write((value >>> 16) & 0xff);
    out.write((value >>> 24) & 0xff);
  }

  private static void writeU64Le(final ByteArrayOutputStream out, final BigInteger value) {
    BigInteger working = value;
    for (int i = 0; i < 8; i++) {
      out.write(working.and(BigInteger.valueOf(0xffL)).intValue());
      working = working.shiftRight(8);
    }
  }

  private static byte[] toFixedBytes(final BigInteger value, final int length) {
    final byte[] raw = value.toByteArray();
    final byte[] out = new byte[length];
    final int copyLength = Math.min(raw.length, length);
    System.arraycopy(raw, raw.length - copyLength, out, length - copyLength, copyLength);
    return out;
  }

  private static void write(final ByteArrayOutputStream out, final byte[] bytes) {
    out.write(bytes, 0, bytes.length);
  }

  private static String hexLower(final byte[] bytes) {
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      builder.append(String.format("%02x", b & 0xff));
    }
    return builder.toString();
  }

  public interface WitnessProvider {
    ProofRequestInput resolveWitness(ProofRequestInput input);
  }

  /** Resolves manifest-declared native prover artifact paths from app-local storage. */
  public interface NativeEvmProverArtifactResolver {
    byte[] resolveArtifact(String path);
  }

  /** One SDK implementation row in an audited Ethereum mainnet native EVM prover bundle. */
  public record EthereumMainnetNativeEvmProverBundleSdkArtifact(
      String sdk,
      String implementation,
      String proofArtifactHash,
      String provingKeyHash,
      String implementationArtifact,
      String implementationHash) {
    public EthereumMainnetNativeEvmProverBundleSdkArtifact {
      if (sdk == null || sdk.isEmpty()) {
        throw new IllegalArgumentException("nativeSdkArtifacts.sdk must be non-empty");
      }
      if (!Objects.equals(ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1.get(sdk), implementation)) {
        throw new IllegalArgumentException(
            sdk + " implementation must match Ethereum native EVM prover bundle profile");
      }
      proofArtifactHash =
          normalizeNativeEvmProverBundleHex32(proofArtifactHash, "proofArtifactHash");
      provingKeyHash = normalizeNativeEvmProverBundleHex32(provingKeyHash, "provingKeyHash");
      if (implementationArtifact != null) {
        implementationArtifact =
            normalizeNativeEvmProverArtifactPath(implementationArtifact, "implementationArtifact");
      }
      implementationHash =
          normalizeNativeEvmProverBundleHex32(implementationHash, "implementationHash");
    }

    public EthereumMainnetNativeEvmProverBundleSdkArtifact(
        final String sdk,
        final String implementation,
        final String proofArtifactHash,
        final String provingKeyHash,
        final String implementationHash) {
      this(sdk, implementation, proofArtifactHash, provingKeyHash, null, implementationHash);
    }
  }

  /** Audited native-only EVM Groth16 prover bundle for Ethereum mainnet. */
  public record EthereumMainnetNativeEvmProverBundle(
      String schema,
      String bundleId,
      int domain,
      String chain,
      String proofBackend,
      String proofArtifact,
      String proofArtifactHash,
      String provingKey,
      String provingKeyHash,
      String verifierKey,
      String verifierKeyHash,
      String destinationBindingHash,
      boolean noWasm,
      boolean remoteProverRequired,
      String browserImplementation,
      String crossSdkFixtureParityArtifact,
      String nativeProverSelfTestArtifact,
      List<EthereumMainnetNativeEvmProverBundleSdkArtifact> nativeSdkArtifacts,
      Map<String, String> auditHashes) {
    public EthereumMainnetNativeEvmProverBundle {
      if (!NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1.equals(schema)) {
        throw new IllegalArgumentException("nativeProverBundle.schema is not supported");
      }
      if (!ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1.equals(bundleId)) {
        throw new IllegalArgumentException("nativeProverBundle.bundleId is not supported");
      }
      if (domain != DOMAIN_ETH) {
        throw new IllegalArgumentException("nativeProverBundle.domain must be ETH");
      }
      if (!"eth".equals(chain)) {
        throw new IllegalArgumentException("nativeProverBundle.chain must be eth");
      }
      if (!GROTH16_BN254_PROOF_BACKEND_V1.equals(proofBackend)) {
        throw new IllegalArgumentException(
            "nativeProverBundle.proofBackend must be evm-groth16-bn254-v1");
      }
      if (!noWasm) {
        throw new IllegalArgumentException("nativeProverBundle.noWasm must be true");
      }
      if (remoteProverRequired) {
        throw new IllegalArgumentException(
            "nativeProverBundle.remoteProverRequired must be false");
      }
      if (!"pure-typescript".equals(browserImplementation)) {
        throw new IllegalArgumentException(
            "nativeProverBundle.browserImplementation must be pure-typescript");
      }
      if (proofArtifact != null) {
        proofArtifact = normalizeNativeEvmProverArtifactPath(proofArtifact, "proofArtifact");
      }
      proofArtifactHash =
          normalizeNativeEvmProverBundleHex32(proofArtifactHash, "proofArtifactHash");
      if (provingKey != null) {
        provingKey = normalizeNativeEvmProverArtifactPath(provingKey, "provingKey");
      }
      provingKeyHash = normalizeNativeEvmProverBundleHex32(provingKeyHash, "provingKeyHash");
      if (verifierKey != null) {
        verifierKey = normalizeNativeEvmProverArtifactPath(verifierKey, "verifierKey");
      }
      verifierKeyHash =
          normalizeNativeEvmProverBundleHex32(verifierKeyHash, "verifierKeyHash");
      destinationBindingHash =
          normalizeNativeEvmProverBundleHex32(destinationBindingHash, "destinationBindingHash");
      if (crossSdkFixtureParityArtifact != null) {
        crossSdkFixtureParityArtifact =
            normalizeNativeEvmProverArtifactPath(
                crossSdkFixtureParityArtifact, "crossSdkFixtureParityArtifact");
      }
      if (nativeProverSelfTestArtifact != null) {
        nativeProverSelfTestArtifact =
            normalizeNativeEvmProverArtifactPath(
                nativeProverSelfTestArtifact, "nativeProverSelfTestArtifact");
      }
      if (auditHashes == null || auditHashes.isEmpty()) {
        throw new IllegalArgumentException("nativeProverBundle.auditHashes must be non-empty");
      }
      for (final String key : auditHashes.keySet()) {
        if (!ETH_NATIVE_EVM_PROVER_REQUIRED_AUDIT_HASHES_V1.contains(key)) {
          throw new IllegalArgumentException("auditHashes." + key + " is not expected");
        }
      }
      final LinkedHashMap<String, String> normalizedAuditHashes = new LinkedHashMap<>();
      for (final String key : ETH_NATIVE_EVM_PROVER_REQUIRED_AUDIT_HASHES_V1) {
        final String value = auditHashes.get(key);
        if (value == null) {
          throw new IllegalArgumentException("auditHashes." + key + " is required");
        }
        normalizedAuditHashes.put(
            key, normalizeNativeEvmProverBundleHex32(value, "auditHashes." + key));
      }
      auditHashes = Collections.unmodifiableMap(normalizedAuditHashes);
      if (nativeSdkArtifacts == null || nativeSdkArtifacts.isEmpty()) {
        throw new IllegalArgumentException("nativeSdkArtifacts must be non-empty");
      }
      final LinkedHashMap<String, EthereumMainnetNativeEvmProverBundleSdkArtifact> bySdk =
          new LinkedHashMap<>();
      for (final EthereumMainnetNativeEvmProverBundleSdkArtifact artifact : nativeSdkArtifacts) {
        if (bySdk.containsKey(artifact.sdk())) {
          throw new IllegalArgumentException(
              "nativeSdkArtifacts contains duplicate sdk: " + artifact.sdk());
        }
        if (!Objects.equals(artifact.proofArtifactHash(), proofArtifactHash)) {
          throw new IllegalArgumentException(
              artifact.sdk() + " proofArtifactHash must match bundle");
        }
        if (!Objects.equals(artifact.provingKeyHash(), provingKeyHash)) {
          throw new IllegalArgumentException(artifact.sdk() + " provingKeyHash must match bundle");
        }
        bySdk.put(artifact.sdk(), artifact);
      }
      for (final String sdk : ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1.keySet()) {
        if (!bySdk.containsKey(sdk)) {
          throw new IllegalArgumentException("nativeSdkArtifacts missing sdk: " + sdk);
        }
      }
      final LinkedHashMap<String, String> hashRoles = new LinkedHashMap<>();
      hashRoles.put("proofArtifactHash", proofArtifactHash);
      hashRoles.put("provingKeyHash", provingKeyHash);
      hashRoles.put("verifierKeyHash", verifierKeyHash);
      hashRoles.put("destinationBindingHash", destinationBindingHash);
      for (final EthereumMainnetNativeEvmProverBundleSdkArtifact artifact : bySdk.values()) {
        hashRoles.put(
            "nativeSdkArtifacts[" + artifact.sdk() + "].implementationHash",
            artifact.implementationHash());
      }
      for (final Map.Entry<String, String> entry : auditHashes.entrySet()) {
        hashRoles.put("auditHashes." + entry.getKey(), entry.getValue());
      }
      requireNativeEvmProverBundleHashRoleSeparation(hashRoles);
      nativeSdkArtifacts = Collections.unmodifiableList(new ArrayList<>(bySdk.values()));
    }

    public EthereumMainnetNativeEvmProverBundle(
        final String schema,
        final String bundleId,
        final int domain,
        final String chain,
        final String proofBackend,
        final String proofArtifact,
        final String proofArtifactHash,
        final String provingKey,
        final String provingKeyHash,
        final String verifierKey,
        final String verifierKeyHash,
        final String destinationBindingHash,
        final boolean noWasm,
        final boolean remoteProverRequired,
        final String browserImplementation,
        final List<EthereumMainnetNativeEvmProverBundleSdkArtifact> nativeSdkArtifacts,
        final Map<String, String> auditHashes) {
      this(
          schema,
          bundleId,
          domain,
          chain,
          proofBackend,
          proofArtifact,
          proofArtifactHash,
          provingKey,
          provingKeyHash,
          verifierKey,
          verifierKeyHash,
          destinationBindingHash,
          noWasm,
          remoteProverRequired,
          browserImplementation,
          null,
          null,
          nativeSdkArtifacts,
          auditHashes);
    }

    public EthereumMainnetNativeEvmProverBundle(
        final String proofArtifactHash,
        final String provingKeyHash,
        final String verifierKeyHash,
        final String destinationBindingHash,
        final List<EthereumMainnetNativeEvmProverBundleSdkArtifact> nativeSdkArtifacts,
        final Map<String, String> auditHashes) {
      this(
          NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1,
          ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1,
          DOMAIN_ETH,
          "eth",
          GROTH16_BN254_PROOF_BACKEND_V1,
          null,
          proofArtifactHash,
          null,
          provingKeyHash,
          null,
          verifierKeyHash,
          destinationBindingHash,
          true,
          false,
          "pure-typescript",
          null,
          null,
          nativeSdkArtifacts,
          auditHashes);
    }

    public EthereumMainnetNativeEvmProverArtifacts verifiedArtifacts(
        final byte[] proofArtifactBytes,
        final byte[] provingKeyBytes,
        final byte[] verifierKeyBytes) {
      return verifiedArtifacts(
          proofArtifactBytes, provingKeyBytes, verifierKeyBytes, null, null, null, null);
    }

    public EthereumMainnetNativeEvmProverArtifacts verifiedArtifacts(
        final byte[] proofArtifactBytes,
        final byte[] provingKeyBytes,
        final byte[] verifierKeyBytes,
        final String sdk,
        final byte[] implementationBytes) {
      return verifiedArtifacts(
          proofArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes,
          sdk,
          implementationBytes,
          null,
          null);
    }

    public EthereumMainnetNativeEvmProverArtifacts verifiedArtifacts(
        final byte[] proofArtifactBytes,
        final byte[] provingKeyBytes,
        final byte[] verifierKeyBytes,
        final String sdk,
        final byte[] implementationBytes,
        final byte[] crossSdkFixtureParityBytes) {
      return verifiedArtifacts(
          proofArtifactBytes,
          provingKeyBytes,
          verifierKeyBytes,
          sdk,
          implementationBytes,
          crossSdkFixtureParityBytes,
          null);
    }

    public EthereumMainnetNativeEvmProverArtifacts verifiedArtifacts(
        final byte[] proofArtifactBytes,
        final byte[] provingKeyBytes,
        final byte[] verifierKeyBytes,
        final String sdk,
        final byte[] implementationBytes,
        final byte[] crossSdkFixtureParityBytes,
        final byte[] nativeProverSelfTestBytes) {
      final String computedProofArtifactHash =
          sha256Hex(Objects.requireNonNull(proofArtifactBytes, "proofArtifactBytes"));
      if (!Objects.equals(computedProofArtifactHash, proofArtifactHash)) {
        throw new IllegalArgumentException(
            "proofArtifactBytes sha256 must match nativeProverBundle.proofArtifactHash");
      }
      final String computedProvingKeyHash =
          sha256Hex(Objects.requireNonNull(provingKeyBytes, "provingKeyBytes"));
      if (!Objects.equals(computedProvingKeyHash, provingKeyHash)) {
        throw new IllegalArgumentException(
            "provingKeyBytes sha256 must match nativeProverBundle.provingKeyHash");
      }
      final String computedVerifierKeyHash =
          sha256Hex(Objects.requireNonNull(verifierKeyBytes, "verifierKeyBytes"));
      if (!Objects.equals(computedVerifierKeyHash, verifierKeyHash)) {
        throw new IllegalArgumentException(
            "verifierKeyBytes sha256 must match nativeProverBundle.verifierKeyHash");
      }
      if (crossSdkFixtureParityBytes == null) {
        throw new IllegalArgumentException(
            "crossSdkFixtureParityBytes are required for nativeProverBundle parity binding");
      }
      final String crossSdkFixtureParityHash = sha256Hex(crossSdkFixtureParityBytes);
      if (!Objects.equals(
          crossSdkFixtureParityHash, auditHashes.get("cross_sdk_fixture_parity"))) {
        throw new IllegalArgumentException(
            "crossSdkFixtureParityBytes sha256 must match nativeProverBundle.auditHashes.cross_sdk_fixture_parity");
      }
      if (nativeProverSelfTestBytes == null) {
        throw new IllegalArgumentException(
            "nativeProverSelfTestBytes are required for nativeProverBundle self-test binding");
      }
      final String nativeProverSelfTestHash = sha256Hex(nativeProverSelfTestBytes);
      if (!Objects.equals(
          nativeProverSelfTestHash, auditHashes.get("native_prover_self_test"))) {
        throw new IllegalArgumentException(
            "nativeProverSelfTestBytes sha256 must match nativeProverBundle.auditHashes.native_prover_self_test");
      }
      requireNativeEvmProverProductionArtifactSize(proofArtifactBytes, "proofArtifactBytes");
      requireNativeEvmProverProductionArtifactSize(provingKeyBytes, "provingKeyBytes");
      requireNativeEvmProverProductionArtifactSize(verifierKeyBytes, "verifierKeyBytes");
      rejectNativeEvmProverForbiddenArtifactMarkers(proofArtifactBytes, "proofArtifactBytes");
      rejectNativeEvmProverForbiddenArtifactMarkers(provingKeyBytes, "provingKeyBytes");
      rejectNativeEvmProverForbiddenArtifactMarkers(verifierKeyBytes, "verifierKeyBytes");
      rejectNativeEvmProverForbiddenArtifactMarkers(
          crossSdkFixtureParityBytes, "crossSdkFixtureParityBytes");
      rejectNativeEvmProverForbiddenArtifactMarkers(
          nativeProverSelfTestBytes, "nativeProverSelfTestBytes");
      final EthereumMainnetNativeEvmProverParityFixture crossSdkFixtureParity =
          EthereumMainnetNativeEvmProverParityFixture.fromJsonBytes(
              crossSdkFixtureParityBytes, this);
      final EthereumMainnetNativeEvmProverSelfTestFixture nativeProverSelfTest =
          EthereumMainnetNativeEvmProverSelfTestFixture.fromJsonBytes(
              nativeProverSelfTestBytes, this);

      if (sdk == null || sdk.isEmpty()) {
        throw new IllegalArgumentException(
            "sdk must be a non-empty string for nativeProverBundle implementation binding");
      }
      if (implementationBytes == null) {
        throw new IllegalArgumentException(
            "implementationBytes are required for nativeProverBundle implementation binding");
      }
      EthereumMainnetNativeEvmProverBundleSdkArtifact artifact = null;
      for (final EthereumMainnetNativeEvmProverBundleSdkArtifact row : nativeSdkArtifacts) {
        if (Objects.equals(row.sdk(), sdk)) {
          artifact = row;
          break;
        }
      }
      if (artifact == null) {
        throw new IllegalArgumentException("nativeProverBundle has no artifact row for sdk: " + sdk);
      }
      final String implementationHash = sha256Hex(implementationBytes);
      if (!Objects.equals(implementationHash, artifact.implementationHash())) {
        throw new IllegalArgumentException(
            "implementationBytes sha256 must match nativeProverBundle implementationHash");
      }
      requireNativeEvmProverProductionArtifactSize(implementationBytes, "implementationBytes");
      rejectNativeEvmProverForbiddenArtifactMarkers(implementationBytes, "implementationBytes");
      final String implementation = artifact.implementation();
      return new EthereumMainnetNativeEvmProverArtifacts(
          NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1,
          this,
          computedProofArtifactHash,
          computedProvingKeyHash,
          computedVerifierKeyHash,
          crossSdkFixtureParityHash,
          crossSdkFixtureParity,
          nativeProverSelfTestHash,
          nativeProverSelfTest,
          sdk,
          implementation,
          implementationHash);
    }

    public EthereumMainnetNativeEvmProverArtifacts verifiedArtifacts(
        final String sdk, final NativeEvmProverArtifactResolver artifactResolver) {
      Objects.requireNonNull(artifactResolver, "artifactResolver");
      if (proofArtifact == null) {
        throw new IllegalArgumentException("proofArtifact is required");
      }
      if (provingKey == null) {
        throw new IllegalArgumentException("provingKey is required");
      }
      if (verifierKey == null) {
        throw new IllegalArgumentException("verifierKey is required");
      }
      if (crossSdkFixtureParityArtifact == null) {
        throw new IllegalArgumentException("crossSdkFixtureParityArtifact is required");
      }
      if (nativeProverSelfTestArtifact == null) {
        throw new IllegalArgumentException("nativeProverSelfTestArtifact is required");
      }
      EthereumMainnetNativeEvmProverBundleSdkArtifact artifact = null;
      for (final EthereumMainnetNativeEvmProverBundleSdkArtifact row : nativeSdkArtifacts) {
        if (Objects.equals(row.sdk(), sdk)) {
          artifact = row;
          break;
        }
      }
      if (artifact == null) {
        throw new IllegalArgumentException("nativeProverBundle has no artifact row for sdk: " + sdk);
      }
      if (artifact.implementationArtifact() == null) {
        throw new IllegalArgumentException("implementationArtifact is required");
      }
      return verifiedArtifacts(
          artifactResolver.resolveArtifact(proofArtifact),
          artifactResolver.resolveArtifact(provingKey),
          artifactResolver.resolveArtifact(verifierKey),
          sdk,
          artifactResolver.resolveArtifact(artifact.implementationArtifact()),
          artifactResolver.resolveArtifact(crossSdkFixtureParityArtifact),
          artifactResolver.resolveArtifact(nativeProverSelfTestArtifact));
    }

    public static EthereumMainnetNativeEvmProverBundle fromJson(final String json) {
      return fromJson(json, null);
    }

    public static EthereumMainnetNativeEvmProverBundle fromJson(
        final String json, final String expectedDestinationBindingHash) {
      final Object parsed;
      try {
        parsed = JsonParser.parse(Objects.requireNonNull(json, "json"));
      } catch (final IllegalStateException ex) {
        throw new IllegalArgumentException(
            "nativeProverBundle JSON is invalid: " + ex.getMessage(), ex);
      }
      return fromMap(
          expectManifestObject(parsed, "nativeProverBundle"),
          expectedDestinationBindingHash);
    }

    public static EthereumMainnetNativeEvmProverBundle fromJsonBytes(final byte[] payload) {
      return fromJsonBytes(payload, null);
    }

    public static EthereumMainnetNativeEvmProverBundle fromJsonBytes(
        final byte[] payload, final String expectedDestinationBindingHash) {
      return fromJson(
          new String(Objects.requireNonNull(payload, "payload"), StandardCharsets.UTF_8),
          expectedDestinationBindingHash);
    }

    public static EthereumMainnetNativeEvmProverBundle fromMap(
        final Map<String, Object> manifest) {
      return fromMap(manifest, null);
    }

    public static EthereumMainnetNativeEvmProverBundle fromMap(
        final Map<String, Object> manifest, final String expectedDestinationBindingHash) {
      Objects.requireNonNull(manifest, "manifest");
      requireManifestKeys(
          manifest, "nativeProverBundle", NATIVE_EVM_PROVER_BUNDLE_MANIFEST_KEYS);
      final String proofArtifactHash =
          manifestString(
              manifestField(
                  manifest,
                  "proofArtifactHash",
                  "proofArtifactHash",
                  "proof_artifact_hash",
                  "proverArtifactHash",
                  "prover_artifact_hash",
                  "circuitArtifactHash",
                  "circuit_artifact_hash"),
              "proofArtifactHash");
      final String provingKeyHash =
          manifestString(
              manifestField(manifest, "provingKeyHash", "provingKeyHash", "proving_key_hash"),
              "provingKeyHash");
      final EthereumMainnetNativeEvmProverBundle bundle =
          new EthereumMainnetNativeEvmProverBundle(
              manifestString(manifestField(manifest, "schema", "schema"), "schema"),
              manifestString(manifestField(manifest, "bundleId", "bundleId", "bundle_id"), "bundleId"),
              manifestDomain(manifestField(manifest, "domain", "domain"), "domain"),
              manifestString(manifestField(manifest, "chain", "chain"), "chain"),
              manifestString(
                  manifestField(
                      manifest, "proofBackend", "proofBackend", "proof_backend", "backend"),
                  "proofBackend"),
              manifestString(
                  manifestField(
                      manifest,
                      "proofArtifact",
                      "proofArtifact",
                      "proof_artifact",
                      "proverArtifact",
                      "prover_artifact",
                      "circuitArtifact",
                      "circuit_artifact"),
                  "proofArtifact"),
              proofArtifactHash,
              manifestString(
                  manifestField(manifest, "provingKey", "provingKey", "proving_key"),
                  "provingKey"),
              provingKeyHash,
              manifestString(
                  manifestField(manifest, "verifierKey", "verifierKey", "verifier_key"),
                  "verifierKey"),
              manifestString(
                  manifestField(
                      manifest, "verifierKeyHash", "verifierKeyHash", "verifier_key_hash"),
                  "verifierKeyHash"),
              manifestString(
                  manifestField(
                      manifest,
                      "destinationBindingHash",
                      "destinationBindingHash",
                      "destination_binding_hash"),
                  "destinationBindingHash"),
              manifestBoolean(manifestField(manifest, "noWasm", "noWasm", "no_wasm"), "noWasm"),
              manifestBoolean(
                  manifestField(
                      manifest,
                      "remoteProverRequired",
                      "remoteProverRequired",
                      "remote_prover_required"),
                  "remoteProverRequired"),
              manifestString(
                  manifestField(
                      manifest,
                      "browserImplementation",
                      "browserImplementation",
                      "browser_implementation"),
                  "browserImplementation"),
              manifestString(
                  manifestField(
                      manifest,
                      "crossSdkFixtureParityArtifact",
                      "crossSdkFixtureParityArtifact",
                      "cross_sdk_fixture_parity_artifact"),
                  "crossSdkFixtureParityArtifact"),
              manifestString(
                  manifestField(
                      manifest,
                      "nativeProverSelfTestArtifact",
                      "nativeProverSelfTestArtifact",
                      "native_prover_self_test_artifact",
                      "selfTestArtifact",
                      "self_test_artifact"),
                  "nativeProverSelfTestArtifact"),
              manifestSdkArtifacts(
                  manifestField(
                      manifest,
                      "nativeSdkArtifacts",
                      "nativeSdkArtifacts",
                      "native_sdk_artifacts",
                      "sdkArtifacts",
                      "sdk_artifacts"),
                  proofArtifactHash,
                  provingKeyHash),
              manifestStringMap(
                  manifestField(manifest, "auditHashes", "auditHashes", "audit_hashes"),
                  "auditHashes"));
      if (expectedDestinationBindingHash != null
          && !Objects.equals(
              normalizeNativeEvmProverBundleHex32(
                  expectedDestinationBindingHash, "expectedDestinationBindingHash"),
              bundle.destinationBindingHash())) {
        throw new IllegalArgumentException(
            "nativeProverBundle.destinationBindingHash must match destinationBinding");
      }
      return bundle;
    }

    public ProofRequestInput applyTo(final ProofRequestInput input) {
      Objects.requireNonNull(input, "input");
      if (!Objects.equals(normalizeHex32(input.destinationBindingHash(), "destinationBindingHash"), destinationBindingHash)) {
        throw new IllegalArgumentException(
            "nativeProverBundle.destinationBindingHash must match destinationBinding");
      }
      if (input.destinationBinding() != null
          && !Objects.equals(
              normalizeHex32(input.destinationBinding().verifierKeyHash, "verifierKeyHash"),
              verifierKeyHash)) {
        throw new IllegalArgumentException(
            "nativeProverBundle.verifierKeyHash must match destinationBinding");
      }
      if (input.proofArtifactHash() != null
          && !Objects.equals(normalizeHex32(input.proofArtifactHash(), "proofArtifactHash"), proofArtifactHash)) {
        throw new IllegalArgumentException(
            "nativeProverBundle.proofArtifactHash must match proof request");
      }
      if (input.provingKeyHash() != null
          && !Objects.equals(normalizeHex32(input.provingKeyHash(), "provingKeyHash"), provingKeyHash)) {
        throw new IllegalArgumentException(
            "nativeProverBundle.provingKeyHash must match proof request");
      }
      if ((input.proofArtifactHash() == null) != (input.provingKeyHash() == null)) {
        throw new IllegalArgumentException(
            "proofArtifactHash and provingKeyHash must be supplied together");
      }
      return new ProofRequestInput(
          input.publicInputs(),
          input.bundleBytes(),
          input.sourceProofBytes(),
          input.statementHash(),
          destinationBindingHash,
          input.backend(),
          input.sourceDomain(),
          input.destinationBinding(),
          proofArtifactHash,
          provingKeyHash);
    }
  }

  public record EthereumMainnetNativeEvmProverParitySdkResult(
      String receiptProofHash,
      String sourceProofHash,
      String destinationBindingHash,
      List<String> publicSignalWords,
      String calldataHash,
      String toriiSubmitPayloadHash) {
    public EthereumMainnetNativeEvmProverParitySdkResult {
      receiptProofHash = normalizeNativeEvmProverBundleHex32(receiptProofHash, "receiptProofHash");
      sourceProofHash = normalizeNativeEvmProverBundleHex32(sourceProofHash, "sourceProofHash");
      destinationBindingHash = normalizeNativeEvmProverParityHex32(destinationBindingHash, "destinationBindingHash");
      if (publicSignalWords == null || publicSignalWords.size() != 9) {
        throw new IllegalArgumentException("publicSignalWords must contain 9 words");
      }
      final ArrayList<String> signals = new ArrayList<>();
      for (int index = 0; index < publicSignalWords.size(); index++) {
        signals.add(normalizeNativeEvmProverParityHex32(publicSignalWords.get(index), "publicSignalWords[" + index + "]"));
      }
      publicSignalWords = Collections.unmodifiableList(signals);
      calldataHash = normalizeNativeEvmProverBundleHex32(calldataHash, "calldataHash");
      toriiSubmitPayloadHash =
          normalizeNativeEvmProverBundleHex32(toriiSubmitPayloadHash, "toriiSubmitPayloadHash");
    }
  }

  public record EthereumMainnetNativeEvmProverParityFixture(
      String schema,
      int domain,
      String chain,
      String proofBackend,
      String proofArtifactHash,
      String provingKeyHash,
      String verifierKeyHash,
      String destinationBindingHash,
      String receiptProofHash,
      String sourceProofHash,
      List<String> publicSignalWords,
      String calldataHash,
      String toriiSubmitPayloadHash,
      Map<String, EthereumMainnetNativeEvmProverParitySdkResult> sdkResults) {
    public EthereumMainnetNativeEvmProverParityFixture(
        final EthereumMainnetNativeEvmProverBundle nativeProverBundle,
        final String schema,
        final int domain,
        final String chain,
        final String proofBackend,
        final String proofArtifactHash,
        final String provingKeyHash,
        final String verifierKeyHash,
        final String destinationBindingHash,
        final String receiptProofHash,
        final String sourceProofHash,
        final List<String> publicSignalWords,
        final String calldataHash,
        final String toriiSubmitPayloadHash,
        final Map<String, EthereumMainnetNativeEvmProverParitySdkResult> sdkResults) {
      this(
          schema,
          domain,
          chain,
          proofBackend,
          proofArtifactHash,
          provingKeyHash,
          verifierKeyHash,
          destinationBindingHash,
          receiptProofHash,
          sourceProofHash,
          publicSignalWords,
          calldataHash,
          toriiSubmitPayloadHash,
          sdkResults);
      Objects.requireNonNull(nativeProverBundle, "nativeProverBundle");
      if (!Objects.equals(this.proofArtifactHash, nativeProverBundle.proofArtifactHash())) {
        throw new IllegalArgumentException(
            "nativeProverParityFixture.proofArtifactHash must match nativeProverBundle");
      }
      if (!Objects.equals(this.provingKeyHash, nativeProverBundle.provingKeyHash())) {
        throw new IllegalArgumentException(
            "nativeProverParityFixture.provingKeyHash must match nativeProverBundle");
      }
      if (!Objects.equals(this.verifierKeyHash, nativeProverBundle.verifierKeyHash())) {
        throw new IllegalArgumentException(
            "nativeProverParityFixture.verifierKeyHash must match nativeProverBundle");
      }
      if (!Objects.equals(this.destinationBindingHash, nativeProverBundle.destinationBindingHash())) {
        throw new IllegalArgumentException(
            "nativeProverParityFixture.destinationBindingHash must match nativeProverBundle");
      }
      if (this.domain != DOMAIN_ETH || this.domain != nativeProverBundle.domain()) {
        throw new IllegalArgumentException(
            "nativeProverParityFixture.domain must match nativeProverBundle");
      }
      if (!Objects.equals(this.chain, nativeProverBundle.chain())) {
        throw new IllegalArgumentException(
            "nativeProverParityFixture.chain must match nativeProverBundle");
      }
      if (!Objects.equals(this.proofBackend, nativeProverBundle.proofBackend())) {
        throw new IllegalArgumentException(
            "nativeProverParityFixture.proofBackend must match nativeProverBundle");
      }
    }

    public EthereumMainnetNativeEvmProverParityFixture {
      if (!ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1.equals(schema)) {
        throw new IllegalArgumentException("nativeProverParityFixture.schema is not supported");
      }
      if (domain != DOMAIN_ETH) {
        throw new IllegalArgumentException("nativeProverParityFixture.domain must be ETH");
      }
      if (!"eth".equals(chain)) {
        throw new IllegalArgumentException("nativeProverParityFixture.chain must be eth");
      }
      if (!GROTH16_BN254_PROOF_BACKEND_V1.equals(proofBackend)) {
        throw new IllegalArgumentException(
            "nativeProverParityFixture.proofBackend must be evm-groth16-bn254-v1");
      }
      proofArtifactHash =
          normalizeNativeEvmProverBundleHex32(proofArtifactHash, "proofArtifactHash");
      provingKeyHash = normalizeNativeEvmProverBundleHex32(provingKeyHash, "provingKeyHash");
      verifierKeyHash = normalizeNativeEvmProverBundleHex32(verifierKeyHash, "verifierKeyHash");
      destinationBindingHash = normalizeNativeEvmProverParityHex32(destinationBindingHash, "destinationBindingHash");
      receiptProofHash = normalizeNativeEvmProverBundleHex32(receiptProofHash, "receiptProofHash");
      sourceProofHash = normalizeNativeEvmProverBundleHex32(sourceProofHash, "sourceProofHash");
      if (publicSignalWords == null || publicSignalWords.size() != 9) {
        throw new IllegalArgumentException("publicSignalWords must contain 9 words");
      }
      final ArrayList<String> signals = new ArrayList<>();
      for (int index = 0; index < publicSignalWords.size(); index++) {
        signals.add(normalizeNativeEvmProverParityHex32(publicSignalWords.get(index), "publicSignalWords[" + index + "]"));
      }
      publicSignalWords = Collections.unmodifiableList(signals);
      calldataHash = normalizeNativeEvmProverBundleHex32(calldataHash, "calldataHash");
      toriiSubmitPayloadHash =
          normalizeNativeEvmProverBundleHex32(toriiSubmitPayloadHash, "toriiSubmitPayloadHash");
      if (sdkResults == null || !sdkResults.keySet().equals(ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1.keySet())) {
        throw new IllegalArgumentException("sdkResults must contain exactly the required SDKs");
      }
      final LinkedHashMap<String, EthereumMainnetNativeEvmProverParitySdkResult> normalizedResults =
          new LinkedHashMap<>();
      for (final String sdk : ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1.keySet()) {
        final EthereumMainnetNativeEvmProverParitySdkResult result = sdkResults.get(sdk);
        if (result == null) {
          throw new IllegalArgumentException("sdkResults missing sdk: " + sdk);
        }
        if (!Objects.equals(result.receiptProofHash(), receiptProofHash)) {
          throw new IllegalArgumentException("sdkResults." + sdk + ".receiptProofHash must match receiptProofHash");
        }
        if (!Objects.equals(result.sourceProofHash(), sourceProofHash)) {
          throw new IllegalArgumentException("sdkResults." + sdk + ".sourceProofHash must match sourceProofHash");
        }
        if (!Objects.equals(result.destinationBindingHash(), destinationBindingHash)) {
          throw new IllegalArgumentException(
              "sdkResults." + sdk + ".destinationBindingHash must match destinationBindingHash");
        }
        if (!Objects.equals(result.publicSignalWords(), publicSignalWords)) {
          throw new IllegalArgumentException(
              "sdkResults." + sdk + ".publicSignalWords must match publicSignalWords");
        }
        if (!Objects.equals(result.calldataHash(), calldataHash)) {
          throw new IllegalArgumentException("sdkResults." + sdk + ".calldataHash must match calldataHash");
        }
        if (!Objects.equals(result.toriiSubmitPayloadHash(), toriiSubmitPayloadHash)) {
          throw new IllegalArgumentException(
              "sdkResults." + sdk + ".toriiSubmitPayloadHash must match toriiSubmitPayloadHash");
        }
        normalizedResults.put(sdk, result);
      }
      sdkResults = Collections.unmodifiableMap(normalizedResults);
    }

    public static EthereumMainnetNativeEvmProverParityFixture fromJson(
        final String json, final EthereumMainnetNativeEvmProverBundle nativeProverBundle) {
      final Object parsed;
      try {
        parsed = JsonParser.parse(Objects.requireNonNull(json, "json"));
      } catch (final IllegalStateException ex) {
        throw new IllegalArgumentException(
            "nativeProverParityFixture JSON is invalid: " + ex.getMessage(), ex);
      }
      return fromMap(expectManifestObject(parsed, "nativeProverParityFixture"), nativeProverBundle);
    }

    public static EthereumMainnetNativeEvmProverParityFixture fromJsonBytes(
        final byte[] payload, final EthereumMainnetNativeEvmProverBundle nativeProverBundle) {
      return fromJson(new String(Objects.requireNonNull(payload, "payload"), StandardCharsets.UTF_8), nativeProverBundle);
    }

    public static EthereumMainnetNativeEvmProverParityFixture fromMap(
        final Map<String, Object> fixture,
        final EthereumMainnetNativeEvmProverBundle nativeProverBundle) {
      Objects.requireNonNull(fixture, "fixture");
      requireManifestKeys(
          fixture, "nativeProverParityFixture", NATIVE_EVM_PROVER_PARITY_FIXTURE_KEYS);
      final List<String> publicSignalWords =
          manifestStringList(
              manifestField(fixture, "publicSignalWords", "publicSignalWords", "public_signal_words"),
              "publicSignalWords");
      final Map<String, Object> sdkResultsInput =
          expectManifestObject(manifestField(fixture, "sdkResults", "sdkResults", "sdk_results"), "sdkResults");
      final LinkedHashMap<String, EthereumMainnetNativeEvmProverParitySdkResult> sdkResults =
          new LinkedHashMap<>();
      for (final String sdk : sdkResultsInput.keySet()) {
        final Map<String, Object> result = expectManifestObject(sdkResultsInput.get(sdk), "sdkResults." + sdk);
        requireManifestKeys(
            result, "sdkResults." + sdk, NATIVE_EVM_PROVER_PARITY_SDK_RESULT_KEYS);
        sdkResults.put(
            sdk,
            new EthereumMainnetNativeEvmProverParitySdkResult(
                manifestString(
                    manifestField(result, "sdkResults." + sdk + ".receiptProofHash", "receiptProofHash", "receipt_proof_hash"),
                    "sdkResults." + sdk + ".receiptProofHash"),
                manifestString(
                    manifestField(result, "sdkResults." + sdk + ".sourceProofHash", "sourceProofHash", "source_proof_hash"),
                    "sdkResults." + sdk + ".sourceProofHash"),
                manifestString(
                    manifestField(
                        result,
                        "sdkResults." + sdk + ".destinationBindingHash",
                        "destinationBindingHash",
                        "destination_binding_hash"),
                    "sdkResults." + sdk + ".destinationBindingHash"),
                manifestStringList(
                    manifestField(
                        result,
                        "sdkResults." + sdk + ".publicSignalWords",
                        "publicSignalWords",
                        "public_signal_words"),
                    "sdkResults." + sdk + ".publicSignalWords"),
                manifestString(
                    manifestField(result, "sdkResults." + sdk + ".calldataHash", "calldataHash", "calldata_hash"),
                    "sdkResults." + sdk + ".calldataHash"),
                manifestString(
                    manifestField(
                        result,
                        "sdkResults." + sdk + ".toriiSubmitPayloadHash",
                        "toriiSubmitPayloadHash",
                        "torii_submit_payload_hash"),
                    "sdkResults." + sdk + ".toriiSubmitPayloadHash")));
      }
      return new EthereumMainnetNativeEvmProverParityFixture(
          nativeProverBundle,
          manifestString(manifestField(fixture, "schema", "schema"), "schema"),
          manifestDomain(manifestField(fixture, "domain", "domain"), "domain"),
          manifestString(manifestField(fixture, "chain", "chain"), "chain"),
          manifestString(
              manifestField(fixture, "proofBackend", "proofBackend", "proof_backend", "backend"),
              "proofBackend"),
          manifestString(
              manifestField(
                  fixture,
                  "proofArtifactHash",
                  "proofArtifactHash",
                  "proof_artifact_hash",
                  "proverArtifactHash",
                  "prover_artifact_hash",
                  "circuitArtifactHash",
                  "circuit_artifact_hash"),
              "proofArtifactHash"),
          manifestString(
              manifestField(fixture, "provingKeyHash", "provingKeyHash", "proving_key_hash"),
              "provingKeyHash"),
          manifestString(
              manifestField(fixture, "verifierKeyHash", "verifierKeyHash", "verifier_key_hash"),
              "verifierKeyHash"),
          manifestString(
              manifestField(
                  fixture,
                  "destinationBindingHash",
                  "destinationBindingHash",
                  "destination_binding_hash"),
              "destinationBindingHash"),
          manifestString(
              manifestField(fixture, "receiptProofHash", "receiptProofHash", "receipt_proof_hash"),
              "receiptProofHash"),
          manifestString(
              manifestField(fixture, "sourceProofHash", "sourceProofHash", "source_proof_hash"),
              "sourceProofHash"),
          publicSignalWords,
          manifestString(
              manifestField(fixture, "calldataHash", "calldataHash", "calldata_hash"),
              "calldataHash"),
          manifestString(
              manifestField(
                  fixture,
                  "toriiSubmitPayloadHash",
                  "toriiSubmitPayloadHash",
                  "torii_submit_payload_hash"),
              "toriiSubmitPayloadHash"),
          sdkResults);
    }
  }

  public record EthereumMainnetNativeEvmProverSelfTestSdkResult(
      String requestHash,
      String witnessHash,
      String sourceProofHash,
      String proofHash,
      List<String> publicSignalWords,
      String calldataHash,
      String toriiSubmitPayloadHash) {
    public EthereumMainnetNativeEvmProverSelfTestSdkResult {
      requestHash = normalizeNativeEvmProverBundleHex32(requestHash, "requestHash");
      witnessHash = normalizeNativeEvmProverBundleHex32(witnessHash, "witnessHash");
      sourceProofHash = normalizeNativeEvmProverBundleHex32(sourceProofHash, "sourceProofHash");
      proofHash = normalizeNativeEvmProverBundleHex32(proofHash, "proofHash");
      if (publicSignalWords == null || publicSignalWords.size() != 9) {
        throw new IllegalArgumentException("publicSignalWords must contain 9 words");
      }
      final ArrayList<String> signals = new ArrayList<>();
      for (int index = 0; index < publicSignalWords.size(); index++) {
        signals.add(
            normalizeNativeEvmProverParityHex32(
                publicSignalWords.get(index), "publicSignalWords[" + index + "]"));
      }
      publicSignalWords = Collections.unmodifiableList(signals);
      calldataHash = normalizeNativeEvmProverBundleHex32(calldataHash, "calldataHash");
      toriiSubmitPayloadHash =
          normalizeNativeEvmProverBundleHex32(toriiSubmitPayloadHash, "toriiSubmitPayloadHash");
    }
  }

  public record EthereumMainnetNativeEvmProverSelfTestFixture(
      String schema,
      int domain,
      String chain,
      String proofBackend,
      String proofArtifactHash,
      String provingKeyHash,
      String verifierKeyHash,
      String destinationBindingHash,
      String requestHash,
      String witnessHash,
      String sourceProofHash,
      String proofHash,
      List<String> publicSignalWords,
      String calldataHash,
      String toriiSubmitPayloadHash,
      Map<String, EthereumMainnetNativeEvmProverSelfTestSdkResult> sdkResults) {
    public EthereumMainnetNativeEvmProverSelfTestFixture(
        final EthereumMainnetNativeEvmProverBundle nativeProverBundle,
        final String schema,
        final int domain,
        final String chain,
        final String proofBackend,
        final String proofArtifactHash,
        final String provingKeyHash,
        final String verifierKeyHash,
        final String destinationBindingHash,
        final String requestHash,
        final String witnessHash,
        final String sourceProofHash,
        final String proofHash,
        final List<String> publicSignalWords,
        final String calldataHash,
        final String toriiSubmitPayloadHash,
        final Map<String, EthereumMainnetNativeEvmProverSelfTestSdkResult> sdkResults) {
      this(
          schema,
          domain,
          chain,
          proofBackend,
          proofArtifactHash,
          provingKeyHash,
          verifierKeyHash,
          destinationBindingHash,
          requestHash,
          witnessHash,
          sourceProofHash,
          proofHash,
          publicSignalWords,
          calldataHash,
          toriiSubmitPayloadHash,
          sdkResults);
      Objects.requireNonNull(nativeProverBundle, "nativeProverBundle");
      if (!Objects.equals(this.proofArtifactHash, nativeProverBundle.proofArtifactHash())) {
        throw new IllegalArgumentException(
            "nativeProverSelfTestFixture.proofArtifactHash must match nativeProverBundle");
      }
      if (!Objects.equals(this.provingKeyHash, nativeProverBundle.provingKeyHash())) {
        throw new IllegalArgumentException(
            "nativeProverSelfTestFixture.provingKeyHash must match nativeProverBundle");
      }
      if (!Objects.equals(this.verifierKeyHash, nativeProverBundle.verifierKeyHash())) {
        throw new IllegalArgumentException(
            "nativeProverSelfTestFixture.verifierKeyHash must match nativeProverBundle");
      }
      if (!Objects.equals(this.destinationBindingHash, nativeProverBundle.destinationBindingHash())) {
        throw new IllegalArgumentException(
            "nativeProverSelfTestFixture.destinationBindingHash must match nativeProverBundle");
      }
      if (this.domain != DOMAIN_ETH || this.domain != nativeProverBundle.domain()) {
        throw new IllegalArgumentException(
            "nativeProverSelfTestFixture.domain must match nativeProverBundle");
      }
      if (!Objects.equals(this.chain, nativeProverBundle.chain())) {
        throw new IllegalArgumentException(
            "nativeProverSelfTestFixture.chain must match nativeProverBundle");
      }
      if (!Objects.equals(this.proofBackend, nativeProverBundle.proofBackend())) {
        throw new IllegalArgumentException(
            "nativeProverSelfTestFixture.proofBackend must match nativeProverBundle");
      }
    }

    public EthereumMainnetNativeEvmProverSelfTestFixture {
      if (!ETH_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1.equals(schema)) {
        throw new IllegalArgumentException("nativeProverSelfTestFixture.schema is not supported");
      }
      if (domain != DOMAIN_ETH) {
        throw new IllegalArgumentException("nativeProverSelfTestFixture.domain must be ETH");
      }
      if (!"eth".equals(chain)) {
        throw new IllegalArgumentException("nativeProverSelfTestFixture.chain must be eth");
      }
      if (!GROTH16_BN254_PROOF_BACKEND_V1.equals(proofBackend)) {
        throw new IllegalArgumentException(
            "nativeProverSelfTestFixture.proofBackend must be evm-groth16-bn254-v1");
      }
      proofArtifactHash =
          normalizeNativeEvmProverBundleHex32(proofArtifactHash, "proofArtifactHash");
      provingKeyHash = normalizeNativeEvmProverBundleHex32(provingKeyHash, "provingKeyHash");
      verifierKeyHash = normalizeNativeEvmProverBundleHex32(verifierKeyHash, "verifierKeyHash");
      destinationBindingHash =
          normalizeNativeEvmProverParityHex32(destinationBindingHash, "destinationBindingHash");
      requestHash = normalizeNativeEvmProverBundleHex32(requestHash, "requestHash");
      witnessHash = normalizeNativeEvmProverBundleHex32(witnessHash, "witnessHash");
      sourceProofHash = normalizeNativeEvmProverBundleHex32(sourceProofHash, "sourceProofHash");
      proofHash = normalizeNativeEvmProverBundleHex32(proofHash, "proofHash");
      if (publicSignalWords == null || publicSignalWords.size() != 9) {
        throw new IllegalArgumentException("publicSignalWords must contain 9 words");
      }
      final ArrayList<String> signals = new ArrayList<>();
      for (int index = 0; index < publicSignalWords.size(); index++) {
        signals.add(
            normalizeNativeEvmProverParityHex32(
                publicSignalWords.get(index), "publicSignalWords[" + index + "]"));
      }
      publicSignalWords = Collections.unmodifiableList(signals);
      calldataHash = normalizeNativeEvmProverBundleHex32(calldataHash, "calldataHash");
      toriiSubmitPayloadHash =
          normalizeNativeEvmProverBundleHex32(toriiSubmitPayloadHash, "toriiSubmitPayloadHash");
      if (sdkResults == null
          || !sdkResults.keySet().equals(ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1.keySet())) {
        throw new IllegalArgumentException("sdkResults must contain exactly the required SDKs");
      }
      final LinkedHashMap<String, EthereumMainnetNativeEvmProverSelfTestSdkResult>
          normalizedResults = new LinkedHashMap<>();
      for (final String sdk : ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1.keySet()) {
        final EthereumMainnetNativeEvmProverSelfTestSdkResult result = sdkResults.get(sdk);
        if (result == null) {
          throw new IllegalArgumentException("sdkResults missing sdk: " + sdk);
        }
        if (!Objects.equals(result.requestHash(), requestHash)) {
          throw new IllegalArgumentException(
              "sdkResults." + sdk + ".requestHash must match requestHash");
        }
        if (!Objects.equals(result.witnessHash(), witnessHash)) {
          throw new IllegalArgumentException(
              "sdkResults." + sdk + ".witnessHash must match witnessHash");
        }
        if (!Objects.equals(result.sourceProofHash(), sourceProofHash)) {
          throw new IllegalArgumentException(
              "sdkResults." + sdk + ".sourceProofHash must match sourceProofHash");
        }
        if (!Objects.equals(result.proofHash(), proofHash)) {
          throw new IllegalArgumentException(
              "sdkResults." + sdk + ".proofHash must match proofHash");
        }
        if (!Objects.equals(result.publicSignalWords(), publicSignalWords)) {
          throw new IllegalArgumentException(
              "sdkResults." + sdk + ".publicSignalWords must match publicSignalWords");
        }
        if (!Objects.equals(result.calldataHash(), calldataHash)) {
          throw new IllegalArgumentException(
              "sdkResults." + sdk + ".calldataHash must match calldataHash");
        }
        if (!Objects.equals(result.toriiSubmitPayloadHash(), toriiSubmitPayloadHash)) {
          throw new IllegalArgumentException(
              "sdkResults." + sdk + ".toriiSubmitPayloadHash must match toriiSubmitPayloadHash");
        }
        normalizedResults.put(sdk, result);
      }
      sdkResults = Collections.unmodifiableMap(normalizedResults);
    }

    public static EthereumMainnetNativeEvmProverSelfTestFixture fromJson(
        final String json, final EthereumMainnetNativeEvmProverBundle nativeProverBundle) {
      final Object parsed;
      try {
        parsed = JsonParser.parse(Objects.requireNonNull(json, "json"));
      } catch (final IllegalStateException ex) {
        throw new IllegalArgumentException(
            "nativeProverSelfTestFixture JSON is invalid: " + ex.getMessage(), ex);
      }
      return fromMap(expectManifestObject(parsed, "nativeProverSelfTestFixture"), nativeProverBundle);
    }

    public static EthereumMainnetNativeEvmProverSelfTestFixture fromJsonBytes(
        final byte[] payload, final EthereumMainnetNativeEvmProverBundle nativeProverBundle) {
      return fromJson(
          new String(Objects.requireNonNull(payload, "payload"), StandardCharsets.UTF_8),
          nativeProverBundle);
    }

    public static EthereumMainnetNativeEvmProverSelfTestFixture fromMap(
        final Map<String, Object> fixture,
        final EthereumMainnetNativeEvmProverBundle nativeProverBundle) {
      Objects.requireNonNull(fixture, "fixture");
      requireManifestKeys(
          fixture, "nativeProverSelfTestFixture", NATIVE_EVM_PROVER_SELF_TEST_FIXTURE_KEYS);
      final List<String> publicSignalWords =
          manifestStringList(
              manifestField(fixture, "publicSignalWords", "publicSignalWords", "public_signal_words"),
              "publicSignalWords");
      final Map<String, Object> sdkResultsInput =
          expectManifestObject(
              manifestField(fixture, "sdkResults", "sdkResults", "sdk_results"), "sdkResults");
      final LinkedHashMap<String, EthereumMainnetNativeEvmProverSelfTestSdkResult> sdkResults =
          new LinkedHashMap<>();
      for (final String sdk : sdkResultsInput.keySet()) {
        final Map<String, Object> result =
            expectManifestObject(sdkResultsInput.get(sdk), "sdkResults." + sdk);
        requireManifestKeys(
            result, "sdkResults." + sdk, NATIVE_EVM_PROVER_SELF_TEST_SDK_RESULT_KEYS);
        sdkResults.put(
            sdk,
            new EthereumMainnetNativeEvmProverSelfTestSdkResult(
                manifestString(
                    manifestField(
                        result, "sdkResults." + sdk + ".requestHash", "requestHash", "request_hash"),
                    "sdkResults." + sdk + ".requestHash"),
                manifestString(
                    manifestField(
                        result, "sdkResults." + sdk + ".witnessHash", "witnessHash", "witness_hash"),
                    "sdkResults." + sdk + ".witnessHash"),
                manifestString(
                    manifestField(
                        result,
                        "sdkResults." + sdk + ".sourceProofHash",
                        "sourceProofHash",
                        "source_proof_hash"),
                    "sdkResults." + sdk + ".sourceProofHash"),
                manifestString(
                    manifestField(result, "sdkResults." + sdk + ".proofHash", "proofHash", "proof_hash"),
                    "sdkResults." + sdk + ".proofHash"),
                manifestStringList(
                    manifestField(
                        result,
                        "sdkResults." + sdk + ".publicSignalWords",
                        "publicSignalWords",
                        "public_signal_words"),
                    "sdkResults." + sdk + ".publicSignalWords"),
                manifestString(
                    manifestField(result, "sdkResults." + sdk + ".calldataHash", "calldataHash", "calldata_hash"),
                    "sdkResults." + sdk + ".calldataHash"),
                manifestString(
                    manifestField(
                        result,
                        "sdkResults." + sdk + ".toriiSubmitPayloadHash",
                        "toriiSubmitPayloadHash",
                        "torii_submit_payload_hash"),
                    "sdkResults." + sdk + ".toriiSubmitPayloadHash")));
      }
      return new EthereumMainnetNativeEvmProverSelfTestFixture(
          nativeProverBundle,
          manifestString(manifestField(fixture, "schema", "schema"), "schema"),
          manifestDomain(manifestField(fixture, "domain", "domain"), "domain"),
          manifestString(manifestField(fixture, "chain", "chain"), "chain"),
          manifestString(
              manifestField(fixture, "proofBackend", "proofBackend", "proof_backend", "backend"),
              "proofBackend"),
          manifestString(
              manifestField(
                  fixture,
                  "proofArtifactHash",
                  "proofArtifactHash",
                  "proof_artifact_hash",
                  "proverArtifactHash",
                  "prover_artifact_hash",
                  "circuitArtifactHash",
                  "circuit_artifact_hash"),
              "proofArtifactHash"),
          manifestString(
              manifestField(fixture, "provingKeyHash", "provingKeyHash", "proving_key_hash"),
              "provingKeyHash"),
          manifestString(
              manifestField(fixture, "verifierKeyHash", "verifierKeyHash", "verifier_key_hash"),
              "verifierKeyHash"),
          manifestString(
              manifestField(
                  fixture,
                  "destinationBindingHash",
                  "destinationBindingHash",
                  "destination_binding_hash"),
              "destinationBindingHash"),
          manifestString(
              manifestField(fixture, "requestHash", "requestHash", "request_hash"),
              "requestHash"),
          manifestString(
              manifestField(fixture, "witnessHash", "witnessHash", "witness_hash"),
              "witnessHash"),
          manifestString(
              manifestField(fixture, "sourceProofHash", "sourceProofHash", "source_proof_hash"),
              "sourceProofHash"),
          manifestString(
              manifestField(fixture, "proofHash", "proofHash", "proof_hash"),
              "proofHash"),
          publicSignalWords,
          manifestString(
              manifestField(fixture, "calldataHash", "calldataHash", "calldata_hash"),
              "calldataHash"),
          manifestString(
              manifestField(
                  fixture,
                  "toriiSubmitPayloadHash",
                  "toriiSubmitPayloadHash",
                  "torii_submit_payload_hash"),
              "toriiSubmitPayloadHash"),
          sdkResults);
    }
  }

  public record EthereumMainnetNativeEvmProverArtifacts(
      String hashAlgorithm,
      EthereumMainnetNativeEvmProverBundle nativeProverBundle,
      String proofArtifactHash,
      String provingKeyHash,
      String verifierKeyHash,
      String crossSdkFixtureParityHash,
      EthereumMainnetNativeEvmProverParityFixture crossSdkFixtureParity,
      String nativeProverSelfTestHash,
      EthereumMainnetNativeEvmProverSelfTestFixture nativeProverSelfTest,
      String sdk,
      String implementation,
      String implementationHash) {}

  @SuppressWarnings("unchecked")
  private static Map<String, Object> expectManifestObject(final Object value, final String label) {
    if (!(value instanceof Map)) {
      throw new IllegalArgumentException(label + " must be an object");
    }
    final LinkedHashMap<String, Object> out = new LinkedHashMap<>();
    for (final Map.Entry<?, ?> entry : ((Map<?, ?>) value).entrySet()) {
      if (!(entry.getKey() instanceof String)) {
        throw new IllegalArgumentException(label + " keys must be strings");
      }
      out.put((String) entry.getKey(), entry.getValue());
    }
    return out;
  }

  private static Object manifestField(
      final Map<String, Object> manifest, final String label, final String... aliases) {
    String present = null;
    for (final String alias : aliases) {
      if (manifest.containsKey(alias)) {
        if (present != null) {
          throw new IllegalArgumentException(label + " must not use multiple aliases");
        }
        present = alias;
      }
    }
    if (present != null) {
      return manifest.get(present);
    }
    throw new IllegalArgumentException(label + " is required");
  }

  private static void requireManifestKeys(
      final Map<String, Object> manifest, final String label, final Set<String> allowedKeys) {
    for (final String key : manifest.keySet()) {
      if (!allowedKeys.contains(key)) {
        throw new IllegalArgumentException(label + " contains unknown field: " + key);
      }
    }
  }

  private static String manifestString(final Object value, final String label) {
    if (!(value instanceof String)) {
      throw new IllegalArgumentException(label + " must be a string");
    }
    return (String) value;
  }

  private static List<String> manifestStringList(final Object value, final String label) {
    if (!(value instanceof List<?>)) {
      throw new IllegalArgumentException(label + " must be an array");
    }
    final List<?> raw = (List<?>) value;
    final ArrayList<String> out = new ArrayList<>();
    for (int index = 0; index < raw.size(); index++) {
      out.add(manifestString(raw.get(index), label + "[" + index + "]"));
    }
    return Collections.unmodifiableList(out);
  }

  private static boolean manifestBoolean(final Object value, final String label) {
    if (!(value instanceof Boolean)) {
      throw new IllegalArgumentException(label + " must be a boolean");
    }
    return (Boolean) value;
  }

  private static int manifestDomain(final Object value, final String label) {
    if (value instanceof Integer
        || value instanceof Long
        || value instanceof Short
        || value instanceof Byte) {
      final long numeric = ((Number) value).longValue();
      if (numeric < 0 || numeric > Integer.MAX_VALUE) {
        throw new IllegalArgumentException(label + " must fit u32");
      }
      return (int) numeric;
    }
    if (value instanceof BigInteger) {
      final BigInteger numeric = (BigInteger) value;
      if (numeric.signum() < 0 || numeric.bitLength() > 31) {
        throw new IllegalArgumentException(label + " must fit u32");
      }
      return numeric.intValue();
    }
    if (value instanceof String) {
      final String text = (String) value;
      if (!isCanonicalDecimalText(text)) {
        throw new IllegalArgumentException(label + " must be a canonical decimal integer");
      }
      final BigInteger numeric = new BigInteger(text);
      if (numeric.signum() < 0 || numeric.bitLength() > 31) {
        throw new IllegalArgumentException(label + " must fit u32");
      }
      return numeric.intValue();
    }
    throw new IllegalArgumentException(label + " must be an integer");
  }

  private static Map<String, String> manifestStringMap(final Object value, final String label) {
    if (!(value instanceof Map)) {
      throw new IllegalArgumentException(label + " must be an object");
    }
    final Map<String, Object> raw = expectManifestObject(value, label);
    if (raw.isEmpty()) {
      throw new IllegalArgumentException(label + " must be non-empty");
    }
    final LinkedHashMap<String, String> out = new LinkedHashMap<>();
    final ArrayList<String> keys = new ArrayList<>(raw.keySet());
    Collections.sort(keys);
    for (final String key : keys) {
      out.put(key, manifestString(raw.get(key), label + "." + key));
    }
    return Collections.unmodifiableMap(out);
  }

  private static List<EthereumMainnetNativeEvmProverBundleSdkArtifact> manifestSdkArtifacts(
      final Object value, final String proofArtifactHash, final String provingKeyHash) {
    if (!(value instanceof List)) {
      throw new IllegalArgumentException("nativeSdkArtifacts must be an array");
    }
    final List<?> raw = (List<?>) value;
    if (raw.isEmpty()) {
      throw new IllegalArgumentException("nativeSdkArtifacts must be non-empty");
    }
    final String normalizedProofArtifactHash =
        normalizeNativeEvmProverBundleHex32(proofArtifactHash, "proofArtifactHash");
    final String normalizedProvingKeyHash =
        normalizeNativeEvmProverBundleHex32(provingKeyHash, "provingKeyHash");
    final ArrayList<EthereumMainnetNativeEvmProverBundleSdkArtifact> out =
        new ArrayList<>(raw.size());
    for (int index = 0; index < raw.size(); index++) {
      final Map<String, Object> artifact =
          expectManifestObject(raw.get(index), "nativeSdkArtifacts[" + index + "]");
      requireManifestKeys(
          artifact,
          "nativeSdkArtifacts[" + index + "]",
          NATIVE_EVM_PROVER_BUNDLE_SDK_ARTIFACT_KEYS);
      final String sdkProofArtifactHash =
          manifestString(
              manifestField(
                  artifact,
                  "nativeSdkArtifacts[" + index + "].proofArtifactHash",
                  "proofArtifactHash",
                  "proof_artifact_hash",
                  "proverArtifactHash",
                  "prover_artifact_hash"),
              "nativeSdkArtifacts[" + index + "].proofArtifactHash");
      if (!Objects.equals(
          normalizeNativeEvmProverBundleHex32(
              sdkProofArtifactHash, "nativeSdkArtifacts[" + index + "].proofArtifactHash"),
          normalizedProofArtifactHash)) {
        throw new IllegalArgumentException(
            "nativeSdkArtifacts[" + index + "].proofArtifactHash must match bundle");
      }
      final String sdkProvingKeyHash =
          manifestString(
              manifestField(
                  artifact,
                  "nativeSdkArtifacts[" + index + "].provingKeyHash",
                  "provingKeyHash",
                  "proving_key_hash"),
              "nativeSdkArtifacts[" + index + "].provingKeyHash");
      if (!Objects.equals(
          normalizeNativeEvmProverBundleHex32(
              sdkProvingKeyHash, "nativeSdkArtifacts[" + index + "].provingKeyHash"),
          normalizedProvingKeyHash)) {
        throw new IllegalArgumentException(
            "nativeSdkArtifacts[" + index + "].provingKeyHash must match bundle");
      }
      out.add(
          new EthereumMainnetNativeEvmProverBundleSdkArtifact(
              manifestString(
                  manifestField(artifact, "nativeSdkArtifacts[" + index + "].sdk", "sdk"),
                  "nativeSdkArtifacts[" + index + "].sdk"),
              manifestString(
                  manifestField(
                      artifact,
                      "nativeSdkArtifacts[" + index + "].implementation",
                      "implementation"),
                  "nativeSdkArtifacts[" + index + "].implementation"),
              sdkProofArtifactHash,
              sdkProvingKeyHash,
              manifestString(
                  manifestField(
                      artifact,
                      "nativeSdkArtifacts[" + index + "].implementationArtifact",
                      "implementationArtifact",
                      "implementation_artifact",
                      "implementationPath",
                      "implementation_path"),
                  "nativeSdkArtifacts[" + index + "].implementationArtifact"),
              manifestString(
                  manifestField(
                      artifact,
                      "nativeSdkArtifacts[" + index + "].implementationHash",
                      "implementationHash",
                      "implementation_hash"),
                  "nativeSdkArtifacts[" + index + "].implementationHash")));
    }
    return Collections.unmodifiableList(out);
  }

  private record Groth16ProverArtifacts(String proofArtifactHash, String provingKeyHash) {}

  public interface ProofEngine {
    byte[] prove(ProofRequest request);
  }

  public record PublicInputsInput(
      int version,
      String messageId,
      String payloadHash,
      int targetDomain,
      String commitmentRoot,
      String finalityHeight,
      String finalityBlockHash) {
    public PublicInputsInput(
        final String messageId,
        final String payloadHash,
        final String commitmentRoot,
        final String finalityHeight,
        final String finalityBlockHash) {
      this(
          1,
          messageId,
          payloadHash,
          DOMAIN_ETH,
          commitmentRoot,
          finalityHeight,
          finalityBlockHash);
    }

  }

  public record ProofRequestInput(
      PublicInputsInput publicInputs,
      byte[] bundleBytes,
      byte[] sourceProofBytes,
      String statementHash,
      String destinationBindingHash,
      String backend,
      int sourceDomain,
      SourceSccpProofs.EvmDestinationBinding destinationBinding,
      String proofArtifactHash,
      String provingKeyHash) {
    public ProofRequestInput(
        final PublicInputsInput publicInputs,
        final byte[] bundleBytes,
        final byte[] sourceProofBytes,
        final String statementHash,
        final String destinationBindingHash,
        final String backend,
        final int sourceDomain,
        final SourceSccpProofs.EvmDestinationBinding destinationBinding) {
      this(
          publicInputs,
          bundleBytes,
          sourceProofBytes,
          statementHash,
          destinationBindingHash,
          backend,
          sourceDomain,
          destinationBinding,
          null,
          null);
    }

    public ProofRequestInput(
        final PublicInputsInput publicInputs,
        final byte[] bundleBytes,
        final byte[] sourceProofBytes,
        final String statementHash,
        final String destinationBindingHash,
        final String backend,
        final int sourceDomain) {
      this(
          publicInputs,
          bundleBytes,
          sourceProofBytes,
          statementHash,
          destinationBindingHash,
          backend,
          sourceDomain,
          null,
          null,
          null);
    }

    public ProofRequestInput(
        final PublicInputsInput publicInputs,
        final byte[] bundleBytes,
        final String statementHash,
        final String destinationBindingHash) {
      this(
          publicInputs,
          bundleBytes,
          new byte[0],
          statementHash,
          destinationBindingHash,
          GROTH16_BN254_PROOF_BACKEND_V1,
          DOMAIN_SORA,
          null,
          null,
          null);
    }

    public ProofRequestInput(
        final PublicInputsInput publicInputs,
        final byte[] bundleBytes,
        final byte[] sourceProofBytes,
        final String statementHash,
        final SourceSccpProofs.EvmDestinationBinding destinationBinding,
        final String backend,
        final int sourceDomain,
        final String proofArtifactHash,
        final String provingKeyHash) {
      this(
          publicInputs,
          bundleBytes,
          sourceProofBytes,
          statementHash,
          requireDestinationBindingHash(publicInputs, destinationBinding, backend, sourceDomain),
          backend,
          sourceDomain,
          destinationBinding,
          proofArtifactHash,
          provingKeyHash);
    }

    public ProofRequestInput(
        final PublicInputsInput publicInputs,
        final byte[] bundleBytes,
        final byte[] sourceProofBytes,
        final String statementHash,
        final SourceSccpProofs.EvmDestinationBinding destinationBinding,
        final String backend,
        final int sourceDomain) {
      this(
          publicInputs,
          bundleBytes,
          sourceProofBytes,
          statementHash,
          destinationBinding,
          backend,
          sourceDomain,
          null,
          null);
    }

    public ProofRequestInput(
        final PublicInputsInput publicInputs,
        final byte[] bundleBytes,
        final byte[] sourceProofBytes,
        final String statementHash,
        final SourceSccpProofs.EvmDestinationBinding destinationBinding) {
      this(
          publicInputs,
          bundleBytes,
          sourceProofBytes,
          statementHash,
          destinationBinding,
          GROTH16_BN254_PROOF_BACKEND_V1,
          DOMAIN_SORA,
          null,
          null);
    }

    public ProofRequestInput(
        final PublicInputsInput publicInputs,
        final byte[] bundleBytes,
        final String statementHash,
        final SourceSccpProofs.EvmDestinationBinding destinationBinding) {
      this(
          publicInputs,
          bundleBytes,
          new byte[0],
          statementHash,
          destinationBinding,
          GROTH16_BN254_PROOF_BACKEND_V1,
          DOMAIN_SORA,
          null,
          null);
    }
  }

  public record ProofContext(int version, String statementHash, String destinationBindingHash) {}

  public record ProofRequest(
      int version,
      String backend,
      int sourceDomain,
      int targetDomain,
      PublicInputsInput publicInputs,
      byte[] publicInputsBytes,
      List<String> publicSignalWords,
      byte[] bundleBytes,
      byte[] sourceProofBytes,
      ProofContext proofContext,
      String statementHash,
      String destinationBindingHash,
      String requestHash,
      SourceSccpProofs.EvmDestinationBinding destinationBinding,
      String proofArtifactHash,
      String provingKeyHash) {
    public ProofRequest(
        final int version,
        final String backend,
        final int sourceDomain,
        final int targetDomain,
        final PublicInputsInput publicInputs,
        final byte[] publicInputsBytes,
        final List<String> publicSignalWords,
        final byte[] bundleBytes,
        final byte[] sourceProofBytes,
        final ProofContext proofContext,
        final String statementHash,
        final String destinationBindingHash,
        final String requestHash) {
      this(
          version,
          backend,
          sourceDomain,
          targetDomain,
          publicInputs,
          publicInputsBytes,
          publicSignalWords,
          bundleBytes,
          sourceProofBytes,
          proofContext,
          statementHash,
          destinationBindingHash,
          requestHash,
          null,
          null,
          null);
    }

    public ProofRequest(
        final int version,
        final String backend,
        final int sourceDomain,
        final int targetDomain,
        final PublicInputsInput publicInputs,
        final byte[] publicInputsBytes,
        final List<String> publicSignalWords,
        final byte[] bundleBytes,
        final byte[] sourceProofBytes,
        final ProofContext proofContext,
        final String statementHash,
        final String destinationBindingHash,
        final String requestHash,
        final SourceSccpProofs.EvmDestinationBinding destinationBinding) {
      this(
          version,
          backend,
          sourceDomain,
          targetDomain,
          publicInputs,
          publicInputsBytes,
          publicSignalWords,
          bundleBytes,
          sourceProofBytes,
          proofContext,
          statementHash,
          destinationBindingHash,
          requestHash,
          destinationBinding,
          null,
          null);
    }

    public ProofRequest {
      publicInputsBytes =
          Arrays.copyOf(
              Objects.requireNonNull(publicInputsBytes, "publicInputsBytes"),
              publicInputsBytes.length);
      publicSignalWords =
          Collections.unmodifiableList(
              new ArrayList<>(Objects.requireNonNull(publicSignalWords, "publicSignalWords")));
      bundleBytes =
          Arrays.copyOf(Objects.requireNonNull(bundleBytes, "bundleBytes"), bundleBytes.length);
      sourceProofBytes =
          Arrays.copyOf(
              Objects.requireNonNull(sourceProofBytes, "sourceProofBytes"),
              sourceProofBytes.length);
    }

    @Override
    public byte[] publicInputsBytes() {
      return Arrays.copyOf(publicInputsBytes, publicInputsBytes.length);
    }

    @Override
    public byte[] bundleBytes() {
      return Arrays.copyOf(bundleBytes, bundleBytes.length);
    }

    @Override
    public byte[] sourceProofBytes() {
      return Arrays.copyOf(sourceProofBytes, sourceProofBytes.length);
    }
  }

  public record ProofResult(
      int version,
      String backend,
      byte[] proofBytes,
      String proofBase64,
      PublicInputsInput publicInputs,
      List<String> publicSignalWords,
      byte[] bundleBytes,
      byte[] sourceProofBytes,
      ProofContext proofContext,
      String statementHash,
      String destinationBindingHash,
      String requestHash,
      String envelopeHash,
      SourceSccpProofs.EvmDestinationBinding destinationBinding,
      String proofArtifactHash,
      String provingKeyHash) {
    public ProofResult(
        final int version,
        final String backend,
        final byte[] proofBytes,
        final String proofBase64,
        final PublicInputsInput publicInputs,
        final List<String> publicSignalWords,
        final byte[] bundleBytes,
        final byte[] sourceProofBytes,
        final ProofContext proofContext,
        final String statementHash,
        final String destinationBindingHash,
        final String requestHash,
        final String envelopeHash) {
      this(
          version,
          backend,
          proofBytes,
          proofBase64,
          publicInputs,
          publicSignalWords,
          bundleBytes,
          sourceProofBytes,
          proofContext,
          statementHash,
          destinationBindingHash,
          requestHash,
          envelopeHash,
          null,
          null,
          null);
    }

    public ProofResult(
        final int version,
        final String backend,
        final byte[] proofBytes,
        final String proofBase64,
        final PublicInputsInput publicInputs,
        final List<String> publicSignalWords,
        final byte[] bundleBytes,
        final byte[] sourceProofBytes,
        final ProofContext proofContext,
        final String statementHash,
        final String destinationBindingHash,
        final String requestHash,
        final String envelopeHash,
        final SourceSccpProofs.EvmDestinationBinding destinationBinding) {
      this(
          version,
          backend,
          proofBytes,
          proofBase64,
          publicInputs,
          publicSignalWords,
          bundleBytes,
          sourceProofBytes,
          proofContext,
          statementHash,
          destinationBindingHash,
          requestHash,
          envelopeHash,
          destinationBinding,
          null,
          null);
    }

    public ProofResult {
      proofBytes = Arrays.copyOf(Objects.requireNonNull(proofBytes, "proofBytes"), proofBytes.length);
      publicSignalWords =
          Collections.unmodifiableList(
              new ArrayList<>(Objects.requireNonNull(publicSignalWords, "publicSignalWords")));
      bundleBytes =
          Arrays.copyOf(Objects.requireNonNull(bundleBytes, "bundleBytes"), bundleBytes.length);
      sourceProofBytes =
          Arrays.copyOf(
              Objects.requireNonNull(sourceProofBytes, "sourceProofBytes"),
              sourceProofBytes.length);
    }

    @Override
    public byte[] proofBytes() {
      return Arrays.copyOf(proofBytes, proofBytes.length);
    }

    @Override
    public byte[] bundleBytes() {
      return Arrays.copyOf(bundleBytes, bundleBytes.length);
    }

    @Override
    public byte[] sourceProofBytes() {
      return Arrays.copyOf(sourceProofBytes, sourceProofBytes.length);
    }
  }

  public record SubmissionInput(
      PublicInputsInput publicInputs,
      byte[] proofBytes,
      String statementHash,
      String destinationBindingHash,
      int sourceDomain,
      ProofResult proofResult,
      List<String> publicSignalWords) {
    public SubmissionInput {
      proofBytes = Arrays.copyOf(Objects.requireNonNull(proofBytes, "proofBytes"), proofBytes.length);
      if (publicSignalWords != null) {
        publicSignalWords = Collections.unmodifiableList(new ArrayList<>(publicSignalWords));
      }
    }

    public SubmissionInput(final ProofResult proofResult) {
      this(
          Objects.requireNonNull(proofResult, "proofResult").publicInputs(),
          proofResult.proofBytes(),
          proofResult.statementHash(),
          proofResult.destinationBindingHash(),
          DOMAIN_SORA,
          proofResult,
          proofResult.publicSignalWords());
    }

    public SubmissionInput(
        final PublicInputsInput publicInputs,
        final byte[] proofBytes,
        final String statementHash,
        final String destinationBindingHash) {
      this(publicInputs, proofBytes, statementHash, destinationBindingHash, DOMAIN_SORA, null, null);
    }

    public SubmissionInput(
        final PublicInputsInput publicInputs,
        final byte[] proofBytes,
        final String statementHash,
        final SourceSccpProofs.EvmDestinationBinding destinationBinding,
        final int sourceDomain,
        final ProofResult proofResult,
        final List<String> publicSignalWords) {
      this(
          publicInputs,
          proofBytes,
          statementHash,
          requireDestinationBindingHash(
              publicInputs, destinationBinding, GROTH16_BN254_PROOF_BACKEND_V1, sourceDomain),
          sourceDomain,
          proofResult,
          publicSignalWords);
    }

    public SubmissionInput(
        final PublicInputsInput publicInputs,
        final byte[] proofBytes,
        final String statementHash,
        final SourceSccpProofs.EvmDestinationBinding destinationBinding) {
      this(publicInputs, proofBytes, statementHash, destinationBinding, DOMAIN_SORA, null, null);
    }

    @Override
    public byte[] proofBytes() {
      return Arrays.copyOf(proofBytes, proofBytes.length);
    }
  }

  public record SubmissionArgument(String key, String encoding, String bytes) {}

  public record Submission(
      int version,
      String proofFamily,
      String verifierBackend,
      String platformPayload,
      String envelopeEncoding,
      String submissionKind,
      String verifierEntrypoint,
      String contractMethod,
      String functionSelector,
      int sourceDomain,
      int targetDomain,
      PublicInputsInput publicInputs,
      List<String> publicInputWords,
      List<String> publicSignalWords,
      String statementHash,
      String destinationBindingHash,
      List<SubmissionArgument> arguments,
      String callDataHex,
      String envelopeHex,
      byte[] proofBytes,
      byte[] publicInputWordsBytes,
      byte[] callData) {
    public Submission {
      publicInputWords =
          Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(publicInputWords)));
      publicSignalWords =
          Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(publicSignalWords)));
      arguments = Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(arguments)));
      proofBytes = Arrays.copyOf(Objects.requireNonNull(proofBytes, "proofBytes"), proofBytes.length);
      publicInputWordsBytes =
          Arrays.copyOf(
              Objects.requireNonNull(publicInputWordsBytes, "publicInputWordsBytes"),
              publicInputWordsBytes.length);
      callData = Arrays.copyOf(Objects.requireNonNull(callData, "callData"), callData.length);
    }

    @Override
    public byte[] proofBytes() {
      return Arrays.copyOf(proofBytes, proofBytes.length);
    }

    @Override
    public byte[] publicInputWordsBytes() {
      return Arrays.copyOf(publicInputWordsBytes, publicInputWordsBytes.length);
    }

    @Override
    public byte[] callData() {
      return Arrays.copyOf(callData, callData.length);
    }

    public byte[] envelopeBytes() {
      return Arrays.copyOf(callData, callData.length);
    }
  }
}
