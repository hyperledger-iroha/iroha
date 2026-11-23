package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.util.Collections
import java.util.LinkedHashMap
import org.hyperledger.iroha.sdk.address.decodePublicKeyLiteral
import org.hyperledger.iroha.sdk.address.compactPublicKeyPayload
import org.hyperledger.iroha.sdk.address.encodePublicKeyMultihash
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.core.model.NetworkId

/** Typed first-release models for Torii DA commitment and pin-intent proofs. */
object DaModels {
    private val U32_MAX = BigInteger("4294967295")
    private val U64_MAX = BigInteger("18446744073709551615")

    /** Canonical transparent Norito JSON wrapper around a 32-byte DA digest. */
    class Digest32 private constructor(bytes: ByteArray) {
        private val value = bytes.copyOf()

        init {
            require(value.size == 32) { "DA digest must contain exactly 32 bytes" }
        }

        fun bytes(): ByteArray = value.copyOf()
        fun hex(): String = value.joinToString("") { "%02x".format(it.toInt() and 0xff) }
        internal fun toJsonValue(): List<List<Int>> =
            listOf(value.map { it.toInt() and 0xff })

        override fun equals(other: Any?): Boolean =
            other is Digest32 && value.contentEquals(other.value)

        override fun hashCode(): Int = value.contentHashCode()
        override fun toString(): String = hex()

        companion object {
            @JvmStatic
            fun fromBytes(bytes: ByteArray): Digest32 = Digest32(bytes)

            @JvmStatic
            fun fromHex(hex: String): Digest32 {
                var body = hex.trim()
                if (body.startsWith("0x", true)) body = body.substring(2)
                require(body.length == 64 && body.matches(Regex("[0-9a-fA-F]{64}"))) {
                    "DA digest must be a 32-byte hex string"
                }
                return Digest32(ByteArray(32) { index ->
                    body.substring(index * 2, index * 2 + 2).toInt(16).toByte()
                })
            }

            internal fun fromJson(value: Any?, field: String): Digest32 {
                val outer = DaJson.list(value, field)
                require(outer.size == 1) { "$field must contain one transparent-wrapper item" }
                val inner = DaJson.list(outer[0], "$field[0]")
                require(inner.size == 32) { "$field must contain exactly 32 bytes" }
                return Digest32(ByteArray(32) { index ->
                    DaJson.u8(inner[index], "$field[0][$index]").toByte()
                })
            }
        }
    }

    /** Canonical ledger tip binding a DA list cursor to one immutable view. */
    data class ListSnapshot(
        val blockHeight: BigInteger,
        val blockHash: String?,
    ) {
        init {
            requireU64(blockHeight, "blockHeight")
            if (blockHash != null) {
                DaJson.requireHash(blockHash, "blockHash")
            }
            require((blockHeight == BigInteger.ZERO) == (blockHash == null)) {
                "DA list snapshots require no block hash at height zero and one at nonzero height"
            }
        }

        internal fun toJsonValue(): Map<String, Any?> = linkedMapOf(
            "block_height" to blockHeight,
            "block_hash" to blockHash,
        )
    }

    /** Canonical `(lane_id, epoch, sequence)` ordering key for commitments. */
    data class CommitmentKey(
        val laneId: Long,
        val epoch: BigInteger,
        val sequence: BigInteger,
    ) {
        init {
            requireU32(BigInteger.valueOf(laneId), "laneId")
            requireU64(epoch, "epoch")
            requireU64(sequence, "sequence")
        }

        internal fun toJsonValue(): Map<String, Any?> = linkedMapOf(
            "lane_id" to laneId,
            "epoch" to epoch,
            "sequence" to sequence,
        )
    }

    /** Forward-only cursor for canonically ordered commitments. */
    data class CommitmentListCursor(
        val snapshot: ListSnapshot,
        val after: CommitmentKey,
    ) {
        internal fun toJsonValue(): Map<String, Any?> = linkedMapOf(
            "snapshot" to snapshot.toJsonValue(),
            "after" to after.toJsonValue(),
        )
    }

    /** Cursor request for bounded commitment traversal. */
    data class CommitmentListRequest(
        val limit: BigInteger? = null,
        val cursor: CommitmentListCursor? = null,
    ) {
        init {
            requireOptionalNonZeroU64(limit, "limit")
        }

        internal fun toJsonBytes(): ByteArray {
            val value = LinkedHashMap<String, Any?>()
            if (limit != null) value["limit"] = limit
            if (cursor != null) value["cursor"] = cursor.toJsonValue()
            return DaJson.encode(value)
        }
    }

    /** Exact selector for producing one commitment proof. */
    data class CommitmentProofRequest(
        val manifestHash: Digest32? = null,
        val laneId: Long? = null,
        val epoch: BigInteger? = null,
        val sequence: BigInteger? = null,
    ) {
        init {
            requireOptionalU32(laneId, "laneId")
            requireOptionalU64(epoch, "epoch")
            requireOptionalU64(sequence, "sequence")
        }

        internal fun toJsonBytes(): ByteArray {
            val value = LinkedHashMap<String, Any?>()
            if (manifestHash != null) value["manifest_hash"] = manifestHash.toJsonValue()
            if (laneId != null) value["lane_id"] = laneId
            if (epoch != null) value["epoch"] = epoch
            if (sequence != null) value["sequence"] = sequence
            return DaJson.encode(value)
        }
    }

    /** Forward-only cursor for canonically ordered pin intents. */
    data class PinIntentListCursor(
        val snapshot: ListSnapshot,
        val after: Location,
    ) {
        internal fun toJsonValue(): Map<String, Any?> = linkedMapOf(
            "snapshot" to snapshot.toJsonValue(),
            "after" to after.toJsonValue(),
        )
    }

    /** Cursor request for bounded pin-intent traversal. */
    data class PinIntentListRequest(
        val limit: BigInteger? = null,
        val cursor: PinIntentListCursor? = null,
    ) {
        init {
            requireOptionalNonZeroU64(limit, "limit")
        }

        internal fun toJsonBytes(): ByteArray {
            val value = LinkedHashMap<String, Any?>()
            if (limit != null) value["limit"] = limit
            if (cursor != null) value["cursor"] = cursor.toJsonValue()
            return DaJson.encode(value)
        }
    }

    /** Exact selector for producing one pin-intent proof. */
    data class PinIntentQueryRequest(
        val manifestHash: Digest32? = null,
        val storageTicket: Digest32? = null,
        val alias: String? = null,
        val laneId: Long? = null,
        val epoch: BigInteger? = null,
        val sequence: BigInteger? = null,
    ) {
        init {
            if (alias != null) {
                requirePinIntentAlias(alias, "DA pin-intent alias")
            }
            requireOptionalU32(laneId, "laneId")
            requireOptionalU64(epoch, "epoch")
            requireOptionalU64(sequence, "sequence")
        }

        internal fun toJsonBytes(): ByteArray {
            val value = LinkedHashMap<String, Any?>()
            if (manifestHash != null) value["manifest_hash"] = manifestHash.toJsonValue()
            if (storageTicket != null) value["storage_ticket"] = storageTicket.toJsonValue()
            if (alias != null) value["alias"] = alias
            if (laneId != null) value["lane_id"] = laneId
            if (epoch != null) value["epoch"] = epoch
            if (sequence != null) value["sequence"] = sequence
            return DaJson.encode(value)
        }
    }

    enum class ProofScheme(val wireName: String) {
        SORAFS_POR_BLAKE3("SorafsPorBlake3");

        internal fun toJsonValue(): Map<String, Any?> =
            linkedMapOf("type" to wireName, "value" to null)

        companion object {
            internal fun fromJson(value: Any?, field: String): ProofScheme {
                val type = DaJson.taggedUnit(
                    value,
                    field,
                    "type",
                    setOf("SorafsPorBlake3"),
                )
                return values().first { it.wireName == type }
            }
        }
    }

    enum class StorageClass(val wireName: String) {
        HOT("Hot"),
        WARM("Warm"),
        COLD("Cold");

        internal fun toJsonValue(): Map<String, Any?> =
            linkedMapOf("type" to wireName, "value" to null)

        companion object {
            internal fun fromJson(value: Any?, field: String): StorageClass {
                val type = DaJson.taggedUnit(
                    value,
                    field,
                    "type",
                    setOf("Hot", "Warm", "Cold"),
                )
                return values().first { it.wireName == type }
            }
        }
    }

    data class RetentionPolicy(
        val hotRetentionSeconds: BigInteger,
        val coldRetentionSeconds: BigInteger,
        val requiredReplicas: Int,
        val storageClass: StorageClass,
        val governanceTag: String,
    ) {
        init {
            requireU64(hotRetentionSeconds, "hotRetentionSeconds")
            requireU64(coldRetentionSeconds, "coldRetentionSeconds")
            require(requiredReplicas in 0..65535) { "requiredReplicas must fit u16" }
        }

        internal fun toJsonValue(): Map<String, Any?> = linkedMapOf(
            "hot_retention_secs" to hotRetentionSeconds,
            "cold_retention_secs" to coldRetentionSeconds,
            "required_replicas" to requiredReplicas,
            "storage_class" to storageClass.toJsonValue(),
            "governance_tag" to listOf(governanceTag),
        )
    }

    data class CommitmentRecord(
        val laneId: Long,
        val epoch: BigInteger,
        val sequence: BigInteger,
        val clientBlobId: Digest32,
        val manifestHash: Digest32,
        val proofScheme: ProofScheme,
        val chunkRoot: Digest32,
        val proofDigest: String?,
        val retentionClass: RetentionPolicy,
        val storageTicket: Digest32,
        val acknowledgementSignature: String,
    ) {
        init {
            requireU32(BigInteger.valueOf(laneId), "laneId")
            requireU64(epoch, "epoch")
            requireU64(sequence, "sequence")
            proofDigest?.let { DaJson.requireHash(it, "proofDigest") }
            requireSignatureHex(acknowledgementSignature, "DA acknowledgement")
        }

        internal fun toJsonValue(): Map<String, Any?> = linkedMapOf(
            "lane_id" to laneId,
            "epoch" to epoch,
            "sequence" to sequence,
            "client_blob_id" to clientBlobId.toJsonValue(),
            "manifest_hash" to manifestHash.toJsonValue(),
            "proof_scheme" to proofScheme.toJsonValue(),
            "chunk_root" to chunkRoot.toJsonValue(),
            "proof_digest" to proofDigest,
            "retention_class" to retentionClass.toJsonValue(),
            "storage_ticket" to storageTicket.toJsonValue(),
            "acknowledgement_sig" to acknowledgementSignature,
        )
    }

    /** Ed25519 or another supported controller's signature over one DA signing domain. */
    data class Witness(val signer: String, val signature: String) {
        private val keyOrder: ByteArray

        init {
            val key = requireNotNull(decodePublicKeyLiteral(signer)) { "invalid DA controller key" }
            require(encodePublicKeyMultihash(key.curveId, key.keyBytes) == signer) {
                "DA controller witness requires the canonical public-key literal"
            }
            keyOrder = compactPublicKeyPayload(key.curveId, key.keyBytes)
            requireSignatureHex(signature, "DA signature")
        }

        internal fun precedes(other: Witness): Boolean {
            for (index in 0 until minOf(keyOrder.size, other.keyOrder.size)) {
                val left = keyOrder[index].toInt() and 0xff
                val right = other.keyOrder[index].toInt() and 0xff
                if (left != right) return left < right
            }
            return keyOrder.size < other.keyOrder.size
        }

        internal fun toJsonValue() = mapOf("signer" to signer, "signature" to signature)
    }

    private fun requireSignatureHex(value: String, field: String) {
        require(value.length in 2..8192 && value.length % 2 == 0 &&
            value.all { it in '0'..'9' || it in 'A'..'F' } && value.any { it != '0' }) {
            "$field must contain nonzero canonical uppercase signature bytes"
        }
    }

    private fun requireOrderedWitnesses(witnesses: List<Witness>) {
        require(witnesses.isNotEmpty() && (1 until witnesses.size).all { witnesses[it - 1].precedes(witnesses[it]) }) {
            "DA controller witnesses must be nonempty and strictly ordered by signer"
        }
    }

    /** Account authorization over the original signed ingest request and its exact byte charge. */
    data class IngestAuthorization(
        val networkId: NetworkId, val owner: String, val laneId: Long,
        val epoch: BigInteger, val sequence: BigInteger, val payloadHash: Digest32,
        val payloadBytes: BigInteger, val requestContentHash: Digest32, val signatures: List<Witness>,
    ) {
        init {
            requireCanonicalI105Address(owner, "authorization.owner")
            requireU32(BigInteger.valueOf(laneId), "authorization.lane_id")
            requireU64(epoch, "authorization.epoch")
            requireU64(sequence, "authorization.sequence")
            requireU64(payloadBytes, "authorization.payload_bytes")
            requireOrderedWitnesses(signatures)
        }

        internal fun toJsonValue(): Map<String, Any?> = linkedMapOf(
            "network_id" to networkId.literal, "owner" to owner, "lane_id" to laneId,
            "epoch" to epoch, "sequence" to sequence, "payload_hash" to payloadHash.toJsonValue(),
            "payload_bytes" to payloadBytes, "request_content_hash" to requestContentHash.toJsonValue(),
            "signatures" to signatures.map(Witness::toJsonValue),
        )
    }

    /** Exact ticket, manifest, commitment and alias approved by the producer. */
    data class PinScope(
        val networkId: NetworkId, val owner: String, val requestAuthorizationDigest: Digest32,
        val laneId: Long, val epoch: BigInteger, val sequence: BigInteger, val storageTicket: Digest32,
        val manifestHash: Digest32, val commitmentDigest: Digest32, val alias: String?,
    ) {
        init {
            requireCanonicalI105Address(owner, "scope.owner")
            requireU32(BigInteger.valueOf(laneId), "scope.lane_id")
            requireU64(epoch, "scope.epoch")
            requireU64(sequence, "scope.sequence")
            alias?.let { requirePinIntentAlias(it, "DA pin alias") }
        }

        internal fun toJsonValue(): Map<String, Any?> = linkedMapOf(
            "network_id" to networkId.literal, "owner" to owner, "lane_id" to laneId,
            "epoch" to epoch, "sequence" to sequence, "storage_ticket" to storageTicket.toJsonValue(),
            "manifest_hash" to manifestHash.toJsonValue(), "commitment_digest" to commitmentDigest.toJsonValue(),
            "request_authorization_digest" to requestAuthorizationDigest.toJsonValue(), "alias" to alias,
        )
    }

    data class PinScopeAuthorization(val scope: PinScope, val signatures: List<Witness>) {
        init { requireOrderedWitnesses(signatures) }
        internal fun toJsonValue() = mapOf("scope" to scope.toJsonValue(), "signatures" to signatures.map(Witness::toJsonValue))
    }

    data class PinIntent(
        val authorization: IngestAuthorization,
        val pinScopeAuthorization: PinScopeAuthorization,
    ) {
        private val scope get() = pinScopeAuthorization.scope
        val laneId get() = scope.laneId
        val epoch get() = scope.epoch
        val sequence get() = scope.sequence
        val storageTicket get() = scope.storageTicket
        val manifestHash get() = scope.manifestHash
        val alias get() = scope.alias

        init {
            require(authorization.networkId == scope.networkId && authorization.owner == scope.owner &&
                authorization.laneId == scope.laneId && authorization.epoch == scope.epoch &&
                authorization.sequence == scope.sequence) { "DA authorizations identify different requests" }
        }

        internal fun toJsonValue(): Map<String, Any?> = linkedMapOf(
            "lane_id" to laneId, "epoch" to epoch, "sequence" to sequence,
            "storage_ticket" to storageTicket.toJsonValue(), "manifest_hash" to manifestHash.toJsonValue(),
            "alias" to alias, "authorization" to authorization.toJsonValue(),
            "pin_scope_authorization" to pinScopeAuthorization.toJsonValue(),
        )
    }

    /** Active lane proof policy. */
    data class ProofPolicy(
        val laneId: Long,
        val dataspaceId: BigInteger,
        val alias: String,
        val proofScheme: ProofScheme,
    ) {
        init {
            requireU32(BigInteger.valueOf(laneId), "laneId")
            requireU64(dataspaceId, "dataspaceId")
            require(alias.isNotBlank() && alias == alias.trim()) {
                "DA policy alias must be exact and non-blank"
            }
        }
    }

    /** Versioned active proof-policy bundle. */
    data class ProofPolicyBundle(
        val version: Int,
        val policyHash: String,
        val policies: List<ProofPolicy>,
    ) {
        init {
            require(version == 1) { "only DA proof-policy bundle V1 is supported" }
            DaJson.requireHash(policyHash, "policy_hash")
        }
    }

    /** Location of a record in an on-chain DA bundle. */
    data class Location(
        val blockHeight: BigInteger,
        val indexInBundle: Long,
    ) {
        init {
            requireU64(blockHeight, "blockHeight")
            require(blockHeight > BigInteger.ZERO) { "DA block height must be nonzero" }
            requireU32(BigInteger.valueOf(indexInBundle), "indexInBundle")
        }

        internal fun toJsonValue(): Map<String, Any?> = linkedMapOf(
            "block_height" to blockHeight,
            "index_in_bundle" to indexInBundle,
        )
    }

    enum class MerkleDirection(val wireName: String) {
        LEFT("Left"),
        RIGHT("Right");

        internal fun toJsonValue(): Map<String, Any?> =
            linkedMapOf("direction" to wireName, "value" to null)
    }

    data class MerklePathItem(
        val sibling: String,
        val direction: MerkleDirection,
    ) {
        init {
            DaJson.requireHash(sibling, "sibling")
        }

        internal fun toJsonValue(): Map<String, Any?> = linkedMapOf(
            "sibling" to sibling,
            "direction" to direction.toJsonValue(),
        )
    }

    /** Typed commitment proof envelope. */
    class CommitmentProof(
        val commitment: CommitmentRecord,
        val location: Location,
        /** Header commitment to the V1 tree version, leaf count, and Merkle root. */
        val bundleHash: String,
        val bundleLength: Long,
        val root: String,
        path: List<MerklePathItem>,
    ) {
        val path: List<MerklePathItem> = Collections.unmodifiableList(ArrayList(path))

        init {
            DaJson.requireHash(bundleHash, "bundle_hash")
            requireU32(BigInteger.valueOf(bundleLength), "bundleLength")
            require(bundleLength > 0) { "DA proof bundle length must be nonzero" }
            DaJson.requireHash(root, "root")
            require(path.size <= 32) { "DA Merkle path exceeds the u32 bundle depth" }
            require(merklePathMatchesLocation(path, location.indexInBundle, bundleLength)) {
                "DA Merkle path shape does not match its bundle location"
            }
        }

        fun toJsonBytes(): ByteArray = DaJson.encode(
            linkedMapOf(
                "commitment" to commitment.toJsonValue(),
                "location" to location.toJsonValue(),
                "bundle_hash" to bundleHash,
                "bundle_len" to bundleLength,
                "root" to root,
                "path" to path.map { it.toJsonValue() },
            ),
        )
    }

    /** Typed pin-intent proof envelope. */
    class PinIntentProof(
        val intent: PinIntent,
        val location: Location,
        /** Header commitment to the V1 tree version, leaf count, and Merkle root. */
        val bundleHash: String,
        val bundleLength: Long,
        val root: String,
        path: List<MerklePathItem>,
    ) {
        val path: List<MerklePathItem> = Collections.unmodifiableList(ArrayList(path))

        init {
            DaJson.requireHash(bundleHash, "bundle_hash")
            requireU32(BigInteger.valueOf(bundleLength), "bundleLength")
            require(bundleLength > 0) { "DA proof bundle length must be nonzero" }
            DaJson.requireHash(root, "root")
            require(path.size <= 32) { "DA Merkle path exceeds the u32 bundle depth" }
            require(merklePathMatchesLocation(path, location.indexInBundle, bundleLength)) {
                "DA Merkle path shape does not match its bundle location"
            }
        }

        fun toJsonBytes(): ByteArray = DaJson.encode(
            linkedMapOf(
                "intent" to intent.toJsonValue(),
                "location" to location.toJsonValue(),
                "bundle_hash" to bundleHash,
                "bundle_len" to bundleLength,
                "root" to root,
                "path" to path.map { it.toJsonValue() },
            ),
        )
    }

    class CommitmentWithLocation(
        val commitment: CommitmentRecord,
        val location: Location,
    )

    class PinIntentWithLocation(
        val intent: PinIntent,
        val location: Location,
    )

    data class CommitmentListResponse(
        val policies: ProofPolicyBundle,
        val commitments: List<CommitmentWithLocation>,
        val nextCursor: CommitmentListCursor?,
    )

    data class CommitmentProofResponse(
        val policies: ProofPolicyBundle,
        val proof: CommitmentProof,
    )

    data class PinIntentListResponse(
        val intents: List<PinIntentWithLocation>,
        val nextCursor: PinIntentListCursor?,
    )

    data class VerifyResponse(val valid: Boolean, val error: String?) {
        init {
            require(valid == (error == null)) {
                "DA verify response must omit errors only for valid proofs"
            }
            if (error != null) {
                require(error.isNotEmpty()) {
                    "DA verification error must be non-empty"
                }
            }
        }
    }

    private fun requireOptionalU32(value: Long?, field: String) {
        if (value != null) requireU32(BigInteger.valueOf(value), field)
    }

    private fun requireOptionalU64(value: BigInteger?, field: String) {
        if (value != null) requireU64(value, field)
    }

    private fun requireOptionalNonZeroU64(value: BigInteger?, field: String) {
        if (value != null) {
            requireU64(value, field)
            require(value > BigInteger.ZERO) { "$field must be nonzero" }
        }
    }

    private fun requirePinIntentAlias(value: String, field: String) {
        require(value.toByteArray(StandardCharsets.UTF_8).size <= 256) {
            "$field must contain at most 256 UTF-8 bytes"
        }
    }

    internal fun requireU32(value: BigInteger, field: String) {
        require(value >= BigInteger.ZERO && value <= U32_MAX) { "$field must fit u32" }
    }

    internal fun requireU64(value: BigInteger, field: String) {
        require(value >= BigInteger.ZERO && value <= U64_MAX) { "$field must fit u64" }
    }

    private fun merklePathMatchesLocation(
        path: List<MerklePathItem>,
        initialIndex: Long,
        initialWidth: Long,
    ): Boolean {
        if (initialWidth <= 0 || initialIndex < 0 || initialIndex >= initialWidth) return false
        var index = initialIndex
        var width = initialWidth
        var pathIndex = 0
        while (width > 1) {
            val expected = when {
                index % 2L == 1L -> MerkleDirection.LEFT
                index + 1L < width -> MerkleDirection.RIGHT
                else -> null
            }
            if (expected != null) {
                if (path.getOrNull(pathIndex)?.direction != expected) return false
                pathIndex += 1
            }
            index /= 2
            width = width / 2 + width % 2
        }
        return pathIndex == path.size
    }
}

internal object DaJson {
    private val HASH_PATTERN = Regex("hash:[0-9A-F]{64}#[0-9A-F]{4}")

    fun encode(value: Any?): ByteArray {
        val out = StringBuilder()
        append(value, out)
        return out.toString().toByteArray(StandardCharsets.UTF_8)
    }

    fun parse(bytes: ByteArray, field: String): Any? {
        require(bytes.isNotEmpty()) { "$field must not be empty" }
        val text = String(bytes, StandardCharsets.UTF_8)
        return try {
            JsonParser.parse(text)
        } catch (error: RuntimeException) {
            throw IllegalArgumentException("invalid $field JSON", error)
        }
    }

    fun parsePolicyBundle(value: Any?, field: String): DaModels.ProofPolicyBundle {
        val objectValue = exactObject(
            value,
            field,
            setOf("version", "policy_hash", "policies"),
        )
        val version = u16(objectValue["version"], "$field.version")
        val policyHash = string(objectValue["policy_hash"], "$field.policy_hash")
        requireHash(policyHash, "$field.policy_hash")
        val policies = list(objectValue["policies"], "$field.policies").mapIndexed { index, raw ->
            val policy = exactObject(
                raw,
                "$field.policies[$index]",
                setOf("lane_id", "dataspace_id", "alias", "proof_scheme"),
            )
            DaModels.ProofPolicy(
                laneId = u32(policy["lane_id"], "$field.policies[$index].lane_id"),
                dataspaceId = u64(policy["dataspace_id"], "$field.policies[$index].dataspace_id"),
                alias = exactNonBlankString(policy["alias"], "$field.policies[$index].alias"),
                proofScheme = DaModels.ProofScheme.fromJson(
                    policy["proof_scheme"],
                    "$field.policies[$index].proof_scheme",
                ),
            )
        }
        return DaModels.ProofPolicyBundle(version, policyHash, Collections.unmodifiableList(policies))
    }

    fun parseCommitmentList(value: Any?, field: String): DaModels.CommitmentListResponse {
        val objectValue = exactObject(
            value,
            field,
            setOf("policies", "commitments", "next_cursor"),
        )
        val policies = parsePolicyBundle(objectValue["policies"], "$field.policies")
        val commitments = list(objectValue["commitments"], "$field.commitments")
            .mapIndexed { index, raw ->
                parseCommitmentWithLocation(raw, "$field.commitments[$index]")
            }
        return DaModels.CommitmentListResponse(
            policies,
            Collections.unmodifiableList(commitments),
            objectValue["next_cursor"]?.let {
                parseCommitmentListCursor(it, "$field.next_cursor")
            },
        )
    }

    fun parseCommitmentProofResponse(
        value: Any?,
        field: String,
    ): DaModels.CommitmentProofResponse? {
        if (value == null) return null
        val objectValue = exactObject(value, field, setOf("policies", "proof"))
        return DaModels.CommitmentProofResponse(
            parsePolicyBundle(objectValue["policies"], "$field.policies"),
            parseCommitmentProof(objectValue["proof"], "$field.proof"),
        )
    }

    fun parseCommitmentProof(value: Any?, field: String): DaModels.CommitmentProof {
        val objectValue = exactObject(
            value,
            field,
            setOf("commitment", "location", "bundle_hash", "bundle_len", "root", "path"),
        )
        return DaModels.CommitmentProof(
            parseCommitmentRecord(objectValue["commitment"], "$field.commitment"),
            parseLocation(objectValue["location"], "$field.location"),
            string(objectValue["bundle_hash"], "$field.bundle_hash"),
            u32(objectValue["bundle_len"], "$field.bundle_len"),
            string(objectValue["root"], "$field.root"),
            parsePath(objectValue["path"], "$field.path"),
        )
    }

    fun parsePinIntentList(value: Any?, field: String): DaModels.PinIntentListResponse {
        val objectValue = exactObject(value, field, setOf("intents", "next_cursor"))
        val intents = list(objectValue["intents"], "$field.intents").mapIndexed { index, raw ->
            parsePinIntentWithLocation(raw, "$field.intents[$index]")
        }
        return DaModels.PinIntentListResponse(
            Collections.unmodifiableList(intents),
            objectValue["next_cursor"]?.let {
                parsePinIntentListCursor(it, "$field.next_cursor")
            },
        )
    }

    fun parsePinIntentProof(value: Any?, field: String): DaModels.PinIntentProof? {
        if (value == null) return null
        val objectValue = exactObject(
            value,
            field,
            setOf("intent", "location", "bundle_hash", "bundle_len", "root", "path"),
        )
        return DaModels.PinIntentProof(
            parsePinIntent(objectValue["intent"], "$field.intent"),
            parseLocation(objectValue["location"], "$field.location"),
            string(objectValue["bundle_hash"], "$field.bundle_hash"),
            u32(objectValue["bundle_len"], "$field.bundle_len"),
            string(objectValue["root"], "$field.root"),
            parsePath(objectValue["path"], "$field.path"),
        )
    }

    fun parseVerifyResponse(value: Any?, field: String): DaModels.VerifyResponse {
        val objectValue = exactObject(value, field, setOf("valid", "error"))
        val valid = objectValue["valid"] as? Boolean
            ?: throw IllegalArgumentException("$field.valid must be a boolean")
        val error = objectValue["error"]?.let {
            val text = string(it, "$field.error")
            require(text.isNotEmpty()) { "$field.error must be non-empty" }
            text
        }
        return DaModels.VerifyResponse(valid, error)
    }

    private fun parseCommitmentRecord(value: Any?, field: String): DaModels.CommitmentRecord {
        val record = exactObject(value, field, setOf(
            "lane_id", "epoch", "sequence", "client_blob_id", "manifest_hash", "proof_scheme",
            "chunk_root", "proof_digest", "retention_class", "storage_ticket",
            "acknowledgement_sig",
        ))
        return DaModels.CommitmentRecord(
            laneId = u32(record["lane_id"], "$field.lane_id"),
            epoch = u64(record["epoch"], "$field.epoch"),
            sequence = u64(record["sequence"], "$field.sequence"),
            clientBlobId = DaModels.Digest32.fromJson(
                record["client_blob_id"],
                "$field.client_blob_id",
            ),
            manifestHash = DaModels.Digest32.fromJson(
                record["manifest_hash"],
                "$field.manifest_hash",
            ),
            proofScheme = DaModels.ProofScheme.fromJson(
                record["proof_scheme"],
                "$field.proof_scheme",
            ),
            chunkRoot = DaModels.Digest32.fromJson(record["chunk_root"], "$field.chunk_root"),
            proofDigest = record["proof_digest"]?.let {
                string(it, "$field.proof_digest")
            },
            retentionClass = parseRetention(
                record["retention_class"],
                "$field.retention_class",
            ),
            storageTicket = DaModels.Digest32.fromJson(
                record["storage_ticket"],
                "$field.storage_ticket",
            ),
            acknowledgementSignature = string(
                record["acknowledgement_sig"],
                "$field.acknowledgement_sig",
            ),
        )
    }

    private fun parseWitnesses(value: Any?, field: String): List<DaModels.Witness> =
        Collections.unmodifiableList(list(value, field).mapIndexed { index, entry ->
            val name = "$field[$index]"
            val item = exactObject(entry, name, setOf("signer", "signature"))
            DaModels.Witness(
                signer = string(item["signer"], "$name.signer"),
                signature = string(item["signature"], "$name.signature"),
            )
        })

    private fun parseIngestAuthorization(value: Any?, field: String): DaModels.IngestAuthorization {
        val auth = exactObject(value, field, setOf(
            "network_id", "owner", "lane_id", "epoch", "sequence", "payload_hash", "payload_bytes",
            "request_content_hash", "signatures",
        ))
        return DaModels.IngestAuthorization(
            networkId = NetworkId.parse(string(auth["network_id"], "$field.network_id")),
            owner = string(auth["owner"], "$field.owner"),
            laneId = u32(auth["lane_id"], "$field.lane_id"),
            epoch = u64(auth["epoch"], "$field.epoch"),
            sequence = u64(auth["sequence"], "$field.sequence"),
            payloadHash = DaModels.Digest32.fromJson(auth["payload_hash"], "$field.payload_hash"),
            payloadBytes = u64(auth["payload_bytes"], "$field.payload_bytes"),
            requestContentHash = DaModels.Digest32.fromJson(auth["request_content_hash"], "$field.request_content_hash"),
            signatures = parseWitnesses(auth["signatures"], "$field.signatures"),
        )
    }

    private fun parseScopeAuthorization(value: Any?, field: String): DaModels.PinScopeAuthorization {
        val auth = exactObject(value, field, setOf("scope", "signatures"))
        val name = "$field.scope"
        val scope = exactObject(auth["scope"], name, setOf(
            "network_id", "owner", "request_authorization_digest", "lane_id", "epoch",
            "sequence", "storage_ticket", "manifest_hash", "commitment_digest", "alias",
        ))
        val parsed = DaModels.PinScope(
            networkId = NetworkId.parse(string(scope["network_id"], "$name.network_id")),
            owner = string(scope["owner"], "$name.owner"),
            requestAuthorizationDigest = DaModels.Digest32.fromJson(scope["request_authorization_digest"], "$name.request_authorization_digest"),
            laneId = u32(scope["lane_id"], "$name.lane_id"),
            epoch = u64(scope["epoch"], "$name.epoch"),
            sequence = u64(scope["sequence"], "$name.sequence"),
            storageTicket = DaModels.Digest32.fromJson(scope["storage_ticket"], "$name.storage_ticket"),
            manifestHash = DaModels.Digest32.fromJson(scope["manifest_hash"], "$name.manifest_hash"),
            commitmentDigest = DaModels.Digest32.fromJson(scope["commitment_digest"], "$name.commitment_digest"),
            alias = scope["alias"]?.let { string(it, "$name.alias") },
        )
        return DaModels.PinScopeAuthorization(parsed, parseWitnesses(auth["signatures"], "$field.signatures"))
    }

    private fun parsePinIntent(value: Any?, field: String): DaModels.PinIntent {
        val row = exactObject(value, field, setOf(
            "lane_id", "epoch", "sequence", "storage_ticket", "manifest_hash", "alias", "authorization", "pin_scope_authorization",
        ))
        val intent = DaModels.PinIntent(
            parseIngestAuthorization(row["authorization"], "$field.authorization"),
            parseScopeAuthorization(row["pin_scope_authorization"], "$field.pin_scope_authorization"),
        )
        require(intent.laneId == u32(row["lane_id"], "$field.lane_id") && intent.epoch == u64(row["epoch"], "$field.epoch") &&
            intent.sequence == u64(row["sequence"], "$field.sequence") &&
            intent.storageTicket == DaModels.Digest32.fromJson(row["storage_ticket"], "$field.storage_ticket") &&
            intent.manifestHash == DaModels.Digest32.fromJson(row["manifest_hash"], "$field.manifest_hash") &&
            intent.alias == row["alias"]) { "$field differs from the authorized DA pin scope" }
        return intent
    }

    fun requireHash(value: String, field: String) {
        val canonicalShape = HASH_PATTERN.matches(value)
        val body = if (canonicalShape) value.substring(5, 69) else ""
        val checksum = if (canonicalShape) value.substring(70, 74).toInt(16) else -1
        val marked = canonicalShape && (body.substring(62, 64).toInt(16) and 1) == 1
        require(canonicalShape && marked && checksum == hashChecksum(body)) {
            "$field must be a canonical checksummed Iroha hash"
        }
    }

    fun taggedUnit(
        value: Any?,
        field: String,
        discriminator: String,
        allowed: Set<String>,
    ): String {
        val objectValue = exactObject(value, field, setOf(discriminator, "value"))
        val type = string(objectValue[discriminator], "$field.$discriminator")
        require(type in allowed) { "$field.$discriminator is unsupported" }
        require(objectValue["value"] == null) { "$field.value must be null" }
        return type
    }

    fun list(value: Any?, field: String): List<Any?> =
        value as? List<Any?> ?: throw IllegalArgumentException("$field must be an array")

    fun u8(value: Any?, field: String): Int {
        val integer = integer(value, field)
        require(integer >= BigInteger.ZERO && integer <= BigInteger.valueOf(255)) {
            "$field must fit u8"
        }
        return integer.toInt()
    }

    private fun parseCommitmentWithLocation(
        value: Any?,
        field: String,
    ): DaModels.CommitmentWithLocation {
        val objectValue = exactObject(value, field, setOf("commitment", "location"))
        return DaModels.CommitmentWithLocation(
            parseCommitmentRecord(objectValue["commitment"], "$field.commitment"),
            parseLocation(objectValue["location"], "$field.location"),
        )
    }

    private fun parsePinIntentWithLocation(
        value: Any?,
        field: String,
    ): DaModels.PinIntentWithLocation {
        val objectValue = exactObject(value, field, setOf("intent", "location"))
        return DaModels.PinIntentWithLocation(
            parsePinIntent(objectValue["intent"], "$field.intent"),
            parseLocation(objectValue["location"], "$field.location"),
        )
    }

    private fun parseListSnapshot(value: Any?, field: String): DaModels.ListSnapshot {
        val objectValue = exactObject(value, field, setOf("block_height", "block_hash"))
        return DaModels.ListSnapshot(
            u64(objectValue["block_height"], "$field.block_height"),
            objectValue["block_hash"]?.let { string(it, "$field.block_hash") },
        )
    }

    private fun parseCommitmentKey(value: Any?, field: String): DaModels.CommitmentKey {
        val objectValue = exactObject(value, field, setOf("lane_id", "epoch", "sequence"))
        return DaModels.CommitmentKey(
            u32(objectValue["lane_id"], "$field.lane_id"),
            u64(objectValue["epoch"], "$field.epoch"),
            u64(objectValue["sequence"], "$field.sequence"),
        )
    }

    private fun parseCommitmentListCursor(
        value: Any?,
        field: String,
    ): DaModels.CommitmentListCursor {
        val objectValue = exactObject(value, field, setOf("snapshot", "after"))
        return DaModels.CommitmentListCursor(
            parseListSnapshot(objectValue["snapshot"], "$field.snapshot"),
            parseCommitmentKey(objectValue["after"], "$field.after"),
        )
    }

    private fun parsePinIntentListCursor(
        value: Any?,
        field: String,
    ): DaModels.PinIntentListCursor {
        val objectValue = exactObject(value, field, setOf("snapshot", "after"))
        return DaModels.PinIntentListCursor(
            parseListSnapshot(objectValue["snapshot"], "$field.snapshot"),
            parseLocation(objectValue["after"], "$field.after"),
        )
    }

    private fun parseLocation(value: Any?, field: String): DaModels.Location {
        val objectValue = exactObject(value, field, setOf("block_height", "index_in_bundle"))
        return DaModels.Location(
            u64(objectValue["block_height"], "$field.block_height"),
            u32(objectValue["index_in_bundle"], "$field.index_in_bundle"),
        )
    }

    private fun parsePath(value: Any?, field: String): List<DaModels.MerklePathItem> {
        val path = list(value, field)
        require(path.size <= 32) { "$field exceeds the u32 bundle depth" }
        return Collections.unmodifiableList(path.mapIndexed { index, raw ->
            val item = exactObject(
                raw,
                "$field[$index]",
                setOf("sibling", "direction"),
            )
            val sibling = string(item["sibling"], "$field[$index].sibling")
            requireHash(sibling, "$field[$index].sibling")
            val direction = taggedUnit(
                item["direction"],
                "$field[$index].direction",
                "direction",
                setOf("Left", "Right"),
            )
            DaModels.MerklePathItem(
                sibling,
                if (direction == "Left") {
                    DaModels.MerkleDirection.LEFT
                } else {
                    DaModels.MerkleDirection.RIGHT
                },
            )
        })
    }

    private fun parseRetention(value: Any?, field: String): DaModels.RetentionPolicy {
        val policy = exactObject(
            value,
            field,
            setOf(
                "hot_retention_secs", "cold_retention_secs", "required_replicas",
                "storage_class", "governance_tag",
            ),
        )
        val tag = list(policy["governance_tag"], "$field.governance_tag")
        require(tag.size == 1) { "$field.governance_tag must contain one item" }
        return DaModels.RetentionPolicy(
            hotRetentionSeconds = u64(
                policy["hot_retention_secs"],
                "$field.hot_retention_secs",
            ),
            coldRetentionSeconds = u64(
                policy["cold_retention_secs"],
                "$field.cold_retention_secs",
            ),
            requiredReplicas = u16(
                policy["required_replicas"],
                "$field.required_replicas",
            ),
            storageClass = DaModels.StorageClass.fromJson(
                policy["storage_class"],
                "$field.storage_class",
            ),
            governanceTag = string(tag[0], "$field.governance_tag[0]"),
        )
    }

    private fun exactObject(
        value: Any?,
        field: String,
        keys: Set<String>,
    ): Map<String, Any?> {
        val objectValue = objectMap(value, field)
        require(objectValue.keys == keys) { "$field contains unknown or missing fields" }
        return objectValue
    }

    @Suppress("UNCHECKED_CAST")
    private fun objectMap(value: Any?, field: String): Map<String, Any?> {
        val raw = value as? Map<*, *> ?: throw IllegalArgumentException("$field must be an object")
        require(raw.keys.all { it is String }) { "$field object keys must be strings" }
        return raw as Map<String, Any?>
    }

    private fun string(value: Any?, field: String): String =
        value as? String ?: throw IllegalArgumentException("$field must be a string")

    private fun exactNonBlankString(value: Any?, field: String): String {
        val text = string(value, field)
        require(text.isNotBlank() && text == text.trim()) { "$field must be exact and non-blank" }
        return text
    }

    private fun integer(value: Any?, field: String): BigInteger = when (value) {
        is BigInteger -> value
        is Byte -> BigInteger.valueOf(value.toLong())
        is Short -> BigInteger.valueOf(value.toLong())
        is Int -> BigInteger.valueOf(value.toLong())
        is Long -> BigInteger.valueOf(value)
        else -> throw IllegalArgumentException("$field must be an integer")
    }

    private fun u16(value: Any?, field: String): Int {
        val integer = integer(value, field)
        require(integer >= BigInteger.ZERO && integer <= BigInteger.valueOf(65535)) {
            "$field must fit u16"
        }
        return integer.toInt()
    }

    private fun u32(value: Any?, field: String): Long {
        val integer = integer(value, field)
        DaModels.requireU32(integer, field)
        return integer.toLong()
    }

    private fun u64(value: Any?, field: String): BigInteger {
        val integer = integer(value, field)
        DaModels.requireU64(integer, field)
        return integer
    }

    private fun append(value: Any?, out: StringBuilder) {
        when (value) {
            null -> out.append("null")
            is String -> appendString(value, out)
            is Boolean -> out.append(if (value) "true" else "false")
            is BigInteger -> out.append(value.toString())
            is Byte, is Short, is Int, is Long -> out.append(value.toString())
            is List<*> -> {
                out.append('[')
                value.forEachIndexed { index, item ->
                    if (index != 0) out.append(',')
                    append(item, out)
                }
                out.append(']')
            }
            is Map<*, *> -> {
                val keys = value.keys.map {
                    it as? String ?: throw IllegalArgumentException("JSON object key must be a string")
                }.sorted()
                out.append('{')
                keys.forEachIndexed { index, key ->
                    if (index != 0) out.append(',')
                    appendString(key, out)
                    out.append(':')
                    append(value[key], out)
                }
                out.append('}')
            }
            else -> throw IllegalArgumentException("unsupported JSON value ${value.javaClass.name}")
        }
    }

    private fun appendString(value: String, out: StringBuilder) {
        out.append('"')
        value.forEach { character ->
            when (character) {
                '"' -> out.append("\\\"")
                '\\' -> out.append("\\\\")
                '\b' -> out.append("\\b")
                '\u000C' -> out.append("\\f")
                '\n' -> out.append("\\n")
                '\r' -> out.append("\\r")
                '\t' -> out.append("\\t")
                else -> {
                    if (character.code < 0x20) {
                        out.append("\\u").append(character.code.toString(16).padStart(4, '0'))
                    } else {
                        out.append(character)
                    }
                }
            }
        }
        out.append('"')
    }

    private fun hashChecksum(body: String): Int {
        var crc = 0xffff
        for (byte in "hash:$body".toByteArray(StandardCharsets.US_ASCII)) {
            crc = crc xor ((byte.toInt() and 0xff) shl 8)
            repeat(8) {
                crc = if ((crc and 0x8000) != 0) {
                    ((crc shl 1) xor 0x1021) and 0xffff
                } else {
                    (crc shl 1) and 0xffff
                }
            }
        }
        return crc
    }
}
