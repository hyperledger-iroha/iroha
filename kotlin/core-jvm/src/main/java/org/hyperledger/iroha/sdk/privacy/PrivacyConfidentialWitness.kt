package org.hyperledger.iroha.sdk.privacy

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.util.Collections
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/** Private input note carried by a production confidential-v2 proof witness. */
class PrivacyConfidentialNoteWitnessV1(
    amount: String,
    rho: ByteArray,
    diversifier: ByteArray,
    @JvmField val leafIndex: Long,
) {
    private val rhoBytes = privacyFixed32(rho, "rho")
    private val diversifierBytes = privacyFixed32(diversifier, "diversifier")

    @JvmField
    val amount: String = privacyCanonicalU128(amount, "amount")

    init {
        require(leafIndex >= 0) { "leafIndex must be non-negative" }
    }

    val rho: ByteArray get() = rhoBytes.copyOf()
    val diversifier: ByteArray get() = diversifierBytes.copyOf()

    fun rhoBytes(): ByteArray = rho
    fun diversifierBytes(): ByteArray = diversifier
}

/** Private output note carried by a production confidential transfer witness. */
class PrivacyConfidentialTransferOutputWitnessV1(
    amount: String,
    rho: ByteArray,
    ownerTag: ByteArray,
) {
    private val rhoBytes = privacyFixed32(rho, "rho")
    private val ownerTagBytes = privacyFixed32(ownerTag, "ownerTag")

    @JvmField
    val amount: String = privacyCanonicalU128(amount, "amount")

    val rho: ByteArray get() = rhoBytes.copyOf()
    val ownerTag: ByteArray get() = ownerTagBytes.copyOf()

    fun rhoBytes(): ByteArray = rho
    fun ownerTagBytes(): ByteArray = ownerTag
}

/** Private change note carried by a production confidential unshield witness. */
class PrivacyConfidentialUnshieldChangeWitnessV1(
    amount: String,
    rho: ByteArray,
) {
    private val rhoBytes = privacyFixed32(rho, "rho")

    @JvmField
    val amount: String = privacyCanonicalU128(amount, "amount")

    val rho: ByteArray get() = rhoBytes.copyOf()

    fun rhoBytes(): ByteArray = rho
}

/** Native `PrivacyConfidentialWitnessV1` payload accepted by `connect_norito_bridge`. */
class PrivacyConfidentialWitnessV1(
    @JvmField val networkId: NetworkId,
    assetDefinitionId: String,
    spendKey: ByteArray,
    treeCommitments: List<ByteArray>,
    inputs: List<PrivacyConfidentialNoteWitnessV1>,
    transferOutputs: List<PrivacyConfidentialTransferOutputWitnessV1>,
    unshieldChange: List<PrivacyConfidentialUnshieldChangeWitnessV1>,
    publicAmount: String,
    rootHint: ByteArray,
) {
    private val spendKeyBytes = privacyFixed32(spendKey, "spendKey")
    private val treeCommitmentBytes = treeCommitments.mapIndexed { index, value ->
        privacyFixed32(value, "treeCommitments[$index]")
    }
    private val inputValues = inputs.toList()
    private val transferOutputValues = transferOutputs.toList()
    private val unshieldChangeValues = unshieldChange.toList()
    private val rootHintBytes = privacyFixed32(rootHint, "rootHint")

    @JvmField
    val assetDefinitionId: String = privacyCanonicalText(assetDefinitionId, "assetDefinitionId")

    @JvmField
    val publicAmount: String = privacyCanonicalU128(publicAmount, "publicAmount")

    init {
        require(treeCommitmentBytes.isNotEmpty()) { "treeCommitments must not be empty" }
        require(treeCommitmentBytes.size <= PrivacyConfidentialWitnessCodecs.CONFIDENTIAL_TREE_CAPACITY_V2) {
            "treeCommitments must not exceed ${PrivacyConfidentialWitnessCodecs.CONFIDENTIAL_TREE_CAPACITY_V2}"
        }
        require(inputValues.size in 1..PrivacyConfidentialWitnessCodecs.CONFIDENTIAL_MAX_INPUTS_V2) {
            "inputs must contain one or two notes"
        }
        require(transferOutputValues.size <= PrivacyConfidentialWitnessCodecs.CONFIDENTIAL_MAX_TRANSFER_OUTPUTS_V2) {
            "transferOutputs must contain at most two notes"
        }
        require(unshieldChangeValues.size <= PrivacyConfidentialWitnessCodecs.CONFIDENTIAL_MAX_UNSHIELD_CHANGE_OUTPUTS_V3) {
            "unshieldChange must contain at most one note"
        }
        require(transferOutputValues.isEmpty() || unshieldChangeValues.isEmpty()) {
            "witness must not mix transferOutputs and unshieldChange"
        }
        require(transferOutputValues.isEmpty() || publicAmount == "0") {
            "transfer witness must not include publicAmount"
        }
        inputValues.forEachIndexed { index, input ->
            require(input.leafIndex < treeCommitmentBytes.size.toLong()) {
                "inputs[$index].leafIndex must reference treeCommitments"
            }
            for (previousIndex in 0 until index) {
                val previous = inputValues[previousIndex]
                require(previous.leafIndex != input.leafIndex) {
                    "inputs[$index].leafIndex duplicates inputs[$previousIndex]"
                }
                require(!previous.rho.contentEquals(input.rho)) {
                    "inputs[$index].rho duplicates inputs[$previousIndex]"
                }
            }
        }
    }

    val spendKey: ByteArray get() = spendKeyBytes.copyOf()
    val treeCommitments: List<ByteArray> get() = privacyCopyList(treeCommitmentBytes)
    val inputs: List<PrivacyConfidentialNoteWitnessV1> get() = Collections.unmodifiableList(inputValues.toList())
    val transferOutputs: List<PrivacyConfidentialTransferOutputWitnessV1>
        get() = Collections.unmodifiableList(transferOutputValues.toList())
    val unshieldChange: List<PrivacyConfidentialUnshieldChangeWitnessV1>
        get() = Collections.unmodifiableList(unshieldChangeValues.toList())
    val rootHint: ByteArray get() = rootHintBytes.copyOf()

    fun spendKeyBytes(): ByteArray = spendKey
    fun treeCommitmentBytes(): List<ByteArray> = treeCommitments
    fun rootHintBytes(): ByteArray = rootHint
}

/** Pure SDK encoders for the native confidential-v2 privacy proof witness/request ABI. */
object PrivacyConfidentialWitnessCodecs {
    const val SCHEMA_PRIVACY_CONFIDENTIAL_WITNESS_V1: String =
        "connect_norito_bridge::privacy_production::PrivacyConfidentialWitnessV1"
    const val CONFIDENTIAL_TREE_CAPACITY_V2: Int = 1 shl 16
    const val CONFIDENTIAL_MAX_INPUTS_V2: Int = 2
    const val CONFIDENTIAL_MAX_TRANSFER_OUTPUTS_V2: Int = 2
    const val CONFIDENTIAL_MAX_UNSHIELD_CHANGE_OUTPUTS_V3: Int = 1

    private const val NORITO_HEADER_BYTES: Int = 40
    private const val REQUEST_FLAGS: Int = NoritoHeader.COMPACT_LEN
    private const val WITNESS_HEADER_PADDING_BYTES: Int = 8
    private val TRANSFER_PUBLIC_INPUTS_SCHEMA: ByteArray =
        (
            "{\"schema\":\"confidential_transfer_v2\",\"public_inputs\":[\"input_commitment_0\"," +
                "\"input_commitment_1\",\"nullifier_0\",\"nullifier_1\",\"output_commitment_0\"," +
                "\"output_commitment_1\",\"root\",\"asset_tag\",\"network_tag\"]}"
            ).toByteArray(StandardCharsets.UTF_8)
    private val UNSHIELD_PUBLIC_INPUTS_SCHEMA: ByteArray =
        (
            "{\"schema\":\"confidential_unshield_v3\",\"public_inputs\":[\"input_commitment_0\"," +
                "\"input_commitment_1\",\"nullifier_0\",\"nullifier_1\",\"change_commitment_0\"," +
                "\"root\",\"public_amount\",\"asset_tag\",\"network_tag\"]}"
            ).toByteArray(StandardCharsets.UTF_8)

    @JvmStatic
    fun confidentialTransferPublicInputsSchemaV1(): ByteArray = TRANSFER_PUBLIC_INPUTS_SCHEMA.copyOf()

    @JvmStatic
    fun confidentialUnshieldPublicInputsSchemaV1(): ByteArray = UNSHIELD_PUBLIC_INPUTS_SCHEMA.copyOf()

    @JvmStatic
    fun encodeWitness(witness: PrivacyConfidentialWitnessV1): ByteArray =
        addNoritoHeaderPadding(
            NoritoCodec.encode(
                witness,
                SCHEMA_PRIVACY_CONFIDENTIAL_WITNESS_V1,
                PrivacyConfidentialWitnessAdapter,
                REQUEST_FLAGS,
            ),
            WITNESS_HEADER_PADDING_BYTES,
        )

    @JvmStatic
    fun encodeTransferWitness(witness: PrivacyConfidentialWitnessV1): ByteArray {
        validateTransferWitness(witness)
        return encodeWitness(witness)
    }

    @JvmStatic
    fun encodeUnshieldWitness(witness: PrivacyConfidentialWitnessV1): ByteArray {
        validateUnshieldWitness(witness)
        return encodeWitness(witness)
    }

    private fun validateTransferWitness(witness: PrivacyConfidentialWitnessV1) {
        require(witness.publicAmount == "0") {
            "confidential transfer witness must not include publicAmount"
        }
        require(witness.unshieldChange.isEmpty()) {
            "confidential transfer witness must not include unshieldChange"
        }
        require(witness.transferOutputs.size in 1..CONFIDENTIAL_MAX_TRANSFER_OUTPUTS_V2) {
            "confidential transfer witness must include one or two transferOutputs"
        }
    }

    private fun validateUnshieldWitness(witness: PrivacyConfidentialWitnessV1) {
        require(witness.transferOutputs.isEmpty()) {
            "confidential unshield witness must not include transferOutputs"
        }
        require(witness.unshieldChange.size <= CONFIDENTIAL_MAX_UNSHIELD_CHANGE_OUTPUTS_V3) {
            "confidential unshield witness supports at most one unshieldChange output"
        }
    }

    private fun addNoritoHeaderPadding(archive: ByteArray, paddingBytes: Int): ByteArray {
        require(archive.size >= NORITO_HEADER_BYTES) { "Norito archive is missing a header" }
        if (paddingBytes == 0) {
            return archive
        }
        val out = ByteArray(archive.size + paddingBytes)
        archive.copyInto(out, endIndex = NORITO_HEADER_BYTES)
        archive.copyInto(out, destinationOffset = NORITO_HEADER_BYTES + paddingBytes, startIndex = NORITO_HEADER_BYTES)
        return out
    }
}

private object PrivacyConfidentialWitnessAdapter : TypeAdapter<PrivacyConfidentialWitnessV1> {
    override fun encode(encoder: NoritoEncoder, value: PrivacyConfidentialWitnessV1) {
        writeField(encoder) { it.writeBytes(value.networkId.bytes()) }
        writeField(encoder) { writeString(it, value.assetDefinitionId) }
        writeField(encoder) { writeBytesVec(it, value.spendKey) }
        writeField(encoder) { writeSequence(it, value.treeCommitments) { item, bytes -> writeBytesVec(item, bytes) } }
        writeField(encoder) { writeSequence(it, value.inputs) { item, note -> NoteWitnessAdapter.encode(item, note) } }
        writeField(encoder) {
            writeSequence(it, value.transferOutputs) { item, output ->
                TransferOutputAdapter.encode(item, output)
            }
        }
        writeField(encoder) {
            writeSequence(it, value.unshieldChange) { item, change ->
                UnshieldChangeAdapter.encode(item, change)
            }
        }
        writeField(encoder) { writeU128(it, value.publicAmount) }
        writeField(encoder) { writeBytesVec(it, value.rootHint) }
    }

    override fun decode(decoder: NoritoDecoder): PrivacyConfidentialWitnessV1 =
        throw UnsupportedOperationException("privacy confidential witnesses are encode-only")
}

private object NoteWitnessAdapter : TypeAdapter<PrivacyConfidentialNoteWitnessV1> {
    override fun encode(encoder: NoritoEncoder, value: PrivacyConfidentialNoteWitnessV1) {
        writeField(encoder) { writeU128(it, value.amount) }
        writeField(encoder) { writeBytesVec(it, value.rho) }
        writeField(encoder) { writeBytesVec(it, value.diversifier) }
        writeField(encoder) { it.writeUInt(value.leafIndex, 64) }
    }

    override fun decode(decoder: NoritoDecoder): PrivacyConfidentialNoteWitnessV1 =
        throw UnsupportedOperationException("privacy confidential note witnesses are encode-only")
}

private object TransferOutputAdapter : TypeAdapter<PrivacyConfidentialTransferOutputWitnessV1> {
    override fun encode(encoder: NoritoEncoder, value: PrivacyConfidentialTransferOutputWitnessV1) {
        writeField(encoder) { writeU128(it, value.amount) }
        writeField(encoder) { writeBytesVec(it, value.rho) }
        writeField(encoder) { writeBytesVec(it, value.ownerTag) }
    }

    override fun decode(decoder: NoritoDecoder): PrivacyConfidentialTransferOutputWitnessV1 =
        throw UnsupportedOperationException("privacy confidential transfer outputs are encode-only")
}

private object UnshieldChangeAdapter : TypeAdapter<PrivacyConfidentialUnshieldChangeWitnessV1> {
    override fun encode(encoder: NoritoEncoder, value: PrivacyConfidentialUnshieldChangeWitnessV1) {
        writeField(encoder) { writeU128(it, value.amount) }
        writeField(encoder) { writeBytesVec(it, value.rho) }
    }

    override fun decode(decoder: NoritoDecoder): PrivacyConfidentialUnshieldChangeWitnessV1 =
        throw UnsupportedOperationException("privacy confidential unshield changes are encode-only")
}

private fun writeField(parent: NoritoEncoder, writePayload: (NoritoEncoder) -> Unit) {
    val child = parent.childEncoder()
    writePayload(child)
    val payload = child.toByteArray()
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

private fun <T> writeSequence(
    encoder: NoritoEncoder,
    values: List<T>,
    writeElement: (NoritoEncoder, T) -> Unit,
) {
    encoder.writeUInt(values.size.toLong(), 64)
    values.forEach { value ->
        writeField(encoder) { writeElement(it, value) }
    }
}

private fun writeU128(encoder: NoritoEncoder, value: String) {
    var integer = BigInteger(value)
    repeat(16) {
        encoder.writeByte(integer.and(PRIVACY_BYTE_MASK).toInt())
        integer = integer.shiftRight(8)
    }
}

private fun compact(encoder: NoritoEncoder): Boolean =
    (encoder.flags and NoritoHeader.COMPACT_LEN) != 0

private val PRIVACY_U128_MAX: BigInteger = BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE)
private val PRIVACY_BYTE_MASK: BigInteger = BigInteger.valueOf(0xffL)

private fun privacyCanonicalU128(value: String, name: String): String {
    val text = privacyCanonicalText(value, name)
    require(text.all { it in '0'..'9' }) { "$name must be an unsigned decimal integer" }
    require(text == "0" || !text.startsWith("0")) {
        "$name must be canonical decimal without leading zeroes"
    }
    val parsed = BigInteger(text)
    require(parsed.signum() >= 0 && parsed <= PRIVACY_U128_MAX) { "$name must fit in u128" }
    return text
}

private fun privacyCanonicalText(value: String, name: String): String {
    val trimmed = value.trim()
    require(trimmed.isNotEmpty()) { "$name must not be blank" }
    require(trimmed == value) { "$name must not contain surrounding whitespace" }
    require(trimmed.indexOf('\u0000') < 0) { "$name must not contain NUL" }
    return trimmed
}

private fun privacyFixed32(value: ByteArray, name: String): ByteArray {
    require(value.size == 32) { "$name must be 32 bytes" }
    return value.copyOf()
}

private fun privacyCopyList(values: List<ByteArray>): List<ByteArray> =
    Collections.unmodifiableList(values.map { it.copyOf() })
