package org.hyperledger.iroha.sdk.core.model.instructions

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.util.Base64
import java.util.Arrays
import org.bouncycastle.crypto.agreement.X25519Agreement
import org.bouncycastle.crypto.params.X25519PrivateKeyParameters
import org.bouncycastle.crypto.params.X25519PublicKeyParameters

private val U128_MAX: BigInteger = BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE)
private const val PROOF_ATTACHMENT_MAX_BYTES: Int = 64 * 1024 * 1024
private val LOW_ORDER_X25519_CHECK_PRIVATE_KEY = ByteArray(32) { 1 }

/** Shielded asset registration mode accepted by `zk::RegisterZkAsset`. */
enum class ZkAssetMode(@JvmField val bridgeCode: Int, @JvmField val wireName: String) {
    ZK_NATIVE(0, "ZkNative"),
    HYBRID(1, "Hybrid");

    companion object {
        @JvmStatic
        fun fromWireName(value: String?): ZkAssetMode {
            val normalized = requireText(value, "mode")
            return entries.firstOrNull { it.wireName == normalized }
                ?: throw IllegalArgumentException("mode must be ZkNative or Hybrid")
        }
    }
}

/**
 * X25519/XChaCha20-Poly1305 encrypted note payload carried by `zk::Shield`.
 *
 * Ciphertext is capped at [MAX_CIPHERTEXT_BYTES] because the encrypted payload
 * is a compact note descriptor, not an arbitrary attachment channel.
 */
class ConfidentialEncryptedPayload @JvmOverloads constructor(
    version: Int = VERSION_V1,
    ephemeralPublicKey: ByteArray,
    nonce: ByteArray,
    ciphertext: ByteArray,
) {
    private val _ephemeralPublicKey = fixedBytes(ephemeralPublicKey, 32, "ephemeralPublicKey")
    private val _nonce = fixedBytes(nonce, 24, "nonce")
    private val _ciphertext = copyNonEmpty(ciphertext, "ciphertext")

    @JvmField
    val version: Int = version

    init {
        require(version == VERSION_V1) { "version must be $VERSION_V1" }
        require(_ciphertext.size <= MAX_CIPHERTEXT_BYTES) {
            "ciphertext must not exceed $MAX_CIPHERTEXT_BYTES bytes"
        }
        require(!isLowOrderX25519PublicKey(_ephemeralPublicKey)) {
            "ephemeralPublicKey must not be low-order"
        }
    }

    val ephemeralPublicKey: ByteArray get() = _ephemeralPublicKey.copyOf()

    val nonce: ByteArray get() = _nonce.copyOf()

    val ciphertext: ByteArray get() = _ciphertext.copyOf()

    fun ephemeralPublicKeyBytes(): ByteArray = ephemeralPublicKey

    fun nonceBytes(): ByteArray = nonce

    fun ciphertextBytes(): ByteArray = ciphertext

    fun toWireBytes(): ByteArray {
        val out = ByteArrayOutputStream()
        out.write(version)
        out.write(_ephemeralPublicKey)
        out.write(_nonce)
        writeCompactVarint(_ciphertext.size, out)
        out.write(_ciphertext)
        return out.toByteArray()
    }

    fun wireBytes(): ByteArray = toWireBytes()

    override fun equals(other: Any?): Boolean =
        other is ConfidentialEncryptedPayload &&
            version == other.version &&
            _ephemeralPublicKey.contentEquals(other._ephemeralPublicKey) &&
            _nonce.contentEquals(other._nonce) &&
            _ciphertext.contentEquals(other._ciphertext)

    override fun hashCode(): Int {
        var result = version
        result = 31 * result + _ephemeralPublicKey.contentHashCode()
        result = 31 * result + _nonce.contentHashCode()
        result = 31 * result + _ciphertext.contentHashCode()
        return result
    }

    companion object {
        const val VERSION_V1: Int = 1
        const val MAX_CIPHERTEXT_BYTES: Int = 64 * 1024

        @JvmStatic
        fun fromWireBytes(bytes: ByteArray?): ConfidentialEncryptedPayload {
            require(bytes != null) { "bytes must be provided" }
            require(bytes.size >= 1 + 32 + 24) { "confidential encrypted payload is truncated" }
            val version = bytes[0].toInt() and 0xff
            val ephemeral = bytes.copyOfRange(1, 33)
            val nonce = bytes.copyOfRange(33, 57)
            val (ciphertextLength, lengthBytes) = readCompactVarint(bytes, 57)
            require(ciphertextLength <= MAX_CIPHERTEXT_BYTES) {
                "ciphertext must not exceed $MAX_CIPHERTEXT_BYTES bytes"
            }
            val ciphertextStart = 57 + lengthBytes
            val ciphertextEnd = ciphertextStart + ciphertextLength
            require(ciphertextEnd <= bytes.size) { "confidential encrypted payload ciphertext is truncated" }
            require(ciphertextEnd == bytes.size) { "confidential encrypted payload has trailing bytes" }
            return ConfidentialEncryptedPayload(
                version,
                ephemeral,
                nonce,
                bytes.copyOfRange(ciphertextStart, ciphertextEnd),
            )
        }

        @JvmStatic
        fun decodeWire(bytes: ByteArray?): ConfidentialEncryptedPayload = fromWireBytes(bytes)
    }
}

/** Reference to a registered verifier key used by a proof attachment. */
class ProofVerifierKeyRef(
    backend: String,
    name: String,
) {
    @JvmField
    val backend: String = requirePortableComponent(backend, "backend")

    @JvmField
    val name: String = requirePortableComponent(name, "name")

    fun wireId(): String = "$backend:$name"

    override fun equals(other: Any?): Boolean =
        other is ProofVerifierKeyRef && backend == other.backend && name == other.name

    override fun hashCode(): Int = 31 * backend.hashCode() + name.hashCode()

    companion object {
        @JvmStatic
        fun fromWireId(wireId: String?): ProofVerifierKeyRef {
            val normalized = requireText(wireId, "verifyingKeyId")
            val split = normalized.indexOf(':')
            require(split > 0 && split < normalized.length - 1) {
                "verifyingKeyId must use backend:name syntax"
            }
            return ProofVerifierKeyRef(
                normalized.substring(0, split),
                normalized.substring(split + 1),
            )
        }
    }
}

/** JSON-serializable proof attachment accepted by native zk transaction encoders. */
class ProofAttachment(
    backend: String,
    proofBytes: ByteArray,
    verifyingKeyRef: ProofVerifierKeyRef,
    verifyingKeyCommitment: ByteArray? = null,
    envelopeHash: ByteArray? = null,
) {
    @JvmField
    val backend: String = requirePortableComponent(backend, "backend")

    private val _proofBytes = copyNonEmpty(proofBytes, "proofBytes")

    @JvmField
    val verifyingKeyRef: ProofVerifierKeyRef = verifyingKeyRef

    private val _verifyingKeyCommitment = verifyingKeyCommitment?.let {
        fixedNonZeroBytes(it, 32, "verifyingKeyCommitment")
    }
    private val _envelopeHash = envelopeHash?.let { fixedBytes(it, 32, "envelopeHash") }

    init {
        require(_proofBytes.size <= PROOF_ATTACHMENT_MAX_BYTES) {
            "proofBytes must not exceed $PROOF_ATTACHMENT_MAX_BYTES bytes"
        }
        require(verifyingKeyRef.backend == this.backend) {
            "verifyingKeyRef.backend must match backend"
        }
    }

    val proofBytes: ByteArray get() = _proofBytes.copyOf()

    val verifyingKeyCommitment: ByteArray? get() = _verifyingKeyCommitment?.copyOf()

    val envelopeHash: ByteArray? get() = _envelopeHash?.copyOf()

    fun proofBytes(): ByteArray = proofBytes

    fun verifyingKeyCommitmentBytes(): ByteArray? = verifyingKeyCommitment

    fun envelopeHashBytes(): ByteArray? = envelopeHash

    fun toNativeJson(): String = buildString {
        append('{')
        append("\"backend\":")
        appendJsonString(backend)
        append(",\"proof_backend\":")
        appendJsonString(backend)
        append(",\"proof_b64\":")
        appendJsonString(Base64.getEncoder().encodeToString(_proofBytes))
        append(",\"vk_ref\":{\"backend\":")
        appendJsonString(verifyingKeyRef.backend)
        append(",\"name\":")
        appendJsonString(verifyingKeyRef.name)
        append('}')
        _verifyingKeyCommitment?.let {
            append(",\"vk_commitment_hex\":")
            appendJsonString(hexLower(it))
        }
        _envelopeHash?.let {
            append(",\"envelope_hash_hex\":")
            appendJsonString(hexLower(it))
        }
        append('}')
    }

    override fun equals(other: Any?): Boolean =
        other is ProofAttachment &&
            backend == other.backend &&
            _proofBytes.contentEquals(other._proofBytes) &&
            verifyingKeyRef == other.verifyingKeyRef &&
            nullableContentEquals(_verifyingKeyCommitment, other._verifyingKeyCommitment) &&
            nullableContentEquals(_envelopeHash, other._envelopeHash)

    override fun hashCode(): Int {
        var result = backend.hashCode()
        result = 31 * result + _proofBytes.contentHashCode()
        result = 31 * result + verifyingKeyRef.hashCode()
        result = 31 * result + (_verifyingKeyCommitment?.contentHashCode() ?: 0)
        result = 31 * result + (_envelopeHash?.contentHashCode() ?: 0)
        return result
    }
}

/** Typed representation of `zk::RegisterZkAsset`. */
class RegisterZkAssetInstruction private constructor(
    @JvmField val asset: String,
    @JvmField val mode: ZkAssetMode,
    @JvmField val allowShield: Boolean,
    @JvmField val allowUnshield: Boolean,
    @JvmField val transferVerifyingKey: String?,
    @JvmField val unshieldVerifyingKey: String?,
    @JvmField val shieldVerifyingKey: String?,
    override val arguments: Map<String, String>,
) : InstructionTemplate {
    override val kind: InstructionKind get() = InstructionKind.REGISTER

    class Builder internal constructor() {
        private var asset: String? = null
        private var mode: ZkAssetMode = ZkAssetMode.ZK_NATIVE
        private var allowShield: Boolean = true
        private var allowUnshield: Boolean = true
        private var transferVerifyingKey: String? = null
        private var unshieldVerifyingKey: String? = null
        private var shieldVerifyingKey: String? = null

        fun setAsset(asset: String?) = apply {
            this.asset = requireText(asset, "asset")
        }

        fun setMode(mode: ZkAssetMode?) = apply {
            this.mode = requireNotNull(mode) { "mode must be provided" }
        }

        fun setAllowShield(allowShield: Boolean) = apply {
            this.allowShield = allowShield
        }

        fun setAllowUnshield(allowUnshield: Boolean) = apply {
            this.allowUnshield = allowUnshield
        }

        fun setTransferVerifyingKey(verifyingKey: String?) = apply {
            this.transferVerifyingKey = optionalVerifyingKeyId(verifyingKey, "transferVerifyingKey")
        }

        fun setUnshieldVerifyingKey(verifyingKey: String?) = apply {
            this.unshieldVerifyingKey = optionalVerifyingKeyId(verifyingKey, "unshieldVerifyingKey")
        }

        fun setShieldVerifyingKey(verifyingKey: String?) = apply {
            this.shieldVerifyingKey = optionalVerifyingKeyId(verifyingKey, "shieldVerifyingKey")
        }

        fun build(): RegisterZkAssetInstruction {
            val selectedAsset = checkNotNull(asset) { "asset must be provided" }
            return RegisterZkAssetInstruction(
                selectedAsset,
                mode,
                allowShield,
                allowUnshield,
                transferVerifyingKey,
                unshieldVerifyingKey,
                shieldVerifyingKey,
                linkedMapOf(
                    "action" to "RegisterZkAsset",
                    "asset" to selectedAsset,
                    "mode" to mode.wireName,
                    "allow_shield" to allowShield.toString(),
                    "allow_unshield" to allowUnshield.toString(),
                    "vk_transfer" to (transferVerifyingKey ?: ""),
                    "vk_unshield" to (unshieldVerifyingKey ?: ""),
                    "vk_shield" to (shieldVerifyingKey ?: ""),
                ),
            )
        }
    }

    companion object {
        @JvmStatic
        fun builder(): Builder = Builder()

        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): RegisterZkAssetInstruction {
            val builder = builder()
                .setAsset(requireArgument(arguments, "asset"))
                .setMode(ZkAssetMode.fromWireName(requireArgument(arguments, "mode")))
                .setAllowShield(parseBoolean(requireArgument(arguments, "allow_shield"), "allow_shield"))
                .setAllowUnshield(parseBoolean(requireArgument(arguments, "allow_unshield"), "allow_unshield"))
            optionalArgument(arguments, "vk_transfer")?.let { builder.setTransferVerifyingKey(it) }
            optionalArgument(arguments, "vk_unshield")?.let { builder.setUnshieldVerifyingKey(it) }
            optionalArgument(arguments, "vk_shield")?.let { builder.setShieldVerifyingKey(it) }
            return builder.build()
        }

        private fun requireArgument(arguments: Map<String, String>, key: String): String {
            val value = arguments[key]
            require(!value.isNullOrBlank()) { "Instruction argument '$key' is required" }
            return value
        }

        private fun optionalArgument(arguments: Map<String, String>, key: String): String? =
            arguments[key]?.takeIf { it.isNotBlank() }

        private fun parseBoolean(value: String, name: String): Boolean =
            when (value) {
                "true" -> true
                "false" -> false
                else -> throw IllegalArgumentException("$name must be 'true' or 'false'")
            }
    }
}

/** Typed representation of `zk::Shield`. */
class ShieldInstruction private constructor(
    @JvmField val asset: String,
    @JvmField val from: String,
    @JvmField val amount: String,
    noteCommitment: ByteArray,
    @JvmField val encryptedPayload: ConfidentialEncryptedPayload,
    override val arguments: Map<String, String>,
) : InstructionTemplate {
    private val _noteCommitment = noteCommitment.copyOf()

    override val kind: InstructionKind get() = InstructionKind.CUSTOM

    val noteCommitment: ByteArray get() = _noteCommitment.copyOf()

    fun noteCommitmentBytes(): ByteArray = noteCommitment

    class Builder internal constructor() {
        private var asset: String? = null
        private var from: String? = null
        private var amount: String? = null
        private var noteCommitment: ByteArray? = null
        private var encryptedPayload: ConfidentialEncryptedPayload? = null

        fun setAsset(asset: String?) = apply {
            this.asset = requireText(asset, "asset")
        }

        fun setFrom(from: String?) = apply {
            this.from = requireText(from, "from")
        }

        fun setAmount(amount: String?) = apply {
            this.amount = canonicalU128(amount, "amount")
        }

        fun setAmount(amount: Number?) = setAmount(amount?.toString())

        fun setNoteCommitment(noteCommitment: ByteArray?) = apply {
            this.noteCommitment = fixedNonZeroBytes(noteCommitment, 32, "noteCommitment")
        }

        fun setEncryptedPayload(encryptedPayload: ConfidentialEncryptedPayload?) = apply {
            this.encryptedPayload = requireNotNull(encryptedPayload) {
                "encryptedPayload must be provided"
            }
        }

        fun build(): ShieldInstruction {
            val selectedAsset = checkNotNull(asset) { "asset must be provided" }
            val selectedFrom = checkNotNull(from) { "from must be provided" }
            val selectedAmount = checkNotNull(amount) { "amount must be provided" }
            val selectedCommitment = checkNotNull(noteCommitment) { "noteCommitment must be provided" }
            val selectedPayload = checkNotNull(encryptedPayload) { "encryptedPayload must be provided" }
            return ShieldInstruction(
                selectedAsset,
                selectedFrom,
                selectedAmount,
                selectedCommitment,
                selectedPayload,
                linkedMapOf(
                    "action" to "Shield",
                    "asset" to selectedAsset,
                    "from" to selectedFrom,
                    "amount" to selectedAmount,
                    "note_commitment" to hexLower(selectedCommitment),
                    "payload_ephemeral" to hexLower(selectedPayload.ephemeralPublicKey),
                    "payload_nonce" to hexLower(selectedPayload.nonce),
                    "payload_ciphertext" to Base64.getEncoder().encodeToString(selectedPayload.ciphertext),
                ),
            )
        }
    }

    companion object {
        @JvmStatic
        fun builder(): Builder = Builder()

        /**
         * Intentionally unsupported. `zk::Shield` carries a 32-byte note commitment and a binary
         * X25519/XChaCha20-Poly1305 encrypted payload that cannot be reconstructed from a generic
         * string argument map. Build instances through [builder] instead.
         */
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): ShieldInstruction =
            throw UnsupportedOperationException(
                "ShieldInstruction cannot be built from an argument map: its note commitment and " +
                    "encrypted payload are binary fields. Use ShieldInstruction.builder().",
            )
    }
}

/** Typed representation of `zk::Unshield`. */
class UnshieldInstruction private constructor(
    @JvmField val asset: String,
    @JvmField val to: String,
    @JvmField val publicAmount: String,
    inputs: List<ByteArray>,
    outputs: List<ByteArray>,
    @JvmField val proof: ProofAttachment,
    rootHint: ByteArray?,
    override val arguments: Map<String, String>,
) : InstructionTemplate {
    private val _inputs = inputs.map { it.copyOf() }
    private val _outputs = outputs.map { it.copyOf() }
    private val _rootHint = rootHint?.copyOf()

    override val kind: InstructionKind get() = InstructionKind.CUSTOM

    val inputs: List<ByteArray> get() = _inputs.map { it.copyOf() }

    val outputs: List<ByteArray> get() = _outputs.map { it.copyOf() }

    val rootHint: ByteArray? get() = _rootHint?.copyOf()

    fun inputNullifiers(): List<ByteArray> = inputs

    fun outputCommitments(): List<ByteArray> = outputs

    fun rootHintBytes(): ByteArray? = rootHint

    class Builder internal constructor() {
        private var asset: String? = null
        private var to: String? = null
        private var publicAmount: String? = null
        private val inputs = mutableListOf<ByteArray>()
        private val outputs = mutableListOf<ByteArray>()
        private var proof: ProofAttachment? = null
        private var rootHint: ByteArray? = null

        fun setAsset(asset: String?) = apply {
            this.asset = requireText(asset, "asset")
        }

        fun setTo(to: String?) = apply {
            this.to = requireText(to, "to")
        }

        fun setPublicAmount(publicAmount: String?) = apply {
            this.publicAmount = canonicalU128(publicAmount, "publicAmount")
        }

        fun setPublicAmount(publicAmount: Number?) = setPublicAmount(publicAmount?.toString())

        fun setInputs(inputs: List<ByteArray>?) = apply {
            this.inputs.clear()
            require(inputs != null && inputs.isNotEmpty()) { "inputs must contain at least one nullifier" }
            inputs.forEachIndexed { index, input ->
                this.inputs.add(fixedNonZeroBytes(input, 32, "inputs[$index]"))
            }
        }

        fun addInput(input: ByteArray?) = apply {
            inputs.add(fixedNonZeroBytes(input, 32, "inputs[${inputs.size}]"))
        }

        fun setOutputs(outputs: List<ByteArray>?) = apply {
            this.outputs.clear()
            outputs?.forEachIndexed { index, output ->
                this.outputs.add(fixedNonZeroBytes(output, 32, "outputs[$index]"))
            }
        }

        fun addOutput(output: ByteArray?) = apply {
            outputs.add(fixedNonZeroBytes(output, 32, "outputs[${outputs.size}]"))
        }

        fun setProof(proof: ProofAttachment?) = apply {
            this.proof = requireNotNull(proof) { "proof must be provided" }
        }

        fun setRootHint(rootHint: ByteArray?) = apply {
            this.rootHint = rootHint?.let { fixedBytes(it, 32, "rootHint") }
        }

        fun build(): UnshieldInstruction {
            val selectedAsset = checkNotNull(asset) { "asset must be provided" }
            val selectedTo = checkNotNull(to) { "to must be provided" }
            val selectedAmount = checkNotNull(publicAmount) { "publicAmount must be provided" }
            check(inputs.isNotEmpty()) { "inputs must contain at least one nullifier" }
            val selectedProof = checkNotNull(proof) { "proof must be provided" }
            return UnshieldInstruction(
                selectedAsset,
                selectedTo,
                selectedAmount,
                inputs,
                outputs,
                selectedProof,
                rootHint,
                linkedMapOf(
                    "action" to "Unshield",
                    "asset" to selectedAsset,
                    "to" to selectedTo,
                    "public_amount" to selectedAmount,
                    "inputs" to inputs.joinToString(",") { hexLower(it) },
                    "outputs" to outputs.joinToString(",") { hexLower(it) },
                    "proof" to selectedProof.toNativeJson(),
                    "root_hint" to (rootHint?.let { hexLower(it) } ?: ""),
                ),
            )
        }
    }

    companion object {
        @JvmStatic
        fun builder(): Builder = Builder()

        /**
         * Intentionally unsupported. `zk::Unshield` carries binary input nullifiers, output
         * commitments and a proof attachment that cannot be reconstructed from a generic string
         * argument map. Build instances through [builder] instead.
         */
        @JvmStatic
        fun fromArguments(arguments: Map<String, String>): UnshieldInstruction =
            throw UnsupportedOperationException(
                "UnshieldInstruction cannot be built from an argument map: its nullifiers, " +
                    "commitments and proof attachment are binary fields. Use UnshieldInstruction.builder().",
            )
    }
}

internal fun requireText(value: String?, name: String): String {
    require(value != null) { "$name must be provided" }
    val trimmed = value.trim()
    require(trimmed.isNotEmpty()) { "$name must not be blank" }
    require(trimmed == value) { "$name must not contain surrounding whitespace" }
    require(trimmed.indexOf('\u0000') < 0) { "$name must not contain NUL" }
    return trimmed
}

private fun requirePortableComponent(value: String?, name: String): String {
    val text = requireText(value, name)
    require(text.length <= 256) { "$name must not exceed 256 characters" }
    for (ch in text) {
        require(ch.code in 0x21..0x7e && ch != ':') {
            "$name must use portable ASCII without ':'"
        }
    }
    return text
}

private fun optionalVerifyingKeyId(value: String?, name: String): String? {
    if (value == null) return null
    val text = requireText(value, name)
    ProofVerifierKeyRef.fromWireId(text)
    return text
}

private fun canonicalU128(value: String?, name: String): String {
    val text = requireText(value, name)
    require(text.all { it in '0'..'9' }) { "$name must be an unsigned decimal integer" }
    require(text == "0" || !text.startsWith("0")) { "$name must be canonical decimal without leading zeroes" }
    val parsed = BigInteger(text)
    require(parsed in BigInteger.ZERO..U128_MAX) { "$name must fit in u128" }
    return text
}

private fun fixedBytes(value: ByteArray?, expected: Int, name: String): ByteArray {
    require(value != null) { "$name must be provided" }
    require(value.size == expected) { "$name must be exactly $expected bytes" }
    return value.copyOf()
}

private fun fixedNonZeroBytes(value: ByteArray?, expected: Int, name: String): ByteArray {
    val bytes = fixedBytes(value, expected, name)
    require(!isAllZero(bytes)) { "$name must not be all zero" }
    return bytes
}

private fun copyNonEmpty(value: ByteArray?, name: String): ByteArray {
    require(value != null) { "$name must be provided" }
    require(value.isNotEmpty()) { "$name must not be empty" }
    return value.copyOf()
}

private fun isAllZero(bytes: ByteArray): Boolean = bytes.all { it.toInt() == 0 }

private fun isLowOrderX25519PublicKey(publicKey: ByteArray): Boolean {
    val peer = X25519PublicKeyParameters(fixedBytes(publicKey, 32, "ephemeralPublicKey"), 0)
    val probe = X25519PrivateKeyParameters(LOW_ORDER_X25519_CHECK_PRIVATE_KEY, 0)
    val agreement = X25519Agreement()
    val shared = ByteArray(32)
    return try {
        agreement.init(probe)
        agreement.calculateAgreement(peer, shared, 0)
        isAllZero(shared)
    } catch (_: IllegalStateException) {
        true
    } finally {
        Arrays.fill(shared, 0.toByte())
    }
}

internal fun flattenFixed32(values: List<ByteArray>): ByteArray {
    val out = ByteArray(values.size * 32)
    values.forEachIndexed { index, value ->
        require(value.size == 32) { "value[$index] must be exactly 32 bytes" }
        System.arraycopy(value, 0, out, index * 32, 32)
    }
    return out
}

internal fun optionalBytes(value: ByteArray?): ByteArray = value?.copyOf() ?: ByteArray(0)

private fun writeCompactVarint(value: Int, out: ByteArrayOutputStream) {
    require(value >= 0) { "compact varint value must be non-negative" }
    var remaining = value
    while (true) {
        var byte = remaining and 0x7f
        remaining = remaining ushr 7
        if (remaining != 0) byte = byte or 0x80
        out.write(byte)
        if (remaining == 0) return
    }
}

private fun readCompactVarint(bytes: ByteArray, offset: Int): Pair<Int, Int> {
    var value = 0
    var shift = 0
    var cursor = offset
    while (cursor < bytes.size && shift < 28) {
        val byte = bytes[cursor].toInt() and 0xff
        value = value or ((byte and 0x7f) shl shift)
        cursor += 1
        if (byte and 0x80 == 0) {
            val encodedBytes = cursor - offset
            require(encodedBytes == 1 || value >= (1 shl (7 * (encodedBytes - 1)))) {
                "non-canonical confidential encrypted payload length"
            }
            return value to encodedBytes
        }
        shift += 7
    }
    throw IllegalArgumentException("invalid confidential encrypted payload length")
}

private fun StringBuilder.appendJsonString(value: String) {
    append('"')
    for (ch in value) {
        when (ch) {
            '"' -> append("\\\"")
            '\\' -> append("\\\\")
            '\b' -> append("\\b")
            '\u000C' -> append("\\f")
            '\n' -> append("\\n")
            '\r' -> append("\\r")
            '\t' -> append("\\t")
            else -> {
                if (ch < ' ') {
                    append("\\u00")
                    append(HEX[(ch.code ushr 4) and 0x0F])
                    append(HEX[ch.code and 0x0F])
                } else {
                    append(ch)
                }
            }
        }
    }
    append('"')
}

private fun hexLower(bytes: ByteArray): String = buildString(bytes.size * 2) {
    for (byte in bytes) {
        val value = byte.toInt() and 0xff
        append(HEX[value ushr 4])
        append(HEX[value and 0x0f])
    }
}

private fun nullableContentEquals(left: ByteArray?, right: ByteArray?): Boolean =
    when {
        left === null && right === null -> true
        left === null || right === null -> false
        else -> left.contentEquals(right)
    }

private val HEX = charArrayOf(
    '0', '1', '2', '3', '4', '5', '6', '7',
    '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
)
