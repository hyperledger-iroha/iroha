package org.hyperledger.iroha.sdk.sccp

import java.io.ByteArrayOutputStream
import java.io.InputStream
import java.net.HttpURLConnection
import java.net.URL
import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import java.util.Base64
import org.bouncycastle.crypto.digests.KeccakDigest
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.crypto.Blake2b

private fun sccpCallbackMapSnapshot(value: Map<String, Any?>): Map<String, Any?> =
    value.mapValues { (_, item) -> sccpCallbackAnySnapshot(item) }

private fun sccpCallbackAnySnapshot(value: Any?): Any? =
    when (value) {
        is ByteArray -> value.copyOf()
        is Map<*, *> -> {
            val snapshot = LinkedHashMap<String, Any?>()
            value.forEach { (key, item) ->
                if (key is String) {
                    snapshot[key] = sccpCallbackAnySnapshot(item)
                }
            }
            snapshot.toMap()
        }
        is List<*> -> value.map { sccpCallbackAnySnapshot(it) }
        is Array<*> -> value.map { sccpCallbackAnySnapshot(it) }
        else -> value
    }

/** EVM-family SCCP Groth16 proof request helpers for local-first UI proof generation. */
object SccpEvm {
    const val DOMAIN_SORA: Int = SccpSourceProofs.DOMAIN_SORA
    const val DOMAIN_ETH: Int = SccpSourceProofs.DOMAIN_ETH
    const val DOMAIN_BSC: Int = SccpSourceProofs.DOMAIN_BSC
    const val GROTH16_BN254_PROOF_BACKEND_V1: String = "evm-groth16-bn254-v1"
    const val NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1: String =
        "sccp-native-evm-groth16-prover-bundle-v1"
    const val ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1: String =
        "sccp-ethereum-mainnet-native-evm-cross-sdk-fixture-parity-v1"
    const val ETH_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1: String =
        "sccp-ethereum-mainnet-native-evm-prover-self-test-v1"
    const val ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1: String =
        "sccp:eth:native-evm-groth16-prover:ethereum-mainnet:v1"
    @JvmField
    val ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1: Map<String, String> = mapOf(
        "javascript" to "pure-typescript",
        "swift" to "native-swift",
        "kotlin" to "native-kotlin",
        "java-android" to "native-java",
        "dotnet" to "native-csharp",
    )
    @JvmField
    val ETH_NATIVE_EVM_PROVER_REQUIRED_AUDIT_HASHES_V1: List<String> = listOf(
        "circuit_security_audit",
        "native_implementation_audit",
        "reproducible_build_attestation",
        "cross_sdk_fixture_parity",
        "native_prover_self_test",
        "no_wasm_no_remote_scan",
    )
    const val NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1: String = "sha256"
    private const val NATIVE_EVM_PROVER_MIN_ARTIFACT_BYTES_V1: Int = 256
    /** Resolves manifest-declared native prover artifact paths from app-local storage. */
    fun interface NativeEvmProverArtifactResolver {
        fun resolveArtifact(path: String): ByteArray
    }

    const val GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1: Int = 384
    const val SOURCE_STATE_MAX_PROOF_BYTES: Int = 2 * 1024 * 1024
    const val NATIVE_RECURSIVE_MAX_PROOF_BYTES: Int = 2 * 1024 * 1024
    const val CONTRACT_CALL_ABI_TUPLE_V1: String = "abi_tuple_v1"
    const val SUBMIT_MESSAGE_PROOF_ABI_V1: String =
        "submitSccpMessageProof(bytes,bytes32[6],bytes32)"
    const val SUBMIT_MESSAGE_PROOF_SELECTOR_V1: String = "0xbd57826c"

    private const val PROOF_REQUEST_PREFIX_V1: String = "sccp:evm:groth16-proof-request:v1"
    private const val PROOF_ENVELOPE_PREFIX_V1: String = "sccp:evm:groth16-proof-envelope:v1"
    private const val STARK_FRI_PROOF_FAMILY_V1: String = "stark-fri-v1"
    private const val SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1: String =
        "submitSccpMessageProof(bytes proof_bytes, bytes32[6] public_inputs, bytes32 statement_hash)"
    private val SUBMIT_MESSAGE_PROOF_SELECTOR_BYTES_V1 =
        byteArrayOf(0xbd.toByte(), 0x57, 0x82.toByte(), 0x6c)
    private val MAX_U64: BigInteger = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
    private val BN254_BASE_FIELD_MODULUS =
        BigInteger("30644e72e131a029b85045b68181585d97816a916871ca8d3c208c16d87cfd47", 16)
    private val BN254_SCALAR_FIELD_MODULUS =
        BigInteger("30644e72e131a029b85045b68181585d2833e84879b9709143e1f593f0000001", 16)
    private val BN254_G2_B_C0 =
        BigInteger("2b149d40ceb8aaae81be18991be06ac3b5b4c5e559dbefa33267e6dc24a138e5", 16)
    private val BN254_G2_B_C1 =
        BigInteger("009713b03af0fed4cd2cafadeed8fdf4a74fa084e52d1852e4a2bd0685c315d2", 16)
    private val NATIVE_EVM_PROVER_BUNDLE_MANIFEST_KEYS = setOf(
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
        "audit_hashes",
    )
    private val NATIVE_EVM_PROVER_BUNDLE_SDK_ARTIFACT_KEYS = setOf(
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
        "implementation_hash",
    )
    private val NATIVE_EVM_PROVER_PARITY_FIXTURE_KEYS = setOf(
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
        "sdk_results",
    )
    private val NATIVE_EVM_PROVER_PARITY_SDK_RESULT_KEYS = setOf(
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
        "torii_submit_payload_hash",
    )
    private val NATIVE_EVM_PROVER_SELF_TEST_FIXTURE_KEYS = setOf(
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
        "sdk_results",
    )
    private val NATIVE_EVM_PROVER_SELF_TEST_SDK_RESULT_KEYS = setOf(
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
    )
    private val SIGNAL_LABELS = listOf(
        "sccp:groth16-bn254:signal:message-id:v1",
        "sccp:groth16-bn254:signal:payload-hash:v1",
        "sccp:groth16-bn254:signal:target-domain:v1",
        "sccp:groth16-bn254:signal:commitment-root:v1",
        "sccp:groth16-bn254:signal:finality-height:v1",
        "sccp:groth16-bn254:signal:finality-block-hash:v1",
        "sccp:groth16-bn254:signal:source-domain:v1",
        "sccp:groth16-bn254:signal:statement-hash:v1",
        "sccp:groth16-bn254:signal:destination-binding-hash:v1",
    )

    /** One SDK implementation row in an audited Ethereum mainnet native EVM prover bundle. */
    class EthereumMainnetNativeEvmProverBundleSdkArtifact(
        val sdk: String,
        val implementation: String,
        proofArtifactHash: String,
        provingKeyHash: String,
        implementationHash: String,
        implementationArtifact: String? = null,
    ) {
        val proofArtifactHash: String =
            normalizeNativeEvmProverBundleHex32(proofArtifactHash, "proofArtifactHash")
        val provingKeyHash: String =
            normalizeNativeEvmProverBundleHex32(provingKeyHash, "provingKeyHash")
        val implementationArtifact: String? =
            implementationArtifact?.let { normalizeNativeEvmProverArtifactPath(it, "implementationArtifact") }
        val implementationHash: String =
            normalizeNativeEvmProverBundleHex32(implementationHash, "implementationHash")

        init {
            require(sdk.isNotEmpty()) { "nativeSdkArtifacts.sdk must be non-empty" }
            require(ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1[sdk] == implementation) {
                "$sdk implementation must match Ethereum native EVM prover bundle profile"
            }
        }
    }

    /** Audited native-only EVM Groth16 prover bundle for Ethereum mainnet. */
    class EthereumMainnetNativeEvmProverBundle @JvmOverloads constructor(
        val schema: String = NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1,
        val bundleId: String = ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1,
        val domain: Int = DOMAIN_ETH,
        val chain: String = "eth",
        val proofBackend: String = GROTH16_BN254_PROOF_BACKEND_V1,
        proofArtifactHash: String,
        provingKeyHash: String,
        verifierKeyHash: String,
        destinationBindingHash: String,
        val noWasm: Boolean = true,
        val remoteProverRequired: Boolean = false,
        val browserImplementation: String = "pure-typescript",
        nativeSdkArtifacts: List<EthereumMainnetNativeEvmProverBundleSdkArtifact>,
        auditHashes: Map<String, String>,
        crossSdkFixtureParityArtifact: String? = null,
        nativeProverSelfTestArtifact: String? = null,
        expectedDestinationBindingHash: String? = null,
        proofArtifact: String? = null,
        provingKey: String? = null,
        verifierKey: String? = null,
    ) {
        val proofArtifactHash: String =
            normalizeNativeEvmProverBundleHex32(proofArtifactHash, "proofArtifactHash")
        val proofArtifact: String? =
            proofArtifact?.let { normalizeNativeEvmProverArtifactPath(it, "proofArtifact") }
        val provingKeyHash: String =
            normalizeNativeEvmProverBundleHex32(provingKeyHash, "provingKeyHash")
        val provingKey: String? =
            provingKey?.let { normalizeNativeEvmProverArtifactPath(it, "provingKey") }
        val verifierKeyHash: String =
            normalizeNativeEvmProverBundleHex32(verifierKeyHash, "verifierKeyHash")
        val verifierKey: String? =
            verifierKey?.let { normalizeNativeEvmProverArtifactPath(it, "verifierKey") }
        val destinationBindingHash: String =
            normalizeNativeEvmProverBundleHex32(destinationBindingHash, "destinationBindingHash")
        val nativeSdkArtifacts: List<EthereumMainnetNativeEvmProverBundleSdkArtifact>
        val crossSdkFixtureParityArtifact: String? =
            crossSdkFixtureParityArtifact?.let {
                normalizeNativeEvmProverArtifactPath(it, "crossSdkFixtureParityArtifact")
            }
        val nativeProverSelfTestArtifact: String? =
            nativeProverSelfTestArtifact?.let {
                normalizeNativeEvmProverArtifactPath(it, "nativeProverSelfTestArtifact")
            }
        val auditHashes: Map<String, String>

        init {
            require(schema == NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1) {
                "nativeProverBundle.schema must be $NATIVE_EVM_PROVER_BUNDLE_SCHEMA_V1"
            }
            require(bundleId == ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1) {
                "nativeProverBundle.bundleId must be $ETH_NATIVE_EVM_PROVER_BUNDLE_ID_V1"
            }
            require(domain == DOMAIN_ETH) { "nativeProverBundle.domain must be ETH" }
            require(chain == "eth") { "nativeProverBundle.chain must be eth" }
            require(proofBackend == GROTH16_BN254_PROOF_BACKEND_V1) {
                "nativeProverBundle.proofBackend must be $GROTH16_BN254_PROOF_BACKEND_V1"
            }
            require(noWasm) { "nativeProverBundle.noWasm must be true" }
            require(!remoteProverRequired) {
                "nativeProverBundle.remoteProverRequired must be false"
            }
            require(browserImplementation == "pure-typescript") {
                "nativeProverBundle.browserImplementation must be pure-typescript"
            }
            expectedDestinationBindingHash?.let {
                require(
                    normalizeNativeEvmProverBundleHex32(
                        it,
                        "expectedDestinationBindingHash",
                    ) == destinationBindingHash,
                ) {
                    "nativeProverBundle.destinationBindingHash must match destinationBinding"
                }
            }
            require(auditHashes.isNotEmpty()) {
                "nativeProverBundle.auditHashes must be non-empty"
            }
            auditHashes.keys
                .filterNot { ETH_NATIVE_EVM_PROVER_REQUIRED_AUDIT_HASHES_V1.contains(it) }
                .forEach { key -> throw IllegalArgumentException("auditHashes.$key is not expected") }
            val normalizedAuditHashes = LinkedHashMap<String, String>()
            ETH_NATIVE_EVM_PROVER_REQUIRED_AUDIT_HASHES_V1.forEach { key ->
                val value = auditHashes[key]
                    ?: throw IllegalArgumentException("auditHashes.$key is required")
                normalizedAuditHashes[key] =
                    normalizeNativeEvmProverBundleHex32(value, "auditHashes.$key")
            }
            val bySdk = LinkedHashMap<String, EthereumMainnetNativeEvmProverBundleSdkArtifact>()
            nativeSdkArtifacts.forEach { artifact ->
                require(!bySdk.containsKey(artifact.sdk)) {
                    "nativeSdkArtifacts contains duplicate sdk: ${artifact.sdk}"
                }
                require(artifact.proofArtifactHash == this.proofArtifactHash) {
                    "${artifact.sdk} proofArtifactHash must match bundle"
                }
                require(artifact.provingKeyHash == this.provingKeyHash) {
                    "${artifact.sdk} provingKeyHash must match bundle"
                }
                bySdk[artifact.sdk] = artifact
            }
            ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1.keys.forEach { sdk ->
                require(bySdk.containsKey(sdk)) { "nativeSdkArtifacts missing sdk: $sdk" }
            }
            requireNativeEvmProverBundleHashRoleSeparation(
                listOf(
                    "proofArtifactHash" to this.proofArtifactHash,
                    "provingKeyHash" to this.provingKeyHash,
                    "verifierKeyHash" to this.verifierKeyHash,
                    "destinationBindingHash" to this.destinationBindingHash,
                ) + bySdk.values.sortedBy { it.sdk }.map {
                    "nativeSdkArtifacts[${it.sdk}].implementationHash" to it.implementationHash
                } + normalizedAuditHashes.entries.sortedBy { it.key }.map { (key, hash) ->
                    "auditHashes.$key" to hash
                },
            )
            this.nativeSdkArtifacts = bySdk.values.sortedBy { it.sdk }
            this.auditHashes = normalizedAuditHashes.toMap()
        }

        @JvmOverloads
        fun verifiedArtifacts(
            proofArtifactBytes: ByteArray,
            provingKeyBytes: ByteArray,
            verifierKeyBytes: ByteArray,
            sdk: String? = null,
            implementationBytes: ByteArray? = null,
            crossSdkFixtureParityBytes: ByteArray? = null,
            nativeProverSelfTestBytes: ByteArray? = null,
        ): EthereumMainnetNativeEvmProverArtifacts {
            val proofArtifactHash = sha256Hex(proofArtifactBytes)
            require(proofArtifactHash == this.proofArtifactHash) {
                "proofArtifactBytes sha256 must match nativeProverBundle.proofArtifactHash"
            }
            val provingKeyHash = sha256Hex(provingKeyBytes)
            require(provingKeyHash == this.provingKeyHash) {
                "provingKeyBytes sha256 must match nativeProverBundle.provingKeyHash"
            }
            val verifierKeyHash = sha256Hex(verifierKeyBytes)
            require(verifierKeyHash == this.verifierKeyHash) {
                "verifierKeyBytes sha256 must match nativeProverBundle.verifierKeyHash"
            }
            require(crossSdkFixtureParityBytes != null) {
                "crossSdkFixtureParityBytes are required for nativeProverBundle parity binding"
            }
            val crossSdkFixtureParityHash = sha256Hex(crossSdkFixtureParityBytes)
            require(crossSdkFixtureParityHash == auditHashes["cross_sdk_fixture_parity"]) {
                "crossSdkFixtureParityBytes sha256 must match nativeProverBundle.auditHashes.cross_sdk_fixture_parity"
            }
            require(nativeProverSelfTestBytes != null) {
                "nativeProverSelfTestBytes are required for nativeProverBundle self-test binding"
            }
            val nativeProverSelfTestHash = sha256Hex(nativeProverSelfTestBytes)
            require(nativeProverSelfTestHash == auditHashes["native_prover_self_test"]) {
                "nativeProverSelfTestBytes sha256 must match nativeProverBundle.auditHashes.native_prover_self_test"
            }
            requireNativeEvmProverProductionArtifactSize(proofArtifactBytes, "proofArtifactBytes")
            requireNativeEvmProverProductionArtifactSize(provingKeyBytes, "provingKeyBytes")
            requireNativeEvmProverProductionArtifactSize(verifierKeyBytes, "verifierKeyBytes")
            rejectNativeEvmProverForbiddenArtifactMarkers(proofArtifactBytes, "proofArtifactBytes")
            rejectNativeEvmProverForbiddenArtifactMarkers(provingKeyBytes, "provingKeyBytes")
            rejectNativeEvmProverForbiddenArtifactMarkers(verifierKeyBytes, "verifierKeyBytes")
            rejectNativeEvmProverForbiddenArtifactMarkers(
                crossSdkFixtureParityBytes,
                "crossSdkFixtureParityBytes",
            )
            rejectNativeEvmProverForbiddenArtifactMarkers(
                nativeProverSelfTestBytes,
                "nativeProverSelfTestBytes",
            )
            val crossSdkFixtureParity = EthereumMainnetNativeEvmProverParityFixture.fromJsonBytes(
                crossSdkFixtureParityBytes,
                this,
            )
            val nativeProverSelfTest = EthereumMainnetNativeEvmProverSelfTestFixture.fromJsonBytes(
                nativeProverSelfTestBytes,
                this,
            )
            require(!sdk.isNullOrEmpty()) {
                "sdk must be a non-empty string for nativeProverBundle implementation binding"
            }
            require(implementationBytes != null) {
                "implementationBytes are required for nativeProverBundle implementation binding"
            }
            val artifact = nativeSdkArtifacts.firstOrNull { it.sdk == sdk }
                ?: throw IllegalArgumentException("nativeProverBundle has no artifact row for sdk: $sdk")
            val implementationHash = sha256Hex(implementationBytes)
            require(implementationHash == artifact.implementationHash) {
                "implementationBytes sha256 must match nativeProverBundle implementationHash"
            }
            requireNativeEvmProverProductionArtifactSize(implementationBytes, "implementationBytes")
            rejectNativeEvmProverForbiddenArtifactMarkers(implementationBytes, "implementationBytes")
            val implementation = artifact.implementation
            return EthereumMainnetNativeEvmProverArtifacts(
                hashAlgorithm = NATIVE_EVM_PROVER_ARTIFACT_HASH_ALGORITHM_V1,
                nativeProverBundle = this,
                proofArtifactHash = proofArtifactHash,
                provingKeyHash = provingKeyHash,
                verifierKeyHash = verifierKeyHash,
                crossSdkFixtureParityHash = crossSdkFixtureParityHash,
                crossSdkFixtureParity = crossSdkFixtureParity,
                nativeProverSelfTestHash = nativeProverSelfTestHash,
                nativeProverSelfTest = nativeProverSelfTest,
                sdk = sdk,
                implementation = implementation,
                implementationHash = implementationHash,
            )
        }

        fun verifiedArtifacts(
            sdk: String,
            artifactResolver: NativeEvmProverArtifactResolver,
        ): EthereumMainnetNativeEvmProverArtifacts {
            val proofArtifactPath =
                proofArtifact ?: throw IllegalArgumentException("proofArtifact is required")
            val provingKeyPath =
                provingKey ?: throw IllegalArgumentException("provingKey is required")
            val verifierKeyPath =
                verifierKey ?: throw IllegalArgumentException("verifierKey is required")
            val parityPath = crossSdkFixtureParityArtifact
                ?: throw IllegalArgumentException("crossSdkFixtureParityArtifact is required")
            val selfTestPath = nativeProverSelfTestArtifact
                ?: throw IllegalArgumentException("nativeProverSelfTestArtifact is required")
            val artifact = nativeSdkArtifacts.firstOrNull { it.sdk == sdk }
                ?: throw IllegalArgumentException("nativeProverBundle has no artifact row for sdk: $sdk")
            val implementationPath = artifact.implementationArtifact
                ?: throw IllegalArgumentException("implementationArtifact is required")
            return verifiedArtifacts(
                proofArtifactBytes = artifactResolver.resolveArtifact(proofArtifactPath),
                provingKeyBytes = artifactResolver.resolveArtifact(provingKeyPath),
                verifierKeyBytes = artifactResolver.resolveArtifact(verifierKeyPath),
                sdk = sdk,
                implementationBytes = artifactResolver.resolveArtifact(implementationPath),
                crossSdkFixtureParityBytes = artifactResolver.resolveArtifact(parityPath),
                nativeProverSelfTestBytes = artifactResolver.resolveArtifact(selfTestPath),
            )
        }

        fun applyTo(input: EvmSccpProofRequestInput): EvmSccpProofRequestInput {
            require(normalizeNonZeroHex32(input.destinationBindingHash, "destinationBindingHash") == destinationBindingHash) {
                "nativeProverBundle.destinationBindingHash must match destinationBinding"
            }
            input.destinationBinding?.let { binding ->
                require(normalizeNonZeroHex32(binding.verifierKeyHash, "verifierKeyHash") == verifierKeyHash) {
                    "nativeProverBundle.verifierKeyHash must match destinationBinding"
                }
            }
            input.proofArtifactHash?.let {
                require(normalizeNonZeroHex32(it, "proofArtifactHash") == proofArtifactHash) {
                    "nativeProverBundle.proofArtifactHash must match proof request"
                }
            }
            input.provingKeyHash?.let {
                require(normalizeNonZeroHex32(it, "provingKeyHash") == provingKeyHash) {
                    "nativeProverBundle.provingKeyHash must match proof request"
                }
            }
            require((input.proofArtifactHash == null) == (input.provingKeyHash == null)) {
                "proofArtifactHash and provingKeyHash must be supplied together"
            }
            return input.copy(
                destinationBindingHash = destinationBindingHash,
                proofArtifactHash = proofArtifactHash,
                provingKeyHash = provingKeyHash,
            )
        }

        companion object {
            @JvmStatic
            @JvmOverloads
            fun fromJson(
                json: String,
                expectedDestinationBindingHash: String? = null,
            ): EthereumMainnetNativeEvmProverBundle {
                val parsed = try {
                    JsonParser.parse(json)
                } catch (ex: IllegalStateException) {
                    throw IllegalArgumentException("nativeProverBundle JSON is invalid: ${ex.message}", ex)
                }
                return fromMap(expectManifestObject(parsed, "nativeProverBundle"), expectedDestinationBindingHash)
            }

            @JvmStatic
            @JvmOverloads
            fun fromJsonBytes(
                payload: ByteArray,
                expectedDestinationBindingHash: String? = null,
            ): EthereumMainnetNativeEvmProverBundle =
                fromJson(String(payload, StandardCharsets.UTF_8), expectedDestinationBindingHash)

            @JvmStatic
            @JvmOverloads
            fun fromMap(
                manifest: Map<String, Any?>,
                expectedDestinationBindingHash: String? = null,
            ): EthereumMainnetNativeEvmProverBundle {
                requireManifestKeys(
                    manifest,
                    "nativeProverBundle",
                    NATIVE_EVM_PROVER_BUNDLE_MANIFEST_KEYS,
                )
                val proofArtifactHash = manifestString(
                    manifestField(
                        manifest,
                        "proofArtifactHash",
                        "proofArtifactHash",
                        "proof_artifact_hash",
                        "proverArtifactHash",
                        "prover_artifact_hash",
                        "circuitArtifactHash",
                        "circuit_artifact_hash",
                    ),
                    "proofArtifactHash",
                )
                val provingKeyHash = manifestString(
                    manifestField(manifest, "provingKeyHash", "provingKeyHash", "proving_key_hash"),
                    "provingKeyHash",
                )
                return EthereumMainnetNativeEvmProverBundle(
                    schema = manifestString(
                        manifestField(manifest, "schema", "schema"),
                        "schema",
                    ),
                    bundleId = manifestString(
                        manifestField(manifest, "bundleId", "bundleId", "bundle_id"),
                        "bundleId",
                    ),
                    domain = manifestDomain(manifestField(manifest, "domain", "domain"), "domain"),
                    chain = manifestString(manifestField(manifest, "chain", "chain"), "chain"),
                    proofBackend = manifestString(
                        manifestField(manifest, "proofBackend", "proofBackend", "proof_backend", "backend"),
                        "proofBackend",
                    ),
                    proofArtifact = manifestString(
                        manifestField(
                            manifest,
                            "proofArtifact",
                            "proofArtifact",
                            "proof_artifact",
                            "proverArtifact",
                            "prover_artifact",
                            "circuitArtifact",
                            "circuit_artifact",
                        ),
                        "proofArtifact",
                    ),
                    proofArtifactHash = proofArtifactHash,
                    provingKey = manifestString(
                        manifestField(manifest, "provingKey", "provingKey", "proving_key"),
                        "provingKey",
                    ),
                    provingKeyHash = provingKeyHash,
                    verifierKey = manifestString(
                        manifestField(manifest, "verifierKey", "verifierKey", "verifier_key"),
                        "verifierKey",
                    ),
                    verifierKeyHash = manifestString(
                        manifestField(manifest, "verifierKeyHash", "verifierKeyHash", "verifier_key_hash"),
                        "verifierKeyHash",
                    ),
                    destinationBindingHash = manifestString(
                        manifestField(
                            manifest,
                            "destinationBindingHash",
                            "destinationBindingHash",
                            "destination_binding_hash",
                        ),
                        "destinationBindingHash",
                    ),
                    noWasm = manifestBoolean(manifestField(manifest, "noWasm", "noWasm", "no_wasm"), "noWasm"),
                    remoteProverRequired = manifestBoolean(
                        manifestField(
                            manifest,
                            "remoteProverRequired",
                            "remoteProverRequired",
                            "remote_prover_required",
                        ),
                        "remoteProverRequired",
                    ),
                    browserImplementation = manifestString(
                        manifestField(
                            manifest,
                            "browserImplementation",
                            "browserImplementation",
                            "browser_implementation",
                        ),
                        "browserImplementation",
                    ),
                    nativeSdkArtifacts = manifestSdkArtifacts(
                        manifestField(
                            manifest,
                            "nativeSdkArtifacts",
                            "nativeSdkArtifacts",
                            "native_sdk_artifacts",
                            "sdkArtifacts",
                            "sdk_artifacts",
                        ),
                        proofArtifactHash,
                        provingKeyHash,
                    ),
                    crossSdkFixtureParityArtifact = manifestString(
                        manifestField(
                            manifest,
                            "crossSdkFixtureParityArtifact",
                            "crossSdkFixtureParityArtifact",
                            "cross_sdk_fixture_parity_artifact",
                        ),
                        "crossSdkFixtureParityArtifact",
                    ),
                    nativeProverSelfTestArtifact = manifestString(
                        manifestField(
                            manifest,
                            "nativeProverSelfTestArtifact",
                            "nativeProverSelfTestArtifact",
                            "native_prover_self_test_artifact",
                            "selfTestArtifact",
                            "self_test_artifact",
                        ),
                        "nativeProverSelfTestArtifact",
                    ),
                    auditHashes = manifestStringMap(
                        manifestField(manifest, "auditHashes", "auditHashes", "audit_hashes"),
                        "auditHashes",
                    ),
                    expectedDestinationBindingHash = expectedDestinationBindingHash,
                )
            }

            private fun manifestSdkArtifacts(
                value: Any?,
                proofArtifactHash: String,
                provingKeyHash: String,
            ): List<EthereumMainnetNativeEvmProverBundleSdkArtifact> {
                val list = value as? List<*> ?: throw IllegalArgumentException("nativeSdkArtifacts must be an array")
                require(list.isNotEmpty()) { "nativeSdkArtifacts must be non-empty" }
                val normalizedProofArtifactHash =
                    normalizeNativeEvmProverBundleHex32(proofArtifactHash, "proofArtifactHash")
                val normalizedProvingKeyHash =
                    normalizeNativeEvmProverBundleHex32(provingKeyHash, "provingKeyHash")
                return list.mapIndexed { index, item ->
                    val artifact = expectManifestObject(item, "nativeSdkArtifacts[$index]")
                    requireManifestKeys(
                        artifact,
                        "nativeSdkArtifacts[$index]",
                        NATIVE_EVM_PROVER_BUNDLE_SDK_ARTIFACT_KEYS,
                    )
                    val sdkProofArtifactHash = manifestString(
                        manifestField(
                            artifact,
                            "nativeSdkArtifacts[$index].proofArtifactHash",
                            "proofArtifactHash",
                            "proof_artifact_hash",
                            "proverArtifactHash",
                            "prover_artifact_hash",
                        ),
                        "nativeSdkArtifacts[$index].proofArtifactHash",
                    )
                    require(
                        normalizeNativeEvmProverBundleHex32(
                            sdkProofArtifactHash,
                            "nativeSdkArtifacts[$index].proofArtifactHash",
                        ) == normalizedProofArtifactHash,
                    ) { "nativeSdkArtifacts[$index].proofArtifactHash must match bundle" }
                    val sdkProvingKeyHash = manifestString(
                        manifestField(
                            artifact,
                            "nativeSdkArtifacts[$index].provingKeyHash",
                            "provingKeyHash",
                            "proving_key_hash",
                        ),
                        "nativeSdkArtifacts[$index].provingKeyHash",
                    )
                    require(
                        normalizeNativeEvmProverBundleHex32(
                            sdkProvingKeyHash,
                            "nativeSdkArtifacts[$index].provingKeyHash",
                        ) == normalizedProvingKeyHash,
                    ) { "nativeSdkArtifacts[$index].provingKeyHash must match bundle" }
                    EthereumMainnetNativeEvmProverBundleSdkArtifact(
                        sdk = manifestString(
                            manifestField(artifact, "nativeSdkArtifacts[$index].sdk", "sdk"),
                            "nativeSdkArtifacts[$index].sdk",
                        ),
                        implementation = manifestString(
                            manifestField(
                                artifact,
                                "nativeSdkArtifacts[$index].implementation",
                                "implementation",
                            ),
                            "nativeSdkArtifacts[$index].implementation",
                        ),
                        proofArtifactHash = sdkProofArtifactHash,
                        provingKeyHash = sdkProvingKeyHash,
                        implementationHash = manifestString(
                            manifestField(
                                artifact,
                                "nativeSdkArtifacts[$index].implementationHash",
                                "implementationHash",
                                "implementation_hash",
                            ),
                            "nativeSdkArtifacts[$index].implementationHash",
                        ),
                        implementationArtifact = manifestString(
                            manifestField(
                                artifact,
                                "nativeSdkArtifacts[$index].implementationArtifact",
                                "implementationArtifact",
                                "implementation_artifact",
                                "implementationPath",
                                "implementation_path",
                            ),
                            "nativeSdkArtifacts[$index].implementationArtifact",
                        ),
                    )
                }
            }
        }
    }

    /** One SDK row in an Ethereum mainnet native EVM prover parity fixture. */
    class EthereumMainnetNativeEvmProverParitySdkResult(
        receiptProofHash: String,
        sourceProofHash: String,
        destinationBindingHash: String,
        publicSignalWords: List<String>,
        calldataHash: String,
        toriiSubmitPayloadHash: String,
    ) {
        val receiptProofHash: String =
            normalizeNativeEvmProverBundleHex32(receiptProofHash, "receiptProofHash")
        val sourceProofHash: String =
            normalizeNativeEvmProverBundleHex32(sourceProofHash, "sourceProofHash")
        val destinationBindingHash: String =
            normalizeNativeEvmProverParityHex32(destinationBindingHash, "destinationBindingHash")
        val publicSignalWords: List<String>
        val calldataHash: String =
            normalizeNativeEvmProverBundleHex32(calldataHash, "calldataHash")
        val toriiSubmitPayloadHash: String =
            normalizeNativeEvmProverBundleHex32(toriiSubmitPayloadHash, "toriiSubmitPayloadHash")

        init {
            require(publicSignalWords.size == 9) { "publicSignalWords must contain 9 words" }
            this.publicSignalWords =
                publicSignalWords.mapIndexed { index, word ->
                    normalizeNativeEvmProverParityHex32(word, "publicSignalWords[$index]")
                }
        }

        override fun equals(other: Any?): Boolean =
            other is EthereumMainnetNativeEvmProverParitySdkResult &&
                receiptProofHash == other.receiptProofHash &&
                sourceProofHash == other.sourceProofHash &&
                destinationBindingHash == other.destinationBindingHash &&
                publicSignalWords == other.publicSignalWords &&
                calldataHash == other.calldataHash &&
                toriiSubmitPayloadHash == other.toriiSubmitPayloadHash

        override fun hashCode(): Int =
            listOf(
                receiptProofHash,
                sourceProofHash,
                destinationBindingHash,
                publicSignalWords,
                calldataHash,
                toriiSubmitPayloadHash,
            ).hashCode()
    }

    /** Cross-SDK output parity fixture for an Ethereum mainnet native EVM prover bundle. */
    class EthereumMainnetNativeEvmProverParityFixture(
        nativeProverBundle: EthereumMainnetNativeEvmProverBundle,
        schema: String = ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1,
        domain: Int = DOMAIN_ETH,
        chain: String = "eth",
        proofBackend: String = GROTH16_BN254_PROOF_BACKEND_V1,
        proofArtifactHash: String,
        provingKeyHash: String,
        verifierKeyHash: String,
        destinationBindingHash: String,
        receiptProofHash: String,
        sourceProofHash: String,
        publicSignalWords: List<String>,
        calldataHash: String,
        toriiSubmitPayloadHash: String,
        sdkResults: Map<String, EthereumMainnetNativeEvmProverParitySdkResult>,
    ) {
        val schema: String = schema
        val domain: Int = domain
        val chain: String = chain
        val proofBackend: String = proofBackend
        val proofArtifactHash: String =
            normalizeNativeEvmProverBundleHex32(proofArtifactHash, "proofArtifactHash")
        val provingKeyHash: String =
            normalizeNativeEvmProverBundleHex32(provingKeyHash, "provingKeyHash")
        val verifierKeyHash: String =
            normalizeNativeEvmProverBundleHex32(verifierKeyHash, "verifierKeyHash")
        val destinationBindingHash: String =
            normalizeNativeEvmProverParityHex32(destinationBindingHash, "destinationBindingHash")
        val receiptProofHash: String =
            normalizeNativeEvmProverBundleHex32(receiptProofHash, "receiptProofHash")
        val sourceProofHash: String =
            normalizeNativeEvmProverBundleHex32(sourceProofHash, "sourceProofHash")
        val publicSignalWords: List<String>
        val calldataHash: String =
            normalizeNativeEvmProverBundleHex32(calldataHash, "calldataHash")
        val toriiSubmitPayloadHash: String =
            normalizeNativeEvmProverBundleHex32(toriiSubmitPayloadHash, "toriiSubmitPayloadHash")
        val sdkResults: Map<String, EthereumMainnetNativeEvmProverParitySdkResult>

        init {
            require(schema == ETH_NATIVE_EVM_PROVER_PARITY_FIXTURE_SCHEMA_V1) {
                "nativeProverParityFixture.schema is not supported"
            }
            require(domain == DOMAIN_ETH && domain == nativeProverBundle.domain) {
                "nativeProverParityFixture.domain must match nativeProverBundle"
            }
            require(chain == nativeProverBundle.chain) {
                "nativeProverParityFixture.chain must match nativeProverBundle"
            }
            require(proofBackend == nativeProverBundle.proofBackend) {
                "nativeProverParityFixture.proofBackend must match nativeProverBundle"
            }
            require(this.proofArtifactHash == nativeProverBundle.proofArtifactHash) {
                "nativeProverParityFixture.proofArtifactHash must match nativeProverBundle"
            }
            require(this.provingKeyHash == nativeProverBundle.provingKeyHash) {
                "nativeProverParityFixture.provingKeyHash must match nativeProverBundle"
            }
            require(this.verifierKeyHash == nativeProverBundle.verifierKeyHash) {
                "nativeProverParityFixture.verifierKeyHash must match nativeProverBundle"
            }
            require(this.destinationBindingHash == nativeProverBundle.destinationBindingHash) {
                "nativeProverParityFixture.destinationBindingHash must match nativeProverBundle"
            }
            require(publicSignalWords.size == 9) { "publicSignalWords must contain 9 words" }
            this.publicSignalWords =
                publicSignalWords.mapIndexed { index, word ->
                    normalizeNativeEvmProverParityHex32(word, "publicSignalWords[$index]")
                }
            require(sdkResults.keys == ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1.keys) {
                "sdkResults must contain exactly the required SDKs"
            }
            sdkResults.forEach { (sdk, result) ->
                require(result.receiptProofHash == this.receiptProofHash) {
                    "sdkResults.$sdk.receiptProofHash must match receiptProofHash"
                }
                require(result.sourceProofHash == this.sourceProofHash) {
                    "sdkResults.$sdk.sourceProofHash must match sourceProofHash"
                }
                require(result.destinationBindingHash == this.destinationBindingHash) {
                    "sdkResults.$sdk.destinationBindingHash must match destinationBindingHash"
                }
                require(result.publicSignalWords == this.publicSignalWords) {
                    "sdkResults.$sdk.publicSignalWords must match publicSignalWords"
                }
                require(result.calldataHash == this.calldataHash) {
                    "sdkResults.$sdk.calldataHash must match calldataHash"
                }
                require(result.toriiSubmitPayloadHash == this.toriiSubmitPayloadHash) {
                    "sdkResults.$sdk.toriiSubmitPayloadHash must match toriiSubmitPayloadHash"
                }
            }
            this.sdkResults = sdkResults.toSortedMap()
        }

        companion object {
            @JvmStatic
            fun fromJson(
                json: String,
                nativeProverBundle: EthereumMainnetNativeEvmProverBundle,
            ): EthereumMainnetNativeEvmProverParityFixture {
                val parsed = try {
                    JsonParser.parse(json)
                } catch (ex: IllegalStateException) {
                    throw IllegalArgumentException(
                        "nativeProverParityFixture JSON is invalid: ${ex.message}",
                        ex,
                    )
                }
                return fromMap(expectManifestObject(parsed, "nativeProverParityFixture"), nativeProverBundle)
            }

            @JvmStatic
            fun fromJsonBytes(
                payload: ByteArray,
                nativeProverBundle: EthereumMainnetNativeEvmProverBundle,
            ): EthereumMainnetNativeEvmProverParityFixture =
                fromJson(String(payload, StandardCharsets.UTF_8), nativeProverBundle)

            @JvmStatic
            fun fromMap(
                fixture: Map<String, Any?>,
                nativeProverBundle: EthereumMainnetNativeEvmProverBundle,
            ): EthereumMainnetNativeEvmProverParityFixture {
                requireManifestKeys(
                    fixture,
                    "nativeProverParityFixture",
                    NATIVE_EVM_PROVER_PARITY_FIXTURE_KEYS,
                )
                val publicSignalWords = manifestStringList(
                    manifestField(
                        fixture,
                        "publicSignalWords",
                        "publicSignalWords",
                        "public_signal_words",
                    ),
                    "publicSignalWords",
                )
                val sdkResultsInput =
                    expectManifestObject(
                        manifestField(fixture, "sdkResults", "sdkResults", "sdk_results"),
                        "sdkResults",
                    )
                val sdkResults = sdkResultsInput.keys.sorted().associateWith { sdk ->
                    val result = expectManifestObject(sdkResultsInput[sdk], "sdkResults.$sdk")
                    requireManifestKeys(
                        result,
                        "sdkResults.$sdk",
                        NATIVE_EVM_PROVER_PARITY_SDK_RESULT_KEYS,
                    )
                    EthereumMainnetNativeEvmProverParitySdkResult(
                        receiptProofHash = manifestString(
                            manifestField(result, "sdkResults.$sdk.receiptProofHash", "receiptProofHash", "receipt_proof_hash"),
                            "sdkResults.$sdk.receiptProofHash",
                        ),
                        sourceProofHash = manifestString(
                            manifestField(result, "sdkResults.$sdk.sourceProofHash", "sourceProofHash", "source_proof_hash"),
                            "sdkResults.$sdk.sourceProofHash",
                        ),
                        destinationBindingHash = manifestString(
                            manifestField(
                                result,
                                "sdkResults.$sdk.destinationBindingHash",
                                "destinationBindingHash",
                                "destination_binding_hash",
                            ),
                            "sdkResults.$sdk.destinationBindingHash",
                        ),
                        publicSignalWords = manifestStringList(
                            manifestField(
                                result,
                                "sdkResults.$sdk.publicSignalWords",
                                "publicSignalWords",
                                "public_signal_words",
                            ),
                            "sdkResults.$sdk.publicSignalWords",
                        ),
                        calldataHash = manifestString(
                            manifestField(result, "sdkResults.$sdk.calldataHash", "calldataHash", "calldata_hash"),
                            "sdkResults.$sdk.calldataHash",
                        ),
                        toriiSubmitPayloadHash = manifestString(
                            manifestField(
                                result,
                                "sdkResults.$sdk.toriiSubmitPayloadHash",
                                "toriiSubmitPayloadHash",
                                "torii_submit_payload_hash",
                            ),
                            "sdkResults.$sdk.toriiSubmitPayloadHash",
                        ),
                    )
                }
                return EthereumMainnetNativeEvmProverParityFixture(
                    nativeProverBundle = nativeProverBundle,
                    schema = manifestString(manifestField(fixture, "schema", "schema"), "schema"),
                    domain = manifestDomain(manifestField(fixture, "domain", "domain"), "domain"),
                    chain = manifestString(manifestField(fixture, "chain", "chain"), "chain"),
                    proofBackend = manifestString(
                        manifestField(fixture, "proofBackend", "proofBackend", "proof_backend", "backend"),
                        "proofBackend",
                    ),
                    proofArtifactHash = manifestString(
                        manifestField(
                            fixture,
                            "proofArtifactHash",
                            "proofArtifactHash",
                            "proof_artifact_hash",
                            "proverArtifactHash",
                            "prover_artifact_hash",
                            "circuitArtifactHash",
                            "circuit_artifact_hash",
                        ),
                        "proofArtifactHash",
                    ),
                    provingKeyHash = manifestString(
                        manifestField(fixture, "provingKeyHash", "provingKeyHash", "proving_key_hash"),
                        "provingKeyHash",
                    ),
                    verifierKeyHash = manifestString(
                        manifestField(fixture, "verifierKeyHash", "verifierKeyHash", "verifier_key_hash"),
                        "verifierKeyHash",
                    ),
                    destinationBindingHash = manifestString(
                        manifestField(
                            fixture,
                            "destinationBindingHash",
                            "destinationBindingHash",
                            "destination_binding_hash",
                        ),
                        "destinationBindingHash",
                    ),
                    receiptProofHash = manifestString(
                        manifestField(fixture, "receiptProofHash", "receiptProofHash", "receipt_proof_hash"),
                        "receiptProofHash",
                    ),
                    sourceProofHash = manifestString(
                        manifestField(fixture, "sourceProofHash", "sourceProofHash", "source_proof_hash"),
                        "sourceProofHash",
                    ),
                    publicSignalWords = publicSignalWords,
                    calldataHash = manifestString(
                        manifestField(fixture, "calldataHash", "calldataHash", "calldata_hash"),
                        "calldataHash",
                    ),
                    toriiSubmitPayloadHash = manifestString(
                        manifestField(
                            fixture,
                            "toriiSubmitPayloadHash",
                            "toriiSubmitPayloadHash",
                            "torii_submit_payload_hash",
                        ),
                        "toriiSubmitPayloadHash",
                    ),
                    sdkResults = sdkResults,
                )
            }
        }
    }

    data class EthereumMainnetNativeEvmProverSelfTestSdkResult(
        val requestHash: String,
        val witnessHash: String,
        val sourceProofHash: String,
        val proofHash: String,
        val publicSignalWords: List<String>,
        val calldataHash: String,
        val toriiSubmitPayloadHash: String,
    ) {
        init {
            normalizeNativeEvmProverBundleHex32(requestHash, "requestHash")
            normalizeNativeEvmProverBundleHex32(witnessHash, "witnessHash")
            normalizeNativeEvmProverBundleHex32(sourceProofHash, "sourceProofHash")
            normalizeNativeEvmProverBundleHex32(proofHash, "proofHash")
            require(publicSignalWords.size == 9) { "publicSignalWords must contain 9 words" }
            publicSignalWords.forEachIndexed { index, word ->
                normalizeNativeEvmProverParityHex32(word, "publicSignalWords[$index]")
            }
            normalizeNativeEvmProverBundleHex32(calldataHash, "calldataHash")
            normalizeNativeEvmProverBundleHex32(toriiSubmitPayloadHash, "toriiSubmitPayloadHash")
        }
    }

    class EthereumMainnetNativeEvmProverSelfTestFixture(
        nativeProverBundle: EthereumMainnetNativeEvmProverBundle,
        schema: String = ETH_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1,
        domain: Int = DOMAIN_ETH,
        chain: String = "eth",
        proofBackend: String = GROTH16_BN254_PROOF_BACKEND_V1,
        proofArtifactHash: String,
        provingKeyHash: String,
        verifierKeyHash: String,
        destinationBindingHash: String,
        requestHash: String,
        witnessHash: String,
        sourceProofHash: String,
        proofHash: String,
        publicSignalWords: List<String>,
        calldataHash: String,
        toriiSubmitPayloadHash: String,
        sdkResults: Map<String, EthereumMainnetNativeEvmProverSelfTestSdkResult>,
    ) {
        val schema: String = schema
        val domain: Int = domain
        val chain: String = chain
        val proofBackend: String = proofBackend
        val proofArtifactHash: String =
            normalizeNativeEvmProverBundleHex32(proofArtifactHash, "proofArtifactHash")
        val provingKeyHash: String =
            normalizeNativeEvmProverBundleHex32(provingKeyHash, "provingKeyHash")
        val verifierKeyHash: String =
            normalizeNativeEvmProverBundleHex32(verifierKeyHash, "verifierKeyHash")
        val destinationBindingHash: String =
            normalizeNativeEvmProverParityHex32(destinationBindingHash, "destinationBindingHash")
        val requestHash: String =
            normalizeNativeEvmProverBundleHex32(requestHash, "requestHash")
        val witnessHash: String =
            normalizeNativeEvmProverBundleHex32(witnessHash, "witnessHash")
        val sourceProofHash: String =
            normalizeNativeEvmProverBundleHex32(sourceProofHash, "sourceProofHash")
        val proofHash: String =
            normalizeNativeEvmProverBundleHex32(proofHash, "proofHash")
        val publicSignalWords: List<String>
        val calldataHash: String =
            normalizeNativeEvmProverBundleHex32(calldataHash, "calldataHash")
        val toriiSubmitPayloadHash: String =
            normalizeNativeEvmProverBundleHex32(toriiSubmitPayloadHash, "toriiSubmitPayloadHash")
        val sdkResults: Map<String, EthereumMainnetNativeEvmProverSelfTestSdkResult>

        init {
            require(schema == ETH_NATIVE_EVM_PROVER_SELF_TEST_SCHEMA_V1) {
                "nativeProverSelfTestFixture.schema is not supported"
            }
            require(domain == DOMAIN_ETH && domain == nativeProverBundle.domain) {
                "nativeProverSelfTestFixture.domain must match nativeProverBundle"
            }
            require(chain == nativeProverBundle.chain) {
                "nativeProverSelfTestFixture.chain must match nativeProverBundle"
            }
            require(proofBackend == nativeProverBundle.proofBackend) {
                "nativeProverSelfTestFixture.proofBackend must match nativeProverBundle"
            }
            require(this.proofArtifactHash == nativeProverBundle.proofArtifactHash) {
                "nativeProverSelfTestFixture.proofArtifactHash must match nativeProverBundle"
            }
            require(this.provingKeyHash == nativeProverBundle.provingKeyHash) {
                "nativeProverSelfTestFixture.provingKeyHash must match nativeProverBundle"
            }
            require(this.verifierKeyHash == nativeProverBundle.verifierKeyHash) {
                "nativeProverSelfTestFixture.verifierKeyHash must match nativeProverBundle"
            }
            require(this.destinationBindingHash == nativeProverBundle.destinationBindingHash) {
                "nativeProverSelfTestFixture.destinationBindingHash must match nativeProverBundle"
            }
            require(publicSignalWords.size == 9) { "publicSignalWords must contain 9 words" }
            this.publicSignalWords =
                publicSignalWords.mapIndexed { index, word ->
                    normalizeNativeEvmProverParityHex32(word, "publicSignalWords[$index]")
                }
            require(sdkResults.keys == ETH_NATIVE_EVM_PROVER_REQUIRED_IMPLEMENTATIONS_V1.keys) {
                "sdkResults must contain exactly the required SDKs"
            }
            sdkResults.forEach { (sdk, result) ->
                require(result.requestHash == this.requestHash) {
                    "sdkResults.$sdk.requestHash must match requestHash"
                }
                require(result.witnessHash == this.witnessHash) {
                    "sdkResults.$sdk.witnessHash must match witnessHash"
                }
                require(result.sourceProofHash == this.sourceProofHash) {
                    "sdkResults.$sdk.sourceProofHash must match sourceProofHash"
                }
                require(result.proofHash == this.proofHash) {
                    "sdkResults.$sdk.proofHash must match proofHash"
                }
                require(result.publicSignalWords == this.publicSignalWords) {
                    "sdkResults.$sdk.publicSignalWords must match publicSignalWords"
                }
                require(result.calldataHash == this.calldataHash) {
                    "sdkResults.$sdk.calldataHash must match calldataHash"
                }
                require(result.toriiSubmitPayloadHash == this.toriiSubmitPayloadHash) {
                    "sdkResults.$sdk.toriiSubmitPayloadHash must match toriiSubmitPayloadHash"
                }
            }
            this.sdkResults = sdkResults.toSortedMap()
        }

        companion object {
            @JvmStatic
            fun fromJson(
                json: String,
                nativeProverBundle: EthereumMainnetNativeEvmProverBundle,
            ): EthereumMainnetNativeEvmProverSelfTestFixture {
                val parsed = try {
                    JsonParser.parse(json)
                } catch (ex: IllegalStateException) {
                    throw IllegalArgumentException(
                        "nativeProverSelfTestFixture JSON is invalid: ${ex.message}",
                        ex,
                    )
                }
                return fromMap(expectManifestObject(parsed, "nativeProverSelfTestFixture"), nativeProverBundle)
            }

            @JvmStatic
            fun fromJsonBytes(
                payload: ByteArray,
                nativeProverBundle: EthereumMainnetNativeEvmProverBundle,
            ): EthereumMainnetNativeEvmProverSelfTestFixture =
                fromJson(String(payload, StandardCharsets.UTF_8), nativeProverBundle)

            @JvmStatic
            fun fromMap(
                fixture: Map<String, Any?>,
                nativeProverBundle: EthereumMainnetNativeEvmProverBundle,
            ): EthereumMainnetNativeEvmProverSelfTestFixture {
                requireManifestKeys(
                    fixture,
                    "nativeProverSelfTestFixture",
                    NATIVE_EVM_PROVER_SELF_TEST_FIXTURE_KEYS,
                )
                val publicSignalWords = manifestStringList(
                    manifestField(
                        fixture,
                        "publicSignalWords",
                        "publicSignalWords",
                        "public_signal_words",
                    ),
                    "publicSignalWords",
                )
                val sdkResultsInput =
                    expectManifestObject(
                        manifestField(fixture, "sdkResults", "sdkResults", "sdk_results"),
                        "sdkResults",
                    )
                val sdkResults = sdkResultsInput.keys.sorted().associateWith { sdk ->
                    val result = expectManifestObject(sdkResultsInput[sdk], "sdkResults.$sdk")
                    requireManifestKeys(
                        result,
                        "sdkResults.$sdk",
                        NATIVE_EVM_PROVER_SELF_TEST_SDK_RESULT_KEYS,
                    )
                    EthereumMainnetNativeEvmProverSelfTestSdkResult(
                        requestHash = manifestString(
                            manifestField(result, "sdkResults.$sdk.requestHash", "requestHash", "request_hash"),
                            "sdkResults.$sdk.requestHash",
                        ),
                        witnessHash = manifestString(
                            manifestField(result, "sdkResults.$sdk.witnessHash", "witnessHash", "witness_hash"),
                            "sdkResults.$sdk.witnessHash",
                        ),
                        sourceProofHash = manifestString(
                            manifestField(result, "sdkResults.$sdk.sourceProofHash", "sourceProofHash", "source_proof_hash"),
                            "sdkResults.$sdk.sourceProofHash",
                        ),
                        proofHash = manifestString(
                            manifestField(result, "sdkResults.$sdk.proofHash", "proofHash", "proof_hash"),
                            "sdkResults.$sdk.proofHash",
                        ),
                        publicSignalWords = manifestStringList(
                            manifestField(
                                result,
                                "sdkResults.$sdk.publicSignalWords",
                                "publicSignalWords",
                                "public_signal_words",
                            ),
                            "sdkResults.$sdk.publicSignalWords",
                        ),
                        calldataHash = manifestString(
                            manifestField(result, "sdkResults.$sdk.calldataHash", "calldataHash", "calldata_hash"),
                            "sdkResults.$sdk.calldataHash",
                        ),
                        toriiSubmitPayloadHash = manifestString(
                            manifestField(
                                result,
                                "sdkResults.$sdk.toriiSubmitPayloadHash",
                                "toriiSubmitPayloadHash",
                                "torii_submit_payload_hash",
                            ),
                            "sdkResults.$sdk.toriiSubmitPayloadHash",
                        ),
                    )
                }
                return EthereumMainnetNativeEvmProverSelfTestFixture(
                    nativeProverBundle = nativeProverBundle,
                    schema = manifestString(manifestField(fixture, "schema", "schema"), "schema"),
                    domain = manifestDomain(manifestField(fixture, "domain", "domain"), "domain"),
                    chain = manifestString(manifestField(fixture, "chain", "chain"), "chain"),
                    proofBackend = manifestString(
                        manifestField(fixture, "proofBackend", "proofBackend", "proof_backend", "backend"),
                        "proofBackend",
                    ),
                    proofArtifactHash = manifestString(
                        manifestField(
                            fixture,
                            "proofArtifactHash",
                            "proofArtifactHash",
                            "proof_artifact_hash",
                            "proverArtifactHash",
                            "prover_artifact_hash",
                            "circuitArtifactHash",
                            "circuit_artifact_hash",
                        ),
                        "proofArtifactHash",
                    ),
                    provingKeyHash = manifestString(
                        manifestField(fixture, "provingKeyHash", "provingKeyHash", "proving_key_hash"),
                        "provingKeyHash",
                    ),
                    verifierKeyHash = manifestString(
                        manifestField(fixture, "verifierKeyHash", "verifierKeyHash", "verifier_key_hash"),
                        "verifierKeyHash",
                    ),
                    destinationBindingHash = manifestString(
                        manifestField(
                            fixture,
                            "destinationBindingHash",
                            "destinationBindingHash",
                            "destination_binding_hash",
                        ),
                        "destinationBindingHash",
                    ),
                    requestHash = manifestString(
                        manifestField(fixture, "requestHash", "requestHash", "request_hash"),
                        "requestHash",
                    ),
                    witnessHash = manifestString(
                        manifestField(fixture, "witnessHash", "witnessHash", "witness_hash"),
                        "witnessHash",
                    ),
                    sourceProofHash = manifestString(
                        manifestField(fixture, "sourceProofHash", "sourceProofHash", "source_proof_hash"),
                        "sourceProofHash",
                    ),
                    proofHash = manifestString(
                        manifestField(fixture, "proofHash", "proofHash", "proof_hash"),
                        "proofHash",
                    ),
                    publicSignalWords = publicSignalWords,
                    calldataHash = manifestString(
                        manifestField(fixture, "calldataHash", "calldataHash", "calldata_hash"),
                        "calldataHash",
                    ),
                    toriiSubmitPayloadHash = manifestString(
                        manifestField(
                            fixture,
                            "toriiSubmitPayloadHash",
                            "toriiSubmitPayloadHash",
                            "torii_submit_payload_hash",
                        ),
                        "toriiSubmitPayloadHash",
                    ),
                    sdkResults = sdkResults,
                )
            }
        }
    }

    data class EthereumMainnetNativeEvmProverArtifacts(
        val hashAlgorithm: String,
        val nativeProverBundle: EthereumMainnetNativeEvmProverBundle,
        val proofArtifactHash: String,
        val provingKeyHash: String,
        val verifierKeyHash: String,
        val crossSdkFixtureParityHash: String? = null,
        val crossSdkFixtureParity: EthereumMainnetNativeEvmProverParityFixture? = null,
        val nativeProverSelfTestHash: String? = null,
        val nativeProverSelfTest: EthereumMainnetNativeEvmProverSelfTestFixture? = null,
        val sdk: String? = null,
        val implementation: String? = null,
        val implementationHash: String? = null,
    )

    @Suppress("UNCHECKED_CAST")
    private fun expectManifestObject(value: Any?, label: String): Map<String, Any?> {
        val raw = value as? Map<*, *> ?: throw IllegalArgumentException("$label must be an object")
        val out = LinkedHashMap<String, Any?>()
        raw.forEach { (key, item) ->
            require(key is String) { "$label keys must be strings" }
            out[key] = item
        }
        return out
    }

    private fun manifestField(
        manifest: Map<String, Any?>,
        label: String,
        vararg aliases: String,
    ): Any? {
        val present = aliases.filter { alias -> manifest.containsKey(alias) }
        require(present.size <= 1) { "$label must not use multiple aliases" }
        if (present.isNotEmpty()) {
            return manifest[present[0]]
        }
        throw IllegalArgumentException("$label is required")
    }

    private fun requireManifestKeys(
        manifest: Map<String, Any?>,
        label: String,
        allowedKeys: Set<String>,
    ) {
        manifest.keys.forEach { key ->
            require(allowedKeys.contains(key)) { "$label contains unknown field: $key" }
        }
    }

    private fun manifestString(value: Any?, label: String): String =
        value as? String ?: throw IllegalArgumentException("$label must be a string")

    private fun manifestStringList(value: Any?, label: String): List<String> {
        val list = value as? List<*> ?: throw IllegalArgumentException("$label must be an array")
        return list.mapIndexed { index, item ->
            manifestString(item, "$label[$index]")
        }
    }

    private fun manifestBoolean(value: Any?, label: String): Boolean =
        value as? Boolean ?: throw IllegalArgumentException("$label must be a boolean")

    private fun manifestDomain(value: Any?, label: String): Int =
        when (value) {
            is Int -> value
            is Long -> {
                require(value in 0..Int.MAX_VALUE.toLong()) { "$label must fit u32" }
                value.toInt()
            }
            is BigInteger -> {
                require(value.signum() >= 0 && value.bitLength() <= 31) { "$label must fit u32" }
                value.toInt()
            }
            is String -> {
                require(isCanonicalDecimalText(value)) { "$label must be a canonical decimal integer" }
                val numeric = BigInteger(value)
                require(numeric.signum() >= 0 && numeric.bitLength() <= 31) { "$label must fit u32" }
                numeric.toInt()
            }
            else -> throw IllegalArgumentException("$label must be an integer")
        }

    private fun manifestStringMap(value: Any?, label: String): Map<String, String> {
        val map = expectManifestObject(value, label)
        require(map.isNotEmpty()) { "$label must be non-empty" }
        return map.keys.sorted().associateWith { key ->
            manifestString(map[key], "$label.$key")
        }
    }

    @JvmStatic
    fun canonicalPublicInputsBytes(input: EvmSccpPublicInputsInput): ByteArray {
        require(input.version == 1) { "publicInputs.version must be 1" }
        require(input.targetDomain != 0) { "publicInputs.targetDomain must not be zero" }
        require(input.targetDomain == DOMAIN_ETH || input.targetDomain == DOMAIN_BSC) {
            "publicInputs.targetDomain must be ETH or BSC"
        }
        val out = ByteArrayOutputStream()
        out.write(input.version)
        out.write(nonZeroHex32Bytes(input.messageId, "messageId"))
        out.write(nonZeroHex32Bytes(input.payloadHash, "payloadHash"))
        writeU32Le(out, input.targetDomain)
        out.write(nonZeroHex32Bytes(input.commitmentRoot, "commitmentRoot"))
        writeU64Le(out, normalizeU64(input.finalityHeight, "finalityHeight"))
        out.write(nonZeroHex32Bytes(input.finalityBlockHash, "finalityBlockHash"))
        return out.toByteArray()
    }

    @JvmStatic
    fun groth16Bn254PublicSignalWords(
        publicInputs: EvmSccpPublicInputsInput,
        sourceDomain: Int,
        statementHash: String,
        destinationBindingHash: String,
    ): List<String> {
        require(publicInputs.version == 1) { "publicInputs.version must be 1" }
        require(publicInputs.targetDomain != 0) { "publicInputs.targetDomain must not be zero" }
        require(publicInputs.targetDomain == DOMAIN_ETH || publicInputs.targetDomain == DOMAIN_BSC) {
            "publicInputs.targetDomain must be ETH or BSC"
        }
        require(sourceDomain == DOMAIN_SORA) { "sourceDomain must be SORA" }
        require(sourceDomain != publicInputs.targetDomain) {
            "sourceDomain and publicInputs.targetDomain must differ"
        }
        val values = listOf(
            nonZeroHex32Bytes(publicInputs.messageId, "messageId"),
            nonZeroHex32Bytes(publicInputs.payloadHash, "payloadHash"),
            abiWordU32(publicInputs.targetDomain),
            nonZeroHex32Bytes(publicInputs.commitmentRoot, "commitmentRoot"),
            abiWordU64(normalizeU64(publicInputs.finalityHeight, "finalityHeight")),
            nonZeroHex32Bytes(publicInputs.finalityBlockHash, "finalityBlockHash"),
            abiWordU32(sourceDomain),
            nonZeroHex32Bytes(statementHash, "statementHash"),
            nonZeroHex32Bytes(destinationBindingHash, "destinationBindingHash"),
        )
        return values.indices.map { index -> groth16SignalWord(SIGNAL_LABELS[index], values[index]) }
    }

    @JvmStatic
    fun buildProofRequest(input: EvmSccpProofRequestInput): EvmSccpProofRequest {
        require(input.backend == GROTH16_BN254_PROOF_BACKEND_V1) {
            "backend must be evm-groth16-bn254-v1"
        }
        val bundleBytes = input.bundleBytes.copyOf()
        val sourceProofBytes = requireOptionalSourceProofBytes(input.sourceProofBytes, "sourceProofBytes")
        require(bundleBytes.isNotEmpty()) { "bundleBytes must not be empty" }
        val publicInputsBytes = canonicalPublicInputsBytes(input.publicInputs)
        val proofContext = normalizeProofContext(input.statementHash, input.destinationBindingHash)
        val proverArtifacts = normalizeOptionalGroth16ProverArtifacts(
            input.proofArtifactHash,
            input.provingKeyHash,
        )
        val publicSignalWords = groth16Bn254PublicSignalWords(
            publicInputs = input.publicInputs,
            sourceDomain = input.sourceDomain,
            statementHash = proofContext.statementHash,
            destinationBindingHash = proofContext.destinationBindingHash,
        )
        val preimage = ByteArrayOutputStream()
        preimage.write(publicInputsBytes)
        writeU32Le(preimage, bundleBytes.size)
        preimage.write(bundleBytes)
        writeU32Le(preimage, sourceProofBytes.size)
        preimage.write(sourceProofBytes)
        preimage.write(hex32Bytes(proofContext.statementHash, "statementHash"))
        preimage.write(hex32Bytes(proofContext.destinationBindingHash, "destinationBindingHash"))
        if (proverArtifacts != null) {
            preimage.write(hex32Bytes(proverArtifacts.proofArtifactHash, "proofArtifactHash"))
            preimage.write(hex32Bytes(proverArtifacts.provingKeyHash, "provingKeyHash"))
        }
        publicSignalWords.forEach { preimage.write(hex32Bytes(it, "publicSignalWords")) }
        return EvmSccpProofRequest(
            version = 1,
            backend = input.backend,
            sourceDomain = input.sourceDomain,
            targetDomain = input.publicInputs.targetDomain,
            publicInputs = input.publicInputs,
            publicInputsBytes = publicInputsBytes,
            publicSignalWords = publicSignalWords,
            bundleBytes = bundleBytes,
            sourceProofBytes = sourceProofBytes,
            proofContext = proofContext,
            statementHash = proofContext.statementHash,
            destinationBindingHash = proofContext.destinationBindingHash,
            requestHash = hashHex(PROOF_REQUEST_PREFIX_V1, preimage.toByteArray()),
            destinationBinding = input.destinationBinding,
            proofArtifactHash = proverArtifacts?.proofArtifactHash,
            provingKeyHash = proverArtifacts?.provingKeyHash,
        )
    }

    @JvmStatic
    fun wrapProofResult(proofBytes: ByteArray, request: EvmSccpProofRequest): EvmSccpProofResult {
        require(request.backend == GROTH16_BN254_PROOF_BACKEND_V1) {
            "backend must be evm-groth16-bn254-v1"
        }
        requireProductionProofRequest(request)
        requireProofBytesForContext(proofBytes, request.publicInputs, request.sourceDomain)
        val envelopePayload = ByteArrayOutputStream()
        envelopePayload.write(hex32Bytes(request.requestHash, "requestHash"))
        envelopePayload.write(proofBytes)
        return EvmSccpProofResult(
            version = 1,
            backend = request.backend,
            proofBytes = proofBytes.copyOf(),
            proofBase64 = Base64.getEncoder().encodeToString(proofBytes),
            publicInputs = request.publicInputs,
            publicSignalWords = request.publicSignalWords,
            bundleBytes = request.bundleBytes,
            sourceProofBytes = request.sourceProofBytes,
            proofContext = request.proofContext,
            statementHash = request.statementHash,
            destinationBindingHash = request.destinationBindingHash,
            requestHash = request.requestHash,
            envelopeHash = hashHex(PROOF_ENVELOPE_PREFIX_V1, envelopePayload.toByteArray()),
            destinationBinding = request.destinationBinding,
            proofArtifactHash = request.proofArtifactHash,
            provingKeyHash = request.provingKeyHash,
        )
    }

    internal fun requireWrappedProofResultForSubmission(
        proofResult: EvmSccpProofResult,
    ): EvmSccpProofResult {
        require(proofResult.backend == GROTH16_BN254_PROOF_BACKEND_V1) {
            "proofResult.backend must be evm-groth16-bn254-v1"
        }
        val expectedProofContext = normalizeProofContext(
            proofResult.statementHash,
            proofResult.destinationBindingHash,
        )
        require(proofResult.proofContext == expectedProofContext) {
            "proofResult.proofContext must match statementHash and destinationBindingHash"
        }
        val proofBytes = proofResult.proofBytes
        requireProofBytesForPublicInputs(proofBytes, proofResult.publicInputs)
        require(proofResult.proofBase64 == Base64.getEncoder().encodeToString(proofBytes)) {
            "proofResult.proofBase64 must match proofResult.proofBytes"
        }
        val requestHash = normalizeNonZeroHex32(proofResult.requestHash, "proofResult.requestHash")
        val envelopeHash = normalizeNonZeroHex32(proofResult.envelopeHash, "proofResult.envelopeHash")
        val envelopePayload = ByteArrayOutputStream()
        envelopePayload.write(hex32Bytes(requestHash, "proofResult.requestHash"))
        envelopePayload.write(proofBytes)
        require(envelopeHash == hashHex(PROOF_ENVELOPE_PREFIX_V1, envelopePayload.toByteArray())) {
            "proofResult.envelopeHash must match wrapped proof bytes"
        }
        val sourceProofBytes = requireOptionalSourceProofBytes(
            proofResult.sourceProofBytes,
            "proofResult.sourceProofBytes",
        )
        val expectedRequest = buildProofRequest(
            EvmSccpProofRequestInput(
                publicInputs = proofResult.publicInputs,
                bundleBytes = proofResult.bundleBytes,
                sourceProofBytes = sourceProofBytes,
                statementHash = proofResult.statementHash,
                destinationBindingHash = proofResult.destinationBindingHash,
                backend = proofResult.backend,
                sourceDomain = DOMAIN_SORA,
                destinationBinding = proofResult.destinationBinding,
                proofArtifactHash = proofResult.proofArtifactHash,
                provingKeyHash = proofResult.provingKeyHash,
            ),
        )
        require(expectedRequest.requestHash == requestHash) {
            "proofResult.requestHash must match bundleBytes and sourceProofBytes"
        }
        return proofResult
    }

    @JvmStatic
    fun messageTransparentPublicInputAbiWords(publicInputs: EvmSccpPublicInputsInput): List<String> =
        messageTransparentPublicInputAbiWordBytes(publicInputs).map { "0x" + hexLower(it) }

    @JvmStatic
    fun submitMessageProofCallData(
        proofBytes: ByteArray,
        publicInputs: EvmSccpPublicInputsInput,
        statementHash: String,
    ): ByteArray = submitMessageProofCallData(proofBytes, publicInputs, statementHash, DOMAIN_SORA)

    @JvmStatic
    fun submitMessageProofCallData(
        proofBytes: ByteArray,
        publicInputs: EvmSccpPublicInputsInput,
        statementHash: String,
        sourceDomain: Int,
    ): ByteArray {
        require(sourceDomain == DOMAIN_SORA) { "sourceDomain must be SORA" }
        requireProofBytesForContext(proofBytes, publicInputs, sourceDomain)
        val publicInputWords = messageTransparentPublicInputAbiWordBytes(publicInputs)
        val out = ByteArrayOutputStream()
        out.write(SUBMIT_MESSAGE_PROOF_SELECTOR_BYTES_V1)
        out.write(abiWordU256(32 * 8))
        publicInputWords.forEach { out.write(it) }
        out.write(nonZeroHex32Bytes(statementHash, "statementHash"))
        out.write(abiWordU256(proofBytes.size))
        out.write(proofBytes)
        val padding = (32 - (proofBytes.size % 32)) % 32
        if (padding > 0) out.write(ByteArray(padding))
        return out.toByteArray()
    }

    @JvmStatic
    fun buildSubmission(input: EvmSccpSubmissionInput): EvmSccpSubmission {
        require(input.sourceDomain == DOMAIN_SORA) { "sourceDomain must be SORA" }
        val publicInputs = input.publicInputs
        canonicalPublicInputsBytes(publicInputs)
        val proofBytes = input.proofBytes
        requireProofBytesForContext(proofBytes, publicInputs, input.sourceDomain)
        val statementHash = normalizeNonZeroHex32(input.statementHash, "statementHash")
        val destinationBindingHash =
            normalizeNonZeroHex32(input.destinationBindingHash, "destinationBindingHash")
        val proofResult = input.proofResult?.let { requireWrappedProofResultForSubmission(it) }
        if (proofResult != null) {
            require(proofResult.backend == GROTH16_BN254_PROOF_BACKEND_V1) {
                "proofResult.backend must be evm-groth16-bn254-v1"
            }
            require(proofResult.publicInputs == publicInputs) {
                "publicInputs must match proofResult.publicInputs"
            }
            require(proofResult.proofBytes.contentEquals(proofBytes)) {
                "proofBytes must match proofResult.proofBytes"
            }
            require(proofResult.statementHash == statementHash) {
                "statementHash must match proofResult.statementHash"
            }
            require(proofResult.destinationBindingHash == destinationBindingHash) {
                "destinationBindingHash must match proofResult.destinationBindingHash"
            }
        }
        val publicSignalWords = groth16Bn254PublicSignalWords(
            publicInputs = publicInputs,
            sourceDomain = input.sourceDomain,
            statementHash = statementHash,
            destinationBindingHash = destinationBindingHash,
        )
        val suppliedPublicSignalWords = input.publicSignalWords ?: proofResult?.publicSignalWords
        if (suppliedPublicSignalWords != null) {
            require(suppliedPublicSignalWords.size == 9) { "publicSignalWords must contain 9 words" }
            val normalized = suppliedPublicSignalWords.mapIndexed { index, word ->
                normalizeHex32(word, "publicSignalWords[$index]")
            }
            require(normalized == publicSignalWords) {
                "publicSignalWords must match publicInputs and proof context"
            }
        }
        val publicInputWords = messageTransparentPublicInputAbiWords(publicInputs)
        val publicInputWordsBytes = messageTransparentPublicInputAbiWordBytes(publicInputs).fold(
            ByteArrayOutputStream(),
        ) { out, word ->
            out.write(word)
            out
        }.toByteArray()
        val callData = submitMessageProofCallData(
            proofBytes,
            publicInputs,
            statementHash,
            input.sourceDomain,
        )
        val arguments = listOf(
            EvmSccpSubmissionArgument("proof_bytes", "raw_bytes", "0x" + hexLower(proofBytes)),
            EvmSccpSubmissionArgument(
                "public_inputs",
                "abi_bytes32x6",
                "0x" + hexLower(publicInputWordsBytes),
            ),
            EvmSccpSubmissionArgument("statement_hash", "abi_bytes32", statementHash),
        )
        return EvmSccpSubmission(
            version = 1,
            proofFamily = STARK_FRI_PROOF_FAMILY_V1,
            verifierBackend = GROTH16_BN254_PROOF_BACKEND_V1,
            platformPayload = "evm_groth16_contract_call",
            envelopeEncoding = CONTRACT_CALL_ABI_TUPLE_V1,
            submissionKind = "contract_call",
            verifierEntrypoint = SUBMIT_MESSAGE_PROOF_ENTRYPOINT_V1,
            contractMethod = SUBMIT_MESSAGE_PROOF_ABI_V1,
            functionSelector = SUBMIT_MESSAGE_PROOF_SELECTOR_V1,
            sourceDomain = input.sourceDomain,
            targetDomain = publicInputs.targetDomain,
            publicInputs = publicInputs,
            publicInputWords = publicInputWords,
            publicSignalWords = publicSignalWords,
            statementHash = statementHash,
            destinationBindingHash = destinationBindingHash,
            arguments = arguments,
            callDataHex = "0x" + hexLower(callData),
            envelopeHex = "0x" + hexLower(callData),
            proofBytes = proofBytes,
            publicInputWordsBytes = publicInputWordsBytes,
            callData = callData,
        )
    }

    private fun requireNonZeroProofBytes(proofBytes: ByteArray) {
        require(proofBytes.isNotEmpty()) { "proofBytes must not be empty" }
        require(proofBytes.any { it.toInt() != 0 }) { "proofBytes must not be all zero" }
        require(proofBytes.size == GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1) {
            "proofBytes must be $GROTH16_BN254_PROOF_ABI_BYTE_LENGTH_V1 bytes"
        }
        requireGroth16ProofTuple(proofBytes, "proofBytes")
    }

    private fun requireProofBytesForPublicInputs(
        proofBytes: ByteArray,
        publicInputs: EvmSccpPublicInputsInput,
    ) {
        requireNonZeroProofBytes(proofBytes)
        require(
            proofWord(proofBytes, 1)
                .contentEquals(nonZeroHex32Bytes(publicInputs.messageId, "publicInputs.messageId")),
        ) {
            "proofBytes.messageId must match publicInputs.messageId"
        }
        require(
            proofWord(proofBytes, 3)
                .contentEquals(nonZeroHex32Bytes(publicInputs.commitmentRoot, "publicInputs.commitmentRoot")),
        ) {
            "proofBytes.commitmentRoot must match publicInputs.commitmentRoot"
        }
    }

    private fun requireProofBytesForContext(
        proofBytes: ByteArray,
        publicInputs: EvmSccpPublicInputsInput,
        sourceDomain: Int,
    ) {
        requireProofBytesForPublicInputs(proofBytes, publicInputs)
        require(proofWordValue(proofBytes, 2) == BigInteger.valueOf(sourceDomain.toLong())) {
            "proofBytes.sourceDomain must match sourceDomain"
        }
    }

    private fun proofWord(proofBytes: ByteArray, index: Int): ByteArray =
        proofBytes.copyOfRange(index * 32, (index + 1) * 32)

    private fun proofWordValue(proofBytes: ByteArray, index: Int): BigInteger =
        BigInteger(1, proofWord(proofBytes, index))

    private fun proofWordIsZero(proofBytes: ByteArray, index: Int): Boolean =
        proofWord(proofBytes, index).all { it.toInt() == 0 }

    private fun requireBaseFieldWord(proofBytes: ByteArray, index: Int, label: String) {
        require(proofWordValue(proofBytes, index) < BN254_BASE_FIELD_MODULUS) {
            "$label must be a BN254 base-field element"
        }
    }

    private fun requireNonZeroPoint(proofBytes: ByteArray, indexes: List<Int>, label: String) {
        require(indexes.any { !proofWordIsZero(proofBytes, it) }) {
            "$label must not be zero"
        }
    }

    private data class G2ProjectivePoint(
        val x: Pair<BigInteger, BigInteger>,
        val y: Pair<BigInteger, BigInteger>,
        val z: Pair<BigInteger, BigInteger>,
        val infinity: Boolean,
    )

    private fun fq(value: BigInteger): BigInteger = value.mod(BN254_BASE_FIELD_MODULUS)

    private fun fq2Add(
        left: Pair<BigInteger, BigInteger>,
        right: Pair<BigInteger, BigInteger>,
    ): Pair<BigInteger, BigInteger> =
        Pair(fq(left.first + right.first), fq(left.second + right.second))

    private fun fq2Sub(
        left: Pair<BigInteger, BigInteger>,
        right: Pair<BigInteger, BigInteger>,
    ): Pair<BigInteger, BigInteger> =
        Pair(fq(left.first - right.first), fq(left.second - right.second))

    private fun fq2Scale(
        left: Pair<BigInteger, BigInteger>,
        scalar: Long,
    ): Pair<BigInteger, BigInteger> =
        Pair(fq(left.first * BigInteger.valueOf(scalar)), fq(left.second * BigInteger.valueOf(scalar)))

    private fun fq2Mul(
        left: Pair<BigInteger, BigInteger>,
        right: Pair<BigInteger, BigInteger>,
    ): Pair<BigInteger, BigInteger> =
        Pair(
            fq(left.first * right.first - left.second * right.second),
            fq(left.first * right.second + left.second * right.first),
        )

    private fun fq2IsZero(value: Pair<BigInteger, BigInteger>): Boolean =
        value.first == BigInteger.ZERO && value.second == BigInteger.ZERO

    private fun g2Infinity(): G2ProjectivePoint =
        G2ProjectivePoint(
            x = Pair(BigInteger.ZERO, BigInteger.ZERO),
            y = Pair(BigInteger.ONE, BigInteger.ZERO),
            z = Pair(BigInteger.ZERO, BigInteger.ZERO),
            infinity = true,
        )

    private fun g2AffineProjective(
        x: Pair<BigInteger, BigInteger>,
        y: Pair<BigInteger, BigInteger>,
    ): G2ProjectivePoint =
        G2ProjectivePoint(x = x, y = y, z = Pair(BigInteger.ONE, BigInteger.ZERO), infinity = false)

    private fun g2ProjectiveIsInfinity(point: G2ProjectivePoint): Boolean =
        point.infinity || fq2IsZero(point.z)

    private fun g2ProjectiveDouble(point: G2ProjectivePoint): G2ProjectivePoint {
        if (g2ProjectiveIsInfinity(point) || fq2IsZero(point.y)) return g2Infinity()
        val xx = fq2Mul(point.x, point.x)
        val yy = fq2Mul(point.y, point.y)
        val yyyy = fq2Mul(yy, yy)
        val s = fq2Scale(
            fq2Sub(
                fq2Sub(fq2Mul(fq2Add(point.x, yy), fq2Add(point.x, yy)), xx),
                yyyy,
            ),
            2,
        )
        val m = fq2Scale(xx, 3)
        val x3 = fq2Sub(fq2Mul(m, m), fq2Scale(s, 2))
        val y3 = fq2Sub(fq2Mul(m, fq2Sub(s, x3)), fq2Scale(yyyy, 8))
        val z3 = fq2Scale(fq2Mul(point.y, point.z), 2)
        return G2ProjectivePoint(x = x3, y = y3, z = z3, infinity = false)
    }

    private fun g2ProjectiveAddAffine(
        point: G2ProjectivePoint,
        affineX: Pair<BigInteger, BigInteger>,
        affineY: Pair<BigInteger, BigInteger>,
    ): G2ProjectivePoint {
        if (g2ProjectiveIsInfinity(point)) return g2AffineProjective(affineX, affineY)
        val z1z1 = fq2Mul(point.z, point.z)
        val u2 = fq2Mul(affineX, z1z1)
        val s2 = fq2Mul(affineY, fq2Mul(point.z, z1z1))
        val h = fq2Sub(u2, point.x)
        if (fq2IsZero(h)) {
            return if (s2 == point.y) g2ProjectiveDouble(point) else g2Infinity()
        }
        val hh = fq2Mul(h, h)
        val i = fq2Scale(hh, 4)
        val j = fq2Mul(h, i)
        val r = fq2Scale(fq2Sub(s2, point.y), 2)
        val v = fq2Mul(point.x, i)
        val x3 = fq2Sub(fq2Sub(fq2Mul(r, r), j), fq2Scale(v, 2))
        val y3 = fq2Sub(fq2Mul(r, fq2Sub(v, x3)), fq2Scale(fq2Mul(point.y, j), 2))
        val z3 = fq2Sub(fq2Sub(fq2Mul(fq2Add(point.z, h), fq2Add(point.z, h)), z1z1), hh)
        return G2ProjectivePoint(x = x3, y = y3, z = z3, infinity = false)
    }

    private fun g2PointIsInPrimeSubgroup(
        x: Pair<BigInteger, BigInteger>,
        y: Pair<BigInteger, BigInteger>,
    ): Boolean {
        var acc = g2Infinity()
        for (index in BN254_SCALAR_FIELD_MODULUS.bitLength() - 1 downTo 0) {
            acc = g2ProjectiveDouble(acc)
            if (BN254_SCALAR_FIELD_MODULUS.testBit(index)) {
                acc = g2ProjectiveAddAffine(acc, x, y)
            }
        }
        return g2ProjectiveIsInfinity(acc)
    }

    private fun requireG1Point(proofBytes: ByteArray, xIndex: Int, yIndex: Int, label: String) {
        requireNonZeroPoint(proofBytes, listOf(xIndex, yIndex), label)
        val x = proofWordValue(proofBytes, xIndex)
        val y = proofWordValue(proofBytes, yIndex)
        val left = fq(y * y)
        val right = fq(x * x * x + BigInteger.valueOf(3))
        require(left == right) { "$label must be a BN254 G1 point" }
    }

    private fun requireG2Point(
        proofBytes: ByteArray,
        x0Index: Int,
        x1Index: Int,
        y0Index: Int,
        y1Index: Int,
        label: String,
    ) {
        requireNonZeroPoint(proofBytes, listOf(x0Index, x1Index, y0Index, y1Index), label)
        val x = Pair(proofWordValue(proofBytes, x0Index), proofWordValue(proofBytes, x1Index))
        val y = Pair(proofWordValue(proofBytes, y0Index), proofWordValue(proofBytes, y1Index))
        val left = fq2Mul(y, y)
        val x2 = fq2Mul(x, x)
        val x3 = fq2Mul(x2, x)
        val right = Pair(fq(x3.first + BN254_G2_B_C0), fq(x3.second + BN254_G2_B_C1))
        require(left == right && g2PointIsInPrimeSubgroup(x, y)) { "$label must be a BN254 G2 point" }
    }

    private fun requireGroth16ProofTuple(proofBytes: ByteArray, label: String) {
        require(proofWordValue(proofBytes, 0) == BigInteger.ONE) { "$label.version must be 1" }
        require(!proofWordIsZero(proofBytes, 1)) { "$label.messageId must not be zero" }
        require(proofWordValue(proofBytes, 2) <= BigInteger.valueOf(0xffff_ffffL)) {
            "$label.sourceDomain must fit u32"
        }
        require(!proofWordIsZero(proofBytes, 3)) { "$label.commitmentRoot must not be zero" }
        listOf("a.x", "a.y", "b.x0", "b.x1", "b.y0", "b.y1", "c.x", "c.y")
            .forEachIndexed { offset, field ->
                requireBaseFieldWord(proofBytes, 4 + offset, "$label.$field")
            }
        requireG1Point(proofBytes, 4, 5, "$label.a")
        requireG2Point(proofBytes, 6, 7, 8, 9, "$label.b")
        requireG1Point(proofBytes, 10, 11, "$label.c")
    }

    private fun requireCanonicalProofRequest(request: EvmSccpProofRequest) {
        val expected = buildProofRequest(
            EvmSccpProofRequestInput(
                publicInputs = request.publicInputs,
                bundleBytes = request.bundleBytes,
                sourceProofBytes = request.sourceProofBytes,
                statementHash = request.statementHash,
                destinationBindingHash = request.destinationBindingHash,
                backend = request.backend,
                sourceDomain = request.sourceDomain,
                destinationBinding = request.destinationBinding,
                proofArtifactHash = request.proofArtifactHash,
                provingKeyHash = request.provingKeyHash,
            ),
        )
        require(
            request.version == expected.version &&
                request.backend == expected.backend &&
                request.sourceDomain == expected.sourceDomain &&
                request.targetDomain == expected.targetDomain &&
                request.publicInputs == expected.publicInputs &&
                request.publicInputsBytes.contentEquals(expected.publicInputsBytes) &&
                request.publicSignalWords == expected.publicSignalWords &&
                request.bundleBytes.contentEquals(expected.bundleBytes) &&
                request.sourceProofBytes.contentEquals(expected.sourceProofBytes) &&
                request.proofContext == expected.proofContext &&
                request.statementHash == expected.statementHash &&
                request.destinationBindingHash == expected.destinationBindingHash &&
                request.proofArtifactHash == expected.proofArtifactHash &&
                request.provingKeyHash == expected.provingKeyHash &&
                request.requestHash == expected.requestHash &&
                request.destinationBinding == expected.destinationBinding,
        ) { "proof request must be canonical" }
    }

    internal fun requireProductionProofRequest(request: EvmSccpProofRequest) {
        requireCanonicalProofRequest(request)
        require(request.version == 1) { "proof request version must be 1" }
        require(request.backend == GROTH16_BN254_PROOF_BACKEND_V1) {
            "EVM-family SCCP proof request backend must be evm-groth16-bn254-v1"
        }
        require(request.sourceDomain == DOMAIN_SORA) {
            "EVM-family SCCP production proofs must start from SORA"
        }
        require(
            request.targetDomain == request.publicInputs.targetDomain &&
                (request.targetDomain == DOMAIN_ETH || request.targetDomain == DOMAIN_BSC),
        ) {
            "EVM-family SCCP production proofs must target ETH or BSC"
        }
        require(request.bundleBytes.isNotEmpty()) {
            "EVM-family SCCP proof request bundleBytes must not be empty"
        }
        requireOptionalSourceProofBytes(
            request.sourceProofBytes,
            "EVM-family SCCP proof request sourceProofBytes",
        )
        requireProductionDestinationBinding(request)
    }

    private fun requireOptionalSourceProofBytes(bytes: ByteArray, label: String): ByteArray {
        val copy = bytes.copyOf()
        require(copy.size <= SOURCE_STATE_MAX_PROOF_BYTES) {
            "$label must be at most $SOURCE_STATE_MAX_PROOF_BYTES bytes"
        }
        require(copy.isEmpty() || copy.any { it.toInt() != 0 }) { "$label must not be all zero" }
        return copy
    }

    private fun requireProductionDestinationBinding(request: EvmSccpProofRequest) {
        val destinationBinding = request.destinationBinding
        require(destinationBinding != null) {
            "EVM-family SCCP production proof request destinationBinding must include deployment material"
        }
        val destinationBindingHash = requireDestinationBindingHashForProofRequest(
            publicInputs = request.publicInputs,
            destinationBinding = destinationBinding,
            backend = request.backend,
            sourceDomain = request.sourceDomain,
        )
        require(request.destinationBindingHash == destinationBindingHash) {
            "destinationBindingHash must match destinationBinding deployment material"
        }
        require(request.proofContext.destinationBindingHash == destinationBindingHash) {
            "proofContext.destinationBindingHash must match destinationBinding deployment material"
        }
    }

    internal fun requireDestinationBindingHashForProofRequest(
        publicInputs: EvmSccpPublicInputsInput,
        destinationBinding: SccpSourceProofs.EvmDestinationBinding,
        backend: String,
        sourceDomain: Int,
    ): String {
        require(destinationBinding.version == 1) {
            "destinationBinding.version must be 1"
        }
        require(destinationBinding.sourceDomain == sourceDomain) {
            "destinationBinding.sourceDomain must match sourceDomain"
        }
        require(destinationBinding.targetDomain == publicInputs.targetDomain) {
            "destinationBinding.targetDomain must match publicInputs.targetDomain"
        }
        require(destinationBinding.verifierBackend == backend) {
            "destinationBinding.verifierBackend must match backend"
        }
        require(destinationBinding.proofFamily == STARK_FRI_PROOF_FAMILY_V1) {
            "destinationBinding.proofFamily must be stark-fri-v1"
        }
        val expectedDestinationBinding = SccpSourceProofs.evmDestinationBinding(
            sourceDomain = sourceDomain,
            targetDomain = publicInputs.targetDomain,
            networkId = destinationBinding.networkId,
            verifierAddress = destinationBinding.verifierAddress,
            bridgeAddress = destinationBinding.bridgeAddress,
            verifierCodeHash = destinationBinding.verifierCodeHash,
            verifierKeyHash = destinationBinding.verifierKeyHash,
            verifierBackend = backend,
            proofFamily = STARK_FRI_PROOF_FAMILY_V1,
        )
        require(destinationBinding == expectedDestinationBinding) {
            "destinationBinding must match deployment material"
        }
        return expectedDestinationBinding.hash
    }

    internal fun callbackRequestSnapshot(request: EvmSccpProofRequest): EvmSccpProofRequest =
        request.copy()

    private data class Groth16ProverArtifacts(
        val proofArtifactHash: String,
        val provingKeyHash: String,
    )

    private fun hashHex(prefix: String, payload: ByteArray): String {
        val prefixBytes = prefix.toByteArray(Charsets.UTF_8)
        val preimage = ByteArray(prefixBytes.size + payload.size)
        System.arraycopy(prefixBytes, 0, preimage, 0, prefixBytes.size)
        System.arraycopy(payload, 0, preimage, prefixBytes.size, payload.size)
        return "0x" + hexLower(Blake2b.digest256(preimage))
    }

    private fun sha256Hex(payload: ByteArray): String =
        "0x" + hexLower(MessageDigest.getInstance("SHA-256").digest(payload))

    private val nativeEvmProverForbiddenArtifactMarkers: List<ByteArray> =
        listOf(
            byteArrayOf(0x77, 0x65, 0x62, 0x61, 0x73, 0x73, 0x65, 0x6d, 0x62, 0x6c, 0x79),
            byteArrayOf(0x77, 0x61, 0x73, 0x6d),
            byteArrayOf(0x73, 0x6e, 0x61, 0x72, 0x6b, 0x6a, 0x73),
            byteArrayOf(0x72, 0x65, 0x6d, 0x6f, 0x74, 0x65, 0x70, 0x72, 0x6f, 0x76, 0x65, 0x72),
            byteArrayOf(0x72, 0x65, 0x6d, 0x6f, 0x74, 0x65, 0x20, 0x70, 0x72, 0x6f, 0x76, 0x65, 0x72),
            byteArrayOf(0x72, 0x65, 0x6d, 0x6f, 0x74, 0x65, 0x5f, 0x70, 0x72, 0x6f, 0x76, 0x65, 0x72),
            byteArrayOf(0x70, 0x72, 0x6f, 0x76, 0x65, 0x72, 0x5f, 0x75, 0x72, 0x6c),
            byteArrayOf(0x70, 0x72, 0x6f, 0x76, 0x65, 0x72, 0x2d, 0x75, 0x72, 0x6c),
            byteArrayOf(0x70, 0x72, 0x6f, 0x76, 0x65, 0x72, 0x65, 0x6e, 0x64, 0x70, 0x6f, 0x69, 0x6e, 0x74),
            byteArrayOf(0x70, 0x72, 0x6f, 0x76, 0x65, 0x72, 0x20, 0x65, 0x6e, 0x64, 0x70, 0x6f, 0x69, 0x6e, 0x74),
        )

    private fun lowerAsciiByte(value: Byte): Int {
        val unsigned = value.toInt() and 0xff
        return if (unsigned in 0x41..0x5a) unsigned + 0x20 else unsigned
    }

    private fun containsNativeEvmProverMarker(bytes: ByteArray, marker: ByteArray): Boolean {
        if (marker.size > bytes.size) return false
        for (offset in 0..(bytes.size - marker.size)) {
            var matched = true
            for (index in marker.indices) {
                if (lowerAsciiByte(bytes[offset + index]) != (marker[index].toInt() and 0xff)) {
                    matched = false
                    break
                }
            }
            if (matched) return true
        }
        return false
    }

    private fun rejectNativeEvmProverForbiddenArtifactMarkers(bytes: ByteArray, field: String) {
        nativeEvmProverForbiddenArtifactMarkers.firstOrNull { marker ->
            containsNativeEvmProverMarker(bytes, marker)
        }?.let {
            throw IllegalArgumentException("$field contains forbidden prover dependency marker")
        }
    }

    private fun requireNativeEvmProverProductionArtifactSize(bytes: ByteArray, field: String) {
        require(bytes.size >= NATIVE_EVM_PROVER_MIN_ARTIFACT_BYTES_V1) {
            "$field must be at least $NATIVE_EVM_PROVER_MIN_ARTIFACT_BYTES_V1 bytes"
        }
    }

    private fun hex32Bytes(value: String, field: String): ByteArray {
        require(value.trim() == value) { "$field must be canonical hex" }
        var body = value
        if (body.startsWith("0x", ignoreCase = true)) body = body.substring(2)
        require(body.length == 64) { "$field must be 32 bytes" }
        val out = ByteArray(32)
        for (i in out.indices) {
            out[i] = body.substring(i * 2, i * 2 + 2).toIntOrNull(16)?.toByte()
                ?: throw IllegalArgumentException("$field must be canonical hex")
        }
        return out
    }

    private fun nonZeroHex32Bytes(value: String, field: String): ByteArray {
        val out = hex32Bytes(value, field)
        require(out.any { it.toInt() != 0 }) { "$field must not be zero" }
        return out
    }

    private fun normalizeNonZeroHex32(value: String, field: String): String =
        "0x" + hexLower(nonZeroHex32Bytes(value, field))

    private fun normalizeNativeEvmProverBundleHex32(value: String, field: String): String {
        val normalized = normalizeNonZeroHex32(value, field)
        require(value == normalized) {
            "$field must be canonical lowercase 0x-prefixed 32-byte hex"
        }
        return normalized
    }

    private fun requireNativeEvmProverBundleHashRoleSeparation(roles: List<Pair<String, String>>) {
        val seen = LinkedHashMap<String, String>()
        roles.forEach { (label, hash) ->
            val previous = seen.putIfAbsent(hash, label)
            require(previous == null) {
                "nativeProverBundle hashes must be role-separated: $label matches $previous"
            }
        }
    }

    private fun normalizeHex32(value: String, field: String): String =
        "0x" + hexLower(hex32Bytes(value, field))

    private fun normalizeNativeEvmProverParityHex32(value: String, field: String): String {
        val normalized = normalizeHex32(value, field)
        require(value == normalized) {
            "$field must be canonical lowercase 0x-prefixed 32-byte hex"
        }
        return normalized
    }

    private fun normalizeNativeEvmProverArtifactPath(value: String, field: String): String {
        require(value.isNotEmpty()) { "$field must be a non-empty relative POSIX path" }
        require(value.none { it.code < 0x20 || it.code == 0x7f }) {
            "$field must not contain control characters"
        }
        require(!value.contains(':')) { "$field must not contain URI schemes or drive prefixes" }
        require(!value.startsWith("/") && !value.contains('\\')) {
            "$field must be a relative POSIX path"
        }
        val normalizedValue = value.lowercase()
        fun forbiddenPathMarker(vararg parts: String): String = parts.joinToString("")
        val forbiddenPathMarkers = listOf(
            forbiddenPathMarker("web", "assem", "bly"),
            forbiddenPathMarker("wa", "sm"),
            forbiddenPathMarker("sn", "ark", "js"),
            forbiddenPathMarker("remote", "pro", "ver"),
            forbiddenPathMarker("remote", "-", "pro", "ver"),
            forbiddenPathMarker("remote", "_", "pro", "ver"),
            forbiddenPathMarker("remote", " ", "pro", "ver"),
            forbiddenPathMarker("pro", "ver", "-", "url"),
            forbiddenPathMarker("pro", "ver", "_", "url"),
            forbiddenPathMarker("pro", "ver", "end", "point"),
            forbiddenPathMarker("pro", "ver", "-", "end", "point"),
            forbiddenPathMarker("pro", "ver", "_", "end", "point"),
            forbiddenPathMarker("pro", "ver", " ", "end", "point"),
        )
        forbiddenPathMarkers.forEach { marker ->
            require(!normalizedValue.contains(marker)) {
                "$field path contains forbidden prover dependency marker: $marker"
            }
        }
        val segments = value.split('/')
        require(segments.isNotEmpty() && segments.all { it.isNotEmpty() && it != "." && it != ".." }) {
            "$field must stay under the manifest directory"
        }
        return value
    }

    private fun normalizeOptionalGroth16ProverArtifacts(
        proofArtifactHash: String?,
        provingKeyHash: String?,
    ): Groth16ProverArtifacts? {
        require((proofArtifactHash == null) == (provingKeyHash == null)) {
            "proofArtifactHash and provingKeyHash must be supplied together"
        }
        if (proofArtifactHash == null || provingKeyHash == null) {
            return null
        }
        return Groth16ProverArtifacts(
            proofArtifactHash = normalizeNonZeroHex32(proofArtifactHash, "proofArtifactHash"),
            provingKeyHash = normalizeNonZeroHex32(provingKeyHash, "provingKeyHash"),
        )
    }

    private fun normalizeProofContext(
        statementHash: String,
        destinationBindingHash: String,
    ): EvmSccpProofContext =
        EvmSccpProofContext(
            version = 1,
            statementHash = normalizeNonZeroHex32(statementHash, "statementHash"),
            destinationBindingHash = normalizeNonZeroHex32(destinationBindingHash, "destinationBindingHash"),
        )

    private fun groth16SignalWord(label: String, value: ByteArray): String {
        val labelHash = keccak256(label.toByteArray(Charsets.UTF_8))
        val payload = ByteArray(labelHash.size + value.size)
        System.arraycopy(labelHash, 0, payload, 0, labelHash.size)
        System.arraycopy(value, 0, payload, labelHash.size, value.size)
        val reduced = BigInteger(1, keccak256(payload)).mod(BN254_SCALAR_FIELD_MODULUS)
        return "0x" + hexLower(toFixedBytes(reduced, 32))
    }

    private fun keccak256(input: ByteArray): ByteArray {
        val digest = KeccakDigest(256)
        digest.update(input, 0, input.size)
        val out = ByteArray(32)
        digest.doFinal(out, 0)
        return out
    }

    private fun abiWordU32(value: Int): ByteArray {
        require(value >= 0) { "domain id must not be negative" }
        val out = ByteArray(32)
        out[28] = ((value ushr 24) and 0xff).toByte()
        out[29] = ((value ushr 16) and 0xff).toByte()
        out[30] = ((value ushr 8) and 0xff).toByte()
        out[31] = (value and 0xff).toByte()
        return out
    }

    private fun abiWordU64(value: BigInteger): ByteArray {
        val out = ByteArray(32)
        var working = value
        for (index in 31 downTo 24) {
            out[index] = working.and(BigInteger.valueOf(0xffL)).toByte()
            working = working.shiftRight(8)
        }
        return out
    }

    private fun abiWordU256(value: Int): ByteArray {
        require(value >= 0) { "u256 value must not be negative" }
        val out = ByteArray(32)
        var working = value
        for (index in 31 downTo 0) {
            out[index] = (working and 0xff).toByte()
            working = working ushr 8
            if (working == 0) break
        }
        return out
    }

    private fun messageTransparentPublicInputAbiWordBytes(
        publicInputs: EvmSccpPublicInputsInput,
    ): List<ByteArray> {
        canonicalPublicInputsBytes(publicInputs)
        return listOf(
            nonZeroHex32Bytes(publicInputs.messageId, "messageId"),
            nonZeroHex32Bytes(publicInputs.payloadHash, "payloadHash"),
            abiWordU32(publicInputs.targetDomain),
            nonZeroHex32Bytes(publicInputs.commitmentRoot, "commitmentRoot"),
            abiWordU64(normalizeU64(publicInputs.finalityHeight, "finalityHeight")),
            nonZeroHex32Bytes(publicInputs.finalityBlockHash, "finalityBlockHash"),
        )
    }

    private fun normalizeU64(value: String, field: String): BigInteger {
        require(isCanonicalDecimalText(value)) { "$field must be an unsigned integer" }
        val numeric = BigInteger(value)
        require(numeric <= MAX_U64) { "$field must fit u64" }
        require(numeric != BigInteger.ZERO) { "$field must not be zero" }
        return numeric
    }

    private fun isCanonicalDecimalText(value: String): Boolean =
        value == "0" || (value.isNotEmpty() && value[0] in '1'..'9' && value.all { it in '0'..'9' })

    private fun writeU32Le(out: ByteArrayOutputStream, value: Int) {
        require(value >= 0) { "u32 value must not be negative" }
        out.write(value and 0xff)
        out.write((value ushr 8) and 0xff)
        out.write((value ushr 16) and 0xff)
        out.write((value ushr 24) and 0xff)
    }

    private fun writeU64Le(out: ByteArrayOutputStream, value: BigInteger) {
        var working = value
        for (i in 0 until 8) {
            out.write(working.and(BigInteger.valueOf(0xffL)).toInt())
            working = working.shiftRight(8)
        }
    }

    private fun toFixedBytes(value: BigInteger, length: Int): ByteArray {
        val raw = value.toByteArray()
        val out = ByteArray(length)
        val copyLength = Math.min(raw.size, length)
        System.arraycopy(raw, raw.size - copyLength, out, length - copyLength, copyLength)
        return out
    }

    private fun hexLower(bytes: ByteArray): String {
        val builder = StringBuilder(bytes.size * 2)
        for (byte in bytes) builder.append(String.format("%02x", byte.toInt() and 0xff))
        return builder.toString()
    }
}

private const val ETHEREUM_MAINNET_BEACON_REST_MAX_RESPONSE_BYTES: Int = 1024 * 1024
private const val ETHEREUM_MAINNET_SECONDS_PER_SLOT: Long = 12L

private data class EthereumBeaconRestHeaderSummary(
    val root: String,
    val slot: BigInteger,
)

private data class EthereumBeaconRestBlockId(
    val id: String,
    val slot: BigInteger? = null,
    val root: String? = null,
)

private data class EthereumBeaconRestFinalityUpdateSummary(
    val finalizedHeaderRoot: String,
    val beaconSlot: BigInteger,
    val finalityBranch: List<String>,
    val syncCommitteeBits: String,
    val syncCommitteeSignature: String,
    val syncCommitteeParticipation: BigInteger,
    val syncSignatureSlot: BigInteger,
)

/** Ethereum mainnet SCCP Groth16 helpers with chain-id and domain checks baked in. */
object SccpEthereumMainnet {
    const val DOMAIN_SORA: Int = SccpEvm.DOMAIN_SORA
    const val DOMAIN_ETH: Int = SccpEvm.DOMAIN_ETH
    const val MAINNET_CHAIN_ID: Long = SccpSourceProofs.ETH_MAINNET_CHAIN_ID
    const val MAINNET_NETWORK_ID: String = SccpSourceProofs.ETH_MAINNET_NETWORK_ID
    const val LOCAL_ADMISSION_ENVELOPE_ENCODING_V1: String = "norito:sccp-local-admission:v1"
    const val LOCAL_ADMISSION_SUBMISSION_KIND_V1: String = "local_admission"
    const val LOCAL_ADMISSION_ENTRYPOINT_V1: String = "SubmitBridgeProof"
    const val STARK_FRI_PROOF_FAMILY_V1: String = "stark-fri-v1"
    const val SOURCE_EVENT_ABI_V1: String = "SccpSourceEvent(bytes32)"
    const val SOURCE_EVENT_TOPIC_V1: String =
        "0x577b41c65ffbce226de59f224b464797257063747891b88ebec1bcd57af82727"

    @JvmStatic
    fun requireMainnetChainId(chainId: Long) {
        require(chainId == MAINNET_CHAIN_ID) {
            "Ethereum mainnet SCCP requires eth_chainId == 1"
        }
    }

    @JvmStatic
    fun sourceEventTopic(): String = SOURCE_EVENT_TOPIC_V1

    @JvmStatic
    @JvmOverloads
    fun destinationBinding(
        verifierAddress: String,
        bridgeAddress: String,
        verifierCodeHash: String,
        verifierKeyHash: String,
        networkId: String = MAINNET_NETWORK_ID,
    ): SccpSourceProofs.EvmDestinationBinding =
        SccpSourceProofs.ethereumMainnetDestinationBinding(
            verifierAddress = verifierAddress,
            bridgeAddress = bridgeAddress,
            verifierCodeHash = verifierCodeHash,
            verifierKeyHash = verifierKeyHash,
            networkId = networkId,
        )

    @JvmStatic
    @JvmOverloads
    fun destinationBindingHash(
        verifierAddress: String,
        bridgeAddress: String,
        verifierCodeHash: String,
        verifierKeyHash: String,
        networkId: String = MAINNET_NETWORK_ID,
    ): String = destinationBinding(
        verifierAddress = verifierAddress,
        bridgeAddress = bridgeAddress,
        verifierCodeHash = verifierCodeHash,
        verifierKeyHash = verifierKeyHash,
        networkId = networkId,
    ).hash

    @JvmStatic
    fun buildProofRequest(
        input: EvmSccpProofRequestInput,
        nativeProverBundle: SccpEvm.EthereumMainnetNativeEvmProverBundle,
    ): EvmSccpProofRequest = buildProofRequest(nativeProverBundle.applyTo(input))

    @JvmStatic
    fun buildProofRequest(input: EvmSccpProofRequestInput): EvmSccpProofRequest {
        require(input.sourceDomain == DOMAIN_SORA) {
            "Ethereum mainnet proof requests must route SORA -> ETH"
        }
        require(input.publicInputs.targetDomain == DOMAIN_ETH) {
            "Ethereum mainnet proof requests must target ETH"
        }
        val destinationBinding = input.destinationBinding
            ?: throw IllegalArgumentException("Ethereum mainnet proof requests require destinationBinding")
        require(destinationBinding.sourceDomain == DOMAIN_SORA) {
            "Ethereum mainnet destinationBinding must start from SORA"
        }
        require(destinationBinding.targetDomain == DOMAIN_ETH) {
            "Ethereum mainnet destinationBinding must target ETH"
        }
        require(destinationBinding.networkId == MAINNET_NETWORK_ID) {
            "Ethereum mainnet destinationBinding.networkId must be chain id 1"
        }
        val destinationBindingHash = SccpEvm.requireDestinationBindingHashForProofRequest(
            publicInputs = input.publicInputs,
            destinationBinding = destinationBinding,
            backend = input.backend,
            sourceDomain = input.sourceDomain,
        )
        require(input.destinationBindingHash == destinationBindingHash) {
            "destinationBindingHash must match Ethereum mainnet destinationBinding"
        }
        return SccpEvm.buildProofRequest(input)
    }

    @JvmStatic
    fun wrapProofResult(proofBytes: ByteArray, request: EvmSccpProofRequest): EvmSccpProofResult {
        require(request.sourceDomain == DOMAIN_SORA) {
            "Ethereum mainnet proof results must route SORA -> ETH"
        }
        require(request.targetDomain == DOMAIN_ETH && request.publicInputs.targetDomain == DOMAIN_ETH) {
            "Ethereum mainnet proof results must target ETH"
        }
        require(request.destinationBinding?.sourceDomain == DOMAIN_SORA) {
            "Ethereum mainnet proof results require SORA source destinationBinding"
        }
        require(request.destinationBinding.networkId == MAINNET_NETWORK_ID) {
            "Ethereum mainnet proof results require chain id 1 destinationBinding"
        }
        require(request.destinationBinding.hash == request.destinationBindingHash) {
            "destinationBindingHash must match Ethereum mainnet destinationBinding"
        }
        return SccpEvm.wrapProofResult(proofBytes, request)
    }

    @JvmStatic
    fun buildSubmission(input: EvmSccpSubmissionInput): EvmSccpSubmission {
        require(input.publicInputs.targetDomain == DOMAIN_ETH) {
            "Ethereum mainnet submissions must target ETH"
        }
        val proofResult = input.proofResult
            ?: throw IllegalArgumentException(
                "Ethereum mainnet submissions require a wrapped proofResult with destinationBinding",
            )
        require(proofResult.publicInputs.targetDomain == DOMAIN_ETH) {
            "Ethereum mainnet proofResult must target ETH"
        }
        val destinationBinding = proofResult.destinationBinding
            ?: throw IllegalArgumentException("Ethereum mainnet proofResult requires destinationBinding")
        require(destinationBinding.networkId == MAINNET_NETWORK_ID) {
            "Ethereum mainnet proofResult requires chain id 1 destinationBinding"
        }
        require(destinationBinding.hash == proofResult.destinationBindingHash) {
            "Ethereum mainnet proofResult destinationBindingHash must match destinationBinding"
        }
        return SccpEvm.buildSubmission(input)
    }

    @JvmStatic
    fun buildLocalAdmissionSubmission(
        input: EthereumMainnetLocalAdmissionSubmissionInput,
    ): EthereumMainnetLocalAdmissionSubmission {
        require(input.sourceDomain == DOMAIN_ETH && input.targetDomain == DOMAIN_SORA) {
            "Ethereum mainnet local-admission submissions must route ETH -> SORA"
        }
        require(input.envelopeEncoding == LOCAL_ADMISSION_ENVELOPE_ENCODING_V1) {
            "Ethereum mainnet local-admission envelopeEncoding is not canonical"
        }
        require(input.submissionKind == LOCAL_ADMISSION_SUBMISSION_KIND_V1) {
            "Ethereum mainnet local-admission submissionKind is not canonical"
        }
        require(input.verifierEntrypoint == LOCAL_ADMISSION_ENTRYPOINT_V1) {
            "Ethereum mainnet local-admission verifierEntrypoint is not canonical"
        }
        require(input.proofFamily == STARK_FRI_PROOF_FAMILY_V1) {
            "Ethereum mainnet local-admission proofFamily is not canonical"
        }
        require(input.verifierBackend == SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1) {
            "Ethereum mainnet local-admission verifierBackend is not canonical"
        }
        val proofBytes = requireNativeRecursiveBytes(input.proofBytes, "proofBytes")
        val publicInputsBytes = requireNativeRecursiveBytes(
            input.publicInputsBytes,
            "publicInputsBytes",
        )
        val bundleBytes = requireNativeRecursiveBytes(input.bundleBytes, "bundleBytes")
        val envelopeBytes = requireNativeRecursiveBytes(input.envelopeBytes, "envelopeBytes")
        val statementHash = normalizeNonZeroHex32(input.statementHash, "statementHash")
        val sourceVerifierMaterialHash = normalizeNonZeroHex32(
            input.sourceVerifierMaterialHash,
            "sourceVerifierMaterialHash",
        )
        val sourceAdapterEngineDeploymentHash = normalizeNonZeroHex32(
            input.sourceAdapterEngineDeploymentHash,
            "sourceAdapterEngineDeploymentHash",
        )
        val localAdmission = EthereumMainnetLocalAdmissionPayload(
            proofBytes = proofBytes,
            publicInputsBytes = publicInputsBytes,
            bundleBytes = bundleBytes,
            statementHash = statementHash,
            sourceVerifierMaterialHash = sourceVerifierMaterialHash,
            sourceAdapterEngineDeploymentHash = sourceAdapterEngineDeploymentHash,
        )
        return EthereumMainnetLocalAdmissionSubmission(
            version = 1,
            proofFamily = input.proofFamily,
            verifierBackend = input.verifierBackend,
            platformPayload = LOCAL_ADMISSION_SUBMISSION_KIND_V1,
            envelopeEncoding = LOCAL_ADMISSION_ENVELOPE_ENCODING_V1,
            submissionKind = LOCAL_ADMISSION_SUBMISSION_KIND_V1,
            verifierEntrypoint = LOCAL_ADMISSION_ENTRYPOINT_V1,
            sourceDomain = DOMAIN_ETH,
            targetDomain = DOMAIN_SORA,
            statementHash = statementHash,
            sourceVerifierMaterialHash = sourceVerifierMaterialHash,
            sourceAdapterEngineDeploymentHash = sourceAdapterEngineDeploymentHash,
            localAdmission = localAdmission,
            proofBytes = proofBytes,
            publicInputsBytes = publicInputsBytes,
            bundleBytes = bundleBytes,
            envelopeBytes = envelopeBytes,
        )
    }

    private fun requireNativeRecursiveBytes(bytes: ByteArray, label: String): ByteArray {
        val copy = bytes.copyOf()
        require(copy.isNotEmpty()) { "$label must not be empty" }
        require(copy.any { it.toInt() != 0 }) { "$label must not be all zero" }
        require(copy.size <= SccpEvm.NATIVE_RECURSIVE_MAX_PROOF_BYTES) {
            "$label must be at most ${SccpEvm.NATIVE_RECURSIVE_MAX_PROOF_BYTES} bytes"
        }
        return copy
    }

    private fun normalizeNonZeroHex32(value: String, field: String): String {
        require(value.startsWith("0x") && value.length == 66) {
            "$field must be 32 bytes of canonical lowercase 0x hex"
        }
        val text = value.substring(2)
        require(text.all { it in '0'..'9' || it in 'a'..'f' }) {
            "$field must be 32 bytes of canonical lowercase 0x hex"
        }
        require(text.any { it != '0' }) { "$field must not be zero" }
        return "0x$text"
    }
}

/** BSC mainnet SCCP Groth16 helpers with chain-id and domain checks baked in. */
object SccpBsc {
    const val DOMAIN_SORA: Int = SccpEvm.DOMAIN_SORA
    const val DOMAIN_BSC: Int = SccpEvm.DOMAIN_BSC
    const val MAINNET_CHAIN_ID: Long = SccpSourceProofs.BSC_MAINNET_CHAIN_ID
    const val MAINNET_NETWORK_ID: String = SccpSourceProofs.BSC_MAINNET_NETWORK_ID
    const val LOCAL_ADMISSION_ENVELOPE_ENCODING_V1: String = "norito:sccp-local-admission:v1"
    const val LOCAL_ADMISSION_SUBMISSION_KIND_V1: String = "local_admission"
    const val LOCAL_ADMISSION_ENTRYPOINT_V1: String = "SubmitBridgeProof"
    const val STARK_FRI_PROOF_FAMILY_V1: String = "stark-fri-v1"

    @JvmStatic
    fun requireMainnetChainId(chainId: Long) {
        require(chainId == MAINNET_CHAIN_ID) {
            "BSC mainnet SCCP requires eth_chainId == 56"
        }
    }

    @JvmStatic
    @JvmOverloads
    fun destinationBinding(
        verifierAddress: String,
        bridgeAddress: String,
        verifierCodeHash: String,
        verifierKeyHash: String,
        networkId: String = MAINNET_NETWORK_ID,
    ): SccpSourceProofs.EvmDestinationBinding =
        SccpSourceProofs.bscMainnetDestinationBinding(
            verifierAddress = verifierAddress,
            bridgeAddress = bridgeAddress,
            verifierCodeHash = verifierCodeHash,
            verifierKeyHash = verifierKeyHash,
            networkId = networkId,
        )

    @JvmStatic
    @JvmOverloads
    fun destinationBindingHash(
        verifierAddress: String,
        bridgeAddress: String,
        verifierCodeHash: String,
        verifierKeyHash: String,
        networkId: String = MAINNET_NETWORK_ID,
    ): String = destinationBinding(
        verifierAddress = verifierAddress,
        bridgeAddress = bridgeAddress,
        verifierCodeHash = verifierCodeHash,
        verifierKeyHash = verifierKeyHash,
        networkId = networkId,
    ).hash

    @JvmStatic
    fun buildProofRequest(input: EvmSccpProofRequestInput): EvmSccpProofRequest {
        require(input.publicInputs.targetDomain == DOMAIN_BSC) {
            "BSC mainnet proof requests must target BSC"
        }
        val destinationBinding = input.destinationBinding
            ?: throw IllegalArgumentException("BSC mainnet proof requests require destinationBinding")
        require(destinationBinding.targetDomain == DOMAIN_BSC) {
            "BSC mainnet destinationBinding must target BSC"
        }
        require(destinationBinding.networkId == MAINNET_NETWORK_ID) {
            "BSC mainnet destinationBinding.networkId must be chain id 56"
        }
        val destinationBindingHash = SccpEvm.requireDestinationBindingHashForProofRequest(
            publicInputs = input.publicInputs,
            destinationBinding = destinationBinding,
            backend = input.backend,
            sourceDomain = input.sourceDomain,
        )
        require(input.destinationBindingHash == destinationBindingHash) {
            "destinationBindingHash must match BSC mainnet destinationBinding"
        }
        return SccpEvm.buildProofRequest(input)
    }

    @JvmStatic
    fun wrapProofResult(proofBytes: ByteArray, request: EvmSccpProofRequest): EvmSccpProofResult {
        require(request.targetDomain == DOMAIN_BSC && request.publicInputs.targetDomain == DOMAIN_BSC) {
            "BSC mainnet proof results must target BSC"
        }
        require(request.destinationBinding?.networkId == MAINNET_NETWORK_ID) {
            "BSC mainnet proof results require chain id 56 destinationBinding"
        }
        require(request.destinationBinding.hash == request.destinationBindingHash) {
            "destinationBindingHash must match BSC mainnet destinationBinding"
        }
        return SccpEvm.wrapProofResult(proofBytes, request)
    }

    @JvmStatic
    fun buildSubmission(input: EvmSccpSubmissionInput): EvmSccpSubmission {
        require(input.publicInputs.targetDomain == DOMAIN_BSC) {
            "BSC mainnet submissions must target BSC"
        }
        val proofResult = input.proofResult
            ?: throw IllegalArgumentException(
                "BSC mainnet submissions require a wrapped proofResult with destinationBinding",
            )
        require(proofResult.publicInputs.targetDomain == DOMAIN_BSC) {
            "BSC mainnet proofResult must target BSC"
        }
        val destinationBinding = proofResult.destinationBinding
            ?: throw IllegalArgumentException("BSC mainnet proofResult requires destinationBinding")
        require(destinationBinding.networkId == MAINNET_NETWORK_ID) {
            "BSC mainnet proofResult requires chain id 56 destinationBinding"
        }
        require(destinationBinding.hash == proofResult.destinationBindingHash) {
            "BSC mainnet proofResult destinationBindingHash must match destinationBinding"
        }
        return SccpEvm.buildSubmission(input)
    }

    @JvmStatic
    fun buildLocalAdmissionSubmission(
        input: BscMainnetLocalAdmissionSubmissionInput,
    ): BscMainnetLocalAdmissionSubmission {
        require(input.sourceDomain == DOMAIN_BSC && input.targetDomain == DOMAIN_SORA) {
            "BSC mainnet local-admission submissions must route BSC -> SORA"
        }
        require(input.envelopeEncoding == LOCAL_ADMISSION_ENVELOPE_ENCODING_V1) {
            "BSC mainnet local-admission envelopeEncoding is not canonical"
        }
        require(input.submissionKind == LOCAL_ADMISSION_SUBMISSION_KIND_V1) {
            "BSC mainnet local-admission submissionKind is not canonical"
        }
        require(input.verifierEntrypoint == LOCAL_ADMISSION_ENTRYPOINT_V1) {
            "BSC mainnet local-admission verifierEntrypoint is not canonical"
        }
        require(input.proofFamily == STARK_FRI_PROOF_FAMILY_V1) {
            "BSC mainnet local-admission proofFamily is not canonical"
        }
        require(input.verifierBackend == SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1) {
            "BSC mainnet local-admission verifierBackend is not canonical"
        }
        val proofBytes = requireNativeRecursiveBytes(input.proofBytes, "proofBytes")
        val publicInputsBytes = requireNativeRecursiveBytes(
            input.publicInputsBytes,
            "publicInputsBytes",
        )
        val bundleBytes = requireNativeRecursiveBytes(input.bundleBytes, "bundleBytes")
        val envelopeBytes = requireNativeRecursiveBytes(input.envelopeBytes, "envelopeBytes")
        val statementHash = normalizeNonZeroHex32(input.statementHash, "statementHash")
        val sourceVerifierMaterialHash = normalizeNonZeroHex32(
            input.sourceVerifierMaterialHash,
            "sourceVerifierMaterialHash",
        )
        val sourceAdapterEngineDeploymentHash = normalizeNonZeroHex32(
            input.sourceAdapterEngineDeploymentHash,
            "sourceAdapterEngineDeploymentHash",
        )
        val localAdmission = BscMainnetLocalAdmissionPayload(
            proofBytes = proofBytes,
            publicInputsBytes = publicInputsBytes,
            bundleBytes = bundleBytes,
            statementHash = statementHash,
            sourceVerifierMaterialHash = sourceVerifierMaterialHash,
            sourceAdapterEngineDeploymentHash = sourceAdapterEngineDeploymentHash,
        )
        return BscMainnetLocalAdmissionSubmission(
            version = 1,
            proofFamily = input.proofFamily,
            verifierBackend = input.verifierBackend,
            platformPayload = LOCAL_ADMISSION_SUBMISSION_KIND_V1,
            envelopeEncoding = LOCAL_ADMISSION_ENVELOPE_ENCODING_V1,
            submissionKind = LOCAL_ADMISSION_SUBMISSION_KIND_V1,
            verifierEntrypoint = LOCAL_ADMISSION_ENTRYPOINT_V1,
            sourceDomain = DOMAIN_BSC,
            targetDomain = DOMAIN_SORA,
            statementHash = statementHash,
            sourceVerifierMaterialHash = sourceVerifierMaterialHash,
            sourceAdapterEngineDeploymentHash = sourceAdapterEngineDeploymentHash,
            localAdmission = localAdmission,
            proofBytes = proofBytes,
            publicInputsBytes = publicInputsBytes,
            bundleBytes = bundleBytes,
            envelopeBytes = envelopeBytes,
        )
    }

    private fun requireNativeRecursiveBytes(bytes: ByteArray, label: String): ByteArray {
        val copy = bytes.copyOf()
        require(copy.isNotEmpty()) { "$label must not be empty" }
        require(copy.any { it.toInt() != 0 }) { "$label must not be all zero" }
        require(copy.size <= SccpEvm.NATIVE_RECURSIVE_MAX_PROOF_BYTES) {
            "$label must be at most ${SccpEvm.NATIVE_RECURSIVE_MAX_PROOF_BYTES} bytes"
        }
        return copy
    }

    private fun normalizeNonZeroHex32(value: String, field: String): String {
        require(value.startsWith("0x") && value.length == 66) {
            "$field must be 32 bytes of canonical lowercase 0x hex"
        }
        val text = value.substring(2)
        require(text.all { it in '0'..'9' || it in 'a'..'f' }) {
            "$field must be 32 bytes of canonical lowercase 0x hex"
        }
        require(text.any { it != '0' }) { "$field must not be zero" }
        return "0x$text"
    }
}

/** Input for Ethereum mainnet -> SORA local-admission submission packaging. */
data class EthereumMainnetLocalAdmissionSubmissionInput(
    val proofBytes: ByteArray,
    val publicInputsBytes: ByteArray,
    val bundleBytes: ByteArray,
    val envelopeBytes: ByteArray,
    val statementHash: String,
    val sourceVerifierMaterialHash: String,
    val sourceAdapterEngineDeploymentHash: String,
    val sourceDomain: Int = SccpEthereumMainnet.DOMAIN_ETH,
    val targetDomain: Int = SccpEthereumMainnet.DOMAIN_SORA,
    val proofFamily: String = SccpEthereumMainnet.STARK_FRI_PROOF_FAMILY_V1,
    val verifierBackend: String = SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1,
    val envelopeEncoding: String = SccpEthereumMainnet.LOCAL_ADMISSION_ENVELOPE_ENCODING_V1,
    val submissionKind: String = SccpEthereumMainnet.LOCAL_ADMISSION_SUBMISSION_KIND_V1,
    val verifierEntrypoint: String = SccpEthereumMainnet.LOCAL_ADMISSION_ENTRYPOINT_V1,
)

/** Ethereum mainnet local-admission payload mirrored from the core SCCP package. */
class EthereumMainnetLocalAdmissionPayload(
    val version: Int = 1,
    proofBytes: ByteArray,
    publicInputsBytes: ByteArray,
    bundleBytes: ByteArray,
    val statementHash: String,
    val sourceVerifierMaterialHash: String,
    val sourceAdapterEngineDeploymentHash: String,
) {
    private val proofBytesStorage: ByteArray = proofBytes.copyOf()
    private val publicInputsBytesStorage: ByteArray = publicInputsBytes.copyOf()
    private val bundleBytesStorage: ByteArray = bundleBytes.copyOf()

    val proofBytes: ByteArray
        get() = proofBytesStorage.copyOf()

    val publicInputsBytes: ByteArray
        get() = publicInputsBytesStorage.copyOf()

    val bundleBytes: ByteArray
        get() = bundleBytesStorage.copyOf()
}

/** Ethereum mainnet -> SORA local-admission package ready for Torii bridge-proof submission. */
class EthereumMainnetLocalAdmissionSubmission(
    val version: Int,
    val proofFamily: String,
    val verifierBackend: String,
    val platformPayload: String,
    val envelopeEncoding: String,
    val submissionKind: String,
    val verifierEntrypoint: String,
    val sourceDomain: Int,
    val targetDomain: Int,
    val statementHash: String,
    val sourceVerifierMaterialHash: String,
    val sourceAdapterEngineDeploymentHash: String,
    val localAdmission: EthereumMainnetLocalAdmissionPayload,
    proofBytes: ByteArray,
    publicInputsBytes: ByteArray,
    bundleBytes: ByteArray,
    envelopeBytes: ByteArray,
) {
    private val proofBytesStorage: ByteArray = proofBytes.copyOf()
    private val publicInputsBytesStorage: ByteArray = publicInputsBytes.copyOf()
    private val bundleBytesStorage: ByteArray = bundleBytes.copyOf()
    private val envelopeBytesStorage: ByteArray = envelopeBytes.copyOf()

    val arguments: List<EvmSccpSubmissionArgument> = emptyList()
    val proofBytesHex: String = "0x" + localHexLower(proofBytes)
    val publicInputsBytesHex: String = "0x" + localHexLower(publicInputsBytes)
    val bundleBytesHex: String = "0x" + localHexLower(bundleBytes)
    val envelopeHex: String = "0x" + localHexLower(envelopeBytes)

    val proofBytes: ByteArray
        get() = proofBytesStorage.copyOf()

    val publicInputsBytes: ByteArray
        get() = publicInputsBytesStorage.copyOf()

    val bundleBytes: ByteArray
        get() = bundleBytesStorage.copyOf()

    val envelopeBytes: ByteArray
        get() = envelopeBytesStorage.copyOf()
}

/** Input for BSC -> SORA local-admission submission packaging. */
data class BscMainnetLocalAdmissionSubmissionInput(
    val proofBytes: ByteArray,
    val publicInputsBytes: ByteArray,
    val bundleBytes: ByteArray,
    val envelopeBytes: ByteArray,
    val statementHash: String,
    val sourceVerifierMaterialHash: String,
    val sourceAdapterEngineDeploymentHash: String,
    val sourceDomain: Int = SccpBsc.DOMAIN_BSC,
    val targetDomain: Int = SccpBsc.DOMAIN_SORA,
    val proofFamily: String = SccpBsc.STARK_FRI_PROOF_FAMILY_V1,
    val verifierBackend: String = SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1,
    val envelopeEncoding: String = SccpBsc.LOCAL_ADMISSION_ENVELOPE_ENCODING_V1,
    val submissionKind: String = SccpBsc.LOCAL_ADMISSION_SUBMISSION_KIND_V1,
    val verifierEntrypoint: String = SccpBsc.LOCAL_ADMISSION_ENTRYPOINT_V1,
)

/** BSC local-admission payload mirrored from the core SCCP package. */
class BscMainnetLocalAdmissionPayload(
    val version: Int = 1,
    proofBytes: ByteArray,
    publicInputsBytes: ByteArray,
    bundleBytes: ByteArray,
    val statementHash: String,
    val sourceVerifierMaterialHash: String,
    val sourceAdapterEngineDeploymentHash: String,
) {
    private val proofBytesStorage: ByteArray = proofBytes.copyOf()
    private val publicInputsBytesStorage: ByteArray = publicInputsBytes.copyOf()
    private val bundleBytesStorage: ByteArray = bundleBytes.copyOf()

    val proofBytes: ByteArray
        get() = proofBytesStorage.copyOf()

    val publicInputsBytes: ByteArray
        get() = publicInputsBytesStorage.copyOf()

    val bundleBytes: ByteArray
        get() = bundleBytesStorage.copyOf()
}

/** BSC -> SORA local-admission package ready for Torii bridge-proof submission. */
class BscMainnetLocalAdmissionSubmission(
    val version: Int,
    val proofFamily: String,
    val verifierBackend: String,
    val platformPayload: String,
    val envelopeEncoding: String,
    val submissionKind: String,
    val verifierEntrypoint: String,
    val sourceDomain: Int,
    val targetDomain: Int,
    val statementHash: String,
    val sourceVerifierMaterialHash: String,
    val sourceAdapterEngineDeploymentHash: String,
    val localAdmission: BscMainnetLocalAdmissionPayload,
    proofBytes: ByteArray,
    publicInputsBytes: ByteArray,
    bundleBytes: ByteArray,
    envelopeBytes: ByteArray,
) {
    private val proofBytesStorage: ByteArray = proofBytes.copyOf()
    private val publicInputsBytesStorage: ByteArray = publicInputsBytes.copyOf()
    private val bundleBytesStorage: ByteArray = bundleBytes.copyOf()
    private val envelopeBytesStorage: ByteArray = envelopeBytes.copyOf()

    val arguments: List<EvmSccpSubmissionArgument> = emptyList()
    val proofBytesHex: String = "0x" + localHexLower(proofBytes)
    val publicInputsBytesHex: String = "0x" + localHexLower(publicInputsBytes)
    val bundleBytesHex: String = "0x" + localHexLower(bundleBytes)
    val envelopeHex: String = "0x" + localHexLower(envelopeBytes)

    val proofBytes: ByteArray
        get() = proofBytesStorage.copyOf()

    val publicInputsBytes: ByteArray
        get() = publicInputsBytesStorage.copyOf()

    val bundleBytes: ByteArray
        get() = bundleBytesStorage.copyOf()

    val envelopeBytes: ByteArray
        get() = envelopeBytesStorage.copyOf()
}

private fun localHexLower(bytes: ByteArray): String =
    bytes.joinToString(separator = "") { byte -> (byte.toInt() and 0xff).toString(16).padStart(2, '0') }

/** SCCP public inputs shared by EVM-family Groth16 proof requests. */
data class EvmSccpPublicInputsInput(
    val version: Int = 1,
    val messageId: String,
    val payloadHash: String,
    val targetDomain: Int = SccpEvm.DOMAIN_ETH,
    val commitmentRoot: String,
    val finalityHeight: String,
    val finalityBlockHash: String,
) {
    internal fun toTronPublicInputs(): TronSccpPublicInputsInput =
        TronSccpPublicInputsInput(
            version = version,
            messageId = messageId,
            payloadHash = payloadHash,
            targetDomain = targetDomain,
            commitmentRoot = commitmentRoot,
            finalityHeight = finalityHeight,
            finalityBlockHash = finalityBlockHash,
        )
}

/** Inputs used to build a local EVM-family SCCP Groth16 proof request. */
data class EvmSccpProofRequestInput @JvmOverloads constructor(
    val publicInputs: EvmSccpPublicInputsInput,
    val bundleBytes: ByteArray,
    val sourceProofBytes: ByteArray = ByteArray(0),
    val statementHash: String,
    val destinationBindingHash: String,
    val backend: String = SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1,
    val sourceDomain: Int = SccpEvm.DOMAIN_SORA,
    val destinationBinding: SccpSourceProofs.EvmDestinationBinding? = null,
    val proofArtifactHash: String? = null,
    val provingKeyHash: String? = null,
) {
    constructor(
        publicInputs: EvmSccpPublicInputsInput,
        bundleBytes: ByteArray,
        sourceProofBytes: ByteArray = ByteArray(0),
        statementHash: String,
        destinationBinding: SccpSourceProofs.EvmDestinationBinding,
        backend: String = SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1,
        sourceDomain: Int = SccpEvm.DOMAIN_SORA,
        proofArtifactHash: String? = null,
        provingKeyHash: String? = null,
    ) : this(
        publicInputs = publicInputs,
        bundleBytes = bundleBytes,
        sourceProofBytes = sourceProofBytes,
        statementHash = statementHash,
        destinationBindingHash = SccpEvm.requireDestinationBindingHashForProofRequest(
            publicInputs = publicInputs,
            destinationBinding = destinationBinding,
            backend = backend,
            sourceDomain = sourceDomain,
        ),
        backend = backend,
        sourceDomain = sourceDomain,
        destinationBinding = destinationBinding,
        proofArtifactHash = proofArtifactHash,
        provingKeyHash = provingKeyHash,
    )
}

/** Statement and verifier deployment context proved by the local EVM-family SCCP prover. */
data class EvmSccpProofContext(
    val version: Int,
    val statementHash: String,
    val destinationBindingHash: String,
)

/** Request passed to a linked local EVM-family Groth16 prover. */
class EvmSccpProofRequest @JvmOverloads constructor(
    val version: Int,
    val backend: String,
    val sourceDomain: Int,
    val targetDomain: Int,
    val publicInputs: EvmSccpPublicInputsInput,
    publicInputsBytes: ByteArray,
    publicSignalWords: List<String>,
    bundleBytes: ByteArray,
    sourceProofBytes: ByteArray,
    val proofContext: EvmSccpProofContext,
    val statementHash: String,
    val destinationBindingHash: String,
    val requestHash: String,
    val destinationBinding: SccpSourceProofs.EvmDestinationBinding? = null,
    val proofArtifactHash: String? = null,
    val provingKeyHash: String? = null,
) {
    private val publicInputsBytesStorage: ByteArray = publicInputsBytes.copyOf()
    private val bundleBytesStorage: ByteArray = bundleBytes.copyOf()
    private val sourceProofBytesStorage: ByteArray = sourceProofBytes.copyOf()

    val publicInputsBytes: ByteArray
        get() = publicInputsBytesStorage.copyOf()

    val publicSignalWords: List<String> = publicSignalWords.toList()

    val bundleBytes: ByteArray
        get() = bundleBytesStorage.copyOf()

    val sourceProofBytes: ByteArray
        get() = sourceProofBytesStorage.copyOf()

    fun copy(
        version: Int = this.version,
        backend: String = this.backend,
        sourceDomain: Int = this.sourceDomain,
        targetDomain: Int = this.targetDomain,
        publicInputs: EvmSccpPublicInputsInput = this.publicInputs,
        publicInputsBytes: ByteArray = this.publicInputsBytes,
        publicSignalWords: List<String> = this.publicSignalWords,
        bundleBytes: ByteArray = this.bundleBytes,
        sourceProofBytes: ByteArray = this.sourceProofBytes,
        proofContext: EvmSccpProofContext = this.proofContext,
        statementHash: String = this.statementHash,
        destinationBindingHash: String = this.destinationBindingHash,
        requestHash: String = this.requestHash,
        destinationBinding: SccpSourceProofs.EvmDestinationBinding? = this.destinationBinding,
        proofArtifactHash: String? = this.proofArtifactHash,
        provingKeyHash: String? = this.provingKeyHash,
    ): EvmSccpProofRequest =
        EvmSccpProofRequest(
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
            proofArtifactHash,
            provingKeyHash,
        )

    operator fun component1(): Int = version
    operator fun component2(): String = backend
    operator fun component3(): Int = sourceDomain
    operator fun component4(): Int = targetDomain
    operator fun component5(): EvmSccpPublicInputsInput = publicInputs
    operator fun component6(): ByteArray = publicInputsBytes
    operator fun component7(): List<String> = publicSignalWords
    operator fun component8(): ByteArray = bundleBytes
    operator fun component9(): ByteArray = sourceProofBytes
    operator fun component10(): EvmSccpProofContext = proofContext
    operator fun component11(): String = statementHash
    operator fun component12(): String = destinationBindingHash
    operator fun component13(): String = requestHash
    operator fun component14(): SccpSourceProofs.EvmDestinationBinding? = destinationBinding
    operator fun component15(): String? = proofArtifactHash
    operator fun component16(): String? = provingKeyHash

    override fun equals(other: Any?): Boolean =
        this === other ||
            other is EvmSccpProofRequest &&
            version == other.version &&
            backend == other.backend &&
            sourceDomain == other.sourceDomain &&
            targetDomain == other.targetDomain &&
            publicInputs == other.publicInputs &&
            publicInputsBytesStorage.contentEquals(other.publicInputsBytesStorage) &&
            publicSignalWords == other.publicSignalWords &&
            bundleBytesStorage.contentEquals(other.bundleBytesStorage) &&
            sourceProofBytesStorage.contentEquals(other.sourceProofBytesStorage) &&
            proofContext == other.proofContext &&
            statementHash == other.statementHash &&
            destinationBindingHash == other.destinationBindingHash &&
            requestHash == other.requestHash &&
            destinationBinding == other.destinationBinding &&
            proofArtifactHash == other.proofArtifactHash &&
            provingKeyHash == other.provingKeyHash

    override fun hashCode(): Int {
        var result = version
        result = 31 * result + backend.hashCode()
        result = 31 * result + sourceDomain
        result = 31 * result + targetDomain
        result = 31 * result + publicInputs.hashCode()
        result = 31 * result + publicInputsBytesStorage.contentHashCode()
        result = 31 * result + publicSignalWords.hashCode()
        result = 31 * result + bundleBytesStorage.contentHashCode()
        result = 31 * result + sourceProofBytesStorage.contentHashCode()
        result = 31 * result + proofContext.hashCode()
        result = 31 * result + statementHash.hashCode()
        result = 31 * result + destinationBindingHash.hashCode()
        result = 31 * result + requestHash.hashCode()
        result = 31 * result + (destinationBinding?.hashCode() ?: 0)
        result = 31 * result + (proofArtifactHash?.hashCode() ?: 0)
        result = 31 * result + (provingKeyHash?.hashCode() ?: 0)
        return result
    }

    override fun toString(): String =
        "EvmSccpProofRequest(version=$version, backend=$backend, sourceDomain=$sourceDomain, " +
            "targetDomain=$targetDomain, publicInputs=$publicInputs, " +
            "publicInputsBytes=${publicInputsBytesStorage.size} bytes, " +
            "publicSignalWords=$publicSignalWords, bundleBytes=${bundleBytesStorage.size} bytes, " +
            "sourceProofBytes=${sourceProofBytesStorage.size} bytes, proofContext=$proofContext, " +
            "statementHash=$statementHash, destinationBindingHash=$destinationBindingHash, " +
            "proofArtifactHash=$proofArtifactHash, provingKeyHash=$provingKeyHash, " +
            "requestHash=$requestHash, destinationBinding=$destinationBinding)"
}

/** Proof bytes returned by a linked local EVM-family Groth16 prover. */
class EvmSccpProofResult @JvmOverloads constructor(
    val version: Int,
    val backend: String,
    proofBytes: ByteArray,
    val proofBase64: String,
    val publicInputs: EvmSccpPublicInputsInput,
    publicSignalWords: List<String>,
    bundleBytes: ByteArray = ByteArray(0),
    sourceProofBytes: ByteArray = ByteArray(0),
    val proofContext: EvmSccpProofContext,
    val statementHash: String,
    val destinationBindingHash: String,
    val requestHash: String,
    val envelopeHash: String,
    val destinationBinding: SccpSourceProofs.EvmDestinationBinding? = null,
    val proofArtifactHash: String? = null,
    val provingKeyHash: String? = null,
) {
    private val proofBytesStorage: ByteArray = proofBytes.copyOf()
    private val bundleBytesStorage: ByteArray = bundleBytes.copyOf()
    private val sourceProofBytesStorage: ByteArray = sourceProofBytes.copyOf()

    val proofBytes: ByteArray
        get() = proofBytesStorage.copyOf()

    val publicSignalWords: List<String> = publicSignalWords.toList()

    val bundleBytes: ByteArray
        get() = bundleBytesStorage.copyOf()

    val sourceProofBytes: ByteArray
        get() = sourceProofBytesStorage.copyOf()

    fun copy(
        version: Int = this.version,
        backend: String = this.backend,
        proofBytes: ByteArray = this.proofBytes,
        proofBase64: String = this.proofBase64,
        publicInputs: EvmSccpPublicInputsInput = this.publicInputs,
        publicSignalWords: List<String> = this.publicSignalWords,
        bundleBytes: ByteArray = this.bundleBytes,
        sourceProofBytes: ByteArray = this.sourceProofBytes,
        proofContext: EvmSccpProofContext = this.proofContext,
        statementHash: String = this.statementHash,
        destinationBindingHash: String = this.destinationBindingHash,
        requestHash: String = this.requestHash,
        envelopeHash: String = this.envelopeHash,
        destinationBinding: SccpSourceProofs.EvmDestinationBinding? = this.destinationBinding,
        proofArtifactHash: String? = this.proofArtifactHash,
        provingKeyHash: String? = this.provingKeyHash,
    ): EvmSccpProofResult =
        EvmSccpProofResult(
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
            proofArtifactHash,
            provingKeyHash,
        )

    operator fun component1(): Int = version
    operator fun component2(): String = backend
    operator fun component3(): ByteArray = proofBytes
    operator fun component4(): String = proofBase64
    operator fun component5(): EvmSccpPublicInputsInput = publicInputs
    operator fun component6(): List<String> = publicSignalWords
    operator fun component7(): ByteArray = bundleBytes
    operator fun component8(): ByteArray = sourceProofBytes
    operator fun component9(): EvmSccpProofContext = proofContext
    operator fun component10(): String = statementHash
    operator fun component11(): String = destinationBindingHash
    operator fun component12(): String = requestHash
    operator fun component13(): String = envelopeHash
    operator fun component14(): SccpSourceProofs.EvmDestinationBinding? = destinationBinding
    operator fun component15(): String? = proofArtifactHash
    operator fun component16(): String? = provingKeyHash

    override fun equals(other: Any?): Boolean =
        this === other ||
            other is EvmSccpProofResult &&
            version == other.version &&
            backend == other.backend &&
            proofBytesStorage.contentEquals(other.proofBytesStorage) &&
            proofBase64 == other.proofBase64 &&
            publicInputs == other.publicInputs &&
            publicSignalWords == other.publicSignalWords &&
            bundleBytesStorage.contentEquals(other.bundleBytesStorage) &&
            sourceProofBytesStorage.contentEquals(other.sourceProofBytesStorage) &&
            proofContext == other.proofContext &&
            statementHash == other.statementHash &&
            destinationBindingHash == other.destinationBindingHash &&
            requestHash == other.requestHash &&
            envelopeHash == other.envelopeHash &&
            destinationBinding == other.destinationBinding &&
            proofArtifactHash == other.proofArtifactHash &&
            provingKeyHash == other.provingKeyHash

    override fun hashCode(): Int {
        var result = version
        result = 31 * result + backend.hashCode()
        result = 31 * result + proofBytesStorage.contentHashCode()
        result = 31 * result + proofBase64.hashCode()
        result = 31 * result + publicInputs.hashCode()
        result = 31 * result + publicSignalWords.hashCode()
        result = 31 * result + bundleBytesStorage.contentHashCode()
        result = 31 * result + sourceProofBytesStorage.contentHashCode()
        result = 31 * result + proofContext.hashCode()
        result = 31 * result + statementHash.hashCode()
        result = 31 * result + destinationBindingHash.hashCode()
        result = 31 * result + requestHash.hashCode()
        result = 31 * result + envelopeHash.hashCode()
        result = 31 * result + (destinationBinding?.hashCode() ?: 0)
        result = 31 * result + (proofArtifactHash?.hashCode() ?: 0)
        result = 31 * result + (provingKeyHash?.hashCode() ?: 0)
        return result
    }

    override fun toString(): String =
        "EvmSccpProofResult(version=$version, backend=$backend, " +
            "proofBytes=${proofBytesStorage.size} bytes, proofBase64=$proofBase64, " +
            "publicInputs=$publicInputs, publicSignalWords=$publicSignalWords, " +
            "bundleBytes=${bundleBytesStorage.size} bytes, " +
            "sourceProofBytes=${sourceProofBytesStorage.size} bytes, " +
            "proofContext=$proofContext, statementHash=$statementHash, " +
            "destinationBindingHash=$destinationBindingHash, requestHash=$requestHash, " +
            "envelopeHash=$envelopeHash, destinationBinding=$destinationBinding, " +
            "proofArtifactHash=$proofArtifactHash, provingKeyHash=$provingKeyHash)"
}

/** Inputs used to package an EVM-family Groth16 proof for verifier-contract submission. */
class EvmSccpSubmissionInput(
    val publicInputs: EvmSccpPublicInputsInput,
    proofBytes: ByteArray,
    val statementHash: String,
    val destinationBindingHash: String,
    val sourceDomain: Int = SccpEvm.DOMAIN_SORA,
    val proofResult: EvmSccpProofResult? = null,
    publicSignalWords: List<String>? = null,
) {
    private val proofBytesStorage: ByteArray = proofBytes.copyOf()

    val proofBytes: ByteArray
        get() = proofBytesStorage.copyOf()

    val publicSignalWords: List<String>? = publicSignalWords?.toList()

    constructor(
        proofResult: EvmSccpProofResult,
        publicInputs: EvmSccpPublicInputsInput = proofResult.publicInputs,
        sourceDomain: Int = SccpEvm.DOMAIN_SORA,
    ) : this(
        publicInputs = publicInputs,
        proofBytes = proofResult.proofBytes,
        statementHash = proofResult.statementHash,
        destinationBindingHash = proofResult.destinationBindingHash,
        sourceDomain = sourceDomain,
        proofResult = proofResult,
        publicSignalWords = proofResult.publicSignalWords,
    )

    constructor(
        publicInputs: EvmSccpPublicInputsInput,
        proofBytes: ByteArray,
        statementHash: String,
        destinationBinding: SccpSourceProofs.EvmDestinationBinding,
        sourceDomain: Int = SccpEvm.DOMAIN_SORA,
        proofResult: EvmSccpProofResult? = null,
        publicSignalWords: List<String>? = null,
    ) : this(
        publicInputs = publicInputs,
        proofBytes = proofBytes,
        statementHash = statementHash,
        destinationBindingHash = SccpEvm.requireDestinationBindingHashForProofRequest(
            publicInputs = publicInputs,
            destinationBinding = destinationBinding,
            backend = SccpEvm.GROTH16_BN254_PROOF_BACKEND_V1,
            sourceDomain = sourceDomain,
        ),
        sourceDomain = sourceDomain,
        proofResult = proofResult,
        publicSignalWords = publicSignalWords,
    )
}

/** ABI argument metadata for an EVM-family SCCP verifier-contract submission. */
data class EvmSccpSubmissionArgument(
    val key: String,
    val encoding: String,
    val bytes: String,
)

/** EVM-family SCCP verifier-contract call data ready for wallet or relayer submission. */
class EvmSccpSubmission(
    val version: Int,
    val proofFamily: String,
    val verifierBackend: String,
    val platformPayload: String,
    val envelopeEncoding: String,
    val submissionKind: String,
    val verifierEntrypoint: String,
    val contractMethod: String,
    val functionSelector: String,
    val sourceDomain: Int,
    val targetDomain: Int,
    val publicInputs: EvmSccpPublicInputsInput,
    publicInputWords: List<String>,
    publicSignalWords: List<String>,
    val statementHash: String,
    val destinationBindingHash: String,
    arguments: List<EvmSccpSubmissionArgument>,
    val callDataHex: String,
    val envelopeHex: String,
    proofBytes: ByteArray,
    publicInputWordsBytes: ByteArray,
    callData: ByteArray,
) {
    private val proofBytesStorage: ByteArray = proofBytes.copyOf()
    private val publicInputWordsBytesStorage: ByteArray = publicInputWordsBytes.copyOf()
    private val callDataStorage: ByteArray = callData.copyOf()

    val publicInputWords: List<String> = publicInputWords.toList()
    val publicSignalWords: List<String> = publicSignalWords.toList()
    val arguments: List<EvmSccpSubmissionArgument> = arguments.toList()

    val proofBytes: ByteArray
        get() = proofBytesStorage.copyOf()

    val publicInputWordsBytes: ByteArray
        get() = publicInputWordsBytesStorage.copyOf()

    val callData: ByteArray
        get() = callDataStorage.copyOf()

    val envelopeBytes: ByteArray
        get() = callDataStorage.copyOf()
}

/** Optional witness resolver backed by app-controlled EVM RPC calls. */
fun interface EvmSccpWitnessProvider {
    fun resolveWitness(input: EvmSccpProofRequestInput): EvmSccpProofRequestInput
}

/** Local EVM-family Groth16 proof engine linked by the application bundle. */
fun interface EvmSccpProofEngine {
    fun prove(request: EvmSccpProofRequest): ByteArray
}

/** App-supplied Ethereum JSON-RPC execution provider for native SCCP evidence collection. */
fun interface EthereumMainnetExecutionProvider {
    fun request(method: String, params: List<Any?>): Any?
}

/** App-supplied Ethereum Beacon REST finality collector for native SCCP evidence collection. */
fun interface EthereumMainnetConsensusProvider {
    fun collectFinalityEvidence(
        receipt: Map<String, Any?>?,
        block: Map<String, Any?>?,
        transactionHash: String?,
    ): Map<String, Any?>
}

/** Minimal HTTP response used by the Ethereum Beacon REST consensus provider. */
data class EthereumMainnetBeaconRestResponse(
    val statusCode: Int,
    val body: ByteArray,
    val statusMessage: String? = null,
) {
    fun snapshot(): EthereumMainnetBeaconRestResponse =
        copy(body = body.copyOf())
}

/** Injectable Beacon REST transport for tests and app-controlled networking stacks. */
fun interface EthereumMainnetBeaconRestTransport {
    fun get(url: String, headers: Map<String, String>): EthereumMainnetBeaconRestResponse
}

/** JDK-only Beacon REST transport for native Ethereum mainnet SCCP finality collection. */
object EthereumMainnetBeaconRestHttpTransport : EthereumMainnetBeaconRestTransport {
    override fun get(url: String, headers: Map<String, String>): EthereumMainnetBeaconRestResponse {
        val connection = URL(url).openConnection() as HttpURLConnection
        try {
            connection.requestMethod = "GET"
            for ((name, value) in headers) {
                connection.setRequestProperty(name, value)
            }
            val statusCode = connection.responseCode
            val body = readEthereumMainnetBeaconRestBody(
                if (statusCode in 200..299) connection.inputStream else connection.errorStream,
            )
            return EthereumMainnetBeaconRestResponse(statusCode, body, connection.responseMessage)
        } finally {
            connection.disconnect()
        }
    }
}

private fun readEthereumMainnetBeaconRestBody(stream: InputStream?): ByteArray {
    if (stream == null) return ByteArray(0)
    stream.use { input ->
        val out = ByteArrayOutputStream()
        val buffer = ByteArray(8192)
        var total = 0
        while (true) {
            val read = input.read(buffer)
            if (read < 0) break
            total += read
            require(total <= ETHEREUM_MAINNET_BEACON_REST_MAX_RESPONSE_BYTES) {
                "Ethereum mainnet Beacon REST response body must be at most " +
                    "$ETHEREUM_MAINNET_BEACON_REST_MAX_RESPONSE_BYTES bytes"
            }
            out.write(buffer, 0, read)
        }
        return out.toByteArray()
    }
}

/** Beacon REST-backed Ethereum mainnet finality collector for local-first SDK flows. */
class EthereumMainnetBeaconRestConsensusProvider @JvmOverloads constructor(
    endpoint: String,
    private val syncCommitteeRoot: String? = null,
    syncCommitteePayload: ByteArray? = null,
    headers: Map<String, String> = emptyMap(),
    private val verifyFinalityCheckpoint: Boolean = true,
    private val transport: EthereumMainnetBeaconRestTransport = EthereumMainnetBeaconRestHttpTransport,
) : EthereumMainnetConsensusProvider {
    private val endpoint: String = normalizeEthereumBeaconRestEndpoint(endpoint)
    private val syncCommitteePayload: ByteArray? = syncCommitteePayload?.copyOf()
    private val headers: Map<String, String> = headers.toMap()

    override fun collectFinalityEvidence(
        receipt: Map<String, Any?>?,
        block: Map<String, Any?>?,
        transactionHash: String?,
    ): Map<String, Any?> {
        require(block != null) {
            "Ethereum mainnet Beacon REST finality collection requires block"
        }
        val blockHash = normalizeEthereumBeaconRestHex(block["hash"], "block.hash", 32)
        val blockNumber = normalizeEthereumBeaconRestQuantity(
            strictFirstPresent(block, "block.number", "number", "blockNumber", "block_number"),
            "block.number",
        )
        require(blockNumber != "0x0") { "block.number must be positive" }
        val receiptsRoot = normalizeEthereumBeaconRestHex(
            strictFirstPresent(block, "block.receiptsRoot", "receiptsRoot", "receipts_root"),
            "block.receiptsRoot",
            32,
        )
        val targetBlockId = beaconRestBlockIdForTarget(block)
        val finalizedHeaderResponse = fetchJsonObject(
            "/eth/v1/beacon/headers/finalized",
            "Ethereum mainnet Beacon REST finalized header",
        )
        val finalizedHeader = ethereumBeaconRestHeaderSummary(
            finalizedHeaderResponse,
            "Ethereum mainnet Beacon REST finalized header",
        )
        val targetHeader = if (targetBlockId.id == "finalized") {
            finalizedHeader
        } else {
            ethereumBeaconRestHeaderSummary(
                fetchJsonObject(
                    "/eth/v1/beacon/headers/${targetBlockId.id}",
                    "Ethereum mainnet Beacon REST finalized target header",
                ),
                "Ethereum mainnet Beacon REST finalized target header",
            )
        }
        require(targetBlockId.slot == null || targetHeader.slot == targetBlockId.slot) {
            "Ethereum mainnet Beacon REST finalized target header slot must match beaconSlot"
        }
        require(targetBlockId.root == null || targetHeader.root == targetBlockId.root) {
            "Ethereum mainnet Beacon REST finalized target header root must match beaconBlockRoot"
        }
        require(targetHeader.slot <= finalizedHeader.slot) {
            "Ethereum mainnet Beacon REST target block is newer than the finalized header"
        }
        require(targetHeader.slot == finalizedHeader.slot) {
            "Ethereum mainnet Beacon REST historical target blocks require an ancestry proof"
        }
        require(targetHeader.slot != finalizedHeader.slot || targetHeader.root == finalizedHeader.root) {
            "Ethereum mainnet Beacon REST target header root must match finalized header root at the same slot"
        }
        val finalizedBlockRootResponse = fetchJsonObject(
            "/eth/v1/beacon/blocks/${targetBlockId.id}/root",
            "Ethereum mainnet Beacon REST finalized block root",
        )
        rejectUnsafeBeaconRestPayload(
            finalizedBlockRootResponse,
            "Ethereum mainnet Beacon REST finalized block root",
        )
        val finalizedBlockRootData = expectBeaconRestObject(
            requireBeaconRestField(
                finalizedBlockRootResponse,
                "Ethereum mainnet Beacon REST finalized block root",
                "data",
            ),
            "Ethereum mainnet Beacon REST finalized block root.data",
        )
        val finalizedBlockRootHash = normalizeEthereumBeaconRestHex(
            requireBeaconRestField(
                finalizedBlockRootData,
                "Ethereum mainnet Beacon REST finalized block root.data",
                "root",
            ),
            "finalizedBlockRoot",
            32,
        )
        require(finalizedBlockRootHash == targetHeader.root) {
            "Ethereum mainnet Beacon REST finalized block root must match finalized header root"
        }
        val finalizedBlockRoot = fetchJsonObject(
            "/eth/v2/beacon/blocks/${targetBlockId.id}",
            "Ethereum mainnet Beacon REST finalized block",
        )
        rejectUnsafeBeaconRestPayload(finalizedBlockRoot, "Ethereum mainnet Beacon REST finalized block")
        val blockData = expectBeaconRestObject(
            requireBeaconRestField(
                finalizedBlockRoot,
                "Ethereum mainnet Beacon REST finalized block",
                "data",
            ),
            "Ethereum mainnet Beacon REST finalized block.data",
        )
        val blockMessage = expectBeaconRestObject(
            requireBeaconRestField(
                blockData,
                "Ethereum mainnet Beacon REST finalized block.data",
                "message",
            ),
            "Ethereum mainnet Beacon REST finalized block.data.message",
        )
        val finalizedBlockSlot = normalizeEthereumBeaconRestUnsigned(
            requireBeaconRestField(
                blockMessage,
                "Ethereum mainnet Beacon REST finalized block.data.message",
                "slot",
            ),
            "Ethereum mainnet Beacon REST finalized block.data.message.slot",
        )
        require(finalizedBlockSlot == targetHeader.slot) {
            "Ethereum mainnet Beacon REST finalized block slot must match finalized header slot"
        }
        val blockBody = expectBeaconRestObject(
            requireBeaconRestField(
                blockMessage,
                "Ethereum mainnet Beacon REST finalized block.data.message",
                "body",
            ),
            "Ethereum mainnet Beacon REST finalized block.data.message.body",
        )
        val executionPayload = expectBeaconRestObject(
            requireBeaconRestField(
                blockBody,
                "Ethereum mainnet Beacon REST finalized block.data.message.body",
                "execution_payload",
            ),
            "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload",
        )
        val payloadBlockHash = normalizeEthereumBeaconRestHex(
            requireBeaconRestField(
                executionPayload,
                "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload",
                "block_hash",
            ),
            "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload.block_hash",
            32,
        )
        require(payloadBlockHash == blockHash) {
            "Ethereum mainnet Beacon REST execution payload block_hash must match block.hash"
        }
        val payloadBlockNumber = normalizeEthereumBeaconRestUnsigned(
            requireBeaconRestField(
                executionPayload,
                "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload",
                "block_number",
            ),
            "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload.block_number",
        )
        require(payloadBlockNumber == normalizeEthereumBeaconRestUnsigned(blockNumber, "block.number")) {
            "Ethereum mainnet Beacon REST execution payload block_number must match block.number"
        }
        val payloadReceiptsRoot = normalizeEthereumBeaconRestHex(
            requireBeaconRestField(
                executionPayload,
                "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload",
                "receipts_root",
            ),
            "Ethereum mainnet Beacon REST finalized block.data.message.body.execution_payload.receipts_root",
            32,
        )
        require(payloadReceiptsRoot == receiptsRoot) {
            "Ethereum mainnet Beacon REST execution payload receipts_root must match block.receiptsRoot"
        }
        if (verifyFinalityCheckpoint) {
            val checkpointRoot = fetchJsonObject(
                "/eth/v1/beacon/states/finalized/finality_checkpoints",
                "Ethereum mainnet Beacon REST finality checkpoints",
            )
            rejectUnsafeBeaconRestPayload(checkpointRoot, "Ethereum mainnet Beacon REST finality checkpoints")
            val checkpointData = expectBeaconRestObject(
                requireBeaconRestField(
                    checkpointRoot,
                    "Ethereum mainnet Beacon REST finality checkpoints",
                    "data",
                ),
                "Ethereum mainnet Beacon REST finality checkpoints.data",
            )
            val finalizedCheckpoint = expectBeaconRestObject(
                requireBeaconRestField(
                    checkpointData,
                    "Ethereum mainnet Beacon REST finality checkpoints.data",
                    "finalized",
                ),
                "Ethereum mainnet Beacon REST finality checkpoints.data.finalized",
            )
            val finalizedCheckpointRoot = normalizeEthereumBeaconRestHex(
                requireBeaconRestField(
                    finalizedCheckpoint,
                    "Ethereum mainnet Beacon REST finality checkpoints.data.finalized",
                    "root",
                ),
                "finalizedCheckpointRoot",
                32,
            )
            require(finalizedCheckpointRoot == finalizedHeader.root) {
                "Ethereum mainnet Beacon REST finality checkpoint root must match finalized header root"
            }
        }
        val finalityUpdate = ethereumBeaconRestFinalityUpdateSummary(
            fetchJsonObject(
                "/eth/v1/beacon/light_client/finality_update",
                "Ethereum mainnet Beacon REST light-client finality update",
            ),
            finalizedHeader.slot,
            finalizedHeader.root,
        )
        return mapOf(
            "executionBlockNumber" to normalizeEthereumBeaconRestUnsigned(blockNumber, "block.number").toString(),
            "executionBlockHash" to blockHash,
            "executionReceiptsRoot" to receiptsRoot,
            "finalizedHeaderRoot" to finalityUpdate.finalizedHeaderRoot,
            "syncCommitteeRoot" to resolveEthereumBeaconRestSyncCommitteeRoot(
                syncCommitteeRoot,
                syncCommitteePayload,
            ),
            "beaconSlot" to finalityUpdate.beaconSlot.toString(),
            "finalityBranch" to finalityUpdate.finalityBranch,
            "syncCommitteeBits" to finalityUpdate.syncCommitteeBits,
            "syncCommitteeSignature" to finalityUpdate.syncCommitteeSignature,
            "syncCommitteeParticipation" to finalityUpdate.syncCommitteeParticipation.toString(),
            "syncSignatureSlot" to finalityUpdate.syncSignatureSlot.toString(),
        )
    }

    private fun beaconRestBlockIdForTarget(block: Map<String, Any?>): EthereumBeaconRestBlockId {
        val rootInput = firstPresent(
            block,
            "beaconBlockRoot",
            "beacon_block_root",
            "targetBeaconBlockRoot",
            "target_beacon_block_root",
        )
        if (rootInput != null) {
            val root = normalizeEthereumBeaconRestHex(rootInput, "block.beaconBlockRoot", 32)
            return EthereumBeaconRestBlockId(root, root = root)
        }
        val idInput = firstPresent(
            block,
            "beaconBlockId",
            "beacon_block_id",
            "targetBeaconBlockId",
            "target_beacon_block_id",
        )
        if (idInput != null) return ethereumBeaconRestBlockIdFromValue(idInput, "block.beaconBlockId")
        val slotInput = firstPresent(
            block,
            "beaconSlot",
            "beacon_slot",
            "finalizedSlot",
            "finalized_slot",
            "slot",
        )
        if (slotInput != null) {
            val slot = normalizeEthereumBeaconRestSlot(slotInput, "block.beaconSlot")
            return EthereumBeaconRestBlockId(slot.toString(), slot = slot)
        }
        val timestampInput = firstPresent(block, "timestamp", "blockTimestamp", "block_timestamp")
        if (timestampInput != null) {
            val timestamp = normalizeEthereumBeaconRestUnsigned(timestampInput, "block.timestamp")
            val genesisTime = beaconRestGenesisTime()
            require(timestamp >= genesisTime) { "block.timestamp must not be before Beacon genesis time" }
            val elapsed = timestamp.subtract(genesisTime)
            val secondsPerSlot = BigInteger.valueOf(ETHEREUM_MAINNET_SECONDS_PER_SLOT)
            require(elapsed.mod(secondsPerSlot) == BigInteger.ZERO) {
                "block.timestamp must align to an Ethereum mainnet Beacon slot"
            }
            val slot = elapsed.divide(secondsPerSlot)
            require(slot > BigInteger.ZERO) { "beaconFinality.beaconSlot must be positive" }
            return EthereumBeaconRestBlockId(slot.toString(), slot = slot)
        }
        return EthereumBeaconRestBlockId("finalized")
    }

    private fun beaconRestGenesisTime(): BigInteger {
        val genesis = fetchJsonObject(
            "/eth/v1/beacon/genesis",
            "Ethereum mainnet Beacon REST genesis",
        )
        val data = expectBeaconRestObject(
            requireBeaconRestField(genesis, "Ethereum mainnet Beacon REST genesis", "data"),
            "Ethereum mainnet Beacon REST genesis.data",
        )
        return normalizeEthereumBeaconRestUnsigned(
            requireBeaconRestField(
                data,
                "Ethereum mainnet Beacon REST genesis.data",
                "genesis_time",
            ),
            "Ethereum mainnet Beacon REST genesis.data.genesis_time",
        )
    }

    private fun fetchJsonObject(path: String, label: String): Map<String, Any?> {
        val response = transport.get(beaconRestUrl(endpoint, path), headers).snapshot()
        require(response.statusCode in 200..299) {
            val suffix = response.statusMessage?.let { " $it" } ?: ""
            "$label request failed ${response.statusCode}$suffix"
        }
        require(response.body.size <= ETHEREUM_MAINNET_BEACON_REST_MAX_RESPONSE_BYTES) {
            "$label response body must be at most $ETHEREUM_MAINNET_BEACON_REST_MAX_RESPONSE_BYTES bytes"
        }
        val parsed = JsonParser.parse(String(response.body, Charsets.UTF_8))
        return expectBeaconRestObject(parsed, "$label response JSON")
    }
}

private fun firstPresent(input: Map<String, Any?>, vararg keys: String): Any? {
    for (key in keys) {
        if (input.containsKey(key)) return input[key]
    }
    return null
}

private fun strictFirstPresent(input: Map<String, Any?>, label: String, vararg keys: String): Any? {
    var selected: Any? = null
    var found = false
    for (key in keys) {
        if (input.containsKey(key)) {
            require(!found) { "$label must not use multiple aliases" }
            selected = input[key]
            found = true
        }
    }
    return selected
}

private fun ethereumBeaconRestHeaderSummary(
    payload: Map<String, Any?>,
    label: String,
): EthereumBeaconRestHeaderSummary {
    rejectUnsafeBeaconRestPayload(payload, label)
    val headerData = expectBeaconRestObject(
        requireBeaconRestField(payload, label, "data"),
        "$label.data",
    )
    rejectNonBooleanBeaconRestCanonical(headerData, label)
    val root = normalizeEthereumBeaconRestHex(
        requireBeaconRestField(headerData, "$label.data", "root"),
        if (label.contains("target")) "targetHeaderRoot" else "finalizedHeaderRoot",
        32,
    )
    val header = expectBeaconRestObject(
        requireBeaconRestField(headerData, "$label.data", "header"),
        "$label.data.header",
    )
    val message = expectBeaconRestObject(
        requireBeaconRestField(header, "$label.data.header", "message"),
        "$label.data.header.message",
    )
    for (field in listOf("parent_root", "state_root", "body_root")) {
        normalizeEthereumBeaconRestHex(
            requireBeaconRestField(message, "$label.data.header.message", field),
            "$label.data.header.message.$field",
            32,
        )
    }
    normalizeEthereumBeaconRestHex(
        requireBeaconRestField(header, "$label.data.header", "signature"),
        "$label.data.header.signature",
        96,
    )
    val slot = normalizeEthereumBeaconRestSlot(
        requireBeaconRestField(message, "$label.data.header.message", "slot"),
        "beaconFinality.beaconSlot",
    )
    return EthereumBeaconRestHeaderSummary(root, slot)
}

private fun ethereumBeaconRestFinalityUpdateSummary(
    payload: Map<String, Any?>,
    expectedFinalizedSlot: BigInteger,
    expectedFinalizedRoot: String,
): EthereumBeaconRestFinalityUpdateSummary {
    val label = "Ethereum mainnet Beacon REST light-client finality update"
    rejectUnsafeBeaconRestPayload(payload, label)
    val data = expectBeaconRestObject(
        requireBeaconRestField(payload, label, "data"),
        "$label.data",
    )
    val finalizedHeader = expectBeaconRestObject(
        requireBeaconRestField(data, "$label.data", "finalized_header"),
        "$label.data.finalized_header",
    )
    val finalizedBeacon = expectBeaconRestObject(
        requireBeaconRestField(finalizedHeader, "$label.data.finalized_header", "beacon"),
        "$label.data.finalized_header.beacon",
    )
    val finalizedSlot = normalizeEthereumBeaconRestSlot(
        requireBeaconRestField(finalizedBeacon, "$label.data.finalized_header.beacon", "slot"),
        "$label.data.finalized_header.beacon.slot",
    )
    require(finalizedSlot == expectedFinalizedSlot) {
        "Ethereum mainnet Beacon REST finality update finalized_header slot must match finalized header slot"
    }
    val finalizedHeaderRoot = SccpSourceProofs.ethBeaconBlockHeaderRoot(
        beaconSlot = finalizedSlot.toString(),
        beaconProposerIndex = normalizeEthereumBeaconRestUnsigned(
            requireBeaconRestField(finalizedBeacon, "$label.data.finalized_header.beacon", "proposer_index"),
            "$label.data.finalized_header.beacon.proposer_index",
        ).toString(),
        beaconParentRoot = normalizeEthereumBeaconRestHex(
            requireBeaconRestField(finalizedBeacon, "$label.data.finalized_header.beacon", "parent_root"),
            "$label.data.finalized_header.beacon.parent_root",
            32,
        ),
        beaconStateRoot = normalizeEthereumBeaconRestHex(
            requireBeaconRestField(finalizedBeacon, "$label.data.finalized_header.beacon", "state_root"),
            "$label.data.finalized_header.beacon.state_root",
            32,
        ),
        beaconBodyRoot = normalizeEthereumBeaconRestHex(
            requireBeaconRestField(finalizedBeacon, "$label.data.finalized_header.beacon", "body_root"),
            "$label.data.finalized_header.beacon.body_root",
            32,
        ),
    )
    require(finalizedHeaderRoot == expectedFinalizedRoot) {
        "Ethereum mainnet Beacon REST finality update finalized_header root must match finalized header root"
    }
    val syncSignatureSlot = normalizeEthereumBeaconRestSlot(
        requireBeaconRestField(data, "$label.data", "signature_slot"),
        "$label.data.signature_slot",
    )
    require(syncSignatureSlot >= expectedFinalizedSlot) {
        "Ethereum mainnet Beacon REST finality update signature_slot must cover finalized header slot"
    }
    val syncAggregate = expectBeaconRestObject(
        requireBeaconRestField(data, "$label.data", "sync_aggregate"),
        "$label.data.sync_aggregate",
    )
    val syncCommitteeBits = normalizeEthereumBeaconRestSyncCommitteeBits(
        requireBeaconRestField(syncAggregate, "$label.data.sync_aggregate", "sync_committee_bits"),
        "$label.data.sync_aggregate.sync_committee_bits",
    )
    val finalityBranch = normalizeEthereumBeaconRestFinalityBranch(
        requireBeaconRestField(data, "$label.data", "finality_branch"),
        "$label.data.finality_branch",
    )
    val syncCommitteeSignature = normalizeEthereumBeaconRestHex(
        requireBeaconRestField(syncAggregate, "$label.data.sync_aggregate", "sync_committee_signature"),
        "$label.data.sync_aggregate.sync_committee_signature",
        96,
    )
    return EthereumBeaconRestFinalityUpdateSummary(
        finalizedHeaderRoot = finalizedHeaderRoot,
        beaconSlot = finalizedSlot,
        finalityBranch = finalityBranch,
        syncCommitteeBits = syncCommitteeBits,
        syncCommitteeSignature = syncCommitteeSignature,
        syncCommitteeParticipation = ethereumBeaconRestSyncCommitteeParticipation(syncCommitteeBits),
        syncSignatureSlot = syncSignatureSlot,
    )
}

private fun ethereumBeaconRestBlockIdFromValue(value: Any?, label: String): EthereumBeaconRestBlockId {
    if (value is String && value.trim() == value && value.startsWith("0x") && value.length == 66) {
        val root = normalizeEthereumBeaconRestHex(value, label, 32)
        return EthereumBeaconRestBlockId(root, root = root)
    }
    val slot = normalizeEthereumBeaconRestSlot(value, label)
    return EthereumBeaconRestBlockId(slot.toString(), slot = slot)
}

private fun normalizeEthereumBeaconRestSlot(value: Any?, label: String): BigInteger {
    val slot = normalizeEthereumBeaconRestUnsigned(value, label)
    require(slot > BigInteger.ZERO) { "beaconFinality.beaconSlot must be positive" }
    return slot
}

private fun normalizeEthereumBeaconRestEndpoint(endpoint: String): String {
    require(endpoint.trim() == endpoint && endpoint.isNotEmpty()) {
        "Ethereum mainnet Beacon REST endpoint must be a non-empty URL"
    }
    val url = URL(endpoint)
    require(url.protocol == "http" || url.protocol == "https") {
        "Ethereum mainnet Beacon REST endpoint must use http or https"
    }
    return endpoint.substringBefore("#")
}

private fun beaconRestUrl(endpoint: String, path: String): String {
    val url = URL(endpoint)
    val basePath = url.path.trimEnd('/')
    val apiPath = if (Regex("/eth/v[0-9]+$").containsMatchIn(basePath) &&
        Regex("^/eth/v[0-9]+/").containsMatchIn(path)
    ) {
        Regex("/eth/v[0-9]+$").replace(basePath, "") + path
    } else {
        basePath + path
    }
    val query = url.query?.let { "?$it" } ?: ""
    return "${url.protocol}://${url.authority}$apiPath$query"
}

private fun expectBeaconRestObject(value: Any?, label: String): Map<String, Any?> {
    @Suppress("UNCHECKED_CAST")
    return value as? Map<String, Any?>
        ?: throw IllegalArgumentException("$label must be an object")
}

private fun requireBeaconRestField(value: Map<String, Any?>, label: String, field: String): Any? {
    require(value.containsKey(field)) { "$label.$field is required" }
    return value[field]
}

private fun rejectUnsafeBeaconRestPayload(payload: Map<String, Any?>, label: String) {
    val executionOptimistic = optionalBeaconRestBoolean(payload, "execution_optimistic", label)
    val executionOptimisticAlias = optionalBeaconRestBoolean(payload, "executionOptimistic", label)
    val finalized = optionalBeaconRestBoolean(payload, "finalized", label)
    require(executionOptimistic != true && executionOptimisticAlias != true) {
        "$label must not be execution optimistic"
    }
    require(finalized != false) {
        "$label must be finalized"
    }
}

private fun rejectNonBooleanBeaconRestCanonical(payload: Map<String, Any?>, label: String) {
    val canonical = optionalBeaconRestBoolean(payload, "canonical", label)
    require(canonical != false) { "$label must be canonical" }
}

private fun optionalBeaconRestBoolean(payload: Map<String, Any?>, field: String, label: String): Boolean? {
    if (!payload.containsKey(field)) {
        return null
    }
    val value = payload[field]
    require(value is Boolean) { "$label.$field must be a boolean" }
    return value
}

private fun normalizeEthereumBeaconRestHex(value: Any?, label: String, byteLength: Int): String {
    require(value is String && value.trim() == value && value.startsWith("0x")) {
        "$label must be canonical lowercase 0x hex"
    }
    val text = value.substring(2)
    require(text.length == byteLength * 2 && text.matches(Regex("[0-9a-f]+"))) {
        "$label must be $byteLength bytes canonical lowercase 0x hex"
    }
    require(text.any { it != '0' }) { "$label must not be zero" }
    return value
}

private fun normalizeEthereumBeaconRestSyncCommitteeBits(value: Any?, label: String): String {
    val bits = normalizeEthereumBeaconRestHex(value, label, 64, allowZero = true)
    val participation = ethereumBeaconRestSyncCommitteeParticipation(bits)
    require(participation > BigInteger.ZERO) {
        "$label must contain at least one participant"
    }
    require(participation.multiply(BigInteger.valueOf(3)) >= BigInteger.valueOf(512L * 2L)) {
        "$label must contain Ethereum sync committee supermajority"
    }
    return bits
}

private fun normalizeEthereumBeaconRestFinalityBranch(value: Any?, label: String): List<String> {
    require(value is List<*>) { "$label must be an array" }
    require(value.size == 6) { "$label must contain 6 siblings" }
    return value.mapIndexed { index, sibling ->
        normalizeEthereumBeaconRestHex(sibling, "$label[$index]", 32, allowZero = true)
    }
}

private fun normalizeEthereumBeaconRestHex(
    value: Any?,
    label: String,
    byteLength: Int,
    allowZero: Boolean,
): String {
    require(value is String && value.trim() == value && value.startsWith("0x")) {
        "$label must be canonical lowercase 0x hex"
    }
    val text = value.substring(2)
    require(text.length == byteLength * 2 && text.matches(Regex("[0-9a-f]+"))) {
        "$label must be $byteLength bytes canonical lowercase 0x hex"
    }
    require(allowZero || text.any { it != '0' }) { "$label must not be zero" }
    return value
}

private fun ethereumBeaconRestSyncCommitteeParticipation(bits: String): BigInteger {
    val text = bits.removePrefix("0x")
    var count = BigInteger.ZERO
    var index = 0
    while (index < text.length) {
        var value = text.substring(index, index + 2).toInt(16)
        while (value != 0) {
            if ((value and 1) == 1) count = count.add(BigInteger.ONE)
            value = value ushr 1
        }
        index += 2
    }
    return count
}

private fun normalizeEthereumBeaconRestQuantity(value: Any?, label: String): String {
    require(value is String && value.trim() == value && value.startsWith("0x")) {
        "$label must be a canonical JSON-RPC quantity"
    }
    val text = value.substring(2)
    require(text.isNotEmpty() && text.matches(Regex("0|[1-9a-f][0-9a-f]*"))) {
        "$label must be a canonical JSON-RPC quantity"
    }
    return "0x" + BigInteger(text, 16).toString(16)
}

private fun normalizeEthereumBeaconRestUnsigned(value: Any?, label: String): BigInteger =
    when (value) {
        is String -> {
            require(value.trim() == value) { "$label must be canonical" }
            if (value.startsWith("0x")) {
                BigInteger(normalizeEthereumBeaconRestQuantity(value, label).substring(2), 16)
            } else {
                require(value.matches(Regex("0|[1-9][0-9]*"))) {
                    "$label must be a canonical decimal integer"
                }
                BigInteger(value, 10)
            }
        }
        is BigInteger -> {
            require(value.signum() >= 0) { "$label must be non-negative" }
            value
        }
        is Long -> {
            require(value >= 0) { "$label must be non-negative" }
            BigInteger.valueOf(value)
        }
        is Int -> {
            require(value >= 0) { "$label must be non-negative" }
            BigInteger.valueOf(value.toLong())
        }
        is Short -> {
            require(value >= 0) { "$label must be non-negative" }
            BigInteger.valueOf(value.toLong())
        }
        is Byte -> {
            require(value >= 0) { "$label must be non-negative" }
            BigInteger.valueOf(value.toLong())
        }
        else -> throw IllegalArgumentException("$label must be an unsigned integer")
    }

private fun resolveEthereumBeaconRestSyncCommitteeRoot(
    syncCommitteeRoot: String?,
    syncCommitteePayload: ByteArray?,
): String {
    val payloadRoot: String? = if (syncCommitteePayload == null) {
        null
    } else {
        SccpSourceProofs.ethSyncCommitteeHashFromPayload(syncCommitteePayload.copyOf())
    }
    if (syncCommitteeRoot != null) {
        val normalizedRoot = normalizeEthereumBeaconRestHex(syncCommitteeRoot, "syncCommitteeRoot", 32)
        require(payloadRoot == null || payloadRoot == normalizedRoot) {
            "syncCommitteeRoot must match syncCommitteePayload"
        }
        return normalizedRoot
    }
    return payloadRoot
        ?: throw IllegalArgumentException(
            "Ethereum mainnet Beacon REST provider requires syncCommitteeRoot or syncCommitteePayload",
        )
}

/** Typed Ethereum beacon finality evidence required before inbound source proving. */
data class EthereumMainnetBeaconFinalityEvidence(
    val executionBlockNumber: String,
    val executionBlockHash: String,
    val executionReceiptsRoot: String,
    val additionalFields: Map<String, Any?> = emptyMap(),
    val beaconSlot: String? = null,
    val syncCommitteeBits: String? = null,
    val syncCommitteeSignature: String? = null,
    val syncCommitteeParticipation: String? = null,
    val syncSignatureSlot: String? = null,
) {
    fun toMap(): Map<String, Any?> =
        additionalFields + listOfNotNull(
            "executionBlockNumber" to executionBlockNumber,
            "executionBlockHash" to executionBlockHash,
            "executionReceiptsRoot" to executionReceiptsRoot,
            beaconSlot?.let { "beaconSlot" to it },
            syncCommitteeBits?.let { "syncCommitteeBits" to it },
            syncCommitteeSignature?.let { "syncCommitteeSignature" to it },
            syncCommitteeParticipation?.let { "syncCommitteeParticipation" to it },
            syncSignatureSlot?.let { "syncSignatureSlot" to it },
        ).toMap()
}

/** Ethereum mainnet receipt-proof transcript collected from app-supplied providers. */
data class EthereumMainnetReceiptProof(
    val sourceEventDigest: String,
    val beaconSlot: String,
    val executionBlockNumber: String,
    val executionBlockHash: String,
    val executionReceiptsRoot: String,
    val beaconFinalizedRoot: String,
    val syncCommitteeRoot: String,
    val receiptRootIndex: String,
    val receiptTrieProofNodes: List<ByteArray>,
    val inclusionBranch: List<ByteArray>,
    val sourceDomain: Int = SccpEvm.DOMAIN_ETH,
) {
    fun snapshot(): EthereumMainnetReceiptProof =
        copy(
            receiptTrieProofNodes = receiptTrieProofNodes.map { it.copyOf() },
            inclusionBranch = inclusionBranch.map { it.copyOf() },
        )
}

/** Local Ethereum mainnet inbound source prover linked by the application bundle. */
fun interface EthereumMainnetInboundProver {
    fun prove(evidence: EthereumMainnetInboundEvidence): ByteArray
}

/** App-supplied Torii submitter for locally generated Ethereum inbound proofs. */
fun interface EthereumMainnetInboundSubmitter {
    fun submit(proofBytes: ByteArray): Any?
}

/** App-supplied Ethereum transaction submitter for locally generated outbound proof calldata. */
fun interface EthereumMainnetOutboundSubmitter {
    fun submit(submission: EvmSccpSubmission): Any?
}

/** App-linked native prover self-test runner for Ethereum mainnet outbound proofs. */
fun interface EthereumMainnetNativeProverSelfTest {
    fun run(
        fixture: SccpEvm.EthereumMainnetNativeEvmProverSelfTestFixture,
        expectedResult: SccpEvm.EthereumMainnetNativeEvmProverSelfTestSdkResult,
        artifacts: SccpEvm.EthereumMainnetNativeEvmProverArtifacts,
    ): SccpEvm.EthereumMainnetNativeEvmProverSelfTestSdkResult
}

/** Locally collected Ethereum mainnet inbound evidence before source-proof generation. */
data class EthereumMainnetInboundEvidence(
    val sourceDomain: Int = SccpEvm.DOMAIN_ETH,
    val targetDomain: Int = SccpEvm.DOMAIN_SORA,
    val transactionHash: String? = null,
    val receipt: Map<String, Any?>? = null,
    val block: Map<String, Any?>? = null,
    val beaconFinality: Map<String, Any?>? = null,
    val receiptProof: EthereumMainnetReceiptProof? = null,
    val receiptProofHash: String? = null,
    val sourceEventDigest: String? = null,
    val sourceBridgeEmitterAddress: String? = null,
    val blockReceipts: List<Map<String, Any?>>? = null,
    val inclusionBranch: List<ByteArray>? = null,
) {
    companion object {
        fun withBeaconFinalityEvidence(
            sourceDomain: Int = SccpEvm.DOMAIN_ETH,
            targetDomain: Int = SccpEvm.DOMAIN_SORA,
            transactionHash: String? = null,
            receipt: Map<String, Any?>? = null,
            block: Map<String, Any?>? = null,
            beaconFinalityEvidence: EthereumMainnetBeaconFinalityEvidence? = null,
            receiptProof: EthereumMainnetReceiptProof? = null,
            receiptProofHash: String? = null,
            sourceEventDigest: String? = null,
            sourceBridgeEmitterAddress: String? = null,
            blockReceipts: List<Map<String, Any?>>? = null,
            inclusionBranch: List<ByteArray>? = null,
        ): EthereumMainnetInboundEvidence =
            EthereumMainnetInboundEvidence(
                sourceDomain = sourceDomain,
                targetDomain = targetDomain,
                transactionHash = transactionHash,
                receipt = receipt,
                block = block,
                beaconFinality = beaconFinalityEvidence?.toMap(),
                receiptProof = receiptProof,
                receiptProofHash = receiptProofHash,
                sourceEventDigest = sourceEventDigest,
                sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
                blockReceipts = blockReceipts,
                inclusionBranch = inclusionBranch,
            )
    }
}

/** App-supplied BSC JSON-RPC execution provider for native SCCP evidence collection. */
fun interface BscMainnetExecutionProvider {
    fun request(method: String, params: List<Any?>): Any?
}

/** App-supplied BSC Parlia finality collector for native SCCP evidence collection. */
fun interface BscMainnetConsensusProvider {
    fun collectFinalityEvidence(
        receipt: Map<String, Any?>?,
        block: Map<String, Any?>?,
        transactionHash: String?,
    ): Map<String, Any?>
}

/** Typed BSC Parlia finality evidence required before inbound source proving. */
data class BscMainnetParliaFinalityEvidence(
    val executionBlockNumber: String,
    val executionBlockHash: String,
    val executionReceiptsRoot: String,
    val additionalFields: Map<String, Any?> = emptyMap(),
) {
    fun toMap(): Map<String, Any?> =
        additionalFields + mapOf(
            "executionBlockNumber" to executionBlockNumber,
            "executionBlockHash" to executionBlockHash,
            "executionReceiptsRoot" to executionReceiptsRoot,
        )
}

/** Local BSC mainnet inbound source prover linked by the application bundle. */
fun interface BscMainnetInboundProver {
    fun prove(evidence: BscMainnetInboundEvidence): ByteArray
}

/** App-supplied Torii submitter for locally generated BSC inbound proofs. */
fun interface BscMainnetInboundSubmitter {
    fun submit(proofBytes: ByteArray): Any?
}

/** App-supplied BSC transaction submitter for locally generated outbound proof calldata. */
fun interface BscMainnetOutboundSubmitter {
    fun submit(submission: EvmSccpSubmission): Any?
}

/** BSC mainnet receipt-proof transcript collected from app-supplied providers. */
data class BscMainnetReceiptProof(
    val sourceEventDigest: String,
    val validatorEpoch: String,
    val blockNumber: String,
    val blockHash: String,
    val receiptsRoot: String,
    val validatorSetHash: String,
    val commitSealHash: String,
    val receiptRootIndex: String,
    val receiptTrieProofNodes: List<ByteArray>,
    val inclusionBranch: List<ByteArray>,
    val sourceDomain: Int = SccpEvm.DOMAIN_BSC,
) {
    fun snapshot(): BscMainnetReceiptProof =
        copy(
            receiptTrieProofNodes = receiptTrieProofNodes.map { it.copyOf() },
            inclusionBranch = inclusionBranch.map { it.copyOf() },
        )
}

/** Locally collected BSC mainnet inbound evidence before source-proof generation. */
data class BscMainnetInboundEvidence(
    val sourceDomain: Int = SccpEvm.DOMAIN_BSC,
    val targetDomain: Int = SccpEvm.DOMAIN_SORA,
    val transactionHash: String? = null,
    val receipt: Map<String, Any?>? = null,
    val block: Map<String, Any?>? = null,
    val parliaFinality: Map<String, Any?>? = null,
    val receiptProofHash: String? = null,
    val receiptProof: BscMainnetReceiptProof? = null,
    val sourceEventDigest: String? = null,
    val sourceBridgeEmitterAddress: String? = null,
) {
    companion object {
        fun withParliaFinalityEvidence(
            sourceDomain: Int = SccpEvm.DOMAIN_BSC,
            targetDomain: Int = SccpEvm.DOMAIN_SORA,
            transactionHash: String? = null,
            receipt: Map<String, Any?>? = null,
            block: Map<String, Any?>? = null,
            parliaFinalityEvidence: BscMainnetParliaFinalityEvidence? = null,
            receiptProofHash: String? = null,
            receiptProof: BscMainnetReceiptProof? = null,
            sourceEventDigest: String? = null,
            sourceBridgeEmitterAddress: String? = null,
        ): BscMainnetInboundEvidence =
            BscMainnetInboundEvidence(
                sourceDomain = sourceDomain,
                targetDomain = targetDomain,
                transactionHash = transactionHash,
                receipt = receipt,
                block = block,
                parliaFinality = parliaFinalityEvidence?.toMap(),
                receiptProofHash = receiptProofHash,
                receiptProof = receiptProof,
                sourceEventDigest = sourceEventDigest,
                sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
            )
    }
}

/** Local-first EVM-family SCCP Groth16 proof wrapper for UI SDKs. */
class EvmSccpProver(
    private val witnessProvider: EvmSccpWitnessProvider? = null,
    private val proofEngine: EvmSccpProofEngine? = null,
) {
    fun buildRequest(input: EvmSccpProofRequestInput): EvmSccpProofRequest =
        SccpEvm.buildProofRequest(witnessProvider?.resolveWitness(witnessProviderInputSnapshot(input)) ?: input)

    fun prove(input: EvmSccpProofRequestInput): EvmSccpProofResult {
        val request = buildRequest(input)
        val engine = proofEngine ?: throw IllegalStateException("EVM-family SCCP Groth16 prover is not linked")
        SccpEvm.requireProductionProofRequest(request)
        return SccpEvm.wrapProofResult(engine.prove(SccpEvm.callbackRequestSnapshot(request)), request)
    }

    private fun witnessProviderInputSnapshot(input: EvmSccpProofRequestInput): EvmSccpProofRequestInput =
        input.copy(
            bundleBytes = input.bundleBytes.copyOf(),
            sourceProofBytes = input.sourceProofBytes.copyOf(),
        )
}

/** Local-first Ethereum mainnet SCCP Groth16 proof wrapper for UI SDKs. */
class EthereumMainnetSccp(
    private val witnessProvider: EvmSccpWitnessProvider? = null,
    private val proofEngine: EvmSccpProofEngine? = null,
    private val executionProvider: EthereumMainnetExecutionProvider? = null,
    private val consensusProvider: EthereumMainnetConsensusProvider? = null,
    private val inboundProver: EthereumMainnetInboundProver? = null,
    private val inboundSubmitter: EthereumMainnetInboundSubmitter? = null,
    private val outboundSubmitter: EthereumMainnetOutboundSubmitter? = null,
    private val nativeProverSelfTest: EthereumMainnetNativeProverSelfTest? = null,
    private val nativeProverBundle: SccpEvm.EthereumMainnetNativeEvmProverBundle? = null,
    private val nativeProverArtifacts: SccpEvm.EthereumMainnetNativeEvmProverArtifacts? = null,
    private val sourceBridgeEmitterAddress: String? = null,
) {
    private val effectiveNativeProverBundle =
        nativeProverArtifacts?.nativeProverBundle ?: nativeProverBundle

    companion object {
        fun fromNativeProverBundle(
            witnessProvider: EvmSccpWitnessProvider? = null,
            proofEngine: EvmSccpProofEngine? = null,
            executionProvider: EthereumMainnetExecutionProvider? = null,
            consensusProvider: EthereumMainnetConsensusProvider? = null,
            inboundProver: EthereumMainnetInboundProver? = null,
            inboundSubmitter: EthereumMainnetInboundSubmitter? = null,
            outboundSubmitter: EthereumMainnetOutboundSubmitter? = null,
            nativeProverSelfTest: EthereumMainnetNativeProverSelfTest? = null,
            nativeProverBundle: SccpEvm.EthereumMainnetNativeEvmProverBundle,
            sdk: String,
            artifactResolver: SccpEvm.NativeEvmProverArtifactResolver,
            sourceBridgeEmitterAddress: String? = null,
        ): EthereumMainnetSccp {
            val verifiedArtifacts = nativeProverBundle.verifiedArtifacts(sdk, artifactResolver)
            return EthereumMainnetSccp(
                witnessProvider = witnessProvider,
                proofEngine = proofEngine,
                executionProvider = executionProvider,
                consensusProvider = consensusProvider,
                inboundProver = inboundProver,
                inboundSubmitter = inboundSubmitter,
                outboundSubmitter = outboundSubmitter,
                nativeProverSelfTest = nativeProverSelfTest,
                nativeProverBundle = verifiedArtifacts.nativeProverBundle,
                nativeProverArtifacts = verifiedArtifacts,
                sourceBridgeEmitterAddress = sourceBridgeEmitterAddress,
            )
        }
    }

    fun validateExecutionProviderMainnet(provider: EthereumMainnetExecutionProvider? = executionProvider): Any? {
        val selectedProvider = provider
            ?: throw IllegalStateException("Ethereum mainnet execution provider is not linked")
        val chainId = selectedProvider.request("eth_chainId", emptyList())
        SccpEthereumMainnet.requireMainnetChainId(normalizeRpcChainId(chainId))
        return chainId
    }

    fun collectInboundEvidenceFromReceipt(
        input: EthereumMainnetInboundEvidence,
        provider: EthereumMainnetExecutionProvider? = executionProvider,
        consensusProvider: EthereumMainnetConsensusProvider? = this.consensusProvider,
    ): EthereumMainnetInboundEvidence {
        require(input.sourceDomain == SccpEvm.DOMAIN_ETH) {
            "Ethereum mainnet inbound evidence sourceDomain must be ETH"
        }
        require(input.targetDomain == SccpEvm.DOMAIN_SORA) {
            "Ethereum mainnet inbound evidence targetDomain must be SORA"
        }
        provider?.let { validateExecutionProviderMainnet(it) }

        var transactionHash = input.transactionHash?.let {
            normalizeRpcHex(it, "transactionHash", 32)
        }
        var receipt = input.receipt
        if (receipt == null && transactionHash != null) {
            val selectedProvider = provider
                ?: throw IllegalStateException(
                    "Ethereum mainnet execution provider is not linked for transactionHash evidence collection",
                )
            receipt = requireMap(
                selectedProvider.request("eth_getTransactionReceipt", listOf(transactionHash)),
                "eth_getTransactionReceipt",
            )
        }
        var receiptProof = input.receiptProof?.snapshot()
        if (receipt == null && receiptProof == null && input.receiptProofHash == null) {
            throw IllegalArgumentException(
                "Ethereum mainnet inbound evidence requires receipt, receiptProof, receiptProofHash, or transactionHash",
            )
        }

        var blockHash: String? = null
        var receiptBlockNumber: String? = null
        var blockReceiptsRoot: String? = null
        if (receipt != null) {
            require(receipt["status"] == "0x1") {
                "Ethereum mainnet inbound receipt status must be 0x1"
            }
            val receiptTransactionHash = normalizeRpcHex(
                strictFirstPresent(
                    receipt,
                    "receipt.transactionHash",
                    "transactionHash",
                    "transaction_hash",
                ),
                "receipt.transactionHash",
                32,
            )
            if (transactionHash != null) {
                require(receiptTransactionHash == transactionHash) {
                    "receipt.transactionHash must match transactionHash"
                }
            }
            transactionHash = receiptTransactionHash
            blockHash = normalizeRpcHex(
                strictFirstPresent(receipt, "receipt.blockHash", "blockHash", "block_hash"),
                "receipt.blockHash",
                32,
            )
            receiptBlockNumber = normalizePositiveRpcQuantity(
                strictFirstPresent(receipt, "receipt.blockNumber", "blockNumber", "block_number"),
                "receipt.blockNumber",
            )
        }

        var block = input.block
        if (block == null && blockHash != null && provider != null) {
            block = requireMap(
                provider.request("eth_getBlockByHash", listOf(blockHash, false)),
                "eth_getBlockByHash",
            )
        }
        if (block != null) {
            val normalizedBlockHash = normalizeRpcHex(block["hash"], "block.hash", 32)
            if (blockHash != null) {
                require(normalizedBlockHash == blockHash) {
                    "block.hash must match receipt.blockHash"
                }
            }
            val blockNumber = normalizePositiveRpcQuantity(
                strictFirstPresent(block, "block.number", "number", "blockNumber", "block_number"),
                "block.number",
            )
            if (receiptBlockNumber != null) {
                require(blockNumber == receiptBlockNumber) {
                    "block.number must match receipt.blockNumber"
                }
            }
            receiptBlockNumber = blockNumber
            blockReceiptsRoot = normalizeRpcHex(
                strictFirstPresent(block, "block.receiptsRoot", "receiptsRoot", "receipts_root"),
                "block.receiptsRoot",
                32,
            )
        }

        val sourceEvent = ethereumReceiptSourceEvent(
            receipt = receipt,
            sourceEventDigestInput = input.sourceEventDigest,
            sourceBridgeEmitterAddressInput = if (
                receipt == null &&
                input.sourceEventDigest == null &&
                input.sourceBridgeEmitterAddress == null
            ) {
                null
            } else {
                resolveSourceBridgeEmitterAddress(
                    input.sourceBridgeEmitterAddress,
                    sourceBridgeEmitterAddress,
                )
            },
            transactionHash = transactionHash,
            blockHash = blockHash,
            blockNumber = receiptBlockNumber,
        )
        receipt = receipt?.let { sccpCallbackMapSnapshot(it) }
        block = block?.let { sccpCallbackMapSnapshot(it) }
        val beaconFinality = input.beaconFinality
            ?: consensusProvider?.collectFinalityEvidence(receipt, block, transactionHash)
        val normalizedBeaconFinality = beaconFinality?.let {
            normalizeBeaconFinality(
                it,
                expectedBlockHash = blockHash,
                expectedBlockNumber = receiptBlockNumber,
                expectedReceiptsRoot = blockReceiptsRoot,
            )
        }
        var blockReceipts = input.blockReceipts
        if (
            receiptProof == null &&
            receipt != null &&
            normalizedBeaconFinality != null &&
            sourceEvent.first != null &&
            input.inclusionBranch != null
        ) {
            val selectedProvider = provider
            if (blockReceipts == null) {
                val receiptsProvider = selectedProvider
                    ?: throw IllegalStateException(
                        "Ethereum mainnet execution provider is not linked for block receipt evidence collection",
                    )
                blockReceipts = requireMapList(
                    receiptsProvider.request("eth_getBlockReceipts", listOf(receiptBlockNumber)),
                    "eth_getBlockReceipts",
                )
            }
            val receiptTransactionIndex = strictFirstPresent(
                receipt,
                "receipt.transactionIndex",
                "transactionIndex",
                "transaction_index",
            )
            val receiptTrieProof = SccpSourceProofs.buildEvmReceiptTrieProofFromReceipts(
                blockReceipts,
                receiptTransactionIndex,
            )
            val expectedReceiptsRoot = blockReceiptsRoot ?: normalizedBeaconFinality["executionReceiptsRoot"] as? String
            require(receiptTrieProof.receiptsRoot == expectedReceiptsRoot) {
                "computed receipt trie root must match block.receiptsRoot"
            }
            val targetIndex = normalizeUnsignedInteger(receiptTransactionIndex, "receipt.transactionIndex")
            require(targetIndex < blockReceipts.size.toLong()) {
                "receipt.transactionIndex must select an eth_getBlockReceipts entry"
            }
            val indexedReceipt = blockReceipts[targetIndex.toInt()]
            val indexedTransactionHash = normalizeRpcHex(
                strictFirstPresent(
                    indexedReceipt,
                    "blockReceipts.transactionHash",
                    "transactionHash",
                    "transaction_hash",
                ),
                "blockReceipts transactionHash",
                32,
            )
            require(indexedTransactionHash == transactionHash) {
                "eth_getBlockReceipts target receipt must match transactionHash"
            }
            val indexedBlockHash = normalizeRpcHex(
                strictFirstPresent(
                    indexedReceipt,
                    "blockReceipts.blockHash",
                    "blockHash",
                    "block_hash",
                ),
                "blockReceipts blockHash",
                32,
            )
            require(indexedBlockHash == blockHash) {
                "eth_getBlockReceipts target receipt blockHash must match receipt"
            }
            val indexedBlockNumber = normalizePositiveRpcQuantity(
                strictFirstPresent(
                    indexedReceipt,
                    "blockReceipts.blockNumber",
                    "blockNumber",
                    "block_number",
                ),
                "blockReceipts blockNumber",
            )
            require(indexedBlockNumber == receiptBlockNumber) {
                "eth_getBlockReceipts target receipt blockNumber must match receipt"
            }
            val receiptRlp = "0x" + hexLower(SccpSourceProofs.canonicalEvmReceiptRlp(receipt))
            require(receiptTrieProof.receiptRlp == receiptRlp) {
                "eth_getBlockReceipts target receipt RLP must match receipt"
            }
            val sourceEventDigest = sourceEvent.first
                ?: throw IllegalArgumentException("sourceEventDigest is required for receiptProof")
            val inclusionBranch = input.inclusionBranch
            receiptProof = EthereumMainnetReceiptProof(
                sourceEventDigest = sourceEventDigest,
                beaconSlot = normalizedBeaconFinality["beaconSlot"] as? String
                    ?: throw IllegalArgumentException("beaconFinality.beaconSlot is required for receiptProof"),
                executionBlockNumber = normalizedBeaconFinality["executionBlockNumber"] as String,
                executionBlockHash = normalizedBeaconFinality["executionBlockHash"] as String,
                executionReceiptsRoot = normalizedBeaconFinality["executionReceiptsRoot"] as String,
                beaconFinalizedRoot = normalizedBeaconFinality["finalizedHeaderRoot"] as? String
                    ?: throw IllegalArgumentException("beaconFinality.finalizedHeaderRoot is required for receiptProof"),
                syncCommitteeRoot = normalizedBeaconFinality["syncCommitteeRoot"] as? String
                    ?: throw IllegalArgumentException("beaconFinality.syncCommitteeRoot is required for receiptProof"),
                receiptRootIndex = targetIndex.toString(),
                receiptTrieProofNodes = receiptTrieProof.receiptTrieProofNodes,
                inclusionBranch = snapshotByteArrayList(inclusionBranch, "inclusionBranch"),
            )
        }
        requireReceiptProofMatchesEvidence(
            receiptProof = receiptProof,
            blockHash = blockHash,
            receiptBlockNumber = receiptBlockNumber,
            blockReceiptsRoot = blockReceiptsRoot,
            beaconFinality = normalizedBeaconFinality,
            sourceEventDigest = sourceEvent.first,
        )

        return inboundCallbackEvidenceSnapshot(input.copy(
            sourceDomain = SccpEvm.DOMAIN_ETH,
            targetDomain = SccpEvm.DOMAIN_SORA,
            transactionHash = transactionHash,
            receipt = receipt,
            block = block,
            beaconFinality = normalizedBeaconFinality,
            receiptProof = receiptProof,
            receiptProofHash = normalizeReceiptProofHash(receiptProof, input.receiptProofHash),
            sourceEventDigest = sourceEvent.first,
            sourceBridgeEmitterAddress = sourceEvent.second,
            blockReceipts = blockReceipts,
            inclusionBranch = input.inclusionBranch?.let { snapshotByteArrayList(it, "inclusionBranch") },
        ))
    }

    fun proveInboundToSora(
        input: EthereumMainnetInboundEvidence,
        provider: EthereumMainnetExecutionProvider? = executionProvider,
        consensusProvider: EthereumMainnetConsensusProvider? = this.consensusProvider,
    ): ByteArray {
        val prover = inboundProver
            ?: throw IllegalStateException("Ethereum mainnet SCCP inbound prover is not linked")
        val evidence = collectInboundEvidenceFromReceipt(input, provider, consensusProvider)
        require(evidence.beaconFinality != null) {
            "Ethereum mainnet SCCP inbound proof requires beaconFinality"
        }
        require(evidence.receiptProof != null) {
            "Ethereum mainnet SCCP inbound proof requires receiptProof"
        }
        require(evidence.sourceEventDigest != null) {
            "Ethereum mainnet SCCP inbound proof requires receipt source event validation"
        }
        require(evidence.beaconFinality["finalizedHeaderRoot"] != null) {
            "Ethereum mainnet SCCP inbound proof requires beaconFinality.finalizedHeaderRoot"
        }
        require(evidence.beaconFinality["syncCommitteeRoot"] != null) {
            "Ethereum mainnet SCCP inbound proof requires beaconFinality.syncCommitteeRoot"
        }
        require(evidence.beaconFinality["beaconSlot"] != null) {
            "Ethereum mainnet SCCP inbound proof requires beaconFinality.beaconSlot"
        }
        listOf(
            "finalityBranch",
            "syncCommitteeBits",
            "syncCommitteeSignature",
            "syncCommitteeParticipation",
            "syncSignatureSlot",
        ).forEach { field ->
            require(evidence.beaconFinality[field] != null) {
                "Ethereum mainnet SCCP inbound proof requires beaconFinality.$field"
            }
        }
        return requireInboundNativeRecursiveProofBytes(
            prover.prove(inboundCallbackEvidenceSnapshot(evidence)),
        )
    }

    fun submitInboundToIroha(proofBytes: ByteArray): Any? {
        val proofBytes = requireInboundNativeRecursiveProofBytes(proofBytes)
        val submitter = inboundSubmitter
            ?: throw IllegalStateException("Ethereum mainnet SCCP inbound submitter is not linked")
        return submitter.submit(proofBytes)
    }

    private fun requireInboundNativeRecursiveProofBytes(proofBytes: ByteArray): ByteArray {
        val copy = proofBytes.copyOf()
        require(copy.isNotEmpty()) { "proofBytes must not be empty" }
        require(copy.any { it.toInt() != 0 }) { "proofBytes must not be all zero" }
        require(copy.size <= SccpEvm.NATIVE_RECURSIVE_MAX_PROOF_BYTES) {
            "proofBytes must be at most ${SccpEvm.NATIVE_RECURSIVE_MAX_PROOF_BYTES} bytes"
        }
        return copy
    }

    fun buildLocalAdmissionSubmission(
        input: EthereumMainnetLocalAdmissionSubmissionInput,
    ): EthereumMainnetLocalAdmissionSubmission =
        SccpEthereumMainnet.buildLocalAdmissionSubmission(input)

    fun buildOutboundProofRequest(input: EvmSccpProofRequestInput): EvmSccpProofRequest {
        val resolved = witnessProvider?.resolveWitness(witnessProviderInputSnapshot(input)) ?: input
        val bound = effectiveNativeProverBundle?.applyTo(resolved) ?: resolved
        return SccpEthereumMainnet.buildProofRequest(bound)
    }

    fun proveOutboundToEthereum(input: EvmSccpProofRequestInput): EvmSccpProofResult {
        val request = buildOutboundProofRequest(input)
        requireVerifiedNativeProverArtifacts(nativeProverArtifacts, request)
        requireNativeProverSelfTest(nativeProverArtifacts, nativeProverSelfTest)
        val engine = proofEngine
            ?: throw IllegalStateException("Ethereum mainnet SCCP Groth16 prover is not linked")
        return SccpEthereumMainnet.wrapProofResult(
            engine.prove(SccpEvm.callbackRequestSnapshot(request)),
            request,
        )
    }

    fun runNativeProverSelfTest(
        artifacts: SccpEvm.EthereumMainnetNativeEvmProverArtifacts? = nativeProverArtifacts,
        nativeProverSelfTest: EthereumMainnetNativeProverSelfTest? = this.nativeProverSelfTest,
    ): SccpEvm.EthereumMainnetNativeEvmProverSelfTestSdkResult =
        requireNativeProverSelfTest(artifacts, nativeProverSelfTest)

    private fun requireVerifiedNativeProverArtifacts(
        artifacts: SccpEvm.EthereumMainnetNativeEvmProverArtifacts?,
        request: EvmSccpProofRequest,
    ) {
        require(artifacts != null) {
            "Ethereum mainnet SCCP outbound proof requires verified native EVM prover artifacts"
        }
        require(artifacts.nativeProverBundle.destinationBindingHash == request.destinationBindingHash) {
            "nativeProverArtifacts destinationBindingHash must match proof request"
        }
        require(
            artifacts.proofArtifactHash == request.proofArtifactHash &&
                artifacts.provingKeyHash == request.provingKeyHash,
        ) {
            "nativeProverArtifacts artifact hashes must match proof request"
        }
        require(artifacts.verifierKeyHash == artifacts.nativeProverBundle.verifierKeyHash) {
            "nativeProverArtifacts verifierKeyHash must match nativeProverBundle"
        }
        require(
            artifacts.crossSdkFixtureParityHash ==
                artifacts.nativeProverBundle.auditHashes["cross_sdk_fixture_parity"] &&
                artifacts.crossSdkFixtureParity?.destinationBindingHash ==
                artifacts.nativeProverBundle.destinationBindingHash,
        ) {
            "nativeProverArtifacts crossSdkFixtureParityHash must match nativeProverBundle"
        }
        require(
            artifacts.nativeProverSelfTestHash ==
                artifacts.nativeProverBundle.auditHashes["native_prover_self_test"] &&
                artifacts.nativeProverSelfTest?.destinationBindingHash ==
                artifacts.nativeProverBundle.destinationBindingHash,
        ) {
            "nativeProverArtifacts nativeProverSelfTestHash must match nativeProverBundle"
        }
        val sdk = artifacts.sdk
        val implementation = artifacts.implementation
        val implementationHash = artifacts.implementationHash
        require(!sdk.isNullOrEmpty() && !implementation.isNullOrEmpty() && !implementationHash.isNullOrEmpty()) {
            "nativeProverArtifacts must bind sdk implementation and implementationHash"
        }
        val artifact = artifacts.nativeProverBundle.nativeSdkArtifacts.firstOrNull { it.sdk == sdk }
            ?: throw IllegalArgumentException("nativeProverBundle has no artifact row for sdk: $sdk")
        require(implementation == artifact.implementation && implementationHash == artifact.implementationHash) {
            "nativeProverArtifacts implementation binding must match nativeProverBundle"
        }
    }

    private fun requireNativeProverSelfTest(
        artifacts: SccpEvm.EthereumMainnetNativeEvmProverArtifacts?,
        nativeProverSelfTest: EthereumMainnetNativeProverSelfTest?,
    ): SccpEvm.EthereumMainnetNativeEvmProverSelfTestSdkResult {
        require(artifacts != null) {
            "nativeProverArtifacts are required"
        }
        val sdk = artifacts.sdk
            ?: throw IllegalArgumentException("nativeProverArtifacts sdk is required")
        val fixture = artifacts.nativeProverSelfTest
            ?: throw IllegalArgumentException("nativeProverArtifacts nativeProverSelfTest is required")
        val expectedResult = fixture.sdkResults[sdk]
            ?: throw IllegalArgumentException("nativeProverSelfTest sdkResults must include $sdk")
        val runner = nativeProverSelfTest
            ?: throw IllegalArgumentException("nativeProverSelfTest runner is required")
        val result = runner.run(fixture, expectedResult, artifacts)
        require(result == expectedResult) {
            "nativeProverSelfTest result must match nativeProverBundle fixture"
        }
        return result
    }

    private fun requireVerifiedNativeProverArtifacts(
        artifacts: SccpEvm.EthereumMainnetNativeEvmProverArtifacts?,
        proofResult: EvmSccpProofResult,
    ) {
        require(artifacts != null) {
            "Ethereum mainnet SCCP submission requires verified native EVM prover artifacts"
        }
        require(artifacts.nativeProverBundle.destinationBindingHash == proofResult.destinationBindingHash) {
            "nativeProverArtifacts destinationBindingHash must match proofResult"
        }
        require(
            artifacts.proofArtifactHash == proofResult.proofArtifactHash &&
                artifacts.provingKeyHash == proofResult.provingKeyHash,
        ) {
            "nativeProverArtifacts artifact hashes must match proofResult"
        }
        require(artifacts.verifierKeyHash == artifacts.nativeProverBundle.verifierKeyHash) {
            "nativeProverArtifacts verifierKeyHash must match nativeProverBundle"
        }
        require(
            artifacts.crossSdkFixtureParityHash ==
                artifacts.nativeProverBundle.auditHashes["cross_sdk_fixture_parity"] &&
                artifacts.crossSdkFixtureParity?.destinationBindingHash ==
                artifacts.nativeProverBundle.destinationBindingHash,
        ) {
            "nativeProverArtifacts crossSdkFixtureParityHash must match nativeProverBundle"
        }
        require(
            artifacts.nativeProverSelfTestHash ==
                artifacts.nativeProverBundle.auditHashes["native_prover_self_test"] &&
                artifacts.nativeProverSelfTest?.destinationBindingHash ==
                artifacts.nativeProverBundle.destinationBindingHash,
        ) {
            "nativeProverArtifacts nativeProverSelfTestHash must match nativeProverBundle"
        }
        val sdk = artifacts.sdk
        val implementation = artifacts.implementation
        val implementationHash = artifacts.implementationHash
        require(!sdk.isNullOrEmpty() && !implementation.isNullOrEmpty() && !implementationHash.isNullOrEmpty()) {
            "nativeProverArtifacts must bind sdk implementation and implementationHash"
        }
        val artifact = artifacts.nativeProverBundle.nativeSdkArtifacts.firstOrNull { it.sdk == sdk }
            ?: throw IllegalArgumentException("nativeProverBundle has no artifact row for sdk: $sdk")
        require(implementation == artifact.implementation && implementationHash == artifact.implementationHash) {
            "nativeProverArtifacts implementation binding must match nativeProverBundle"
        }
    }

    fun buildEthereumCalldata(input: EvmSccpSubmissionInput): EvmSccpSubmission {
        val submission = SccpEthereumMainnet.buildSubmission(input)
        requireVerifiedNativeProverArtifacts(
            nativeProverArtifacts,
            input.proofResult
                ?: throw IllegalArgumentException(
                    "Ethereum mainnet submissions require a wrapped proofResult with destinationBinding",
                ),
        )
        return submission
    }

    fun submitOutboundToEthereum(input: EvmSccpSubmissionInput): Any? {
        val submitter = outboundSubmitter
            ?: throw IllegalStateException("Ethereum mainnet SCCP outbound submitter is not linked")
        executionProvider?.let { validateExecutionProviderMainnet(it) }
        return submitter.submit(buildEthereumCalldata(input))
    }

    fun requireExecutionMainnet(chainId: Long) =
        SccpEthereumMainnet.requireMainnetChainId(chainId)

    private fun witnessProviderInputSnapshot(input: EvmSccpProofRequestInput): EvmSccpProofRequestInput =
        input.copy(
            bundleBytes = input.bundleBytes.copyOf(),
            sourceProofBytes = input.sourceProofBytes.copyOf(),
        )

    private fun inboundCallbackEvidenceSnapshot(
        evidence: EthereumMainnetInboundEvidence,
    ): EthereumMainnetInboundEvidence =
        evidence.copy(
            receipt = evidence.receipt?.let { sccpCallbackMapSnapshot(it) },
            block = evidence.block?.let { sccpCallbackMapSnapshot(it) },
            beaconFinality = evidence.beaconFinality?.let { sccpCallbackMapSnapshot(it) },
            receiptProof = evidence.receiptProof?.snapshot(),
            blockReceipts = evidence.blockReceipts?.map { sccpCallbackMapSnapshot(it) },
            inclusionBranch = evidence.inclusionBranch?.let { snapshotByteArrayList(it, "inclusionBranch") },
        )

    private fun normalizeRpcChainId(value: Any?): Long {
        val quantity = normalizeRpcQuantity(value, "eth_chainId")
        val parsed = BigInteger(quantity.substring(2), 16)
        require(parsed.bitLength() <= 63) { "eth_chainId must fit positive i64" }
        return parsed.toLong()
    }

    private fun normalizeUnsignedInteger(value: Any?, label: String): Long =
        when (value) {
            is Long -> {
                require(value >= 0) { "$label must be non-negative" }
                value
            }
            is Int -> {
                require(value >= 0) { "$label must be non-negative" }
                value.toLong()
            }
            is Short -> {
                require(value >= 0) { "$label must be non-negative" }
                value.toLong()
            }
            is Byte -> {
                require(value >= 0) { "$label must be non-negative" }
                value.toLong()
            }
            is BigInteger -> {
                require(value.signum() >= 0 && value.bitLength() <= 63) {
                    "$label must fit positive i64"
                }
                value.toLong()
            }
            is String -> {
                require(value.trim() == value) { "$label must be canonical" }
                val parsed = if (value.startsWith("0x")) {
                    val text = value.substring(2)
                    require(text.isNotEmpty() && text.matches(Regex("0|[1-9a-f][0-9a-f]*"))) {
                        "$label must be a canonical JSON-RPC quantity"
                    }
                    BigInteger(text, 16)
                } else {
                    require(value.matches(Regex("0|[1-9][0-9]*"))) {
                        "$label must be a canonical decimal integer"
                    }
                    BigInteger(value, 10)
                }
                require(parsed.bitLength() <= 63) { "$label must fit positive i64" }
                parsed.toLong()
            }
            else -> throw IllegalArgumentException("$label must be a JSON-RPC quantity or integer")
        }

    private fun requireMap(value: Any?, label: String): Map<String, Any?> {
        @Suppress("UNCHECKED_CAST")
        return value as? Map<String, Any?>
            ?: throw IllegalArgumentException("$label must return an object")
    }

    private fun requireMapList(value: Any?, label: String): List<Map<String, Any?>> {
        val list = value as? List<*>
            ?: throw IllegalArgumentException("$label must return an array")
        return list.mapIndexed { index, item ->
            @Suppress("UNCHECKED_CAST")
            item as? Map<String, Any?>
                ?: throw IllegalArgumentException("$label[$index] must be an object")
        }
    }

    private fun snapshotByteArrayList(values: List<ByteArray>, label: String): List<ByteArray> =
        values.mapIndexed { index, value ->
            require(value.isNotEmpty()) { "$label[$index] must not be empty" }
            value.copyOf()
        }

    private fun normalizeRpcHex(value: Any?, label: String, byteLength: Int, allowZero: Boolean = false): String {
        require(value is String) { "$label must be canonical lowercase 0x hex" }
        require(value.trim() == value && value.startsWith("0x")) {
            "$label must be canonical lowercase 0x hex"
        }
        val text = value.substring(2)
        require(text.length == byteLength * 2 && text.matches(Regex("[0-9a-f]+"))) {
            "$label must be $byteLength bytes canonical lowercase 0x hex"
        }
        require(allowZero || text.any { it != '0' }) { "$label must not be zero" }
        return value
    }

    private fun hexLower(bytes: ByteArray): String {
        val builder = StringBuilder(bytes.size * 2)
        for (byte in bytes) {
            builder.append(String.format("%02x", byte.toInt() and 0xff))
        }
        return builder.toString()
    }

    private fun normalizeReceiptProofHash(
        receiptProof: EthereumMainnetReceiptProof?,
        suppliedHash: String?,
    ): String? {
        var normalizedHash = suppliedHash?.let { normalizeRpcHex(it, "receiptProofHash", 32) }
        if (receiptProof == null) {
            return normalizedHash
        }
        require(receiptProof.sourceDomain == SccpEvm.DOMAIN_ETH) {
            "receiptProof.sourceDomain must be ETH"
        }
        val computedHash = SccpSourceProofs.evmReceiptProofHash(
            sourceEventDigest = receiptProof.sourceEventDigest,
            beaconSlot = receiptProof.beaconSlot,
            executionBlockNumber = receiptProof.executionBlockNumber,
            executionBlockHash = receiptProof.executionBlockHash,
            executionReceiptsRoot = receiptProof.executionReceiptsRoot,
            beaconFinalizedRoot = receiptProof.beaconFinalizedRoot,
            syncCommitteeRoot = receiptProof.syncCommitteeRoot,
            receiptRootIndex = receiptProof.receiptRootIndex,
            receiptTrieProofNodes = receiptProof.receiptTrieProofNodes,
            inclusionBranch = receiptProof.inclusionBranch,
            sourceDomain = receiptProof.sourceDomain,
        )
        if (normalizedHash != null && normalizedHash != computedHash) {
            throw IllegalArgumentException("receiptProofHash must match receiptProof")
        }
        normalizedHash = computedHash
        return normalizedHash
    }

    private fun requireReceiptProofMatchesEvidence(
        receiptProof: EthereumMainnetReceiptProof?,
        blockHash: String?,
        receiptBlockNumber: String?,
        blockReceiptsRoot: String?,
        beaconFinality: Map<String, Any?>?,
        sourceEventDigest: String?,
    ) {
        if (receiptProof == null) {
            return
        }
        val proofBlockNumber = normalizeUnsignedInteger(
            receiptProof.executionBlockNumber,
            "receiptProof.executionBlockNumber",
        )
        if (receiptBlockNumber != null) {
            require(proofBlockNumber == normalizeUnsignedInteger(receiptBlockNumber, "block.number")) {
                "receiptProof.executionBlockNumber must match block.number"
            }
        }
        if (beaconFinality != null) {
            require(
                proofBlockNumber == normalizeUnsignedInteger(
                    beaconFinality["executionBlockNumber"],
                    "beaconFinality.executionBlockNumber",
                ),
            ) {
                "receiptProof.executionBlockNumber must match beaconFinality.executionBlockNumber"
            }
        }
        val proofBlockHash = normalizeRpcHex(
            receiptProof.executionBlockHash,
            "receiptProof.executionBlockHash",
            32,
        )
        if (blockHash != null) {
            require(proofBlockHash == blockHash) {
                "receiptProof.executionBlockHash must match block.hash"
            }
        }
        if (beaconFinality != null) {
            require(proofBlockHash == beaconFinality["executionBlockHash"]) {
                "receiptProof.executionBlockHash must match beaconFinality.executionBlockHash"
            }
        }
        val proofReceiptsRoot = normalizeRpcHex(
            receiptProof.executionReceiptsRoot,
            "receiptProof.executionReceiptsRoot",
            32,
        )
        if (blockReceiptsRoot != null) {
            require(proofReceiptsRoot == blockReceiptsRoot) {
                "receiptProof.executionReceiptsRoot must match block.receiptsRoot"
            }
        }
        if (beaconFinality != null) {
            require(proofReceiptsRoot == beaconFinality["executionReceiptsRoot"]) {
                "receiptProof.executionReceiptsRoot must match beaconFinality.executionReceiptsRoot"
            }
            val finalityFinalizedRoot = strictFirstPresent(
                beaconFinality,
                "beaconFinality.finalizedHeaderRoot",
                "finalizedHeaderRoot",
                "finalized_header_root",
                "beaconFinalizedRoot",
                "beacon_finalized_root",
            )
            if (finalityFinalizedRoot != null) {
                require(
                    normalizeRpcHex(
                        receiptProof.beaconFinalizedRoot,
                        "receiptProof.beaconFinalizedRoot",
                        32,
                    ) == normalizeRpcHex(
                        finalityFinalizedRoot,
                        "beaconFinality.finalizedHeaderRoot",
                        32,
                    ),
                ) {
                    "receiptProof.beaconFinalizedRoot must match beaconFinality.finalizedHeaderRoot"
                }
            }
            val finalitySyncCommitteeRoot = strictFirstPresent(
                beaconFinality,
                "beaconFinality.syncCommitteeRoot",
                "syncCommitteeRoot",
                "sync_committee_root",
            )
            if (finalitySyncCommitteeRoot != null) {
                require(
                    normalizeRpcHex(
                        receiptProof.syncCommitteeRoot,
                        "receiptProof.syncCommitteeRoot",
                        32,
                    ) == normalizeRpcHex(
                        finalitySyncCommitteeRoot,
                        "beaconFinality.syncCommitteeRoot",
                        32,
                    ),
                ) {
                    "receiptProof.syncCommitteeRoot must match beaconFinality.syncCommitteeRoot"
                }
            }
            val finalityBeaconSlot = strictFirstPresent(
                beaconFinality,
                "beaconFinality.beaconSlot",
                "beaconSlot",
                "beacon_slot",
                "finalizedSlot",
                "finalized_slot",
                "slot",
            )
            if (finalityBeaconSlot != null) {
                require(
                    normalizeUnsignedInteger(
                        receiptProof.beaconSlot,
                        "receiptProof.beaconSlot",
                    ) == normalizeUnsignedInteger(
                        finalityBeaconSlot,
                        "beaconFinality.beaconSlot",
                    ),
                ) {
                    "receiptProof.beaconSlot must match beaconFinality.beaconSlot"
                }
            }
        }
        if (sourceEventDigest != null) {
            val proofSourceEventDigest = normalizeRpcHex(
                receiptProof.sourceEventDigest,
                "receiptProof.sourceEventDigest",
                32,
            )
            require(proofSourceEventDigest == sourceEventDigest) {
                "receiptProof.sourceEventDigest must match receipt source event"
            }
        }
    }

    private fun ethereumReceiptSourceEvent(
        receipt: Map<String, Any?>?,
        sourceEventDigestInput: String?,
        sourceBridgeEmitterAddressInput: String?,
        transactionHash: String?,
        blockHash: String?,
        blockNumber: String?,
    ): Pair<String?, String?> {
        val sourceEventDigest = sourceEventDigestInput?.let {
            normalizeRpcHex(it, "sourceEventDigest", 32)
        }
        val sourceBridgeEmitterAddress = sourceBridgeEmitterAddressInput?.let {
            normalizeRpcHex(it, "sourceBridgeEmitterAddress", 20)
        }
        if (sourceEventDigest == null && sourceBridgeEmitterAddress == null) {
            return Pair(null, null)
        }
        require(sourceBridgeEmitterAddress != null) {
            "sourceBridgeEmitterAddress is required when validating sourceEventDigest"
        }
        val logs = receipt?.get("logs") as? List<*>
            ?: throw IllegalArgumentException("receipt.logs is required for SCCP source event validation")
        var matchedDigest: String? = null
        for ((index, logInput) in logs.withIndex()) {
            val log = logInput as? Map<*, *>
                ?: throw IllegalArgumentException("receipt.logs[$index] must be an object")
            if (log["removed"] == true) {
                throw IllegalArgumentException("receipt.logs must not contain removed logs")
            }
            val logAddress = normalizeRpcHex(
                log["address"],
                "receipt.logs[$index].address",
                20,
                allowZero = true,
            )
            val topics = log["topics"] as? List<*>
                ?: throw IllegalArgumentException("receipt.logs[$index].topics must be an array")
            require(topics.size <= 4) {
                "receipt.logs[$index].topics must contain at most 4 entries"
            }
            val normalizedTopics = topics.mapIndexed { topicIndex, topic ->
                normalizeRpcHex(
                    topic,
                    "receipt.logs[$index].topics[$topicIndex]",
                    32,
                    allowZero = true,
                )
            }
            if (
                logAddress == sourceBridgeEmitterAddress &&
                normalizedTopics.firstOrNull() == SccpEthereumMainnet.SOURCE_EVENT_TOPIC_V1
            ) {
                require(normalizedTopics.size == 2) {
                    "SCCP source event log must contain exactly 2 topics"
                }
                val data = log["data"] as? String
                    ?: throw IllegalArgumentException("receipt.logs[$index].data is required")
                require(data == "0x") {
                    "SCCP source event log data must be 0x"
                }
                @Suppress("UNCHECKED_CAST")
                val logFields = log as Map<String, Any?>
                val logTransactionHash = normalizeRpcHex(
                    strictFirstPresent(
                        logFields,
                        "receipt.logs[$index].transactionHash",
                        "transactionHash",
                        "transaction_hash",
                    ),
                    "receipt.logs[$index].transactionHash",
                    32,
                )
                require(transactionHash == null || logTransactionHash == transactionHash) {
                    "receipt.logs transactionHash must match receipt.transactionHash"
                }
                val logBlockHash = normalizeRpcHex(
                    strictFirstPresent(
                        logFields,
                        "receipt.logs[$index].blockHash",
                        "blockHash",
                        "block_hash",
                    ),
                    "receipt.logs[$index].blockHash",
                    32,
                )
                require(blockHash == null || logBlockHash == blockHash) {
                    "receipt.logs blockHash must match receipt.blockHash"
                }
                val logBlockNumber = normalizePositiveRpcQuantity(
                    strictFirstPresent(
                        logFields,
                        "receipt.logs[$index].blockNumber",
                        "blockNumber",
                        "block_number",
                    ),
                    "receipt.logs[$index].blockNumber",
                )
                require(blockNumber == null || logBlockNumber == blockNumber) {
                    "receipt.logs blockNumber must match receipt.blockNumber"
                }
                val candidateDigest = normalizedTopics[1]
                require(candidateDigest != "0x" + "0".repeat(64)) {
                    "SCCP source event digest must not be zero"
                }
                if (sourceEventDigest != null && candidateDigest != sourceEventDigest) {
                    continue
                }
                require(matchedDigest == null) {
                    "receipt.logs must contain exactly one matching SCCP source event"
                }
                matchedDigest = candidateDigest
            }
        }
        return Pair(
            matchedDigest
                ?: throw IllegalArgumentException("receipt.logs must contain the expected SCCP source event"),
            sourceBridgeEmitterAddress,
        )
    }

    private fun resolveSourceBridgeEmitterAddress(inputAddress: String?, defaultAddress: String?): String? {
        val normalizedInput = inputAddress?.let {
            normalizeRpcHex(it, "sourceBridgeEmitterAddress", 20)
        }
        val normalizedDefault = defaultAddress?.let {
            normalizeRpcHex(it, "sourceBridgeEmitterAddress", 20)
        }
        require(normalizedInput == null || normalizedDefault == null || normalizedInput == normalizedDefault) {
            "sourceBridgeEmitterAddress values must match"
        }
        return normalizedInput ?: normalizedDefault
    }

    private fun normalizeRpcQuantity(value: Any?, label: String): String {
        require(value is String && value.trim() == value && value.startsWith("0x")) {
            "$label must be a canonical JSON-RPC quantity"
        }
        val text = value.substring(2)
        require(text.isNotEmpty() && text.matches(Regex("0|[1-9a-f][0-9a-f]*"))) {
            "$label must be a canonical JSON-RPC quantity"
        }
        return "0x" + BigInteger(text, 16).toString(16)
    }

    private fun normalizePositiveRpcQuantity(value: Any?, label: String): String {
        val quantity = normalizeRpcQuantity(value, label)
        require(quantity != "0x0") { "$label must be positive" }
        return quantity
    }

    private val beaconFinalityAliasKeys = setOf(
        "executionBlockNumber",
        "execution_block_number",
        "finalityHeight",
        "finality_height",
        "executionBlockHash",
        "execution_block_hash",
        "finalityBlockHash",
        "finality_block_hash",
        "executionReceiptsRoot",
        "execution_receipts_root",
        "receiptsRoot",
        "receipts_root",
        "finalizedHeaderRoot",
        "finalized_header_root",
        "beaconFinalizedRoot",
        "beacon_finalized_root",
        "syncCommitteeRoot",
        "sync_committee_root",
        "beaconSlot",
        "beacon_slot",
        "finalizedSlot",
        "finalized_slot",
        "slot",
        "finalityBranch",
        "finality_branch",
        "syncCommitteeBits",
        "sync_committee_bits",
        "syncCommitteeSignature",
        "sync_committee_signature",
        "syncSignatureSlot",
        "sync_signature_slot",
        "signatureSlot",
        "signature_slot",
        "syncCommitteeParticipation",
        "sync_committee_participation",
    )

    private fun normalizeBeaconFinality(
        finality: Map<String, Any?>,
        expectedBlockHash: String?,
        expectedBlockNumber: String?,
        expectedReceiptsRoot: String?,
    ): Map<String, Any?> {
        val executionBlockNumber = normalizeUnsignedInteger(
            strictFirstPresent(
                finality,
                "beaconFinality.executionBlockNumber",
                "executionBlockNumber",
                "execution_block_number",
                "finalityHeight",
                "finality_height",
            ),
            "beaconFinality.executionBlockNumber",
        )
        require(executionBlockNumber > 0) { "beaconFinality.executionBlockNumber must be positive" }
        if (expectedBlockNumber != null) {
            require(executionBlockNumber == normalizeUnsignedInteger(expectedBlockNumber, "block.number")) {
                "beaconFinality.executionBlockNumber must match block.number"
            }
        }
        val executionBlockHash = normalizeRpcHex(
            strictFirstPresent(
                finality,
                "beaconFinality.executionBlockHash",
                "executionBlockHash",
                "execution_block_hash",
                "finalityBlockHash",
                "finality_block_hash",
            ),
            "beaconFinality.executionBlockHash",
            32,
        )
        if (expectedBlockHash != null) {
            require(executionBlockHash == expectedBlockHash) {
                "beaconFinality.executionBlockHash must match block.hash"
            }
        }
        val executionReceiptsRoot = normalizeRpcHex(
            strictFirstPresent(
                finality,
                "beaconFinality.executionReceiptsRoot",
                "executionReceiptsRoot",
                "execution_receipts_root",
                "receiptsRoot",
                "receipts_root",
            ),
            "beaconFinality.executionReceiptsRoot",
            32,
        )
        if (expectedReceiptsRoot != null) {
            require(executionReceiptsRoot == expectedReceiptsRoot) {
                "beaconFinality.executionReceiptsRoot must match block.receiptsRoot"
            }
        }
        val normalized = (finality - beaconFinalityAliasKeys) + mapOf(
            "executionBlockNumber" to executionBlockNumber.toString(),
            "executionBlockHash" to executionBlockHash,
            "executionReceiptsRoot" to executionReceiptsRoot,
        )
        val finalizedHeaderRoot = strictFirstPresent(
            finality,
            "beaconFinality.finalizedHeaderRoot",
            "finalizedHeaderRoot",
            "finalized_header_root",
            "beaconFinalizedRoot",
            "beacon_finalized_root",
        )
        val syncCommitteeRoot = strictFirstPresent(
            finality,
            "beaconFinality.syncCommitteeRoot",
            "syncCommitteeRoot",
            "sync_committee_root",
        )
        val beaconSlot = strictFirstPresent(
            finality,
            "beaconFinality.beaconSlot",
            "beaconSlot",
            "beacon_slot",
            "finalizedSlot",
            "finalized_slot",
            "slot",
        )
        val syncCommitteeBits = strictFirstPresent(
            finality,
            "beaconFinality.syncCommitteeBits",
            "syncCommitteeBits",
            "sync_committee_bits",
        )
        val finalityBranch = strictFirstPresent(
            finality,
            "beaconFinality.finalityBranch",
            "finalityBranch",
            "finality_branch",
        )
        val syncCommitteeSignature = strictFirstPresent(
            finality,
            "beaconFinality.syncCommitteeSignature",
            "syncCommitteeSignature",
            "sync_committee_signature",
        )
        val syncSignatureSlot = strictFirstPresent(
            finality,
            "beaconFinality.syncSignatureSlot",
            "syncSignatureSlot",
            "sync_signature_slot",
            "signatureSlot",
            "signature_slot",
        )
        val syncCommitteeParticipation = strictFirstPresent(
            finality,
            "beaconFinality.syncCommitteeParticipation",
            "syncCommitteeParticipation",
            "sync_committee_participation",
        )
        val normalizedBeaconSlot = beaconSlot?.let {
            val normalizedSlot = normalizeUnsignedInteger(
                it,
                "beaconFinality.beaconSlot",
            )
            require(normalizedSlot > 0) {
                "beaconFinality.beaconSlot must be positive"
            }
            normalizedSlot
        }
        val normalizedSyncSignatureSlot = syncSignatureSlot?.let {
            val normalizedSlot = normalizeUnsignedInteger(
                it,
                "beaconFinality.syncSignatureSlot",
            )
            require(normalizedSlot > 0L) {
                "beaconFinality.syncSignatureSlot must be positive"
            }
            normalizedSlot
        }
        if (normalizedBeaconSlot != null && normalizedSyncSignatureSlot != null) {
            require(normalizedSyncSignatureSlot >= normalizedBeaconSlot) {
                "beaconFinality.syncSignatureSlot must cover beaconFinality.beaconSlot"
            }
        }
        val normalizedSyncCommitteeBits = syncCommitteeBits?.let {
            normalizeEthereumBeaconRestSyncCommitteeBits(
                it,
                "beaconFinality.syncCommitteeBits",
            )
        }
        val normalizedFinalityBranch = finalityBranch?.let {
            normalizeEthereumBeaconRestFinalityBranch(
                it,
                "beaconFinality.finalityBranch",
            )
        }
        val normalizedSyncCommitteeParticipation = syncCommitteeParticipation?.let {
            val normalizedParticipation = normalizeUnsignedInteger(
                it,
                "beaconFinality.syncCommitteeParticipation",
            )
            require(normalizedParticipation > 0L) {
                "beaconFinality.syncCommitteeParticipation must be positive"
            }
            normalizedParticipation
        }
        if (normalizedSyncCommitteeParticipation != null && normalizedSyncCommitteeBits == null) {
            throw IllegalArgumentException(
                "beaconFinality.syncCommitteeBits is required when beaconFinality.syncCommitteeParticipation is present",
            )
        }
        if (
            normalizedSyncCommitteeBits != null &&
            normalizedSyncCommitteeParticipation != null
        ) {
            require(
                normalizedSyncCommitteeParticipation ==
                    ethereumBeaconRestSyncCommitteeParticipation(normalizedSyncCommitteeBits).toLong(),
            ) {
                "beaconFinality.syncCommitteeParticipation must match syncCommitteeBits"
            }
        }
        return normalized +
            listOfNotNull(
                finalizedHeaderRoot?.let {
                    "finalizedHeaderRoot" to normalizeRpcHex(
                        it,
                        "beaconFinality.finalizedHeaderRoot",
                        32,
                    )
                },
                syncCommitteeRoot?.let {
                    "syncCommitteeRoot" to normalizeRpcHex(
                        it,
                        "beaconFinality.syncCommitteeRoot",
                        32,
                    )
                },
                normalizedBeaconSlot?.let { "beaconSlot" to it.toString() },
                normalizedFinalityBranch?.let { "finalityBranch" to it },
                normalizedSyncCommitteeBits?.let { "syncCommitteeBits" to it },
                syncCommitteeSignature?.let {
                    "syncCommitteeSignature" to normalizeEthereumBeaconRestHex(
                        it,
                    "beaconFinality.syncCommitteeSignature",
                    96,
                )
            },
            normalizedSyncSignatureSlot?.let { "syncSignatureSlot" to it.toString() },
            normalizedSyncCommitteeParticipation?.let {
                "syncCommitteeParticipation" to it.toString()
            },
        ).toMap()
    }
}

/** Local-first BSC mainnet SCCP facade for native UI SDKs. */
class BscMainnetSccp(
    private val witnessProvider: EvmSccpWitnessProvider? = null,
    private val proofEngine: EvmSccpProofEngine? = null,
    private val executionProvider: BscMainnetExecutionProvider? = null,
    private val consensusProvider: BscMainnetConsensusProvider? = null,
    private val inboundProver: BscMainnetInboundProver? = null,
    private val inboundSubmitter: BscMainnetInboundSubmitter? = null,
    private val outboundSubmitter: BscMainnetOutboundSubmitter? = null,
    private val sourceBridgeEmitterAddress: String? = null,
) {
    fun validateExecutionProviderMainnet(provider: BscMainnetExecutionProvider? = executionProvider): Any? {
        val selectedProvider = provider
            ?: throw IllegalStateException("BSC mainnet execution provider is not linked")
        val chainId = selectedProvider.request("eth_chainId", emptyList())
        SccpBsc.requireMainnetChainId(normalizeRpcChainId(chainId))
        return chainId
    }

    fun collectInboundEvidenceFromReceipt(
        input: BscMainnetInboundEvidence,
        provider: BscMainnetExecutionProvider? = executionProvider,
        consensusProvider: BscMainnetConsensusProvider? = this.consensusProvider,
    ): BscMainnetInboundEvidence {
        require(input.sourceDomain == SccpEvm.DOMAIN_BSC) {
            "BSC mainnet inbound evidence sourceDomain must be BSC"
        }
        require(input.targetDomain == SccpEvm.DOMAIN_SORA) {
            "BSC mainnet inbound evidence targetDomain must be SORA"
        }
        provider?.let { validateExecutionProviderMainnet(it) }

        var transactionHash = input.transactionHash?.let {
            normalizeRpcHex(it, "transactionHash", 32)
        }
        var receipt = input.receipt
        if (receipt == null && transactionHash != null && provider != null) {
            receipt = requireMap(
                provider.request("eth_getTransactionReceipt", listOf(transactionHash)),
                "eth_getTransactionReceipt",
            )
        }
        val receiptProof = input.receiptProof?.snapshot()
        if (receipt == null && receiptProof == null && input.receiptProofHash == null) {
            throw IllegalArgumentException(
                "BSC mainnet inbound evidence requires receipt, receiptProof, receiptProofHash, or transactionHash",
            )
        }

        var blockHash: String? = null
        var receiptBlockNumber: String? = null
        var blockReceiptsRoot: String? = null
        var sourceEventDigest: String? = null
        var normalizedSourceBridgeEmitterAddress: String? = null
        if (receipt != null) {
            require(receipt["status"] == "0x1") {
                "BSC mainnet inbound receipt status must be 0x1"
            }
            val receiptTransactionHash = normalizeRpcHex(
                receipt["transactionHash"] ?: receipt["transaction_hash"],
                "receipt.transactionHash",
                32,
            )
            if (transactionHash != null) {
                require(receiptTransactionHash == transactionHash) {
                    "receipt.transactionHash must match transactionHash"
                }
            }
            transactionHash = receiptTransactionHash
            blockHash = normalizeRpcHex(receipt["blockHash"] ?: receipt["block_hash"], "receipt.blockHash", 32)
            receiptBlockNumber = normalizePositiveRpcQuantity(
                receipt["blockNumber"] ?: receipt["block_number"],
                "receipt.blockNumber",
            )
            val sourceEvent = evmReceiptSourceEvent(
                receipt = receipt,
                sourceEventDigestInput = input.sourceEventDigest,
                sourceBridgeEmitterAddressInput = resolveSourceBridgeEmitterAddress(
                    input.sourceBridgeEmitterAddress,
                    sourceBridgeEmitterAddress,
                ),
                transactionHash = transactionHash,
                blockHash = blockHash,
                blockNumber = receiptBlockNumber,
            )
            sourceEventDigest = sourceEvent.first
            normalizedSourceBridgeEmitterAddress = sourceEvent.second
        } else if (input.sourceEventDigest != null || input.sourceBridgeEmitterAddress != null) {
            throw IllegalArgumentException("receipt.logs is required for SCCP source event validation")
        }

        var block = input.block
        if (block == null && blockHash != null && provider != null) {
            block = requireMap(
                provider.request("eth_getBlockByHash", listOf(blockHash, false)),
                "eth_getBlockByHash",
            )
        }
        if (block != null) {
            val normalizedBlockHash = normalizeRpcHex(block["hash"], "block.hash", 32)
            if (blockHash != null) {
                require(normalizedBlockHash == blockHash) {
                    "block.hash must match receipt.blockHash"
                }
            }
            blockHash = normalizedBlockHash
            val blockNumber = normalizePositiveRpcQuantity(
                block["number"] ?: block["blockNumber"] ?: block["block_number"],
                "block.number",
            )
            if (receiptBlockNumber != null) {
                require(blockNumber == receiptBlockNumber) {
                    "block.number must match receipt.blockNumber"
                }
            }
            receiptBlockNumber = blockNumber
            blockReceiptsRoot = normalizeRpcHex(block["receiptsRoot"] ?: block["receipts_root"], "block.receiptsRoot", 32)
        }

        receipt = receipt?.let { sccpCallbackMapSnapshot(it) }
        block = block?.let { sccpCallbackMapSnapshot(it) }
        val parliaFinality = input.parliaFinality
            ?: consensusProvider?.collectFinalityEvidence(receipt, block, transactionHash)
        val normalizedParliaFinality = parliaFinality?.let {
            normalizeParliaFinality(
                it,
                expectedBlockHash = blockHash,
                expectedBlockNumber = receiptBlockNumber,
                expectedReceiptsRoot = blockReceiptsRoot,
            )
        }
        requireReceiptProofMatchesEvidence(
            receiptProof = receiptProof,
            blockHash = blockHash,
            receiptBlockNumber = receiptBlockNumber,
            blockReceiptsRoot = blockReceiptsRoot,
            parliaFinality = normalizedParliaFinality,
            sourceEventDigest = sourceEventDigest,
        )

        return inboundCallbackEvidenceSnapshot(input.copy(
            sourceDomain = SccpEvm.DOMAIN_BSC,
            targetDomain = SccpEvm.DOMAIN_SORA,
            transactionHash = transactionHash,
            receipt = receipt,
            block = block,
            parliaFinality = normalizedParliaFinality,
            receiptProofHash = normalizeReceiptProofHash(receiptProof, input.receiptProofHash),
            receiptProof = receiptProof,
            sourceEventDigest = sourceEventDigest,
            sourceBridgeEmitterAddress = normalizedSourceBridgeEmitterAddress,
        ))
    }

    fun proveInboundToSora(
        input: BscMainnetInboundEvidence,
        provider: BscMainnetExecutionProvider? = executionProvider,
        consensusProvider: BscMainnetConsensusProvider? = this.consensusProvider,
    ): ByteArray {
        val prover = inboundProver
            ?: throw IllegalStateException("BSC mainnet SCCP inbound prover is not linked")
        val evidence = collectInboundEvidenceFromReceipt(input, provider, consensusProvider)
        require(evidence.parliaFinality != null) {
            "BSC mainnet SCCP inbound proof requires parliaFinality"
        }
        require(evidence.receiptProof != null) {
            "BSC mainnet SCCP inbound proof requires receiptProof"
        }
        require(evidence.sourceEventDigest != null) {
            "BSC mainnet SCCP inbound proof requires receipt source event validation"
        }
        val proofBytes = prover.prove(inboundCallbackEvidenceSnapshot(evidence))
        require(proofBytes.isNotEmpty()) { "proofBytes must not be empty" }
        require(proofBytes.any { it.toInt() != 0 }) { "proofBytes must not be all zero" }
        return proofBytes.copyOf()
    }

    fun submitInboundToIroha(proofBytes: ByteArray): Any? {
        require(proofBytes.isNotEmpty()) { "proofBytes must not be empty" }
        require(proofBytes.any { it.toInt() != 0 }) { "proofBytes must not be all zero" }
        val submitter = inboundSubmitter
            ?: throw IllegalStateException("BSC mainnet SCCP inbound submitter is not linked")
        return submitter.submit(proofBytes.copyOf())
    }

    fun buildOutboundProofRequest(input: EvmSccpProofRequestInput): EvmSccpProofRequest =
        SccpBsc.buildProofRequest(
            witnessProvider?.resolveWitness(witnessProviderInputSnapshot(input)) ?: input,
        )

    fun proveOutboundToBsc(input: EvmSccpProofRequestInput): EvmSccpProofResult {
        val request = buildOutboundProofRequest(input)
        val engine = proofEngine
            ?: throw IllegalStateException("BSC mainnet SCCP Groth16 prover is not linked")
        return SccpBsc.wrapProofResult(
            engine.prove(SccpEvm.callbackRequestSnapshot(request)),
            request,
        )
    }

    fun buildBscCalldata(input: EvmSccpSubmissionInput): EvmSccpSubmission =
        SccpBsc.buildSubmission(input)

    fun buildLocalAdmissionSubmission(
        input: BscMainnetLocalAdmissionSubmissionInput,
    ): BscMainnetLocalAdmissionSubmission =
        SccpBsc.buildLocalAdmissionSubmission(input)

    fun submitOutboundToBsc(input: EvmSccpSubmissionInput): Any? {
        val submitter = outboundSubmitter
            ?: throw IllegalStateException("BSC mainnet SCCP outbound submitter is not linked")
        return submitter.submit(buildBscCalldata(input))
    }

    fun requireExecutionMainnet(chainId: Long) =
        SccpBsc.requireMainnetChainId(chainId)

    private fun witnessProviderInputSnapshot(input: EvmSccpProofRequestInput): EvmSccpProofRequestInput =
        input.copy(
            bundleBytes = input.bundleBytes.copyOf(),
            sourceProofBytes = input.sourceProofBytes.copyOf(),
        )

    private fun inboundCallbackEvidenceSnapshot(
        evidence: BscMainnetInboundEvidence,
    ): BscMainnetInboundEvidence =
        evidence.copy(
            receipt = evidence.receipt?.let { sccpCallbackMapSnapshot(it) },
            block = evidence.block?.let { sccpCallbackMapSnapshot(it) },
            parliaFinality = evidence.parliaFinality?.let { sccpCallbackMapSnapshot(it) },
            receiptProof = evidence.receiptProof?.snapshot(),
        )

    private fun normalizeRpcChainId(value: Any?): Long {
        val quantity = normalizeRpcQuantity(value, "eth_chainId")
        val parsed = BigInteger(quantity.substring(2), 16)
        require(parsed.bitLength() <= 63) { "eth_chainId must fit positive i64" }
        return parsed.toLong()
    }

    private fun requireMap(value: Any?, label: String): Map<String, Any?> {
        @Suppress("UNCHECKED_CAST")
        return value as? Map<String, Any?>
            ?: throw IllegalArgumentException("$label must return an object")
    }

    private fun normalizeRpcHex(
        value: Any?,
        label: String,
        byteLength: Int,
        allowZero: Boolean = false,
    ): String {
        require(value is String) { "$label must be canonical lowercase 0x hex" }
        require(value.trim() == value && value.startsWith("0x")) {
            "$label must be canonical lowercase 0x hex"
        }
        val text = value.substring(2)
        require(text.length == byteLength * 2 && text.matches(Regex("[0-9a-f]+"))) {
            "$label must be $byteLength bytes canonical lowercase 0x hex"
        }
        require(allowZero || text.any { it != '0' }) { "$label must not be zero" }
        return value
    }

    private fun normalizeReceiptProofHash(
        receiptProof: BscMainnetReceiptProof?,
        suppliedHash: String?,
    ): String? {
        var normalizedHash = suppliedHash?.let { normalizeRpcHex(it, "receiptProofHash", 32) }
        if (receiptProof == null) {
            return normalizedHash
        }
        require(receiptProof.sourceDomain == SccpEvm.DOMAIN_BSC) {
            "receiptProof.sourceDomain must be BSC"
        }
        val computedHash = SccpSourceProofs.bscReceiptProofHash(
            sourceEventDigest = receiptProof.sourceEventDigest,
            validatorEpoch = receiptProof.validatorEpoch,
            blockNumber = receiptProof.blockNumber,
            blockHash = receiptProof.blockHash,
            receiptsRoot = receiptProof.receiptsRoot,
            validatorSetHash = receiptProof.validatorSetHash,
            commitSealHash = receiptProof.commitSealHash,
            receiptRootIndex = receiptProof.receiptRootIndex,
            receiptTrieProofNodes = receiptProof.receiptTrieProofNodes,
            inclusionBranch = receiptProof.inclusionBranch,
            sourceDomain = receiptProof.sourceDomain,
        )
        if (normalizedHash != null && normalizedHash != computedHash) {
            throw IllegalArgumentException("receiptProofHash must match receiptProof")
        }
        normalizedHash = computedHash
        return normalizedHash
    }

    private fun requireReceiptProofMatchesEvidence(
        receiptProof: BscMainnetReceiptProof?,
        blockHash: String?,
        receiptBlockNumber: String?,
        blockReceiptsRoot: String?,
        parliaFinality: Map<String, Any?>?,
        sourceEventDigest: String?,
    ) {
        if (receiptProof == null) {
            return
        }
        val proofBlockNumber = normalizeUnsignedInteger(receiptProof.blockNumber, "receiptProof.blockNumber")
        if (receiptBlockNumber != null) {
            require(proofBlockNumber == normalizeUnsignedInteger(receiptBlockNumber, "block.number")) {
                "receiptProof.blockNumber must match block.number"
            }
        }
        if (parliaFinality != null) {
            require(
                proofBlockNumber == normalizeUnsignedInteger(
                    parliaFinality["executionBlockNumber"],
                    "parliaFinality.executionBlockNumber",
                ),
            ) {
                "receiptProof.blockNumber must match parliaFinality.executionBlockNumber"
            }
        }
        val proofBlockHash = normalizeRpcHex(receiptProof.blockHash, "receiptProof.blockHash", 32)
        if (blockHash != null) {
            require(proofBlockHash == blockHash) {
                "receiptProof.blockHash must match block.hash"
            }
        }
        if (parliaFinality != null) {
            require(proofBlockHash == parliaFinality["executionBlockHash"]) {
                "receiptProof.blockHash must match parliaFinality.executionBlockHash"
            }
        }
        val proofReceiptsRoot = normalizeRpcHex(receiptProof.receiptsRoot, "receiptProof.receiptsRoot", 32)
        if (blockReceiptsRoot != null) {
            require(proofReceiptsRoot == blockReceiptsRoot) {
                "receiptProof.receiptsRoot must match block.receiptsRoot"
            }
        }
        if (parliaFinality != null) {
            require(proofReceiptsRoot == parliaFinality["executionReceiptsRoot"]) {
                "receiptProof.receiptsRoot must match parliaFinality.executionReceiptsRoot"
            }
            val finalityValidatorEpoch = parliaFinality["validatorEpoch"] ?: parliaFinality["validator_epoch"]
            if (finalityValidatorEpoch != null) {
                require(
                    normalizeUnsignedInteger(
                        receiptProof.validatorEpoch,
                        "receiptProof.validatorEpoch",
                    ) == normalizeUnsignedInteger(
                        finalityValidatorEpoch,
                        "parliaFinality.validatorEpoch",
                    ),
                ) {
                    "receiptProof.validatorEpoch must match parliaFinality.validatorEpoch"
                }
            }
            val finalityValidatorSetHash = parliaFinality["validatorSetHash"] ?: parliaFinality["validator_set_hash"]
            if (finalityValidatorSetHash != null) {
                require(
                    normalizeRpcHex(
                        receiptProof.validatorSetHash,
                        "receiptProof.validatorSetHash",
                        32,
                    ) == normalizeRpcHex(
                        finalityValidatorSetHash,
                        "parliaFinality.validatorSetHash",
                        32,
                    ),
                ) {
                    "receiptProof.validatorSetHash must match parliaFinality.validatorSetHash"
                }
            }
            val finalityCommitSealHash = parliaFinality["commitSealHash"] ?: parliaFinality["commit_seal_hash"]
            if (finalityCommitSealHash != null) {
                require(
                    normalizeRpcHex(
                        receiptProof.commitSealHash,
                        "receiptProof.commitSealHash",
                        32,
                    ) == normalizeRpcHex(
                        finalityCommitSealHash,
                        "parliaFinality.commitSealHash",
                        32,
                    ),
                ) {
                    "receiptProof.commitSealHash must match parliaFinality.commitSealHash"
                }
            }
        }
        if (sourceEventDigest != null) {
            val proofSourceEventDigest = normalizeRpcHex(
                receiptProof.sourceEventDigest,
                "receiptProof.sourceEventDigest",
                32,
            )
            require(proofSourceEventDigest == sourceEventDigest) {
                "receiptProof.sourceEventDigest must match receipt source event"
            }
        }
    }

    private fun evmReceiptSourceEvent(
        receipt: Map<String, Any?>?,
        sourceEventDigestInput: String?,
        sourceBridgeEmitterAddressInput: String?,
        transactionHash: String?,
        blockHash: String?,
        blockNumber: String?,
    ): Pair<String?, String?> {
        val sourceEventDigest = sourceEventDigestInput?.let {
            normalizeRpcHex(it, "sourceEventDigest", 32)
        }
        val sourceBridgeEmitterAddress = sourceBridgeEmitterAddressInput?.let {
            normalizeRpcHex(it, "sourceBridgeEmitterAddress", 20)
        }
        if (sourceEventDigest == null && sourceBridgeEmitterAddress == null) {
            return Pair(null, null)
        }
        require(sourceBridgeEmitterAddress != null) {
            "sourceBridgeEmitterAddress is required when validating sourceEventDigest"
        }
        val logs = receipt?.get("logs") as? List<*>
            ?: throw IllegalArgumentException("receipt.logs is required for SCCP source event validation")
        var matchedDigest: String? = null
        for ((index, logInput) in logs.withIndex()) {
            val log = logInput as? Map<*, *>
                ?: throw IllegalArgumentException("receipt.logs[$index] must be an object")
            if (log["removed"] == true) {
                throw IllegalArgumentException("receipt.logs must not contain removed logs")
            }
            val logAddress = normalizeRpcHex(
                log["address"],
                "receipt.logs[$index].address",
                20,
                allowZero = true,
            )
            val topics = log["topics"] as? List<*>
                ?: throw IllegalArgumentException("receipt.logs[$index].topics must be an array")
            require(topics.size <= 4) {
                "receipt.logs[$index].topics must contain at most 4 entries"
            }
            val normalizedTopics = topics.mapIndexed { topicIndex, topic ->
                normalizeRpcHex(
                    topic,
                    "receipt.logs[$index].topics[$topicIndex]",
                    32,
                    allowZero = true,
                )
            }
            if (
                logAddress == sourceBridgeEmitterAddress &&
                normalizedTopics.firstOrNull() == SccpEthereumMainnet.SOURCE_EVENT_TOPIC_V1
            ) {
                require(normalizedTopics.size == 2) {
                    "SCCP source event log must contain exactly 2 topics"
                }
                val data = log["data"] as? String
                    ?: throw IllegalArgumentException("receipt.logs[$index].data is required")
                require(data == "0x") {
                    "SCCP source event log data must be 0x"
                }
                @Suppress("UNCHECKED_CAST")
                val logFields = log as Map<String, Any?>
                val logTransactionHash = normalizeRpcHex(
                    strictFirstPresent(
                        logFields,
                        "receipt.logs[$index].transactionHash",
                        "transactionHash",
                        "transaction_hash",
                    ),
                    "receipt.logs[$index].transactionHash",
                    32,
                )
                require(transactionHash == null || logTransactionHash == transactionHash) {
                    "receipt.logs transactionHash must match receipt.transactionHash"
                }
                val logBlockHash = normalizeRpcHex(
                    strictFirstPresent(
                        logFields,
                        "receipt.logs[$index].blockHash",
                        "blockHash",
                        "block_hash",
                    ),
                    "receipt.logs[$index].blockHash",
                    32,
                )
                require(blockHash == null || logBlockHash == blockHash) {
                    "receipt.logs blockHash must match receipt.blockHash"
                }
                val logBlockNumber = normalizePositiveRpcQuantity(
                    strictFirstPresent(
                        logFields,
                        "receipt.logs[$index].blockNumber",
                        "blockNumber",
                        "block_number",
                    ),
                    "receipt.logs[$index].blockNumber",
                )
                require(blockNumber == null || logBlockNumber == blockNumber) {
                    "receipt.logs blockNumber must match receipt.blockNumber"
                }
                val candidateDigest = normalizedTopics[1]
                require(candidateDigest != "0x" + "0".repeat(64)) {
                    "SCCP source event digest must not be zero"
                }
                if (sourceEventDigest != null && candidateDigest != sourceEventDigest) {
                    continue
                }
                require(matchedDigest == null) {
                    "receipt.logs must contain exactly one matching SCCP source event"
                }
                matchedDigest = candidateDigest
            }
        }
        return Pair(
            matchedDigest
                ?: throw IllegalArgumentException("receipt.logs must contain the expected SCCP source event"),
            sourceBridgeEmitterAddress,
        )
    }

    private fun resolveSourceBridgeEmitterAddress(inputAddress: String?, defaultAddress: String?): String? {
        val normalizedInput = inputAddress?.let {
            normalizeRpcHex(it, "sourceBridgeEmitterAddress", 20)
        }
        val normalizedDefault = defaultAddress?.let {
            normalizeRpcHex(it, "sourceBridgeEmitterAddress", 20)
        }
        require(normalizedInput == null || normalizedDefault == null || normalizedInput == normalizedDefault) {
            "sourceBridgeEmitterAddress values must match"
        }
        return normalizedInput ?: normalizedDefault
    }

    private fun normalizeRpcQuantity(value: Any?, label: String): String {
        require(value is String && value.trim() == value && value.startsWith("0x")) {
            "$label must be a canonical JSON-RPC quantity"
        }
        val text = value.substring(2)
        require(text.isNotEmpty() && text.matches(Regex("0|[1-9a-f][0-9a-f]*"))) {
            "$label must be a canonical JSON-RPC quantity"
        }
        return "0x" + BigInteger(text, 16).toString(16)
    }

    private fun normalizePositiveRpcQuantity(value: Any?, label: String): String {
        val quantity = normalizeRpcQuantity(value, label)
        require(quantity != "0x0") { "$label must be positive" }
        return quantity
    }

    private fun normalizeUnsignedInteger(value: Any?, label: String): Long =
        when (value) {
            is Long -> {
                require(value >= 0) { "$label must be non-negative" }
                value
            }
            is Int -> {
                require(value >= 0) { "$label must be non-negative" }
                value.toLong()
            }
            is Short -> {
                require(value >= 0) { "$label must be non-negative" }
                value.toLong()
            }
            is Byte -> {
                require(value >= 0) { "$label must be non-negative" }
                value.toLong()
            }
            is BigInteger -> {
                require(value.signum() >= 0 && value.bitLength() <= 63) {
                    "$label must fit positive i64"
                }
                value.toLong()
            }
            is String -> {
                require(value.trim() == value) { "$label must be canonical" }
                val parsed = if (value.startsWith("0x")) {
                    val text = value.substring(2)
                    require(text.isNotEmpty() && text.matches(Regex("0|[1-9a-f][0-9a-f]*"))) {
                        "$label must be a canonical JSON-RPC quantity"
                    }
                    BigInteger(text, 16)
                } else {
                    require(value.matches(Regex("0|[1-9][0-9]*"))) {
                        "$label must be a canonical decimal integer"
                    }
                    BigInteger(value, 10)
                }
                require(parsed.bitLength() <= 63) { "$label must fit positive i64" }
                parsed.toLong()
            }
            else -> throw IllegalArgumentException("$label must be a JSON-RPC quantity or integer")
        }

    private fun normalizeParliaFinality(
        finality: Map<String, Any?>,
        expectedBlockHash: String?,
        expectedBlockNumber: String?,
        expectedReceiptsRoot: String?,
    ): Map<String, Any?> {
        val executionBlockNumber = normalizeUnsignedInteger(
            finality["executionBlockNumber"]
                ?: finality["execution_block_number"]
                ?: finality["finalityHeight"]
                ?: finality["finality_height"],
            "parliaFinality.executionBlockNumber",
        )
        require(executionBlockNumber > 0) { "parliaFinality.executionBlockNumber must be positive" }
        if (expectedBlockNumber != null) {
            require(executionBlockNumber == normalizeUnsignedInteger(expectedBlockNumber, "block.number")) {
                "parliaFinality.executionBlockNumber must match block.number"
            }
        }
        val executionBlockHash = normalizeRpcHex(
            finality["executionBlockHash"]
                ?: finality["execution_block_hash"]
                ?: finality["finalityBlockHash"]
                ?: finality["finality_block_hash"],
            "parliaFinality.executionBlockHash",
            32,
        )
        if (expectedBlockHash != null) {
            require(executionBlockHash == expectedBlockHash) {
                "parliaFinality.executionBlockHash must match block.hash"
            }
        }
        val executionReceiptsRoot = normalizeRpcHex(
            finality["executionReceiptsRoot"]
                ?: finality["execution_receipts_root"]
                ?: finality["receiptsRoot"]
                ?: finality["receipts_root"],
            "parliaFinality.executionReceiptsRoot",
            32,
        )
        if (expectedReceiptsRoot != null) {
            require(executionReceiptsRoot == expectedReceiptsRoot) {
                "parliaFinality.executionReceiptsRoot must match block.receiptsRoot"
            }
        }
        return finality + mapOf(
            "executionBlockNumber" to executionBlockNumber.toString(),
            "executionBlockHash" to executionBlockHash,
            "executionReceiptsRoot" to executionReceiptsRoot,
        )
    }
}

/** Local-first BSC mainnet SCCP Groth16 proof wrapper for UI SDKs. */
class BscSccpProver(
    private val witnessProvider: EvmSccpWitnessProvider? = null,
    private val proofEngine: EvmSccpProofEngine? = null,
) {
    fun buildRequest(input: EvmSccpProofRequestInput): EvmSccpProofRequest =
        SccpBsc.buildProofRequest(witnessProvider?.resolveWitness(witnessProviderInputSnapshot(input)) ?: input)

    fun prove(input: EvmSccpProofRequestInput): EvmSccpProofResult {
        val request = buildRequest(input)
        val engine = proofEngine ?: throw IllegalStateException("BSC mainnet SCCP Groth16 prover is not linked")
        return SccpBsc.wrapProofResult(engine.prove(SccpEvm.callbackRequestSnapshot(request)), request)
    }

    private fun witnessProviderInputSnapshot(input: EvmSccpProofRequestInput): EvmSccpProofRequestInput =
        input.copy(
            bundleBytes = input.bundleBytes.copyOf(),
            sourceProofBytes = input.sourceProofBytes.copyOf(),
        )
}
