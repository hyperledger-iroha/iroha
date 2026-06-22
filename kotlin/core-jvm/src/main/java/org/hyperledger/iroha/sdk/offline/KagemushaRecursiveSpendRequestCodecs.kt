package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.AccountAddressException
import org.hyperledger.iroha.sdk.address.AssetDefinitionIdEncoder
import org.hyperledger.iroha.sdk.address.MultisigMemberPayload
import org.hyperledger.iroha.sdk.address.MultisigPolicyPayload
import org.hyperledger.iroha.sdk.crypto.Blake2b
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
        KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
            recordArchiveBytes,
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_VERIFYING_KEY_RECORD,
            "recordBytes",
        )
    }

    /** Norito archive of `iroha_data_model::proof::VerifyingKeyRecord`. */
    val recordBytes: ByteArray get() = recordArchiveBytes.copyOf()

    fun recordBytes(): ByteArray = recordBytes
}

/** One checked private hop plus the chain context that proof-output archives do not carry. */
class VerifiedFoldHopEvidence(
    proofOutputArchive: ByteArray,
    val verifierRecord: VerifierRecordRef,
    val chainId: String,
    val asset: String,
    rootAfter: ByteArray,
) {
    private val proofOutputArchiveBytes = proofOutputArchive.copyOf()
    private val rootAfterBytes = fixed32(rootAfter, "rootAfter")

    init {
        require(proofOutputArchiveBytes.isNotEmpty()) { "proofOutputArchive must not be empty" }
        requireNonBlankUnpadded(chainId, "chainId")
        try {
            AssetDefinitionIdEncoder.parseAddressBytes(asset)
        } catch (ex: IllegalArgumentException) {
            throw IllegalArgumentException("asset must be a canonical asset definition id", ex)
        }
        require(!isZero32(rootAfterBytes)) { "rootAfter must be non-zero" }
    }

    /** Privacy build-result archive for one confidential-transfer-v2 hop. */
    val proofOutputArchive: ByteArray get() = proofOutputArchiveBytes.copyOf()

    /** Shielded Merkle root after this hop. */
    val rootAfter: ByteArray get() = rootAfterBytes.copyOf()

    fun proofOutputArchiveBytes(): ByteArray = proofOutputArchive

    fun rootAfterBytes(): ByteArray = rootAfter
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

    @JvmOverloads
    constructor(
        recordBundle: ByteArray,
        pallasOpenEnvelopes: ByteArray,
        currentNote: SpendableNoteDescriptor,
        lineageKeyArtifacts: KagemushaRecursiveSpendProver.LineageKeyArtifacts,
        blockHeight: Long? = null,
    ) : this(
        recordBundle = recordBundle,
        pallasOpenEnvelopes = pallasOpenEnvelopes,
        currentNote = currentNote,
        lineageKeyMaterial = lineageKeyMaterialForInit(lineageKeyArtifacts),
        blockHeight = blockHeight,
    )

    private constructor(
        recordBundle: ByteArray,
        pallasOpenEnvelopes: ByteArray,
        currentNote: SpendableNoteDescriptor,
        lineageKeyMaterial: LineageKeyMaterial,
        blockHeight: Long?,
    ) : this(
        recordBundle = recordBundle,
        pallasOpenEnvelopes = pallasOpenEnvelopes,
        currentNote = currentNote,
        lineageVerifierKey = lineageKeyMaterial.verifierKey,
        lineageProvingKeyArchive = lineageKeyMaterial.provingKeyArchive,
        blockHeight = blockHeight,
    )

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
        val recordBundlePayload = KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
            recordBundleArchive,
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_RECORD_BUNDLE,
            "recordBundle",
        )
        val recordBundleHopCount = readVerifiedFoldRecordBundleHopCount(
            recordBundlePayload,
            NoritoHeader.COMPACT_LEN,
            "recordBundle",
        )
        requirePallasOpenEnvelopesArchive(
            pallasOpenEnvelopesArchive,
            expectedEnvelopeCount = recordBundleHopCount,
            field = "pallasOpenEnvelopes",
            maxBytes = KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES,
        )
        validateLineageKeyArtifactsForInit(lineageVerifierKeyBytes, lineageProvingKeyArchiveBytes)
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

    constructor(
        previousBundle: ByteArray,
        recordBundle: ByteArray,
        pallasOpenEnvelopes: ByteArray,
        currentNote: SpendableNoteDescriptor,
        outputProofCircuitId: String?,
        previousLineageVerifierRecord: VerifierRecordRef?,
        previousProofOpenEnvelopes: ByteArray?,
        lineageKeyArtifacts: KagemushaRecursiveSpendProver.LineageKeyArtifacts?,
        blockHeight: Long?,
    ) : this(
        previousBundle = previousBundle,
        recordBundle = recordBundle,
        pallasOpenEnvelopes = pallasOpenEnvelopes,
        currentNote = currentNote,
        outputProofCircuitId = outputProofCircuitId,
        previousLineageVerifierRecord = previousLineageVerifierRecord,
        previousProofOpenEnvelopes = previousProofOpenEnvelopes,
        lineageKeyMaterial = lineageKeyMaterialForAppend(lineageKeyArtifacts),
        blockHeight = blockHeight,
    )

    private constructor(
        previousBundle: ByteArray,
        recordBundle: ByteArray,
        pallasOpenEnvelopes: ByteArray,
        currentNote: SpendableNoteDescriptor,
        outputProofCircuitId: String?,
        previousLineageVerifierRecord: VerifierRecordRef?,
        previousProofOpenEnvelopes: ByteArray?,
        lineageKeyMaterial: LineageKeyMaterial,
        blockHeight: Long?,
    ) : this(
        previousBundle = previousBundle,
        recordBundle = recordBundle,
        pallasOpenEnvelopes = pallasOpenEnvelopes,
        currentNote = currentNote,
        outputProofCircuitId = outputProofCircuitId,
        previousLineageVerifierRecord = previousLineageVerifierRecord,
        previousProofOpenEnvelopes = previousProofOpenEnvelopes,
        lineageVerifierKey = lineageKeyMaterial.verifierKey,
        lineageProvingKeyArchive = lineageKeyMaterial.provingKeyArchive,
        blockHeight = blockHeight,
    )

    init {
        requireNonNegativeHeight(blockHeight)
        val recordBundlePayload = KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
            recordBundleArchive,
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_RECORD_BUNDLE,
            "recordBundle",
        )
        val recordBundleHopCount = readVerifiedFoldRecordBundleHopCount(
            recordBundlePayload,
            NoritoHeader.COMPACT_LEN,
            "recordBundle",
        )
        requirePallasOpenEnvelopesArchive(
            pallasOpenEnvelopesArchive,
            expectedEnvelopeCount = recordBundleHopCount,
            field = "pallasOpenEnvelopes",
            maxBytes = KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES,
        )
        val previousSummary = KagemushaRecursiveSpendRequestCodecs.decodeBundle(previousBundleArchive)
        val normalizedOutput =
            KagemushaRecursiveSpendProver.normalizeAppendOutputCircuitId(outputProofCircuitId)
        val appendNeedsPreviousProofOpenEnvelopes =
            KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
                normalizedOutput,
                previousSummary.hopCount,
            )
        val appendNeedsPreviousLineageVerifierRecord =
            KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
                previousSummary.proofCircuitId,
            )
        val appendNeedsLineageKeyArtifacts =
            KagemushaRecursiveSpendProver.requiresLineageKeyArtifactsForAppendOutput(normalizedOutput)
        require(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                previousSummary.proofCircuitId,
                normalizedOutput,
                previousSummary.hopCount,
            ),
        ) {
            "outputProofCircuitId is not valid for the previous bundle"
        }
        val suppliedLineageKeyMaterial =
            lineageVerifierKeyBytes != null || lineageProvingKeyArchiveBytes != null
        require(!suppliedLineageKeyMaterial || appendNeedsLineageKeyArtifacts) {
            "lineageKeyArtifacts are only valid for lineage append output"
        }
        if (appendNeedsPreviousLineageVerifierRecord) {
            require(previousLineageVerifierRecord != null) {
                "previousLineageVerifierRecord is required for lineage previous bundles"
            }
        } else {
            require(previousLineageVerifierRecord == null) {
                "previousLineageVerifierRecord is only valid for lineage previous bundles"
            }
        }
        require(previousProofOpenEnvelopesArchive == null || appendNeedsPreviousProofOpenEnvelopes) {
            "previousProofOpenEnvelopes are only valid for lineage append output"
        }
        previousProofOpenEnvelopesArchive?.let {
            requirePreviousProofOpenEnvelopesArchive(it)
        }
        if (appendNeedsPreviousProofOpenEnvelopes) {
            require(previousProofOpenEnvelopesArchive != null) {
                "previousProofOpenEnvelopes is required for lineage append output"
            }
        }
        if (appendNeedsLineageKeyArtifacts) {
            require(lineageVerifierKeyBytes != null && lineageVerifierKeyBytes.isNotEmpty()) {
                "lineageVerifierKey is required for lineage append output"
            }
            require(lineageProvingKeyArchiveBytes != null && lineageProvingKeyArchiveBytes.isNotEmpty()) {
                "lineageProvingKeyArchive is required for lineage append output"
            }
            lineageProvingKeyArchiveBytes.let {
                requireValidNestedArchive(it, "lineageProvingKeyArchive")
            }
            validateLineageKeyArtifactsForAppend(lineageVerifierKeyBytes, lineageProvingKeyArchiveBytes)
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
        val bundleSummary = KagemushaRecursiveSpendRequestCodecs.decodeBundle(bundleArchive)
        require(
            !KagemushaRecursiveSpendProver.isLineageProofCircuitId(bundleSummary.proofCircuitId) ||
                lineageVerifierRecord != null,
        ) {
            "lineageVerifierRecord is required for reserved-lineage bundles"
        }
        require(
            KagemushaRecursiveSpendProver.isLineageProofCircuitId(bundleSummary.proofCircuitId) ||
                lineageVerifierRecord == null,
        ) {
            "lineageVerifierRecord is only valid for reserved-lineage bundles"
        }
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
    changeOutput: ByteArray? = null,
    val lineageVerifierRecord: VerifierRecordRef? = null,
    val blockHeight: Long? = null,
) {
    private val bundleArchive = bundle.copyOf()
    private val redeemProofArchive = redeemProof.copyOf()
    private val lineageWitnessArchive = lineageWitness?.copyOf()
    private val changeOutputBytes = changeOutput?.let {
        fixed32(it, "changeOutput").also { fixed ->
            require(!isZero32(fixed)) { "changeOutput must be non-zero" }
        }
    }
    private val canonicalPublicAmount = canonicalU128Decimal(publicAmount, "publicAmount")

    init {
        requireNonNegativeHeight(blockHeight)
        requireNonBlankUnpadded(recipient, "recipient")
        val bundleSummary = KagemushaRecursiveSpendRequestCodecs.decodeBundle(bundleArchive)
        requireRedeemChangeBinding(
            canonicalPublicAmount,
            bundleSummary.currentNote.amount,
            changeOutputBytes != null,
        )
        val finalIsLineage =
            KagemushaRecursiveSpendProver.isLineageProofCircuitId(bundleSummary.proofCircuitId)
        val witnessHasReservedPrevious =
            if (lineageWitnessArchive != null) {
                KagemushaRecursiveSpendRequestCodecs.lineageWitnessHasReservedPreviousProof(
                    lineageWitnessArchive,
                )
            } else {
                false
            }
        if (!finalIsLineage) {
            require(!witnessHasReservedPrevious || lineageVerifierRecord != null) {
                "lineageVerifierRecord is required for lineage witnesses with reserved-lineage previous proofs"
            }
            require(witnessHasReservedPrevious || lineageVerifierRecord == null) {
                "lineageVerifierRecord is only valid for reserved-lineage bundles or lineage witnesses"
            }
        }
        require(
            !KagemushaRecursiveSpendProver.requiresLineageWitnessForRedeem(
                bundleSummary.proofCircuitId,
                bundleSummary.hopCount,
            ) || lineageWitnessArchive != null,
        ) {
            "lineageWitness is required for this bundle"
        }
        require(
            !KagemushaRecursiveSpendProver.isLineageProofCircuitId(bundleSummary.proofCircuitId) ||
                lineageVerifierRecord != null,
        ) {
            "lineageVerifierRecord is required for reserved-lineage bundles"
        }
        KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
            redeemProofArchive,
            KagemushaRecursiveSpendRequestCodecs.SCHEMA_PROOF_ATTACHMENT,
            "redeemProof",
        )
        lineageWitnessArchive?.let {
            KagemushaRecursiveSpendRequestCodecs.compactPayloadForRequest(
                it,
                KagemushaRecursiveSpendRequestCodecs.SCHEMA_LINEAGE_WITNESS,
                "lineageWitness",
            )
        }
    }

    val bundle: ByteArray get() = bundleArchive.copyOf()

    val redeemProof: ByteArray get() = redeemProofArchive.copyOf()

    val lineageWitness: ByteArray? get() = lineageWitnessArchive?.copyOf()

    val changeOutput: ByteArray? get() = changeOutputBytes?.copyOf()

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
    const val SCHEMA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS: String =
        "iroha_data_model::offline::model::KagemushaRecursiveAggregationProofPublicInputs"
    const val SCHEMA_PROOF_ATTACHMENT: String =
        "iroha_data_model::proof::ProofAttachment"
    const val SCHEMA_VERIFYING_KEY_RECORD: String =
        "iroha_data_model::proof::VerifyingKeyRecord"
    const val SCHEMA_OPEN_VERIFY_ENVELOPE: String =
        "iroha_data_model::zk::OpenVerifyEnvelope"

    const val CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID: String =
        "halo2/pasta/ipa/anon-transfer-2x2-merkle16-poseidon-diversified"
    const val CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID: String =
        "halo2/pasta/ipa/anon-unshield-2in-1change-merkle16-poseidon-diversified"

    private const val REQUEST_FLAGS = NoritoHeader.COMPACT_LEN
    private const val PRIVACY_FFI_VERSION_V1 = 1
    private const val PRIVACY_FFI_STATUS_OK = 0
    private const val PRIVACY_SCHEMA_BUILD_PROOF_RESULT = 0x42
    private const val BACKEND_TAG_HALO2_IPA_PASTA = 0L
    private const val CONFIDENTIAL_STATUS_ACTIVE = 1
    private const val CONFIDENTIAL_V2_MAX_PROOF_BYTES = 192 * 1024
    private const val KAGEMUSHA_VERIFIER_NAMESPACE = "offline_kagemusha"
    private const val ZK_BACKEND_HALO2_IPA = "halo2/ipa"
    private const val CONFIDENTIAL_TRANSFER_ALGORITHM_ID = "confidential-transfer-v2"
    private const val CONFIDENTIAL_TRANSFER_ENTRYPOINT = "buildConfidentialTransferProofV2"
    private const val CONFIDENTIAL_UNSHIELD_ALGORITHM_ID = "unshield"
    private const val CONFIDENTIAL_UNSHIELD_ENTRYPOINT = "buildConfidentialUnshieldProofV3"
    private const val CONFIDENTIAL_RECORD_CURVE = "pallas"
    private const val ZK1_MAX_TLV_BYTES = 8 * 1024 * 1024
    private const val ZK1_MAX_INSTANCE_COLUMNS = 64
    private const val ZK1_MAX_INSTANCE_ROWS = 8192

    private val CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA: ByteArray =
        (
            "{\"schema\":\"confidential_transfer_v2\",\"public_inputs\":[\"input_commitment_0\"," +
                "\"input_commitment_1\",\"nullifier_0\",\"nullifier_1\",\"output_commitment_0\"," +
                "\"output_commitment_1\",\"root\",\"asset_tag\",\"chain_tag\"]}"
            ).toByteArray(StandardCharsets.UTF_8)
    private val CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA: ByteArray =
        (
            "{\"schema\":\"confidential_unshield_v3\",\"public_inputs\":[\"input_commitment_0\"," +
                "\"input_commitment_1\",\"nullifier_0\",\"nullifier_1\",\"change_commitment_0\"," +
                "\"root\",\"public_amount\",\"asset_tag\",\"chain_tag\"]}"
            ).toByteArray(StandardCharsets.UTF_8)
    private val ZK1_MAGIC = byteArrayOf(0x5a, 0x4b, 0x31, 0x00)

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
    fun buildPallasOpenEnvelopesArchive(hops: List<VerifiedFoldHopEvidence>?): ByteArray {
        require(!hops.isNullOrEmpty()) { "hops must not be empty" }
        return buildPallasOpenEnvelopesArchiveForRecordBundle(buildVerifiedFoldRecordBundle(hops))
    }

    @JvmStatic
    fun buildPallasOpenEnvelopesArchiveForRecordBundle(recordBundle: ByteArray?): ByteArray {
        require(recordBundle != null) { "recordBundle is required" }
        compactPayloadForRequest(recordBundle, SCHEMA_RECORD_BUNDLE, "recordBundle")
        return KagemushaRecursiveSpendProver.buildPallasOpenEnvelopesArchive(recordBundle)
    }

    @JvmStatic
    fun buildPreviousProofOpenEnvelopesArchive(previousBundle: ByteArray?): ByteArray {
        require(previousBundle != null) { "previousBundle is required" }
        decodeBundle(previousBundle)
        return KagemushaRecursiveSpendProver.buildPreviousProofOpenEnvelopesArchive(previousBundle)
    }

    @JvmStatic
    fun buildVerifiedFoldRecordBundle(
        hopProofOutputArchives: List<ByteArray>?,
        hopVerifierRecords: List<VerifierRecordRef>?,
    ): ByteArray {
        require(!hopProofOutputArchives.isNullOrEmpty()) { "hopProofOutputArchives must not be empty" }
        require(hopVerifierRecords != null && hopVerifierRecords.size == hopProofOutputArchives.size) {
            "hopVerifierRecords must match hopProofOutputArchives"
        }
        throw IllegalArgumentException(
            "chainId, asset, and rootAfter are required to build KagemushaVerifiedFoldRecordBundle; " +
                "use VerifiedFoldHopEvidence inputs instead",
        )
    }

    @JvmStatic
    fun buildVerifiedFoldRecordBundle(hops: List<VerifiedFoldHopEvidence>?): ByteArray {
        require(!hops.isNullOrEmpty()) { "hops must not be empty" }
        require(hops.size <= KagemushaRecursiveSpendProver.COMPACT_TOKEN_MAX_HOPS) {
            "hops must not exceed ${KagemushaRecursiveSpendProver.COMPACT_TOKEN_MAX_HOPS}"
        }
        val prepared = hops.mapIndexed { index, hop -> prepareTransferHop(index, hop) }
        val chainId = prepared.first().chainId
        val asset = prepared.first().asset
        var expectedRootBefore: ByteArray? = null
        for ((index, hop) in prepared.withIndex()) {
            require(hop.chainId == chainId) { "hop $index chainId does not match first hop" }
            require(hop.asset == asset) { "hop $index asset does not match first hop" }
            expectedRootBefore?.let { previousRoot ->
                require(hop.publicInputs.rootBefore.contentEquals(previousRoot)) {
                    "hop $index rootBefore must equal previous hop rootAfter"
                }
            }
            require(!hop.publicInputs.rootBefore.contentEquals(hop.rootAfter)) {
                "hop $index rootAfter must differ from rootBefore"
            }
            expectedRootBefore = hop.rootAfter
        }
        return NoritoCodec.encode(prepared, SCHEMA_RECORD_BUNDLE, VerifiedFoldRecordBundleAdapter, REQUEST_FLAGS)
    }

    @JvmStatic
    fun buildRedeemProofAttachment(
        unshieldProofOutputArchive: ByteArray?,
        unshieldVerifierRecord: VerifierRecordRef?,
    ): ByteArray {
        require(unshieldProofOutputArchive != null) { "unshieldProofOutputArchive is required" }
        require(unshieldVerifierRecord != null) { "unshieldVerifierRecord is required" }
        val proof = parsePrivacyBuildResult(
            unshieldProofOutputArchive,
            CONFIDENTIAL_UNSHIELD_ALGORITHM_ID,
            CONFIDENTIAL_UNSHIELD_ENTRYPOINT,
            "unshieldProofOutputArchive",
        )
        val envelope = decodeOpenVerifyEnvelope(proof.proof, "unshield proof")
        val verifierRecord = decodeAndValidateVerifierRecord(
            unshieldVerifierRecord,
            envelope,
            expectedCircuitId = CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID,
            expectedSchema = CONFIDENTIAL_UNSHIELD_V3_PUBLIC_INPUTS_SCHEMA,
            proofArchiveSize = proof.proof.size,
            label = "unshieldVerifierRecord",
        )
        return NoritoCodec.encode(
            proofAttachmentPayload(envelope, verifierRecord),
            SCHEMA_PROOF_ATTACHMENT,
            RawPayloadAdapter,
            REQUEST_FLAGS,
        )
    }

    @JvmStatic
    @JvmOverloads
    fun buildRecursiveSpendInitRequest(
        hop: VerifiedFoldHopEvidence?,
        pallasOpenEnvelopes: ByteArray?,
        spendableNote: SpendableNoteDescriptor?,
        lineageVerifierKey: ByteArray? = null,
        lineageProvingKeyArchive: ByteArray? = null,
        blockHeight: Long? = null,
    ): ByteArray {
        require(hop != null) { "hop is required" }
        require(pallasOpenEnvelopes != null) { "pallasOpenEnvelopes is required" }
        require(spendableNote != null) { "spendableNote is required" }
        return encodeInitRequest(
            InitSpendRequest(
                recordBundle = buildVerifiedFoldRecordBundle(listOf(hop)),
                pallasOpenEnvelopes = pallasOpenEnvelopes,
                currentNote = spendableNote,
                lineageVerifierKey = lineageVerifierKey,
                lineageProvingKeyArchive = lineageProvingKeyArchive,
                blockHeight = blockHeight,
            ),
        )
    }

    @JvmStatic
    @JvmOverloads
    fun buildRecursiveSpendInitRequest(
        hop: VerifiedFoldHopEvidence?,
        spendableNote: SpendableNoteDescriptor?,
        lineageVerifierKey: ByteArray? = null,
        lineageProvingKeyArchive: ByteArray? = null,
        blockHeight: Long? = null,
    ): ByteArray {
        require(hop != null) { "hop is required" }
        require(spendableNote != null) { "spendableNote is required" }
        val recordBundle = buildVerifiedFoldRecordBundle(listOf(hop))
        preflightInitLineageKeyMaterialForAutoGeneration(
            lineageVerifierKey,
            lineageProvingKeyArchive,
        )
        val pallasOpenEnvelopes = buildPallasOpenEnvelopesArchiveForRecordBundle(recordBundle)
        return encodeInitRequest(
            InitSpendRequest(
                recordBundle = recordBundle,
                pallasOpenEnvelopes = pallasOpenEnvelopes,
                currentNote = spendableNote,
                lineageVerifierKey = lineageVerifierKey,
                lineageProvingKeyArchive = lineageProvingKeyArchive,
                blockHeight = blockHeight,
            ),
        )
    }

    @JvmStatic
    @JvmOverloads
    fun buildRecursiveSpendInitRequest(
        hop: VerifiedFoldHopEvidence?,
        pallasOpenEnvelopes: ByteArray?,
        spendableNote: SpendableNoteDescriptor?,
        lineageKeyArtifacts: KagemushaRecursiveSpendProver.LineageKeyArtifacts,
        blockHeight: Long? = null,
    ): ByteArray {
        require(hop != null) { "hop is required" }
        require(pallasOpenEnvelopes != null) { "pallasOpenEnvelopes is required" }
        require(spendableNote != null) { "spendableNote is required" }
        return encodeInitRequest(
            InitSpendRequest(
                recordBundle = buildVerifiedFoldRecordBundle(listOf(hop)),
                pallasOpenEnvelopes = pallasOpenEnvelopes,
                currentNote = spendableNote,
                lineageKeyArtifacts = lineageKeyArtifacts,
                blockHeight = blockHeight,
            ),
        )
    }

    @JvmStatic
    @JvmOverloads
    fun buildRecursiveSpendInitRequest(
        hop: VerifiedFoldHopEvidence?,
        spendableNote: SpendableNoteDescriptor?,
        lineageKeyArtifacts: KagemushaRecursiveSpendProver.LineageKeyArtifacts,
        blockHeight: Long? = null,
    ): ByteArray {
        require(hop != null) { "hop is required" }
        require(spendableNote != null) { "spendableNote is required" }
        val recordBundle = buildVerifiedFoldRecordBundle(listOf(hop))
        val checkedLineageKeyArtifacts = requireInitLineageKeyArtifacts(lineageKeyArtifacts)
        val pallasOpenEnvelopes = buildPallasOpenEnvelopesArchiveForRecordBundle(recordBundle)
        return encodeInitRequest(
            InitSpendRequest(
                recordBundle = recordBundle,
                pallasOpenEnvelopes = pallasOpenEnvelopes,
                currentNote = spendableNote,
                lineageKeyArtifacts = checkedLineageKeyArtifacts,
                blockHeight = blockHeight,
            ),
        )
    }

    @JvmStatic
    @JvmOverloads
    fun buildRecursiveSpendInitRequest(
        proofOutputArchive: ByteArray?,
        verifierRecord: VerifierRecordRef?,
        spendableNote: SpendableNoteDescriptor?,
        lineageVerifierKey: ByteArray? = null,
        lineageProvingKeyArchive: ByteArray? = null,
        blockHeight: Long? = null,
    ): ByteArray {
        require(proofOutputArchive != null && proofOutputArchive.isNotEmpty()) {
            "proofOutputArchive must not be empty"
        }
        require(verifierRecord != null) { "verifierRecord is required" }
        require(spendableNote != null) { "spendableNote is required" }
        failClosedProofOnlyRecursiveSpendRequest()
    }

    @JvmStatic
    @JvmOverloads
    fun buildRecursiveSpendAppendRequest(
        previousBundle: ByteArray?,
        hop: VerifiedFoldHopEvidence?,
        pallasOpenEnvelopes: ByteArray?,
        spendableNote: SpendableNoteDescriptor?,
        outputCircuitId: String? = null,
        previousLineageVerifierRecord: VerifierRecordRef? = null,
        previousProofOpenEnvelopes: ByteArray? = null,
        lineageVerifierKey: ByteArray? = null,
        lineageProvingKeyArchive: ByteArray? = null,
        blockHeight: Long? = null,
    ): ByteArray {
        require(previousBundle != null) { "previousBundle is required" }
        require(hop != null) { "hop is required" }
        require(pallasOpenEnvelopes != null) { "pallasOpenEnvelopes is required" }
        require(spendableNote != null) { "spendableNote is required" }
        return encodeAppendRequest(
            AppendSpendRequest(
                previousBundle = previousBundle,
                recordBundle = buildVerifiedFoldRecordBundle(listOf(hop)),
                pallasOpenEnvelopes = pallasOpenEnvelopes,
                currentNote = spendableNote,
                outputProofCircuitId = outputCircuitId,
                previousLineageVerifierRecord = previousLineageVerifierRecord,
                previousProofOpenEnvelopes = previousProofOpenEnvelopes,
                lineageVerifierKey = lineageVerifierKey,
                lineageProvingKeyArchive = lineageProvingKeyArchive,
                blockHeight = blockHeight,
            ),
        )
    }

    @JvmStatic
    @JvmOverloads
    fun buildRecursiveSpendAppendRequest(
        previousBundle: ByteArray?,
        hop: VerifiedFoldHopEvidence?,
        spendableNote: SpendableNoteDescriptor?,
        outputCircuitId: String? = null,
        previousLineageVerifierRecord: VerifierRecordRef? = null,
        previousProofOpenEnvelopes: ByteArray? = null,
        lineageVerifierKey: ByteArray? = null,
        lineageProvingKeyArchive: ByteArray? = null,
        blockHeight: Long? = null,
    ): ByteArray {
        require(previousBundle != null) { "previousBundle is required" }
        require(hop != null) { "hop is required" }
        require(spendableNote != null) { "spendableNote is required" }
        val recordBundle = buildVerifiedFoldRecordBundle(listOf(hop))
        val previousSummary = preflightAppendPreviousLineageForAutoGeneration(
            previousBundle,
            outputCircuitId,
            previousLineageVerifierRecord,
        )
        val pallasOpenEnvelopes = buildPallasOpenEnvelopesArchiveForRecordBundle(recordBundle)
        val previousOpenEnvelopes = previousProofOpenEnvelopesOrGenerated(
            previousBundle,
            outputCircuitId,
            previousSummary,
            previousProofOpenEnvelopes,
        )
        return encodeAppendRequest(
            AppendSpendRequest(
                previousBundle = previousBundle,
                recordBundle = recordBundle,
                pallasOpenEnvelopes = pallasOpenEnvelopes,
                currentNote = spendableNote,
                outputProofCircuitId = outputCircuitId,
                previousLineageVerifierRecord = previousLineageVerifierRecord,
                previousProofOpenEnvelopes = previousOpenEnvelopes,
                lineageVerifierKey = lineageVerifierKey,
                lineageProvingKeyArchive = lineageProvingKeyArchive,
                blockHeight = blockHeight,
            ),
        )
    }

    @JvmStatic
    fun buildRecursiveSpendAppendRequest(
        previousBundle: ByteArray?,
        hop: VerifiedFoldHopEvidence?,
        pallasOpenEnvelopes: ByteArray?,
        spendableNote: SpendableNoteDescriptor?,
        outputCircuitId: String?,
        previousLineageVerifierRecord: VerifierRecordRef?,
        previousProofOpenEnvelopes: ByteArray?,
        lineageKeyArtifacts: KagemushaRecursiveSpendProver.LineageKeyArtifacts?,
        blockHeight: Long?,
    ): ByteArray {
        require(previousBundle != null) { "previousBundle is required" }
        require(hop != null) { "hop is required" }
        require(pallasOpenEnvelopes != null) { "pallasOpenEnvelopes is required" }
        require(spendableNote != null) { "spendableNote is required" }
        return encodeAppendRequest(
            AppendSpendRequest(
                previousBundle = previousBundle,
                recordBundle = buildVerifiedFoldRecordBundle(listOf(hop)),
                pallasOpenEnvelopes = pallasOpenEnvelopes,
                currentNote = spendableNote,
                outputProofCircuitId = outputCircuitId,
                previousLineageVerifierRecord = previousLineageVerifierRecord,
                previousProofOpenEnvelopes = previousProofOpenEnvelopes,
                lineageKeyArtifacts = lineageKeyArtifacts,
                blockHeight = blockHeight,
            ),
        )
    }

    @JvmStatic
    fun buildRecursiveSpendAppendRequest(
        previousBundle: ByteArray?,
        hop: VerifiedFoldHopEvidence?,
        spendableNote: SpendableNoteDescriptor?,
        outputCircuitId: String?,
        previousLineageVerifierRecord: VerifierRecordRef?,
        previousProofOpenEnvelopes: ByteArray?,
        lineageKeyArtifacts: KagemushaRecursiveSpendProver.LineageKeyArtifacts?,
        blockHeight: Long?,
    ): ByteArray {
        require(previousBundle != null) { "previousBundle is required" }
        require(hop != null) { "hop is required" }
        require(spendableNote != null) { "spendableNote is required" }
        val recordBundle = buildVerifiedFoldRecordBundle(listOf(hop))
        val previousSummary = preflightAppendPreviousLineageForAutoGeneration(
            previousBundle,
            outputCircuitId,
            previousLineageVerifierRecord,
        )
        val pallasOpenEnvelopes = buildPallasOpenEnvelopesArchiveForRecordBundle(recordBundle)
        val previousOpenEnvelopes = previousProofOpenEnvelopesOrGenerated(
            previousBundle,
            outputCircuitId,
            previousSummary,
            previousProofOpenEnvelopes,
        )
        return encodeAppendRequest(
            AppendSpendRequest(
                previousBundle = previousBundle,
                recordBundle = recordBundle,
                pallasOpenEnvelopes = pallasOpenEnvelopes,
                currentNote = spendableNote,
                outputProofCircuitId = outputCircuitId,
                previousLineageVerifierRecord = previousLineageVerifierRecord,
                previousProofOpenEnvelopes = previousOpenEnvelopes,
                lineageKeyArtifacts = lineageKeyArtifacts,
                blockHeight = blockHeight,
            ),
        )
    }

    @JvmStatic
    @JvmOverloads
    fun buildRecursiveSpendAppendRequest(
        previousBundle: ByteArray?,
        proofOutputArchive: ByteArray?,
        verifierRecord: VerifierRecordRef?,
        spendableNote: SpendableNoteDescriptor?,
        outputCircuitId: String? = null,
        previousLineageVerifierRecord: VerifierRecordRef? = null,
        previousProofOpenEnvelopes: ByteArray? = null,
        lineageVerifierKey: ByteArray? = null,
        lineageProvingKeyArchive: ByteArray? = null,
        blockHeight: Long? = null,
    ): ByteArray {
        require(previousBundle != null) { "previousBundle is required" }
        require(proofOutputArchive != null && proofOutputArchive.isNotEmpty()) {
            "proofOutputArchive must not be empty"
        }
        require(verifierRecord != null) { "verifierRecord is required" }
        require(spendableNote != null) { "spendableNote is required" }
        failClosedProofOnlyRecursiveSpendRequest()
    }

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
        require(KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(proofCircuitId)) {
            "bundle.proof_circuit_id unsupported recursive proof circuit id"
        }
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

    internal fun lineageWitnessHasReservedPreviousProof(archive: ByteArray): Boolean {
        val payload = requirePayloadArchive(archive, SCHEMA_LINEAGE_WITNESS, "lineageWitness")
        require(payload.flags == REQUEST_FLAGS) {
            "lineageWitness must use compact Norito layout"
        }
        val decoder = NoritoDecoder(payload.payload, payload.flags)
        skipFields(decoder, 3)
        val previousProofsPayload = readField(decoder) { it.readRemainingBytes() }
        require(decoder.remaining() == 0) { "Trailing bytes after lineageWitness" }
        val previousProofs = NoritoDecoder(previousProofsPayload, payload.flags)
        val count = checkedInt(
            previousProofs.readUInt(64),
            "lineageWitness.previousRecursiveProofs count",
        )
        var hasReserved = false
        repeat(count) { index ->
            val itemLength = checkedInt(
                previousProofs.readLength(compact(previousProofs)),
                "lineageWitness.previousRecursiveProofs[$index] length",
            )
            val proofPayload = previousProofs.readBytes(itemLength)
            val circuitId = readPreviousRecursiveProofCircuitId(proofPayload, payload.flags)
            hasReserved = hasReserved || KagemushaRecursiveSpendProver.isLineageProofCircuitId(circuitId)
        }
        require(previousProofs.remaining() == 0) {
            "Trailing bytes after lineageWitness.previousRecursiveProofs"
        }
        return hasReserved
    }

    private fun readPreviousRecursiveProofCircuitId(payload: ByteArray, flags: Int): String {
        val decoder = NoritoDecoder(payload, flags)
        val verifierKeyIdPayload = readField(decoder) { it.readRemainingBytes() }
        skipFields(decoder, 3)
        require(decoder.remaining() == 0) {
            "Trailing bytes after lineageWitness.previousRecursiveProofs"
        }
        val verifierKeyId = readVerifyingKeyId(verifierKeyIdPayload, flags)
        require(KagemushaRecursiveSpendProver.isSupportedPreviousProofCircuitId(verifierKeyId.name)) {
            "lineageWitness.previousRecursiveProofs verifierKeyId unsupported recursive proof circuit id"
        }
        return verifierKeyId.name
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

    private fun failClosedProofOnlyRecursiveSpendRequest(): Nothing {
        throw IllegalArgumentException(
            "recursive spend requests require explicit VerifiedFoldHopEvidence and a bridge-generated or explicit " +
                "Pallas open-envelopes archive; privacy proof outputs alone do not carry " +
                "Pallas IPA opening envelopes, chainId, asset, or rootAfter",
        )
    }

    private fun preflightInitLineageKeyMaterialForAutoGeneration(
        lineageVerifierKey: ByteArray?,
        lineageProvingKeyArchive: ByteArray?,
    ) {
        require(lineageVerifierKey != null) {
            "lineageVerifierKey is required for recursive spend init"
        }
        require(lineageProvingKeyArchive != null) {
            "lineageProvingKeyArchive is required for recursive spend init"
        }
        require(lineageVerifierKey.isNotEmpty()) { "lineageVerifierKey must not be empty" }
        require(lineageProvingKeyArchive.isNotEmpty()) {
            "lineageProvingKeyArchive must not be empty"
        }
        validateLineageKeyArtifactsForInit(lineageVerifierKey, lineageProvingKeyArchive)
    }

    private fun preflightAppendPreviousLineageForAutoGeneration(
        previousBundle: ByteArray,
        outputCircuitId: String?,
        previousLineageVerifierRecord: VerifierRecordRef?,
    ): SpendBundleSummary {
        val previousSummary = decodeBundle(previousBundle)
        val normalizedOutput =
            KagemushaRecursiveSpendProver.normalizeAppendOutputCircuitId(outputCircuitId)
        require(
            KagemushaRecursiveSpendProver.canSelectAppendOutputCircuitId(
                previousSummary.proofCircuitId,
                normalizedOutput,
                previousSummary.hopCount,
            ),
        ) {
            "outputProofCircuitId is not valid for the previous bundle"
        }
        val appendNeedsPreviousLineageVerifierRecord =
            KagemushaRecursiveSpendProver.requiresPreviousLineageVerifierRecordForAppend(
                previousSummary.proofCircuitId,
            )
        if (appendNeedsPreviousLineageVerifierRecord) {
            require(previousLineageVerifierRecord != null) {
                "previousLineageVerifierRecord is required for lineage previous bundles"
            }
        } else {
            require(previousLineageVerifierRecord == null) {
                "previousLineageVerifierRecord is only valid for lineage previous bundles"
            }
        }
        return previousSummary
    }

    private fun previousProofOpenEnvelopesOrGenerated(
        previousBundle: ByteArray,
        outputCircuitId: String?,
        previousSummary: SpendBundleSummary,
        provided: ByteArray?,
    ): ByteArray? {
        if (provided != null) return provided
        return if (
            KagemushaRecursiveSpendProver.requiresPreviousProofOpenEnvelopesForAppend(
                outputCircuitId,
                previousSummary.hopCount,
            )
        ) {
            buildPreviousProofOpenEnvelopesArchive(previousBundle)
        } else {
            null
        }
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
            writeField(encoder) { writeOptionFixed32(it, value.changeOutput) }
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
        val domain = readField(decoder) { readString(it) }
        require(domain == KagemushaRecursiveSpendProver.RECURSIVE_SPEND_ACCUMULATOR_DOMAIN) {
            "bundle.accumulator.domain must be ${KagemushaRecursiveSpendProver.RECURSIVE_SPEND_ACCUMULATOR_DOMAIN}"
        }
        val chainId = readField(decoder) { readChainId(it) }
        val asset = readField(decoder) { readAssetDefinitionId(it) }
        val initialRoot = readField(decoder) { it.readFixed32("initial_root") }
        val finalRoot = readField(decoder) { it.readFixed32("final_root") }
        skipFields(decoder, 1) // topup_anchor_nullifiers
        val hopCount = readField(decoder) { checkedInt(it.readUInt(32), "hop_count") }
        require(hopCount in 1..KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1) {
            "bundle.accumulator.hop_count must be in 1..${KagemushaRecursiveSpendProver.RECURSIVE_SPEND_LINEAGE_WITNESSLESS_MAX_HOPS_V1}"
        }
        skipFields(decoder, 15)
        val currentNote = readField(decoder) { readSpendableNote(it) }
        require(decoder.remaining() == 0) { "Trailing bytes after accumulator" }
        return AccumulatorSummary(chainId, asset, initialRoot, finalRoot, hopCount, currentNote)
    }

    private fun readRecursiveProofCircuitId(payload: ByteArray, flags: Int): String {
        val decoder = NoritoDecoder(payload, flags)
        val verifierKeyIdPayload = readField(decoder) { it.readRemainingBytes() }
        val publicInputsPayload = readField(decoder) { it.readRemainingBytes() }
        require(publicInputsPayload.isNotEmpty()) { "bundle.proof_public_inputs empty recursive proof inputs" }
        val publicInputsHash = readField(decoder) { it.readFixed32("proof_public_inputs_hash") }
        require(!isZero32(publicInputsHash)) { "bundle.proof_public_inputs_hash must be non-zero" }
        val publicInputsArchive = NoritoCodec.encode(
            publicInputsPayload,
            SCHEMA_RECURSIVE_AGGREGATION_PROOF_PUBLIC_INPUTS,
            RawPayloadAdapter,
            NoritoHeader.COMPACT_LEN,
        )
        require(publicInputsHash.contentEquals(irohaHash(publicInputsArchive))) {
            "bundle.proof_public_inputs_hash mismatch"
        }
        val proofPayload = readField(decoder) { it.readRemainingBytes() }
        require(decoder.remaining() == 0) { "Trailing bytes after recursive proof" }
        val verifierKeyId = readVerifyingKeyId(verifierKeyIdPayload, flags)
        val proofBackend = readProofBoxBackend(proofPayload, flags)
        require(proofBackend == verifierKeyId.backend) {
            "bundle.proof_backend recursive proof backend mismatch"
        }
        return verifierKeyId.name
    }

    private object RawPayloadAdapter : TypeAdapter<ByteArray> {
        override fun encode(encoder: NoritoEncoder, value: ByteArray) {
            encoder.writeBytes(value)
        }

        override fun decode(decoder: NoritoDecoder): ByteArray {
            throw UnsupportedOperationException("raw payload archives are encode-only")
        }
    }

    private object VerifiedFoldRecordBundleAdapter : TypeAdapter<List<PreparedVerifiedFoldHop>> {
        override fun encode(encoder: NoritoEncoder, value: List<PreparedVerifiedFoldHop>) {
            writeField(encoder) { bundle ->
                writeField(bundle) { writeChainId(it, value.first().chainId) }
                writeField(bundle) { writeAssetDefinitionId(it, value.first().asset) }
                writeField(bundle) { writeVerifiedFoldSteps(it, value) }
            }
            writeField(encoder) { writeVerifiedFoldVerifierRecords(it, value) }
        }

        override fun decode(decoder: NoritoDecoder): List<PreparedVerifiedFoldHop> {
            throw UnsupportedOperationException("verified fold bundles are encode-only")
        }
    }

    private fun prepareTransferHop(index: Int, hop: VerifiedFoldHopEvidence): PreparedVerifiedFoldHop {
        val proof = parsePrivacyBuildResult(
            hop.proofOutputArchive,
            CONFIDENTIAL_TRANSFER_ALGORITHM_ID,
            CONFIDENTIAL_TRANSFER_ENTRYPOINT,
            "hop $index proofOutputArchive",
        )
        val envelope = decodeOpenVerifyEnvelope(proof.proof, "hop $index proof")
        val publicInputs = parseTransferPublicInputs(envelope.proofBytes, "hop $index")
        require(!isZero32(publicInputs.rootBefore)) { "hop $index rootBefore must be non-zero" }
        val verifierRecord = decodeAndValidateVerifierRecord(
            hop.verifierRecord,
            envelope,
            expectedCircuitId = CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
            expectedSchema = CONFIDENTIAL_TRANSFER_V2_PUBLIC_INPUTS_SCHEMA,
            proofArchiveSize = proof.proof.size,
            label = "hop $index verifierRecord",
        )
        return PreparedVerifiedFoldHop(
            chainId = hop.chainId,
            asset = hop.asset,
            rootAfter = hop.rootAfter,
            publicInputs = publicInputs,
            envelope = envelope,
            verifierRecord = verifierRecord,
        )
    }

    private fun parsePrivacyBuildResult(
        archive: ByteArray,
        expectedAlgorithmId: String,
        expectedEntrypoint: String,
        label: String,
    ): PrivacyBuildResult {
        val payload = requirePrivacyBuildResultPayload(archive, label)
        require(payload.flags == REQUEST_FLAGS) { "$label must use compact Norito layout" }
        val decoder = NoritoDecoder(payload.payload, payload.flags)
        val version = readField(decoder) { checkedInt(it.readUInt(32), "$label.version") }
        val status = readField(decoder) { checkedInt(it.readUInt(32), "$label.status") }
        val errorCode = readField(decoder) { checkedInt(it.readUInt(32), "$label.error_code") }
        val message = readField(decoder) { readString(it) }
        val algorithmId = readField(decoder) { readString(it) }
        val entrypoint = readField(decoder) { readString(it) }
        val vkRef = readField(decoder) { readString(it) }
        val publicInputs = readField(decoder) { readBytesVec(it) }
        val proof = readField(decoder) { readBytesVec(it) }
        val verified = readField(decoder) { it.readBool() }
        require(decoder.remaining() == 0) { "Trailing bytes after $label" }
        require(version == PRIVACY_FFI_VERSION_V1) { "$label version must be $PRIVACY_FFI_VERSION_V1" }
        require(status == PRIVACY_FFI_STATUS_OK && errorCode == 0) {
            "$label must be a successful privacy proof result: status=$status error_code=$errorCode"
        }
        require(message.isEmpty()) { "$label success message must be empty" }
        require(algorithmId == expectedAlgorithmId) { "$label algorithm_id must be $expectedAlgorithmId" }
        require(entrypoint == expectedEntrypoint) { "$label entrypoint must be $expectedEntrypoint" }
        requirePortableId(vkRef, "$label.vk_ref")
        require(publicInputs.isEmpty()) { "$label public_inputs must be empty; envelope carries authoritative inputs" }
        require(proof.isNotEmpty()) { "$label proof must not be empty" }
        require(!verified) { "$label build results must not claim online verification" }
        return PrivacyBuildResult(proof)
    }

    private fun requirePrivacyBuildResultPayload(archive: ByteArray, label: String): ArchivePayload {
        require(archive.isNotEmpty()) { "$label must not be empty" }
        require(archive.size <= KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES) {
            "$label must not exceed ${KagemushaRecursiveSpendProver.NATIVE_ARCHIVE_MAX_BYTES} bytes"
        }
        val decoded = try {
            NoritoHeader.decode(archive, null)
        } catch (ex: IllegalArgumentException) {
            throw IllegalArgumentException("$label must be a valid privacy build-result Norito archive", ex)
        }
        require(decoded.header.schemaHash.all { (it.toInt() and 0xff) == PRIVACY_SCHEMA_BUILD_PROOF_RESULT }) {
            "$label must use privacy build-result schema marker"
        }
        require(decoded.header.compression == NoritoHeader.COMPRESSION_NONE) { "$label must not be compressed" }
        require(decoded.header.payloadLength > 0) { "$label must contain a non-empty Norito payload" }
        decoded.header.validateChecksum(decoded.payload)
        return ArchivePayload(decoded.payload, decoded.header.flags)
    }

    private fun decodeOpenVerifyEnvelope(archive: ByteArray, label: String): OpenVerifyEnvelopeValue {
        val payload = requirePayloadArchive(archive, SCHEMA_OPEN_VERIFY_ENVELOPE, label)
        require(payload.flags == REQUEST_FLAGS) { "$label must use compact Norito layout" }
        val decoder = NoritoDecoder(payload.payload, payload.flags)
        val backendTag = readField(decoder) { it.readUInt(32) }
        val circuitId = readField(decoder) { readString(it) }
        val vkHash = readField(decoder) { it.readFixed32("vk_hash") }
        val publicInputs = readField(decoder) { readBytesVec(it) }
        val proofBytes = readField(decoder) { readBytesVec(it) }
        val aux = readField(decoder) { readBytesVec(it) }
        require(decoder.remaining() == 0) { "Trailing bytes after $label OpenVerifyEnvelope" }
        require(backendTag == BACKEND_TAG_HALO2_IPA_PASTA) { "$label backend must be Halo2IpaPasta" }
        requirePortableId(circuitId, "$label.circuit_id")
        require(!isZero32(vkHash)) { "$label vk_hash must be non-zero" }
        require(proofBytes.isNotEmpty()) { "$label proof_bytes must not be empty" }
        require(aux.isEmpty()) { "$label aux must be empty" }
        return OpenVerifyEnvelopeValue(
            archive = archive.copyOf(),
            circuitId = circuitId,
            vkHash = vkHash,
            publicInputs = publicInputs,
            proofBytes = proofBytes,
        )
    }

    private fun decodeAndValidateVerifierRecord(
        ref: VerifierRecordRef,
        envelope: OpenVerifyEnvelopeValue,
        expectedCircuitId: String,
        expectedSchema: ByteArray,
        proofArchiveSize: Int,
        label: String,
    ): VerifierRecordValue {
        val recordPayload = compactPayloadForRequest(ref.recordBytes, SCHEMA_VERIFYING_KEY_RECORD, label)
        val record = decodeVerifierRecordPayload(recordPayload, label)
        val id = parseVerifierKeyId(ref.verifierKeyId, "$label.verifierKeyId")
        require(id.backend == ZK_BACKEND_HALO2_IPA) { "$label verifierKeyId backend must be $ZK_BACKEND_HALO2_IPA" }
        require(record.status == CONFIDENTIAL_STATUS_ACTIVE) { "$label status must be Active" }
        require(record.namespace == KAGEMUSHA_VERIFIER_NAMESPACE) {
            "$label namespace must be $KAGEMUSHA_VERIFIER_NAMESPACE"
        }
        require(record.backendTag == BACKEND_TAG_HALO2_IPA_PASTA) { "$label backend must be Halo2IpaPasta" }
        require(record.curve == CONFIDENTIAL_RECORD_CURVE) { "$label curve must be $CONFIDENTIAL_RECORD_CURVE" }
        require(record.circuitId == expectedCircuitId) { "$label circuit_id must be $expectedCircuitId" }
        require(envelope.circuitId == expectedCircuitId) { "$label envelope circuit_id must be $expectedCircuitId" }
        require(envelope.publicInputs.contentEquals(expectedSchema)) { "$label public-input schema mismatch" }
        require(record.publicInputsSchemaHash.contentEquals(Blake2b.digest256(expectedSchema))) {
            "$label public_inputs_schema_hash mismatch"
        }
        require(record.commitment.contentEquals(envelope.vkHash)) { "$label commitment must match envelope vk_hash" }
        require(record.maxProofBytes > 0) { "$label max_proof_bytes must be non-zero" }
        require(record.maxProofBytes <= CONFIDENTIAL_V2_MAX_PROOF_BYTES) {
            "$label max_proof_bytes exceeds confidential-v2 cap"
        }
        require(proofArchiveSize <= record.maxProofBytes) { "$label proof exceeds max_proof_bytes" }
        val key = record.key ?: throw IllegalArgumentException("$label must include inline verifier key")
        require(key.backend == ZK_BACKEND_HALO2_IPA) { "$label verifier key backend must be $ZK_BACKEND_HALO2_IPA" }
        require(key.bytes.isNotEmpty()) { "$label verifier key bytes must not be empty" }
        require(record.vkLen == key.bytes.size) { "$label vk_len must equal inline verifier key bytes length" }
        require(record.commitment.contentEquals(verifyingKeyCommitment(key.backend, key.bytes))) {
            "$label inline verifier-key commitment mismatch"
        }
        return VerifierRecordValue(
            id = id,
            recordPayload = recordPayload,
            commitment = record.commitment,
            key = key,
        )
    }

    private fun decodeVerifierRecordPayload(payload: ByteArray, label: String): DecodedVerifierRecord {
        val decoder = NoritoDecoder(payload, REQUEST_FLAGS)
        readField(decoder) { checkedInt(it.readUInt(32), "$label.version") }
        val circuitId = readField(decoder) { readString(it) }
        readField(decoder) { readOptionString(it) }
        val namespace = readField(decoder) { readString(it) }
        val backendTag = readField(decoder) { it.readUInt(32) }
        val curve = readField(decoder) { readString(it) }
        val schemaHash = readField(decoder) { it.readFixed32("public_inputs_schema_hash") }
        val commitment = readField(decoder) { it.readFixed32("commitment") }
        val vkLen = readField(decoder) { checkedInt(it.readUInt(32), "$label.vk_len") }
        val maxProofBytes = readField(decoder) { checkedInt(it.readUInt(32), "$label.max_proof_bytes") }
        readField(decoder) { readOptionString(it) }
        readField(decoder) { readOptionString(it) }
        readField(decoder) { readOptionString(it) }
        readField(decoder) { readOptionU64Value(it) }
        readField(decoder) { readOptionU64Value(it) }
        val key = readField(decoder) {
            readOptionRawPayload(it)?.let { keyPayload -> readVerifyingKeyBoxPayload(keyPayload, "$label.key") }
        }
        val status = readField(decoder) { checkedInt(it.readUInt(8), "$label.status") }
        require(decoder.remaining() == 0) { "Trailing bytes after $label" }
        return DecodedVerifierRecord(
            circuitId = circuitId,
            namespace = namespace,
            backendTag = backendTag,
            curve = curve,
            publicInputsSchemaHash = schemaHash,
            commitment = commitment,
            vkLen = vkLen,
            maxProofBytes = maxProofBytes,
            key = key,
            status = status,
        )
    }

    private fun readVerifyingKeyBoxPayload(payload: ByteArray, label: String): VerifyingKeyBoxValue {
        val decoder = NoritoDecoder(payload, REQUEST_FLAGS)
        val backend = readField(decoder) { readString(it) }
        val bytes = readField(decoder) { readBytesVec(it) }
        require(decoder.remaining() == 0) { "Trailing bytes after $label" }
        return VerifyingKeyBoxValue(backend, bytes)
    }

    private fun parseTransferPublicInputs(proofBytes: ByteArray, label: String): TransferPublicInputs {
        val columns = parseZk1InstanceColumns(proofBytes, label)
        require(columns.size == 9 && columns.all { it.size == 1 }) {
            "$label transfer proof must expose exactly 9 single-row instance columns"
        }
        val inputNullifiers = nonZeroSorted(
            listOf(columns[2][0], columns[3][0]),
            "$label input nullifiers",
        )
        val outputCommitments = nonZeroSorted(
            listOf(columns[4][0], columns[5][0]),
            "$label output commitments",
        )
        return TransferPublicInputs(
            rootBefore = columns[6][0],
            inputNullifiers = inputNullifiers,
            outputCommitments = outputCommitments,
        )
    }

    private fun parseZk1InstanceColumns(proofBytes: ByteArray, label: String): List<List<ByteArray>> {
        require(proofBytes.size >= ZK1_MAGIC.size && proofBytes.copyOfRange(0, ZK1_MAGIC.size).contentEquals(ZK1_MAGIC)) {
            "$label proof must use strict ZK1 envelope"
        }
        var offset = ZK1_MAGIC.size
        var sawProof = false
        var columns: List<List<ByteArray>>? = null
        while (offset < proofBytes.size) {
            require(offset + 8 <= proofBytes.size) { "$label malformed ZK1 TLV" }
            val tag = proofBytes.copyOfRange(offset, offset + 4)
            val length = readUInt32LittleEndian(proofBytes, offset + 4)
            require(length <= ZK1_MAX_TLV_BYTES) { "$label oversized ZK1 TLV" }
            val payloadStart = offset + 8
            val payloadEndLong = payloadStart.toLong() + length.toLong()
            require(payloadEndLong <= proofBytes.size.toLong()) { "$label truncated ZK1 TLV" }
            val payloadEnd = payloadEndLong.toInt()
            val payload = proofBytes.copyOfRange(payloadStart, payloadEnd)
            when (String(tag, StandardCharsets.US_ASCII)) {
                "PROF" -> {
                    require(!sawProof) { "$label duplicate PROF TLV" }
                    require(payload.isNotEmpty()) { "$label empty PROF TLV" }
                    sawProof = true
                }
                "I10P" -> {
                    require(columns == null) { "$label duplicate I10P TLV" }
                    columns = readZk1InstanceColumnsPayload(payload, label)
                }
                else -> throw IllegalArgumentException("$label unexpected ZK1 TLV")
            }
            offset = payloadEnd
        }
        require(sawProof) { "$label missing PROF TLV" }
        return columns ?: throw IllegalArgumentException("$label missing I10P TLV")
    }

    private fun readZk1InstanceColumnsPayload(payload: ByteArray, label: String): List<List<ByteArray>> {
        require(payload.size >= 8) { "$label malformed I10P TLV" }
        val columnCount = readUInt32LittleEndian(payload, 0)
        val rowCount = readUInt32LittleEndian(payload, 4)
        require(columnCount > 0 && rowCount > 0) { "$label empty I10P TLV" }
        require(columnCount <= ZK1_MAX_INSTANCE_COLUMNS && rowCount <= ZK1_MAX_INSTANCE_ROWS) {
            "$label oversized I10P TLV"
        }
        val expected = 8L + columnCount.toLong() * rowCount.toLong() * 32L
        require(expected <= Int.MAX_VALUE && payload.size == expected.toInt()) { "$label malformed I10P length" }
        val columns = MutableList(columnCount) { ArrayList<ByteArray>(rowCount) }
        var offset = 8
        repeat(rowCount) {
            for (column in columns) {
                column.add(payload.copyOfRange(offset, offset + 32))
                offset += 32
            }
        }
        return columns
    }

    private fun proofAttachmentPayload(
        envelope: OpenVerifyEnvelopeValue,
        verifierRecord: VerifierRecordValue,
    ): ByteArray {
        val encoder = NoritoEncoder(REQUEST_FLAGS)
        writeField(encoder) { writeString(it, ZK_BACKEND_HALO2_IPA) }
        writeField(encoder) { writeProofBox(it, envelope.archive) }
        writeField(encoder) { writeVerifierKeyId(it, verifierRecord.id) }
        writeField(encoder) { writeOptionFixed32(it, verifierRecord.commitment) }
        writeField(encoder) { writeOptionFixed32(it, Blake2b.digest256(envelope.archive)) }
        writeField(encoder) { writeOptionRaw(it, null) }
        return encoder.toByteArray()
    }

    private fun writeVerifiedFoldSteps(encoder: NoritoEncoder, hops: List<PreparedVerifiedFoldHop>) {
        encoder.writeUInt(hops.size.toLong(), 64)
        for (hop in hops) {
            writeField(encoder) { step ->
                writeField(step) { writeConstVec(it, hop.publicInputs.rootBefore) }
                writeField(step) { writeFixed32Vec(it, hop.publicInputs.inputNullifiers) }
                writeField(step) { writeFixed32Vec(it, hop.publicInputs.outputCommitments) }
                writeField(step) { writeConstVec(it, hop.rootAfter) }
                writeRawField(step, proofAttachmentPayload(hop.envelope, hop.verifierRecord))
                writeRawField(step, verifyingKeyBoxPayload(hop.verifierRecord.key.backend, hop.verifierRecord.key.bytes))
            }
        }
    }

    private fun writeVerifiedFoldVerifierRecords(
        encoder: NoritoEncoder,
        hops: List<PreparedVerifiedFoldHop>,
    ) {
        val unique = ArrayList<VerifierRecordValue>()
        for (hop in hops) {
            if (unique.none { it.id == hop.verifierRecord.id }) {
                unique.add(hop.verifierRecord)
            }
        }
        encoder.writeUInt(unique.size.toLong(), 64)
        for (record in unique) {
            writeField(encoder) { entry ->
                writeField(entry) { writeVerifierKeyId(it, record.id) }
                writeRawField(entry, record.recordPayload)
            }
        }
    }
}

private data class PrivacyBuildResult(val proof: ByteArray)

private data class OpenVerifyEnvelopeValue(
    val archive: ByteArray,
    val circuitId: String,
    val vkHash: ByteArray,
    val publicInputs: ByteArray,
    val proofBytes: ByteArray,
)

private data class TransferPublicInputs(
    val rootBefore: ByteArray,
    val inputNullifiers: List<ByteArray>,
    val outputCommitments: List<ByteArray>,
)

private data class VerifierKeyIdValue(
    val backend: String,
    val name: String,
)

private data class VerifyingKeyBoxValue(
    val backend: String,
    val bytes: ByteArray,
)

private data class DecodedVerifierRecord(
    val circuitId: String,
    val namespace: String,
    val backendTag: Long,
    val curve: String,
    val publicInputsSchemaHash: ByteArray,
    val commitment: ByteArray,
    val vkLen: Int,
    val maxProofBytes: Int,
    val key: VerifyingKeyBoxValue?,
    val status: Int,
)

private data class VerifierRecordValue(
    val id: VerifierKeyIdValue,
    val recordPayload: ByteArray,
    val commitment: ByteArray,
    val key: VerifyingKeyBoxValue,
)

private data class PreparedVerifiedFoldHop(
    val chainId: String,
    val asset: String,
    val rootAfter: ByteArray,
    val publicInputs: TransferPublicInputs,
    val envelope: OpenVerifyEnvelopeValue,
    val verifierRecord: VerifierRecordValue,
)

private fun verifyingKeyBoxPayload(bytes: ByteArray): ByteArray =
    verifyingKeyBoxPayload(KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND, bytes)

private fun verifyingKeyBoxPayload(backend: String, bytes: ByteArray): ByteArray {
    requireNonBlankUnpadded(backend, "verifierKeyBackend")
    require(bytes.isNotEmpty()) { "verifierKey must not be empty" }
    val encoder = NoritoEncoder(NoritoHeader.COMPACT_LEN)
    writeField(encoder) { writeString(it, backend) }
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

private fun validateLineageKeyArtifactsForInit(
    lineageVerifierKey: ByteArray,
    lineageProvingKeyArchive: ByteArray,
) {
    try {
        KagemushaRecursiveSpendProver.lineageKeyArtifactsForInit(
            LINEAGE_KEY_ARTIFACT_VALIDATION_OPENING_LEN,
            KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
            lineageVerifierKey,
            lineageProvingKeyArchive,
        )
    } catch (ex: IllegalArgumentException) {
        throw IllegalArgumentException("lineage key artifacts are invalid for recursive spend init", ex)
    }
}

private data class LineageKeyMaterial(
    val verifierKey: ByteArray?,
    val provingKeyArchive: ByteArray?,
)

private fun lineageKeyMaterialForInit(
    artifacts: KagemushaRecursiveSpendProver.LineageKeyArtifacts,
): LineageKeyMaterial {
    val checked = requireInitLineageKeyArtifacts(artifacts)
    return LineageKeyMaterial(
        verifierKey = checked.lineageVerifierKey(),
        provingKeyArchive = checked.lineageProvingKeyArchive(),
    )
}

private fun requireInitLineageKeyArtifacts(
    artifacts: KagemushaRecursiveSpendProver.LineageKeyArtifacts,
): KagemushaRecursiveSpendProver.LineageKeyArtifacts {
    val checked = KagemushaRecursiveSpendProver.validateLineageKeyArtifacts(artifacts)
    require(checked.isInitArtifact()) { "lineageKeyArtifacts must be init artifacts" }
    return checked
}

private fun validateLineageKeyArtifactsForAppend(
    lineageVerifierKey: ByteArray,
    lineageProvingKeyArchive: ByteArray,
) {
    try {
        KagemushaRecursiveSpendProver.lineageKeyArtifactsForAppend(
            LINEAGE_KEY_ARTIFACT_VALIDATION_OPENING_LEN,
            KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND,
            lineageVerifierKey,
            lineageProvingKeyArchive,
        )
    } catch (ex: IllegalArgumentException) {
        throw IllegalArgumentException("lineage key artifacts are invalid for lineage append output", ex)
    }
}

private fun lineageKeyMaterialForAppend(
    artifacts: KagemushaRecursiveSpendProver.LineageKeyArtifacts?,
): LineageKeyMaterial {
    if (artifacts == null) {
        return LineageKeyMaterial(verifierKey = null, provingKeyArchive = null)
    }
    val checked = requireAppendLineageKeyArtifacts(artifacts)
    return LineageKeyMaterial(
        verifierKey = checked.lineageVerifierKey(),
        provingKeyArchive = checked.lineageProvingKeyArchive(),
    )
}

private fun requireAppendLineageKeyArtifacts(
    artifacts: KagemushaRecursiveSpendProver.LineageKeyArtifacts,
): KagemushaRecursiveSpendProver.LineageKeyArtifacts {
    val checked = KagemushaRecursiveSpendProver.validateLineageKeyArtifacts(artifacts)
    require(checked.isAppendArtifact()) { "lineageKeyArtifacts must be append artifacts" }
    return checked
}

private fun readVerifiedFoldRecordBundleHopCount(payload: ByteArray, flags: Int, field: String): Int {
    val decoder = NoritoDecoder(payload, flags)
    val bundlePayload = readField(decoder) { it.readRemainingBytes() }
    readField(decoder) { it.readRemainingBytes() } // verifier records
    require(decoder.remaining() == 0) { "Trailing bytes after $field" }

    val bundle = NoritoDecoder(bundlePayload, flags)
    skipFields(bundle, 2) // chain_id, asset
    val hopCount = readField(bundle) { readVerifiedFoldStepCount(it, "$field.steps") }
    require(bundle.remaining() == 0) { "Trailing bytes after $field bundle" }
    require(hopCount > 0) { "$field must contain at least one fold step" }
    require(hopCount <= KagemushaRecursiveSpendProver.COMPACT_TOKEN_MAX_HOPS) {
        "$field fold step count must not exceed ${KagemushaRecursiveSpendProver.COMPACT_TOKEN_MAX_HOPS}"
    }
    return hopCount
}

private fun readVerifiedFoldStepCount(decoder: NoritoDecoder, field: String): Int {
    val count = checkedInt(decoder.readUInt(64), "$field count")
    repeat(count) { index ->
        val itemLength = checkedInt(decoder.readLength(compact(decoder)), "$field item length")
        val item = NoritoDecoder(decoder.readBytes(itemLength), decoder.flags, decoder.flagsHint)
        skipFields(item, 6)
        require(item.remaining() == 0) { "Trailing bytes after $field[$index]" }
    }
    require(decoder.remaining() == 0) { "Trailing bytes after $field" }
    return count
}

private fun requirePallasOpenEnvelopesArchive(
    archive: ByteArray,
    expectedEnvelopeCount: Int,
    field: String,
    maxBytes: Int,
) {
    require(archive.isNotEmpty()) { "$field must not be empty" }
    require(archive.size <= maxBytes) {
        "$field must not exceed $maxBytes bytes"
    }
    val decoded = try {
        NoritoHeader.decode(archive, OPEN_VERIFY_ENVELOPES_SCHEMA_HASH)
    } catch (ex: IllegalArgumentException) {
        throw IllegalArgumentException(
            "$field must be a valid Vec<iroha_zkp_halo2::OpenVerifyEnvelope> Norito archive",
            ex,
        )
    }
    require(decoded.header.compression == NoritoHeader.COMPRESSION_NONE) {
        "$field must not be compressed"
    }
    require(decoded.header.payloadLength > 0) { "$field must contain a non-empty Norito payload" }
    decoded.header.validateChecksum(decoded.payload)
    require(decoded.header.flags == NoritoHeader.COMPACT_LEN) {
        "$field must use compact Norito layout"
    }
    val decoder = NoritoDecoder(decoded.payload, decoded.header.flags)
    val count = checkedInt(decoder.readUInt(64), "$field envelope count")
    require(count == expectedEnvelopeCount) {
        "$field requires exactly $expectedEnvelopeCount envelope(s)"
    }
    repeat(count) { index ->
        val itemLength = checkedInt(decoder.readLength(compact(decoder)), "$field envelope length")
        val itemPayload = decoder.readBytes(itemLength)
        validatePallasOpenEnvelopePayload(itemPayload, decoded.header.flags, "$field[$index]")
    }
    require(decoder.remaining() == 0) { "Trailing bytes after $field archive" }
}

private fun requirePreviousProofOpenEnvelopesArchive(archive: ByteArray) {
    requirePallasOpenEnvelopesArchive(
        archive,
        expectedEnvelopeCount = KagemushaRecursiveSpendProver
            .RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_REQUIRED_COUNT_V1,
        field = "previousProofOpenEnvelopes",
        maxBytes = KagemushaRecursiveSpendProver.RECURSIVE_PREVIOUS_PROOF_OPEN_ENVELOPES_MAX_BYTES,
    )
}

private fun validatePallasOpenEnvelopePayload(payload: ByteArray, flags: Int, field: String) {
    val decoder = NoritoDecoder(payload, flags)
    val paramsN = readField(decoder) { readPallasIpaParams(it, "$field.params") }
    val publicN = readField(decoder) { readPallasPolyOpenPublic(it, "$field.public") }
    require(publicN == paramsN) { "$field public opening length must match params.n" }
    readField(decoder) { readPallasIpaProof(it, paramsN, "$field.proof") }
    val transcriptLabel = readField(decoder) { readString(it) }
    require(transcriptLabel.isNotEmpty()) { "$field transcript_label must be non-empty" }
    require(transcriptLabel.length <= KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES) {
        "$field transcript_label exceeds " +
            "$KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES bytes"
    }
    readField(decoder) { readRequiredMetadataOption(it, "$field.vk_commitment") }
    readField(decoder) { readRequiredMetadataOption(it, "$field.public_inputs_schema_hash") }
    readField(decoder) { readRequiredMetadataOption(it, "$field.domain_tag") }
    require(decoder.remaining() == 0) { "Trailing bytes after $field" }
}

private fun readPallasIpaParams(decoder: NoritoDecoder, field: String): Int {
    val version = readField(decoder) { checkedInt(it.readUInt(16), "$field.version") }
    require(version == 1) { "$field.version must be 1" }
    val curveId = readField(decoder) { checkedInt(it.readUInt(16), "$field.curve_id") }
    require(curveId == PALLAS_CURVE_ID) { "$field.curve_id must be Pallas" }
    val n = readField(decoder) { checkedInt(it.readUInt(32), "$field.n") }
    require(n >= 2 && n.countOneBits() == 1) { "$field.n must be a power of two >= 2" }
    require(n <= KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_N) {
        "$field.n exceeds max 2^$KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_K"
    }
    val gCount = readField(decoder) { readFixed32SequenceCount(it, "$field.g") }
    require(gCount == n) { "$field.g length must equal params.n" }
    val hCount = readField(decoder) { readFixed32SequenceCount(it, "$field.h") }
    require(hCount == n) { "$field.h length must equal params.n" }
    readField(decoder) { it.readFixed32("$field.u") }
    require(decoder.remaining() == 0) { "Trailing bytes after $field" }
    return n
}

private fun readPallasPolyOpenPublic(decoder: NoritoDecoder, field: String): Int {
    val version = readField(decoder) { checkedInt(it.readUInt(16), "$field.version") }
    require(version == 1) { "$field.version must be 1" }
    val curveId = readField(decoder) { checkedInt(it.readUInt(16), "$field.curve_id") }
    require(curveId == PALLAS_CURVE_ID) { "$field.curve_id must be Pallas" }
    val n = readField(decoder) { checkedInt(it.readUInt(32), "$field.n") }
    readField(decoder) { it.readFixed32("$field.z") }
    readField(decoder) { it.readFixed32("$field.t") }
    readField(decoder) { it.readFixed32("$field.p_g") }
    require(decoder.remaining() == 0) { "Trailing bytes after $field" }
    return n
}

private fun readPallasIpaProof(decoder: NoritoDecoder, n: Int, field: String) {
    val version = readField(decoder) { checkedInt(it.readUInt(16), "$field.version") }
    require(version == 1) { "$field.version must be 1" }
    val lCount = readField(decoder) { readFixed32SequenceCount(it, "$field.l") }
    val rCount = readField(decoder) { readFixed32SequenceCount(it, "$field.r") }
    require(lCount == rCount) { "$field L/R round count mismatch" }
    val expectedRounds = n.countTrailingZeroBits()
    require(lCount == expectedRounds) {
        "$field round count mismatch: expected $expectedRounds, found $lCount"
    }
    readField(decoder) { it.readFixed32("$field.a_final") }
    readField(decoder) { it.readFixed32("$field.b_final") }
    require(decoder.remaining() == 0) { "Trailing bytes after $field" }
}

private fun readFixed32SequenceCount(decoder: NoritoDecoder, field: String): Int {
    val count = checkedInt(decoder.readUInt(64), "$field count")
    repeat(count) { index ->
        val itemLength = checkedInt(decoder.readLength(compact(decoder)), "$field item length")
        val item = NoritoDecoder(decoder.readBytes(itemLength), decoder.flags, decoder.flagsHint)
        item.readFixed32("$field[$index]")
        require(item.remaining() == 0) { "Trailing bytes after $field[$index]" }
    }
    return count
}

private fun readRequiredMetadataOption(decoder: NoritoDecoder, field: String): ByteArray {
    val payload = readOptionRawPayload(decoder)
        ?: throw IllegalArgumentException("$field is required")
    val child = NoritoDecoder(payload, decoder.flags, decoder.flagsHint)
    val value = child.readFixed32(field)
    require(child.remaining() == 0) { "Trailing bytes after $field" }
    require(!isZero32(value)) { "$field must be non-zero" }
    return value
}

private val OPEN_VERIFY_ENVELOPES_SCHEMA_HASH = byteArrayOf(
    0xfe.toByte(),
    0x38,
    0x26,
    0x32,
    0x8f.toByte(),
    0x08,
    0x17,
    0x71,
    0x75,
    0x0f,
    0x24,
    0xfe.toByte(),
    0x11,
    0x02,
    0x60,
    0xca.toByte(),
)

private const val PALLAS_CURVE_ID = 1
private const val LINEAGE_KEY_ARTIFACT_VALIDATION_OPENING_LEN = 2
private const val KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_K = 24
private const val KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_N = 16_777_216
private const val KAGEMUSHA_RECURSIVE_PALLAS_OPEN_ENVELOPE_MAX_TRANSCRIPT_LABEL_BYTES = 128

private fun fixed32(value: ByteArray, field: String): ByteArray {
    require(value.size == 32) { "$field must be exactly 32 bytes" }
    return value.copyOf()
}

private fun isZero32(value: ByteArray): Boolean = value.all { it.toInt() == 0 }

private fun irohaHash(value: ByteArray): ByteArray {
    val digest = Blake2b.digest256(value)
    digest[digest.lastIndex] = (digest.last().toInt() or 0x01).toByte()
    return digest
}

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

private fun requireRedeemChangeBinding(
    publicAmount: String,
    currentAmount: String,
    hasChangeOutput: Boolean,
) {
    val comparison = compareCanonicalDecimal(publicAmount, currentAmount)
    if (hasChangeOutput) {
        require(comparison < 0) {
            "publicAmount must be less than current note amount when changeOutput is present"
        }
    } else {
        require(comparison >= 0) {
            "changeOutput is required when publicAmount is less than current note amount"
        }
        require(comparison == 0) {
            "publicAmount must not exceed current note amount"
        }
    }
}

private fun compareCanonicalDecimal(left: String, right: String): Int =
    when {
        left.length != right.length -> left.length.compareTo(right.length)
        else -> left.compareTo(right)
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

private fun fixed32Payload(value: ByteArray): ByteArray {
    val encoder = NoritoEncoder(NoritoHeader.COMPACT_LEN)
    writeConstVec(encoder, fixed32(value, "fixed32"))
    return encoder.toByteArray()
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

private fun writeOptionFixed32(encoder: NoritoEncoder, value: ByteArray?) {
    writeOptionRaw(encoder, value?.let(::fixed32Payload))
}

private fun writeSpendableNote(encoder: NoritoEncoder, value: SpendableNoteDescriptor) {
    writeField(encoder) { writeConstVec(it, value.noteCommitment) }
    writeField(encoder) { writeConstVec(it, value.spendNullifier) }
    writeField(encoder) { writeNumeric(it, value.amount) }
}

private fun writeChainId(encoder: NoritoEncoder, value: String) {
    requireNonBlankUnpadded(value, "chainId")
    writeField(encoder) { writeString(it, value) }
}

private fun writeAssetDefinitionId(encoder: NoritoEncoder, value: String) {
    val bytes = try {
        AssetDefinitionIdEncoder.parseAddressBytes(value)
    } catch (ex: IllegalArgumentException) {
        throw IllegalArgumentException("asset must be a canonical asset definition id", ex)
    }
    encoder.writeBytes(bytes)
}

private fun writeFixed32Vec(encoder: NoritoEncoder, values: List<ByteArray>) {
    require(values.isNotEmpty()) { "fixed32 vector must not be empty" }
    encoder.writeUInt(values.size.toLong(), 64)
    for (value in values) {
        writeField(encoder) { writeConstVec(it, fixed32(value, "fixed32 vector element")) }
    }
}

private fun writeProofBox(encoder: NoritoEncoder, proofBytes: ByteArray) {
    require(proofBytes.isNotEmpty()) { "proof bytes must not be empty" }
    writeField(encoder) { writeString(it, KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND) }
    writeField(encoder) { writeBytesVec(it, proofBytes) }
}

private fun writeVerifierKeyId(encoder: NoritoEncoder, id: VerifierKeyIdValue) {
    requirePortableId(id.backend, "verifierKeyId.backend")
    requirePortableId(id.name, "verifierKeyId.name")
    writeField(encoder) { writeString(it, id.backend) }
    writeField(encoder) { writeString(it, id.name) }
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

private data class VerifyingKeyIdParts(val backend: String, val name: String)

private fun readVerifyingKeyId(payload: ByteArray, flags: Int): VerifyingKeyIdParts {
    val decoder = NoritoDecoder(payload, flags)
    val backend = readField(decoder) { readString(it) }
    val name = readField(decoder) { readString(it) }
    require(decoder.remaining() == 0) { "Trailing bytes after verifier key id" }
    requirePortableId(backend, "verifierKeyId.backend")
    require(backend == KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND) {
        "bundle.proof_backend unsupported recursive proof backend"
    }
    requirePortableId(name, "verifierKeyId")
    return VerifyingKeyIdParts(backend, name)
}

private fun readVerifyingKeyIdName(payload: ByteArray, flags: Int): String {
    return readVerifyingKeyId(payload, flags).name
}

private fun readProofBoxBackend(payload: ByteArray, flags: Int): String {
    val decoder = NoritoDecoder(payload, flags)
    val backend = readField(decoder) { readString(it) }
    val proofBytes = readField(decoder) { readBytesVec(it) }
    require(decoder.remaining() == 0) { "Trailing bytes after proof" }
    requirePortableId(backend, "proof.backend")
    require(backend == KagemushaRecursiveSpendProver.RECURSIVE_AGGREGATION_PROOF_BACKEND) {
        "bundle.proof_backend unsupported recursive proof backend"
    }
    require(proofBytes.isNotEmpty()) { "bundle.proof_bytes empty recursive proof" }
    return backend
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

private fun readBytesVec(decoder: NoritoDecoder): ByteArray {
    val length = checkedInt(decoder.readUInt(64), "byte vector length")
    return decoder.readBytes(length)
}

private fun readOptionRawPayload(decoder: NoritoDecoder): ByteArray? {
    val tag = decoder.readByte()
    require(tag == 0 || tag == 1) { "option tag must be 0 or 1" }
    if (tag == 0) {
        return null
    }
    val length = checkedInt(decoder.readLength(compact(decoder)), "option payload length")
    return decoder.readBytes(length)
}

private fun readOptionString(decoder: NoritoDecoder): String? =
    readOptionRawPayload(decoder)?.let { payload ->
        val child = NoritoDecoder(payload, decoder.flags, decoder.flagsHint)
        val value = readString(child)
        require(child.remaining() == 0) { "Trailing bytes after option string" }
        value
    }

private fun readOptionU64Value(decoder: NoritoDecoder): Long? =
    readOptionRawPayload(decoder)?.let { payload ->
        val child = NoritoDecoder(payload, decoder.flags, decoder.flagsHint)
        val value = child.readUInt(64)
        require(child.remaining() == 0) { "Trailing bytes after option u64" }
        value
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

private fun parseVerifierKeyId(value: String, field: String): VerifierKeyIdValue {
    requirePortableId(value, field)
    val separator = value.indexOf(':')
    require(separator > 0 && separator < value.lastIndex) {
        "$field must use backend:name syntax"
    }
    val backend = value.substring(0, separator)
    val name = value.substring(separator + 1)
    requirePortableId(backend, "$field.backend")
    requirePortableId(name, "$field.name")
    return VerifierKeyIdValue(backend, name)
}

private fun verifyingKeyCommitment(backend: String, bytes: ByteArray): ByteArray {
    requireNonBlankUnpadded(backend, "verifierKeyBackend")
    require(bytes.isNotEmpty()) { "verifierKey must not be empty" }
    val backendBytes = backend.toByteArray(StandardCharsets.UTF_8)
    val digest = MessageDigest.getInstance("SHA-256")
    digest.update("iroha:zk:v1:vk".toByteArray(StandardCharsets.US_ASCII))
    digest.update(longBigEndian(backendBytes.size.toLong()))
    digest.update(backendBytes)
    digest.update(longBigEndian(bytes.size.toLong()))
    digest.update(bytes)
    return digest.digest()
}

private fun longBigEndian(value: Long): ByteArray {
    val out = ByteArray(8)
    for (index in out.indices) {
        out[index] = ((value ushr ((7 - index) * 8)) and 0xff).toByte()
    }
    return out
}

private fun readUInt32LittleEndian(bytes: ByteArray, offset: Int): Int {
    require(offset >= 0 && offset + 4 <= bytes.size) { "u32 field is truncated" }
    return (bytes[offset].toInt() and 0xff) or
        ((bytes[offset + 1].toInt() and 0xff) shl 8) or
        ((bytes[offset + 2].toInt() and 0xff) shl 16) or
        ((bytes[offset + 3].toInt() and 0xff) shl 24)
}

private fun nonZeroSorted(values: List<ByteArray>, field: String): List<ByteArray> {
    val filtered = values.map { fixed32(it, field) }.filterNot(::isZero32)
    require(filtered.isNotEmpty()) { "$field must contain at least one non-zero value" }
    val sorted = filtered.sortedWith { left, right -> compareUnsigned(left, right) }
    for (index in 1 until sorted.size) {
        require(!sorted[index - 1].contentEquals(sorted[index])) { "$field must not contain duplicates" }
    }
    return sorted
}

private const val MULTISIG_POLICY_VERSION = 1
