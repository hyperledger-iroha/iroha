package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.charset.CharacterCodingException
import java.nio.charset.CodingErrorAction
import java.security.MessageDigest

/** Exact ABI-18 Kagemusha recursive-spend bridge. */
class KagemushaRecursiveSpendProver private constructor() {
    enum class Mode(val wireName: String) {
        RECURSIVE_SPEND_V1("recursive_spend_v1"),
    }

    companion object {
        const val REQUIRED_NATIVE_BRIDGE_ABI_VERSION: Int = 18
        const val RECURSIVE_COMPACT_REQUIRED_NATIVE_BRIDGE_ABI_VERSION: Int = 7
        const val TOP_UP_REQUIRED_NATIVE_BRIDGE_ABI_VERSION: Int = 15
        const val PASTA_CYCLE_V3_REQUIRED_NATIVE_BRIDGE_ABI_VERSION: Int = 18
        const val PASTA_CYCLE_V3_ARTIFACT_MANIFEST_SCHEMA: String =
            "kagemusha.offline.recursive_spend.artifact_manifest.v3"
        const val MODE: String = "recursive_spend_v1"
        const val PASTA_CYCLE_V3_MODE: String = MODE
        const val PASTA_CYCLE_V3_PROOF_BACKEND: String = "halo2/ipa-pasta-cycle-v1"
        const val PASTA_CYCLE_V3_TRANSCRIPT_PROFILE: String =
            "kagemusha-pasta-cycle-poseidon-v1"
        const val PASTA_CYCLE_V3_TRANSITION_CIRCUIT_ID: String =
            "kagemusha-recursive-spend-transition-eq-v1"
        const val PASTA_CYCLE_V3_STATE_CIRCUIT_ID: String =
            "kagemusha-recursive-spend-state-ep-v1"
        const val PASTA_CYCLE_V3_MAX_PROOF_BYTES: Int = 4_096
        const val MAX_MANIFEST_BYTES: Int = 1024 * 1024
        const val RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 =
            "kagemusha-recursive-aggregation-v1"
        const val RECURSIVE_COMPACT_CIRCUIT_ID_V1 =
            "kagemusha-recursive-compact-v1"
        const val RECURSIVE_AGGREGATION_PROOF_BACKEND =
            "halo2/ipa"
        const val RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1 =
            "kagemusha-recursive-spend-lineage-onehop-v1"
        const val RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1 =
            "kagemusha-recursive-spend-lineage-append-v1"
        const val COMPACT_TOKEN_MAX_HOPS: Int = 64
        const val RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1: Int = 64
        /** Reserved-lineage transition proofs remain fail-closed until the verifier is wired. */
        const val RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1: Boolean = false
        const val RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1: Int = 1
        const val RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES: Int = 8 * 1024 * 1024
        const val RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES: Int = 128
        const val NATIVE_ARCHIVE_MAX_BYTES: Int = 256 * 1024 * 1024
        const val RECURSIVE_SPEND_ACCUMULATOR_DOMAIN: String =
            "iroha:kagemusha:v1:recursive-spend-accumulator"
        const val RECURSIVE_SPEND_TRANSITION_PROFILE_DOMAIN: String =
            "iroha:kagemusha:v1:recursive-spend-transition-profile"
        const val RECURSIVE_SPEND_TRANSITION_PROFILE_DIGEST_DOMAIN: String =
            "iroha:kagemusha:v1:recursive-spend-transition-profile-digest"
        const val RECURSIVE_SPEND_TRANSITION_PROFILE_BINDING_DIGEST_DOMAIN: String =
            "iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest"
        const val RECURSIVE_SPEND_LINEAGE_APPEND_OPENINGS_PREFLIGHT_DOMAIN_V1: String =
            "iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1"
        const val RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_DOMAIN_V1: String =
            "iroha:kagemusha:recursive-spend-lineage-append-boundary:v1"
        const val RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_CHAIN_ASSET_BINDING_DOMAIN_V1: String =
            "iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1"
        const val RECURSIVE_SPEND_LINEAGE_APPEND_BOUNDARY_FINAL_NOTE_BINDING_DOMAIN_V1: String =
            "iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1"

        private const val LIBRARY_NAME = "connect_norito_bridge"
        private val MALFORMED_NATIVE_PROBE_ARCHIVE = byteArrayOf(0x00)
        private val KAGEMUSHA_ZK1_MAGIC = byteArrayOf(0x5a, 0x4b, 0x31, 0x00)
        private val KAGEMUSHA_ZK1_TLV_CID1 = "CID1".toByteArray(Charsets.US_ASCII)
        private val KAGEMUSHA_ZK1_TLV_IPAK = "IPAK".toByteArray(Charsets.US_ASCII)
        private val KAGEMUSHA_ZK1_TLV_H2VK = "H2VK".toByteArray(Charsets.US_ASCII)
        private const val KAGEMUSHA_NORITO_COMPACT_LEN_FLAG = 0x02
        private const val KAGEMUSHA_NORITO_PACKED_STRUCT_FLAG = 0x04
        private const val PRIVACY_NORITO_FIELD_BITSET_FLAG = 0x20
        private const val KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_VERSION_V1 = 1
        private val KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH =
            byteArrayOf(
                0xc8.toByte(), 0x84.toByte(), 0x89.toByte(), 0x61.toByte(),
                0x8a.toByte(), 0x01, 0x2c, 0x28,
                0x3f, 0xf3.toByte(), 0xbb.toByte(), 0x2e.toByte(),
                0xba.toByte(), 0xbc.toByte(), 0x77, 0x75,
            )
        private val nativeAvailable: Boolean = loadLibrary()
        private val nativeTopUpAvailable: Boolean =
            nativeAvailable &&
                detectNativeAvailability(
                    loadLibrary = {},
                    nativeBridgeAbiVersionProbe = { nativeBridgeAbiVersion() },
                    probeSymbol = {
                        expectIllegalArgumentProbe {
                            nativeTopUpInstruction(MALFORMED_NATIVE_PROBE_ARCHIVE)
                        }
                    },
                    requiredNativeBridgeAbiVersion = REQUIRED_NATIVE_BRIDGE_ABI_VERSION,
                )
        private val pastaCycleV3ArtifactIngestAvailable: Boolean =
            loadPastaCycleV3ArtifactIngestBridge()
        private val pastaCycleV3BackendAvailable: Boolean = loadPastaCycleV3Backend()

        private class LineageProvingKeyArchive(
            val version: Int,
            val circuitFamily: String,
            val verifierKeyCommitment: ByteArray,
            val provingKey: ByteArray,
        )

        private class NoritoField(
            val payload: ByteArray,
            val offset: Int,
        )

        private class NoritoLength(
            val value: Int,
            val offset: Int,
        )

        @JvmStatic
        fun isNativeAvailable(): Boolean = nativeAvailable

        @JvmStatic
        fun isTopUpNativeAvailable(): Boolean = nativeTopUpAvailable

        /** Exact ABI-18/V3 proof capability; legacy ABI-6 symbol availability is insufficient. */
        @JvmStatic
        fun isPastaCycleV3BackendAvailable(): Boolean = pastaCycleV3BackendAvailable

        /** Exact ABI-18 streaming artifact-ingest surface; independent of backend readiness. */
        @JvmStatic
        fun isPastaCycleV3ArtifactIngestAvailable(): Boolean =
            pastaCycleV3ArtifactIngestAvailable

        @JvmStatic
        fun preferredMode(): Mode? = preferredMode(pastaCycleV3BackendAvailable)

        @JvmStatic
        fun preferredMode(pastaCycleV3BackendAvailable: Boolean): Mode? =
            if (pastaCycleV3BackendAvailable) Mode.RECURSIVE_SPEND_V1 else null

        /** True only for the sole first-release spend-again product selector. */
        @JvmStatic
        fun isSpendAgainMode(mode: String?): Boolean = mode == MODE

        /** Begin one manifest-bound artifact spool for an atomic six-artifact install. */
        @JvmStatic
        fun beginArtifactIngest(
            manifestNorito: ByteArray?,
            manifestSha256: ByteArray?,
            artifactSha256: ByteArray?,
        ): ArtifactIngest {
            val manifest =
                requireArtifactInput(
                    manifestNorito,
                    "manifestNorito",
                    digest = false,
                    maxBytes = MAX_MANIFEST_BYTES,
                )
            val manifestDigest =
                requireArtifactInput(manifestSha256, "manifestSha256", digest = true)
            val artifactDigest =
                requireArtifactInput(artifactSha256, "artifactSha256", digest = true)
            requireArtifactBridge()
            val handle = nativeArtifactBeginV3(manifest, manifestDigest, artifactDigest)
            check(handle > 0) { "native Kagemusha V3 artifact ingest returned no handle" }
            return ArtifactIngest(handle)
        }

        /** Begin one all-or-nothing installation of the manifest's six artifact roles. */
        @JvmStatic
        fun beginArtifactInstallSession(
            manifestNorito: ByteArray?,
            manifestSha256: ByteArray?,
        ): ArtifactInstallSession {
            val manifest =
                requireArtifactInput(
                    manifestNorito,
                    "manifestNorito",
                    digest = false,
                    maxBytes = MAX_MANIFEST_BYTES,
                )
            val manifestDigest =
                requireArtifactInput(manifestSha256, "manifestSha256", digest = true)
            requireArtifactBridge()
            return ArtifactInstallSession(manifest, manifestDigest)
        }

        private fun requireArtifactInput(
            bytes: ByteArray?,
            name: String,
            digest: Boolean,
            maxBytes: Int? = null,
        ): ByteArray {
            require(bytes != null && bytes.isNotEmpty()) { "$name must not be empty" }
            require(!digest || bytes.size == 32) { "$name must be exactly 32 bytes" }
            require(!digest || bytes.any { it.toInt() != 0 }) { "$name must not be all zero" }
            require(maxBytes == null || bytes.size <= maxBytes) {
                "$name must not exceed $maxBytes bytes"
            }
            return bytes.copyOf()
        }

        private fun requireArtifactBridge() {
            check(pastaCycleV3ArtifactIngestAvailable) {
                "$LIBRARY_NAME exact ABI-18 artifact ingest is not available in this runtime"
            }
        }

        @JvmStatic
        fun canRedeemWitnessless(circuitId: String?, hopCount: Int): Boolean {
            val hopCountSupported =
                hopCount >= 1 &&
                    hopCount <= RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1
            val oneHopLineage =
                circuitId == RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1 && hopCountSupported
            val appendLineage =
                circuitId == RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1 && hopCountSupported
            return RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1 &&
                (oneHopLineage || appendLineage)
        }

        @JvmStatic
        fun isLineageProofCircuitId(circuitId: String?): Boolean =
            circuitId == RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1 ||
                circuitId == RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1

        @JvmStatic
        fun isLineageAppendOutputCircuitId(outputCircuitId: String?): Boolean =
            outputCircuitId == RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1

        @JvmStatic
        fun isSupportedLineageKeyArtifactOpeningLen(verifierOpeningLen: Int): Boolean =
            when (verifierOpeningLen) {
                2, 4, 8, 16, 32, 64, 128 -> true
                else -> false
            }

        @JvmStatic
        fun lineageKeyArtifactsForInit(
            verifierOpeningLen: Int,
            lineageVerifierKeyBackend: String?,
            lineageVerifierKey: ByteArray?,
            lineageProvingKeyArchive: ByteArray?,
        ): LineageKeyArtifacts =
            lineageKeyArtifacts(
                RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1,
                verifierOpeningLen,
                lineageVerifierKeyBackend,
                lineageVerifierKey,
                lineageProvingKeyArchive,
            )

        @JvmStatic
        fun lineageKeyArtifactsForAppend(
            verifierOpeningLen: Int,
            lineageVerifierKeyBackend: String?,
            lineageVerifierKey: ByteArray?,
            lineageProvingKeyArchive: ByteArray?,
        ): LineageKeyArtifacts =
            lineageKeyArtifacts(
                RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                verifierOpeningLen,
                lineageVerifierKeyBackend,
                lineageVerifierKey,
                lineageProvingKeyArchive,
            )

        @JvmStatic
        fun validateLineageKeyArtifacts(artifacts: LineageKeyArtifacts?): LineageKeyArtifacts {
            require(artifacts != null) { "lineage_key_artifacts" }
            validateLineageKeyArtifactFields(
                artifacts.proofCircuitId,
                artifacts.verifierOpeningLen,
                artifacts.lineageVerifierKeyBackend,
                artifacts.lineageVerifierKey(),
                artifacts.lineageProvingKeyArchive(),
            )
            return artifacts
        }

        private fun validateLineageKeyArtifactFields(
            proofCircuitId: String?,
            verifierOpeningLen: Int,
            lineageVerifierKeyBackend: String?,
            lineageVerifierKey: ByteArray?,
            lineageProvingKeyArchive: ByteArray?,
        ) {
            require(
                proofCircuitId == RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1 ||
                    proofCircuitId == RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
            ) {
                "proof_circuit_id"
            }
            require(isSupportedLineageKeyArtifactOpeningLen(verifierOpeningLen)) {
                "verifier_opening_len"
            }
            require(lineageVerifierKeyBackend == RECURSIVE_AGGREGATION_PROOF_BACKEND) {
                "lineage_verifier_key"
            }
            require(lineageVerifierKey != null && lineageVerifierKey.isNotEmpty()) {
                "lineage_verifier_key"
            }
            require(lineageProvingKeyArchive != null && lineageProvingKeyArchive.isNotEmpty()) {
                "lineage_proving_key_archive"
            }
            validateLineageKeyArtifactPackageBinding(
                proofCircuitId,
                lineageVerifierKeyBackend,
                lineageVerifierKey,
                lineageProvingKeyArchive,
            )
        }

        private fun validateLineageKeyArtifactPackageBinding(
            proofCircuitId: String,
            lineageVerifierKeyBackend: String,
            lineageVerifierKey: ByteArray,
            lineageProvingKeyArchive: ByteArray,
        ) {
            val verifierCircuitId = lineageVerifierKeyEnvelopeCircuitId(lineageVerifierKey)
            require(verifierCircuitId == proofCircuitId) {
                "lineage_verifier_key"
            }
            val archivePayload = lineageProvingKeyArchivePayload(lineageProvingKeyArchive)
            val circuitIdBytes = proofCircuitId.toByteArray(Charsets.UTF_8)
            val verifierKeyCommitment =
                verifyingKeyCommitment(lineageVerifierKeyBackend, lineageVerifierKey)
            require(
                archivePayload.indexOfSlice(circuitIdBytes) >= 0 &&
                    archivePayload.indexOfSlice(verifierKeyCommitment) >= 0,
            ) {
                "lineage_proving_key_archive"
            }
            val archive =
                decodeLineageProvingKeyArchivePayload(
                    archivePayload,
                    lineageProvingKeyArchive[39].toInt() and 0xff,
                )
            require(
                archive.version == KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_VERSION_V1 &&
                    archive.circuitFamily == proofCircuitId &&
                    archive.verifierKeyCommitment.contentEquals(verifierKeyCommitment) &&
                    archive.provingKey.isNotEmpty(),
            ) {
                "lineage_proving_key_archive"
            }
        }

        private fun lineageVerifierKeyEnvelopeCircuitId(lineageVerifierKey: ByteArray): String {
            require(lineageVerifierKey.startsWithBytes(KAGEMUSHA_ZK1_MAGIC)) {
                "lineage_verifier_key"
            }
            var offset = KAGEMUSHA_ZK1_MAGIC.size
            var circuitId: String? = null
            var sawIpaK = false
            var sawH2Vk = false
            while (offset < lineageVerifierKey.size) {
                require(offset + 8 <= lineageVerifierKey.size) {
                    "lineage_verifier_key"
                }
                val tag = lineageVerifierKey.copyOfRange(offset, offset + 4)
                val payloadLength = readIntLittleEndian(lineageVerifierKey, offset + 4)
                val payloadStart = offset + 8
                val payloadEndLong = payloadStart.toLong() + payloadLength.toLong()
                require(payloadLength >= 0 && payloadEndLong <= lineageVerifierKey.size.toLong()) {
                    "lineage_verifier_key"
                }
                val payloadEnd = payloadEndLong.toInt()
                val payload = lineageVerifierKey.copyOfRange(payloadStart, payloadEnd)
                when {
                    tag.contentEquals(KAGEMUSHA_ZK1_TLV_CID1) -> {
                        require(
                            circuitId == null &&
                                payload.isNotEmpty() &&
                                payload.all { byte ->
                                    val value = byte.toInt() and 0xff
                                    value in 0x20..0x7e
                                },
                        ) {
                            "lineage_verifier_key"
                        }
                        val decoded = payload.toString(Charsets.UTF_8)
                        require(decoded.isNotEmpty()) {
                            "lineage_verifier_key"
                        }
                        circuitId = decoded
                    }
                    tag.contentEquals(KAGEMUSHA_ZK1_TLV_IPAK) -> {
                        require(!sawIpaK && payload.size == 4) {
                            "lineage_verifier_key"
                        }
                        sawIpaK = true
                    }
                    tag.contentEquals(KAGEMUSHA_ZK1_TLV_H2VK) -> {
                        require(!sawH2Vk && payload.isNotEmpty()) {
                            "lineage_verifier_key"
                        }
                        sawH2Vk = true
                    }
                    else -> throw IllegalArgumentException("lineage_verifier_key")
                }
                offset = payloadEnd
            }
            require(circuitId != null && sawIpaK && sawH2Vk) {
                "lineage_verifier_key"
            }
            return circuitId
        }

        private fun lineageProvingKeyArchivePayload(lineageProvingKeyArchive: ByteArray): ByteArray {
            require(
                NoritoArchiveValidation.isValidNoritoArchive(lineageProvingKeyArchive) &&
                    NoritoArchiveValidation.hasNonEmptyNoritoPayload(lineageProvingKeyArchive),
            ) {
                "lineage_proving_key_archive"
            }
            require(
                lineageProvingKeyArchive.copyOfRange(6, 22)
                    .contentEquals(KAGEMUSHA_LINEAGE_PROVING_KEY_ARCHIVE_SCHEMA_HASH) &&
                    (lineageProvingKeyArchive[39].toInt() and KAGEMUSHA_NORITO_PACKED_STRUCT_FLAG) == 0 &&
                    (lineageProvingKeyArchive[39].toInt() and PRIVACY_NORITO_FIELD_BITSET_FLAG) == 0,
            ) {
                "lineage_proving_key_archive"
            }
            val payloadLength = readLongLittleEndian(lineageProvingKeyArchive, 23)
            require(payloadLength > 0 && payloadLength <= Int.MAX_VALUE.toLong()) {
                "lineage_proving_key_archive"
            }
            val payloadOffset = lineageProvingKeyArchive.size - payloadLength.toInt()
            return lineageProvingKeyArchive.copyOfRange(payloadOffset, lineageProvingKeyArchive.size)
        }

        private fun decodeLineageProvingKeyArchivePayload(
            payload: ByteArray,
            flags: Int,
        ): LineageProvingKeyArchive {
            var offset = 0
            var field = readNoritoField(payload, offset, flags)
            require(field.payload.size == 2) {
                "lineage_proving_key_archive"
            }
            val version = readUnsignedShortLittleEndian(field.payload, 0)
            offset = field.offset

            field = readNoritoField(payload, offset, flags)
            val circuitFamily = decodeNoritoString(field.payload, flags)
            offset = field.offset

            field = readNoritoField(payload, offset, flags)
            val verifierKeyCommitment = field.payload
            require(verifierKeyCommitment.size == 32) {
                "lineage_proving_key_archive"
            }
            offset = field.offset

            field = readNoritoField(payload, offset, flags)
            val provingKey = decodeNoritoByteVec(field.payload)
            offset = field.offset
            require(offset == payload.size) {
                "lineage_proving_key_archive"
            }
            return LineageProvingKeyArchive(
                version,
                circuitFamily,
                verifierKeyCommitment,
                provingKey,
            )
        }

        private fun readNoritoField(
            buffer: ByteArray,
            offset: Int,
            flags: Int,
        ): NoritoField {
            val length = readNoritoLength(buffer, offset, flags)
            val payloadEnd = length.offset.toLong() + length.value.toLong()
            require(payloadEnd <= buffer.size.toLong()) {
                "lineage_proving_key_archive"
            }
            return NoritoField(
                buffer.copyOfRange(length.offset, payloadEnd.toInt()),
                payloadEnd.toInt(),
            )
        }

        private fun readNoritoLength(
            buffer: ByteArray,
            offset: Int,
            flags: Int,
        ): NoritoLength {
            require(offset >= 0) {
                "lineage_proving_key_archive"
            }
            if ((flags and KAGEMUSHA_NORITO_COMPACT_LEN_FLAG) == 0) {
                require(offset + 8 <= buffer.size) {
                    "lineage_proving_key_archive"
                }
                val value = readLongLittleEndian(buffer, offset)
                require(value >= 0 && value <= Int.MAX_VALUE.toLong() && value <= buffer.size.toLong()) {
                    "lineage_proving_key_archive"
                }
                return NoritoLength(value.toInt(), offset + 8)
            }

            var value = 0L
            var shift = 0
            var cursor = offset
            repeat(10) {
                require(cursor < buffer.size) {
                    "lineage_proving_key_archive"
                }
                val byte = buffer[cursor].toInt() and 0xff
                cursor += 1
                val chunk = (byte and 0x7f).toLong()
                require(shift < 63 || chunk == 0L) {
                    "lineage_proving_key_archive"
                }
                value = value or (chunk shl shift)
                if ((byte and 0x80) == 0) {
                    val encodedLength = cursor - offset
                    require(encodedLength <= 5) {
                        "lineage_proving_key_archive"
                    }
                    require(encodedLength <= 1 || value >= (1L shl (7 * (encodedLength - 1)))) {
                        "lineage_proving_key_archive"
                    }
                    require(value <= Int.MAX_VALUE.toLong() && value <= buffer.size.toLong()) {
                        "lineage_proving_key_archive"
                    }
                    return NoritoLength(value.toInt(), cursor)
                }
                shift += 7
            }
            throw IllegalArgumentException("lineage_proving_key_archive")
        }

        private fun decodeNoritoString(
            payload: ByteArray,
            flags: Int,
        ): String {
            val length = readNoritoLength(payload, 0, flags)
            val end = length.offset + length.value
            require(end == payload.size) {
                "lineage_proving_key_archive"
            }
            return try {
                Charsets.UTF_8
                    .newDecoder()
                    .onMalformedInput(CodingErrorAction.REPORT)
                    .onUnmappableCharacter(CodingErrorAction.REPORT)
                    .decode(ByteBuffer.wrap(payload, length.offset, length.value))
                    .toString()
            } catch (ex: CharacterCodingException) {
                throw IllegalArgumentException("lineage_proving_key_archive", ex)
            }
        }

        private fun decodeNoritoByteVec(payload: ByteArray): ByteArray {
            require(payload.size >= 8) {
                "lineage_proving_key_archive"
            }
            val length = readLongLittleEndian(payload, 0)
            val end = 8L + length
            require(length >= 0 && end == payload.size.toLong()) {
                "lineage_proving_key_archive"
            }
            return payload.copyOfRange(8, end.toInt())
        }

        private fun verifyingKeyCommitment(
            lineageVerifierKeyBackend: String,
            lineageVerifierKey: ByteArray,
        ): ByteArray {
            val backend = lineageVerifierKeyBackend.toByteArray(Charsets.UTF_8)
            val digest = MessageDigest.getInstance("SHA-256")
            digest.update("iroha:zk:v1:vk".toByteArray(Charsets.US_ASCII))
            digest.update(longBigEndian(backend.size.toLong()))
            digest.update(backend)
            digest.update(longBigEndian(lineageVerifierKey.size.toLong()))
            digest.update(lineageVerifierKey)
            return digest.digest()
        }

        private fun readIntLittleEndian(bytes: ByteArray, offset: Int): Int {
            require(offset >= 0 && offset + 4 <= bytes.size) {
                "lineage_verifier_key"
            }
            return (bytes[offset].toInt() and 0xff) or
                ((bytes[offset + 1].toInt() and 0xff) shl 8) or
                ((bytes[offset + 2].toInt() and 0xff) shl 16) or
                ((bytes[offset + 3].toInt() and 0xff) shl 24)
        }

        private fun readUnsignedShortLittleEndian(bytes: ByteArray, offset: Int): Int {
            require(offset >= 0 && offset + 2 <= bytes.size) {
                "lineage_proving_key_archive"
            }
            return (bytes[offset].toInt() and 0xff) or
                ((bytes[offset + 1].toInt() and 0xff) shl 8)
        }

        private fun readLongLittleEndian(bytes: ByteArray, offset: Int): Long {
            var value = 0L
            for (index in 0 until 8) {
                value = value or ((bytes[offset + index].toLong() and 0xffL) shl (index * 8))
            }
            return value
        }

        private fun longBigEndian(value: Long): ByteArray {
            val output = ByteArray(8)
            for (index in output.indices) {
                output[index] = ((value ushr ((7 - index) * 8)) and 0xff).toByte()
            }
            return output
        }

        private fun ByteArray.startsWithBytes(prefix: ByteArray): Boolean =
            size >= prefix.size && copyOfRange(0, prefix.size).contentEquals(prefix)

        private fun ByteArray.indexOfSlice(needle: ByteArray): Int {
            if (needle.isEmpty() || needle.size > size) {
                return -1
            }
            for (offset in 0..(size - needle.size)) {
                var matched = true
                for (index in needle.indices) {
                    if (this[offset + index] != needle[index]) {
                        matched = false
                        break
                    }
                }
                if (matched) {
                    return offset
                }
            }
            return -1
        }

        @JvmStatic
        fun lineageKeyArtifacts(
            proofCircuitId: String?,
            verifierOpeningLen: Int,
            lineageVerifierKeyBackend: String?,
            lineageVerifierKey: ByteArray?,
            lineageProvingKeyArchive: ByteArray?,
        ): LineageKeyArtifacts {
            validateLineageKeyArtifactFields(
                proofCircuitId,
                verifierOpeningLen,
                lineageVerifierKeyBackend,
                lineageVerifierKey,
                lineageProvingKeyArchive,
            )
            return LineageKeyArtifacts(
                proofCircuitId!!,
                verifierOpeningLen,
                lineageVerifierKeyBackend!!,
                lineageVerifierKey!!,
                lineageProvingKeyArchive!!,
            )
        }

        @JvmStatic
        fun requiresLineageKeyArtifactsForInit(): Boolean = false

        @JvmStatic
        fun requiresLineageWitnessForRedeem(circuitId: String?, hopCount: Int): Boolean =
            !canRedeemWitnessless(circuitId, hopCount)

        @JvmStatic
        fun canAppendWitnesslessLineage(previousHopCount: Int): Boolean =
            RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1 &&
                previousHopCount >= 1 &&
                previousHopCount < RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1

        @JvmStatic
        fun normalizeAppendOutputCircuitId(outputCircuitId: String?): String =
            if (outputCircuitId.isNullOrEmpty()) {
                ""
            } else {
                outputCircuitId
            }

        @JvmStatic
        fun isSupportedAppendOutputCircuitId(outputCircuitId: String?): Boolean =
            when (normalizeAppendOutputCircuitId(outputCircuitId)) {
                RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1,
                RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1,
                -> true
                else -> false
            }

        @JvmStatic
        fun requiresLineageKeyArtifactsForAppendOutput(outputCircuitId: String?): Boolean =
            isLineageAppendOutputCircuitId(normalizeAppendOutputCircuitId(outputCircuitId))

        @JvmStatic
        fun isSupportedPreviousProofCircuitId(previousProofCircuitId: String?): Boolean =
            previousProofCircuitId == RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 ||
                isLineageProofCircuitId(previousProofCircuitId)

        @JvmStatic
        fun requiresPreviousLineageVerifierRecordForAppend(previousProofCircuitId: String?): Boolean =
            isLineageProofCircuitId(previousProofCircuitId)

        @JvmStatic
        fun isSupportedAppendProofTransition(
            previousProofCircuitId: String?,
            outputCircuitId: String?,
        ): Boolean {
            val normalizedOutput = normalizeAppendOutputCircuitId(outputCircuitId)
            return previousProofCircuitId == RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 &&
                normalizedOutput == RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 ||
                isLineageProofCircuitId(previousProofCircuitId) &&
                    (
                        normalizedOutput == RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 ||
                            normalizedOutput == RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
                    )
        }

        @JvmStatic
        fun preferredAppendOutputCircuitId(previousHopCount: Int): String =
            if (canAppendWitnesslessLineage(previousHopCount)) {
                RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
            } else {
                RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
            }

        @JvmStatic
        fun canProveAppendOutputCircuitId(outputCircuitId: String?, previousHopCount: Int): Boolean {
            if (previousHopCount < 1) {
                return false
            }
            return when (normalizeAppendOutputCircuitId(outputCircuitId)) {
                RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 ->
                    previousHopCount < COMPACT_TOKEN_MAX_HOPS
                RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1 ->
                    canAppendWitnesslessLineage(previousHopCount)
                else -> false
            }
        }

        @JvmStatic
        fun canSelectAppendOutputCircuitId(
            previousProofCircuitId: String?,
            outputCircuitId: String?,
            previousHopCount: Int,
        ): Boolean {
            if (!canProveAppendOutputCircuitId(outputCircuitId, previousHopCount)) {
                return false
            }
            if (!isSupportedPreviousProofCircuitId(previousProofCircuitId)) {
                return false
            }
            return isSupportedAppendProofTransition(previousProofCircuitId, outputCircuitId)
        }

        @JvmStatic
        fun requiresPreviousProofOpenEnvelopesForAppend(
            outputCircuitId: String?,
            previousHopCount: Int,
        ): Boolean =
            isLineageAppendOutputCircuitId(normalizeAppendOutputCircuitId(outputCircuitId)) &&
                previousHopCount >= 1

        @JvmStatic
        fun initSpend(requestArchive: ByteArray?): ByteArray =
            call("init", requestArchive, ::nativeInitSpend)

        @JvmStatic
        fun initSpend(request: InitSpendRequest): ByteArray =
            initSpend(KagemushaRecursiveSpendRequestCodecs.encodeInitRequest(request))

        @JvmStatic
        fun topUpSpend(requestArchive: ByteArray?): ByteArray =
            call(
                "top-up",
                requestArchive,
                ::nativeTopUpInstruction,
                bridgeAvailable = nativeTopUpAvailable,
            )

        @JvmStatic
        fun topUpSpend(request: TopUpSpendRequest): ByteArray =
            topUpSpend(KagemushaRecursiveSpendRequestCodecs.encodeTopUpRequest(request))

        @JvmStatic
        fun appendSpend(requestArchive: ByteArray?): ByteArray =
            call("append", requestArchive, ::nativeAppendSpend)

        @JvmStatic
        fun appendSpend(request: AppendSpendRequest): ByteArray =
            appendSpend(KagemushaRecursiveSpendRequestCodecs.encodeAppendRequest(request))

        @JvmStatic
        fun transitionProfileInit(requestArchive: ByteArray?): ByteArray =
            call("transition profile init", requestArchive, ::nativeTransitionProfileInit)

        @JvmStatic
        fun transitionProfileAppend(requestArchive: ByteArray?): ByteArray =
            call("transition profile append", requestArchive, ::nativeTransitionProfileAppend)

        @JvmStatic
        fun lineageAppendBoundary(profileArchive: ByteArray?): ByteArray =
            callArchive(
                "lineage append boundary",
                "profileArchive",
                profileArchive,
                ::nativeLineageAppendBoundary,
            )

        @JvmStatic
        fun lineageWitnessFromInitResult(
            requestArchive: ByteArray?,
            bundleArchive: ByteArray?,
        ): ByteArray =
            call(
                "lineage witness from init result",
                requestArchive,
                bundleArchive,
                ::nativeLineageWitnessFromInitResult,
            )

        @JvmStatic
        fun lineageWitnessAppendResult(
            previousWitnessArchive: ByteArray?,
            requestArchive: ByteArray?,
            bundleArchive: ByteArray?,
        ): ByteArray =
            call(
                "lineage witness append result",
                previousWitnessArchive,
                requestArchive,
                bundleArchive,
                ::nativeLineageWitnessAppendResult,
            )

        @JvmStatic
        fun verifySpend(requestArchive: ByteArray?): ByteArray =
            call("verify", requestArchive, ::nativeVerifySpend)

        @JvmStatic
        fun verifySpend(request: VerifySpendRequest): VerifySpendResult =
            KagemushaRecursiveSpendRequestCodecs.decodeVerifyResult(
                verifySpend(KagemushaRecursiveSpendRequestCodecs.encodeVerifyRequest(request)),
            )

        @JvmStatic
        fun redeemSpend(requestArchive: ByteArray?): ByteArray =
            call("redeem", requestArchive, ::nativeRedeemSpend)

        @JvmStatic
        fun redeemSpend(request: RedeemSpendRequest): ByteArray =
            redeemSpend(KagemushaRecursiveSpendRequestCodecs.encodeRedeemRequest(request))

        @JvmStatic
        fun buildPallasOpenEnvelopesArchive(recordBundleArchive: ByteArray?): ByteArray =
            callArchive(
                "build Pallas open envelopes",
                "recordBundleArchive",
                recordBundleArchive,
                ::nativeBuildPallasOpenEnvelopesArchive,
            )

        @JvmStatic
        fun buildPreviousProofOpenEnvelopesArchive(previousBundleArchive: ByteArray?): ByteArray =
            callArchive(
                "build previous proof open envelopes",
                "previousBundleArchive",
                previousBundleArchive,
                ::nativeBuildPreviousProofOpenEnvelopesArchive,
            )

        private fun call(
            label: String,
            requestArchive: ByteArray?,
            nativeCall: (ByteArray) -> ByteArray?,
            bridgeAvailable: Boolean = nativeAvailable,
        ): ByteArray =
            callArchive(label, "requestArchive", requestArchive, nativeCall, bridgeAvailable)

        private fun callArchive(
            label: String,
            archiveName: String,
            archive: ByteArray?,
            nativeCall: (ByteArray) -> ByteArray?,
            bridgeAvailable: Boolean = nativeAvailable,
        ): ByteArray {
            val ownedArchive = ownedNativeInput(archive, archiveName)
            check(bridgeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            val output = nativeCall(ownedArchive)
            return requireRecursiveSpendOutput(output, label)
        }

        private fun call(
            label: String,
            requestArchive: ByteArray?,
            bundleArchive: ByteArray?,
            nativeCall: (ByteArray, ByteArray) -> ByteArray?,
        ): ByteArray {
            val request = ownedNativeInput(requestArchive, "requestArchive")
            val bundle = ownedNativeInput(bundleArchive, "bundleArchive")
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            val output = nativeCall(request, bundle)
            return requireRecursiveSpendOutput(output, label)
        }

        private fun call(
            label: String,
            previousWitnessArchive: ByteArray?,
            requestArchive: ByteArray?,
            bundleArchive: ByteArray?,
            nativeCall: (ByteArray, ByteArray, ByteArray) -> ByteArray?,
        ): ByteArray {
            val previousWitness = ownedNativeInput(previousWitnessArchive, "previousWitnessArchive")
            val request = ownedNativeInput(requestArchive, "requestArchive")
            val bundle = ownedNativeInput(bundleArchive, "bundleArchive")
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            val output = nativeCall(previousWitness, request, bundle)
            return requireRecursiveSpendOutput(output, label)
        }

        internal fun requireRecursiveSpendOutput(output: ByteArray?, label: String): ByteArray =
            requireNativeOutput(output, "native $label")

        internal fun ownedNativeInput(archiveInput: ByteArray?, archiveName: String): ByteArray {
            val archive = requireNativeInput(archiveInput, archiveName)
            return archive.copyOf()
        }

        private fun requireNativeInput(archive: ByteArray?, archiveName: String): ByteArray {
            require(archive != null && archive.isNotEmpty()) { "$archiveName must not be empty" }
            require(archive.size <= NATIVE_ARCHIVE_MAX_BYTES) {
                "$archiveName must not exceed $NATIVE_ARCHIVE_MAX_BYTES bytes"
            }
            require(NoritoArchiveValidation.isValidNoritoArchive(archive)) {
                "$archiveName must be a valid Norito archive"
            }
            require(NoritoArchiveValidation.hasNonEmptyNoritoPayload(archive)) {
                "$archiveName must contain a non-empty Norito payload"
            }
            return archive
        }

        private fun requireNativeOutput(output: ByteArray?, label: String): ByteArray {
            check(output != null) { "$label returned no output" }
            check(output.isNotEmpty()) { "$label returned empty output" }
            check(output.size <= NATIVE_ARCHIVE_MAX_BYTES) { "$label returned oversized output" }
            check(NoritoArchiveValidation.isValidNoritoArchive(output)) {
                "$label returned invalid Norito archive"
            }
            check(NoritoArchiveValidation.hasNonEmptyNoritoPayload(output)) {
                "$label returned empty Norito payload"
            }
            return output
        }

        private fun loadLibrary(): Boolean =
            detectNativeAvailability(
                loadLibrary = { System.loadLibrary(LIBRARY_NAME) },
                nativeBridgeAbiVersionProbe = { nativeBridgeAbiVersion() },
                probeSymbol = { probeRequiredNativeSymbols() },
            )

        private fun loadPastaCycleV3ArtifactIngestBridge(): Boolean {
            return detectExactNativeAvailability(
                loadLibrary = { System.loadLibrary(LIBRARY_NAME) },
                nativeBridgeAbiVersionProbe = { nativeBridgeAbiVersion() },
                probeSymbol = {
                    expectIllegalArgumentProbe {
                        nativeArtifactBeginV3(byteArrayOf(0), ByteArray(32), ByteArray(32))
                    }
                },
                expectedNativeBridgeAbiVersion =
                    PASTA_CYCLE_V3_REQUIRED_NATIVE_BRIDGE_ABI_VERSION,
            )
        }

        private fun loadPastaCycleV3Backend(): Boolean =
            detectExactNativeAvailability(
                loadLibrary = { System.loadLibrary(LIBRARY_NAME) },
                nativeBridgeAbiVersionProbe = { nativeBridgeAbiVersion() },
                probeSymbol = { nativePastaCycleV3BackendAvailable() },
                expectedNativeBridgeAbiVersion =
                    PASTA_CYCLE_V3_REQUIRED_NATIVE_BRIDGE_ABI_VERSION,
            )

        private fun probeRequiredNativeSymbols(): Boolean {
            val probe = MALFORMED_NATIVE_PROBE_ARCHIVE
            var available = true
            available = expectIllegalArgumentProbe { nativeInitSpend(probe) } && available
            available = expectIllegalArgumentProbe { nativeAppendSpend(probe) } && available
            available =
                expectIllegalArgumentProbe { nativeTransitionProfileInit(ByteArray(0)) } && available
            available =
                expectIllegalArgumentProbe { nativeTransitionProfileAppend(ByteArray(0)) } && available
            available =
                expectIllegalArgumentProbe { nativeLineageAppendBoundary(ByteArray(0)) } && available
            available = expectIllegalArgumentProbe { nativeVerifySpend(probe) } && available
            available = expectIllegalArgumentProbe {
                nativeLineageWitnessFromInitResult(probe, probe)
            } && available
            available = expectIllegalArgumentProbe {
                nativeLineageWitnessAppendResult(probe, probe, probe)
            } && available
            available = expectIllegalArgumentProbe { nativeRedeemSpend(probe) } && available
            available =
                expectIllegalArgumentProbe { nativeBuildPallasOpenEnvelopesArchive(probe) } &&
                    available
            available =
                expectIllegalArgumentProbe {
                    nativeBuildPreviousProofOpenEnvelopesArchive(probe)
                } && available
            return available
        }

        internal fun expectIllegalArgumentProbe(probe: () -> Unit): Boolean =
            try {
                probe()
                false
            } catch (_: IllegalArgumentException) {
                true
            }

        internal fun detectNativeAvailability(
            loadLibrary: () -> Unit,
            nativeBridgeAbiVersionProbe: () -> Int,
            probeSymbol: () -> Boolean,
            requiredNativeBridgeAbiVersion: Int = REQUIRED_NATIVE_BRIDGE_ABI_VERSION,
        ): Boolean {
            if (requiredNativeBridgeAbiVersion <= 0) {
                return false
            }
            try {
                loadLibrary()
            } catch (_: UnsatisfiedLinkError) {
                return false
            } catch (_: RuntimeException) {
                return false
            }
            val abiVersion = try {
                nativeBridgeAbiVersionProbe()
            } catch (_: UnsatisfiedLinkError) {
                return false
            } catch (_: RuntimeException) {
                return false
            }
            if (abiVersion < requiredNativeBridgeAbiVersion) {
                return false
            }
            return try {
                probeSymbol()
            } catch (_: UnsatisfiedLinkError) {
                false
            } catch (_: RuntimeException) {
                false
            }
        }

        internal fun detectExactNativeAvailability(
            loadLibrary: () -> Unit,
            nativeBridgeAbiVersionProbe: () -> Int,
            probeSymbol: () -> Boolean,
            expectedNativeBridgeAbiVersion: Int,
        ): Boolean {
            if (expectedNativeBridgeAbiVersion <= 0) return false
            try {
                loadLibrary()
            } catch (_: UnsatisfiedLinkError) {
                return false
            } catch (_: RuntimeException) {
                return false
            }
            val abiVersion =
                try {
                    nativeBridgeAbiVersionProbe()
                } catch (_: UnsatisfiedLinkError) {
                    return false
                } catch (_: RuntimeException) {
                    return false
                }
            if (abiVersion != expectedNativeBridgeAbiVersion) return false
            return try {
                probeSymbol()
            } catch (_: UnsatisfiedLinkError) {
                false
            } catch (_: RuntimeException) {
                false
            }
        }

        @JvmStatic
        private external fun nativeBridgeAbiVersion(): Int

        @JvmStatic
        private external fun nativePastaCycleV3BackendAvailable(): Boolean

        @JvmStatic
        private external fun nativeArtifactBeginV3(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
            artifactSha256: ByteArray,
        ): Long

        @JvmStatic
        private external fun nativeArtifactWriteV3(handle: Long, chunk: ByteArray)

        @JvmStatic
        private external fun nativeArtifactFinalizeV3(handle: Long)

        @JvmStatic
        private external fun nativeArtifactCancelV3(handle: Long)

        @JvmStatic
        private external fun nativeArtifactSetInstallV3(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
            handles: LongArray,
        )

        @JvmStatic
        private external fun nativeArtifactSetIsInstalledV3(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
        ): Boolean

        @JvmStatic
        private external fun nativeArtifactSetUninstallV3(manifestSha256: ByteArray)

        @JvmStatic
        private external fun nativeInitSpend(requestArchive: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeAppendSpend(requestArchive: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeTopUpInstruction(requestArchive: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeTransitionProfileInit(requestArchive: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeTransitionProfileAppend(requestArchive: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeLineageAppendBoundary(profileArchive: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeLineageWitnessFromInitResult(
            requestArchive: ByteArray,
            bundleArchive: ByteArray,
        ): ByteArray?

        @JvmStatic
        private external fun nativeLineageWitnessAppendResult(
            previousWitnessArchive: ByteArray,
            requestArchive: ByteArray,
            bundleArchive: ByteArray,
        ): ByteArray?

        @JvmStatic
        private external fun nativeVerifySpend(requestArchive: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeRedeemSpend(requestArchive: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeBuildPallasOpenEnvelopesArchive(
            recordBundleArchive: ByteArray,
        ): ByteArray?

        @JvmStatic
        private external fun nativeBuildPreviousProofOpenEnvelopesArchive(
            previousBundleArchive: ByteArray,
        ): ByteArray?
    }

    /** Owns a native KRV3 spool until the caller closes it. */
    class ArtifactIngest internal constructor(initialHandle: Long) : AutoCloseable {
        private var handle: Long = initialHandle
        private var finalized: Boolean = false
        private var installClaimed: Boolean = false

        @Synchronized
        fun write(chunk: ByteArray?) {
            requireOpen()
            val bytes = requireArtifactInput(chunk, "chunk", false)
            nativeArtifactWriteV3(handle, bytes)
        }

        @Synchronized
        fun finish() {
            requireOpen()
            nativeArtifactFinalizeV3(handle)
            finalized = true
        }

        @Synchronized
        fun isFinalized(): Boolean = finalized

        @Synchronized
        override fun close() {
            if (handle == 0L) return
            check(!installClaimed) { "Kagemusha V3 artifact ingest is being installed" }
            val current = handle
            nativeArtifactCancelV3(current)
            handle = 0L
            finalized = false
        }

        @Synchronized
        internal fun claimFinalizedHandle(): Long {
            check(handle != 0L && finalized && !installClaimed) {
                "Kagemusha V3 artifact ingest is not installable"
            }
            installClaimed = true
            return handle
        }

        @Synchronized
        internal fun releaseInstallClaim(expectedHandle: Long) {
            if (handle == expectedHandle && installClaimed) installClaimed = false
        }

        @Synchronized
        internal fun relinquishInstalledHandle(expectedHandle: Long) {
            check(handle == expectedHandle && finalized && installClaimed) {
                "Kagemusha V3 artifact install ownership mismatch"
            }
            handle = 0L
            finalized = false
            installClaimed = false
        }

        private fun requireOpen() {
            check(handle != 0L) { "Kagemusha V3 artifact ingest is closed" }
            check(!finalized) { "Kagemusha V3 artifact ingest is already finalized" }
            check(!installClaimed) { "Kagemusha V3 artifact ingest is being installed" }
        }
    }

    /** Coordinates one atomic six-artifact V3 generation install. */
    class ArtifactInstallSession internal constructor(
        manifestNorito: ByteArray,
        manifestSha256: ByteArray,
    ) : AutoCloseable {
        private val manifestNorito = manifestNorito.copyOf()
        private val manifestSha256 = manifestSha256.copyOf()
        private val artifacts = linkedMapOf<String, ArtifactIngest>()
        private var installed = false
        private var closed = false

        @Synchronized
        fun beginArtifact(expectedArtifactSha256: ByteArray?): ArtifactIngest {
            requirePending()
            check(artifacts.size < 6) { "artifact set already has six streams" }
            val digest =
                requireArtifactInput(
                    expectedArtifactSha256,
                    "expectedArtifactSha256",
                    digest = true,
                )
            val key = digest.joinToString(separator = "") { byte ->
                "%02x".format(byte.toInt() and 0xff)
            }
            require(!artifacts.containsKey(key)) { "expectedArtifactSha256 is duplicated" }
            return beginArtifactIngest(manifestNorito, manifestSha256, digest)
                .also { artifacts[key] = it }
        }

        /** Native failure consumes no handles and preserves the previous generation. */
        @Synchronized
        fun install() {
            requirePending()
            check(artifacts.size == 6) { "artifact set must contain exactly six streams" }
            val ordered = artifacts.values.toList()
            val handles = LongArray(6)
            var claimed = 0
            try {
                while (claimed < ordered.size) {
                    handles[claimed] = ordered[claimed].claimFinalizedHandle()
                    claimed += 1
                }
                nativeArtifactSetInstallV3(manifestNorito, manifestSha256, handles)
            } catch (failure: Throwable) {
                repeat(claimed) { index ->
                    ordered[index].releaseInstallClaim(handles[index])
                }
                throw failure
            }
            ordered.forEachIndexed { index, artifact ->
                artifact.relinquishInstalledHandle(handles[index])
            }
            artifacts.clear()
            installed = true
        }

        @Synchronized
        fun isInstalled(): Boolean {
            if (closed && !installed) return false
            return nativeArtifactSetIsInstalledV3(manifestNorito, manifestSha256)
        }

        /** The digest guard prevents a stale session from removing a newer generation. */
        @Synchronized
        fun uninstall() {
            if (!installed || closed) return
            nativeArtifactSetUninstallV3(manifestSha256)
            installed = false
            closed = true
        }

        /** Cancels pending streams; installed generations require explicit uninstall. */
        @Synchronized
        override fun close() {
            if (closed || installed) return
            var firstFailure: RuntimeException? = null
            artifacts.values.forEach { artifact ->
                try {
                    artifact.close()
                } catch (failure: RuntimeException) {
                    if (firstFailure == null) firstFailure = failure
                }
            }
            artifacts.clear()
            closed = true
            firstFailure?.let { throw it }
        }

        private fun requirePending() {
            check(!closed && !installed) { "artifact install session is not pending" }
        }
    }

    /** Portable Reserved-lineage verifier/proving key artifact package. */
    class LineageKeyArtifacts internal constructor(
        val proofCircuitId: String,
        val verifierOpeningLen: Int,
        val lineageVerifierKeyBackend: String,
        lineageVerifierKey: ByteArray,
        lineageProvingKeyArchive: ByteArray,
    ) {
        private val lineageVerifierKeyBytes = lineageVerifierKey.copyOf()
        private val lineageProvingKeyArchiveBytes = lineageProvingKeyArchive.copyOf()

        fun isInitArtifact(): Boolean =
            proofCircuitId == RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1

        fun isAppendArtifact(): Boolean =
            proofCircuitId == RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1

        fun lineageVerifierKey(): ByteArray = lineageVerifierKeyBytes.copyOf()

        fun lineageProvingKeyArchive(): ByteArray = lineageProvingKeyArchiveBytes.copyOf()
    }
}
