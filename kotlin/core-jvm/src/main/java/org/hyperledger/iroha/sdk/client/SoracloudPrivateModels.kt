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
import org.hyperledger.iroha.sdk.crypto.Blake3
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.numeric.KotodamaQuantity

private const val SORACLOUD_U32_MAX = 4_294_967_295L
private val SORACLOUD_U64_MAX = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
private const val SORACLOUD_NAME_MAX_UTF8_BYTES = 255
private const val SORACLOUD_UPLOADED_MODEL_IDENTIFIER_MAX_BYTES_V1 = 128
private const val SORACLOUD_UPLOADED_MODEL_SERVICE_VERSION_MAX_BYTES_V1 = 256
private const val SORACLOUD_PRIVATE_RUNTIME_VERSION_V1 = "soracloud.quantized-cpu.v1"
private const val SORACLOUD_X25519_HKDF_SHA256 = "X25519HkdfSha256"
private const val SORACLOUD_AES_256_GCM = "Aes256Gcm"
private const val SORACLOUD_PRIVATE_RECEIPT_CURSOR_LENGTH_V1 = 114
private const val SORAFS_AUTO_REPLICATION_ORDER_DOMAIN_V1 =
    "sorafs:auto-replication-order:v1"
private const val SORAFS_AUTO_REPLICATION_ORDER_NAMESPACE_TAG = 0x80
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
        private val BUNDLE_FIELDS = setOf(
            "schema_version", "service_name", "model_id", "weight_version", "family",
            "modalities", "plaintext_root", "runtime_format", "bundle_root",
            "sorafs_manifest_digest", "chunk_count", "plaintext_bytes", "ciphertext_bytes",
            "chunk_manifest_root", "upload_recipient", "wrapped_bundle_key", "pricing_policy",
            "decryption_policy_ref",
        )
        private val ARTIFACT_STATUS_FIELDS = setOf(
            "service_name", "model_name", "artifact_id", "training_job_id", "weight_version",
            "weight_artifact_hash", "dataset_ref", "training_config_hash",
            "reproducibility_hash", "provenance_attestation_hash", "registered_sequence",
            "consumed_by_version", "chunk_manifest_root",
        )
        private val RECIPIENT_FIELDS = setOf(
            "schema_version", "key_id", "key_version", "kem", "aead", "public_key_bytes",
            "public_key_fingerprint",
        )
        private val WRAPPED_KEY_FIELDS = setOf(
            "schema_version", "recipient_key_id", "recipient_key_version", "kem", "aead",
            "ephemeral_public_key", "nonce", "wrapped_key_ciphertext", "ciphertext_hash",
            "aad_digest",
        )
        private val UNIT_VARIANT_FIELDS = setOf("value")
        private val PRICING_FIELDS = setOf("storage_price")
        private const val MAX_JSON_DEPTH = 128

        internal fun exactUploadedModelStatus(
            value: Map<String, Any?>,
        ): SoracloudImmutableJsonObject {
            validateUploadedModelStatus(value, "status")
            return snapshotObject(value, "status", IdentityHashMap(), 0)
        }

        internal fun validateUploadedModelStatus(value: Map<String, Any?>, path: String) {
            requireExactFields(value, STATUS_FIELDS, path)
            requireSchemaVersionOne(value["schema_version"], "$path.schema_version")
            val bundle = exactObject(value["bundle"], BUNDLE_FIELDS, "$path.bundle")
            validateBundle(bundle, "$path.bundle")
            value["artifact"]?.let {
                validateArtifactStatus(
                    exactObject(it, ARTIFACT_STATUS_FIELDS, "$path.artifact"),
                    "$path.artifact",
                )
            }
        }

        private fun validateBundle(bundle: Map<String, Any?>, path: String) {
            requireSchemaVersionOne(bundle["schema_version"], "$path.schema_version")
            requireCanonicalSoracloudName(
                exactString(bundle["service_name"], "$path.service_name"),
                "$path.service_name",
            )
            requireSoracloudUploadedModelIdentifier(
                exactString(bundle["model_id"], "$path.model_id"),
                "$path.model_id",
            )
            requireSoracloudUploadedModelIdentifier(
                exactString(bundle["weight_version"], "$path.weight_version"),
                "$path.weight_version",
            )
            requireCanonicalSoracloudString(exactString(bundle["family"], "$path.family"), "$path.family")
            val modalities = exactArray(bundle["modalities"], "$path.modalities")
            require(modalities.isNotEmpty()) { "$path.modalities must not be empty" }
            val canonicalModalities = modalities.mapIndexed { index, value ->
                exactString(value, "$path.modalities[$index]").also {
                    requireCanonicalSoracloudString(it, "$path.modalities[$index]")
                }
            }
            require(canonicalModalities.toSet().size == canonicalModalities.size) {
                "$path.modalities entries must be unique"
            }
            exactHash(bundle["plaintext_root"], "$path.plaintext_root")
            exactUnitVariant(
                bundle["runtime_format"],
                "runtime_format",
                "DeterministicQuantizedCpuV1",
                "$path.runtime_format",
            )
            exactHash(bundle["bundle_root"], "$path.bundle_root")
            exactManifestDigest(bundle["sorafs_manifest_digest"], "$path.sorafs_manifest_digest")
            exactUnsigned(
                bundle["chunk_count"],
                "$path.chunk_count",
                BigInteger.valueOf(SORACLOUD_U32_MAX),
                positive = true,
            )
            exactUnsigned(
                bundle["plaintext_bytes"],
                "$path.plaintext_bytes",
                SORACLOUD_U64_MAX,
                positive = true,
            )
            exactUnsigned(
                bundle["ciphertext_bytes"],
                "$path.ciphertext_bytes",
                SORACLOUD_U64_MAX,
                positive = true,
            )
            exactHash(bundle["chunk_manifest_root"], "$path.chunk_manifest_root")
            val recipient = exactObject(bundle["upload_recipient"], RECIPIENT_FIELDS, "$path.upload_recipient")
            validateRecipient(recipient, "$path.upload_recipient")
            val wrappedKey = exactObject(bundle["wrapped_bundle_key"], WRAPPED_KEY_FIELDS, "$path.wrapped_bundle_key")
            validateWrappedKey(wrappedKey, recipient, "$path.wrapped_bundle_key")
            val pricing = exactObject(bundle["pricing_policy"], PRICING_FIELDS, "$path.pricing_policy")
            val storagePrice = exactString(pricing["storage_price"], "$path.pricing_policy.storage_price")
            try {
                KotodamaQuantity.parseCanonical(storagePrice)
            } catch (error: IllegalArgumentException) {
                throw IllegalArgumentException(
                    "$path.pricing_policy.storage_price must be a canonical quantity",
                    error,
                )
            }
            requireCanonicalSoracloudString(
                exactString(bundle["decryption_policy_ref"], "$path.decryption_policy_ref"),
                "$path.decryption_policy_ref",
            )
        }

        private fun validateArtifactStatus(artifact: Map<String, Any?>, path: String) {
            requireCanonicalSoracloudName(
                exactString(artifact["service_name"], "$path.service_name"),
                "$path.service_name",
            )
            for (field in listOf("model_name", "artifact_id", "training_job_id", "dataset_ref")) {
                requireCanonicalSoracloudString(exactString(artifact[field], "$path.$field"), "$path.$field")
            }
            optionalString(artifact["weight_version"], "$path.weight_version")?.let {
                requireSoracloudUploadedModelIdentifier(it, "$path.weight_version")
            }
            for (field in listOf(
                "weight_artifact_hash", "training_config_hash", "reproducibility_hash",
                "provenance_attestation_hash",
            )) {
                exactHash(artifact[field], "$path.$field")
            }
            exactUnsigned(
                artifact["registered_sequence"],
                "$path.registered_sequence",
                SORACLOUD_U64_MAX,
                positive = true,
            )
            optionalString(artifact["consumed_by_version"], "$path.consumed_by_version")?.let {
                requireCanonicalSoracloudString(it, "$path.consumed_by_version")
            }
            artifact["chunk_manifest_root"]?.let { exactHash(it, "$path.chunk_manifest_root") }
        }

        private fun validateRecipient(recipient: Map<String, Any?>, path: String) {
            val schemaVersion = exactUnsigned(
                recipient["schema_version"],
                "$path.schema_version",
                BigInteger.ONE,
                positive = true,
            )
            val keyId = exactString(recipient["key_id"], "$path.key_id")
            val keyVersion = exactUnsigned(
                recipient["key_version"],
                "$path.key_version",
                BigInteger.valueOf(SORACLOUD_U32_MAX),
                positive = true,
            )
            val kem = exactUnitVariant(recipient["kem"], "kem", SORACLOUD_X25519_HKDF_SHA256, "$path.kem")
            val aead = exactUnitVariant(recipient["aead"], "aead", SORACLOUD_AES_256_GCM, "$path.aead")
            SoracloudUploadedModelEncryptionRecipient(
                schemaVersion = schemaVersion.toLong(),
                keyId = keyId,
                keyVersion = keyVersion.toLong(),
                kem = kem,
                aead = aead,
                publicKeyBytesBase64 = exactString(recipient["public_key_bytes"], "$path.public_key_bytes"),
                publicKeyFingerprint = exactHash(recipient["public_key_fingerprint"], "$path.public_key_fingerprint"),
            )
        }

        private fun validateWrappedKey(
            wrappedKey: Map<String, Any?>,
            recipient: Map<String, Any?>,
            path: String,
        ) {
            requireSchemaVersionOne(wrappedKey["schema_version"], "$path.schema_version")
            val recipientKeyId = exactString(wrappedKey["recipient_key_id"], "$path.recipient_key_id")
            requireCanonicalSoracloudString(recipientKeyId, "$path.recipient_key_id")
            val recipientKeyVersion = exactUnsigned(
                wrappedKey["recipient_key_version"],
                "$path.recipient_key_version",
                BigInteger.valueOf(SORACLOUD_U32_MAX),
                positive = true,
            )
            exactUnitVariant(wrappedKey["kem"], "kem", SORACLOUD_X25519_HKDF_SHA256, "$path.kem")
            exactUnitVariant(wrappedKey["aead"], "aead", SORACLOUD_AES_256_GCM, "$path.aead")
            decodeCanonicalSoracloudX25519PublicKey(
                exactString(wrappedKey["ephemeral_public_key"], "$path.ephemeral_public_key"),
                "$path.ephemeral_public_key",
            )
            canonicalBase64(wrappedKey["nonce"], "$path.nonce", 1, 256)
            val ciphertext = canonicalBase64(
                wrappedKey["wrapped_key_ciphertext"],
                "$path.wrapped_key_ciphertext",
                1,
                4_096,
            )
            val ciphertextHash = exactHash(wrappedKey["ciphertext_hash"], "$path.ciphertext_hash")
            require(HashLiteral.canonicalize(IrohaHash.prehash(ciphertext)) == ciphertextHash) {
                "$path.ciphertext_hash must match wrapped_key_ciphertext"
            }
            exactHash(wrappedKey["aad_digest"], "$path.aad_digest")
            require(recipientKeyId == recipient["key_id"] && recipientKeyVersion == exactUnsigned(
                recipient["key_version"],
                "$path.recipient_key_version",
                BigInteger.valueOf(SORACLOUD_U32_MAX),
                positive = true,
            )) {
                "$path recipient key must match upload_recipient"
            }
        }

        private fun exactObject(value: Any?, fields: Set<String>, path: String): Map<String, Any?> {
            require(value is Map<*, *>) { "$path must be a JSON object" }
            require(value.keys.all { it is String }) { "$path keys must be strings" }
            @Suppress("UNCHECKED_CAST")
            val objectValue = value as Map<String, Any?>
            requireExactFields(objectValue, fields, path)
            return objectValue
        }

        private fun requireExactFields(value: Map<String, Any?>, fields: Set<String>, path: String) {
            require(value.keys == fields) { "$path must contain exactly ${fields.joinToString()}" }
        }

        private fun exactArray(value: Any?, path: String): List<Any?> {
            require(value is List<*>) { "$path must be a JSON array" }
            return value
        }

        private fun exactString(value: Any?, path: String): String {
            require(value is String) { "$path must be a string" }
            return value
        }

        private fun optionalString(value: Any?, path: String): String? =
            if (value == null) null else exactString(value, path)

        private fun requireSchemaVersionOne(value: Any?, path: String) {
            require(isSchemaVersionOne(value)) { "$path must equal 1" }
        }

        private fun exactUnsigned(
            value: Any?,
            path: String,
            maximum: BigInteger,
            positive: Boolean,
        ): BigInteger {
            val parsed = when (value) {
                is BigInteger -> value
                is Byte, is Short, is Int, is Long -> BigInteger.valueOf((value as Number).toLong())
                else -> throw IllegalArgumentException("$path must be an integer")
            }
            require(parsed.signum() >= (if (positive) 1 else 0) && parsed <= maximum) {
                "$path is outside its unsigned range"
            }
            return parsed
        }

        private fun exactManifestDigest(value: Any?, path: String): List<Int> {
            val values = exactArray(value, path)
            require(values.size == 32) { "$path must contain exactly 32 bytes" }
            return values.mapIndexed { index, element ->
                exactUnsigned(element, "$path[$index]", BigInteger.valueOf(255L), positive = false).toInt()
            }
        }

        private fun exactHash(value: Any?, path: String): String {
            val literal = exactString(value, path)
            requireSoracloudHashLiteral(literal, path)
            return literal
        }

        private fun exactUnitVariant(
            value: Any?,
            tag: String,
            expected: String,
            path: String,
        ): String {
            val variant = exactObject(value, UNIT_VARIANT_FIELDS + tag, path)
            require(exactString(variant[tag], "$path.$tag") == expected) {
                "$path.$tag must equal $expected"
            }
            require(variant["value"] == null) { "$path.value must be null" }
            return expected
        }

        private fun canonicalBase64(
            value: Any?,
            path: String,
            minimumBytes: Int,
            maximumBytes: Int,
        ): ByteArray {
            val encoded = exactString(value, path)
            val decoded = try {
                Base64.getDecoder().decode(encoded)
            } catch (error: IllegalArgumentException) {
                throw IllegalArgumentException("$path must be canonical base64", error)
            }
            require(
                decoded.size in minimumBytes..maximumBytes &&
                    Base64.getEncoder().encodeToString(decoded) == encoded
            ) {
                "$path must be canonical base64 containing $minimumBytes..=$maximumBytes bytes"
            }
            return decoded
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

/** Closed first-release phase of private uploaded-model transaction submission. */
enum class SoracloudPrivateUploadedModelSubmissionPhase(
    /** Exact Norito JSON spelling. */
    @JvmField val wireValue: String,
) {
    /** Encrypted output exists, but its durability transaction has not been submitted. */
    AWAITING_OUTPUT_DURABILITY("awaiting_output_durability"),

    /** The output pin transaction has been submitted. */
    OUTPUT_PIN_SUBMITTED("output_pin_submitted"),

    /** The durable-output receipt transaction has been submitted. */
    RECEIPT_SUBMITTED("receipt_submitted"),

    /** The execution receipt has committed with ledger-assigned coordinates. */
    COMMITTED("committed"),
    ;

    override fun toString(): String = wireValue

    companion object {
        /** Parse the exact first-release Norito JSON spelling. */
        @JvmStatic
        fun fromWireValue(value: String): SoracloudPrivateUploadedModelSubmissionPhase =
            when (value) {
                "awaiting_output_durability" -> AWAITING_OUTPUT_DURABILITY
                "output_pin_submitted" -> OUTPUT_PIN_SUBMITTED
                "receipt_submitted" -> RECEIPT_SUBMITTED
                "committed" -> COMMITTED
                else -> throw IllegalArgumentException(
                    "submission phase must equal awaiting_output_durability, " +
                        "output_pin_submitted, receipt_submitted, or committed"
                )
            }

        internal fun requiresTransactionHash(
            phase: SoracloudPrivateUploadedModelSubmissionPhase,
        ): Boolean = phase == OUTPUT_PIN_SUBMITTED || phase == RECEIPT_SUBMITTED

        internal fun requiresAssignedReceipt(
            phase: SoracloudPrivateUploadedModelSubmissionPhase,
        ): Boolean = phase == COMMITTED
    }
}

/**
 * Deterministic private uploaded-model receipt; coordinates are zero until commit and positive
 * once committed.
 */
class SoracloudPrivateUploadedModelExecutionReceipt(
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
    @JvmField val outputReplicationOrderId: SoracloudImmutableList<Int>,
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
        outputReplicationOrderId: List<Int>,
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
        SoracloudImmutableList.copyOf(outputReplicationOrderId),
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
        requireOutputReplicationOrderId(
            outputReplicationOrderId,
            outputArtifact.sorafsManifestDigest,
            "outputReplicationOrderId",
        )
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

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is SoracloudPrivateUploadedModelExecutionReceipt) return false
        return schemaVersion == other.schemaVersion &&
            networkId == other.networkId &&
            receiptId == other.receiptId &&
            serviceName == other.serviceName &&
            serviceVersion == other.serviceVersion &&
            modelId == other.modelId &&
            weightVersion == other.weightVersion &&
            runtimeVersion == other.runtimeVersion &&
            modelManifestDigest == other.modelManifestDigest &&
            modelBundleRoot == other.modelBundleRoot &&
            policyId == other.policyId &&
            decryptionRequestId == other.decryptionRequestId &&
            attestingValidator == other.attestingValidator &&
            inputArtifact == other.inputArtifact &&
            outputArtifact == other.outputArtifact &&
            outputReplicationOrderId == other.outputReplicationOrderId &&
            inputCommitment == other.inputCommitment &&
            outputCommitment == other.outputCommitment &&
            outputRecipient == other.outputRecipient &&
            requestCommitment == other.requestCommitment &&
            resultCommitment == other.resultCommitment &&
            emittedSequence == other.emittedSequence &&
            emittedBlockHeight == other.emittedBlockHeight
    }

    override fun hashCode(): Int {
        var result = schemaVersion.hashCode()
        result = 31 * result + networkId.hashCode()
        result = 31 * result + receiptId.hashCode()
        result = 31 * result + serviceName.hashCode()
        result = 31 * result + serviceVersion.hashCode()
        result = 31 * result + modelId.hashCode()
        result = 31 * result + weightVersion.hashCode()
        result = 31 * result + runtimeVersion.hashCode()
        result = 31 * result + modelManifestDigest.hashCode()
        result = 31 * result + modelBundleRoot.hashCode()
        result = 31 * result + policyId.hashCode()
        result = 31 * result + decryptionRequestId.hashCode()
        result = 31 * result + attestingValidator.hashCode()
        result = 31 * result + inputArtifact.hashCode()
        result = 31 * result + outputArtifact.hashCode()
        result = 31 * result + outputReplicationOrderId.hashCode()
        result = 31 * result + inputCommitment.hashCode()
        result = 31 * result + outputCommitment.hashCode()
        result = 31 * result + outputRecipient.hashCode()
        result = 31 * result + requestCommitment.hashCode()
        result = 31 * result + resultCommitment.hashCode()
        result = 31 * result + emittedSequence.hashCode()
        result = 31 * result + emittedBlockHeight.hashCode()
        return result
    }

    override fun toString(): String =
        "SoracloudPrivateUploadedModelExecutionReceipt(" +
            "schemaVersion=$schemaVersion, networkId=$networkId, receiptId=$receiptId, " +
            "serviceName=$serviceName, serviceVersion=$serviceVersion, modelId=$modelId, " +
            "weightVersion=$weightVersion, runtimeVersion=$runtimeVersion, " +
            "modelManifestDigest=$modelManifestDigest, modelBundleRoot=$modelBundleRoot, " +
            "policyId=$policyId, decryptionRequestId=$decryptionRequestId, " +
            "attestingValidator=$attestingValidator, inputArtifact=$inputArtifact, " +
            "outputArtifact=$outputArtifact, " +
            "outputReplicationOrderId=$outputReplicationOrderId, " +
            "inputCommitment=$inputCommitment, outputCommitment=$outputCommitment, " +
            "outputRecipient=$outputRecipient, requestCommitment=$requestCommitment, " +
            "resultCommitment=$resultCommitment, emittedSequence=$emittedSequence, " +
            "emittedBlockHeight=$emittedBlockHeight)"
}

internal fun requireUploadedModelStatusMatchesReceipt(
    status: Map<String, Any?>,
    receipt: SoracloudPrivateUploadedModelExecutionReceipt,
    path: String,
) {
    @Suppress("UNCHECKED_CAST")
    val bundle = status["bundle"] as Map<String, Any?>
    for ((field, expected) in listOf(
        "service_name" to receipt.serviceName,
        "model_id" to receipt.modelId,
        "weight_version" to receipt.weightVersion,
        "bundle_root" to receipt.modelBundleRoot,
        "decryption_policy_ref" to receipt.policyId,
    )) {
        require(bundle[field] == expected) {
            "$path.bundle.$field must match receipt"
        }
    }
    require(statusUnsignedBytes(bundle["sorafs_manifest_digest"]) == receipt.modelManifestDigest) {
        "$path.bundle.sorafs_manifest_digest must match receipt.modelManifestDigest"
    }
    status["artifact"]?.let { rawArtifact ->
        @Suppress("UNCHECKED_CAST")
        val artifact = rawArtifact as Map<String, Any?>
        require(artifact["service_name"] == receipt.serviceName) {
            "$path.artifact.service_name must match receipt.serviceName"
        }
        require(artifact["weight_version"] == receipt.weightVersion) {
            "$path.artifact.weight_version must match receipt.weightVersion"
        }
        require(artifact["chunk_manifest_root"] == bundle["chunk_manifest_root"]) {
            "$path.artifact.chunk_manifest_root must match bundle.chunk_manifest_root"
        }
    }
}

private fun statusUnsignedBytes(value: Any?): List<Int> {
    @Suppress("UNCHECKED_CAST")
    return (value as List<Any?>).map { (it as Number).toInt() }
}

/** Response emitted by `/v1/soracloud/model/upload/private/execute`. */
class SoracloudPrivateUploadedModelExecuteResponse(
    @JvmField val schemaVersion: Long,
    @JvmField val status: SoracloudImmutableJsonObject,
    @JvmField val submissionPhase: SoracloudPrivateUploadedModelSubmissionPhase,
    @JvmField val transactionHash: String?,
    @JvmField val receipt: SoracloudPrivateUploadedModelExecutionReceipt,
    @JvmField val outputArtifact: SoracloudPrivateModelArtifactRef,
) {
    /** Build an execute response while taking a recursively immutable exact V1 status snapshot. */
    constructor(
        schemaVersion: Long,
        status: Map<String, Any?>,
        submissionPhase: SoracloudPrivateUploadedModelSubmissionPhase,
        transactionHash: String?,
        receipt: SoracloudPrivateUploadedModelExecutionReceipt,
        outputArtifact: SoracloudPrivateModelArtifactRef,
    ) : this(
        schemaVersion,
        SoracloudImmutableJsonObject.exactUploadedModelStatus(status),
        submissionPhase,
        transactionHash,
        receipt,
        outputArtifact,
    )

    init {
        require(schemaVersion == 1L) { "schemaVersion must equal 1" }
        transactionHash?.let { requireSoracloudHashLiteral(it, "transactionHash") }
        val requiresTransactionHash =
            SoracloudPrivateUploadedModelSubmissionPhase.requiresTransactionHash(submissionPhase)
        require(requiresTransactionHash == (transactionHash != null)) {
            if (requiresTransactionHash) {
                "transactionHash is required for ${submissionPhase.wireValue}"
            } else {
                "transactionHash must be null for ${submissionPhase.wireValue}"
            }
        }
        val receiptIsAssigned =
            receipt.emittedSequence > BigInteger.ZERO && receipt.emittedBlockHeight > BigInteger.ZERO
        val requiresAssignedReceipt =
            SoracloudPrivateUploadedModelSubmissionPhase.requiresAssignedReceipt(submissionPhase)
        require(requiresAssignedReceipt == receiptIsAssigned) {
            if (requiresAssignedReceipt) {
                "committed receipt must use positive ledger coordinates"
            } else {
                "${submissionPhase.wireValue} receipt must use zero ledger coordinates"
            }
        }
        require(outputArtifact == receipt.outputArtifact) {
            "outputArtifact must match receipt.outputArtifact"
        }
        requireUploadedModelStatusMatchesReceipt(status, receipt, "status")
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is SoracloudPrivateUploadedModelExecuteResponse) return false
        return schemaVersion == other.schemaVersion &&
            status == other.status &&
            submissionPhase == other.submissionPhase &&
            transactionHash == other.transactionHash &&
            receipt == other.receipt &&
            outputArtifact == other.outputArtifact
    }

    override fun hashCode(): Int {
        var result = schemaVersion.hashCode()
        result = 31 * result + status.hashCode()
        result = 31 * result + submissionPhase.hashCode()
        result = 31 * result + (transactionHash?.hashCode() ?: 0)
        result = 31 * result + receipt.hashCode()
        result = 31 * result + outputArtifact.hashCode()
        return result
    }

    override fun toString(): String =
        "SoracloudPrivateUploadedModelExecuteResponse(" +
            "schemaVersion=$schemaVersion, status=$status, submissionPhase=$submissionPhase, " +
            "transactionHash=$transactionHash, receipt=$receipt, outputArtifact=$outputArtifact)"
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

private fun requireOutputReplicationOrderId(
    value: List<Int>,
    outputManifestDigest: List<Int>,
    field: String,
) {
    requireManifestDigest(value, field)
    require(value == deriveSorafsAutoReplicationOrderIdV1(outputManifestDigest)) {
        "$field must equal the tagged automatic replication-order ID derived from " +
            "outputArtifact.sorafsManifestDigest"
    }
}

/** Derive the tagged automatic SoraFS replication-order ID for an output manifest digest. */
internal fun deriveSorafsAutoReplicationOrderIdV1(
    outputManifestDigest: List<Int>,
): List<Int> {
    requireManifestDigest(outputManifestDigest, "outputManifestDigest")
    val domain = SORAFS_AUTO_REPLICATION_ORDER_DOMAIN_V1
        .toByteArray(StandardCharsets.US_ASCII)
    val preimage = ByteArray(domain.size + outputManifestDigest.size)
    domain.copyInto(preimage)
    outputManifestDigest.forEachIndexed { index, value ->
        preimage[domain.size + index] = value.toByte()
    }
    val orderId = Blake3.hash(preimage)
    orderId[0] = (orderId[0].toInt() or SORAFS_AUTO_REPLICATION_ORDER_NAMESPACE_TAG).toByte()
    return orderId.map { it.toInt() and 0xff }
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
