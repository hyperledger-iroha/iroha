// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.privacy

import java.math.BigInteger
import java.security.MessageDigest
import java.util.Collections
import java.util.LinkedHashMap
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.core.model.NetworkId

/** Closed selector union for authenticated finalized privacy-state query IDs 97 through 104. */
sealed interface PrivacyFinalizedStateRequestV1 {
    val queryId: Int
    val protocolIndex: Int

    /** Exact native selector binding. The returned array is always a defensive copy. */
    fun requestBinding(): ByteArray
}

/** Finalized provenance selector for one consumed ZK-ACE replay nullifier (ID 97). */
class PrivacyZkAceReplayNullifierRequestV1(
    policyId: ByteArray,
    replayNullifier: ByteArray,
) : PrivacyFinalizedStateRequestV1 {
    private val policy = fixed32V1(policyId, "policyId")
    private val replay = fixed32V1(replayNullifier, "replayNullifier")
    val policyId: ByteArray get() = policy.copyOf()
    val replayNullifier: ByteArray get() = replay.copyOf()
    override val queryId: Int = 97
    override val protocolIndex: Int = 0
    override fun requestBinding(): ByteArray = concat32V1(policy, replay)
}

/** Finalized FCMP++, private-IVM, or PQ-MASP pool selector (ID 98). */
class PrivacyProofManagedPoolStateRequestV1(
    val protocolId: PrivacyProtocolIdV1,
    poolId: ByteArray,
) : PrivacyFinalizedStateRequestV1 {
    private val pool = fixed32V1(poolId, "poolId")
    val poolId: ByteArray get() = pool.copyOf()
    override val queryId: Int = 98
    override val protocolIndex: Int = proofManagedProtocolIndexV1(protocolId)
    override fun requestBinding(): ByteArray = pool.copyOf()
}

/** Finalized governed Orchard pool selector (ID 99). */
class PrivacyOrchardPoolStateRequestV1(poolId: ByteArray) : PrivacyFinalizedStateRequestV1 {
    private val pool = fixed32V1(poolId, "poolId")
    val poolId: ByteArray get() = pool.copyOf()
    override val queryId: Int = 99
    override val protocolIndex: Int = 0
    override fun requestBinding(): ByteArray = pool.copyOf()
}

/** Finalized consumed Orchard-nullifier selector (ID 100). */
class PrivacyOrchardNullifierRequestV1(
    poolId: ByteArray,
    nullifier: ByteArray,
) : PrivacyFinalizedStateRequestV1 {
    private val pool = fixed32V1(poolId, "poolId")
    private val consumed = fixed32V1(nullifier, "nullifier")
    val poolId: ByteArray get() = pool.copyOf()
    val nullifier: ByteArray get() = consumed.copyOf()
    override val queryId: Int = 100
    override val protocolIndex: Int = 0
    override fun requestBinding(): ByteArray = concat32V1(pool, consumed)
}

/** Finalized Anonymous PGC pool selector (ID 101). */
class PrivacyAnonymousPgcPoolStateRequestV1(poolId: ByteArray) : PrivacyFinalizedStateRequestV1 {
    private val pool = fixed32V1(poolId, "poolId")
    val poolId: ByteArray get() = pool.copyOf()
    override val queryId: Int = 101
    override val protocolIndex: Int = 0
    override fun requestBinding(): ByteArray = pool.copyOf()
}

/** Finalized ZK-AMS credential-batch admission selector (ID 102). */
class PrivacyZkAmsAdmissionRequestV1(
    issuerId: ByteArray,
    registryId: ByteArray,
    policyId: ByteArray,
    phcHash: ByteArray,
) : PrivacyFinalizedStateRequestV1 {
    private val issuer = fixed32V1(issuerId, "issuerId")
    private val registry = fixed32V1(registryId, "registryId")
    private val policy = fixed32V1(policyId, "policyId")
    private val phc = fixed32V1(phcHash, "phcHash")
    val issuerId: ByteArray get() = issuer.copyOf()
    val registryId: ByteArray get() = registry.copyOf()
    val policyId: ByteArray get() = policy.copyOf()
    val phcHash: ByteArray get() = phc.copyOf()
    override val queryId: Int = 102
    override val protocolIndex: Int = 0
    override fun requestBinding(): ByteArray = concat32V1(issuer, registry, policy, phc)
}

/** Finalized anonymous ZK-AMS provisioning selector (ID 103). */
class PrivacyZkAmsProvisionRequestV1(
    issuerId: ByteArray,
    registryId: ByteArray,
    policyId: ByteArray,
    keyImage: ByteArray,
) : PrivacyFinalizedStateRequestV1 {
    private val issuer = fixed32V1(issuerId, "issuerId")
    private val registry = fixed32V1(registryId, "registryId")
    private val policy = fixed32V1(policyId, "policyId")
    private val image = fixed32V1(keyImage, "keyImage")
    val issuerId: ByteArray get() = issuer.copyOf()
    val registryId: ByteArray get() = registry.copyOf()
    val policyId: ByteArray get() = policy.copyOf()
    val keyImage: ByteArray get() = image.copyOf()
    override val queryId: Int = 103
    override val protocolIndex: Int = 0
    override fun requestBinding(): ByteArray = concat32V1(issuer, registry, policy, image)
}

/** Finalized consumed ZK-X509 certificate-nullifier selector (ID 104). */
class PrivacyZkX509CertificateNullifierRequestV1(
    trustAnchorId: ByteArray,
    policyId: ByteArray,
    nullifier: ByteArray,
) : PrivacyFinalizedStateRequestV1 {
    private val trustAnchor = fixed32V1(trustAnchorId, "trustAnchorId")
    private val policy = fixed32V1(policyId, "policyId")
    private val consumed = fixed32V1(nullifier, "nullifier")
    val trustAnchorId: ByteArray get() = trustAnchor.copyOf()
    val policyId: ByteArray get() = policy.copyOf()
    val nullifier: ByteArray get() = consumed.copyOf()
    override val queryId: Int = 104
    override val protocolIndex: Int = 0
    override fun requestBinding(): ByteArray = concat32V1(trustAnchor, policy, consumed)
}

/**
 * Native-verified immutable finalized state.
 *
 * Native code has already decoded canonical Norito, selected the exact response variant, called
 * that view's validation routine, and checked its NetworkId and selector against the signed
 * request. This managed view retains the complete projected field inventory without exposing a
 * mutable map or byte array.
 */
sealed class PrivacyFinalizedStateViewV1 protected constructor(
    val queryId: Int,
    val networkId: NetworkId,
    val finalizedHeight: BigInteger,
    finalizedBlockHash: ByteArray,
    projection: Map<String, Any?>,
) {
    private val blockHash = finalizedBlockHash.copyOf()
    private val fields = freezeObjectV1(projection)

    val finalizedBlockHash: ByteArray get() = blockHash.copyOf()

    /** Return a defensive, deeply immutable copy of the complete native-validated projection. */
    fun projectionFields(): Map<String, Any?> = freezeObjectV1(fields)

    /** Return one defensive projection value, or null when the native field itself is null. */
    fun projectionField(name: String): Any? {
        require(name in fields) { "unknown finalized privacy-state projection field" }
        return freezeValueV1(fields[name])
    }

    /** Read one projected canonical unsigned integer string without truncating the u64 domain. */
    fun unsignedField(name: String): BigInteger = unsignedV1(fields[name], name, allowZero = true)

    /** Read one projected native fixed32 byte-array field through a defensive copy. */
    fun fixed32Field(name: String): ByteArray = fixed32ProjectionV1(fields[name], name)

    /** Read one projected string field. */
    fun stringField(name: String): String = stringV1(fields[name], name)
}

class PrivacyZkAceReplayNullifierProvenanceV1 internal constructor(
    networkId: NetworkId,
    finalizedHeight: BigInteger,
    finalizedBlockHash: ByteArray,
    projection: Map<String, Any?>,
    policyId: ByteArray,
    replayNullifier: ByteArray,
) : PrivacyFinalizedStateViewV1(97, networkId, finalizedHeight, finalizedBlockHash, projection) {
    private val policy = policyId.copyOf()
    private val replay = replayNullifier.copyOf()
    val policyId: ByteArray get() = policy.copyOf()
    val replayNullifier: ByteArray get() = replay.copyOf()
}

class PrivacyProofManagedPoolStateViewV1 internal constructor(
    networkId: NetworkId,
    finalizedHeight: BigInteger,
    finalizedBlockHash: ByteArray,
    projection: Map<String, Any?>,
    val protocolId: PrivacyProtocolIdV1,
    poolId: ByteArray,
) : PrivacyFinalizedStateViewV1(98, networkId, finalizedHeight, finalizedBlockHash, projection) {
    private val pool = poolId.copyOf()
    val poolId: ByteArray get() = pool.copyOf()
}

class PrivacyOrchardPoolStateViewV1 internal constructor(
    networkId: NetworkId,
    finalizedHeight: BigInteger,
    finalizedBlockHash: ByteArray,
    projection: Map<String, Any?>,
    poolId: ByteArray,
) : PrivacyFinalizedStateViewV1(99, networkId, finalizedHeight, finalizedBlockHash, projection) {
    private val pool = poolId.copyOf()
    val poolId: ByteArray get() = pool.copyOf()
}

class PrivacyOrchardNullifierProvenanceV1 internal constructor(
    networkId: NetworkId,
    finalizedHeight: BigInteger,
    finalizedBlockHash: ByteArray,
    projection: Map<String, Any?>,
    poolId: ByteArray,
    nullifier: ByteArray,
) : PrivacyFinalizedStateViewV1(100, networkId, finalizedHeight, finalizedBlockHash, projection) {
    private val pool = poolId.copyOf()
    private val consumed = nullifier.copyOf()
    val poolId: ByteArray get() = pool.copyOf()
    val nullifier: ByteArray get() = consumed.copyOf()
}

class PrivacyAnonymousPgcPoolStateViewV1 internal constructor(
    networkId: NetworkId,
    finalizedHeight: BigInteger,
    finalizedBlockHash: ByteArray,
    projection: Map<String, Any?>,
    poolId: ByteArray,
) : PrivacyFinalizedStateViewV1(101, networkId, finalizedHeight, finalizedBlockHash, projection) {
    private val pool = poolId.copyOf()
    val poolId: ByteArray get() = pool.copyOf()
}

class PrivacyZkAmsAdmissionViewV1 internal constructor(
    networkId: NetworkId,
    finalizedHeight: BigInteger,
    finalizedBlockHash: ByteArray,
    projection: Map<String, Any?>,
    issuerId: ByteArray,
    registryId: ByteArray,
    policyId: ByteArray,
    phcHash: ByteArray,
) : PrivacyFinalizedStateViewV1(102, networkId, finalizedHeight, finalizedBlockHash, projection) {
    private val issuer = issuerId.copyOf()
    private val registry = registryId.copyOf()
    private val policy = policyId.copyOf()
    private val phc = phcHash.copyOf()
    val issuerId: ByteArray get() = issuer.copyOf()
    val registryId: ByteArray get() = registry.copyOf()
    val policyId: ByteArray get() = policy.copyOf()
    val phcHash: ByteArray get() = phc.copyOf()
}

class PrivacyZkAmsProvisionViewV1 internal constructor(
    networkId: NetworkId,
    finalizedHeight: BigInteger,
    finalizedBlockHash: ByteArray,
    projection: Map<String, Any?>,
    issuerId: ByteArray,
    registryId: ByteArray,
    policyId: ByteArray,
    keyImage: ByteArray,
) : PrivacyFinalizedStateViewV1(103, networkId, finalizedHeight, finalizedBlockHash, projection) {
    private val issuer = issuerId.copyOf()
    private val registry = registryId.copyOf()
    private val policy = policyId.copyOf()
    private val image = keyImage.copyOf()
    val issuerId: ByteArray get() = issuer.copyOf()
    val registryId: ByteArray get() = registry.copyOf()
    val policyId: ByteArray get() = policy.copyOf()
    val keyImage: ByteArray get() = image.copyOf()
}

class PrivacyZkX509CertificateNullifierProvenanceV1 internal constructor(
    networkId: NetworkId,
    finalizedHeight: BigInteger,
    finalizedBlockHash: ByteArray,
    projection: Map<String, Any?>,
    trustAnchorId: ByteArray,
    policyId: ByteArray,
    nullifier: ByteArray,
) : PrivacyFinalizedStateViewV1(104, networkId, finalizedHeight, finalizedBlockHash, projection) {
    private val trustAnchor = trustAnchorId.copyOf()
    private val policy = policyId.copyOf()
    private val consumed = nullifier.copyOf()
    val trustAnchorId: ByteArray get() = trustAnchor.copyOf()
    val policyId: ByteArray get() = policy.copyOf()
    val nullifier: ByteArray get() = consumed.copyOf()
}

/** Strict managed projection parser used only after native canonical response verification. */
object PrivacyFinalizedStateProjectionV1 {
    const val MAX_PROJECTION_BYTES: Int = 256 * 1024

    @JvmStatic
    fun parse(
        projectionJson: ByteArray,
        request: PrivacyFinalizedStateRequestV1,
        expectedNetworkId: NetworkId,
    ): PrivacyFinalizedStateViewV1 {
        require(projectionJson.isNotEmpty() && projectionJson.size <= MAX_PROJECTION_BYTES) {
            "finalized privacy-state projection violates its closed byte bound"
        }
        val text = projectionJson.toString(Charsets.UTF_8)
        require(text.toByteArray(Charsets.UTF_8).contentEquals(projectionJson)) {
            "finalized privacy-state projection is not exact UTF-8"
        }
        val root = JsonParser.parse(text) as? Map<*, *>
            ?: throw IllegalStateException("finalized privacy-state projection must be an object")
        val fields = exactStringMapV1(root, "finalized privacy-state projection")
        require(fields.keys == expectedFieldsV1(request.queryId)) {
            "finalized privacy-state projection has an unexpected field inventory"
        }
        val networkId = NetworkId.parse(stringV1(fields["network_id"], "network_id"))
        require(networkId == expectedNetworkId) {
            "finalized privacy-state projection differs from its signed NetworkId"
        }
        val finalizedHeight = unsignedV1(fields["finalized_height"], "finalized_height", false)
        val finalizedBlockHash = hashLiteralV1(fields["finalized_block_hash"], "finalized_block_hash")
        val binding = request.requestBinding()
        when (request) {
            is PrivacyZkAceReplayNullifierRequestV1 -> {
                requireBindingV1(binding, fields, "policy_id", "replay_nullifier")
                return PrivacyZkAceReplayNullifierProvenanceV1(
                    networkId, finalizedHeight, finalizedBlockHash, fields,
                    request.policyId, request.replayNullifier,
                )
            }
            is PrivacyProofManagedPoolStateRequestV1 -> {
                requireBindingV1(binding, fields, "pool_id")
                requireProofManagedProtocolV1(fields["protocol_id"], request.protocolId)
                return PrivacyProofManagedPoolStateViewV1(
                    networkId, finalizedHeight, finalizedBlockHash, fields,
                    request.protocolId, request.poolId,
                )
            }
            is PrivacyOrchardPoolStateRequestV1 -> {
                requireBindingV1(binding, fields, "pool_id")
                return PrivacyOrchardPoolStateViewV1(
                    networkId, finalizedHeight, finalizedBlockHash, fields, request.poolId,
                )
            }
            is PrivacyOrchardNullifierRequestV1 -> {
                requireBindingV1(binding, fields, "pool_id", "nullifier")
                return PrivacyOrchardNullifierProvenanceV1(
                    networkId, finalizedHeight, finalizedBlockHash, fields,
                    request.poolId, request.nullifier,
                )
            }
            is PrivacyAnonymousPgcPoolStateRequestV1 -> {
                requireBindingV1(binding, fields, "pool_id")
                return PrivacyAnonymousPgcPoolStateViewV1(
                    networkId, finalizedHeight, finalizedBlockHash, fields, request.poolId,
                )
            }
            is PrivacyZkAmsAdmissionRequestV1 -> {
                requireBindingV1(
                    binding, fields, "issuer_id", "registry_id", "policy_id", "phc_hash",
                )
                return PrivacyZkAmsAdmissionViewV1(
                    networkId, finalizedHeight, finalizedBlockHash, fields,
                    request.issuerId, request.registryId, request.policyId, request.phcHash,
                )
            }
            is PrivacyZkAmsProvisionRequestV1 -> {
                requireBindingV1(
                    binding, fields, "issuer_id", "registry_id", "policy_id", "key_image",
                )
                return PrivacyZkAmsProvisionViewV1(
                    networkId, finalizedHeight, finalizedBlockHash, fields,
                    request.issuerId, request.registryId, request.policyId, request.keyImage,
                )
            }
            is PrivacyZkX509CertificateNullifierRequestV1 -> {
                requireBindingV1(binding, fields, "trust_anchor_id", "policy_id", "nullifier")
                return PrivacyZkX509CertificateNullifierProvenanceV1(
                    networkId, finalizedHeight, finalizedBlockHash, fields,
                    request.trustAnchorId, request.policyId, request.nullifier,
                )
            }
        }
    }
}

private fun fixed32V1(value: ByteArray, field: String): ByteArray {
    require(value.size == 32 && value.any { it != 0.toByte() }) {
        "$field must contain exactly 32 nonzero bytes"
    }
    return value.copyOf()
}

private fun concat32V1(vararg chunks: ByteArray): ByteArray =
    ByteArray(chunks.size * 32).also { output ->
        chunks.forEachIndexed { index, chunk ->
            fixed32V1(chunk, "selector[$index]").copyInto(output, index * 32)
        }
    }

private fun proofManagedProtocolIndexV1(protocolId: PrivacyProtocolIdV1): Int = when (protocolId) {
    PrivacyProtocolIdV1.MONERO_FCMP_PLUS_PLUS_V1 -> 0
    PrivacyProtocolIdV1.IROHA_IVM_PRIVATE_NOTE_STARK_V1 -> 1
    PrivacyProtocolIdV1.PQ_MASP_STARK_V0 -> 2
    else -> throw IllegalArgumentException(
        "proof-managed state supports only FCMP++, private-IVM, or PQ-MASP",
    )
}

private fun expectedFieldsV1(queryId: Int): Set<String> = when (queryId) {
    97 -> setOf(
        "network_id", "policy_id", "replay_nullifier", "policy_record_digest",
        "statement_digest", "admitted_at_height", "action_index", "finalized_height",
        "finalized_block_hash",
    )
    98 -> setOf(
        "network_id", "protocol_id", "pool_id", "asset_definition_id", "root_role",
        "bootstrap_digest", "initial_root", "current_epoch", "current_root", "output_count",
        "bootstrap_admitted_at_height", "latest_transition", "finalized_height",
        "finalized_block_hash",
    )
    99 -> setOf(
        "network_id", "pool_id", "asset_definition_id", "public_balance_scope",
        "reserve_account", "bootstrap_digest", "current_epoch", "current_root", "tree_size",
        "latest_transition", "finalized_height", "finalized_block_hash",
    )
    100 -> setOf(
        "network_id", "pool_id", "nullifier", "bootstrap_digest", "statement_digest",
        "admitted_at_height", "action_index", "finalized_height", "finalized_block_hash",
    )
    101 -> setOf(
        "network_id", "pool_id", "total_supply", "bootstrap_root", "bootstrap_digest",
        "bootstrap_proof_digest", "current_epoch", "current_root", "account_count",
        "current_state_admitted_at_height", "latest_transition", "finalized_height",
        "finalized_block_hash",
    )
    102 -> setOf(
        "network_id", "issuer_id", "registry_id", "policy_id", "phc_hash",
        "seed_public_key", "bootstrap_digest", "issuer_policy_record_digest", "policy_digest",
        "registry_record_digest", "parent_epoch", "parent_root", "anchor_index", "batch_size",
        "successor_epoch", "successor_root", "statement_digest", "admitted_at_height",
        "action_index", "finalized_height", "finalized_block_hash",
    )
    103 -> setOf(
        "network_id", "issuer_id", "registry_id", "policy_id", "key_image", "account_id",
        "bootstrap_digest", "issuer_policy_record_digest", "policy_digest",
        "registry_record_digest", "registry_epoch", "registry_root", "statement_digest",
        "admitted_at_height", "action_index", "finalized_height", "finalized_block_hash",
    )
    104 -> setOf(
        "network_id", "trust_anchor_id", "policy_id", "nullifier",
        "trust_anchor_record_digest", "trust_anchor_record_epoch",
        "certificate_policy_record_digest", "certificate_policy_record_epoch",
        "crl_record_digest", "crl_record_epoch", "statement_digest", "admitted_at_height",
        "action_index", "finalized_height", "finalized_block_hash",
    )
    else -> throw IllegalArgumentException("privacy state-query ID is outside 97 through 104")
}

private fun requireBindingV1(
    expected: ByteArray,
    fields: Map<String, Any?>,
    vararg names: String,
) {
    val actual = concat32V1(*names.map { fixed32ProjectionV1(fields[it], it) }.toTypedArray())
    require(MessageDigest.isEqual(expected, actual)) {
        "finalized privacy-state projection differs from its signed selector"
    }
}

private fun requireProofManagedProtocolV1(value: Any?, expected: PrivacyProtocolIdV1) {
    val tagged = exactStringMapV1(
        value as? Map<*, *> ?: throw IllegalStateException("protocol_id must be an object"),
        "protocol_id",
    )
    require(tagged.keys == setOf("protocol", "value") && tagged["value"] == null) {
        "protocol_id must be an exact unit tagged value"
    }
    require(stringV1(tagged["protocol"], "protocol_id.protocol") == expected.canonicalLabel) {
        "proof-managed protocol differs from its signed selector"
    }
}

private fun fixed32ProjectionV1(value: Any?, field: String): ByteArray {
    val values = value as? List<*>
        ?: throw IllegalStateException("$field must be a fixed32 byte array")
    require(values.size == 32) { "$field must contain exactly 32 bytes" }
    val bytes = ByteArray(32)
    values.forEachIndexed { index, item ->
        val number = item as? Number
            ?: throw IllegalStateException("$field byte must be an integer")
        val integer = when (number) {
            is Byte, is Short, is Int, is Long -> number.toLong()
            is BigInteger -> number.longValueExact()
            else -> throw IllegalStateException("$field byte must be an integer")
        }
        require(integer in 0..255) { "$field byte is outside u8" }
        bytes[index] = integer.toByte()
    }
    require(bytes.any { it != 0.toByte() }) { "$field must not be all zero" }
    return bytes
}

private fun unsignedV1(value: Any?, field: String, allowZero: Boolean): BigInteger {
    val text = stringV1(value, field)
    require(
        text.isNotEmpty() &&
            (text == "0" || text.first() in '1'..'9') &&
            text.all { it in '0'..'9' },
    ) { "$field must be a canonical unsigned decimal string" }
    val parsed = text.toBigInteger()
    require(parsed.signum() >= 0 && parsed.bitLength() <= 64 && (allowZero || parsed.signum() > 0)) {
        "$field must fit the required u64 range"
    }
    return parsed
}

private fun hashLiteralV1(value: Any?, field: String): ByteArray {
    val literal = stringV1(value, field)
    require(literal.matches(Regex("hash:[0-9A-F]{64}#[0-9A-F]{4}"))) {
        "$field must be an exact canonical nonzero hash literal"
    }
    val body = literal.substring(5, 69)
    val suppliedChecksum = literal.substring(70).toInt(16)
    require(crc16V1("hash:$body".toByteArray(Charsets.US_ASCII)) == suppliedChecksum) {
        "$field has an invalid canonical hash checksum"
    }
    val bytes = ByteArray(32) { index ->
        body.substring(index * 2, index * 2 + 2).toInt(16).toByte()
    }
    require(bytes.any { it != 0.toByte() }) { "$field must not be all zero" }
    return bytes
}

private fun crc16V1(value: ByteArray): Int {
    var crc = 0xffff
    value.forEach { item ->
        crc = crc xor ((item.toInt() and 0xff) shl 8)
        repeat(8) {
            crc = if ((crc and 0x8000) != 0) {
                ((crc shl 1) xor 0x1021) and 0xffff
            } else {
                (crc shl 1) and 0xffff
            }
        }
    }
    return crc and 0xffff
}

private fun stringV1(value: Any?, field: String): String =
    value as? String ?: throw IllegalStateException("$field must be a string")

private fun exactStringMapV1(value: Map<*, *>, field: String): Map<String, Any?> {
    val output = LinkedHashMap<String, Any?>(value.size)
    value.forEach { (key, child) ->
        require(key is String && output.put(key, child) == null) {
            "$field must contain unique string keys"
        }
    }
    return output
}

private fun freezeObjectV1(value: Map<String, Any?>): Map<String, Any?> {
    val output = LinkedHashMap<String, Any?>(value.size)
    value.forEach { (key, child) -> output[key] = freezeValueV1(child) }
    return Collections.unmodifiableMap(output)
}

private fun freezeValueV1(value: Any?): Any? = when (value) {
    is Map<*, *> -> freezeObjectV1(exactStringMapV1(value, "projection object"))
    is List<*> -> Collections.unmodifiableList(value.map(::freezeValueV1))
    is ByteArray -> value.copyOf()
    null, is String, is Boolean, is Number -> value
    else -> throw IllegalStateException("projection contains an unsupported managed value")
}
