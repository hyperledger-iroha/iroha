package org.hyperledger.iroha.sdk.privacy

import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import java.util.Collections

/** Raw Norito V1 privacy proof bridge backed by `connect_norito_bridge`. */
class PrivacyNativeBridge private constructor() {
    companion object {
        const val REQUIRED_BRIDGE_ABI_VERSION: Int = 7
        const val PRIVACY_FFI_VERSION_V1: Int = 1
        const val PRODUCTION_GATE_VERSION: String = "privacy-production-gate-v1"
        // 0 unambiguously signals a successful build/verify result; the native bridge
        // only emits it on the real proving path. STATUS_ERROR (1) covers every failure.
        const val STATUS_OK: Int = 0
        const val STATUS_ERROR: Int = 1
        const val ERROR_NULL_POINTER: Int = 1
        const val ERROR_MALFORMED_NORITO: Int = 2
        const val ERROR_UNSUPPORTED_ALGORITHM: Int = 3
        const val ERROR_PRODUCTION_DISABLED: Int = 4
        const val ERROR_INVALID_REQUEST: Int = 5
        // The real prover returns this when a structurally valid request fails inside the
        // circuit (e.g. root mismatch, non-canonical scalar) rather than at request decode.
        const val ERROR_PROVING_FAILED: Int = 6
        const val PRIVACY_NATIVE_ARCHIVE_MAX_BYTES: Int = 64 * 1024 * 1024

        private const val PRIVACY_NORITO_HEADER_BYTES: Int = 40
        private const val PRIVACY_NORITO_MAX_HEADER_PADDING_BYTES: Int = 64
        private const val PRIVACY_NORITO_SUPPORTED_FLAGS_MASK: Int = 0x27
        private const val PRIVACY_NORITO_FIELD_BITSET_FLAG: Int = 0x20
        private const val PRIVACY_NORITO_FIELD_BITSET_REQUIRED_FLAGS: Int = 0x06
        private const val PRIVACY_REQUEST_TEXT_FIELD_MAX_BYTES: Int = 1024
        private const val PRIVACY_REQUEST_PUBLIC_INPUTS_MAX_BYTES: Int = 1024 * 1024
        private const val PRIVACY_REQUEST_WITNESS_MAX_BYTES: Int = PRIVACY_NATIVE_ARCHIVE_MAX_BYTES / 2
        private const val PRIVACY_REQUEST_PROOF_MAX_BYTES: Int = PRIVACY_NATIVE_ARCHIVE_MAX_BYTES / 2
        private const val PRIVACY_SCHEMA_REQUEST: Int = 0x52
        private const val PRIVACY_SCHEMA_CAPABILITIES_RESULT: Int = 0x50
        private const val PRIVACY_SCHEMA_BUILD_PROOF_RESULT: Int = 0x42
        private const val PRIVACY_SCHEMA_VERIFY_PROOF_RESULT: Int = 0x56
        private const val PRIVACY_CRC64_REFLECTED_POLY: Long = -3932672073523589310L
        private const val CONFIDENTIAL_TRANSFER_ALGORITHM_ID: String = "confidential-transfer-v2"
        private const val UNSHIELD_ALGORITHM_ID: String = "unshield"
        private const val LIBRARY_NAME = "connect_norito_bridge"
        private val PRIVACY_NORITO_MAGIC = byteArrayOf(
            'N'.code.toByte(),
            'R'.code.toByte(),
            'T'.code.toByte(),
            '0'.code.toByte(),
        )
        private val PRIVACY_CRC64_TABLE: LongArray = buildPrivacyCrc64Table()
        private val PRIVACY_NATIVE_AVAILABILITY_PROBE_ARCHIVE =
            buildPrivacyNativeAvailabilityProbeArchive()
        private val nativeAvailable: Boolean = loadLibrary()

        internal data class NativeGateStatus(
            val key: String,
            val passed: Boolean,
        )

        internal data class NativeProductionGate(
            val version: String,
            val ready: Boolean,
            val gates: List<NativeGateStatus>,
            val requiredGates: List<String>,
            val missing: List<String>,
            val auditReferences: List<String>,
        )

        internal data class NativeCapability(
            val algorithmId: String,
            val proofFamily: String,
            val backendFamily: String,
            val sdkEntrypoints: List<String>,
            val plannedEntrypoints: List<String>,
            val productionReady: Boolean,
            val productionGate: NativeProductionGate,
        )

        internal data class NativeCapabilities(
            val version: Int,
            val gateVersion: String,
            val algorithms: List<NativeCapability>,
        )

        private val nativeGateStatusAdapter =
            NoritoAdapters.struct(
                listOf(
                    NoritoAdapters.field("key", NoritoAdapters.stringAdapter()),
                    NoritoAdapters.field("passed", NoritoAdapters.boolAdapter()),
                ),
            ) { fields ->
                NativeGateStatus(
                    key = fields.stringField("key"),
                    passed = fields.booleanField("passed"),
                )
            }

        private val nativeProductionGateAdapter =
            NoritoAdapters.struct(
                listOf(
                    NoritoAdapters.field("version", NoritoAdapters.stringAdapter()),
                    NoritoAdapters.field("ready", NoritoAdapters.boolAdapter()),
                    NoritoAdapters.field("gates", NoritoAdapters.sequence(nativeGateStatusAdapter)),
                    NoritoAdapters.field("required_gates", NoritoAdapters.sequence(NoritoAdapters.stringAdapter())),
                    NoritoAdapters.field("missing", NoritoAdapters.sequence(NoritoAdapters.stringAdapter())),
                    NoritoAdapters.field("audit_references", NoritoAdapters.sequence(NoritoAdapters.stringAdapter())),
                ),
            ) { fields ->
                NativeProductionGate(
                    version = fields.stringField("version"),
                    ready = fields.booleanField("ready"),
                    gates = fields.listField("gates"),
                    requiredGates = fields.listField("required_gates"),
                    missing = fields.listField("missing"),
                    auditReferences = fields.listField("audit_references"),
                )
            }

        private val nativeCapabilityAdapter =
            NoritoAdapters.struct(
                listOf(
                    NoritoAdapters.field("algorithm_id", NoritoAdapters.stringAdapter()),
                    NoritoAdapters.field("proof_family", NoritoAdapters.stringAdapter()),
                    NoritoAdapters.field("backend_family", NoritoAdapters.stringAdapter()),
                    NoritoAdapters.field("sdk_entrypoints", NoritoAdapters.sequence(NoritoAdapters.stringAdapter())),
                    NoritoAdapters.field("planned_entrypoints", NoritoAdapters.sequence(NoritoAdapters.stringAdapter())),
                    NoritoAdapters.field("production_ready", NoritoAdapters.boolAdapter()),
                    NoritoAdapters.field("production_gate", nativeProductionGateAdapter),
                ),
            ) { fields ->
                NativeCapability(
                    algorithmId = fields.stringField("algorithm_id"),
                    proofFamily = fields.stringField("proof_family"),
                    backendFamily = fields.stringField("backend_family"),
                    sdkEntrypoints = fields.listField("sdk_entrypoints"),
                    plannedEntrypoints = fields.listField("planned_entrypoints"),
                    productionReady = fields.booleanField("production_ready"),
                    productionGate = fields.objectField("production_gate"),
                )
            }

        private val nativeCapabilitiesAdapter =
            NoritoAdapters.struct(
                listOf(
                    NoritoAdapters.field("version", NoritoAdapters.uint(32)),
                    NoritoAdapters.field("gate_version", NoritoAdapters.stringAdapter()),
                    NoritoAdapters.field("algorithms", NoritoAdapters.sequence(nativeCapabilityAdapter)),
                ),
            ) { fields ->
                NativeCapabilities(
                    version = fields.uintField("version"),
                    gateVersion = fields.stringField("gate_version"),
                    algorithms = fields.listField("algorithms"),
                )
            }

        @JvmStatic
        fun isNativeAvailable(): Boolean = nativeAvailable

        @JvmStatic
        fun privacyCapabilities(): PrivacyCapabilities = privacyCapabilities(nativeAvailable)

        @JvmStatic
        fun capabilitiesArchive(): ByteArray {
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            return requireNativeOutput(
                invokeNativeOutput("privacy capabilities") { nativeCapabilities() },
                "privacy capabilities",
                PRIVACY_SCHEMA_CAPABILITIES_RESULT,
            )
        }

        @JvmStatic
        @JvmOverloads
        fun privacyProofRequestV1(
            algorithmId: String?,
            entrypoint: String?,
            vkRef: String?,
            publicInputs: ByteArray?,
            witness: ByteArray? = ByteArray(0),
            proof: ByteArray? = ByteArray(0),
        ): ByteArray {
            val algorithmIdBytes = privacyRequestTextBytes(algorithmId, "algorithmId")
            val entrypointBytes = privacyRequestTextBytes(entrypoint, "entrypoint")
            val vkRefBytes = privacyRequestTextBytes(vkRef, "vkRef")
            val publicInputsBytes = privacyRequestComponentBytes(
                publicInputs,
                "publicInputs",
                PRIVACY_REQUEST_PUBLIC_INPUTS_MAX_BYTES,
                allowEmpty = false,
            )
            val witnessBytes = privacyRequestComponentBytes(
                witness,
                "witness",
                PRIVACY_REQUEST_WITNESS_MAX_BYTES,
                allowEmpty = true,
            )
            val proofBytes = privacyRequestComponentBytes(
                proof,
                "proof",
                PRIVACY_REQUEST_PROOF_MAX_BYTES,
                allowEmpty = true,
            )
            try {
                check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
                return requireNativeOutput(
                    invokeNativeOutput("privacy proof request") {
                        nativeProofRequest(
                            algorithmIdBytes,
                            entrypointBytes,
                            vkRefBytes,
                            publicInputsBytes,
                            witnessBytes,
                            proofBytes,
                        )
                    },
                    "privacy proof request",
                    PRIVACY_SCHEMA_REQUEST,
                )
            } finally {
                algorithmIdBytes.fill(0)
                entrypointBytes.fill(0)
                vkRefBytes.fill(0)
                publicInputsBytes.fill(0)
                witnessBytes.fill(0)
                proofBytes.fill(0)
            }
        }

        @JvmStatic
        fun buildProof(requestArchive: ByteArray?): ByteArray =
            call("build proof", requestArchive, ::nativeBuildProof)

        @JvmStatic
        fun buildConfidentialTransferProofV2(requestArchive: ByteArray?): ByteArray =
            buildProof(requestArchive)

        @JvmStatic
        fun buildConfidentialUnshieldProofV3(requestArchive: ByteArray?): ByteArray =
            buildProof(requestArchive)

        @JvmStatic
        fun buildZkAceAuthorizationProofV1(requestArchive: ByteArray?): ByteArray =
            buildProof(requestArchive)

        @JvmStatic
        fun buildJindoLatticeProofV0(requestArchive: ByteArray?): ByteArray =
            buildProof(requestArchive)

        @JvmStatic
        fun buildSisHintsAnonymousCredentialProofV0(requestArchive: ByteArray?): ByteArray =
            buildProof(requestArchive)

        @JvmStatic
        fun buildSilentThresholdCredentialShowingProofV0(requestArchive: ByteArray?): ByteArray =
            buildProof(requestArchive)

        @JvmStatic
        fun buildVegaCredentialPredicateProofV0(requestArchive: ByteArray?): ByteArray =
            buildProof(requestArchive)

        @JvmStatic
        fun buildZkAmsAdmissionBatchProofV0(requestArchive: ByteArray?): ByteArray =
            buildProof(requestArchive)

        @JvmStatic
        fun buildZkAtPolicyProofV1(requestArchive: ByteArray?): ByteArray =
            buildProof(requestArchive)

        @JvmStatic
        fun verifyProof(requestArchive: ByteArray?): ByteArray =
            call("verify proof", requestArchive, ::nativeVerifyProof)

        @JvmStatic
        fun verifyJindoPolynomialCommitmentV0(requestArchive: ByteArray?): ByteArray =
            verifyProof(requestArchive)

        @JvmStatic
        fun verifySisHintsAnonymousCredentialProofV0(requestArchive: ByteArray?): ByteArray =
            verifyProof(requestArchive)

        @JvmStatic
        fun verifySilentThresholdCredentialShowingProofV0(requestArchive: ByteArray?): ByteArray =
            verifyProof(requestArchive)

        @JvmStatic
        fun verifyVegaCredentialPredicateProofV0(requestArchive: ByteArray?): ByteArray =
            verifyProof(requestArchive)

        @JvmStatic
        fun verifyZkAmsAdmissionBatchProofV0(requestArchive: ByteArray?): ByteArray =
            verifyProof(requestArchive)

        @JvmStatic
        fun verifyZkAtPolicyProofV1(requestArchive: ByteArray?): ByteArray =
            verifyProof(requestArchive)

        internal fun call(
            label: String,
            requestArchive: ByteArray?,
            nativeCall: (ByteArray) -> ByteArray?,
            bridgeAvailable: Boolean = nativeAvailable,
        ): ByteArray {
            require(requestArchive != null && requestArchive.isNotEmpty()) {
                "requestArchive must not be empty"
            }
            require(requestArchive.size <= PRIVACY_NATIVE_ARCHIVE_MAX_BYTES) {
                "requestArchive must not exceed $PRIVACY_NATIVE_ARCHIVE_MAX_BYTES bytes"
            }
            require(isValidPrivacyNoritoArchive(requestArchive)) {
                "requestArchive must be a valid Norito V1 archive"
            }
            require(hasPrivacyNoritoSchema(requestArchive, PRIVACY_SCHEMA_REQUEST)) {
                "requestArchive must use the privacy request schema"
            }
            require(hasNonEmptyPrivacyNoritoPayload(requestArchive)) {
                "requestArchive must contain a non-empty privacy request payload"
            }
            check(bridgeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            val outputLabel = "privacy $label"
            val expectedSchemaByte = expectedPrivacyResultSchema(outputLabel)
                ?: throw IllegalStateException("$outputLabel is not a supported privacy native operation")
            val request = requestArchive.copyOf()
            try {
                return requireNativeOutput(
                    invokeNativeOutput(outputLabel) { nativeCall(request) },
                    outputLabel,
                    expectedSchemaByte,
                )
            } finally {
                request.fill(0)
            }
        }

        internal fun invokeNativeOutput(label: String, nativeCall: () -> ByteArray?): ByteArray? {
            try {
                return nativeCall()
            } catch (_: RuntimeException) {
                throw IllegalStateException("$label failed")
            } catch (_: LinkageError) {
                throw IllegalStateException("$label failed")
            }
        }

        internal fun requireNativeOutput(
            output: ByteArray?,
            label: String,
            expectedSchemaByte: Int = expectedPrivacyResultSchema(label)
                ?: throw IllegalStateException("$label is not a supported privacy native operation"),
        ): ByteArray {
            if (expectedSchemaByte < 0) {
                throw IllegalStateException("$label is not a supported privacy native operation")
            }
            if (output == null) {
                throw IllegalStateException("$label returned no output")
            }
            try {
                if (output.isEmpty()) {
                    throw IllegalStateException("$label returned empty output")
                }
                if (output.size > PRIVACY_NATIVE_ARCHIVE_MAX_BYTES) {
                    throw IllegalStateException("$label returned oversized output")
                }
                if (!isValidPrivacyNoritoArchive(output)) {
                    throw IllegalStateException("$label returned invalid Norito V1 archive")
                }
                if (!hasNonEmptyPrivacyNoritoPayload(output)) {
                    throw IllegalStateException("$label returned empty privacy result payload")
                }
                if (!hasPrivacyNoritoSchema(output, expectedSchemaByte)) {
                    throw IllegalStateException("$label returned unexpected privacy result schema")
                }
                return output.copyOf()
            } finally {
                output.fill(0)
            }
        }

        private fun loadLibrary(): Boolean =
            detectNativeAvailability(
                loadLibrary = { System.loadLibrary(LIBRARY_NAME) },
                bridgeAbiVersion = { nativeBridgeAbiVersion() },
                probeSymbol = { probeRequiredNativeSymbols() },
            )

        private fun probeRequiredNativeSymbols(): Boolean {
            var available = true
            available = returnsOutputProbe(PRIVACY_SCHEMA_CAPABILITIES_RESULT) {
                nativeCapabilities()
            } && available
            available = proofRequestOutputProbe() && available
            available = returnsOutputProbe(PRIVACY_SCHEMA_BUILD_PROOF_RESULT) {
                nativeBuildProof(privacyNativeAvailabilityProbeArchive())
            } && available
            available = returnsOutputProbe(PRIVACY_SCHEMA_VERIFY_PROOF_RESULT) {
                nativeVerifyProof(privacyNativeAvailabilityProbeArchive())
            } && available
            return available
        }

        private fun proofRequestOutputProbe(): Boolean {
            val algorithmId = "zk-ace-pq-authorization-v0".toByteArray(Charsets.UTF_8)
            val entrypoint = "buildZkAceAuthorizationProofV1".toByteArray(Charsets.UTF_8)
            val vkRef = "stark-fri:zk_ace_pq_authorization_v0".toByteArray(Charsets.UTF_8)
            val publicInputs = "public-inputs".toByteArray(Charsets.UTF_8)
            return try {
                returnsOutputProbe(PRIVACY_SCHEMA_REQUEST) {
                    nativeProofRequest(
                        algorithmId,
                        entrypoint,
                        vkRef,
                        publicInputs,
                        ByteArray(0),
                        ByteArray(0),
                    )
                }
            } finally {
                algorithmId.fill(0)
                entrypoint.fill(0)
                vkRef.fill(0)
                publicInputs.fill(0)
            }
        }

        internal fun privacyNativeAvailabilityProbeArchive(): ByteArray =
            PRIVACY_NATIVE_AVAILABILITY_PROBE_ARCHIVE.copyOf()

        internal fun returnsOutputProbe(
            expectedSchemaByte: Int,
            probe: () -> ByteArray?,
        ): Boolean {
            try {
                val output = probe() ?: return false
                try {
                    return output.isNotEmpty() &&
                        output.size <= PRIVACY_NATIVE_ARCHIVE_MAX_BYTES &&
                        isValidPrivacyNoritoArchive(output) &&
                        hasNonEmptyPrivacyNoritoPayload(output) &&
                        hasPrivacyNoritoSchema(output, expectedSchemaByte)
                } finally {
                    output.fill(0)
                }
            } catch (_: RuntimeException) {
                return false
            } catch (_: LinkageError) {
                return false
            }
        }

        internal fun isValidPrivacyNoritoArchive(output: ByteArray?): Boolean {
            if (
                output == null ||
                output.size < PRIVACY_NORITO_HEADER_BYTES ||
                output.size > PRIVACY_NATIVE_ARCHIVE_MAX_BYTES
            ) {
                return false
            }
            for (index in PRIVACY_NORITO_MAGIC.indices) {
                if (output[index] != PRIVACY_NORITO_MAGIC[index]) {
                    return false
                }
            }
            if (output[4] != 0.toByte() || output[5] != 0.toByte() || output[22] != 0.toByte()) {
                return false
            }
            val flags = output[39].toInt() and 0xff
            if ((flags and PRIVACY_NORITO_SUPPORTED_FLAGS_MASK.inv()) != 0) {
                return false
            }
            if (
                (flags and PRIVACY_NORITO_FIELD_BITSET_FLAG) != 0 &&
                (flags and PRIVACY_NORITO_FIELD_BITSET_REQUIRED_FLAGS) !=
                PRIVACY_NORITO_FIELD_BITSET_REQUIRED_FLAGS
            ) {
                return false
            }
            val payloadLengthLong = readLongLittleEndian(output, 23)
            if (
                payloadLengthLong < 0 ||
                payloadLengthLong > Int.MAX_VALUE - PRIVACY_NORITO_HEADER_BYTES
            ) {
                return false
            }
            val payloadLength = payloadLengthLong.toInt()
            val minimumLength = PRIVACY_NORITO_HEADER_BYTES + payloadLength
            if (output.size < minimumLength) {
                return false
            }
            val paddingLength = output.size - minimumLength
            if (paddingLength > PRIVACY_NORITO_MAX_HEADER_PADDING_BYTES) {
                return false
            }
            for (index in PRIVACY_NORITO_HEADER_BYTES until PRIVACY_NORITO_HEADER_BYTES + paddingLength) {
                if (output[index] != 0.toByte()) {
                    return false
                }
            }
            val payloadOffset = PRIVACY_NORITO_HEADER_BYTES + paddingLength
            val expectedCrc = readLongLittleEndian(output, 31)
            return privacyCrc64(output, payloadOffset, output.size - payloadOffset) == expectedCrc
        }

        internal fun hasNonEmptyPrivacyNoritoPayload(output: ByteArray?): Boolean =
            output != null && isValidPrivacyNoritoArchive(output) && readLongLittleEndian(output, 23) > 0

        private fun expectedPrivacyResultSchema(label: String): Int? =
            when (label) {
                "privacy capabilities" -> PRIVACY_SCHEMA_CAPABILITIES_RESULT
                "privacy proof request" -> PRIVACY_SCHEMA_REQUEST
                "privacy build proof" -> PRIVACY_SCHEMA_BUILD_PROOF_RESULT
                "privacy verify proof" -> PRIVACY_SCHEMA_VERIFY_PROOF_RESULT
                else -> null
            }

        private fun privacyRequestTextBytes(value: String?, name: String): ByteArray {
            require(value != null) { "$name must not be null" }
            val bytes = value.toByteArray(Charsets.UTF_8)
            require(bytes.size <= PRIVACY_REQUEST_TEXT_FIELD_MAX_BYTES) {
                "$name must not exceed $PRIVACY_REQUEST_TEXT_FIELD_MAX_BYTES bytes"
            }
            return bytes
        }

        private fun privacyRequestComponentBytes(
            value: ByteArray?,
            name: String,
            maxBytes: Int,
            allowEmpty: Boolean,
        ): ByteArray {
            require(value != null) { "$name must not be null" }
            require(allowEmpty || value.isNotEmpty()) { "$name must not be empty" }
            require(value.size <= maxBytes) { "$name must not exceed $maxBytes bytes" }
            return value.copyOf()
        }

        internal fun hasPrivacyNoritoSchema(output: ByteArray, expectedSchemaByte: Int): Boolean {
            val expected = expectedSchemaByte
            val expectedByte = expected.toByte()
            for (index in 6 until 22) {
                if (output[index] != expectedByte) {
                    return false
                }
            }
            return true
        }

        private fun buildPrivacyCrc64Table(): LongArray =
            LongArray(256) { index ->
                var crc = index.toLong()
                repeat(8) {
                    crc = if ((crc and 1L) != 0L) {
                        (crc ushr 1) xor PRIVACY_CRC64_REFLECTED_POLY
                    } else {
                        crc ushr 1
                    }
                }
                crc
            }

        private fun buildPrivacyNativeAvailabilityProbeArchive(): ByteArray =
            ByteArray(PRIVACY_NORITO_HEADER_BYTES).also { archive ->
                PRIVACY_NORITO_MAGIC.copyInto(archive)
                archive.fill(PRIVACY_SCHEMA_REQUEST.toByte(), 6, 22)
            }

        private fun privacyCrc64(output: ByteArray, offset: Int, length: Int): Long {
            var crc = -1L
            for (index in offset until offset + length) {
                crc = PRIVACY_CRC64_TABLE[((crc.toInt() xor output[index].toInt()) and 0xff)] xor (crc ushr 8)
            }
            return crc xor -1L
        }

        private fun readLongLittleEndian(output: ByteArray, offset: Int): Long {
            var value = 0L
            for (index in 0 until 8) {
                value = value or ((output[offset + index].toLong() and 0xffL) shl (8 * index))
            }
            return value
        }

        internal fun detectNativeAvailability(
            loadLibrary: () -> Unit,
            bridgeAbiVersion: () -> Int,
            probeSymbol: () -> Boolean,
        ): Boolean {
            try {
                loadLibrary()
            } catch (_: RuntimeException) {
                return false
            } catch (_: LinkageError) {
                return false
            }
            val abiVersion = try {
                bridgeAbiVersion()
            } catch (_: RuntimeException) {
                return false
            } catch (_: LinkageError) {
                return false
            }
            if (abiVersion < REQUIRED_BRIDGE_ABI_VERSION) {
                return false
            }
            return try {
                probeSymbol()
            } catch (_: RuntimeException) {
                false
            } catch (_: LinkageError) {
                false
            }
        }

        internal fun privacyCapabilities(bridgeAvailable: Boolean): PrivacyCapabilities {
            if (!bridgeAvailable || !nativeAvailable) {
                return PrivacyCapabilities.failClosed(bridgeAvailable)
            }
            return privacyCapabilitiesFromNative(bridgeAvailable) { nativeCapabilities() }
        }

        internal fun privacyCapabilitiesFromNative(
            bridgeAvailable: Boolean,
            nativeCapabilitiesOutput: () -> ByteArray?,
        ): PrivacyCapabilities =
            try {
                val archive = requireNativeOutput(
                    invokeNativeOutput("privacy capabilities", nativeCapabilitiesOutput),
                    "privacy capabilities",
                    PRIVACY_SCHEMA_CAPABILITIES_RESULT,
                )
                privacyCapabilitiesFromArchive(archive, bridgeAvailable)
            } catch (_: RuntimeException) {
                PrivacyCapabilities.failClosed(bridgeAvailable)
            } catch (_: LinkageError) {
                PrivacyCapabilities.failClosed(bridgeAvailable)
            }

        internal fun privacyCapabilitiesFromArchive(
            archive: ByteArray,
            bridgeAvailable: Boolean = true,
        ): PrivacyCapabilities =
            try {
                val native = decodeNativeCapabilities(archive)
                PrivacyCapabilities.fromNative(native, bridgeAvailable)
            } catch (_: RuntimeException) {
                PrivacyCapabilities.failClosed(bridgeAvailable)
            }

        private fun decodeNativeCapabilities(archive: ByteArray): NativeCapabilities =
            nativeCapabilitiesAdapter.decodeArchive(archive)

        @Suppress("UNCHECKED_CAST")
        private fun <T> Any.decodeArchive(archive: ByteArray): T =
            NoritoCodec.decode(archive, this as org.hyperledger.iroha.sdk.norito.TypeAdapter<T>, null)

        private fun Map<String, Any?>.stringField(name: String): String =
            this[name] as? String ?: throw IllegalArgumentException("missing native capability string field $name")

        private fun Map<String, Any?>.booleanField(name: String): Boolean =
            this[name] as? Boolean ?: throw IllegalArgumentException("missing native capability bool field $name")

        private fun Map<String, Any?>.uintField(name: String): Int {
            val value = this[name] as? Long
                ?: throw IllegalArgumentException("missing native capability uint field $name")
            require(value in 0..Int.MAX_VALUE.toLong()) {
                "native capability uint field $name is out of range"
            }
            return value.toInt()
        }

        @Suppress("UNCHECKED_CAST")
        private fun <T> Map<String, Any?>.listField(name: String): List<T> =
            (this[name] as? List<T>)
                ?: throw IllegalArgumentException("missing native capability list field $name")

        @Suppress("UNCHECKED_CAST")
        private fun <T> Map<String, Any?>.objectField(name: String): T =
            this[name] as? T
                ?: throw IllegalArgumentException("missing native capability object field $name")

        @JvmStatic
        private external fun nativeBridgeAbiVersion(): Int

        @JvmStatic
        private external fun nativeCapabilities(): ByteArray?

        @JvmStatic
        private external fun nativeProofRequest(
            algorithmId: ByteArray,
            entrypoint: ByteArray,
            vkRef: ByteArray,
            publicInputs: ByteArray,
            witness: ByteArray,
            proof: ByteArray,
        ): ByteArray?

        @JvmStatic
        private external fun nativeBuildProof(requestArchive: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeVerifyProof(requestArchive: ByteArray): ByteArray?
    }

    class PrivacyCapabilities private constructor(
        val kotlinSdkAvailable: Boolean,
        val bridgeAvailable: Boolean,
        val productionReady: Boolean,
        val productionGate: PrivacyProductionGate,
    ) {
        companion object {
            internal fun failClosed(bridgeAvailable: Boolean): PrivacyCapabilities =
                PrivacyCapabilities(
                    kotlinSdkAvailable = true,
                    bridgeAvailable = bridgeAvailable,
                    productionReady = false,
                    productionGate = PrivacyProductionGate.failClosed(),
                )

            internal fun fromNative(
                native: NativeCapabilities,
                bridgeAvailable: Boolean,
            ): PrivacyCapabilities {
                if (
                    native.version != PRIVACY_FFI_VERSION_V1 ||
                    native.gateVersion != PRODUCTION_GATE_VERSION
                ) {
                    return failClosed(bridgeAvailable)
                }
                val algorithmsById = native.algorithms.associateBy { it.algorithmId }
                if (algorithmsById.size != native.algorithms.size) {
                    return failClosed(bridgeAvailable)
                }
                val confidentialTransfer = algorithmsById[CONFIDENTIAL_TRANSFER_ALGORITHM_ID]
                    ?: return failClosed(bridgeAvailable)
                val unshield = algorithmsById[UNSHIELD_ALGORITHM_ID]
                    ?: return failClosed(bridgeAvailable)
                val productionGate = PrivacyProductionGate.fromNativeRows(
                    listOf(confidentialTransfer, unshield),
                )
                return PrivacyCapabilities(
                    kotlinSdkAvailable = true,
                    bridgeAvailable = bridgeAvailable,
                    productionReady = productionGate.ready,
                    productionGate = productionGate,
                )
            }
        }
    }

    class PrivacyProductionGate private constructor(
        val version: String,
        val ready: Boolean,
        val realProving: Boolean,
        val realVerification: Boolean,
        val chainAdmission: Boolean,
        val sdkParity: Boolean,
        val walletState: Boolean,
        val witnessPrivacyChecks: Boolean,
        val deterministicTests: Boolean,
        val negativeAdversarialTests: Boolean,
        val replayNullifierTests: Boolean,
        val fuzzing: Boolean,
        val parserFuzzing: Boolean,
        val verifierFuzzing: Boolean,
        val performanceGates: Boolean,
        val externalAudit: Boolean,
        val requiredGates: List<String>,
        val missing: List<String>,
        val auditReferences: List<String>,
    ) {
        companion object {
            @JvmField
            val MISSING_REASONS: List<String> =
                Collections.unmodifiableList(
                    listOf(
                        "real proving engine is not registered",
                        "real verifier is not registered",
                        "chain admission path is not enabled",
                        "cross-SDK parity is incomplete",
                        "wallet/state support is incomplete",
                        "witness privacy checks are incomplete",
                        "deterministic tests are incomplete",
                        "negative/adversarial tests are incomplete",
                        "replay/nullifier rejection tests are incomplete",
                        "fuzzing gate is incomplete",
                        "parser fuzzing gate is incomplete",
                        "verifier fuzzing gate is incomplete",
                        "performance gate is incomplete",
                        "internal cryptographic review signoff is missing",
                        "implementation stage is not production-hardened",
                        "planned SDK entrypoints remain",
                        "dev fixture entrypoints are not production entrypoints",
                        "Iroha production allowlist is not enabled for this audited row",
                    ),
                )
            @JvmField
            val REQUIRED_GATES: List<String> =
                Collections.unmodifiableList(
                    listOf(
                        "real_proving",
                        "real_verification",
                        "chain_admission",
                        "sdk_parity",
                        "wallet_state",
                        "witness_privacy_checks",
                        "deterministic_tests",
                        "negative_adversarial_tests",
                        "replay_nullifier_tests",
                        "fuzzing",
                        "parser_fuzzing",
                        "verifier_fuzzing",
                        "performance_gates",
                        "external_audit",
                    ),
                )
            private val READY_AUDIT_REFERENCE_PREFIXES: List<String> =
                Collections.unmodifiableList(
                    listOf(
                        "chain_id:",
                        "reviewer:",
                        "review_artifact_hash:",
                        "review_artifact_signature:",
                        "fuzz_artifact_hash:",
                        "performance_artifact_hash:",
                        "localnet_run_id:",
                        "localnet_smoke_tx_hash:",
                        "localnet_replay_rejection_hash:",
                        "localnet_restart_replay_rejection_hash:",
                        "localnet_state_recovery_hash:",
                        "localnet_lifecycle_shield_tx_hash:",
                        "localnet_lifecycle_hop_proof_hash:",
                        "localnet_lifecycle_recursive_init_hash:",
                        "localnet_lifecycle_recursive_init_verify_hash:",
                        "localnet_lifecycle_recursive_append_hash:",
                        "localnet_lifecycle_recursive_append_verify_hash:",
                        "localnet_lifecycle_unshield_proof_hash:",
                        "localnet_lifecycle_redeem_tx_hash:",
                    ),
                )
            private val READY_HASH_REFERENCE_PREFIXES: Set<String> =
                READY_AUDIT_REFERENCE_PREFIXES
                    .filter { it.endsWith("_hash:") || it.endsWith("_tx_hash:") || it.endsWith("_proof_hash:") }
                    .toSet()
            private val EMPTY_AUDIT_REFERENCES: List<String> =
                Collections.unmodifiableList(emptyList())

            @JvmStatic
            fun failClosed(): PrivacyProductionGate =
                PrivacyProductionGate(
                    version = PRODUCTION_GATE_VERSION,
                    ready = false,
                    realProving = false,
                    realVerification = false,
                    chainAdmission = false,
                    sdkParity = false,
                    walletState = false,
                    witnessPrivacyChecks = false,
                    deterministicTests = false,
                    negativeAdversarialTests = false,
                    replayNullifierTests = false,
                    fuzzing = false,
                    parserFuzzing = false,
                    verifierFuzzing = false,
                    performanceGates = false,
                    externalAudit = false,
                    requiredGates = REQUIRED_GATES,
                    missing = MISSING_REASONS,
                    auditReferences = EMPTY_AUDIT_REFERENCES,
                )

            internal fun fromNativeRows(rows: List<NativeCapability>): PrivacyProductionGate {
                if (
                    rows.isEmpty() ||
                    rows.any { !nativeCapabilityRowIsExact(it) }
                ) {
                    return failClosed()
                }
                val ready = rows.all { row ->
                    row.productionReady &&
                        row.plannedEntrypoints.isEmpty() &&
                        row.productionGate.ready
                }
                val auditReferences = stableDistinct(rows.flatMap { it.productionGate.auditReferences })
                val aggregateReady = ready &&
                    REQUIRED_GATES.all { key -> nativeGatePassed(rows, key) } &&
                    auditReferences.isNotEmpty()
                val missing = stableDistinct(rows.flatMap { it.productionGate.missing })
                return PrivacyProductionGate(
                    version = PRODUCTION_GATE_VERSION,
                    ready = aggregateReady,
                    realProving = nativeGatePassed(rows, "real_proving"),
                    realVerification = nativeGatePassed(rows, "real_verification"),
                    chainAdmission = nativeGatePassed(rows, "chain_admission"),
                    sdkParity = nativeGatePassed(rows, "sdk_parity"),
                    walletState = nativeGatePassed(rows, "wallet_state"),
                    witnessPrivacyChecks = nativeGatePassed(rows, "witness_privacy_checks"),
                    deterministicTests = nativeGatePassed(rows, "deterministic_tests"),
                    negativeAdversarialTests = nativeGatePassed(rows, "negative_adversarial_tests"),
                    replayNullifierTests = nativeGatePassed(rows, "replay_nullifier_tests"),
                    fuzzing = nativeGatePassed(rows, "fuzzing"),
                    parserFuzzing = nativeGatePassed(rows, "parser_fuzzing"),
                    verifierFuzzing = nativeGatePassed(rows, "verifier_fuzzing"),
                    performanceGates = nativeGatePassed(rows, "performance_gates"),
                    externalAudit = nativeGatePassed(rows, "external_audit"),
                    requiredGates = REQUIRED_GATES,
                    missing = if (aggregateReady) EMPTY_AUDIT_REFERENCES else stableDistinct(
                        missing.ifEmpty { MISSING_REASONS },
                    ),
                    auditReferences = stableDistinct(auditReferences),
                )
            }

            private fun nativeGatePassed(rows: List<NativeCapability>, key: String): Boolean =
                rows.all { row ->
                    key !in row.productionGate.requiredGates ||
                        row.productionGate.gates.any { status ->
                            status.key == key && status.passed
                        }
                }

            private fun stableDistinct(values: List<String>): List<String> =
                Collections.unmodifiableList(values.distinct())

            private fun nativeCapabilityRowIsExact(row: NativeCapability): Boolean {
                val gate = row.productionGate
                if (
                    gate.version != PRODUCTION_GATE_VERSION ||
                    row.productionReady != gate.ready ||
                    gate.requiredGates != REQUIRED_GATES ||
                    gate.gates.map { it.key } != REQUIRED_GATES ||
                    gate.gates.any { it.passed != gate.ready }
                ) {
                    return false
                }

                return if (gate.ready) {
                    row.plannedEntrypoints.isEmpty() &&
                        gate.missing.isEmpty() &&
                        readyAuditReferencesAreExact(gate.auditReferences)
                } else {
                    gate.auditReferences.isEmpty() && gate.missing.isNotEmpty()
                }
            }

            private fun readyAuditReferencesAreExact(references: List<String>): Boolean {
                if (
                    references.size != READY_AUDIT_REFERENCE_PREFIXES.size ||
                    references.distinct().size != references.size
                ) {
                    return false
                }

                return references.zip(READY_AUDIT_REFERENCE_PREFIXES).all { (reference, prefix) ->
                    reference.startsWith(prefix) &&
                        productionEvidenceTextIsClean(reference) &&
                        when (prefix) {
                            "review_artifact_signature:" ->
                                productionSignatureIsValid(reference.removePrefix(prefix))
                            in READY_HASH_REFERENCE_PREFIXES ->
                                productionHashIsValid(reference.removePrefix(prefix))
                            else -> true
                        }
                }
            }

            private fun productionHashIsValid(value: String): Boolean =
                value.startsWith("sha256:") &&
                    value.length == "sha256:".length + 64 &&
                    value.removePrefix("sha256:").all { it in '0'..'9' || it in 'a'..'f' }

            private fun productionSignatureIsValid(value: String): Boolean =
                value.startsWith("ed25519:") &&
                    value.length == "ed25519:".length + 128 &&
                    value.removePrefix("ed25519:").all { it in '0'..'9' || it in 'a'..'f' }

            private fun productionEvidenceTextIsClean(value: String): Boolean {
                if (
                    value.isEmpty() ||
                    value.length > 768 ||
                    value.trim() != value ||
                    value.any { it.code !in 0x20..0x7e || it == '\\' }
                ) {
                    return false
                }
                val compact = value.filter { it.isLetterOrDigit() }.lowercase()
                return !compact.contains("devfixture") &&
                    !compact.contains("devprooffixture") &&
                    !compact.contains("localonly") &&
                    !compact.contains("mock")
            }
        }
    }
}
