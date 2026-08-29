package org.hyperledger.iroha.sdk.sccp

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.security.MessageDigest
import org.hyperledger.iroha.sdk.core.model.instructions.TransferWirePayloadEncoder

/** Closed replay-boundary tags shared by SORA and destination contracts. */
enum class SccpReplayBoundaryV1(val tag: Int) {
    SORA_OUTBOUND_LOCK(0x01),
    SORA_INBOUND_RELEASE(0x02),
    EVM_SOURCE_BURN(0x10),
    EVM_DESTINATION_MINT(0x11),
    TRON_SOURCE_BURN(0x20),
    TRON_DESTINATION_MINT(0x21),
    TON_BRIDGE_INBOUND_MINT(0x30),
    TON_BRIDGE_OUTBOUND_BURN(0x31),
    TON_MASTER_MINT(0x32),
    TON_MASTER_BURN(0x33),
    TON_WALLET_MINT_CREDIT(0x34),
    TON_WALLET_BURN_DEBIT(0x35),
    TON_WALLET_REFUND_DEBIT(0x36),
    TON_WALLET_REFUND_CREDIT(0x37),
}

/** Canonical contract identity committed by one replay domain. */
class SccpReplayActorV1 private constructor(internal val kind: Int, bytes: ByteArray) {
    internal val bytes = bytes.copyOf()

    companion object {
        @JvmStatic fun route(): SccpReplayActorV1 = SccpReplayActorV1(0, ByteArray(0))

        @JvmStatic fun evm(address: ByteArray): SccpReplayActorV1 =
            SccpReplayActorV1(1, requireExact(address, 20, "EVM replay actor"))

        @JvmStatic fun tron(address: ByteArray): SccpReplayActorV1 =
            SccpReplayActorV1(2, requireExact(address, 20, "TRON replay actor"))

        @JvmStatic fun ton(workchain: Int, account: ByteArray): SccpReplayActorV1 =
            SccpReplayActorV1(
                3,
                signedI32Be(workchain) + requireExact(account, 32, "TON replay actor"),
            )
    }
}

/** Canonical economic principal committed by an occupied replay leaf. */
class SccpReplayPrincipalV1 private constructor(internal val kind: Int, bytes: ByteArray) {
    internal val bytes = bytes.copyOf()

    companion object {
        /** Construct from the exact canonical Norito `AccountId` bytes. */
        @JvmStatic fun soraAccount(canonicalAccountId: ByteArray): SccpReplayPrincipalV1 =
            SccpReplayPrincipalV1(0, canonicalSoraAccountId(canonicalAccountId))

        @JvmStatic fun evm(address: ByteArray): SccpReplayPrincipalV1 =
            SccpReplayPrincipalV1(1, requireExact(address, 20, "EVM replay principal"))

        @JvmStatic fun tron(address: ByteArray): SccpReplayPrincipalV1 =
            SccpReplayPrincipalV1(2, requireExact(address, 20, "TRON replay principal"))

        @JvmStatic fun ton(workchain: Int, account: ByteArray): SccpReplayPrincipalV1 =
            SccpReplayPrincipalV1(
                3,
                signedI32Be(workchain) + requireExact(account, 32, "TON replay principal"),
            )
    }
}

/** Canonically compressed leaf-up sparse-Merkle witness. */
class SccpSparseMerkleWitnessV1(
    expectedShardRoot: ByteArray,
    priorRecordDigest: ByteArray,
    siblingBitmap: ByteArray,
    siblings: List<ByteArray>,
) {
    internal val expectedShardRoot = requireExact(expectedShardRoot, 32, "expected shard root")
    internal val priorRecordDigest = requireExact(
        priorRecordDigest,
        32,
        "prior record digest",
        nonzero = false,
    )
    internal val siblingBitmap = requireExact(
        siblingBitmap,
        32,
        "sibling bitmap",
        nonzero = false,
    )
    internal val siblings = siblings.mapIndexed { index, sibling ->
        requireExact(sibling, 32, "sibling[$index]")
    }
}

/** Result of reconstructing one shard path. */
class SccpReplayWitnessRootV1 internal constructor(
    root: ByteArray,
    expectedRoot: ByteArray,
    val shard: Int,
) {
    private val reconstructed = root.copyOf()
    private val expected = expectedRoot.copyOf()
    val matchesExpectedRoot: Boolean get() = reconstructed.contentEquals(expected)
    fun root(): ByteArray = reconstructed.copyOf()
    fun expectedRoot(): ByteArray = expected.copyOf()
}

/** SHA-256 sparse-Merkle replay hashing shared with Rust and destination runtimes. */
object SccpReplayV1 {
    const val DEPTH = 248
    private val magic = "SCCP-REPLAY-SMT-V1".toByteArray(Charsets.US_ASCII)
    private val maxU128 = BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE)

    /** Hash one complete production replay domain. */
    @JvmStatic fun domainHash(
        source: SccpNetworkV1,
        target: SccpNetworkV1,
        boundary: SccpReplayBoundaryV1,
        routeRevision: Long,
        routeConfigurationHash: ByteArray,
        actor: SccpReplayActorV1,
    ): ByteArray {
        require(source.production && target.production) { "replay domains admit production networks only" }
        require(routeRevision in 1..0xffff_ffffL) { "route revision must be a nonzero u32" }
        require(validDirection(source, target, boundary, actor.kind)) {
            "invalid replay boundary, direction, or actor"
        }
        return hash(
            magic,
            byteArrayOf(0),
            unsignedBe(BigInteger.valueOf(source.tag.toLong()), 4, "source tag"),
            unsignedBe(BigInteger.valueOf(target.tag.toLong()), 4, "target tag"),
            byteArrayOf(boundary.tag.toByte()),
            unsignedBe(BigInteger.valueOf(routeRevision), 4, "route revision"),
            requireExact(routeConfigurationHash, 32, "route configuration hash"),
            byteArrayOf(actor.kind.toByte()),
            unsignedBe(BigInteger.valueOf(actor.bytes.size.toLong()), 2, "actor length"),
            actor.bytes,
        )
    }

    /** Derive the full replay key; its first byte selects one of 256 shards. */
    @JvmStatic fun replayKey(domainHash: ByteArray, replayId: ByteArray): ByteArray = hash(
        magic,
        byteArrayOf(1),
        requireExact(domainHash, 32, "domain hash"),
        requireExact(replayId, 32, "replay id"),
    )

    /** Hash one canonical occupied replay record with a scale-9 u128 amount. */
    @JvmStatic fun recordDigest(
        operation: SccpReplayBoundaryV1,
        replayId: ByteArray,
        payloadSha256: ByteArray,
        amountScale9: BigInteger,
        principal: SccpReplayPrincipalV1,
        auxiliaryIdentitySha256: ByteArray,
    ): ByteArray {
        require(amountScale9.signum() > 0 && amountScale9 <= maxU128) {
            "replay amount must be a positive u128"
        }
        val principalDigest = hash(
            magic,
            byteArrayOf(3, principal.kind.toByte()),
            unsignedBe(BigInteger.valueOf(principal.bytes.size.toLong()), 2, "principal length"),
            principal.bytes,
        )
        val auxiliary = hash(
            magic,
            byteArrayOf(4, operation.tag.toByte()),
            requireExact(auxiliaryIdentitySha256, 32, "auxiliary identity SHA-256"),
        )
        return hash(
            magic,
            byteArrayOf(2, operation.tag.toByte()),
            requireExact(replayId, 32, "replay id"),
            requireExact(payloadSha256, 32, "payload SHA-256"),
            unsignedBe(amountScale9, 16, "scale-9 amount"),
            principalDigest,
            auxiliary,
        )
    }

    /** Return all canonical empty hashes in leaf-up order. */
    @JvmStatic fun emptyHashes(): List<ByteArray> {
        val hashes = ArrayList<ByteArray>(DEPTH + 1)
        hashes.add(hash(magic, byteArrayOf(0x10)))
        for (level in 0 until DEPTH) hashes.add(parent(level, hashes[level], hashes[level]))
        return hashes.map { it.copyOf() }
    }

    /** Reconstruct and strictly validate a canonical compressed witness. */
    @JvmStatic fun rootFromWitness(
        keyValue: ByteArray,
        recordDigest: ByteArray?,
        witness: SccpSparseMerkleWitnessV1,
    ): SccpReplayWitnessRootV1 {
        val key = requireExact(keyValue, 32, "replay key")
        require(witness.siblingBitmap[0].toInt() == 0) { "witness bitmap has reserved high bits" }
        val setBits = witness.siblingBitmap.fold(0) { count, byte ->
            count + Integer.bitCount(byte.toInt() and 0xff)
        }
        require(setBits == witness.siblings.size && setBits <= DEPTH) {
            "witness sibling count does not match bitmap"
        }
        val empty = emptyHashes()
        var current = if (recordDigest == null) {
            require(witness.priorRecordDigest.all { it.toInt() == 0 }) {
                "non-membership witness has an occupied digest"
            }
            empty[0]
        } else {
            val digest = requireExact(recordDigest, 32, "record digest")
            require(digest.contentEquals(witness.priorRecordDigest)) {
                "membership witness record digest mismatch"
            }
            hash(magic, byteArrayOf(0x11), key, digest)
        }
        var supplied = 0
        for (level in 0 until DEPTH) {
            val sibling = if (bit(witness.siblingBitmap, level)) {
                witness.siblings[supplied++].also {
                    require(!it.contentEquals(empty[level])) {
                        "witness explicitly encodes a default sibling"
                    }
                }
            } else {
                empty[level]
            }
            current = if (bit(key, level)) parent(level, sibling, current)
            else parent(level, current, sibling)
        }
        return SccpReplayWitnessRootV1(current, witness.expectedShardRoot, key[0].toInt() and 0xff)
    }

    private fun parent(level: Int, left: ByteArray, right: ByteArray): ByteArray = hash(
        magic,
        byteArrayOf(0x12),
        unsignedBe(BigInteger.valueOf(level.toLong()), 2, "tree level"),
        left,
        right,
    )

    private fun bit(value: ByteArray, level: Int): Boolean =
        (value[31 - level / 8].toInt() and (1 shl (level % 8))) != 0

    private fun validDirection(
        source: SccpNetworkV1,
        target: SccpNetworkV1,
        boundary: SccpReplayBoundaryV1,
        actorKind: Int,
    ): Boolean = when (boundary) {
        SccpReplayBoundaryV1.SORA_OUTBOUND_LOCK ->
            source == SccpNetworkV1.SORA_TAIRA && target.isExternal && actorKind == 0
        SccpReplayBoundaryV1.SORA_INBOUND_RELEASE ->
            source.isExternal && target == SccpNetworkV1.SORA_TAIRA && actorKind == 0
        SccpReplayBoundaryV1.EVM_SOURCE_BURN ->
            source in setOf(SccpNetworkV1.ETHEREUM_MAINNET, SccpNetworkV1.BSC_MAINNET) &&
                target == SccpNetworkV1.SORA_TAIRA && actorKind == 1
        SccpReplayBoundaryV1.EVM_DESTINATION_MINT ->
            source == SccpNetworkV1.SORA_TAIRA &&
                target in setOf(SccpNetworkV1.ETHEREUM_MAINNET, SccpNetworkV1.BSC_MAINNET) &&
                actorKind == 1
        SccpReplayBoundaryV1.TRON_SOURCE_BURN ->
            source == SccpNetworkV1.TRON_MAINNET && target == SccpNetworkV1.SORA_TAIRA && actorKind == 2
        SccpReplayBoundaryV1.TRON_DESTINATION_MINT ->
            source == SccpNetworkV1.SORA_TAIRA && target == SccpNetworkV1.TRON_MAINNET && actorKind == 2
        SccpReplayBoundaryV1.TON_BRIDGE_INBOUND_MINT,
        SccpReplayBoundaryV1.TON_MASTER_MINT,
        SccpReplayBoundaryV1.TON_WALLET_MINT_CREDIT,
        SccpReplayBoundaryV1.TON_WALLET_REFUND_DEBIT,
        SccpReplayBoundaryV1.TON_WALLET_REFUND_CREDIT ->
            source == SccpNetworkV1.SORA_TAIRA && target == SccpNetworkV1.TON_MAINNET && actorKind == 3
        SccpReplayBoundaryV1.TON_BRIDGE_OUTBOUND_BURN,
        SccpReplayBoundaryV1.TON_MASTER_BURN,
        SccpReplayBoundaryV1.TON_WALLET_BURN_DEBIT ->
            source == SccpNetworkV1.TON_MAINNET && target == SccpNetworkV1.SORA_TAIRA && actorKind == 3
    }

    private fun hash(vararg parts: ByteArray): ByteArray {
        val digest = MessageDigest.getInstance("SHA-256")
        for (part in parts) digest.update(part)
        return digest.digest()
    }
}

private fun canonicalSoraAccountId(payload: ByteArray): ByteArray {
    require(payload.isNotEmpty() && payload.size <= 0xffff) {
        "SORA replay principal must be canonical nonempty AccountId bytes"
    }
    val rendered = try {
        TransferWirePayloadEncoder.decodeAccountIdPayload(
            payload,
            SccpV1.TAIRA_I105_DISCRIMINANT_V1,
        )
    } catch (error: RuntimeException) {
        throw IllegalArgumentException("SORA replay principal is not a canonical AccountId", error)
    }
    val canonical = try {
        TransferWirePayloadEncoder.encodeAccountIdPayload(rendered)
    } catch (error: RuntimeException) {
        throw IllegalArgumentException("SORA replay principal is not a canonical AccountId", error)
    }
    require(canonical.contentEquals(payload)) {
        "SORA replay principal is not the canonical AccountId encoding"
    }
    return payload.copyOf()
}

private fun requireExact(
    value: ByteArray,
    length: Int,
    label: String,
    nonzero: Boolean = true,
): ByteArray {
    require(value.size == length && (!nonzero || value.any { it.toInt() != 0 })) {
        "$label must be ${if (nonzero) "nonzero " else ""}$length bytes"
    }
    return value.copyOf()
}

private fun signedI32Be(value: Int): ByteArray = byteArrayOf(
    (value ushr 24).toByte(),
    (value ushr 16).toByte(),
    (value ushr 8).toByte(),
    value.toByte(),
)

private fun unsignedBe(value: BigInteger, width: Int, label: String): ByteArray {
    require(value.signum() >= 0 && value.bitLength() <= width * 8) { "$label exceeds u${width * 8}" }
    val source = value.toByteArray()
    val result = ByteArray(width)
    val copyLength = minOf(width, source.size)
    System.arraycopy(source, source.size - copyLength, result, width - copyLength, copyLength)
    return result
}
