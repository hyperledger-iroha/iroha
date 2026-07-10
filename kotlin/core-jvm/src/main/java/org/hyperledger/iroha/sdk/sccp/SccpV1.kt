package org.hyperledger.iroha.sdk.sccp

import java.io.ByteArrayOutputStream
import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.ByteOrder
import org.bouncycastle.crypto.digests.KeccakDigest
import org.hyperledger.iroha.sdk.crypto.Blake2b

/** Closed first-release SCCP network inventory. Profile parsing is exact and case-sensitive. */
enum class SccpNetworkV1(
    val profileKey: String,
    val tag: Int,
    val domainId: Int,
    val production: Boolean,
) {
    SORA_NEXUS("sora-nexus", 0, 0, true),
    SORA_TAIRA("sora-taira", 1, 0, false),
    ETHEREUM_MAINNET("ethereum-mainnet", 2, 1, true),
    ETHEREUM_SEPOLIA("ethereum-sepolia", 3, 1, false),
    BSC_MAINNET("bsc-mainnet", 4, 2, true),
    BSC_TESTNET("bsc-testnet", 5, 2, false),
    SOLANA_MAINNET_BETA("solana-mainnet-beta", 6, 3, true),
    SOLANA_TESTNET("solana-testnet", 7, 3, false),
    TON_MAINNET("ton-mainnet", 8, 4, true),
    TON_TESTNET("ton-testnet", 9, 4, false),
    TRON_MAINNET("tron-mainnet", 10, 5, true),
    TRON_NILE("tron-nile", 11, 5, false),
    TRON_SHASTA("tron-shasta", 12, 5, false);

    val isSora: Boolean get() = this == SORA_NEXUS || this == SORA_TAIRA
    val isExternal: Boolean get() = !isSora

    companion object {
        private val byProfile = values().associateBy(SccpNetworkV1::profileKey)
        private val byTag = values().associateBy(SccpNetworkV1::tag)

        /** Parse only a canonical profile key; aliases, case changes, and whitespace are rejected. */
        @JvmStatic fun fromProfileKey(profile: String): SccpNetworkV1? = byProfile[profile]

        /** Decode a stable first-release profile tag. */
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

/** Exact governed context for a SORA-origin message. */
class SccpOutboundMessageContextV1(lane: SccpLaneIdV1, destinationBindingHash: ByteArray) {
    val lane: SccpLaneIdV1 = lane
    private val binding = requireHash(destinationBindingHash, "destinationBindingHash")

    init {
        require(lane.isOutbound) { "outbound SCCP context must use a SORA-to-external lane" }
    }

    fun destinationBindingHash(): ByteArray = binding.copyOf()

    override fun equals(other: Any?): Boolean =
        other is SccpOutboundMessageContextV1 && lane == other.lane && binding.contentEquals(other.binding)

    override fun hashCode(): Int = 31 * lane.hashCode() + binding.contentHashCode()
}

/** Stable hub commitment kind tags. */
enum class SccpHubMessageKindV1(val tag: Int) {
    TOKEN_ADD(0),
    TOKEN_PAUSE(1),
    TOKEN_RESUME(2),
    ASSET_REGISTER(3),
    ROUTE_ACTIVATE(4),
    TRANSFER(5);

    companion object {
        @JvmStatic fun fromTag(tag: Int): SccpHubMessageKindV1? = values().firstOrNull { it.tag == tag }
    }
}

/** Canonical first-release SCCP payload. */
sealed class SccpPayloadV1 protected constructor(
    internal val discriminant: Int,
    val kind: SccpHubMessageKindV1,
) {
    internal abstract val sourceDomain: Int
    internal abstract val targetDomain: Int
    internal abstract fun encodeBody(out: ByteArrayOutputStream)

    fun canonicalBytes(): ByteArray = ByteArrayOutputStream().also { out ->
        out.write(discriminant)
        encodeBody(out)
    }.toByteArray()
}

/** Asset registration payload. */
class SccpAssetRegisterPayloadV1(
    val target: Int,
    val home: Int,
    nonce: BigInteger,
    val assetIdCodec: Int,
    assetId: ByteArray,
    val decimals: Int,
) : SccpPayloadV1(0, SccpHubMessageKindV1.ASSET_REGISTER) {
    val nonce: BigInteger = requireUnsigned(nonce, 64, "nonce")
    private val asset = requireCodecValue(assetIdCodec, assetId, "assetId")
    override val sourceDomain: Int = home
    override val targetDomain: Int = target

    init {
        requireDomain(target, "target")
        requireDomain(home, "home")
        require(home != target) { "asset registration endpoints must differ" }
        require(decimals in 0..255) { "decimals must fit u8" }
    }

    constructor(target: Int, home: Int, nonce: Long, assetIdCodec: Int, assetId: ByteArray, decimals: Int) :
        this(target, home, BigInteger.valueOf(nonce), assetIdCodec, assetId, decimals)

    fun assetId(): ByteArray = asset.copyOf()

    override fun encodeBody(out: ByteArrayOutputStream) {
        out.write(1)
        writeU32(out, target)
        writeU32(out, home)
        writeUnsignedLe(out, nonce, 8)
        out.write(assetIdCodec)
        writeBytes(out, asset)
        out.write(decimals)
    }
}

/** Route activation payload. */
class SccpRouteActivatePayloadV1(
    val source: Int,
    val target: Int,
    nonce: BigInteger,
    val assetIdCodec: Int,
    assetId: ByteArray,
    val routeIdCodec: Int,
    routeId: ByteArray,
) : SccpPayloadV1(1, SccpHubMessageKindV1.ROUTE_ACTIVATE) {
    val nonce: BigInteger = requireUnsigned(nonce, 64, "nonce")
    private val asset = requireCodecValue(assetIdCodec, assetId, "assetId")
    private val route = requireCodecValue(routeIdCodec, routeId, "routeId")
    override val sourceDomain: Int = source
    override val targetDomain: Int = target

    init {
        requireDomain(source, "source")
        requireDomain(target, "target")
        require(source != target) { "route endpoints must differ" }
    }

    constructor(source: Int, target: Int, nonce: Long, assetIdCodec: Int, assetId: ByteArray, routeIdCodec: Int, routeId: ByteArray) :
        this(source, target, BigInteger.valueOf(nonce), assetIdCodec, assetId, routeIdCodec, routeId)

    fun assetId(): ByteArray = asset.copyOf()
    fun routeId(): ByteArray = route.copyOf()

    override fun encodeBody(out: ByteArrayOutputStream) {
        out.write(1)
        writeU32(out, source)
        writeU32(out, target)
        writeUnsignedLe(out, nonce, 8)
        out.write(assetIdCodec)
        writeBytes(out, asset)
        out.write(routeIdCodec)
        writeBytes(out, route)
    }
}

/** The sole value-moving SCCP payload in V1. */
class SccpTransferPayloadV1(
    val source: Int,
    val destination: Int,
    nonce: BigInteger,
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
) : SccpPayloadV1(2, SccpHubMessageKindV1.TRANSFER) {
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
        require(senderCodec == accountCodec(source)) { "sender codec does not match source domain" }
        require(recipientCodec == accountCodec(destination)) { "recipient codec does not match destination domain" }
    }

    fun assetId(): ByteArray = asset.copyOf()
    fun sender(): ByteArray = senderValue.copyOf()
    fun recipient(): ByteArray = recipientValue.copyOf()
    fun routeId(): ByteArray = route.copyOf()

    override fun encodeBody(out: ByteArrayOutputStream) {
        out.write(1)
        writeU32(out, source)
        writeU32(out, destination)
        writeUnsignedLe(out, nonce, 8)
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

/** Token creation payload. */
class SccpTokenAddPayloadV1(
    val target: Int,
    nonce: BigInteger,
    soraAssetId: ByteArray,
    val decimals: Int,
    name: ByteArray,
    symbol: ByteArray,
) : SccpPayloadV1(3, SccpHubMessageKindV1.TOKEN_ADD) {
    val nonce: BigInteger = requireUnsigned(nonce, 64, "nonce")
    private val asset = requireHash(soraAssetId, "soraAssetId")
    private val tokenName = requireFixedAscii(name, "name")
    private val tokenSymbol = requireFixedAscii(symbol, "symbol")
    override val sourceDomain: Int = 0
    override val targetDomain: Int = target

    init {
        requireExternalDomain(target, "target")
        require(decimals in 0..255) { "decimals must fit u8" }
    }

    constructor(target: Int, nonce: Long, soraAssetId: ByteArray, decimals: Int, name: ByteArray, symbol: ByteArray) :
        this(target, BigInteger.valueOf(nonce), soraAssetId, decimals, name, symbol)

    override fun encodeBody(out: ByteArrayOutputStream) {
        out.write(1)
        writeU32(out, target)
        writeUnsignedLe(out, nonce, 8)
        out.write(asset)
        out.write(decimals)
        out.write(tokenName)
        out.write(tokenSymbol)
    }
}

/** Token pause payload. */
class SccpTokenPausePayloadV1(target: Int, nonce: BigInteger, soraAssetId: ByteArray) :
    SccpTokenControlPayloadV1(4, SccpHubMessageKindV1.TOKEN_PAUSE, target, nonce, soraAssetId) {
    constructor(target: Int, nonce: Long, soraAssetId: ByteArray) : this(target, BigInteger.valueOf(nonce), soraAssetId)
}

/** Token resume payload. */
class SccpTokenResumePayloadV1(target: Int, nonce: BigInteger, soraAssetId: ByteArray) :
    SccpTokenControlPayloadV1(5, SccpHubMessageKindV1.TOKEN_RESUME, target, nonce, soraAssetId) {
    constructor(target: Int, nonce: Long, soraAssetId: ByteArray) : this(target, BigInteger.valueOf(nonce), soraAssetId)
}

/** Shared canonical implementation for pause/resume payloads. */
sealed class SccpTokenControlPayloadV1 protected constructor(
    discriminant: Int,
    kind: SccpHubMessageKindV1,
    val target: Int,
    nonce: BigInteger,
    soraAssetId: ByteArray,
) : SccpPayloadV1(discriminant, kind) {
    val nonce: BigInteger = requireUnsigned(nonce, 64, "nonce")
    private val asset = requireHash(soraAssetId, "soraAssetId")
    override val sourceDomain: Int = 0
    override val targetDomain: Int = target

    init { requireExternalDomain(target, "target") }

    fun soraAssetId(): ByteArray = asset.copyOf()

    override fun encodeBody(out: ByteArrayOutputStream) {
        out.write(1)
        writeU32(out, target)
        writeUnsignedLe(out, nonce, 8)
        out.write(asset)
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
    private val laneHashPrefix = "sccp:lane-id:v1".toByteArray(Charsets.UTF_8)
    private val messageIdPrefix = "sccp:lane-message-id:v1".toByteArray(Charsets.UTF_8)
    private val payloadHashPrefix = "sccp:payload:v1".toByteArray(Charsets.UTF_8)
    private val leafHashPrefix = "sccp:hub:leaf:v1".toByteArray(Charsets.UTF_8)
    private val sourceEventDigestPrefix = "sccp:source:event:v1".toByteArray(Charsets.UTF_8)

    /** Canonical profile bytes independent of Kotlin/JVM enum layout. */
    @JvmStatic fun canonicalNetworkBytes(network: SccpNetworkV1): ByteArray = ByteArrayOutputStream().also { out ->
        out.write(1)
        out.write(network.tag)
        writeU32(out, network.domainId)
        when (network) {
            SccpNetworkV1.SORA_NEXUS -> out.write(hex("00000000000000000000000000000753"))
            SccpNetworkV1.SORA_TAIRA -> out.write(hex("809574f5fee75e69bfcf52451e42d50f"))
            SccpNetworkV1.ETHEREUM_MAINNET -> writeUnsignedLe(out, BigInteger.ONE, 8)
            SccpNetworkV1.ETHEREUM_SEPOLIA -> writeUnsignedLe(out, BigInteger.valueOf(11_155_111L), 8)
            SccpNetworkV1.BSC_MAINNET -> writeUnsignedLe(out, BigInteger.valueOf(56), 8)
            SccpNetworkV1.BSC_TESTNET -> writeUnsignedLe(out, BigInteger.valueOf(97), 8)
            SccpNetworkV1.SOLANA_MAINNET_BETA -> writeBytes(out, "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdp".toByteArray(Charsets.US_ASCII))
            SccpNetworkV1.SOLANA_TESTNET -> writeBytes(out, "4uhcVJyU9pJkvQyS88uRDiswHXSCkY3zQawwpjk2NsNY".toByteArray(Charsets.US_ASCII))
            SccpNetworkV1.TON_MAINNET -> writeI32(out, -239)
            SccpNetworkV1.TON_TESTNET -> writeI32(out, -3)
            SccpNetworkV1.TRON_MAINNET -> writeU32Bits(out, 0x2b6653dcL)
            SccpNetworkV1.TRON_NILE -> writeU32Bits(out, 0xcd8690dcL)
            SccpNetworkV1.TRON_SHASTA -> writeU32Bits(out, 0x94a9059eL)
        }
    }.toByteArray()

    /** Canonical exact-lane bytes. */
    @JvmStatic fun canonicalLaneBytes(lane: SccpLaneIdV1): ByteArray = ByteArrayOutputStream().also { out ->
        out.write(1)
        writeBytes(out, canonicalNetworkBytes(lane.source))
        writeBytes(out, canonicalNetworkBytes(lane.target))
    }.toByteArray()

    /** Blake2b-256 of the domain-separated exact lane. */
    @JvmStatic fun laneHash(lane: SccpLaneIdV1): ByteArray = prefixedBlake2b(laneHashPrefix, canonicalLaneBytes(lane))

    /** Lane-bound message identity. The destination binding is deliberately excluded. */
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

    /** Canonical contract-computable source-event preimage after the domain prefix. */
    @JvmStatic fun canonicalSourceEventBytes(
        lane: SccpLaneIdV1,
        messageId: ByteArray,
        payloadHash: ByteArray,
    ): ByteArray {
        val laneHash = laneHash(lane)
        val message = requireHash(messageId, "messageId")
        val payload = requireHash(payloadHash, "payloadHash")
        val roles = listOf(laneHash, message, payload)
        require(roles.indices.all { left ->
            (left + 1 until roles.size).all { right -> !roles[left].contentEquals(roles[right]) }
        }) { "SCCP lane, message, and payload hash roles must be distinct" }
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
    @JvmStatic fun commitment(context: SccpOutboundMessageContextV1, payload: SccpPayloadV1): SccpHubCommitmentV1 {
        val laneHash = laneHash(context.lane)
        val messageId = messageId(context.lane, payload)
        val payloadHash = payloadHash(payload)
        val binding = context.destinationBindingHash()
        val roles = listOf(laneHash, binding, messageId, payloadHash)
        require(roles.all { hash -> hash.any { it.toInt() != 0 } }) { "SCCP hash roles must be nonzero" }
        require(roles.indices.all { left -> (left + 1 until roles.size).all { right -> !roles[left].contentEquals(roles[right]) } }) {
            "SCCP lane, binding, message, and payload hash roles must be distinct"
        }
        return SccpHubCommitmentV1(payload.kind, context, messageId, payloadHash)
    }

    /** Fixed V1 commitment bytes: version, kind, exact profile tags, and three hashes. */
    @JvmStatic fun canonicalCommitmentBytes(commitment: SccpHubCommitmentV1): ByteArray = ByteArrayOutputStream().also { out ->
        out.write(1)
        out.write(commitment.kind.tag)
        out.write(commitment.context.lane.source.tag)
        out.write(commitment.context.lane.target.tag)
        out.write(commitment.context.destinationBindingHash())
        out.write(commitment.messageId())
        out.write(commitment.payloadHash())
    }.toByteArray()

    /** Decode and canonically re-encode a fixed V1 commitment, rejecting unknown or colliding roles. */
    @JvmStatic fun decodeCanonicalCommitment(bytes: ByteArray): SccpHubCommitmentV1 {
        require(bytes.size == 100) { "canonical SCCP commitment must contain 100 bytes" }
        require((bytes[0].toInt() and 0xff) == 1) { "unsupported SCCP commitment version" }
        val kind = SccpHubMessageKindV1.fromTag(bytes[1].toInt() and 0xff)
            ?: throw IllegalArgumentException("unknown SCCP commitment kind")
        val source = SccpNetworkV1.fromTag(bytes[2].toInt() and 0xff)
            ?: throw IllegalArgumentException("unknown SCCP source profile tag")
        val target = SccpNetworkV1.fromTag(bytes[3].toInt() and 0xff)
            ?: throw IllegalArgumentException("unknown SCCP target profile tag")
        val context = SccpOutboundMessageContextV1(SccpLaneIdV1(source, target), bytes.copyOfRange(4, 36))
        val result = SccpHubCommitmentV1(kind, context, bytes.copyOfRange(36, 68), bytes.copyOfRange(68, 100))
        val roles = listOf(laneHash(context.lane), context.destinationBindingHash(), result.messageId(), result.payloadHash())
        require(roles.indices.all { left -> (left + 1 until roles.size).all { right -> !roles[left].contentEquals(roles[right]) } }) {
            "SCCP commitment hash roles must be distinct"
        }
        require(canonicalCommitmentBytes(result).contentEquals(bytes)) { "non-canonical SCCP commitment" }
        return result
    }

    /** Domain-separated leaf/root for an empty Merkle path. */
    @JvmStatic fun commitmentRoot(commitment: SccpHubCommitmentV1): ByteArray =
        prefixedBlake2b(leafHashPrefix, canonicalCommitmentBytes(commitment))

    /** Strict lowercase, prefixless hexadecimal decoder used by shared vector fixtures. */
    @JvmStatic fun decodeLowerHex(value: String): ByteArray = hex(value)

    /** Lowercase, prefixless hexadecimal encoder. */
    @JvmStatic fun encodeLowerHex(value: ByteArray): String = value.joinToString("") { "%02x".format(it.toInt() and 0xff) }
}

private fun requireDomain(value: Int, field: String) {
    require(value in 0..5) { "$field must be a supported SCCP domain" }
}

private fun requireExternalDomain(value: Int, field: String) {
    requireDomain(value, field)
    require(value != 0) { "$field must be external" }
}

private fun accountCodec(domain: Int): Int = when (domain) {
    0 -> 1
    1, 2 -> 2
    3 -> 3
    4 -> 4
    5 -> 5
    else -> throw IllegalArgumentException("unsupported SCCP domain")
}

private fun requireHash(value: ByteArray, field: String): ByteArray {
    require(value.size == 32) { "$field must contain 32 bytes" }
    require(value.any { it.toInt() != 0 }) { "$field must be nonzero" }
    return value.copyOf()
}

private fun requireUnsigned(value: BigInteger, bits: Int, field: String): BigInteger {
    require(value.signum() >= 0 && value.bitLength() <= bits) { "$field must fit u$bits" }
    return value
}

private fun requireFixedAscii(value: ByteArray, field: String): ByteArray {
    require(value.size == 32) { "$field must contain 32 bytes" }
    val end = value.indexOf(0).let { if (it < 0) value.size else it }
    require(value.copyOfRange(0, end).any { it.toInt() != 0 }) { "$field must be nonempty" }
    require(value.all { (it.toInt() and 0xff) < 0x80 }) { "$field must be ASCII" }
    return value.copyOf()
}

private fun requireCodecValue(codec: Int, value: ByteArray, field: String): ByteArray {
    require(value.isNotEmpty()) { "$field must be nonempty" }
    val valid = when (codec) {
        1 -> value.size <= 256 && value.all { (it.toInt() and 0xff) in 0x21..0x7e }
        2 -> value.size == 20 && value.any { it.toInt() != 0 }
        3 -> value.size == 32 && value.any { it.toInt() != 0 }
        4 -> value.size == 36 &&
            ByteBuffer.wrap(value, 0, 4).order(ByteOrder.LITTLE_ENDIAN).int in setOf(-1, 0) &&
            value.copyOfRange(4, 36).any { it.toInt() != 0 }
        5 -> value.size == 21 && (value[0].toInt() and 0xff) == 0x41 &&
            value.copyOfRange(1, 21).any { it.toInt() != 0 }
        6 -> value.size == 32 && value.any { it.toInt() != 0 }
        else -> false
    }
    require(valid) { "$field does not match SCCP codec $codec" }
    return value.copyOf()
}

private fun writeU32(out: ByteArrayOutputStream, value: Int) {
    require(value >= 0) { "u32 value must be non-negative" }
    writeU32Bits(out, value.toLong())
}

private fun writeU32Bits(out: ByteArrayOutputStream, value: Long) {
    require(value in 0..0xffff_ffffL) { "value must fit u32" }
    repeat(4) { shift -> out.write(((value ushr (shift * 8)) and 0xff).toInt()) }
}

private fun writeI32(out: ByteArrayOutputStream, value: Int) {
    val bytes = ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN).putInt(value).array()
    out.write(bytes)
}

private fun writeUnsignedLe(out: ByteArrayOutputStream, value: BigInteger, size: Int) {
    require(value.signum() >= 0 && value.bitLength() <= size * 8) { "unsigned integer does not fit" }
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

private fun prefixedBlake2b(prefix: ByteArray, payload: ByteArray): ByteArray = Blake2b.digest256(prefix + payload)

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
    return ByteArray(value.length / 2) { index -> value.substring(index * 2, index * 2 + 2).toInt(16).toByte() }
}
