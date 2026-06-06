package org.hyperledger.iroha.sdk.privacy

import java.util.Collections

/** Raw Norito V1 privacy proof bridge backed by `connect_norito_bridge`. */
class PrivacyNativeBridge private constructor() {
    companion object {
        const val REQUIRED_BRIDGE_ABI_VERSION: Int = 6
        const val PRIVACY_FFI_VERSION_V1: Int = 1
        const val PRODUCTION_GATE_VERSION: String = "privacy-production-gate-v1"
        const val STATUS_ERROR: Int = 1
        const val ERROR_NULL_POINTER: Int = 1
        const val ERROR_MALFORMED_NORITO: Int = 2
        const val ERROR_UNSUPPORTED_ALGORITHM: Int = 3
        const val ERROR_PRODUCTION_DISABLED: Int = 4
        const val ERROR_INVALID_REQUEST: Int = 5
        const val PRIVACY_NATIVE_ARCHIVE_MAX_BYTES: Int = 64 * 1024 * 1024

        private const val PRIVACY_NORITO_HEADER_BYTES: Int = 40
        private const val PRIVACY_NORITO_MAX_HEADER_PADDING_BYTES: Int = 64
        private const val PRIVACY_NORITO_SUPPORTED_FLAGS_MASK: Int = 0x27
        private const val PRIVACY_NORITO_FIELD_BITSET_FLAG: Int = 0x20
        private const val PRIVACY_NORITO_FIELD_BITSET_REQUIRED_FLAGS: Int = 0x06
        private const val PRIVACY_SCHEMA_REQUEST: Int = 0x52
        private const val PRIVACY_SCHEMA_CAPABILITIES_RESULT: Int = 0x50
        private const val PRIVACY_SCHEMA_BUILD_PROOF_RESULT: Int = 0x42
        private const val PRIVACY_SCHEMA_VERIFY_PROOF_RESULT: Int = 0x56
        private const val PRIVACY_CRC64_REFLECTED_POLY: Long = -3932672073523589310L
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
        fun buildProof(requestArchive: ByteArray?): ByteArray =
            call("build proof", requestArchive, ::nativeBuildProof)

        @JvmStatic
        fun verifyProof(requestArchive: ByteArray?): ByteArray =
            call("verify proof", requestArchive, ::nativeVerifyProof)

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
            available = returnsOutputProbe(PRIVACY_SCHEMA_BUILD_PROOF_RESULT) {
                nativeBuildProof(privacyNativeAvailabilityProbeArchive())
            } && available
            available = returnsOutputProbe(PRIVACY_SCHEMA_VERIFY_PROOF_RESULT) {
                nativeVerifyProof(privacyNativeAvailabilityProbeArchive())
            } && available
            return available
        }

        internal fun privacyNativeAvailabilityProbeArchive(): ByteArray =
            PRIVACY_NATIVE_AVAILABILITY_PROBE_ARCHIVE.copyOf()

        internal fun returnsOutputProbe(
            expectedSchemaByte: Int,
            probe: () -> ByteArray?,
        ): Boolean =
            try {
                val output = probe()
                output != null &&
                    output.isNotEmpty() &&
                    output.size <= PRIVACY_NATIVE_ARCHIVE_MAX_BYTES &&
                    isValidPrivacyNoritoArchive(output) &&
                    hasNonEmptyPrivacyNoritoPayload(output) &&
                    hasPrivacyNoritoSchema(output, expectedSchemaByte)
            } catch (_: RuntimeException) {
                false
            } catch (_: LinkageError) {
                false
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
                "privacy build proof" -> PRIVACY_SCHEMA_BUILD_PROOF_RESULT
                "privacy verify proof" -> PRIVACY_SCHEMA_VERIFY_PROOF_RESULT
                else -> null
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

        internal fun privacyCapabilities(bridgeAvailable: Boolean): PrivacyCapabilities =
            PrivacyCapabilities.failClosed(bridgeAvailable)

        @JvmStatic
        private external fun nativeBridgeAbiVersion(): Int

        @JvmStatic
        private external fun nativeCapabilities(): ByteArray?

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
        val deterministicTests: Boolean,
        val fuzzing: Boolean,
        val performanceGates: Boolean,
        val externalAudit: Boolean,
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
                        "deterministic tests are incomplete",
                        "fuzzing gate is incomplete",
                        "performance gate is incomplete",
                        "external audit signoff is missing",
                        "implementation stage is not production-hardened",
                        "planned SDK entrypoints remain",
                        "dev fixture entrypoints are not production entrypoints",
                        "Iroha production allowlist is not enabled for this audited row",
                    ),
                )
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
                    deterministicTests = false,
                    fuzzing = false,
                    performanceGates = false,
                    externalAudit = false,
                    missing = MISSING_REASONS,
                    auditReferences = EMPTY_AUDIT_REFERENCES,
                )
        }
    }
}
