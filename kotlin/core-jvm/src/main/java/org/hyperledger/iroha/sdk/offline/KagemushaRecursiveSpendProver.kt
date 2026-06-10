package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.charset.CharacterCodingException
import java.nio.charset.CodingErrorAction
import java.security.MessageDigest

/** Native recursive Kagemusha spend ABI-6 bridge. */
class KagemushaRecursiveSpendProver private constructor() {
    enum class Mode(val wireName: String) {
        CHECKED_PREFOLD_V1("checked_prefold_v1"),
        RECURSIVE_COMPACT_V1("recursive_compact_v1"),
        RECURSIVE_SPEND_V1("recursive_spend_v1"),
    }

    companion object {
        const val REQUIRED_BRIDGE_ABI_VERSION: Int = 6
        const val RECURSIVE_COMPACT_REQUIRED_BRIDGE_ABI_VERSION: Int = 7
        const val RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1 =
            "kagemusha-recursive-aggregation-v1"
        const val RECURSIVE_COMPACT_CIRCUIT_ID_V1 =
            "kagemusha-recursive-compact-v1"
        const val RECURSIVE_AGGREGATION_PROOF_BACKEND =
            "halo2/ipa"
        const val RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1 =
            "kagemusha-recursive-spend-lineage-v1"
        const val RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1 =
            "kagemusha-recursive-spend-lineage-onehop-v1"
        const val RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1 =
            "kagemusha-recursive-spend-lineage-append-v1"
        const val COMPACT_TOKEN_MAX_HOPS: Int = 64
        const val RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1: Int = 64
        const val RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1: Boolean = true
        const val RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1: Int = 1
        const val RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES: Int = 8 * 1024 * 1024
        const val RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES: Int = 128
        const val NATIVE_ARCHIVE_MAX_BYTES: Int = 64 * 1024 * 1024
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
        fun preferredMode(): Mode =
            preferredMode(
                recursiveCompactAvailable = KagemushaRecursiveCompactPaymentTokenProver.isNativeAvailable(),
                recursiveSpendAvailable = nativeAvailable,
            )

        @JvmStatic
        fun preferredMode(recursiveSpendAvailable: Boolean): Mode =
            preferredMode(
                recursiveCompactAvailable = false,
                recursiveSpendAvailable = recursiveSpendAvailable,
            )

        @JvmStatic
        @Suppress("UNUSED_PARAMETER")
        fun preferredMode(
            recursiveCompactAvailable: Boolean,
            recursiveSpendAvailable: Boolean,
        ): Mode {
            // ABI-7 compact mode is not a production default yet.
            return if (recursiveSpendAvailable) {
                Mode.RECURSIVE_SPEND_V1
            } else {
                Mode.CHECKED_PREFOLD_V1
            }
        }

        @JvmStatic
        fun canRedeemWitnessless(circuitId: String?, hopCount: Int): Boolean {
            val hopCountSupported =
                hopCount >= 1 &&
                    hopCount <= RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1
            val canonicalLineage =
                circuitId == RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1 && hopCountSupported
            val oneHopLineage =
                circuitId == RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1 && hopCountSupported
            val appendLineage =
                circuitId == RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1 && hopCountSupported
            return RECURSIVE_SPEND_LINEAGE_TRANSITION_CIRCUIT_WIRED_V1 &&
                (canonicalLineage || oneHopLineage || appendLineage)
        }

        @JvmStatic
        fun isLineageProofCircuitId(circuitId: String?): Boolean =
            circuitId == RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1 ||
                circuitId == RECURSIVE_SPEND_LINEAGE_ONE_HOP_PROOF_CIRCUIT_ID_V1 ||
                circuitId == RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1

        @JvmStatic
        fun isLineageAppendOutputCircuitId(outputCircuitId: String?): Boolean =
            outputCircuitId == RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1 ||
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
                        val decoded = payload.toString(Charsets.UTF_8).trim()
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
                KagemushaCompactPaymentTokenProver.isValidNoritoArchive(lineageProvingKeyArchive) &&
                    KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(lineageProvingKeyArchive),
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
        fun requiresLineageKeyArtifactsForInit(): Boolean = true

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
                RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1
            } else if (outputCircuitId == RECURSIVE_SPEND_LINEAGE_PROOF_CIRCUIT_ID_V1) {
                RECURSIVE_SPEND_LINEAGE_APPEND_PROOF_CIRCUIT_ID_V1
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
        fun appendSpend(requestArchive: ByteArray?): ByteArray =
            call("append", requestArchive, ::nativeAppendSpend)

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
        fun redeemSpend(requestArchive: ByteArray?): ByteArray =
            call("redeem", requestArchive, ::nativeRedeemSpend)

        private fun call(
            label: String,
            requestArchive: ByteArray?,
            nativeCall: (ByteArray) -> ByteArray?,
        ): ByteArray =
            callArchive(label, "requestArchive", requestArchive, nativeCall)

        private fun callArchive(
            label: String,
            archiveName: String,
            archive: ByteArray?,
            nativeCall: (ByteArray) -> ByteArray?,
        ): ByteArray {
            val ownedArchive = ownedNativeInput(archive, archiveName)
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
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
            require(KagemushaCompactPaymentTokenProver.isValidNoritoArchive(archive)) {
                "$archiveName must be a valid Norito archive"
            }
            require(KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(archive)) {
                "$archiveName must contain a non-empty Norito payload"
            }
            return archive
        }

        private fun requireNativeOutput(output: ByteArray?, label: String): ByteArray {
            check(output != null) { "$label returned no output" }
            check(output.isNotEmpty()) { "$label returned empty output" }
            check(output.size <= NATIVE_ARCHIVE_MAX_BYTES) { "$label returned oversized output" }
            check(KagemushaCompactPaymentTokenProver.isValidNoritoArchive(output)) {
                "$label returned invalid Norito archive"
            }
            check(KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(output)) {
                "$label returned empty Norito payload"
            }
            return output
        }

        private fun loadLibrary(): Boolean =
            detectNativeAvailability(
                loadLibrary = { System.loadLibrary(LIBRARY_NAME) },
                bridgeAbiVersion = { nativeBridgeAbiVersion() },
                probeSymbol = { probeRequiredNativeSymbols() },
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
            bridgeAbiVersion: () -> Int,
            probeSymbol: () -> Boolean,
            requiredBridgeAbiVersion: Int = REQUIRED_BRIDGE_ABI_VERSION,
        ): Boolean {
            try {
                loadLibrary()
            } catch (_: UnsatisfiedLinkError) {
                return false
            } catch (_: RuntimeException) {
                return false
            }
            val abiVersion = try {
                bridgeAbiVersion()
            } catch (_: UnsatisfiedLinkError) {
                return false
            } catch (_: RuntimeException) {
                return false
            }
            if (abiVersion < requiredBridgeAbiVersion) {
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

        @JvmStatic
        private external fun nativeBridgeAbiVersion(): Int

        @JvmStatic
        private external fun nativeInitSpend(requestArchive: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeAppendSpend(requestArchive: ByteArray): ByteArray?

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
