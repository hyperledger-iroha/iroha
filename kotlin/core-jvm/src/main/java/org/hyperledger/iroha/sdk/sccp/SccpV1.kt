package org.hyperledger.iroha.sdk.sccp

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import org.bouncycastle.crypto.digests.KeccakDigest
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.crypto.Blake2b

/** Closed first-release SCCP network inventory. Parsing is exact and case-sensitive. */
enum class SccpNetworkV1(
    val profileKey: String,
    val tag: Int,
    val domainId: Int,
    val production: Boolean,
) {
    SORA_TAIRA("sora-taira", 0x40, 0, true),
    ETHEREUM_MAINNET("ethereum-mainnet", 0x41, 1, true),
    BSC_MAINNET("bsc-mainnet", 0x42, 2, true),
    TRON_MAINNET("tron-mainnet", 0x43, 3, true),
    TON_MAINNET("ton-mainnet", 0x44, 4, true);

    val isSora: Boolean get() = this == SORA_TAIRA
    val isExternal: Boolean get() = !isSora

    companion object {
        private val byProfile = values().associateBy(SccpNetworkV1::profileKey)
        private val byTag = values().associateBy(SccpNetworkV1::tag)

        /** Parse one canonical profile key; aliases, case changes, and whitespace fail. */
        @JvmStatic fun fromProfileKey(profile: String): SccpNetworkV1? = byProfile[profile]

        /** Decode one stable profile tag. Reserved and unknown tags fail. */
        @JvmStatic fun fromTag(tag: Int): SccpNetworkV1? = byTag[tag]
    }
}

/** Directed SCCP lane between exact network profiles. */
class SccpLaneIdV1(val source: SccpNetworkV1, val target: SccpNetworkV1) {
    init {
        require(source.isSora != target.isSora && source.domainId != target.domainId) {
            "SCCP lane must join exactly one SORA profile and one external profile"
        }
    }

    val isOutbound: Boolean get() = source.isSora && target.isExternal
    val isInbound: Boolean get() = source.isExternal && target.isSora

    override fun equals(other: Any?): Boolean =
        other is SccpLaneIdV1 && source == other.source && target == other.target

    override fun hashCode(): Int = 31 * source.hashCode() + target.hashCode()

    override fun toString(): String = "${source.profileKey}->${target.profileKey}"
}

/** Exact governed context for one SORA-origin message. */
class SccpOutboundMessageContextV1(
    val lane: SccpLaneIdV1,
    destinationBindingHash: ByteArray,
    routeConfigurationHash: ByteArray,
) {
    private val binding = requireHash(destinationBindingHash, "destinationBindingHash")
    private val configuration = requireHash(routeConfigurationHash, "routeConfigurationHash")

    init {
        require(lane.isOutbound) { "outbound SCCP context must use a SORA-to-external lane" }
        require(!binding.contentEquals(configuration)) {
            "destination binding and route configuration must be distinct"
        }
    }

    fun destinationBindingHash(): ByteArray = binding.copyOf()
    fun routeConfigurationHash(): ByteArray = configuration.copyOf()

    override fun equals(other: Any?): Boolean =
        other is SccpOutboundMessageContextV1 &&
            lane == other.lane &&
            binding.contentEquals(other.binding) &&
            configuration.contentEquals(other.configuration)

    override fun hashCode(): Int =
        31 * (31 * lane.hashCode() + binding.contentHashCode()) + configuration.contentHashCode()
}

/** The sole stable hub-commitment kind shipped in SCCP V1. */
enum class SccpHubMessageKindV1(val tag: Int) {
    TRANSFER(0);

    companion object {
        @JvmStatic fun fromTag(tag: Int): SccpHubMessageKindV1? =
            values().firstOrNull { it.tag == tag }
    }
}

/** Closed canonical SCCP payload. V1 contains only transfer. */
sealed class SccpPayloadV1 protected constructor(val kind: SccpHubMessageKindV1) {
    internal abstract val sourceDomain: Int
    internal abstract val targetDomain: Int
    internal abstract fun encodeBody(out: ByteArrayOutputStream)

    fun canonicalBytes(): ByteArray = ByteArrayOutputStream().also { out ->
        out.write(TRANSFER_DISCRIMINANT)
        encodeBody(out)
    }.toByteArray()

    private companion object {
        const val TRANSFER_DISCRIMINANT = 0
    }
}

/** The sole value-moving payload in SCCP V1. */
class SccpTransferPayloadV1(
    val source: Int,
    val destination: Int,
    nonce: BigInteger,
    val routeRevision: Long,
    val assetHomeDomain: Int,
    val assetIdCodec: Int,
    assetId: ByteArray,
    amount: BigInteger,
    val senderCodec: Int,
    sender: ByteArray,
    val recipientCodec: Int,
    recipient: ByteArray,
    val routeIdCodec: Int,
    routeId: ByteArray,
) : SccpPayloadV1(SccpHubMessageKindV1.TRANSFER) {
    val nonce: BigInteger = requireUnsigned(nonce, 64, "nonce")
    val amount: BigInteger = requireUnsigned(amount, 128, "amount").also {
        require(it.signum() != 0) { "amount must be nonzero" }
    }
    private val asset = requireCodecValue(assetIdCodec, assetId, "assetId")
    private val senderValue = requireCodecValue(senderCodec, sender, "sender")
    private val recipientValue = requireCodecValue(recipientCodec, recipient, "recipient")
    private val route = requireCodecValue(routeIdCodec, routeId, "routeId")
    override val sourceDomain: Int = source
    override val targetDomain: Int = destination

    init {
        requireDomain(source, "source")
        requireDomain(destination, "destination")
        requireDomain(assetHomeDomain, "assetHomeDomain")
        require(source != destination) { "transfer endpoints must differ" }
        require(routeRevision in 1..0xffff_ffffL) { "routeRevision must be a nonzero u32" }
        require(senderCodec == accountCodec(source)) { "sender codec does not match source domain" }
        require(recipientCodec == accountCodec(destination)) {
            "recipient codec does not match destination domain"
        }
    }

    constructor(
        source: Int,
        destination: Int,
        nonce: Long,
        routeRevision: Long,
        assetHomeDomain: Int,
        assetIdCodec: Int,
        assetId: ByteArray,
        amount: BigInteger,
        senderCodec: Int,
        sender: ByteArray,
        recipientCodec: Int,
        recipient: ByteArray,
        routeIdCodec: Int,
        routeId: ByteArray,
    ) : this(
        source,
        destination,
        BigInteger.valueOf(nonce),
        routeRevision,
        assetHomeDomain,
        assetIdCodec,
        assetId,
        amount,
        senderCodec,
        sender,
        recipientCodec,
        recipient,
        routeIdCodec,
        routeId,
    )

    fun assetId(): ByteArray = asset.copyOf()
    fun sender(): ByteArray = senderValue.copyOf()
    fun recipient(): ByteArray = recipientValue.copyOf()
    fun routeId(): ByteArray = route.copyOf()

    override fun encodeBody(out: ByteArrayOutputStream) {
        out.write(1)
        writeU32(out, source)
        writeU32(out, destination)
        writeUnsignedLe(out, nonce, 8)
        writeU32Bits(out, routeRevision)
        writeU32(out, assetHomeDomain)
        out.write(assetIdCodec)
        writeBytes(out, asset)
        writeUnsignedLe(out, amount, 16)
        out.write(senderCodec)
        writeBytes(out, senderValue)
        out.write(recipientCodec)
        writeBytes(out, recipientValue)
        out.write(routeIdCodec)
        writeBytes(out, route)
    }
}

/** Exact fixed-width SORA hub commitment. */
class SccpHubCommitmentV1 internal constructor(
    val kind: SccpHubMessageKindV1,
    val context: SccpOutboundMessageContextV1,
    messageId: ByteArray,
    payloadHash: ByteArray,
) {
    private val message = requireHash(messageId, "messageId")
    private val payload = requireHash(payloadHash, "payloadHash")

    fun messageId(): ByteArray = message.copyOf()
    fun payloadHash(): ByteArray = payload.copyOf()
}

/** Consensus-compatible exact-lane hashing and fixed layouts for SCCP V1. */
object SccpV1 {
    /** Exact I105 discriminant used by the public SORA Taira SCCP endpoint. */
    const val TAIRA_I105_DISCRIMINANT_V1 = 369

    private val laneHashPrefix = "sccp:lane-id:v1".toByteArray(Charsets.UTF_8)
    private val messageIdPrefix = "sccp:lane-message-id:v1".toByteArray(Charsets.UTF_8)
    private val payloadHashPrefix = "sccp:payload:v1".toByteArray(Charsets.UTF_8)
    private val leafHashPrefix = "sccp:hub:leaf:v1".toByteArray(Charsets.UTF_8)
    private val sourceEventDigestPrefix = "sccp:source:event:v1".toByteArray(Charsets.UTF_8)

    /** Canonical profile bytes independent of JVM enum layout. */
    @JvmStatic fun canonicalNetworkBytes(network: SccpNetworkV1): ByteArray =
        ByteArrayOutputStream().also { out ->
            out.write(1)
            out.write(network.tag)
            writeU32(out, network.domainId)
            when (network) {
                SccpNetworkV1.SORA_TAIRA ->
                    out.write(hex("fc56984b2be7431d840e21514d1883f0"))
                SccpNetworkV1.ETHEREUM_MAINNET -> writeUnsignedLe(out, BigInteger.ONE, 8)
                SccpNetworkV1.BSC_MAINNET -> writeUnsignedLe(out, BigInteger.valueOf(56), 8)
                SccpNetworkV1.TRON_MAINNET -> writeU32Bits(out, 0x2b6653dcL)
                SccpNetworkV1.TON_MAINNET -> writeTonNetwork(
                    out,
                    -239,
                    "17a3a92992aabea785a7a090985a265cd31f323d849da51239737e321fb05569",
                    "5e994fcf4d425c0a6ce6a792594b7173205f740a39cd56f537defd28b48a0f6e",
                )
            }
        }.toByteArray()

    /** Canonical exact-lane bytes. */
    @JvmStatic fun canonicalLaneBytes(lane: SccpLaneIdV1): ByteArray =
        ByteArrayOutputStream().also { out ->
            out.write(1)
            writeBytes(out, canonicalNetworkBytes(lane.source))
            writeBytes(out, canonicalNetworkBytes(lane.target))
        }.toByteArray()

    /** Blake2b-256 of the domain-separated exact lane. */
    @JvmStatic fun laneHash(lane: SccpLaneIdV1): ByteArray =
        prefixedBlake2b(laneHashPrefix, canonicalLaneBytes(lane))

    /** Lane-bound message identity. Governed deployment hashes are deliberately excluded. */
    @JvmStatic fun messageId(lane: SccpLaneIdV1, payload: SccpPayloadV1): ByteArray {
        require(lane.source.domainId == payload.sourceDomain && lane.target.domainId == payload.targetDomain) {
            "payload domains do not match exact SCCP lane"
        }
        val body = ByteArrayOutputStream()
        body.write(1)
        writeBytes(body, canonicalLaneBytes(lane))
        writeBytes(body, payload.canonicalBytes())
        return prefixedKeccak(messageIdPrefix, body.toByteArray()).also {
            require(it.any { byte -> byte.toInt() != 0 }) { "messageId must be nonzero" }
        }
    }

    /** Hash the exact canonical payload. */
    @JvmStatic fun payloadHash(payload: SccpPayloadV1): ByteArray =
        prefixedBlake2b(payloadHashPrefix, payload.canonicalBytes())

    /** Decode exactly one canonical transfer payload and reject trailing or retired variants. */
    @JvmStatic fun decodeCanonicalPayload(bytes: ByteArray): SccpTransferPayloadV1 {
        val cursor = Cursor(bytes)
        require(cursor.u8() == 0) { "unsupported or retired SCCP payload discriminant" }
        require(cursor.u8() == 1) { "unsupported SCCP transfer version" }
        val source = cursor.u32Domain("source")
        val destination = cursor.u32Domain("destination")
        val nonce = cursor.unsigned(8)
        val revision = cursor.u32()
        val home = cursor.u32Domain("assetHomeDomain")
        val assetCodec = cursor.u8()
        val asset = cursor.bytes()
        val amount = cursor.unsigned(16)
        val senderCodec = cursor.u8()
        val sender = cursor.bytes()
        val recipientCodec = cursor.u8()
        val recipient = cursor.bytes()
        val routeCodec = cursor.u8()
        val route = cursor.bytes()
        require(cursor.finished()) { "canonical SCCP payload must not contain trailing bytes" }
        val payload = SccpTransferPayloadV1(
            source,
            destination,
            nonce,
            revision,
            home,
            assetCodec,
            asset,
            amount,
            senderCodec,
            sender,
            recipientCodec,
            recipient,
            routeCodec,
            route,
        )
        require(payload.canonicalBytes().contentEquals(bytes)) { "non-canonical SCCP payload" }
        return payload
    }

    /** Canonical contract-computable source-event preimage after the domain prefix. */
    @JvmStatic fun canonicalSourceEventBytes(
        lane: SccpLaneIdV1,
        messageId: ByteArray,
        payloadHash: ByteArray,
    ): ByteArray {
        val laneHash = laneHash(lane)
        val message = requireHash(messageId, "messageId")
        val payload = requireHash(payloadHash, "payloadHash")
        requireDistinctHashes(listOf(laneHash, message, payload), "source-event")
        return ByteArrayOutputStream(97).also { out ->
            out.write(1)
            out.write(laneHash)
            out.write(message)
            out.write(payload)
        }.toByteArray()
    }

    /** Keccak-256 digest committed by every exact native source event. */
    @JvmStatic fun sourceEventDigest(
        lane: SccpLaneIdV1,
        messageId: ByteArray,
        payloadHash: ByteArray,
    ): ByteArray = prefixedKeccak(
        sourceEventDigestPrefix,
        canonicalSourceEventBytes(lane, messageId, payloadHash),
    )

    /** Construct a role-separated exact outbound commitment. */
    @JvmStatic fun commitment(
        context: SccpOutboundMessageContextV1,
        payload: SccpPayloadV1,
    ): SccpHubCommitmentV1 {
        val laneHash = laneHash(context.lane)
        val messageId = messageId(context.lane, payload)
        val payloadHash = payloadHash(payload)
        requireDistinctHashes(
            listOf(
                laneHash,
                context.destinationBindingHash(),
                context.routeConfigurationHash(),
                messageId,
                payloadHash,
            ),
            "commitment",
        )
        return SccpHubCommitmentV1(payload.kind, context, messageId, payloadHash)
    }

    /** Fixed V1 commitment bytes with four independently governed/hash roles. */
    @JvmStatic fun canonicalCommitmentBytes(commitment: SccpHubCommitmentV1): ByteArray =
        ByteArrayOutputStream().also { out ->
            out.write(1)
            out.write(commitment.kind.tag)
            out.write(commitment.context.lane.source.tag)
            out.write(commitment.context.lane.target.tag)
            out.write(commitment.context.destinationBindingHash())
            out.write(commitment.context.routeConfigurationHash())
            out.write(commitment.messageId())
            out.write(commitment.payloadHash())
        }.toByteArray()

    /** Decode and re-encode a fixed V1 commitment, rejecting unknown or colliding roles. */
    @JvmStatic fun decodeCanonicalCommitment(bytes: ByteArray): SccpHubCommitmentV1 {
        require(bytes.size == 132) { "canonical SCCP commitment must contain 132 bytes" }
        require((bytes[0].toInt() and 0xff) == 1) { "unsupported SCCP commitment version" }
        val kind = SccpHubMessageKindV1.fromTag(bytes[1].toInt() and 0xff)
            ?: throw IllegalArgumentException("unknown SCCP commitment kind")
        val source = SccpNetworkV1.fromTag(bytes[2].toInt() and 0xff)
            ?: throw IllegalArgumentException("unknown SCCP source profile tag")
        val target = SccpNetworkV1.fromTag(bytes[3].toInt() and 0xff)
            ?: throw IllegalArgumentException("unknown SCCP target profile tag")
        val context = SccpOutboundMessageContextV1(
            SccpLaneIdV1(source, target),
            bytes.copyOfRange(4, 36),
            bytes.copyOfRange(36, 68),
        )
        val result = SccpHubCommitmentV1(
            kind,
            context,
            bytes.copyOfRange(68, 100),
            bytes.copyOfRange(100, 132),
        )
        requireDistinctHashes(
            listOf(
                laneHash(context.lane),
                context.destinationBindingHash(),
                context.routeConfigurationHash(),
                result.messageId(),
                result.payloadHash(),
            ),
            "commitment",
        )
        require(canonicalCommitmentBytes(result).contentEquals(bytes)) {
            "non-canonical SCCP commitment"
        }
        return result
    }

    /** Domain-separated leaf/root for an empty Merkle path. */
    @JvmStatic fun commitmentRoot(commitment: SccpHubCommitmentV1): ByteArray =
        prefixedBlake2b(leafHashPrefix, canonicalCommitmentBytes(commitment))

    /** Strict lowercase, prefixless hexadecimal decoder used by shared fixtures. */
    @JvmStatic fun decodeLowerHex(value: String): ByteArray = hex(value)

    /** Lowercase, prefixless hexadecimal encoder. */
    @JvmStatic fun encodeLowerHex(value: ByteArray): String =
        value.joinToString("") { "%02x".format(it.toInt() and 0xff) }
}

private class Cursor(private val input: ByteArray) {
    private var offset = 0

    fun u8(): Int = exact(1)[0].toInt() and 0xff

    fun u32(): Long {
        val value = exact(4)
        return (value[0].toLong() and 0xff) or
            ((value[1].toLong() and 0xff) shl 8) or
            ((value[2].toLong() and 0xff) shl 16) or
            ((value[3].toLong() and 0xff) shl 24)
    }

    fun u32Domain(field: String): Int = u32().also {
        require(it <= Int.MAX_VALUE.toLong()) { "$field is outside the closed domain inventory" }
    }.toInt().also { requireDomain(it, field) }

    fun unsigned(size: Int): BigInteger = BigInteger(1, exact(size).reversedArray())

    fun bytes(): ByteArray {
        val size = u32()
        require(size <= Int.MAX_VALUE.toLong()) { "SCCP byte vector is too large" }
        return exact(size.toInt())
    }

    fun finished(): Boolean = offset == input.size

    private fun exact(size: Int): ByteArray {
        require(size >= 0 && offset <= input.size - size) { "truncated SCCP canonical bytes" }
        val value = input.copyOfRange(offset, offset + size)
        offset += size
        return value
    }
}

private fun requireDomain(value: Int, field: String) {
    require(value in 0..4) {
        "$field must be a supported SCCP domain"
    }
}

private fun accountCodec(domain: Int): Int = when (domain) {
    0 -> 0
    1, 2 -> 1
    3 -> 2
    4 -> 3
    else -> throw IllegalArgumentException("unsupported SCCP domain")
}

private fun requireHash(value: ByteArray, field: String): ByteArray {
    require(value.size == 32) { "$field must contain 32 bytes" }
    require(value.any { it.toInt() != 0 }) { "$field must be nonzero" }
    return value.copyOf()
}

private fun requireDistinctHashes(values: List<ByteArray>, role: String) {
    require(values.all { value -> value.size == 32 && value.any { it.toInt() != 0 } }) {
        "SCCP $role hash roles must be nonzero"
    }
    require(values.indices.all { left ->
        (left + 1 until values.size).all { right -> !values[left].contentEquals(values[right]) }
    }) { "SCCP $role hash roles must be pairwise distinct" }
}

private fun requireUnsigned(value: BigInteger, bits: Int, field: String): BigInteger {
    require(value.signum() >= 0 && value.bitLength() <= bits) { "$field must fit u$bits" }
    return value
}

private fun requireCodecValue(codec: Int, value: ByteArray, field: String): ByteArray {
    val valid = when (codec) {
        0 -> isCanonicalText(value, field)
        1 -> value.size == 20 && value.any { it.toInt() != 0 }
        2 -> value.size == 21 && (value[0].toInt() and 0xff) == 0x41 &&
            value.copyOfRange(1, 21).any { it.toInt() != 0 }
        3 -> value.size == 36 && value.copyOfRange(0, 4).all { it.toInt() == 0 } &&
            value.copyOfRange(4, 36).any { it.toInt() != 0 }
        else -> false
    }
    require(valid) { "$field does not match closed SCCP codec $codec" }
    return value.copyOf()
}

private fun isCanonicalText(value: ByteArray, field: String): Boolean {
    if (value.isEmpty() || value.size > 256) return false
    if (value.all { (it.toInt() and 0xff) in 0x21..0x7e }) return true

    val literal = value.toString(Charsets.UTF_8)
    if (!literal.toByteArray(Charsets.UTF_8).contentEquals(value)) return false
    return try {
        requireCanonicalI105Address(literal, field)
        true
    } catch (_: IllegalArgumentException) {
        false
    }
}

private fun writeU32(out: ByteArrayOutputStream, value: Int) {
    require(value >= 0) { "u32 value must be non-negative" }
    writeU32Bits(out, value.toLong())
}

private fun writeU32Bits(out: ByteArrayOutputStream, value: Long) {
    require(value in 0..0xffff_ffffL) { "value must fit u32" }
    repeat(4) { shift -> out.write(((value ushr (shift * 8)) and 0xff).toInt()) }
}

private fun writeI32Bits(out: ByteArrayOutputStream, value: Int) {
    repeat(4) { shift -> out.write((value ushr (shift * 8)) and 0xff) }
}

private fun writeTonNetwork(
    out: ByteArrayOutputStream,
    globalId: Int,
    zeroStateRootHex: String,
    zeroStateFileHex: String,
) {
    writeI32Bits(out, globalId)
    writeI32Bits(out, -1)
    writeUnsignedLe(out, BigInteger.ONE.shiftLeft(63), 8)
    writeU32(out, 0)
    out.write(hex(zeroStateRootHex))
    out.write(hex(zeroStateFileHex))
}

private fun writeUnsignedLe(out: ByteArrayOutputStream, value: BigInteger, size: Int) {
    require(value.signum() >= 0 && value.bitLength() <= size * 8) {
        "unsigned integer does not fit"
    }
    val bigEndian = value.toByteArray()
    repeat(size) { index ->
        val source = bigEndian.size - 1 - index
        out.write(if (source >= 0) bigEndian[source].toInt() and 0xff else 0)
    }
}

private fun writeBytes(out: ByteArrayOutputStream, value: ByteArray) {
    writeU32Bits(out, value.size.toLong())
    out.write(value)
}

private fun prefixedBlake2b(prefix: ByteArray, payload: ByteArray): ByteArray =
    Blake2b.digest256(prefix + payload)

private fun prefixedKeccak(prefix: ByteArray, payload: ByteArray): ByteArray {
    val digest = KeccakDigest(256)
    digest.update(prefix, 0, prefix.size)
    digest.update(payload, 0, payload.size)
    return ByteArray(32).also { digest.doFinal(it, 0) }
}

private fun hex(value: String): ByteArray {
    require(value.length % 2 == 0 && value.all { it in '0'..'9' || it in 'a'..'f' }) {
        "hex must be canonical lowercase without 0x"
    }
    return ByteArray(value.length / 2) { index ->
        value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
    }
}
