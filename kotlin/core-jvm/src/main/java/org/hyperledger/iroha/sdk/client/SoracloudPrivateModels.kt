package org.hyperledger.iroha.sdk.client

import java.math.BigDecimal
import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.text.Normalizer
import java.util.Arrays
import java.util.Base64
import java.util.Collections
import java.util.IdentityHashMap
import java.util.LinkedHashMap
import java.util.RandomAccess
import org.bouncycastle.crypto.agreement.X25519Agreement
import org.bouncycastle.crypto.params.X25519PrivateKeyParameters
import org.bouncycastle.crypto.params.X25519PublicKeyParameters
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.AccountAddressException
import org.hyperledger.iroha.sdk.address.decodePublicKeyLiteral
import org.hyperledger.iroha.sdk.address.encodePublicKeyMultihash
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.core.util.HashLiteral
import org.hyperledger.iroha.sdk.crypto.IrohaHash

private const val SORACLOUD_U32_MAX = 4_294_967_295L
private val SORACLOUD_U64_MAX = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
private const val SORACLOUD_NAME_MAX_UTF8_BYTES = 255
private const val SORACLOUD_UPLOADED_MODEL_IDENTIFIER_MAX_BYTES_V1 = 128
private const val SORACLOUD_UPLOADED_MODEL_SERVICE_VERSION_MAX_BYTES_V1 = 256
private const val SORACLOUD_PRIVATE_RUNTIME_VERSION_V1 = "soracloud.quantized-cpu.v1"
private const val SORACLOUD_X25519_HKDF_SHA256 = "X25519HkdfSha256"
private const val SORACLOUD_AES_256_GCM = "Aes256Gcm"
private const val SORACLOUD_PRIVATE_RECEIPT_CURSOR_LENGTH_V1 = 114
private const val SORACLOUD_ZERO_PREHASH_SENTINEL =
    "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
internal const val SORACLOUD_PRIVATE_MODEL_ENCRYPTED_ARTIFACT_MAX_BYTES_V1 =
    72L * 1024L * 1024L
private val SORACLOUD_LOW_ORDER_X25519_PROBE_PRIVATE_KEY = ByteArray(32) { 1 }

/** Immutable, structurally comparable snapshot of a caller-provided list. */
class SoracloudImmutableList<E> private constructor(
    private val values: ArrayList<E>,
) : AbstractList<E>(), RandomAccess {
    override val size: Int
        get() = values.size

    override fun get(index: Int): E = values[index]

    companion object {
        /** Copy the supplied values into an immutable list snapshot. */
        @JvmStatic
        fun <E> copyOf(values: Collection<E>): SoracloudImmutableList<E> =
            SoracloudImmutableList(ArrayList(values))
    }
}

/** Recursively immutable snapshot of one JSON object. */
class SoracloudImmutableJsonObject private constructor(
    private val snapshot: Map<String, Any?>,
) : AbstractMap<String, Any?>() {
    override val entries: Set<Map.Entry<String, Any?>>
        get() = snapshot.entries

    companion object {
        private val STATUS_FIELDS = setOf("schema_version", "bundle", "artifact")
        private const val MAX_JSON_DEPTH = 128

        internal fun exactUploadedModelStatus(
            value: Map<String, Any?>,
        ): SoracloudImmutableJsonObject {
            require(value.keys == STATUS_FIELDS) {
                "status must contain exactly schema_version, bundle, and artifact"
            }
            require(isSchemaVersionOne(value["schema_version"])) {
                "status.schema_version must equal 1"
            }
            require(value["bundle"] is Map<*, *>) {
                "status.bundle must be a JSON object"
            }
            require(value["artifact"] == null || value["artifact"] is Map<*, *>) {
                "status.artifact must be null or a JSON object"
            }
            return snapshotObject(value, "status", IdentityHashMap(), 0)
        }

        private fun snapshotObject(
            value: Map<*, *>,
            path: String,
            active: IdentityHashMap<Any, Boolean>,
            depth: Int,
        ): SoracloudImmutableJsonObject {
            require(depth <= MAX_JSON_DEPTH) { "$path exceeds maximum JSON nesting depth" }
            require(active.put(value, true) == null) { "$path must not contain a reference cycle" }
            try {
                val copy = LinkedHashMap<String, Any?>(value.size)
                for ((rawKey, rawValue) in value) {
                    require(rawKey is String) { "$path object keys must be strings" }
                    copy[rawKey] = snapshotValue(rawValue, "$path.$rawKey", active, depth + 1)
                }
                return SoracloudImmutableJsonObject(Collections.unmodifiableMap(copy))
            } finally {
                active.remove(value)
            }
        }

        private fun snapshotList(
            value: List<*>,
            path: String,
            active: IdentityHashMap<Any, Boolean>,
            depth: Int,
        ): SoracloudImmutableList<Any?> {
            require(depth <= MAX_JSON_DEPTH) { "$path exceeds maximum JSON nesting depth" }
            require(active.put(value, true) == null) { "$path must not contain a reference cycle" }
            try {
                val copy = ArrayList<Any?>(value.size)
                value.forEachIndexed { index, item ->
                    copy.add(snapshotValue(item, "$path[$index]", active, depth + 1))
                }
                return SoracloudImmutableList.copyOf(copy)
            } finally {
                active.remove(value)
            }
        }

        private fun snapshotValue(
            value: Any?,
            path: String,
            active: IdentityHashMap<Any, Boolean>,
            depth: Int,
        ): Any? = when (value) {
            null, is String, is Boolean, is BigInteger, is BigDecimal,
            is Byte, is Short, is Int, is Long -> value
            is Float -> {
                require(value.isFinite()) { "$path must be a finite JSON number" }
                value
            }
            is Double -> {
                require(value.isFinite()) { "$path must be a finite JSON number" }
                value
            }
            is Map<*, *> -> snapshotObject(value, path, active, depth)
            is List<*> -> snapshotList(value, path, active, depth)
            else -> throw IllegalArgumentException("$path must contain only JSON values")
        }

        private fun isSchemaVersionOne(value: Any?): Boolean = when (value) {
            is BigInteger -> value == BigInteger.ONE
            is Byte, is Short, is Int, is Long -> (value as Number).toLong() == 1L
            else -> false
        }
    }
}

/** SoraFS-backed encrypted artifact reference used by private uploaded-model execution. */
data class SoracloudPrivateModelArtifactRef(
    @JvmField val schemaVersion: Long,
    @JvmField val sorafsManifestDigest: SoracloudImmutableList<Int>,
    @JvmField val sorafsRootCid: SoracloudImmutableList<Int>,
    @JvmField val artifactHash: String,
    @JvmField val ciphertextBytes: Long,
    @JvmField val artifactRole: String,
) {
    /** Build an artifact reference while taking immutable snapshots of its byte lists. */
    constructor(
        schemaVersion: Long,
        sorafsManifestDigest: List<Int>,
        sorafsRootCid: List<Int>,
        artifactHash: String,
        ciphertextBytes: Long,
        artifactRole: String,
    ) : this(
        schemaVersion,
        SoracloudImmutableList.copyOf(sorafsManifestDigest),
        SoracloudImmutableList.copyOf(sorafsRootCid),
        artifactHash,
        ciphertextBytes,
        artifactRole,
    )

    init {
        require(schemaVersion == 1L) { "schemaVersion must equal 1" }
        requireManifestDigest(sorafsManifestDigest, "sorafsManifestDigest")
        require(sorafsRootCid.size == 36) { "sorafsRootCid must contain exactly 36 bytes" }
        require(sorafsRootCid.all { it in 0..255 }) {
            "sorafsRootCid elements must be unsigned bytes"
        }
        require(sorafsRootCid.subList(0, 4) == listOf(1, 0x71, 0x1f, 32)) {
            "sorafsRootCid must use canonical CIDv1/dag-cbor/BLAKE3-256 framing"
        }
        require(sorafsRootCid.subList(4, 36).any { it != 0 }) {
            "sorafsRootCid digest must be nonzero"
        }
        requireSoracloudHashLiteral(artifactHash, "artifactHash")
        require(ciphertextBytes in 1L..SORACLOUD_PRIVATE_MODEL_ENCRYPTED_ARTIFACT_MAX_BYTES_V1) {
            "ciphertextBytes must be within " +
                "1..=$SORACLOUD_PRIVATE_MODEL_ENCRYPTED_ARTIFACT_MAX_BYTES_V1"
        }
        requireCanonicalSoracloudString(artifactRole, "artifactRole")
        require(artifactRole == "input" || artifactRole == "output") {
            "artifactRole must equal input or output"
        }
    }
}

/** Exact active validator that attested a deterministic private execution receipt. */
data class SoracloudRuntimeDeterministicValidatorHost(
    @JvmField val laneId: Long,
    @JvmField val validatorAccountId: String,
    @JvmField val peerId: String,
) {
    init {
        require(laneId in 0L..SORACLOUD_U32_MAX) {
            "laneId must be within 0..=$SORACLOUD_U32_MAX"
        }
        requireSoracloudValidatorIdentity(validatorAccountId, peerId)
    }
}

/** Public key metadata to which a private execution output is encrypted. */
data class SoracloudUploadedModelEncryptionRecipient(
    @JvmField val schemaVersion: Long,
    @JvmField val keyId: String,
    @JvmField val keyVersion: Long,
    @JvmField val kem: String,
    @JvmField val aead: String,
    @JvmField val publicKeyBytesBase64: String,
    @JvmField val publicKeyFingerprint: String,
) {
    private val decodedPublicKey =
        decodeCanonicalSoracloudX25519PublicKey(publicKeyBytesBase64, "publicKeyBytesBase64")

    init {
        require(schemaVersion == 1L) { "schemaVersion must equal 1" }
        requireCanonicalSoracloudString(keyId, "keyId")
        require(keyVersion in 1L..SORACLOUD_U32_MAX) {
            "keyVersion must be within 1..=$SORACLOUD_U32_MAX"
        }
        require(kem == SORACLOUD_X25519_HKDF_SHA256) {
            "kem must equal $SORACLOUD_X25519_HKDF_SHA256"
        }
        require(aead == SORACLOUD_AES_256_GCM) {
            "aead must equal $SORACLOUD_AES_256_GCM"
        }
        val fingerprint = requireSoracloudHashLiteral(
            publicKeyFingerprint,
            "publicKeyFingerprint",
        )
        require(fingerprint.contentEquals(IrohaHash.prehash(decodedPublicKey))) {
            "publicKeyFingerprint must equal IrohaHash.prehash(publicKeyBytes)"
        }
    }

    /** Return a fresh decoded copy of the recipient public key. */
    fun publicKeyBytes(): ByteArray = decodedPublicKey.copyOf()
}

/**
 * Deterministic private uploaded-model receipt; coordinates are zero for submission and positive
 * once committed.
 */
data class SoracloudPrivateUploadedModelExecutionReceipt(
    @JvmField val schemaVersion: Long,
    @JvmField val networkId: String,
    @JvmField val receiptId: String,
    @JvmField val serviceName: String,
    @JvmField val serviceVersion: String,
    @JvmField val modelId: String,
    @JvmField val weightVersion: String,
    @JvmField val runtimeVersion: String,
    @JvmField val modelManifestDigest: SoracloudImmutableList<Int>,
    @JvmField val modelBundleRoot: String,
    @JvmField val policyId: String,
    @JvmField val decryptionRequestId: String,
    @JvmField val attestingValidator: SoracloudRuntimeDeterministicValidatorHost,
    @JvmField val inputArtifact: SoracloudPrivateModelArtifactRef,
    @JvmField val outputArtifact: SoracloudPrivateModelArtifactRef,
    @JvmField val inputCommitment: String,
    @JvmField val outputCommitment: String,
    @JvmField val outputRecipient: SoracloudUploadedModelEncryptionRecipient,
    @JvmField val requestCommitment: String,
    @JvmField val resultCommitment: String,
    @JvmField val emittedSequence: BigInteger,
    @JvmField val emittedBlockHeight: BigInteger,
) {
    /** Build a receipt while taking an immutable snapshot of its manifest digest. */
    constructor(
        schemaVersion: Long,
        networkId: String,
        receiptId: String,
        serviceName: String,
        serviceVersion: String,
        modelId: String,
        weightVersion: String,
        runtimeVersion: String,
        modelManifestDigest: List<Int>,
        modelBundleRoot: String,
        policyId: String,
        decryptionRequestId: String,
        attestingValidator: SoracloudRuntimeDeterministicValidatorHost,
        inputArtifact: SoracloudPrivateModelArtifactRef,
        outputArtifact: SoracloudPrivateModelArtifactRef,
        inputCommitment: String,
        outputCommitment: String,
        outputRecipient: SoracloudUploadedModelEncryptionRecipient,
        requestCommitment: String,
        resultCommitment: String,
        emittedSequence: BigInteger,
        emittedBlockHeight: BigInteger,
    ) : this(
        schemaVersion,
        networkId,
        receiptId,
        serviceName,
        serviceVersion,
        modelId,
        weightVersion,
        runtimeVersion,
        SoracloudImmutableList.copyOf(modelManifestDigest),
        modelBundleRoot,
        policyId,
        decryptionRequestId,
        attestingValidator,
        inputArtifact,
        outputArtifact,
        inputCommitment,
        outputCommitment,
        outputRecipient,
        requestCommitment,
        resultCommitment,
        emittedSequence,
        emittedBlockHeight,
    )

    init {
        require(schemaVersion == 1L) { "schemaVersion must equal 1" }
        requireManifestDigest(modelManifestDigest, "modelManifestDigest")
        require(NetworkId.parse(networkId).literal == networkId) {
            "networkId must be an exact canonical checksummed 32-byte NetworkId literal"
        }
        for ((field, value) in listOf(
            "receiptId" to receiptId,
            "modelBundleRoot" to modelBundleRoot,
            "inputCommitment" to inputCommitment,
            "outputCommitment" to outputCommitment,
            "requestCommitment" to requestCommitment,
            "resultCommitment" to resultCommitment,
        )) {
            requireSoracloudHashLiteral(value, field)
        }
        requireCanonicalSoracloudName(serviceName, "serviceName")
        requireSoracloudUploadedModelServiceVersion(serviceVersion, "serviceVersion")
        requireSoracloudUploadedModelIdentifier(modelId, "modelId")
        requireSoracloudUploadedModelIdentifier(weightVersion, "weightVersion")
        for ((field, value) in listOf(
            "runtimeVersion" to runtimeVersion,
            "policyId" to policyId,
            "decryptionRequestId" to decryptionRequestId,
        )) {
            requireCanonicalSoracloudString(value, field)
        }
        require(runtimeVersion == SORACLOUD_PRIVATE_RUNTIME_VERSION_V1) {
            "runtimeVersion must equal $SORACLOUD_PRIVATE_RUNTIME_VERSION_V1"
        }
        require(inputArtifact.artifactRole == "input") {
            "inputArtifact.artifactRole must equal input"
        }
        require(outputArtifact.artifactRole == "output") {
            "outputArtifact.artifactRole must equal output"
        }
        require(inputArtifact.artifactHash != outputArtifact.artifactHash) {
            "outputArtifact.artifactHash must differ from inputArtifact.artifactHash"
        }
        requireSoracloudU64(emittedSequence, "emittedSequence")
        requireSoracloudU64(emittedBlockHeight, "emittedBlockHeight")
        require(
            (emittedSequence == BigInteger.ZERO && emittedBlockHeight == BigInteger.ZERO) ||
                (emittedSequence > BigInteger.ZERO && emittedBlockHeight > BigInteger.ZERO)
        ) {
            "emittedSequence and emittedBlockHeight must both be zero or both be positive"
        }
    }
}

/** Response emitted by `/v1/soracloud/model/upload/private/execute`. */
data class SoracloudPrivateUploadedModelExecuteResponse(
    @JvmField val schemaVersion: Long,
    @JvmField val status: SoracloudImmutableJsonObject,
    @JvmField val submissionStatus: String,
    @JvmField val transactionHash: String?,
    @JvmField val receipt: SoracloudPrivateUploadedModelExecutionReceipt,
    @JvmField val outputArtifact: SoracloudPrivateModelArtifactRef,
) {
    /** Build an execute response while taking a recursively immutable exact V1 status snapshot. */
    constructor(
        schemaVersion: Long,
        status: Map<String, Any?>,
        submissionStatus: String,
        transactionHash: String?,
        receipt: SoracloudPrivateUploadedModelExecutionReceipt,
        outputArtifact: SoracloudPrivateModelArtifactRef,
    ) : this(
        schemaVersion,
        SoracloudImmutableJsonObject.exactUploadedModelStatus(status),
        submissionStatus,
        transactionHash,
        receipt,
        outputArtifact,
    )

    init {
        require(schemaVersion == 1L) { "schemaVersion must equal 1" }
        require(submissionStatus == "submitted" || submissionStatus == "committed") {
            "submissionStatus must equal submitted or committed"
        }
        transactionHash?.let { requireSoracloudHashLiteral(it, "transactionHash") }
        require(submissionStatus != "submitted" || transactionHash != null) {
            "transactionHash is required for submitted"
        }
        require(submissionStatus != "committed" || transactionHash == null) {
            "transactionHash must be null for committed"
        }
        require(
            submissionStatus != "submitted" ||
                (receipt.emittedSequence == BigInteger.ZERO &&
                    receipt.emittedBlockHeight == BigInteger.ZERO)
        ) {
            "submitted receipt must use zero ledger coordinates"
        }
        require(
            submissionStatus != "committed" ||
                (receipt.emittedSequence > BigInteger.ZERO &&
                    receipt.emittedBlockHeight > BigInteger.ZERO)
        ) {
            "committed receipt must use positive ledger coordinates"
        }
        require(outputArtifact == receipt.outputArtifact) {
            "outputArtifact must match receipt.outputArtifact"
        }
    }
}

/** Response emitted by `/v1/soracloud/model/upload/private/receipts`. */
data class SoracloudPrivateUploadedModelReceiptListResponse(
    @JvmField val schemaVersion: Long,
    @JvmField val receipts: SoracloudImmutableList<SoracloudPrivateUploadedModelExecutionReceipt>,
    @JvmField val total: Long?,
    @JvmField val returnedItems: Long,
    @JvmField val remainingItems: Long?,
    @JvmField val hasMore: Boolean,
    @JvmField val countMode: String,
    @JvmField val continueCursor: String?,
) {
    /** Build a receipt-list response while taking an immutable receipt snapshot. */
    constructor(
        schemaVersion: Long,
        receipts: List<SoracloudPrivateUploadedModelExecutionReceipt>,
        total: Long?,
        returnedItems: Long,
        remainingItems: Long?,
        hasMore: Boolean,
        countMode: String,
        continueCursor: String?,
    ) : this(
        schemaVersion,
        SoracloudImmutableList.copyOf(receipts),
        total,
        returnedItems,
        remainingItems,
        hasMore,
        countMode,
        continueCursor,
    )

    init {
        require(schemaVersion == 1L) { "schemaVersion must equal 1" }
        require(countMode == "bounded" || countMode == "exact") {
            "countMode must equal bounded or exact"
        }
        require(total == null || total in 0L..SORACLOUD_U32_MAX) {
            "total must be null or within 0..=$SORACLOUD_U32_MAX"
        }
        require(returnedItems in 0L..SORACLOUD_U32_MAX) {
            "returnedItems must be within 0..=$SORACLOUD_U32_MAX"
        }
        require(remainingItems == null || remainingItems in 0L..SORACLOUD_U32_MAX) {
            "remainingItems must be null or within 0..=$SORACLOUD_U32_MAX"
        }
        require((countMode == "bounded") == (total == null)) {
            "total must be null for bounded countMode and non-null for exact countMode"
        }
        require((countMode == "bounded") == (remainingItems == null)) {
            "remainingItems must be null for bounded countMode and non-null for exact countMode"
        }
        require(returnedItems == receipts.size.toLong()) {
            "returnedItems must equal receipts.size"
        }
        require(hasMore == (continueCursor != null)) {
            "hasMore must equal continueCursor presence"
        }
        remainingItems?.let { remaining ->
            require(hasMore == (remaining > 0L)) {
                "hasMore must equal (remainingItems > 0) in exact mode"
            }
            val saturatedKnownItems = minOf(SORACLOUD_U32_MAX, returnedItems + remaining)
            require(requireNotNull(total) >= saturatedKnownItems) {
                "total must cover returnedItems and remainingItems in exact mode"
            }
        }
        require(
            receipts.all {
                it.emittedSequence > BigInteger.ZERO &&
                    it.emittedBlockHeight > BigInteger.ZERO
            }
        ) {
            "receipt-list entries must have positive ledger coordinates"
        }
        require(receipts.zipWithNext().all { (left, right) ->
            left.emittedSequence < right.emittedSequence ||
                (left.emittedSequence == right.emittedSequence && left.receiptId < right.receiptId)
        }) {
            "receipt-list entries must be strictly ordered by emittedSequence and receiptId"
        }
        continueCursor?.let { requireSoracloudPrivateReceiptCursor(it, "continueCursor") }
    }
}

private fun requireSoracloudPrivateReceiptCursor(value: String, field: String) {
    requireCanonicalSoracloudString(value, field)
    require(
        value.length == SORACLOUD_PRIVATE_RECEIPT_CURSOR_LENGTH_V1 &&
            value.all { it.isLetterOrDigit() && it.code < 128 || it == '-' || it == '_' }
    ) { "$field must be an exact canonical V1 receipt cursor" }
}

private fun requireSoracloudU64(value: BigInteger, field: String) {
    require(value >= BigInteger.ZERO && value <= SORACLOUD_U64_MAX) {
        "$field must fit in unsigned 64-bit range"
    }
}

private fun requireSoracloudUploadedModelIdentifier(value: String, field: String) {
    requireCanonicalSoracloudString(value, field)
    val utf8Bytes = value.toByteArray(StandardCharsets.UTF_8).size
    require(utf8Bytes <= SORACLOUD_UPLOADED_MODEL_IDENTIFIER_MAX_BYTES_V1) {
        "$field must contain at most " +
            "$SORACLOUD_UPLOADED_MODEL_IDENTIFIER_MAX_BYTES_V1 ASCII bytes"
    }
    require(value.all { character ->
        character in 'A'..'Z' ||
            character in 'a'..'z' ||
            character in '0'..'9' ||
            character == '-' ||
            character == '_' ||
            character == '.' ||
            character == ':' ||
            character == '#'
    }) {
        "$field must use only ASCII letters, digits, or [-_.:#]"
    }
}

private fun requireSoracloudUploadedModelServiceVersion(value: String, field: String) {
    requireCanonicalSoracloudString(value, field)
    val utf8Bytes = value.toByteArray(StandardCharsets.UTF_8).size
    require(utf8Bytes <= SORACLOUD_UPLOADED_MODEL_SERVICE_VERSION_MAX_BYTES_V1) {
        "$field must contain at most " +
            "$SORACLOUD_UPLOADED_MODEL_SERVICE_VERSION_MAX_BYTES_V1 UTF-8 bytes"
    }
}

private fun requireManifestDigest(value: List<Int>, field: String) {
    require(value.size == 32) { "$field must contain exactly 32 bytes" }
    require(value.all { it in 0..255 }) { "$field elements must be unsigned bytes" }
}

private fun requireCanonicalSoracloudString(value: String, field: String) {
    require(value.isNotBlank()) { "$field must be a non-empty string" }
    require(value == value.trim()) { "$field must not contain leading or trailing whitespace" }
    var index = 0
    while (index < value.length) {
        val character = value[index]
        when {
            Character.isHighSurrogate(character) -> {
                require(index + 1 < value.length && Character.isLowSurrogate(value[index + 1])) {
                    "$field must contain valid Unicode scalar values"
                }
                index += 2
            }
            Character.isLowSurrogate(character) -> {
                throw IllegalArgumentException("$field must contain valid Unicode scalar values")
            }
            else -> {
                require(!Character.isISOControl(character)) {
                    "$field must be free of control characters"
                }
                index++
            }
        }
    }
}

internal fun requireCanonicalSoracloudName(value: String, field: String) {
    requireCanonicalSoracloudString(value, field)
    require(value.toByteArray(StandardCharsets.UTF_8).size <= SORACLOUD_NAME_MAX_UTF8_BYTES) {
        "$field must contain at most $SORACLOUD_NAME_MAX_UTF8_BYTES UTF-8 bytes"
    }
    require(Normalizer.isNormalized(value, Normalizer.Form.NFC)) {
        "$field must use its exact NFC-normalized spelling"
    }
    var index = 0
    while (index < value.length) {
        val codePoint = Character.codePointAt(value, index)
        require(
            !Character.isWhitespace(codePoint) &&
                !Character.isSpaceChar(codePoint) &&
                !isSoracloudBidiControl(codePoint) &&
                codePoint != '@'.code &&
                codePoint != '#'.code &&
                codePoint != '$'.code
        ) { "$field must be a canonical Iroha Name" }
        index += Character.charCount(codePoint)
    }
}

private fun isSoracloudBidiControl(value: Int): Boolean =
    value == 0x061C ||
        value == 0x200E ||
        value == 0x200F ||
        value in 0x202A..0x202E ||
        value in 0x2066..0x2069

internal fun requireSoracloudHashLiteral(value: String, field: String): ByteArray {
    val bytes = try {
        HashLiteral.decode(value)
    } catch (error: IllegalArgumentException) {
        throw IllegalArgumentException(
            "$field must be an exact uppercase checksummed hash literal",
            error,
        )
    }
    require((bytes[bytes.size - 1].toInt() and 1) == 1) {
        "$field hash marker bit must be set"
    }
    require(HashLiteral.canonicalize(bytes) == value) {
        "$field must be an exact uppercase checksummed hash literal"
    }
    require(value != SORACLOUD_ZERO_PREHASH_SENTINEL) {
        "$field must not be the zero prehash sentinel"
    }
    return bytes
}

internal fun requireSoracloudValidatorIdentity(
    validatorAccountId: String,
    peerId: String,
) {
    val canonicalAccountId = requireCanonicalI105Address(
        validatorAccountId,
        "validatorAccountId",
    )
    val account = try {
        AccountAddress.fromI105(canonicalAccountId, null)
    } catch (error: AccountAddressException) {
        throw IllegalArgumentException(
            "validatorAccountId must use a canonical universal domainless AccountId",
            error,
        )
    }
    val signatory = requireNotNull(account.singleKeyPayload()) {
        "validatorAccountId must have exactly one signatory"
    }
    val peer = try {
        decodePublicKeyLiteral(peerId)
    } catch (error: IllegalArgumentException) {
        throw IllegalArgumentException("peerId must be a canonical PeerId", error)
    }
    require(peer != null && encodePublicKeyMultihash(peer.curveId, peer.keyBytes) == peerId) {
        "peerId must use the exact canonical peer public-key spelling"
    }
    require(
        peer.curveId == signatory.curveId &&
            peer.keyBytes.contentEquals(signatory.publicKey)
    ) {
        "peerId must equal validatorAccountId's exact single signatory"
    }
}

internal fun decodeCanonicalSoracloudX25519PublicKey(value: String, field: String): ByteArray {
    require(value == value.trim()) { "$field must not contain leading or trailing whitespace" }
    val publicKey = try {
        Base64.getDecoder().decode(value)
    } catch (error: IllegalArgumentException) {
        throw IllegalArgumentException("$field must be canonical base64", error)
    }
    require(
        publicKey.size == 32 && Base64.getEncoder().encodeToString(publicKey) == value
    ) {
        "$field must be canonical base64 encoding exactly 32 bytes"
    }
    require(!isLowOrderSoracloudX25519PublicKey(publicKey)) {
        "$field must not be a low-order X25519 public key"
    }
    return publicKey
}

private fun isLowOrderSoracloudX25519PublicKey(publicKey: ByteArray): Boolean {
    val peer = X25519PublicKeyParameters(publicKey, 0)
    val probe = X25519PrivateKeyParameters(SORACLOUD_LOW_ORDER_X25519_PROBE_PRIVATE_KEY, 0)
    val agreement = X25519Agreement()
    val shared = ByteArray(32)
    return try {
        agreement.init(probe)
        agreement.calculateAgreement(peer, shared, 0)
        shared.all { it.toInt() == 0 }
    } catch (_: IllegalStateException) {
        true
    } finally {
        Arrays.fill(shared, 0.toByte())
    }
}
