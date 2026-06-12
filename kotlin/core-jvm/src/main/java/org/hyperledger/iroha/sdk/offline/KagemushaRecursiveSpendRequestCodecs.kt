package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.AccountAddressException
import org.hyperledger.iroha.sdk.address.AssetDefinitionIdEncoder
import org.hyperledger.iroha.sdk.address.MultisigMemberPayload
import org.hyperledger.iroha.sdk.address.MultisigPolicyPayload
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/** Spendable note descriptor carried by recursive Kagemusha spend requests and bundles. */
class SpendableNoteDescriptor(
    noteCommitment: ByteArray,
    spendNullifier: ByteArray,
    amount: String,
) {
    private val noteCommitmentBytes = fixed32(noteCommitment, "noteCommitment")
    private val spendNullifierBytes = fixed32(spendNullifier, "spendNullifier")

    /** Canonical nonzero unsigned integer amount accepted by recursive spend v1. */
    val amount: String = canonicalU128Decimal(amount, "amount")

    /** 32-byte current note commitment. */
    val noteCommitment: ByteArray get() = noteCommitmentBytes.copyOf()

    /** 32-byte nullifier consumed by the next hop or online redeem. */
    val spendNullifier: ByteArray get() = spendNullifierBytes.copyOf()

    init {
        require(!isZero32(noteCommitmentBytes)) { "noteCommitment must be non-zero" }
        require(!isZero32(spendNullifierBytes)) { "spendNullifier must be non-zero" }
        require(!noteCommitmentBytes.contentEquals(spendNullifierBytes)) {
            "spendNullifier must differ from noteCommitment"
        }
    }

    fun noteCommitmentBytes(): ByteArray = noteCommitment

    fun spendNullifierBytes(): ByteArray = spendNullifier

    override fun equals(other: Any?): Boolean =
        other is SpendableNoteDescriptor &&
            noteCommitmentBytes.contentEquals(other.noteCommitmentBytes) &&
            spendNullifierBytes.contentEquals(other.spendNullifierBytes) &&
            amount == other.amount

    override fun hashCode(): Int =
        31 * (31 * noteCommitmentBytes.contentHashCode() + spendNullifierBytes.contentHashCode()) +
            amount.hashCode()
}

/** Active verifier-record archive paired with the verifier-key registry id used to fetch it. */
class VerifierRecordRef(
    val verifierKeyId: String,
    recordBytes: ByteArray,
) {
    private val recordArchiveBytes = recordBytes.copyOf()

    init {
        requirePortableId(verifierKeyId, "verifierKeyId")
        require(recordArchiveBytes.isNotEmpty()) { "recordBytes must not be empty" }
        KagemushaRecursiveSpendRequestCodecs.requirePayloadArchive(
            recordArchiveBytes,
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFYING_KEY_RECORD,
            "recordBytes",
        )
    }

    /** Norito archive of `iroha_data_model::proof::VerifyingKeyRecord`. */
    val recordBytes: ByteArray get() = recordArchiveBytes.copyOf()

    fun recordBytes(): ByteArray = recordBytes
}

/** Typed request for `KagemushaRecursiveSpendInitRequestV1`. */
class InitSpendRequest @JvmOverloads constructor(
    recordBundle: ByteArray,
    pallasOpenEnvelopes: ByteArray,
    val currentNote: SpendableNoteDescriptor,
    lineageVerifierKey: ByteArray? = null,
    lineageProvingKeyArchive: ByteArray? = null,
    val blockHeight: Long? = null,
) {
    private val recordBundleArchive = recordBundle.copyOf()
    private val pallasOpenEnvelopesArchive = pallasOpenEnvelopes.copyOf()
    private val lineageVerifierKeyBytes = lineageVerifierKey?.copyOf()
    private val lineageProvingKeyArchiveBytes = lineageProvingKeyArchive?.copyOf()

    init {
        requireNonNegativeHeight(blockHeight)
        require(lineageVerifierKeyBytes != null) {
            "lineageVerifierKey is required for recursive spend init"
        }
        require(lineageProvingKeyArchiveBytes != null) {
            "lineageProvingKeyArchive is required for recursive spend init"
        }
        require(lineageVerifierKeyBytes.isNotEmpty()) { "lineageVerifierKey must not be empty" }
        require(lineageProvingKeyArchiveBytes.isNotEmpty()) {
            "lineageProvingKeyArchive must not be empty"
        }
        requireValidNestedArchive(pallasOpenEnvelopesArchive, "pallasOpenEnvelopes")
        requireValidNestedArchive(lineageProvingKeyArchiveBytes, "lineageProvingKeyArchive")
    }

    val recordBundle: ByteArray get() = recordBundleArchive.copyOf()

    val pallasOpenEnvelopes: ByteArray get() = pallasOpenEnvelopesArchive.copyOf()

    val lineageVerifierKey: ByteArray? get() = lineageVerifierKeyBytes?.copyOf()

    val lineageProvingKeyArchive: ByteArray? get() = lineageProvingKeyArchiveBytes?.copyOf()
}

/** Typed request for `KagemushaRecursiveSpendAppendRequestV1`. */
class AppendSpendRequest @JvmOverloads constructor(
    previousBundle: ByteArray,
    recordBundle: ByteArray,
    pallasOpenEnvelopes: ByteArray,
    val currentNote: SpendableNoteDescriptor,
    val outputProofCircuitId: String? = null,
    val previousLineageVerifierRecord: VerifierRecordRef? = null,
    previousProofOpenEnvelopes: ByteArray? = null,
    lineageVerifierKey: ByteArray? = null,
    lineageProvingKeyArchive: ByteArray? = null,
    val blockHeight: Long? = null,
) {
    private val previousBundleArchive = previousBundle.copyOf()
    private val recordBundleArchive = recordBundle.copyOf()
    private val pallasOpenEnvelopesArchive = pallasOpenEnvelopes.copyOf()
    private val previousProofOpenEnvelopesArchive = previousProofOpenEnvelopes?.copyOf()
    private val lineageVerifierKeyBytes = lineageVerifierKey?.copyOf()
    private val lineageProvingKeyArchiveBytes = lineageProvingKeyArchive?.copyOf()

    init {
        requireNonNegativeHeight(blockHeight)
        requireValidNestedArchive(pallasOpenEnvelopesArchive, "pallasOpenEnvelopes")
        previousProofOpenEnvelopesArchive?.let {
            require(it.size <= KagemushaRecursiveSpendProver.RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES) {
                "previousProofOpenEnvelopes must not exceed " +
                    "${KagemushaRecursiveSpendProver.RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES} bytes"
            }
            requireValidNestedArchive(it, "previousProofOpenEnvelopes")
        }
        lineageProvingKeyArchiveBytes?.let {
            requireValidNestedArchive(it, "lineageProvingKeyArchive")
        }

        val previousSummary = KagemushaRecursiveSpendRequestCodecs.decodeBundle(previousBundleArchive)
        val normalizedOutput =
            KagemushaRecursiveSpendProver.normalizeAppendOutputCircuitId(outputProofCircuitId)
        require(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                previousSummary.proofCircuitId,
                normalizedOutput,
                previousSummary.hopCount,
            ),
        ) {
            "outputProofCircuitId is not valid for the previous bundle"
        }
        if (KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
                previousSummary.proofCircuitId,
            )
        ) {
            require(previousLineageVerifierRecord != null) {
                "previousLineageVerifierRecord is required for lineage previous bundles"
            }
        }
        if (KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
                normalizedOutput,
                previousSummary.hopCount,
            )
        ) {
            require(previousProofOpenEnvelopesArchive != null) {
                "previousProofOpenEnvelopes is required for lineage append output"
            }
        }
        if (KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(normalizedOutput)) {
            require(lineageVerifierKeyBytes != null && lineageVerifierKeyBytes.isNotEmpty()) {
                "lineageVerifierKey is required for lineage append output"
            }
            require(lineageProvingKeyArchiveBytes != null && lineageProvingKeyArchiveBytes.isNotEmpty()) {
                "lineageProvingKeyArchive is required for lineage append output"
            }
        }
    }

    val previousBundle: ByteArray get() = previousBundleArchive.copyOf()

    val recordBundle: ByteArray get() = recordBundleArchive.copyOf()

    val pallasOpenEnvelopes: ByteArray get() = pallasOpenEnvelopesArchive.copyOf()

    val previousProofOpenEnvelopes: ByteArray? get() = previousProofOpenEnvelopesArchive?.copyOf()

    val lineageVerifierKey: ByteArray? get() = lineageVerifierKeyBytes?.copyOf()

    val lineageProvingKeyArchive: ByteArray? get() = lineageProvingKeyArchiveBytes?.copyOf()
}

/** Typed request for `KagemushaRecursiveSpendVerifyRequestV1`. */
class VerifySpendRequest @JvmOverloads constructor(
    bundle: ByteArray,
    val lineageVerifierRecord: VerifierRecordRef? = null,
    val blockHeight: Long? = null,
) {
    private val bundleArchive = bundle.copyOf()

    init {
        requireNonNegativeHeight(blockHeight)
    }

    val bundle: ByteArray get() = bundleArchive.copyOf()
}

/** Decoded `KagemushaRecursiveSpendVerifyResultV1`. */
data class VerifySpendResult(
    val valid: Boolean,
    val hopCount: Int,
    val encodedBytes: Int,
    val reason: String,
    val chainAdmissible: Boolean,
    val chainAdmissionReason: String,
    val witnesslessRedeemSupported: Boolean = false,
    val lineageWitnessRequired: Boolean = false,
)

/** Typed request for `KagemushaRecursiveSpendRedeemRequestV1`. */
class RedeemSpendRequest @JvmOverloads constructor(
    bundle: ByteArray,
    val recipient: String,
    val publicAmount: String,
    redeemProof: ByteArray,
    lineageWitness: ByteArray? = null,
    val lineageVerifierRecord: VerifierRecordRef? = null,
    val blockHeight: Long? = null,
) {
    private val bundleArchive = bundle.copyOf()
    private val redeemProofArchive = redeemProof.copyOf()
    private val lineageWitnessArchive = lineageWitness?.copyOf()
    private val canonicalPublicAmount = canonicalU128Decimal(publicAmount, "publicAmount")

    init {
        requireNonNegativeHeight(blockHeight)
        requireNonBlankUnpadded(recipient, "recipient")
        lineageWitnessArchive?.let {
            requireValidNestedArchive(it, "lineageWitness")
        }
    }

    val bundle: ByteArray get() = bundleArchive.copyOf()

    val redeemProof: ByteArray get() = redeemProofArchive.copyOf()

    val lineageWitness: ByteArray? get() = lineageWitnessArchive?.copyOf()

    fun canonicalPublicAmount(): String = canonicalPublicAmount
}

/** Read-only summary of a recursive spend bundle. */
class SpendBundleSummary(
    val hopCount: Int,
    val proofCircuitId: String,
    val asset: String,
    val chainId: String,
    initialRoot: ByteArray,
    finalRoot: ByteArray,
    val currentNote: SpendableNoteDescriptor,
) {
    private val initialRootBytes = fixed32(initialRoot, "initialRoot")
    private val finalRootBytes = fixed32(finalRoot, "finalRoot")

    val initialRoot: ByteArray get() = initialRootBytes.copyOf()

    val finalRoot: ByteArray get() = finalRootBytes.copyOf()
}

/** Norito request builders and result decoders for recursive Kagemusha spend ABI v1. */
object KagemushaRecursiveSpendRequestCodecs {
    const val SCHEMA_INIT_REQUEST: String =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendInitRequestV1"
    const val SCHEMA_APPEND_REQUEST: String =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendAppendRequestV1"
    const val SCHEMA_VERIFY_REQUEST: String =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendVerifyRequestV1"
    const val SCHEMA_VERIFY_RESULT: String =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendVerifyResultV1"
    const val SCHEMA_REDEEM_REQUEST: String =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendRedeemRequestV1"
    const val SCHEMA_BUNDLE: String =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendBundleV1"
    const val SCHEMA_RECORD_BUNDLE: String =
        "iroha_data_model::offline::model::KagemushaVerifiedFoldRecordBundle"
    const val SCHEMA_LINEAGE_WITNESS: String =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendLineageWitnessV1"
    const val SCHEMA_PROOF_ATTACHMENT: String =
        "iroha_data_model::proof::ProofAttachment"
    const val SCHEMA_VERIFYING_KEY_RECORD: String =
        "iroha_data_model::proof::VerifyingKeyRecord"

    private const val REQUEST_FLAGS = NoritoHeader.COMPACT_LEN

    @JvmStatic
    fun encodeInitRequest(request: InitSpendRequest): ByteArray =
        NoritoCodec.encode(request, SCHEMA_INIT_REQUEST, InitRequestAdapter, REQUEST_FLAGS)

    @JvmStatic
    fun encodeAppendRequest(request: AppendSpendRequest): ByteArray =
        NoritoCodec.encode(request, SCHEMA_APPEND_REQUEST, AppendRequestAdapter, REQUEST_FLAGS)

    @JvmStatic
    fun encodeVerifyRequest(request: VerifySpendRequest): ByteArray =
        NoritoCodec.encode(request, SCHEMA_VERIFY_REQUEST, VerifyRequestAdapter, REQUEST_FLAGS)

    @JvmStatic
    fun encodeRedeemRequest(request: RedeemSpendRequest): ByteArray =
        NoritoCodec.encode(request, SCHEMA_REDEEM_REQUEST, RedeemRequestAdapter, REQUEST_FLAGS)

    @JvmStatic
    fun decodeVerifyResult(archive: ByteArray): VerifySpendResult {
        val payload = requirePayloadArchive(archive, SCHEMA_VERIFY_RESULT, "verifyResult")
        require(payload.flags == REQUEST_FLAGS) {
            "verifyResult must use compact Norito layout"
        }
        val decoder = NoritoDecoder(payload.payload, payload.flags)
        val valid = readField(decoder) { it.readBool() }
        val hopCount = readField(decoder) { checkedInt(it.readUInt(32), "hop_count") }
        val encodedBytes = readField(decoder) { checkedInt(it.readUInt(32), "encoded_bytes") }
        val reason = readField(decoder) { readString(it) }
        val chainAdmissible = readField(decoder) { it.readBool() }
        val chainAdmissionReason = readField(decoder) { readString(it) }
        val witnesslessRedeemSupported =
            if (decoder.remaining() == 0) false else readField(decoder) { it.readBool() }
        val lineageWitnessRequired =
            if (decoder.remaining() == 0) false else readField(decoder) { it.readBool() }
        require(decoder.remaining() == 0) { "Trailing bytes after verify result" }
        return VerifySpendResult(
            valid = valid,
            hopCount = hopCount,
            encodedBytes = encodedBytes,
            reason = reason,
            chainAdmissible = chainAdmissible,
            chainAdmissionReason = chainAdmissionReason,
            witnesslessRedeemSupported = witnesslessRedeemSupported,
            lineageWitnessRequired = lineageWitnessRequired,
        )
    }

    @JvmStatic
    fun decodeBundle(archive: ByteArray): SpendBundleSummary {
        val payload = requirePayloadArchive(archive, SCHEMA_BUNDLE, "bundle")
        require(payload.flags == REQUEST_FLAGS) { "bundle must use compact Norito layout" }
        val decoder = NoritoDecoder(payload.payload, payload.flags)
        val accumulatorPayload = readField(decoder) { it.readRemainingBytes() }
        val proofPayload = readField(decoder) { it.readRemainingBytes() }
        require(decoder.remaining() == 0) { "Trailing bytes after bundle" }

        val accumulator = readAccumulatorSummary(accumulatorPayload, payload.flags)
        val proofCircuitId = readRecursiveProofCircuitId(proofPayload, payload.flags)
        return SpendBundleSummary(
            hopCount = accumulator.hopCount,
            proofCircuitId = proofCircuitId,
            asset = accumulator.asset,
            chainId = accumulator.chainId,
            initialRoot = accumulator.initialRoot,
            finalRoot = accumulator.finalRoot,
            currentNote = accumulator.currentNote,
        )
    }

    internal fun requirePayloadArchive(
        archive: ByteArray,
        schema: String,
        field: String,
    ): ArchivePayload {
        require(archive.isNotEmpty()) { "$field must not be empty" }
        require(archive.size <= KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES) {
            "$field must not exceed ${KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES} bytes"
        }
        val decoded = try {
            NoritoHeader.decode(archive, SchemaHash.hash16(schema))
        } catch (ex: IllegalArgumentException) {
            throw IllegalArgumentException("$field must be a valid $schema Norito archive", ex)
        }
        require(decoded.header.compression == NoritoHeader.COMPRESSION_NONE) {
            "$field must not be compressed"
        }
        require(decoded.header.payloadLength > 0) {
            "$field must contain a non-empty Norito payload"
        }
        decoded.header.validateChecksum(decoded.payload)
        return ArchivePayload(decoded.payload, decoded.header.flags)
    }

    internal fun compactPayloadForRequest(
        archive: ByteArray,
        schema: String,
        field: String,
    ): ByteArray {
        val payload = requirePayloadArchive(archive, schema, field)
        require(payload.flags == REQUEST_FLAGS) {
            "$field must use compact Norito layout"
        }
        return payload.payload
    }

    internal class ArchivePayload(
        val payload: ByteArray,
        val flags: Int,
    )

    private object InitRequestAdapter : TypeAdapter<InitSpendRequest> {
        override fun encode(encoder: NoritoEncoder, value: InitSpendRequest) {
            writeRawField(
                encoder,
                compactPayloadForRequest(value.recordBundle, SCHEMA_RECORD_BUNDLE, "recordBundle"),
            )
            writeField(encoder) { writeBytesVec(it, value.pallasOpenEnvelopes) }
            writeField(encoder) { writeSpendableNote(it, value.currentNote) }
            writeField(encoder) {
                writeOptionRaw(it, value.lineageVerifierKey?.let(::verifyingKeyBoxPayload))
            }
            writeField(encoder) {
                writeOptionBytesVec(it, value.lineageProvingKeyArchive)
            }
            writeField(encoder) { writeOptionU64(it, value.blockHeight) }
        }

        override fun decode(decoder: NoritoDecoder): InitSpendRequest {
            throw UnsupportedOperationException("recursive spend requests are encode-only")
        }
    }

    private object AppendRequestAdapter : TypeAdapter<AppendSpendRequest> {
        override fun encode(encoder: NoritoEncoder, value: AppendSpendRequest) {
            writeRawField(
                encoder,
                compactPayloadForRequest(value.previousBundle, SCHEMA_BUNDLE, "previousBundle"),
            )
            writeRawField(
                encoder,
                compactPayloadForRequest(value.recordBundle, SCHEMA_RECORD_BUNDLE, "recordBundle"),
            )
            writeField(encoder) { writeBytesVec(it, value.pallasOpenEnvelopes) }
            writeField(encoder) { writeSpendableNote(it, value.currentNote) }
            writeField(encoder) {
                val normalized =
                    KagemushaRecursiveSpendProver.normalizeAppendOutputCircuitId(value.outputProofCircuitId)
                val wire = if (normalized == KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_CIRCUIT_ID_V1) {
                    ""
                } else {
                    normalized
                }
                writeString(it, wire)
            }
            writeField(encoder) {
                writeOptionRaw(
                    it,
                    value.previousLineageVerifierRecord?.recordBytes?.let { record ->
                        compactPayloadForRequest(record, SCHEMA_VERIFYING_KEY_RECORD, "previousLineageVerifierRecord")
                    },
                )
            }
            writeField(encoder) { writeBytesVec(it, value.previousProofOpenEnvelopes ?: ByteArray(0)) }
            writeField(encoder) {
                writeOptionRaw(it, value.lineageVerifierKey?.let(::verifyingKeyBoxPayload))
            }
            writeField(encoder) { writeOptionBytesVec(it, value.lineageProvingKeyArchive) }
            writeField(encoder) { writeOptionU64(it, value.blockHeight) }
        }

        override fun decode(decoder: NoritoDecoder): AppendSpendRequest {
            throw UnsupportedOperationException("recursive spend requests are encode-only")
        }
    }

    private object VerifyRequestAdapter : TypeAdapter<VerifySpendRequest> {
        override fun encode(encoder: NoritoEncoder, value: VerifySpendRequest) {
            writeRawField(encoder, compactPayloadForRequest(value.bundle, SCHEMA_BUNDLE, "bundle"))
            writeField(encoder) {
                writeOptionRaw(
                    it,
                    value.lineageVerifierRecord?.recordBytes?.let { record ->
                        compactPayloadForRequest(record, SCHEMA_VERIFYING_KEY_RECORD, "lineageVerifierRecord")
                    },
                )
            }
            writeField(encoder) { writeOptionU64(it, value.blockHeight) }
        }

        override fun decode(decoder: NoritoDecoder): VerifySpendRequest {
            throw UnsupportedOperationException("recursive spend requests are encode-only")
        }
    }

    private object RedeemRequestAdapter : TypeAdapter<RedeemSpendRequest> {
        override fun encode(encoder: NoritoEncoder, value: RedeemSpendRequest) {
            writeRawField(encoder, compactPayloadForRequest(value.bundle, SCHEMA_BUNDLE, "bundle"))
            writeField(encoder) { writeAccountId(it, value.recipient) }
            writeField(encoder) { writeU128(it, value.canonicalPublicAmount()) }
            writeRawField(
                encoder,
                compactPayloadForRequest(value.redeemProof, SCHEMA_PROOF_ATTACHMENT, "redeemProof"),
            )
            writeField(encoder) {
                writeOptionRaw(
                    it,
                    value.lineageWitness?.let { witness ->
                        compactPayloadForRequest(witness, SCHEMA_LINEAGE_WITNESS, "lineageWitness")
                    },
                )
            }
            writeField(encoder) {
                writeOptionRaw(
                    it,
                    value.lineageVerifierRecord?.recordBytes?.let { record ->
                        compactPayloadForRequest(record, SCHEMA_VERIFYING_KEY_RECORD, "lineageVerifierRecord")
                    },
                )
            }
            writeField(encoder) { writeOptionU64(it, value.blockHeight) }
        }

        override fun decode(decoder: NoritoDecoder): RedeemSpendRequest {
            throw UnsupportedOperationException("recursive spend requests are encode-only")
        }
    }

    private class AccumulatorSummary(
        val chainId: String,
        val asset: String,
        val initialRoot: ByteArray,
        val finalRoot: ByteArray,
        val hopCount: Int,
        val currentNote: SpendableNoteDescriptor,
    )

    private fun readAccumulatorSummary(payload: ByteArray, flags: Int): AccumulatorSummary {
        val decoder = NoritoDecoder(payload, flags)
        readField(decoder) { readString(it) } // domain
        val chainId = readField(decoder) { readChainId(it) }
        val asset = readField(decoder) { readAssetDefinitionId(it) }
        val initialRoot = readField(decoder) { it.readFixed32("initial_root") }
        val finalRoot = readField(decoder) { it.readFixed32("final_root") }
        skipFields(decoder, 1) // topup_anchor_nullifiers
        val hopCount = readField(decoder) { checkedInt(it.readUInt(32), "hop_count") }
        skipFields(decoder, 15)
        val currentNote = readField(decoder) { readSpendableNote(it) }
        require(decoder.remaining() == 0) { "Trailing bytes after accumulator" }
        return AccumulatorSummary(chainId, asset, initialRoot, finalRoot, hopCount, currentNote)
    }

    private fun readRecursiveProofCircuitId(payload: ByteArray, flags: Int): String {
        val decoder = NoritoDecoder(payload, flags)
        val verifierKeyIdPayload = readField(decoder) { it.readRemainingBytes() }
        return readVerifyingKeyIdName(verifierKeyIdPayload, flags)
    }
}

private fun verifyingKeyBoxPayload(bytes: ByteArray): ByteArray {
    require(bytes.isNotEmpty()) { "lineageVerifierKey must not be empty" }
    val encoder = NoritoEncoder(NoritoHeader.COMPACT_LEN)
    writeField(encoder) {
        writeString(it, KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND)
    }
    writeField(encoder) { writeBytesVec(it, bytes) }
    return encoder.toByteArray()
}

private fun requireValidNestedArchive(archive: ByteArray, field: String) {
    require(archive.isNotEmpty()) { "$field must not be empty" }
    require(archive.size <= KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES) {
        "$field must not exceed ${KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES} bytes"
    }
    require(KagemushaCompactPaymentTokenProver.isValidNoritoArchive(archive)) {
        "$field must be a valid Norito archive"
    }
    require(KagemushaCompactPaymentTokenProver.hasNonEmptyNoritoPayload(archive)) {
        "$field must contain a non-empty Norito payload"
    }
}

private fun fixed32(value: ByteArray, field: String): ByteArray {
    require(value.size == 32) { "$field must be exactly 32 bytes" }
    return value.copyOf()
}

private fun isZero32(value: ByteArray): Boolean = value.all { it.toInt() == 0 }

private fun requireNonNegativeHeight(blockHeight: Long?) {
    require(blockHeight == null || blockHeight >= 0L) { "blockHeight must be non-negative" }
}

private fun requirePortableId(value: String, field: String) {
    requireNonBlankUnpadded(value, field)
    require(value.length <= 256) { "$field must not exceed 256 characters" }
    require(value.all { it in 'A'..'Z' || it in 'a'..'z' || it in '0'..'9' || it in PORTABLE_ID_CHARS }) {
        "$field must use portable registry syntax"
    }
}

private val PORTABLE_ID_CHARS = setOf('.', '_', '-', '/', ':', '@', '+', '=')

private fun requireNonBlankUnpadded(value: String, field: String) {
    require(value.trim().isNotEmpty()) { "$field must not be blank" }
    require(value.trim() == value) { "$field must not contain surrounding whitespace" }
}

private fun canonicalU128Decimal(value: String, field: String): String {
    require(value.isNotEmpty()) { "$field must be a decimal integer" }
    require(value.all { it in '0'..'9' }) { "$field must be a decimal integer" }
    require(value.length == 1 || value[0] != '0') { "$field must be canonical" }
    val integer = BigInteger(value)
    require(integer > BigInteger.ZERO) { "$field must be greater than zero" }
    require(integer <= MAX_U128) { "$field must fit in u128" }
    return integer.toString()
}

private val MAX_U128: BigInteger = BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE)

private fun writeField(parent: NoritoEncoder, writePayload: (NoritoEncoder) -> Unit) {
    val child = parent.childEncoder()
    writePayload(child)
    writeRawField(parent, child.toByteArray())
}

private fun writeRawField(parent: NoritoEncoder, payload: ByteArray) {
    parent.writeLength(payload.size.toLong(), compact(parent))
    parent.writeBytes(payload)
}

private fun writeString(encoder: NoritoEncoder, value: String) {
    val bytes = value.toByteArray(StandardCharsets.UTF_8)
    encoder.writeLength(bytes.size.toLong(), compact(encoder))
    encoder.writeBytes(bytes)
}

private fun writeBytesVec(encoder: NoritoEncoder, value: ByteArray) {
    encoder.writeUInt(value.size.toLong(), 64)
    encoder.writeBytes(value)
}

private fun writeConstVec(encoder: NoritoEncoder, value: ByteArray) {
    for (byte in value) {
        encoder.writeLength(1, compact(encoder))
        encoder.writeByte(byte.toInt())
    }
}

private fun writeOptionRaw(encoder: NoritoEncoder, payload: ByteArray?) {
    if (payload == null) {
        encoder.writeByte(0)
        return
    }
    encoder.writeByte(1)
    encoder.writeLength(payload.size.toLong(), compact(encoder))
    encoder.writeBytes(payload)
}

private fun writeOptionBytesVec(encoder: NoritoEncoder, value: ByteArray?) {
    if (value == null) {
        encoder.writeByte(0)
        return
    }
    encoder.writeByte(1)
    writeField(encoder) { writeBytesVec(it, value) }
}

private fun writeOptionU64(encoder: NoritoEncoder, value: Long?) {
    if (value == null) {
        encoder.writeByte(0)
        return
    }
    encoder.writeByte(1)
    writeField(encoder) { it.writeUInt(value, 64) }
}

private fun writeSpendableNote(encoder: NoritoEncoder, value: SpendableNoteDescriptor) {
    writeField(encoder) { writeConstVec(it, value.noteCommitment) }
    writeField(encoder) { writeConstVec(it, value.spendNullifier) }
    writeField(encoder) { writeNumeric(it, value.amount) }
}

private fun writeNumeric(encoder: NoritoEncoder, value: String) {
    val integer = BigInteger(value)
    val mantissaBytes = toTwosComplementLittleEndian(integer)
    writeField(encoder) { mantissa ->
        mantissa.writeUInt(mantissaBytes.size.toLong(), 32)
        mantissa.writeBytes(mantissaBytes)
    }
    writeField(encoder) { it.writeUInt(0, 32) }
}

private fun writeU128(encoder: NoritoEncoder, value: String) {
    var integer = BigInteger(value)
    repeat(16) {
        encoder.writeByte(integer.and(BYTE_MASK).toInt())
        integer = integer.shiftRight(8)
    }
}

private val BYTE_MASK = BigInteger.valueOf(0xffL)

private fun toTwosComplementLittleEndian(value: BigInteger): ByteArray {
    if (value.signum() == 0) return ByteArray(0)
    val bigEndian = value.toByteArray()
    val little = ByteArray(bigEndian.size)
    for (idx in bigEndian.indices) {
        little[idx] = bigEndian[bigEndian.size - 1 - idx]
    }
    return little
}

private fun writeAccountId(encoder: NoritoEncoder, accountId: String) {
    val address = try {
        AccountAddress.parseEncodedIgnoringCurveSupport(accountId, null).address
    } catch (ex: AccountAddressException) {
        throw IllegalArgumentException("recipient must use canonical I105 account form", ex)
    }
    val single = address.singleKeyPayloadIgnoringCurveSupport()
    if (single != null) {
        encoder.writeUInt(0, 32)
        writeField(encoder) { writePublicKey(it, single.curveId, single.publicKey) }
        return
    }
    val multisig = address.multisigPolicyPayloadIgnoringCurveSupport()
        ?: throw IllegalArgumentException("recipient has no supported controller")
    encoder.writeUInt(1, 32)
    writeField(encoder) { writeMultisigPolicy(it, multisig) }
}

private fun writeMultisigPolicy(encoder: NoritoEncoder, policy: MultisigPolicyPayload) {
    require(policy.version == MULTISIG_POLICY_VERSION) { "unsupported multisig policy version" }
    require(policy.threshold > 0) { "multisig threshold must be positive" }
    require(policy.members.isNotEmpty()) { "multisig policy must have members" }
    writeField(encoder) { it.writeUInt(policy.version.toLong(), 8) }
    writeField(encoder) { it.writeUInt(policy.threshold.toLong(), 16) }
    writeField(encoder) { writeMultisigMembers(it, policy.members) }
}

private fun writeMultisigMembers(encoder: NoritoEncoder, members: List<MultisigMemberPayload>) {
    val sorted = members.sortedWith { left, right -> compareUnsigned(memberSortKey(left), memberSortKey(right)) }
    for (idx in 1 until sorted.size) {
        require(!memberSortKey(sorted[idx - 1]).contentEquals(memberSortKey(sorted[idx]))) {
            "duplicate multisig member"
        }
    }
    encoder.writeUInt(sorted.size.toLong(), 64)
    for (member in sorted) {
        writeField(encoder) { memberEncoder ->
            writeField(memberEncoder) { writePublicKey(it, member.curveId, member.publicKey) }
            writeField(memberEncoder) { it.writeUInt(member.weight.toLong(), 16) }
        }
    }
}

private fun memberSortKey(member: MultisigMemberPayload): ByteArray =
    byteArrayOf(member.curveId.toByte()) + member.publicKey

private fun compareUnsigned(left: ByteArray, right: ByteArray): Int {
    val min = minOf(left.size, right.size)
    for (idx in 0 until min) {
        val cmp = (left[idx].toInt() and 0xff) - (right[idx].toInt() and 0xff)
        if (cmp != 0) return cmp
    }
    return left.size - right.size
}

private fun writePublicKey(encoder: NoritoEncoder, curveId: Int, publicKey: ByteArray) {
    writeConstVec(encoder, publicKeyCompactPayload(curveId, publicKey))
}

private fun publicKeyCompactPayload(curveId: Int, publicKey: ByteArray): ByteArray {
    val tag = when (curveId) {
        0x01 -> 0
        0x04 -> 1
        0x03 -> 2
        0x05 -> 3
        0x02 -> 4
        0x0A -> 5
        0x0B -> 6
        0x0C -> 7
        0x0D -> 8
        0x0E -> 9
        0x0F -> 10
        else -> throw IllegalArgumentException("Unsupported recipient curve id: $curveId")
    }
    val bytes = ByteArray(1 + publicKey.size)
    bytes[0] = tag.toByte()
    System.arraycopy(publicKey, 0, bytes, 1, publicKey.size)
    return bytes
}

private fun readSpendableNote(decoder: NoritoDecoder): SpendableNoteDescriptor =
    SpendableNoteDescriptor(
        noteCommitment = readField(decoder) { it.readFixed32("note_commitment") },
        spendNullifier = readField(decoder) { it.readFixed32("spend_nullifier") },
        amount = readField(decoder) { readNumeric(it) },
    )

private fun readNumeric(decoder: NoritoDecoder): String {
    val mantissaBytes = readField(decoder) { payload ->
        val length = checkedInt(payload.readUInt(32), "numeric mantissa length")
        payload.readBytes(length)
    }
    val scale = readField(decoder) { checkedInt(it.readUInt(32), "numeric scale") }
    require(scale == 0) { "numeric scale must be zero" }
    val value = bigIntegerFromLittleEndianTwosComplement(mantissaBytes)
    require(value > BigInteger.ZERO) { "numeric amount must be greater than zero" }
    require(value <= MAX_U128) { "numeric amount must fit in u128" }
    return value.toString()
}

private fun bigIntegerFromLittleEndianTwosComplement(bytes: ByteArray): BigInteger {
    if (bytes.isEmpty()) return BigInteger.ZERO
    val bigEndian = ByteArray(bytes.size)
    for (idx in bytes.indices) {
        bigEndian[idx] = bytes[bytes.size - 1 - idx]
    }
    return BigInteger(bigEndian)
}

private fun readChainId(decoder: NoritoDecoder): String =
    readField(decoder) { readString(it) }

private fun readAssetDefinitionId(decoder: NoritoDecoder): String {
    val bytes = decoder.readFixedBytes(16, "asset")
    return try {
        AssetDefinitionIdEncoder.encodeFromBytes(bytes)
    } catch (_: IllegalArgumentException) {
        "hex:${bytes.toHex()}"
    }
}

private fun ByteArray.toHex(): String =
    joinToString(separator = "") { byte -> "%02x".format(byte.toInt() and 0xff) }

private fun readVerifyingKeyIdName(payload: ByteArray, flags: Int): String {
    val decoder = NoritoDecoder(payload, flags)
    readField(decoder) { readString(it) } // backend
    val name = readField(decoder) { readString(it) }
    require(decoder.remaining() == 0) { "Trailing bytes after verifier key id" }
    requirePortableId(name, "verifierKeyId")
    return name
}

private fun <T> readField(parent: NoritoDecoder, readPayload: (NoritoDecoder) -> T): T {
    val length = checkedInt(parent.readLength(compact(parent)), "field length")
    val child = NoritoDecoder(parent.readBytes(length), parent.flags, parent.flagsHint)
    val value = readPayload(child)
    require(child.remaining() == 0) { "Trailing bytes after field decode" }
    return value
}

private fun readString(decoder: NoritoDecoder): String {
    val length = checkedInt(decoder.readLength(compact(decoder)), "string length")
    return String(decoder.readBytes(length), StandardCharsets.UTF_8)
}

private fun skipFields(decoder: NoritoDecoder, count: Int) {
    repeat(count) {
        val length = checkedInt(decoder.readLength(compact(decoder)), "field length")
        decoder.readBytes(length)
    }
}

private fun checkedInt(value: Long, field: String): Int {
    require(value >= 0) { "$field must be non-negative" }
    require(value <= Int.MAX_VALUE) { "$field exceeds JVM Int range" }
    return value.toInt()
}

private fun NoritoDecoder.readBool(): Boolean {
    val raw = readByte()
    require(raw == 0 || raw == 1) { "boolean field must be 0 or 1" }
    return raw == 1
}

private fun NoritoDecoder.readFixed32(field: String): ByteArray {
    return readFixedBytes(32, field)
}

private fun NoritoDecoder.readFixedBytes(expectedSize: Int, field: String): ByteArray {
    if (remaining() == expectedSize) {
        return readBytes(expectedSize)
    }
    val bytes = ArrayList<Byte>(expectedSize)
    while (remaining() > 0) {
        val length = readLength(compact(this))
        require(length == 1L) { "$field byte field length must be 1" }
        bytes.add(readByte().toByte())
    }
    require(bytes.size == expectedSize) { "$field must be exactly $expectedSize bytes" }
    return bytes.toByteArray()
}

private fun NoritoDecoder.readRemainingBytes(): ByteArray =
    readBytes(remaining())

private fun compact(encoder: NoritoEncoder): Boolean =
    encoder.flags and NoritoHeader.COMPACT_LEN != 0

private fun compact(decoder: NoritoDecoder): Boolean =
    decoder.flags and NoritoHeader.COMPACT_LEN != 0

private const val MULTISIG_POLICY_VERSION = 1
