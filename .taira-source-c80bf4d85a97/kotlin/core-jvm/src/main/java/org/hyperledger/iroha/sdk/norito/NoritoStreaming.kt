// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

@file:Suppress("TooManyFunctions")

package org.hyperledger.iroha.sdk.norito

import java.math.BigInteger
import java.util.Optional

private val UINT128_MAX: BigInteger = BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE)

/** Norito streaming helpers mirroring the Rust codec surface. */
object NoritoStreaming {

    const val HASH_LEN = 32
    const val SIGNATURE_LEN = 64

    private val UINT8: TypeAdapter<Long> = NoritoAdapters.uint(8)
    private val UINT16: TypeAdapter<Long> = NoritoAdapters.uint(16)
    private val UINT32: TypeAdapter<Long> = NoritoAdapters.uint(32)
    private val UINT64: TypeAdapter<Long> = NoritoAdapters.uint(64)
    private val SINT16: TypeAdapter<Long> = NoritoAdapters.sint(16)
    private val SINT32: TypeAdapter<Long> = NoritoAdapters.sint(32)
    private val BOOL: TypeAdapter<Boolean> = NoritoAdapters.boolAdapter()
    private val HASH: TypeAdapter<ByteArray> = NoritoAdapters.fixedBytes(HASH_LEN)
    private val SIGNATURE: TypeAdapter<ByteArray> = NoritoAdapters.fixedBytes(SIGNATURE_LEN)
    private val BYTES: TypeAdapter<ByteArray> = NoritoAdapters.bytesAdapter()
    private val STRING: TypeAdapter<String> = NoritoAdapters.stringAdapter()
    private val STRING_LIST: TypeAdapter<List<String>> = NoritoAdapters.sequence(STRING)
    private val INT16_LIST: TypeAdapter<List<Long>> = NoritoAdapters.sequence(SINT16)
    private val INT32_LIST: TypeAdapter<List<Long>> = NoritoAdapters.sequence(SINT32)
    private val UINT32_LIST: TypeAdapter<List<Long>> = NoritoAdapters.sequence(UINT32)

    private fun cloneBytes(value: ByteArray): ByteArray = value.clone()

    private fun cloneOptionalBytes(value: Optional<ByteArray>): Optional<ByteArray> =
        value.map { it.clone() }

    private fun toLittleEndian(value: BigInteger, length: Int, max: BigInteger): ByteArray {
        require(value.signum() >= 0 && value <= max) {
            "value out of range for unsigned ${length * 8}-bit integer"
        }
        val be = value.toByteArray()
        var offset = 0
        var beLen = be.size
        if (beLen > length) {
            if (be[0].toInt() == 0 && beLen == length + 1) {
                offset = 1
                beLen -= 1
            } else {
                throw IllegalArgumentException("value does not fit in ${length * 8} bits")
            }
        }
        val le = ByteArray(length)
        for (i in 0 until beLen) {
            le[i] = be[offset + beLen - 1 - i]
        }
        return le
    }

    private fun fromLittleEndian(data: ByteArray): BigInteger {
        val be = ByteArray(data.size)
        for (i in data.indices) {
            be[be.size - 1 - i] = data[i]
        }
        return BigInteger(1, be)
    }

    // --- Value types (records) ---

    data class ProfileId(@JvmField val value: Int) {
        init {
            require(value in 0..0xFFFF) { "ProfileId must fit in u16" }
        }
    }

    data class CapabilityFlags(@JvmField val bits: Long) {
        init {
            require(bits in 0..0xFFFF_FFFFL) { "CapabilityFlags must fit in u32" }
        }
    }

    data class TicketCapabilities(@JvmField val bits: Int) {
        init {
            require(bits.toLong() and 0xFFFF_FFFFL == bits.toLong()) { "TicketCapabilities must fit in u32" }
        }

        fun contains(mask: Int): Boolean = (bits and mask) == mask
        fun insert(mask: Int): TicketCapabilities = TicketCapabilities(bits or mask)
        fun remove(mask: Int): TicketCapabilities = TicketCapabilities(bits and mask.inv())

        companion object {
            const val LIVE = 1 shl 0
            const val VOD = 1 shl 1
            const val PREMIUM_PROFILE = 1 shl 2
            const val HDR = 1 shl 3
            const val SPATIAL_AUDIO = 1 shl 4
        }
    }

    data class TicketPolicy(
        @JvmField val maxRelays: Int,
        @JvmField val allowedRegions: List<String>,
        @JvmField val maxBandwidthKbps: Optional<Long>,
    ) {
        init {
            require(maxRelays in 0..0xFFFF) { "maxRelays must fit in u16" }
            for (region in allowedRegions) {
                require(region.isNotEmpty()) { "region codes must not be empty" }
            }
            maxBandwidthKbps.ifPresent { value ->
                require(value in 0..0xFFFF_FFFFL) { "maxBandwidthKbps must fit in u32" }
            }
        }
    }

    class StreamingTicket(
        ticketId: ByteArray,
        @JvmField val owner: String,
        @JvmField val dsid: Long,
        @JvmField val laneId: Int,
        @JvmField val settlementBucket: Long,
        @JvmField val startSlot: Long,
        @JvmField val expireSlot: Long,
        @JvmField val prepaidTeu: BigInteger,
        @JvmField val chunkTeu: Long,
        @JvmField val fanoutQuota: Int,
        keyCommitment: ByteArray,
        @JvmField val nonce: Long,
        contractSignature: ByteArray,
        commitment: ByteArray,
        nullifier: ByteArray,
        proofId: ByteArray,
        @JvmField val issuedAt: Long,
        @JvmField val expiresAt: Long,
        @JvmField val policy: Optional<TicketPolicy>,
        @JvmField val capabilities: TicketCapabilities,
    ) {
        private val _ticketId: ByteArray = ticketId.copyOf()
        private val _keyCommitment: ByteArray = keyCommitment.copyOf()
        private val _contractSignature: ByteArray = contractSignature.copyOf()
        private val _commitment: ByteArray = commitment.copyOf()
        private val _nullifier: ByteArray = nullifier.copyOf()
        private val _proofId: ByteArray = proofId.copyOf()

        val ticketId: ByteArray get() = _ticketId.copyOf()
        val keyCommitment: ByteArray get() = _keyCommitment.copyOf()
        val contractSignature: ByteArray get() = _contractSignature.copyOf()
        val commitment: ByteArray get() = _commitment.copyOf()
        val nullifier: ByteArray get() = _nullifier.copyOf()
        val proofId: ByteArray get() = _proofId.copyOf()

        init {
            require(_ticketId.size == HASH_LEN) { "ticketId must be 32 bytes" }
            require(dsid >= 0) { "dsid must be non-negative" }
            require(laneId in 0..0xFF) { "laneId must fit in u8" }
            require(settlementBucket >= 0) { "settlementBucket must be non-negative" }
            require(startSlot >= 0 && expireSlot >= 0) { "slot values must be non-negative" }
            require(chunkTeu in 0..0xFFFF_FFFFL) { "chunkTeu must fit in u32" }
            require(fanoutQuota in 0..0xFFFF) { "fanoutQuota must fit in u16" }
            require(_keyCommitment.size == HASH_LEN) { "keyCommitment must be 32 bytes" }
            require(nonce >= 0) { "nonce must be non-negative" }
            require(_contractSignature.size == SIGNATURE_LEN) { "contractSignature must be 64 bytes" }
            require(_commitment.size == HASH_LEN) { "commitment must be 32 bytes" }
            require(_nullifier.size == HASH_LEN) { "nullifier must be 32 bytes" }
            require(_proofId.size == HASH_LEN) { "proofId must be 32 bytes" }
            require(issuedAt >= 0 && expiresAt >= 0) { "timestamp must be non-negative" }
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is StreamingTicket) return false
            return _ticketId.contentEquals(other._ticketId)
                && owner == other.owner
                && dsid == other.dsid
                && laneId == other.laneId
                && settlementBucket == other.settlementBucket
                && startSlot == other.startSlot
                && expireSlot == other.expireSlot
                && prepaidTeu == other.prepaidTeu
                && chunkTeu == other.chunkTeu
                && fanoutQuota == other.fanoutQuota
                && _keyCommitment.contentEquals(other._keyCommitment)
                && nonce == other.nonce
                && _contractSignature.contentEquals(other._contractSignature)
                && _commitment.contentEquals(other._commitment)
                && _nullifier.contentEquals(other._nullifier)
                && _proofId.contentEquals(other._proofId)
                && issuedAt == other.issuedAt
                && expiresAt == other.expiresAt
                && policy == other.policy
                && capabilities == other.capabilities
        }

        override fun hashCode(): Int {
            var result = _ticketId.contentHashCode()
            result = 31 * result + owner.hashCode()
            result = 31 * result + dsid.hashCode()
            result = 31 * result + laneId.hashCode()
            result = 31 * result + settlementBucket.hashCode()
            result = 31 * result + startSlot.hashCode()
            result = 31 * result + expireSlot.hashCode()
            result = 31 * result + prepaidTeu.hashCode()
            result = 31 * result + chunkTeu.hashCode()
            result = 31 * result + fanoutQuota.hashCode()
            result = 31 * result + _keyCommitment.contentHashCode()
            result = 31 * result + nonce.hashCode()
            result = 31 * result + _contractSignature.contentHashCode()
            result = 31 * result + _commitment.contentHashCode()
            result = 31 * result + _nullifier.contentHashCode()
            result = 31 * result + _proofId.contentHashCode()
            result = 31 * result + issuedAt.hashCode()
            result = 31 * result + expiresAt.hashCode()
            result = 31 * result + policy.hashCode()
            result = 31 * result + capabilities.hashCode()
            return result
        }
    }

    class TicketRevocation(
        ticketId: ByteArray,
        nullifier: ByteArray,
        @JvmField val reasonCode: Int,
        revocationSignature: ByteArray,
    ) {
        private val _ticketId: ByteArray = ticketId.copyOf()
        private val _nullifier: ByteArray = nullifier.copyOf()
        private val _revocationSignature: ByteArray = revocationSignature.copyOf()

        val ticketId: ByteArray get() = _ticketId.copyOf()
        val nullifier: ByteArray get() = _nullifier.copyOf()
        val revocationSignature: ByteArray get() = _revocationSignature.copyOf()

        init {
            require(_ticketId.size == HASH_LEN) { "ticketId must be 32 bytes" }
            require(_nullifier.size == HASH_LEN) { "nullifier must be 32 bytes" }
            require(_revocationSignature.size == SIGNATURE_LEN) { "revocationSignature must be 64 bytes" }
            require(reasonCode in 0..0xFFFF) { "reasonCode must fit in u16" }
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is TicketRevocation) return false
            return _ticketId.contentEquals(other._ticketId)
                && _nullifier.contentEquals(other._nullifier)
                && reasonCode == other.reasonCode
                && _revocationSignature.contentEquals(other._revocationSignature)
        }

        override fun hashCode(): Int {
            var result = _ticketId.contentHashCode()
            result = 31 * result + _nullifier.contentHashCode()
            result = 31 * result + reasonCode.hashCode()
            result = 31 * result + _revocationSignature.contentHashCode()
            return result
        }
    }

    data class PrivacyCapabilities(@JvmField val bits: Long) {
        init {
            require(bits in 0..0xFFFF_FFFFL) { "PrivacyCapabilities must fit in u32" }
        }
    }

    enum class HpkeSuite(@JvmField val bit: Int, @JvmField val suiteId: Int) {
        KYBER768_AUTH_PSK(0, 0x0001),
        KYBER1024_AUTH_PSK(1, 0x0002),
    }

    data class HpkeSuiteMask(@JvmField val bits: Int) {
        init {
            require(bits in 0..0xFFFF) { "HpkeSuiteMask must fit in u16" }
        }

        fun contains(suite: HpkeSuite): Boolean = (bits and (1 shl suite.bit)) != 0
    }

    enum class PrivacyBucketGranularity { STANDARD_V1 }
    enum class FecScheme { RS12_10, RS_WIN14_10, RS18_14 }
    enum class StorageClass { EPHEMERAL, PERMANENT }
    enum class CapabilityRole { PUBLISHER, VIEWER }
    enum class AudioLayout { MONO, STEREO, FIRST_ORDER_AMBISONICS }
    enum class ErrorCode { UNKNOWN_CHUNK, ACCESS_DENIED, RATE_LIMITED, PROTOCOL_VIOLATION }
    enum class ResolutionKind { R720P, R1080P, R1440P, R2160P, CUSTOM }

    enum class ControlFrameVariant {
        MANIFEST_ANNOUNCE, CHUNK_REQUEST, CHUNK_ACKNOWLEDGE, TRANSPORT_CAPABILITIES,
        CAPABILITY_REPORT, CAPABILITY_ACK, FEEDBACK_HINT, RECEIVER_REPORT, KEY_UPDATE,
        CONTENT_KEY_UPDATE, PRIVACY_ROUTE_UPDATE, PRIVACY_ROUTE_ACK, ERROR,
    }

    sealed interface EncryptionSuite

    class X25519ChaCha20Poly1305Suite(fingerprint: ByteArray) : EncryptionSuite {
        private val _fingerprint: ByteArray = fingerprint.copyOf()

        val fingerprint: ByteArray get() = _fingerprint.copyOf()

        init {
            require(_fingerprint.size == HASH_LEN) { "fingerprint must be 32 bytes" }
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is X25519ChaCha20Poly1305Suite) return false
            return _fingerprint.contentEquals(other._fingerprint)
        }

        override fun hashCode(): Int = _fingerprint.contentHashCode()
    }

    class Kyber768XChaCha20Poly1305Suite(fingerprint: ByteArray) : EncryptionSuite {
        private val _fingerprint: ByteArray = fingerprint.copyOf()

        val fingerprint: ByteArray get() = _fingerprint.copyOf()

        init {
            require(_fingerprint.size == HASH_LEN) { "fingerprint must be 32 bytes" }
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is Kyber768XChaCha20Poly1305Suite) return false
            return _fingerprint.contentEquals(other._fingerprint)
        }

        override fun hashCode(): Int = _fingerprint.contentHashCode()
    }

    data class TransportCapabilities(
        @JvmField val hpkeSuites: HpkeSuiteMask,
        @JvmField val supportsDatagram: Boolean,
        @JvmField val maxSegmentDatagramSize: Long,
        @JvmField val fecFeedbackIntervalMs: Long,
        @JvmField val privacyBucketGranularity: PrivacyBucketGranularity,
    ) {
        init {
            require(maxSegmentDatagramSize in 0..0xFFFFL) { "maxSegmentDatagramSize must fit in u16" }
            require(fecFeedbackIntervalMs in 0..0xFFFFL) { "fecFeedbackIntervalMs must fit in u16" }
        }
    }

    class StreamMetadata(
        @JvmField val title: String,
        @JvmField val description: Optional<String>,
        accessPolicyId: Optional<ByteArray>,
        @JvmField val tags: List<String>,
    ) {
        private val _accessPolicyId: Optional<ByteArray> = accessPolicyId.map { it.copyOf() }

        val accessPolicyId: Optional<ByteArray> get() = _accessPolicyId.map { it.copyOf() }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is StreamMetadata) return false
            if (title != other.title) return false
            if (description != other.description) return false
            if (!_accessPolicyId.isPresent && !other._accessPolicyId.isPresent) {
                return tags == other.tags
            }
            if (_accessPolicyId.isPresent != other._accessPolicyId.isPresent) return false
            if (!_accessPolicyId.get().contentEquals(other._accessPolicyId.get())) return false
            return tags == other.tags
        }

        override fun hashCode(): Int {
            var result = title.hashCode()
            result = 31 * result + description.hashCode()
            result = 31 * result + _accessPolicyId.map { it.contentHashCode() }.orElse(0)
            result = 31 * result + tags.hashCode()
            return result
        }
    }

    class PrivacyRelay(
        relayId: ByteArray,
        @JvmField val endpoint: String,
        keyFingerprint: ByteArray,
        @JvmField val capabilities: PrivacyCapabilities,
    ) {
        private val _relayId: ByteArray = relayId.copyOf()
        private val _keyFingerprint: ByteArray = keyFingerprint.copyOf()

        val relayId: ByteArray get() = _relayId.copyOf()
        val keyFingerprint: ByteArray get() = _keyFingerprint.copyOf()

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is PrivacyRelay) return false
            return _relayId.contentEquals(other._relayId)
                && endpoint == other.endpoint
                && _keyFingerprint.contentEquals(other._keyFingerprint)
                && capabilities == other.capabilities
        }

        override fun hashCode(): Int {
            var result = _relayId.contentHashCode()
            result = 31 * result + endpoint.hashCode()
            result = 31 * result + _keyFingerprint.contentHashCode()
            result = 31 * result + capabilities.hashCode()
            return result
        }
    }

    enum class SoranetAccessKind { READ_ONLY, AUTHENTICATED }

    class SoranetRoute(
        channelId: ByteArray,
        @JvmField val exitMultiaddr: String,
        @JvmField val paddingBudgetMs: Int?,
        @JvmField val accessKind: SoranetAccessKind,
    ) {
        private val _channelId: ByteArray = channelId.copyOf()

        val channelId: ByteArray get() = _channelId.copyOf()

        init {
            if (paddingBudgetMs != null) {
                require(paddingBudgetMs >= 0) { "paddingBudgetMs must be non-negative when set" }
            }
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is SoranetRoute) return false
            return _channelId.contentEquals(other._channelId)
                && exitMultiaddr == other.exitMultiaddr
                && paddingBudgetMs == other.paddingBudgetMs
                && accessKind == other.accessKind
        }

        override fun hashCode(): Int {
            var result = _channelId.contentHashCode()
            result = 31 * result + exitMultiaddr.hashCode()
            result = 31 * result + (paddingBudgetMs?.hashCode() ?: 0)
            result = 31 * result + accessKind.hashCode()
            return result
        }
    }

    class PrivacyRoute(
        routeId: ByteArray,
        @JvmField val entry: PrivacyRelay,
        @JvmField val exit: PrivacyRelay,
        ticketEntry: ByteArray,
        ticketExit: ByteArray,
        @JvmField val expirySegment: Long,
        @JvmField val soranet: SoranetRoute?,
    ) {
        private val _routeId: ByteArray = routeId.copyOf()
        private val _ticketEntry: ByteArray = ticketEntry.copyOf()
        private val _ticketExit: ByteArray = ticketExit.copyOf()

        val routeId: ByteArray get() = _routeId.copyOf()
        val ticketEntry: ByteArray get() = _ticketEntry.copyOf()
        val ticketExit: ByteArray get() = _ticketExit.copyOf()

        init {
            require(expirySegment >= 0) { "expirySegment must be non-negative" }
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is PrivacyRoute) return false
            return _routeId.contentEquals(other._routeId)
                && entry == other.entry
                && exit == other.exit
                && _ticketEntry.contentEquals(other._ticketEntry)
                && _ticketExit.contentEquals(other._ticketExit)
                && expirySegment == other.expirySegment
                && soranet == other.soranet
        }

        override fun hashCode(): Int {
            var result = _routeId.contentHashCode()
            result = 31 * result + entry.hashCode()
            result = 31 * result + exit.hashCode()
            result = 31 * result + _ticketEntry.contentHashCode()
            result = 31 * result + _ticketExit.contentHashCode()
            result = 31 * result + expirySegment.hashCode()
            result = 31 * result + (soranet?.hashCode() ?: 0)
            return result
        }
    }

    class NeuralBundle(
        @JvmField val bundleId: String,
        weightsSha256: ByteArray,
        @JvmField val activationScale: List<Long>,
        @JvmField val bias: List<Long>,
        metadataSignature: ByteArray,
        metalShaderSha256: Optional<ByteArray>,
        cudaPtxSha256: Optional<ByteArray>,
    ) {
        private val _weightsSha256: ByteArray = weightsSha256.copyOf()
        private val _metadataSignature: ByteArray = metadataSignature.copyOf()
        private val _metalShaderSha256: Optional<ByteArray> = metalShaderSha256.map { it.copyOf() }
        private val _cudaPtxSha256: Optional<ByteArray> = cudaPtxSha256.map { it.copyOf() }

        val weightsSha256: ByteArray get() = _weightsSha256.copyOf()
        val metadataSignature: ByteArray get() = _metadataSignature.copyOf()
        val metalShaderSha256: Optional<ByteArray> get() = _metalShaderSha256.map { it.copyOf() }
        val cudaPtxSha256: Optional<ByteArray> get() = _cudaPtxSha256.map { it.copyOf() }

        init {
            for (value in activationScale) {
                require(value in -0x8000L..0x7FFFL) { "activationScale value out of i16 range" }
            }
            for (value in bias) {
                require(value in -0x8000_0000L..0x7FFF_FFFFL) { "bias value out of i32 range" }
            }
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is NeuralBundle) return false
            if (bundleId != other.bundleId) return false
            if (!_weightsSha256.contentEquals(other._weightsSha256)) return false
            if (activationScale != other.activationScale) return false
            if (bias != other.bias) return false
            if (!_metadataSignature.contentEquals(other._metadataSignature)) return false
            if (!optionalBytesEquals(_metalShaderSha256, other._metalShaderSha256)) return false
            if (!optionalBytesEquals(_cudaPtxSha256, other._cudaPtxSha256)) return false
            return true
        }

        override fun hashCode(): Int {
            var result = bundleId.hashCode()
            result = 31 * result + _weightsSha256.contentHashCode()
            result = 31 * result + activationScale.hashCode()
            result = 31 * result + bias.hashCode()
            result = 31 * result + _metadataSignature.contentHashCode()
            result = 31 * result + _metalShaderSha256.map { it.contentHashCode() }.orElse(0)
            result = 31 * result + _cudaPtxSha256.map { it.contentHashCode() }.orElse(0)
            return result
        }

        private fun optionalBytesEquals(a: Optional<ByteArray>, b: Optional<ByteArray>): Boolean {
            if (!a.isPresent && !b.isPresent) return true
            if (a.isPresent != b.isPresent) return false
            return a.get().contentEquals(b.get())
        }
    }

    class ChunkDescriptor(
        @JvmField val chunkId: Long,
        @JvmField val offset: Long,
        @JvmField val length: Long,
        commitment: ByteArray,
        @JvmField val parity: Boolean,
    ) {
        private val _commitment: ByteArray = commitment.copyOf()

        val commitment: ByteArray get() = _commitment.copyOf()

        init {
            require(chunkId in 0..0xFFFFL) { "chunkId must fit in u16" }
            require(offset in 0..0xFFFF_FFFFL) { "offset must fit in u32" }
            require(length in 0..0xFFFF_FFFFL) { "length must fit in u32" }
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is ChunkDescriptor) return false
            return chunkId == other.chunkId
                && offset == other.offset
                && length == other.length
                && parity == other.parity
                && _commitment.contentEquals(other._commitment)
        }

        override fun hashCode(): Int {
            var result = chunkId.hashCode()
            result = 31 * result + offset.hashCode()
            result = 31 * result + length.hashCode()
            result = 31 * result + parity.hashCode()
            result = 31 * result + _commitment.contentHashCode()
            return result
        }
    }

    data class ResolutionCustom(@JvmField val width: Int, @JvmField val height: Int) {
        init {
            require(width in 0..0xFFFF) { "width must fit in u16" }
            require(height in 0..0xFFFF) { "height must fit in u16" }
        }
    }

    data class Resolution(@JvmField val kind: ResolutionKind, @JvmField val custom: Optional<ResolutionCustom>) {
        init {
            if (kind == ResolutionKind.CUSTOM) {
                require(custom.isPresent) { "custom resolution requires parameters" }
            } else {
                require(!custom.isPresent) { "non-custom resolution must not carry parameters" }
            }
        }
    }

    class ManifestV1(
        streamId: ByteArray,
        @JvmField val protocolVersion: Long,
        @JvmField val segmentNumber: Long,
        @JvmField val publishedAt: Long,
        @JvmField val profile: ProfileId,
        @JvmField val daEndpoint: String,
        chunkRoot: ByteArray,
        @JvmField val contentKeyId: Long,
        nonceSalt: ByteArray,
        @JvmField val chunkDescriptors: List<ChunkDescriptor>,
        transportCapabilitiesHash: ByteArray,
        @JvmField val encryptionSuite: EncryptionSuite,
        @JvmField val fecSuite: FecScheme,
        @JvmField val privacyRoutes: List<PrivacyRoute>,
        @JvmField val neuralBundle: Optional<NeuralBundle>,
        @JvmField val publicMetadata: StreamMetadata,
        @JvmField val capabilities: CapabilityFlags,
        signature: ByteArray,
    ) {
        private val _streamId: ByteArray = streamId.copyOf()
        private val _chunkRoot: ByteArray = chunkRoot.copyOf()
        private val _nonceSalt: ByteArray = nonceSalt.copyOf()
        private val _transportCapabilitiesHash: ByteArray = transportCapabilitiesHash.copyOf()
        private val _signature: ByteArray = signature.copyOf()

        val streamId: ByteArray get() = _streamId.copyOf()
        val chunkRoot: ByteArray get() = _chunkRoot.copyOf()
        val nonceSalt: ByteArray get() = _nonceSalt.copyOf()
        val transportCapabilitiesHash: ByteArray get() = _transportCapabilitiesHash.copyOf()
        val signature: ByteArray get() = _signature.copyOf()

        init {
            require(protocolVersion in 0..0xFFFFL) { "protocolVersion must fit in u16" }
            require(contentKeyId >= 0) { "contentKeyId must be non-negative" }
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is ManifestV1) return false
            return _streamId.contentEquals(other._streamId)
                && protocolVersion == other.protocolVersion
                && segmentNumber == other.segmentNumber
                && publishedAt == other.publishedAt
                && profile == other.profile
                && daEndpoint == other.daEndpoint
                && _chunkRoot.contentEquals(other._chunkRoot)
                && contentKeyId == other.contentKeyId
                && _nonceSalt.contentEquals(other._nonceSalt)
                && chunkDescriptors == other.chunkDescriptors
                && _transportCapabilitiesHash.contentEquals(other._transportCapabilitiesHash)
                && encryptionSuite == other.encryptionSuite
                && fecSuite == other.fecSuite
                && privacyRoutes == other.privacyRoutes
                && neuralBundle == other.neuralBundle
                && publicMetadata == other.publicMetadata
                && capabilities == other.capabilities
                && _signature.contentEquals(other._signature)
        }

        override fun hashCode(): Int {
            var result = _streamId.contentHashCode()
            result = 31 * result + protocolVersion.hashCode()
            result = 31 * result + segmentNumber.hashCode()
            result = 31 * result + publishedAt.hashCode()
            result = 31 * result + profile.hashCode()
            result = 31 * result + daEndpoint.hashCode()
            result = 31 * result + _chunkRoot.contentHashCode()
            result = 31 * result + contentKeyId.hashCode()
            result = 31 * result + _nonceSalt.contentHashCode()
            result = 31 * result + chunkDescriptors.hashCode()
            result = 31 * result + _transportCapabilitiesHash.contentHashCode()
            result = 31 * result + encryptionSuite.hashCode()
            result = 31 * result + fecSuite.hashCode()
            result = 31 * result + privacyRoutes.hashCode()
            result = 31 * result + neuralBundle.hashCode()
            result = 31 * result + publicMetadata.hashCode()
            result = 31 * result + capabilities.hashCode()
            result = 31 * result + _signature.contentHashCode()
            return result
        }
    }

    // --- Control Frame types ---

    sealed interface ControlFramePayload

    data class ManifestAnnounceFrame(@JvmField val manifest: ManifestV1) : ControlFramePayload

    data class ChunkRequestFrame(@JvmField val segment: Long, @JvmField val chunkId: Long) : ControlFramePayload {
        init {
            require(chunkId in 0..0xFFFFL) { "chunkId must fit in u16" }
            require(segment >= 0) { "segment must be non-negative" }
        }
    }

    data class ChunkAcknowledgeFrame(@JvmField val segment: Long, @JvmField val chunkId: Long) : ControlFramePayload {
        init {
            require(chunkId in 0..0xFFFFL) { "chunkId must fit in u16" }
            require(segment >= 0) { "segment must be non-negative" }
        }
    }

    data class TransportCapabilitiesFrame(
        @JvmField val endpointRole: CapabilityRole,
        @JvmField val capabilities: TransportCapabilities,
    ) : ControlFramePayload

    class CapabilityReport(
        streamId: ByteArray,
        @JvmField val endpointRole: CapabilityRole,
        @JvmField val protocolVersion: Long,
        @JvmField val maxResolution: Resolution,
        @JvmField val hdrSupported: Boolean,
        @JvmField val captureHdr: Boolean,
        @JvmField val neuralBundles: List<String>,
        @JvmField val audioCaps: AudioCapability,
        @JvmField val featureBits: CapabilityFlags,
        @JvmField val maxDatagramSize: Long,
        @JvmField val dplpmtud: Boolean,
    ) : ControlFramePayload {
        private val _streamId: ByteArray = streamId.copyOf()

        val streamId: ByteArray get() = _streamId.copyOf()

        init {
            require(protocolVersion in 0..0xFFFFL) { "protocolVersion must fit in u16" }
            require(maxDatagramSize in 0..0xFFFFL) { "maxDatagramSize must fit in u16" }
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is CapabilityReport) return false
            return _streamId.contentEquals(other._streamId)
                && endpointRole == other.endpointRole
                && protocolVersion == other.protocolVersion
                && maxResolution == other.maxResolution
                && hdrSupported == other.hdrSupported
                && captureHdr == other.captureHdr
                && neuralBundles == other.neuralBundles
                && audioCaps == other.audioCaps
                && featureBits == other.featureBits
                && maxDatagramSize == other.maxDatagramSize
                && dplpmtud == other.dplpmtud
        }

        override fun hashCode(): Int {
            var result = _streamId.contentHashCode()
            result = 31 * result + endpointRole.hashCode()
            result = 31 * result + protocolVersion.hashCode()
            result = 31 * result + maxResolution.hashCode()
            result = 31 * result + hdrSupported.hashCode()
            result = 31 * result + captureHdr.hashCode()
            result = 31 * result + neuralBundles.hashCode()
            result = 31 * result + audioCaps.hashCode()
            result = 31 * result + featureBits.hashCode()
            result = 31 * result + maxDatagramSize.hashCode()
            result = 31 * result + dplpmtud.hashCode()
            return result
        }
    }

    class CapabilityAck(
        streamId: ByteArray,
        @JvmField val acceptedVersion: Long,
        @JvmField val negotiatedFeatures: CapabilityFlags,
        @JvmField val maxDatagramSize: Long,
        @JvmField val dplpmtud: Boolean,
    ) : ControlFramePayload {
        private val _streamId: ByteArray = streamId.copyOf()

        val streamId: ByteArray get() = _streamId.copyOf()

        init {
            require(acceptedVersion in 0..0xFFFFL) { "acceptedVersion must fit in u16" }
            require(maxDatagramSize in 0..0xFFFFL) { "maxDatagramSize must fit in u16" }
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is CapabilityAck) return false
            return _streamId.contentEquals(other._streamId)
                && acceptedVersion == other.acceptedVersion
                && negotiatedFeatures == other.negotiatedFeatures
                && maxDatagramSize == other.maxDatagramSize
                && dplpmtud == other.dplpmtud
        }

        override fun hashCode(): Int {
            var result = _streamId.contentHashCode()
            result = 31 * result + acceptedVersion.hashCode()
            result = 31 * result + negotiatedFeatures.hashCode()
            result = 31 * result + maxDatagramSize.hashCode()
            result = 31 * result + dplpmtud.hashCode()
            return result
        }
    }

    data class AudioCapability(
        @JvmField val sampleRates: List<Long>,
        @JvmField val ambisonics: Boolean,
        @JvmField val maxChannels: Long,
    ) {
        init {
            require(maxChannels in 0..0xFFL) { "maxChannels must fit in u8" }
        }
    }

    class AudioFrame(
        @JvmField val sequence: Long,
        @JvmField val timestampNs: Long,
        @JvmField val fecLevel: Long,
        @JvmField val channelLayout: AudioLayout,
        payload: ByteArray,
    ) {
        private val _payload: ByteArray = payload.copyOf()

        val payload: ByteArray get() = _payload.copyOf()

        init {
            require(sequence >= 0) { "sequence must be non-negative" }
            require(fecLevel in 0..0xFFL) { "fecLevel must fit in u8" }
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is AudioFrame) return false
            return sequence == other.sequence
                && timestampNs == other.timestampNs
                && fecLevel == other.fecLevel
                && channelLayout == other.channelLayout
                && _payload.contentEquals(other._payload)
        }

        override fun hashCode(): Int {
            var result = sequence.hashCode()
            result = 31 * result + timestampNs.hashCode()
            result = 31 * result + fecLevel.hashCode()
            result = 31 * result + channelLayout.hashCode()
            result = 31 * result + _payload.contentHashCode()
            return result
        }
    }

    class FeedbackHintFrame(
        streamId: ByteArray,
        @JvmField val lossEwmaQ16: Long,
        @JvmField val latencyGradientQ16: Long,
        @JvmField val observedRttMs: Long,
        @JvmField val reportIntervalMs: Long,
        @JvmField val parityChunks: Long,
    ) : ControlFramePayload {
        private val _streamId: ByteArray = streamId.copyOf()

        val streamId: ByteArray get() = _streamId.copyOf()

        init {
            require(lossEwmaQ16 in 0..0xFFFF_FFFFL) { "lossEwmaQ16 must fit in u32" }
            require(latencyGradientQ16 in Int.MIN_VALUE.toLong()..Int.MAX_VALUE.toLong()) {
                "latencyGradientQ16 must fit in i32"
            }
            require(observedRttMs in 0..0xFFFFL) { "observedRttMs must fit in u16" }
            require(reportIntervalMs in 0..0xFFFFL) { "reportIntervalMs must fit in u16" }
            require(parityChunks in 0..0xFFL) { "parityChunks must fit in u8" }
        }
    }

    data class SyncDiagnostics(
        @JvmField val windowMs: Long,
        @JvmField val samples: Long,
        @JvmField val avgAudioJitterMs: Long,
        @JvmField val maxAudioJitterMs: Long,
        @JvmField val avgAvDriftMs: Long,
        @JvmField val maxAvDriftMs: Long,
        @JvmField val ewmaAvDriftMs: Long,
        @JvmField val violationCount: Long,
    ) {
        init {
            require(windowMs in 0..0xFFFFL) { "windowMs must fit in u16" }
            require(samples in 0..0xFFFFL) { "samples must fit in u16" }
            require(avgAudioJitterMs in 0..0xFFFFL) { "avgAudioJitterMs must fit in u16" }
            require(maxAudioJitterMs in 0..0xFFFFL) { "maxAudioJitterMs must fit in u16" }
            require(avgAvDriftMs in -0x8000L..0x7FFFL) { "avgAvDriftMs must fit in i16" }
            require(maxAvDriftMs in 0..0xFFFFL) { "maxAvDriftMs must fit in u16" }
            require(ewmaAvDriftMs in -0x8000L..0x7FFFL) { "ewmaAvDriftMs must fit in i16" }
            require(violationCount in 0..0xFFFFL) { "violationCount must fit in u16" }
        }
    }

    class ReceiverReport(
        streamId: ByteArray,
        @JvmField val latestSegment: Long,
        @JvmField val layerMask: Long,
        @JvmField val measuredThroughputKbps: Long,
        @JvmField val rttMs: Long,
        @JvmField val lossPercentX100: Long,
        @JvmField val decoderBufferMs: Long,
        @JvmField val activeResolution: Resolution,
        @JvmField val hdrActive: Boolean,
        @JvmField val ecnCeCount: Long,
        @JvmField val jitterMs: Long,
        @JvmField val deliveredSequence: Long,
        @JvmField val parityApplied: Long,
        @JvmField val fecBudget: Long,
        @JvmField val syncDiagnostics: Optional<SyncDiagnostics>,
    ) : ControlFramePayload {
        private val _streamId: ByteArray = streamId.copyOf()

        val streamId: ByteArray get() = _streamId.copyOf()

        init {
            require(latestSegment >= 0) { "latestSegment must be non-negative" }
            require(layerMask in 0..0xFFFF_FFFFL) { "layerMask must fit in u32" }
            require(measuredThroughputKbps in 0..0xFFFF_FFFFL) { "measuredThroughputKbps must fit in u32" }
            require(rttMs in 0..0xFFFFL) { "rttMs must fit in u16" }
            require(lossPercentX100 in 0..0xFFFFL) { "lossPercentX100 must fit in u16" }
            require(decoderBufferMs in 0..0xFFFFL) { "decoderBufferMs must fit in u16" }
            require(ecnCeCount in 0..0xFFFF_FFFFL) { "ecnCeCount must fit in u32" }
            require(jitterMs in 0..0xFFFFL) { "jitterMs must fit in u16" }
            require(deliveredSequence >= 0) { "deliveredSequence must be non-negative" }
            require(parityApplied in 0..0xFFL) { "parityApplied must fit in u8" }
            require(fecBudget in 0..0xFFL) { "fecBudget must fit in u8" }
        }
    }

    data class TelemetryEncodeStats(
        @JvmField val segment: Long,
        @JvmField val avgLatencyMs: Long,
        @JvmField val droppedLayers: Long,
        @JvmField val avgAudioJitterMs: Long,
        @JvmField val maxAudioJitterMs: Long,
    ) {
        init {
            require(avgLatencyMs in 0..0xFFFFL) { "avgLatencyMs must fit in u16" }
            require(droppedLayers in 0..0xFFFF_FFFFL) { "droppedLayers must fit in u32" }
            require(avgAudioJitterMs in 0..0xFFFFL) { "avgAudioJitterMs must fit in u16" }
            require(maxAudioJitterMs in 0..0xFFFFL) { "maxAudioJitterMs must fit in u16" }
        }
    }

    data class TelemetryDecodeStats(
        @JvmField val segment: Long,
        @JvmField val bufferMs: Long,
        @JvmField val droppedFrames: Long,
        @JvmField val maxDecodeQueueMs: Long,
        @JvmField val avgAvDriftMs: Long,
        @JvmField val maxAvDriftMs: Long,
    ) {
        init {
            require(bufferMs in 0..0xFFFFL) { "bufferMs must fit in u16" }
            require(droppedFrames in 0..0xFFFFL) { "droppedFrames must fit in u16" }
            require(maxDecodeQueueMs in 0..0xFFFFL) { "maxDecodeQueueMs must fit in u16" }
            require(avgAvDriftMs in -0x8000L..0x7FFFL) { "avgAvDriftMs must fit in i16" }
            require(maxAvDriftMs in 0..0xFFFFL) { "maxAvDriftMs must fit in u16" }
        }
    }

    data class TelemetryNetworkStats(
        @JvmField val rttMs: Long,
        @JvmField val lossPercentX100: Long,
        @JvmField val fecRepairs: Long,
        @JvmField val fecFailures: Long,
        @JvmField val datagramReinjects: Long,
    )

    data class TelemetrySecurityStats(
        @JvmField val suite: EncryptionSuite,
        @JvmField val rekeys: Long,
        @JvmField val gckRotations: Long,
        @JvmField val lastContentKeyId: Optional<Long>,
        @JvmField val lastContentKeyValidFrom: Optional<Long>,
    ) {
        init {
            require(rekeys in 0..0xFFFF_FFFFL) { "rekeys must fit in u32" }
            require(gckRotations in 0..0xFFFF_FFFFL) { "gckRotations must fit in u32" }
            lastContentKeyId.ifPresent { value ->
                require(value >= 0 || java.lang.Long.compareUnsigned(value, -1L) <= 0) {
                    "lastContentKeyId must fit in u64"
                }
            }
            lastContentKeyValidFrom.ifPresent { value ->
                require(value >= 0 || java.lang.Long.compareUnsigned(value, -1L) <= 0) {
                    "lastContentKeyValidFrom must fit in u64"
                }
            }
        }
    }

    data class TelemetryEnergyStats(
        @JvmField val segment: Long,
        @JvmField val encoderMilliwatts: Long,
        @JvmField val decoderMilliwatts: Long,
    )

    data class TelemetryAuditOutcome(
        @JvmField val traceId: String,
        @JvmField val slotHeight: Long,
        @JvmField val reviewer: String,
        @JvmField val status: String,
        @JvmField val mitigationUrl: Optional<String>,
    )

    sealed interface TelemetryEvent
    data class TelemetryEncodeEvent(@JvmField val stats: TelemetryEncodeStats) : TelemetryEvent
    data class TelemetryDecodeEvent(@JvmField val stats: TelemetryDecodeStats) : TelemetryEvent
    data class TelemetryNetworkEvent(@JvmField val stats: TelemetryNetworkStats) : TelemetryEvent
    data class TelemetrySecurityEvent(@JvmField val stats: TelemetrySecurityStats) : TelemetryEvent
    data class TelemetryEnergyEvent(@JvmField val stats: TelemetryEnergyStats) : TelemetryEvent
    data class TelemetryAuditOutcomeEvent(@JvmField val stats: TelemetryAuditOutcome) : TelemetryEvent

    class KeyUpdate(
        sessionId: ByteArray,
        @JvmField val suite: EncryptionSuite,
        @JvmField val protocolVersion: Long,
        pubEphemeral: ByteArray,
        @JvmField val keyCounter: Long,
        signature: ByteArray,
    ) : ControlFramePayload {
        private val _sessionId: ByteArray = sessionId.copyOf()
        private val _pubEphemeral: ByteArray = pubEphemeral.copyOf()
        private val _signature: ByteArray = signature.copyOf()

        val sessionId: ByteArray get() = _sessionId.copyOf()
        val pubEphemeral: ByteArray get() = _pubEphemeral.copyOf()
        val signature: ByteArray get() = _signature.copyOf()

        init {
            require(protocolVersion in 0..0xFFFFL) { "protocolVersion must fit in u16" }
            require(keyCounter >= 0) { "keyCounter must be non-negative" }
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is KeyUpdate) return false
            return _sessionId.contentEquals(other._sessionId)
                && suite == other.suite
                && protocolVersion == other.protocolVersion
                && _pubEphemeral.contentEquals(other._pubEphemeral)
                && keyCounter == other.keyCounter
                && _signature.contentEquals(other._signature)
        }

        override fun hashCode(): Int {
            var result = _sessionId.contentHashCode()
            result = 31 * result + suite.hashCode()
            result = 31 * result + protocolVersion.hashCode()
            result = 31 * result + _pubEphemeral.contentHashCode()
            result = 31 * result + keyCounter.hashCode()
            result = 31 * result + _signature.contentHashCode()
            return result
        }
    }

    class ContentKeyUpdate(
        @JvmField val contentKeyId: Long,
        gckWrapped: ByteArray,
        @JvmField val validFromSegment: Long,
    ) : ControlFramePayload {
        private val _gckWrapped: ByteArray = gckWrapped.copyOf()

        val gckWrapped: ByteArray get() = _gckWrapped.copyOf()

        init {
            require(contentKeyId >= 0) { "contentKeyId must be non-negative" }
            require(validFromSegment >= 0) { "validFromSegment must be non-negative" }
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is ContentKeyUpdate) return false
            return contentKeyId == other.contentKeyId
                && validFromSegment == other.validFromSegment
                && _gckWrapped.contentEquals(other._gckWrapped)
        }

        override fun hashCode(): Int {
            var result = contentKeyId.hashCode()
            result = 31 * result + validFromSegment.hashCode()
            result = 31 * result + _gckWrapped.contentHashCode()
            return result
        }
    }

    sealed interface CryptoError {
        data class NonMonotonicKeyCounter(@JvmField val previous: Long, @JvmField val found: Long) : CryptoError
        data class SuiteChanged(@JvmField val expected: EncryptionSuite, @JvmField val found: EncryptionSuite) : CryptoError
        data class ContentKeyRegression(@JvmField val previous: Long, @JvmField val found: Long) : CryptoError
        data class InvalidValidFrom(@JvmField val previous: Long, @JvmField val found: Long) : CryptoError
        data object InvalidWrappedKey : CryptoError
    }

    class CryptoException(
        @JvmField val error: CryptoError,
    ) : RuntimeException(error.toString())

    data class KeyUpdateSnapshot(@JvmField val suite: EncryptionSuite, @JvmField val lastCounter: Long) {
        init {
            require(lastCounter >= 0 || java.lang.Long.compareUnsigned(lastCounter, -1L) <= 0) {
                "lastCounter must fit in u64"
            }
        }
    }

    data class ContentKeySnapshot(@JvmField val lastId: Long, @JvmField val lastValidFrom: Long) {
        init {
            require(lastId >= 0 || java.lang.Long.compareUnsigned(lastId, -1L) <= 0) {
                "lastId must fit in u64"
            }
            require(lastValidFrom >= 0 || java.lang.Long.compareUnsigned(lastValidFrom, -1L) <= 0) {
                "lastValidFrom must fit in u64"
            }
        }
    }

    class KeyUpdateState {
        private var suite: EncryptionSuite? = null
        private var lastCounter: Long? = null

        fun record(frame: KeyUpdate) {
            val lc = lastCounter
            if (lc != null && frame.keyCounter <= lc) {
                throw CryptoException(CryptoError.NonMonotonicKeyCounter(lc, frame.keyCounter))
            }
            val s = suite
            if (s != null) {
                if (s != frame.suite) {
                    throw CryptoException(CryptoError.SuiteChanged(s, frame.suite))
                }
            } else {
                suite = frame.suite
            }
            lastCounter = frame.keyCounter
        }

        fun suite(): Optional<EncryptionSuite> = Optional.ofNullable(suite)

        fun lastCounter(): Optional<Long> = Optional.ofNullable(lastCounter)

        fun restore(lastCounter: Optional<Long>, suite: Optional<EncryptionSuite>) {
            this.lastCounter = lastCounter.orElse(null)
            this.suite = suite.orElse(null)
        }

        fun snapshot(): Optional<KeyUpdateSnapshot> {
            val s = suite ?: return Optional.empty()
            val lc = lastCounter ?: return Optional.empty()
            return Optional.of(KeyUpdateSnapshot(s, lc))
        }

        companion object {
            @JvmStatic
            fun fromSnapshot(snapshot: KeyUpdateSnapshot): KeyUpdateState {
                val state = KeyUpdateState()
                state.restore(Optional.of(snapshot.lastCounter), Optional.of(snapshot.suite))
                return state
            }
        }
    }

    class ContentKeyState {
        private var lastId: Long? = null
        private var lastValidFrom: Long? = null

        fun record(update: ContentKeyUpdate) {
            require(update.gckWrapped.isNotEmpty()) {
                throw CryptoException(CryptoError.InvalidWrappedKey)
            }
            val li = lastId
            if (li != null && update.contentKeyId <= li) {
                throw CryptoException(CryptoError.ContentKeyRegression(li, update.contentKeyId))
            }
            val lvf = lastValidFrom
            if (lvf != null && update.validFromSegment < lvf) {
                throw CryptoException(CryptoError.InvalidValidFrom(lvf, update.validFromSegment))
            }
            lastId = update.contentKeyId
            lastValidFrom = update.validFromSegment
        }

        fun lastId(): Optional<Long> = Optional.ofNullable(lastId)

        fun lastValidFrom(): Optional<Long> = Optional.ofNullable(lastValidFrom)

        fun restore(lastId: Optional<Long>, lastValidFrom: Optional<Long>) {
            this.lastId = lastId.orElse(null)
            this.lastValidFrom = lastValidFrom.orElse(null)
        }

        fun snapshot(): Optional<ContentKeySnapshot> {
            val li = lastId ?: return Optional.empty()
            val lvf = lastValidFrom ?: return Optional.empty()
            return Optional.of(ContentKeySnapshot(li, lvf))
        }

        companion object {
            @JvmStatic
            fun fromSnapshot(snapshot: ContentKeySnapshot): ContentKeyState {
                val state = ContentKeyState()
                state.restore(Optional.of(snapshot.lastId), Optional.of(snapshot.lastValidFrom))
                return state
            }
        }
    }

    class PrivacyRouteUpdate(
        routeId: ByteArray,
        streamId: ByteArray,
        @JvmField val contentKeyId: Long,
        @JvmField val validFromSegment: Long,
        @JvmField val validUntilSegment: Long,
        exitToken: ByteArray,
        @JvmField val soranet: SoranetRoute?,
    ) : ControlFramePayload {
        private val _routeId: ByteArray = routeId.copyOf()
        private val _streamId: ByteArray = streamId.copyOf()
        private val _exitToken: ByteArray = exitToken.copyOf()

        val routeId: ByteArray get() = _routeId.copyOf()
        val streamId: ByteArray get() = _streamId.copyOf()
        val exitToken: ByteArray get() = _exitToken.copyOf()

        init {
            require(contentKeyId >= 0) { "contentKeyId must be non-negative" }
            require(validFromSegment >= 0 && validUntilSegment >= 0) { "segment bounds must be non-negative" }
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is PrivacyRouteUpdate) return false
            return _routeId.contentEquals(other._routeId)
                && _streamId.contentEquals(other._streamId)
                && contentKeyId == other.contentKeyId
                && validFromSegment == other.validFromSegment
                && validUntilSegment == other.validUntilSegment
                && _exitToken.contentEquals(other._exitToken)
                && soranet == other.soranet
        }

        override fun hashCode(): Int {
            var result = _routeId.contentHashCode()
            result = 31 * result + _streamId.contentHashCode()
            result = 31 * result + contentKeyId.hashCode()
            result = 31 * result + validFromSegment.hashCode()
            result = 31 * result + validUntilSegment.hashCode()
            result = 31 * result + _exitToken.contentHashCode()
            result = 31 * result + (soranet?.hashCode() ?: 0)
            return result
        }
    }

    class PrivacyRouteAckFrame(routeId: ByteArray) : ControlFramePayload {
        private val _routeId: ByteArray = routeId.copyOf()

        val routeId: ByteArray get() = _routeId.copyOf()

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is PrivacyRouteAckFrame) return false
            return _routeId.contentEquals(other._routeId)
        }

        override fun hashCode(): Int = _routeId.contentHashCode()
    }

    data class ControlErrorFrame(@JvmField val code: ErrorCode, @JvmField val message: String) : ControlFramePayload

    data class ControlFrame(@JvmField val variant: ControlFrameVariant, @JvmField val payload: ControlFramePayload)

    // --- Adapters ---

    private inline fun <reified E : Enum<E>> enumAdapter(): TypeAdapter<E> {
        val values = enumValues<E>()
        val name = E::class.simpleName
        return object : TypeAdapter<E> {
            override fun encode(encoder: NoritoEncoder, value: E) {
                encoder.writeUInt(value.ordinal.toLong(), 32)
            }

            override fun decode(decoder: NoritoDecoder): E {
                val raw = decoder.readUInt(32)
                require(raw in 0 until values.size) {
                    "Invalid $name discriminant: $raw"
                }
                return values[raw.toInt()]
            }

            override fun fixedSize(): Int = 4
        }
    }

    private val PROFILE_ID_ADAPTER: TypeAdapter<ProfileId> = object : TypeAdapter<ProfileId> {
        override fun encode(encoder: NoritoEncoder, value: ProfileId) {
            UINT16.encode(encoder, value.value.toLong())
        }

        override fun decode(decoder: NoritoDecoder): ProfileId = ProfileId(UINT16.decode(decoder).toInt())

        override fun fixedSize(): Int = UINT16.fixedSize()
    }

    private val CAPABILITY_FLAGS_ADAPTER: TypeAdapter<CapabilityFlags> = object : TypeAdapter<CapabilityFlags> {
        override fun encode(encoder: NoritoEncoder, value: CapabilityFlags) {
            UINT32.encode(encoder, value.bits)
        }

        override fun decode(decoder: NoritoDecoder): CapabilityFlags = CapabilityFlags(UINT32.decode(decoder))

        override fun fixedSize(): Int = UINT32.fixedSize()
    }

    private val PRIVACY_CAPABILITIES_ADAPTER: TypeAdapter<PrivacyCapabilities> =
        object : TypeAdapter<PrivacyCapabilities> {
            override fun encode(encoder: NoritoEncoder, value: PrivacyCapabilities) {
                UINT32.encode(encoder, value.bits)
            }

            override fun decode(decoder: NoritoDecoder): PrivacyCapabilities =
                PrivacyCapabilities(UINT32.decode(decoder))

            override fun fixedSize(): Int = UINT32.fixedSize()
        }

    private val HPKE_SUITE_MASK_ADAPTER: TypeAdapter<HpkeSuiteMask> = object : TypeAdapter<HpkeSuiteMask> {
        override fun encode(encoder: NoritoEncoder, value: HpkeSuiteMask) {
            UINT16.encode(encoder, value.bits.toLong())
        }

        override fun decode(decoder: NoritoDecoder): HpkeSuiteMask = HpkeSuiteMask(UINT16.decode(decoder).toInt())

        override fun fixedSize(): Int = UINT16.fixedSize()
    }

    private val PRIVACY_BUCKET_GRANULARITY_ADAPTER: TypeAdapter<PrivacyBucketGranularity> =
        enumAdapter()

    private val FEC_SCHEME_ADAPTER: TypeAdapter<FecScheme> = enumAdapter()

    private val CAPABILITY_ROLE_ADAPTER: TypeAdapter<CapabilityRole> = enumAdapter()

    private val AUDIO_LAYOUT_ADAPTER: TypeAdapter<AudioLayout> = enumAdapter()

    private val ERROR_CODE_ADAPTER: TypeAdapter<ErrorCode> = enumAdapter()

    private val RESOLUTION_KIND_ADAPTER: TypeAdapter<ResolutionKind> = enumAdapter()

    private val ENCRYPTION_SUITE_ADAPTER: TypeAdapter<EncryptionSuite> = object : TypeAdapter<EncryptionSuite> {
        override fun encode(encoder: NoritoEncoder, value: EncryptionSuite) {
            when (value) {
                is X25519ChaCha20Poly1305Suite -> {
                    encoder.writeUInt(0, 32)
                    HASH.encode(encoder, value.fingerprint)
                }
                is Kyber768XChaCha20Poly1305Suite -> {
                    encoder.writeUInt(1, 32)
                    HASH.encode(encoder, value.fingerprint)
                }
            }
        }

        override fun decode(decoder: NoritoDecoder): EncryptionSuite {
            val tag = decoder.readUInt(32)
            return when (tag) {
                0L -> X25519ChaCha20Poly1305Suite(HASH.decode(decoder))
                1L -> Kyber768XChaCha20Poly1305Suite(HASH.decode(decoder))
                else -> throw IllegalArgumentException("Invalid EncryptionSuite discriminant: $tag")
            }
        }

        override fun fixedSize(): Int = 4 + HASH_LEN
    }

    private val RESOLUTION_CUSTOM_ADAPTER: TypeAdapter<ResolutionCustom> = object : TypeAdapter<ResolutionCustom> {
        override fun encode(encoder: NoritoEncoder, value: ResolutionCustom) {
            UINT16.encode(encoder, value.width.toLong())
            UINT16.encode(encoder, value.height.toLong())
        }

        override fun decode(decoder: NoritoDecoder): ResolutionCustom =
            ResolutionCustom(UINT16.decode(decoder).toInt(), UINT16.decode(decoder).toInt())

        override fun fixedSize(): Int = UINT16.fixedSize() * 2
    }

    private val RESOLUTION_ADAPTER: TypeAdapter<Resolution> = object : TypeAdapter<Resolution> {
        override fun encode(encoder: NoritoEncoder, value: Resolution) {
            RESOLUTION_KIND_ADAPTER.encode(encoder, value.kind)
            if (value.kind == ResolutionKind.CUSTOM) {
                RESOLUTION_CUSTOM_ADAPTER.encode(encoder, value.custom.orElseThrow { NoSuchElementException() })
            }
        }

        override fun decode(decoder: NoritoDecoder): Resolution {
            val kind = RESOLUTION_KIND_ADAPTER.decode(decoder)
            return if (kind == ResolutionKind.CUSTOM) {
                Resolution(kind, Optional.of(RESOLUTION_CUSTOM_ADAPTER.decode(decoder)))
            } else {
                Resolution(kind, Optional.empty())
            }
        }
    }

    private val TRANSPORT_CAPABILITIES_ADAPTER: TypeAdapter<TransportCapabilities> =
        object : TypeAdapter<TransportCapabilities> {
            override fun encode(encoder: NoritoEncoder, value: TransportCapabilities) {
                HPKE_SUITE_MASK_ADAPTER.encode(encoder, value.hpkeSuites)
                BOOL.encode(encoder, value.supportsDatagram)
                UINT16.encode(encoder, value.maxSegmentDatagramSize)
                UINT16.encode(encoder, value.fecFeedbackIntervalMs)
                PRIVACY_BUCKET_GRANULARITY_ADAPTER.encode(encoder, value.privacyBucketGranularity)
            }

            override fun decode(decoder: NoritoDecoder): TransportCapabilities {
                val mask = HPKE_SUITE_MASK_ADAPTER.decode(decoder)
                val datagram = BOOL.decode(decoder)
                val maxDatagram = UINT16.decode(decoder)
                val fecInterval = UINT16.decode(decoder)
                val granularity = PRIVACY_BUCKET_GRANULARITY_ADAPTER.decode(decoder)
                return TransportCapabilities(mask, datagram, maxDatagram, fecInterval, granularity)
            }
        }

    private val STREAM_METADATA_ADAPTER: TypeAdapter<StreamMetadata> = object : TypeAdapter<StreamMetadata> {
        private val optionalString: TypeAdapter<Optional<String>> = NoritoAdapters.option(STRING)
        private val optionalHash: TypeAdapter<Optional<ByteArray>> = NoritoAdapters.option(HASH)

        override fun encode(encoder: NoritoEncoder, value: StreamMetadata) {
            STRING.encode(encoder, value.title)
            optionalString.encode(encoder, value.description)
            optionalHash.encode(encoder, value.accessPolicyId)
            STRING_LIST.encode(encoder, value.tags)
        }

        override fun decode(decoder: NoritoDecoder): StreamMetadata {
            val title = STRING.decode(decoder)
            val description = optionalString.decode(decoder)
            val accessPolicy = optionalHash.decode(decoder)
            val tags = STRING_LIST.decode(decoder)
            return StreamMetadata(title, description, accessPolicy, tags)
        }
    }

    private val TICKET_CAPABILITIES_ADAPTER: TypeAdapter<TicketCapabilities> =
        object : TypeAdapter<TicketCapabilities> {
            override fun encode(encoder: NoritoEncoder, value: TicketCapabilities) {
                UINT32.encode(encoder, value.bits.toLong() and 0xFFFF_FFFFL)
            }

            override fun decode(decoder: NoritoDecoder): TicketCapabilities {
                val raw = UINT32.decode(decoder)
                return TicketCapabilities((raw and 0xFFFF_FFFFL).toInt())
            }
        }

    private val TICKET_POLICY_ADAPTER: TypeAdapter<TicketPolicy> = object : TypeAdapter<TicketPolicy> {
        private val optionalUint32: TypeAdapter<Optional<Long>> = NoritoAdapters.option(UINT32)

        override fun encode(encoder: NoritoEncoder, value: TicketPolicy) {
            UINT16.encode(encoder, value.maxRelays.toLong() and 0xFFFFL)
            STRING_LIST.encode(encoder, value.allowedRegions)
            optionalUint32.encode(encoder, value.maxBandwidthKbps)
        }

        override fun decode(decoder: NoritoDecoder): TicketPolicy {
            val maxRelays = (UINT16.decode(decoder) and 0xFFFFL).toInt()
            val regions = STRING_LIST.decode(decoder)
            val maxBandwidth = optionalUint32.decode(decoder)
            return TicketPolicy(maxRelays, regions, maxBandwidth)
        }
    }

    private val UINT128_ADAPTER: TypeAdapter<BigInteger> = object : TypeAdapter<BigInteger> {
        override fun encode(encoder: NoritoEncoder, value: BigInteger) {
            encoder.writeBytes(toLittleEndian(value, 16, UINT128_MAX))
        }

        override fun decode(decoder: NoritoDecoder): BigInteger = fromLittleEndian(decoder.readBytes(16))

        override fun fixedSize(): Int = 16
    }

    private val STREAMING_TICKET_ADAPTER: TypeAdapter<StreamingTicket> = object : TypeAdapter<StreamingTicket> {
        private val optionalPolicy: TypeAdapter<Optional<TicketPolicy>> = NoritoAdapters.option(TICKET_POLICY_ADAPTER)

        override fun encode(encoder: NoritoEncoder, value: StreamingTicket) {
            HASH.encode(encoder, value.ticketId)
            STRING.encode(encoder, value.owner)
            UINT64.encode(encoder, value.dsid)
            UINT8.encode(encoder, value.laneId.toLong())
            UINT64.encode(encoder, value.settlementBucket)
            UINT64.encode(encoder, value.startSlot)
            UINT64.encode(encoder, value.expireSlot)
            UINT128_ADAPTER.encode(encoder, value.prepaidTeu)
            UINT32.encode(encoder, value.chunkTeu)
            UINT16.encode(encoder, value.fanoutQuota.toLong() and 0xFFFFL)
            HASH.encode(encoder, value.keyCommitment)
            UINT64.encode(encoder, value.nonce)
            SIGNATURE.encode(encoder, value.contractSignature)
            HASH.encode(encoder, value.commitment)
            HASH.encode(encoder, value.nullifier)
            HASH.encode(encoder, value.proofId)
            UINT64.encode(encoder, value.issuedAt)
            UINT64.encode(encoder, value.expiresAt)
            optionalPolicy.encode(encoder, value.policy)
            TICKET_CAPABILITIES_ADAPTER.encode(encoder, value.capabilities)
        }

        override fun decode(decoder: NoritoDecoder): StreamingTicket {
            val ticketId = HASH.decode(decoder)
            val owner = STRING.decode(decoder)
            val dsid = UINT64.decode(decoder)
            val laneId = (UINT8.decode(decoder) and 0xFFL).toInt()
            val settlementBucket = UINT64.decode(decoder)
            val startSlot = UINT64.decode(decoder)
            val expireSlot = UINT64.decode(decoder)
            val prepaidTeu = UINT128_ADAPTER.decode(decoder)
            val chunkTeu = UINT32.decode(decoder)
            val fanoutQuota = (UINT16.decode(decoder) and 0xFFFFL).toInt()
            val keyCommitment = HASH.decode(decoder)
            val nonce = UINT64.decode(decoder)
            val contractSig = SIGNATURE.decode(decoder)
            val commitment = HASH.decode(decoder)
            val nullifier = HASH.decode(decoder)
            val proofId = HASH.decode(decoder)
            val issuedAt = UINT64.decode(decoder)
            val expiresAt = UINT64.decode(decoder)
            val policy = optionalPolicy.decode(decoder)
            val capabilities = TICKET_CAPABILITIES_ADAPTER.decode(decoder)
            return StreamingTicket(
                ticketId, owner, dsid, laneId, settlementBucket, startSlot, expireSlot,
                prepaidTeu, chunkTeu, fanoutQuota, keyCommitment, nonce, contractSig,
                commitment, nullifier, proofId, issuedAt, expiresAt, policy, capabilities,
            )
        }
    }

    private val TICKET_REVOCATION_ADAPTER: TypeAdapter<TicketRevocation> = object : TypeAdapter<TicketRevocation> {
        override fun encode(encoder: NoritoEncoder, value: TicketRevocation) {
            HASH.encode(encoder, value.ticketId)
            HASH.encode(encoder, value.nullifier)
            UINT16.encode(encoder, value.reasonCode.toLong() and 0xFFFFL)
            SIGNATURE.encode(encoder, value.revocationSignature)
        }

        override fun decode(decoder: NoritoDecoder): TicketRevocation {
            val ticketId = HASH.decode(decoder)
            val nullifier = HASH.decode(decoder)
            val reasonCode = (UINT16.decode(decoder) and 0xFFFFL).toInt()
            val signature = SIGNATURE.decode(decoder)
            return TicketRevocation(ticketId, nullifier, reasonCode, signature)
        }
    }

    private val PRIVACY_RELAY_ADAPTER: TypeAdapter<PrivacyRelay> = object : TypeAdapter<PrivacyRelay> {
        override fun encode(encoder: NoritoEncoder, value: PrivacyRelay) {
            HASH.encode(encoder, value.relayId)
            STRING.encode(encoder, value.endpoint)
            HASH.encode(encoder, value.keyFingerprint)
            PRIVACY_CAPABILITIES_ADAPTER.encode(encoder, value.capabilities)
        }

        override fun decode(decoder: NoritoDecoder): PrivacyRelay {
            val relayId = HASH.decode(decoder)
            val endpoint = STRING.decode(decoder)
            val keyFingerprint = HASH.decode(decoder)
            val capabilities = PRIVACY_CAPABILITIES_ADAPTER.decode(decoder)
            return PrivacyRelay(relayId, endpoint, keyFingerprint, capabilities)
        }
    }

    private val SORANET_ACCESS_KIND_ADAPTER: TypeAdapter<SoranetAccessKind> =
        object : TypeAdapter<SoranetAccessKind> {
            override fun encode(encoder: NoritoEncoder, value: SoranetAccessKind) {
                UINT8.encode(encoder, value.ordinal.toLong())
            }

            override fun decode(decoder: NoritoDecoder): SoranetAccessKind {
                val ordinal = UINT8.decode(decoder)
                return when (ordinal.toInt()) {
                    0 -> SoranetAccessKind.READ_ONLY
                    1 -> SoranetAccessKind.AUTHENTICATED
                    else -> throw IllegalArgumentException("unknown SoranetAccessKind ordinal: $ordinal")
                }
            }
        }

    private val SORANET_ROUTE_ADAPTER: TypeAdapter<SoranetRoute> = object : TypeAdapter<SoranetRoute> {
        private val optionalUint16: TypeAdapter<Optional<Long>> = NoritoAdapters.option(UINT16)

        override fun encode(encoder: NoritoEncoder, value: SoranetRoute) {
            HASH.encode(encoder, value.channelId)
            STRING.encode(encoder, value.exitMultiaddr)
            val padding: Optional<Long> =
                if (value.paddingBudgetMs == null) Optional.empty()
                else Optional.of(value.paddingBudgetMs.toLong())
            optionalUint16.encode(encoder, padding)
            SORANET_ACCESS_KIND_ADAPTER.encode(encoder, value.accessKind)
        }

        override fun decode(decoder: NoritoDecoder): SoranetRoute {
            val channelId = HASH.decode(decoder)
            val exitMultiaddr = STRING.decode(decoder)
            val padding = optionalUint16.decode(decoder)
            val paddingBudgetMs: Int? = padding.map { it.toInt() }.orElse(null)
            val accessKind = SORANET_ACCESS_KIND_ADAPTER.decode(decoder)
            return SoranetRoute(channelId, exitMultiaddr, paddingBudgetMs, accessKind)
        }
    }

    private val OPTIONAL_SORANET_ROUTE: TypeAdapter<Optional<SoranetRoute>> =
        NoritoAdapters.option(SORANET_ROUTE_ADAPTER)

    private val PRIVACY_ROUTE_ADAPTER: TypeAdapter<PrivacyRoute> = object : TypeAdapter<PrivacyRoute> {
        override fun encode(encoder: NoritoEncoder, value: PrivacyRoute) {
            HASH.encode(encoder, value.routeId)
            PRIVACY_RELAY_ADAPTER.encode(encoder, value.entry)
            PRIVACY_RELAY_ADAPTER.encode(encoder, value.exit)
            BYTES.encode(encoder, value.ticketEntry)
            BYTES.encode(encoder, value.ticketExit)
            UINT64.encode(encoder, value.expirySegment)
            OPTIONAL_SORANET_ROUTE.encode(encoder, Optional.ofNullable(value.soranet))
        }

        override fun decode(decoder: NoritoDecoder): PrivacyRoute {
            val routeId = HASH.decode(decoder)
            val entry = PRIVACY_RELAY_ADAPTER.decode(decoder)
            val exit = PRIVACY_RELAY_ADAPTER.decode(decoder)
            val ticketEntry = BYTES.decode(decoder)
            val ticketExit = BYTES.decode(decoder)
            val expirySegment = UINT64.decode(decoder)
            val soranet = OPTIONAL_SORANET_ROUTE.decode(decoder).orElse(null)
            return PrivacyRoute(routeId, entry, exit, ticketEntry, ticketExit, expirySegment, soranet)
        }
    }

    private val NEURAL_BUNDLE_ADAPTER: TypeAdapter<NeuralBundle> = object : TypeAdapter<NeuralBundle> {
        private val optionalHash: TypeAdapter<Optional<ByteArray>> = NoritoAdapters.option(HASH)

        override fun encode(encoder: NoritoEncoder, value: NeuralBundle) {
            STRING.encode(encoder, value.bundleId)
            HASH.encode(encoder, value.weightsSha256)
            INT16_LIST.encode(encoder, value.activationScale)
            INT32_LIST.encode(encoder, value.bias)
            SIGNATURE.encode(encoder, value.metadataSignature)
            optionalHash.encode(encoder, value.metalShaderSha256)
            optionalHash.encode(encoder, value.cudaPtxSha256)
        }

        override fun decode(decoder: NoritoDecoder): NeuralBundle {
            val bundleId = STRING.decode(decoder)
            val weights = HASH.decode(decoder)
            val activationScale = INT16_LIST.decode(decoder)
            val bias = INT32_LIST.decode(decoder)
            val signature = SIGNATURE.decode(decoder)
            val metal = optionalHash.decode(decoder)
            val cuda = optionalHash.decode(decoder)
            return NeuralBundle(bundleId, weights, activationScale, bias, signature, metal, cuda)
        }
    }

    private val CHUNK_DESCRIPTOR_ADAPTER: TypeAdapter<ChunkDescriptor> = object : TypeAdapter<ChunkDescriptor> {
        override fun encode(encoder: NoritoEncoder, value: ChunkDescriptor) {
            UINT16.encode(encoder, value.chunkId)
            UINT32.encode(encoder, value.offset)
            UINT32.encode(encoder, value.length)
            HASH.encode(encoder, value.commitment)
            BOOL.encode(encoder, value.parity)
        }

        override fun decode(decoder: NoritoDecoder): ChunkDescriptor {
            val chunkId = UINT16.decode(decoder)
            val offset = UINT32.decode(decoder)
            val length = UINT32.decode(decoder)
            val commitment = HASH.decode(decoder)
            val parity = BOOL.decode(decoder)
            return ChunkDescriptor(chunkId, offset, length, commitment, parity)
        }
    }

    private val CHUNK_DESCRIPTOR_LIST_ADAPTER: TypeAdapter<List<ChunkDescriptor>> =
        NoritoAdapters.sequence(CHUNK_DESCRIPTOR_ADAPTER)

    private val PRIVACY_ROUTE_LIST_ADAPTER: TypeAdapter<List<PrivacyRoute>> =
        NoritoAdapters.sequence(PRIVACY_ROUTE_ADAPTER)

    private val OPTIONAL_NEURAL_BUNDLE_ADAPTER: TypeAdapter<Optional<NeuralBundle>> =
        NoritoAdapters.option(NEURAL_BUNDLE_ADAPTER)

    private val OPTIONAL_UINT64_ADAPTER: TypeAdapter<Optional<Long>> = NoritoAdapters.option(UINT64)

    private val MANIFEST_ADAPTER: TypeAdapter<ManifestV1> = object : TypeAdapter<ManifestV1> {
        override fun encode(encoder: NoritoEncoder, value: ManifestV1) {
            HASH.encode(encoder, value.streamId)
            UINT16.encode(encoder, value.protocolVersion)
            UINT64.encode(encoder, value.segmentNumber)
            UINT64.encode(encoder, value.publishedAt)
            PROFILE_ID_ADAPTER.encode(encoder, value.profile)
            STRING.encode(encoder, value.daEndpoint)
            HASH.encode(encoder, value.chunkRoot)
            UINT64.encode(encoder, value.contentKeyId)
            HASH.encode(encoder, value.nonceSalt)
            CHUNK_DESCRIPTOR_LIST_ADAPTER.encode(encoder, value.chunkDescriptors)
            HASH.encode(encoder, value.transportCapabilitiesHash)
            ENCRYPTION_SUITE_ADAPTER.encode(encoder, value.encryptionSuite)
            FEC_SCHEME_ADAPTER.encode(encoder, value.fecSuite)
            PRIVACY_ROUTE_LIST_ADAPTER.encode(encoder, value.privacyRoutes)
            OPTIONAL_NEURAL_BUNDLE_ADAPTER.encode(encoder, value.neuralBundle)
            STREAM_METADATA_ADAPTER.encode(encoder, value.publicMetadata)
            CAPABILITY_FLAGS_ADAPTER.encode(encoder, value.capabilities)
            SIGNATURE.encode(encoder, value.signature)
        }

        override fun decode(decoder: NoritoDecoder): ManifestV1 {
            val streamId = HASH.decode(decoder)
            val protocolVersion = UINT16.decode(decoder)
            val segmentNumber = UINT64.decode(decoder)
            val publishedAt = UINT64.decode(decoder)
            val profile = PROFILE_ID_ADAPTER.decode(decoder)
            val daEndpoint = STRING.decode(decoder)
            val chunkRoot = HASH.decode(decoder)
            val contentKeyId = UINT64.decode(decoder)
            val nonceSalt = HASH.decode(decoder)
            val chunkDescriptors = CHUNK_DESCRIPTOR_LIST_ADAPTER.decode(decoder)
            val transportHash = HASH.decode(decoder)
            val encryptionSuite = ENCRYPTION_SUITE_ADAPTER.decode(decoder)
            val fecScheme = FEC_SCHEME_ADAPTER.decode(decoder)
            val privacyRoutes = PRIVACY_ROUTE_LIST_ADAPTER.decode(decoder)
            val neuralBundle = OPTIONAL_NEURAL_BUNDLE_ADAPTER.decode(decoder)
            val publicMetadata = STREAM_METADATA_ADAPTER.decode(decoder)
            val capabilities = CAPABILITY_FLAGS_ADAPTER.decode(decoder)
            val signature = SIGNATURE.decode(decoder)
            return ManifestV1(
                streamId, protocolVersion, segmentNumber, publishedAt, profile, daEndpoint,
                chunkRoot, contentKeyId, nonceSalt, chunkDescriptors, transportHash,
                encryptionSuite, fecScheme, privacyRoutes, neuralBundle, publicMetadata,
                capabilities, signature,
            )
        }
    }

    private val MANIFEST_ANNOUNCE_FRAME_ADAPTER: TypeAdapter<ManifestAnnounceFrame> =
        object : TypeAdapter<ManifestAnnounceFrame> {
            override fun encode(encoder: NoritoEncoder, value: ManifestAnnounceFrame) {
                MANIFEST_ADAPTER.encode(encoder, value.manifest)
            }

            override fun decode(decoder: NoritoDecoder): ManifestAnnounceFrame =
                ManifestAnnounceFrame(MANIFEST_ADAPTER.decode(decoder))
        }

    private val CHUNK_REQUEST_FRAME_ADAPTER: TypeAdapter<ChunkRequestFrame> =
        object : TypeAdapter<ChunkRequestFrame> {
            override fun encode(encoder: NoritoEncoder, value: ChunkRequestFrame) {
                UINT64.encode(encoder, value.segment)
                UINT16.encode(encoder, value.chunkId)
            }

            override fun decode(decoder: NoritoDecoder): ChunkRequestFrame =
                ChunkRequestFrame(UINT64.decode(decoder), UINT16.decode(decoder))
        }

    private val CHUNK_ACK_FRAME_ADAPTER: TypeAdapter<ChunkAcknowledgeFrame> =
        object : TypeAdapter<ChunkAcknowledgeFrame> {
            override fun encode(encoder: NoritoEncoder, value: ChunkAcknowledgeFrame) {
                UINT64.encode(encoder, value.segment)
                UINT16.encode(encoder, value.chunkId)
            }

            override fun decode(decoder: NoritoDecoder): ChunkAcknowledgeFrame =
                ChunkAcknowledgeFrame(UINT64.decode(decoder), UINT16.decode(decoder))
        }

    private val TRANSPORT_CAPABILITIES_FRAME_ADAPTER: TypeAdapter<TransportCapabilitiesFrame> =
        object : TypeAdapter<TransportCapabilitiesFrame> {
            override fun encode(encoder: NoritoEncoder, value: TransportCapabilitiesFrame) {
                CAPABILITY_ROLE_ADAPTER.encode(encoder, value.endpointRole)
                TRANSPORT_CAPABILITIES_ADAPTER.encode(encoder, value.capabilities)
            }

            override fun decode(decoder: NoritoDecoder): TransportCapabilitiesFrame {
                val role = CAPABILITY_ROLE_ADAPTER.decode(decoder)
                val capabilities = TRANSPORT_CAPABILITIES_ADAPTER.decode(decoder)
                return TransportCapabilitiesFrame(role, capabilities)
            }
        }

    private val AUDIO_CAPABILITY_ADAPTER: TypeAdapter<AudioCapability> = object : TypeAdapter<AudioCapability> {
        override fun encode(encoder: NoritoEncoder, value: AudioCapability) {
            UINT32_LIST.encode(encoder, value.sampleRates)
            BOOL.encode(encoder, value.ambisonics)
            UINT8.encode(encoder, value.maxChannels)
        }

        override fun decode(decoder: NoritoDecoder): AudioCapability {
            val sampleRates = UINT32_LIST.decode(decoder)
            val ambisonics = BOOL.decode(decoder)
            val maxChannels = UINT8.decode(decoder)
            return AudioCapability(sampleRates, ambisonics, maxChannels)
        }
    }

    private val CAPABILITY_REPORT_ADAPTER: TypeAdapter<CapabilityReport> = object : TypeAdapter<CapabilityReport> {
        override fun encode(encoder: NoritoEncoder, value: CapabilityReport) {
            HASH.encode(encoder, value.streamId)
            CAPABILITY_ROLE_ADAPTER.encode(encoder, value.endpointRole)
            UINT16.encode(encoder, value.protocolVersion)
            RESOLUTION_ADAPTER.encode(encoder, value.maxResolution)
            BOOL.encode(encoder, value.hdrSupported)
            BOOL.encode(encoder, value.captureHdr)
            STRING_LIST.encode(encoder, value.neuralBundles)
            AUDIO_CAPABILITY_ADAPTER.encode(encoder, value.audioCaps)
            CAPABILITY_FLAGS_ADAPTER.encode(encoder, value.featureBits)
            UINT16.encode(encoder, value.maxDatagramSize)
            BOOL.encode(encoder, value.dplpmtud)
        }

        override fun decode(decoder: NoritoDecoder): CapabilityReport {
            val streamId = HASH.decode(decoder)
            val role = CAPABILITY_ROLE_ADAPTER.decode(decoder)
            val protocolVersion = UINT16.decode(decoder)
            val resolution = RESOLUTION_ADAPTER.decode(decoder)
            val hdrSupported = BOOL.decode(decoder)
            val captureHdr = BOOL.decode(decoder)
            val neuralBundles = STRING_LIST.decode(decoder)
            val audioCaps = AUDIO_CAPABILITY_ADAPTER.decode(decoder)
            val featureBits = CAPABILITY_FLAGS_ADAPTER.decode(decoder)
            val maxDatagramSize = UINT16.decode(decoder)
            val dplpmtud = BOOL.decode(decoder)
            return CapabilityReport(
                streamId, role, protocolVersion, resolution, hdrSupported, captureHdr,
                neuralBundles, audioCaps, featureBits, maxDatagramSize, dplpmtud,
            )
        }
    }

    private val CAPABILITY_ACK_ADAPTER: TypeAdapter<CapabilityAck> = object : TypeAdapter<CapabilityAck> {
        override fun encode(encoder: NoritoEncoder, value: CapabilityAck) {
            HASH.encode(encoder, value.streamId)
            UINT16.encode(encoder, value.acceptedVersion)
            CAPABILITY_FLAGS_ADAPTER.encode(encoder, value.negotiatedFeatures)
            UINT16.encode(encoder, value.maxDatagramSize)
            BOOL.encode(encoder, value.dplpmtud)
        }

        override fun decode(decoder: NoritoDecoder): CapabilityAck {
            val streamId = HASH.decode(decoder)
            val acceptedVersion = UINT16.decode(decoder)
            val negotiatedFeatures = CAPABILITY_FLAGS_ADAPTER.decode(decoder)
            val maxDatagramSize = UINT16.decode(decoder)
            val dplpmtud = BOOL.decode(decoder)
            return CapabilityAck(streamId, acceptedVersion, negotiatedFeatures, maxDatagramSize, dplpmtud)
        }
    }

    private val AUDIO_FRAME_ADAPTER: TypeAdapter<AudioFrame> = object : TypeAdapter<AudioFrame> {
        override fun encode(encoder: NoritoEncoder, value: AudioFrame) {
            UINT64.encode(encoder, value.sequence)
            UINT64.encode(encoder, value.timestampNs)
            UINT8.encode(encoder, value.fecLevel)
            AUDIO_LAYOUT_ADAPTER.encode(encoder, value.channelLayout)
            BYTES.encode(encoder, value.payload)
        }

        override fun decode(decoder: NoritoDecoder): AudioFrame {
            val sequence = UINT64.decode(decoder)
            val timestampNs = UINT64.decode(decoder)
            val fecLevel = UINT8.decode(decoder)
            val layout = AUDIO_LAYOUT_ADAPTER.decode(decoder)
            val payload = BYTES.decode(decoder)
            return AudioFrame(sequence, timestampNs, fecLevel, layout, payload)
        }
    }

    private val FEEDBACK_HINT_FRAME_ADAPTER: TypeAdapter<FeedbackHintFrame> =
        object : TypeAdapter<FeedbackHintFrame> {
            override fun encode(encoder: NoritoEncoder, value: FeedbackHintFrame) {
                HASH.encode(encoder, value.streamId)
                UINT32.encode(encoder, value.lossEwmaQ16)
                SINT32.encode(encoder, value.latencyGradientQ16)
                UINT16.encode(encoder, value.observedRttMs)
                UINT16.encode(encoder, value.reportIntervalMs)
                UINT8.encode(encoder, value.parityChunks)
            }

            override fun decode(decoder: NoritoDecoder): FeedbackHintFrame {
                val streamId = HASH.decode(decoder)
                val lossEwma = UINT32.decode(decoder)
                val latencyGradient = SINT32.decode(decoder)
                val observedRtt = UINT16.decode(decoder)
                val reportInterval = UINT16.decode(decoder)
                val parityChunks = UINT8.decode(decoder)
                return FeedbackHintFrame(streamId, lossEwma, latencyGradient, observedRtt, reportInterval, parityChunks)
            }
        }

    private val SYNC_DIAGNOSTICS_ADAPTER: TypeAdapter<SyncDiagnostics> = object : TypeAdapter<SyncDiagnostics> {
        override fun encode(encoder: NoritoEncoder, value: SyncDiagnostics) {
            UINT16.encode(encoder, value.windowMs)
            UINT16.encode(encoder, value.samples)
            UINT16.encode(encoder, value.avgAudioJitterMs)
            UINT16.encode(encoder, value.maxAudioJitterMs)
            SINT16.encode(encoder, value.avgAvDriftMs)
            UINT16.encode(encoder, value.maxAvDriftMs)
            SINT16.encode(encoder, value.ewmaAvDriftMs)
            UINT16.encode(encoder, value.violationCount)
        }

        override fun decode(decoder: NoritoDecoder): SyncDiagnostics {
            val windowMs = UINT16.decode(decoder)
            val samples = UINT16.decode(decoder)
            val avgAudioJitter = UINT16.decode(decoder)
            val maxAudioJitter = UINT16.decode(decoder)
            val avgAvDrift = SINT16.decode(decoder)
            val maxAvDrift = UINT16.decode(decoder)
            val ewmaAvDrift = SINT16.decode(decoder)
            val violationCount = UINT16.decode(decoder)
            return SyncDiagnostics(
                windowMs, samples, avgAudioJitter, maxAudioJitter,
                avgAvDrift, maxAvDrift, ewmaAvDrift, violationCount,
            )
        }
    }

    private val OPTIONAL_SYNC_DIAGNOSTICS_ADAPTER: TypeAdapter<Optional<SyncDiagnostics>> =
        NoritoAdapters.option(SYNC_DIAGNOSTICS_ADAPTER)

    private val RECEIVER_REPORT_ADAPTER: TypeAdapter<ReceiverReport> = object : TypeAdapter<ReceiverReport> {
        override fun encode(encoder: NoritoEncoder, value: ReceiverReport) {
            HASH.encode(encoder, value.streamId)
            UINT64.encode(encoder, value.latestSegment)
            UINT32.encode(encoder, value.layerMask)
            UINT32.encode(encoder, value.measuredThroughputKbps)
            UINT16.encode(encoder, value.rttMs)
            UINT16.encode(encoder, value.lossPercentX100)
            UINT16.encode(encoder, value.decoderBufferMs)
            RESOLUTION_ADAPTER.encode(encoder, value.activeResolution)
            BOOL.encode(encoder, value.hdrActive)
            UINT32.encode(encoder, value.ecnCeCount)
            UINT16.encode(encoder, value.jitterMs)
            UINT64.encode(encoder, value.deliveredSequence)
            UINT8.encode(encoder, value.parityApplied)
            UINT8.encode(encoder, value.fecBudget)
            OPTIONAL_SYNC_DIAGNOSTICS_ADAPTER.encode(encoder, value.syncDiagnostics)
        }

        override fun decode(decoder: NoritoDecoder): ReceiverReport {
            val streamId = HASH.decode(decoder)
            val latestSegment = UINT64.decode(decoder)
            val layerMask = UINT32.decode(decoder)
            val throughput = UINT32.decode(decoder)
            val rtt = UINT16.decode(decoder)
            val loss = UINT16.decode(decoder)
            val buffer = UINT16.decode(decoder)
            val resolution = RESOLUTION_ADAPTER.decode(decoder)
            val hdrActive = BOOL.decode(decoder)
            val ecnCe = UINT32.decode(decoder)
            val jitter = UINT16.decode(decoder)
            val delivered = UINT64.decode(decoder)
            val parityApplied = UINT8.decode(decoder)
            val fecBudget = UINT8.decode(decoder)
            val syncDiagnostics = OPTIONAL_SYNC_DIAGNOSTICS_ADAPTER.decode(decoder)
            return ReceiverReport(
                streamId, latestSegment, layerMask, throughput, rtt, loss, buffer,
                resolution, hdrActive, ecnCe, jitter, delivered, parityApplied,
                fecBudget, syncDiagnostics,
            )
        }
    }

    private val TELEMETRY_ENCODE_STATS_ADAPTER: TypeAdapter<TelemetryEncodeStats> =
        object : TypeAdapter<TelemetryEncodeStats> {
            override fun encode(encoder: NoritoEncoder, value: TelemetryEncodeStats) {
                UINT64.encode(encoder, value.segment)
                UINT16.encode(encoder, value.avgLatencyMs)
                UINT32.encode(encoder, value.droppedLayers)
                UINT16.encode(encoder, value.avgAudioJitterMs)
                UINT16.encode(encoder, value.maxAudioJitterMs)
            }

            override fun decode(decoder: NoritoDecoder): TelemetryEncodeStats {
                val segment = UINT64.decode(decoder)
                val latency = UINT16.decode(decoder)
                val dropped = UINT32.decode(decoder)
                val avgJitter = UINT16.decode(decoder)
                val maxJitter = UINT16.decode(decoder)
                return TelemetryEncodeStats(segment, latency, dropped, avgJitter, maxJitter)
            }
        }

    private val TELEMETRY_DECODE_STATS_ADAPTER: TypeAdapter<TelemetryDecodeStats> =
        object : TypeAdapter<TelemetryDecodeStats> {
            override fun encode(encoder: NoritoEncoder, value: TelemetryDecodeStats) {
                UINT64.encode(encoder, value.segment)
                UINT16.encode(encoder, value.bufferMs)
                UINT16.encode(encoder, value.droppedFrames)
                UINT16.encode(encoder, value.maxDecodeQueueMs)
                SINT16.encode(encoder, value.avgAvDriftMs)
                UINT16.encode(encoder, value.maxAvDriftMs)
            }

            override fun decode(decoder: NoritoDecoder): TelemetryDecodeStats {
                val segment = UINT64.decode(decoder)
                val bufferMs = UINT16.decode(decoder)
                val droppedFrames = UINT16.decode(decoder)
                val maxQueue = UINT16.decode(decoder)
                val avgDrift = SINT16.decode(decoder)
                val maxDrift = UINT16.decode(decoder)
                return TelemetryDecodeStats(segment, bufferMs, droppedFrames, maxQueue, avgDrift, maxDrift)
            }
        }

    private val TELEMETRY_NETWORK_STATS_ADAPTER: TypeAdapter<TelemetryNetworkStats> =
        object : TypeAdapter<TelemetryNetworkStats> {
            override fun encode(encoder: NoritoEncoder, value: TelemetryNetworkStats) {
                UINT16.encode(encoder, value.rttMs)
                UINT16.encode(encoder, value.lossPercentX100)
                UINT32.encode(encoder, value.fecRepairs)
                UINT32.encode(encoder, value.fecFailures)
                UINT32.encode(encoder, value.datagramReinjects)
            }

            override fun decode(decoder: NoritoDecoder): TelemetryNetworkStats {
                val rtt = UINT16.decode(decoder)
                val loss = UINT16.decode(decoder)
                val repairs = UINT32.decode(decoder)
                val failures = UINT32.decode(decoder)
                val reinjects = UINT32.decode(decoder)
                return TelemetryNetworkStats(rtt, loss, repairs, failures, reinjects)
            }
        }

    private val TELEMETRY_SECURITY_STATS_ADAPTER: TypeAdapter<TelemetrySecurityStats> =
        object : TypeAdapter<TelemetrySecurityStats> {
            override fun encode(encoder: NoritoEncoder, value: TelemetrySecurityStats) {
                ENCRYPTION_SUITE_ADAPTER.encode(encoder, value.suite)
                UINT32.encode(encoder, value.rekeys)
                UINT32.encode(encoder, value.gckRotations)
                OPTIONAL_UINT64_ADAPTER.encode(encoder, value.lastContentKeyId)
                OPTIONAL_UINT64_ADAPTER.encode(encoder, value.lastContentKeyValidFrom)
            }

            override fun decode(decoder: NoritoDecoder): TelemetrySecurityStats {
                val suite = ENCRYPTION_SUITE_ADAPTER.decode(decoder)
                val rekeys = UINT32.decode(decoder)
                val rotations = UINT32.decode(decoder)
                val lastId = OPTIONAL_UINT64_ADAPTER.decode(decoder)
                val lastValidFrom = OPTIONAL_UINT64_ADAPTER.decode(decoder)
                return TelemetrySecurityStats(suite, rekeys, rotations, lastId, lastValidFrom)
            }
        }

    private val TELEMETRY_ENERGY_STATS_ADAPTER: TypeAdapter<TelemetryEnergyStats> =
        object : TypeAdapter<TelemetryEnergyStats> {
            override fun encode(encoder: NoritoEncoder, value: TelemetryEnergyStats) {
                UINT64.encode(encoder, value.segment)
                UINT32.encode(encoder, value.encoderMilliwatts)
                UINT32.encode(encoder, value.decoderMilliwatts)
            }

            override fun decode(decoder: NoritoDecoder): TelemetryEnergyStats {
                val segment = UINT64.decode(decoder)
                val encoderMw = UINT32.decode(decoder)
                val decoderMw = UINT32.decode(decoder)
                return TelemetryEnergyStats(segment, encoderMw, decoderMw)
            }
        }

    private val TELEMETRY_AUDIT_OUTCOME_ADAPTER: TypeAdapter<TelemetryAuditOutcome> =
        object : TypeAdapter<TelemetryAuditOutcome> {
            private val optionalString: TypeAdapter<Optional<String>> = NoritoAdapters.option(STRING)

            override fun encode(encoder: NoritoEncoder, value: TelemetryAuditOutcome) {
                STRING.encode(encoder, value.traceId)
                UINT64.encode(encoder, value.slotHeight)
                STRING.encode(encoder, value.reviewer)
                STRING.encode(encoder, value.status)
                optionalString.encode(encoder, value.mitigationUrl)
            }

            override fun decode(decoder: NoritoDecoder): TelemetryAuditOutcome {
                val traceId = STRING.decode(decoder)
                val slotHeight = UINT64.decode(decoder)
                val reviewer = STRING.decode(decoder)
                val status = STRING.decode(decoder)
                val mitigationUrl = optionalString.decode(decoder)
                return TelemetryAuditOutcome(traceId, slotHeight, reviewer, status, mitigationUrl)
            }
        }

    private data class TelemetryEntry<T : TelemetryEvent>(
        val matches: (TelemetryEvent) -> Boolean,
        val extractor: (TelemetryEvent) -> Any,
        val factory: (Any) -> T,
        val adapter: TypeAdapter<*>,
    ) {
        fun encode(encoder: NoritoEncoder, value: TelemetryEvent) {
            val payload = extractor(value)
            @Suppress("UNCHECKED_CAST")
            val typedAdapter = adapter as TypeAdapter<Any>
            typedAdapter.encode(encoder, payload)
        }

        fun decode(decoder: NoritoDecoder): T {
            @Suppress("UNCHECKED_CAST")
            val typedAdapter = adapter as TypeAdapter<Any>
            val payload = typedAdapter.decode(decoder)
            return factory(payload)
        }
    }

    private val TELEMETRY_EVENT_ADAPTER: TypeAdapter<TelemetryEvent> = object : TypeAdapter<TelemetryEvent> {
        private val entries: List<TelemetryEntry<*>> = listOf(
            TelemetryEntry(
                { it is TelemetryEncodeEvent },
                { (it as TelemetryEncodeEvent).stats },
                { TelemetryEncodeEvent(it as TelemetryEncodeStats) },
                TELEMETRY_ENCODE_STATS_ADAPTER,
            ),
            TelemetryEntry(
                { it is TelemetryDecodeEvent },
                { (it as TelemetryDecodeEvent).stats },
                { TelemetryDecodeEvent(it as TelemetryDecodeStats) },
                TELEMETRY_DECODE_STATS_ADAPTER,
            ),
            TelemetryEntry(
                { it is TelemetryNetworkEvent },
                { (it as TelemetryNetworkEvent).stats },
                { TelemetryNetworkEvent(it as TelemetryNetworkStats) },
                TELEMETRY_NETWORK_STATS_ADAPTER,
            ),
            TelemetryEntry(
                { it is TelemetrySecurityEvent },
                { (it as TelemetrySecurityEvent).stats },
                { TelemetrySecurityEvent(it as TelemetrySecurityStats) },
                TELEMETRY_SECURITY_STATS_ADAPTER,
            ),
            TelemetryEntry(
                { it is TelemetryEnergyEvent },
                { (it as TelemetryEnergyEvent).stats },
                { TelemetryEnergyEvent(it as TelemetryEnergyStats) },
                TELEMETRY_ENERGY_STATS_ADAPTER,
            ),
            TelemetryEntry(
                { it is TelemetryAuditOutcomeEvent },
                { (it as TelemetryAuditOutcomeEvent).stats },
                { TelemetryAuditOutcomeEvent(it as TelemetryAuditOutcome) },
                TELEMETRY_AUDIT_OUTCOME_ADAPTER,
            ),
        )

        override fun encode(encoder: NoritoEncoder, value: TelemetryEvent) {
            for (i in entries.indices) {
                val entry = entries[i]
                if (entry.matches(value)) {
                    encoder.writeUInt(i.toLong(), 32)
                    entry.encode(encoder, value)
                    return
                }
            }
            throw IllegalArgumentException("Unsupported TelemetryEvent: ${value::class.simpleName}")
        }

        override fun decode(decoder: NoritoDecoder): TelemetryEvent {
            val tag = decoder.readUInt(32)
            require(tag in 0 until entries.size) { "Invalid TelemetryEvent discriminant: $tag" }
            return entries[tag.toInt()].decode(decoder)
        }

        override fun fixedSize(): Int = -1
    }

    private val KEY_UPDATE_ADAPTER: TypeAdapter<KeyUpdate> = object : TypeAdapter<KeyUpdate> {
        override fun encode(encoder: NoritoEncoder, value: KeyUpdate) {
            HASH.encode(encoder, value.sessionId)
            ENCRYPTION_SUITE_ADAPTER.encode(encoder, value.suite)
            UINT16.encode(encoder, value.protocolVersion)
            BYTES.encode(encoder, value.pubEphemeral)
            UINT64.encode(encoder, value.keyCounter)
            SIGNATURE.encode(encoder, value.signature)
        }

        override fun decode(decoder: NoritoDecoder): KeyUpdate {
            val sessionId = HASH.decode(decoder)
            val suite = ENCRYPTION_SUITE_ADAPTER.decode(decoder)
            val protocolVersion = UINT16.decode(decoder)
            val pubEphemeral = BYTES.decode(decoder)
            val keyCounter = UINT64.decode(decoder)
            val signature = SIGNATURE.decode(decoder)
            return KeyUpdate(sessionId, suite, protocolVersion, pubEphemeral, keyCounter, signature)
        }
    }

    private val CONTENT_KEY_UPDATE_ADAPTER: TypeAdapter<ContentKeyUpdate> = object : TypeAdapter<ContentKeyUpdate> {
        override fun encode(encoder: NoritoEncoder, value: ContentKeyUpdate) {
            UINT64.encode(encoder, value.contentKeyId)
            BYTES.encode(encoder, value.gckWrapped)
            UINT64.encode(encoder, value.validFromSegment)
        }

        override fun decode(decoder: NoritoDecoder): ContentKeyUpdate {
            val contentKeyId = UINT64.decode(decoder)
            val gckWrapped = BYTES.decode(decoder)
            val validFrom = UINT64.decode(decoder)
            return ContentKeyUpdate(contentKeyId, gckWrapped, validFrom)
        }
    }

    private val PRIVACY_ROUTE_UPDATE_ADAPTER: TypeAdapter<PrivacyRouteUpdate> =
        object : TypeAdapter<PrivacyRouteUpdate> {
            override fun encode(encoder: NoritoEncoder, value: PrivacyRouteUpdate) {
                HASH.encode(encoder, value.routeId)
                HASH.encode(encoder, value.streamId)
                UINT64.encode(encoder, value.contentKeyId)
                UINT64.encode(encoder, value.validFromSegment)
                UINT64.encode(encoder, value.validUntilSegment)
                BYTES.encode(encoder, value.exitToken)
                OPTIONAL_SORANET_ROUTE.encode(encoder, Optional.ofNullable(value.soranet))
            }

            override fun decode(decoder: NoritoDecoder): PrivacyRouteUpdate {
                val routeId = HASH.decode(decoder)
                val streamId = HASH.decode(decoder)
                val contentKeyId = UINT64.decode(decoder)
                val validFrom = UINT64.decode(decoder)
                val validUntil = UINT64.decode(decoder)
                val exitToken = BYTES.decode(decoder)
                val soranet = OPTIONAL_SORANET_ROUTE.decode(decoder).orElse(null)
                return PrivacyRouteUpdate(routeId, streamId, contentKeyId, validFrom, validUntil, exitToken, soranet)
            }
        }

    private val PRIVACY_ROUTE_ACK_FRAME_ADAPTER: TypeAdapter<PrivacyRouteAckFrame> =
        object : TypeAdapter<PrivacyRouteAckFrame> {
            override fun encode(encoder: NoritoEncoder, value: PrivacyRouteAckFrame) {
                HASH.encode(encoder, value.routeId)
            }

            override fun decode(decoder: NoritoDecoder): PrivacyRouteAckFrame =
                PrivacyRouteAckFrame(HASH.decode(decoder))
        }

    private val CONTROL_ERROR_FRAME_ADAPTER: TypeAdapter<ControlErrorFrame> =
        object : TypeAdapter<ControlErrorFrame> {
            override fun encode(encoder: NoritoEncoder, value: ControlErrorFrame) {
                ERROR_CODE_ADAPTER.encode(encoder, value.code)
                STRING.encode(encoder, value.message)
            }

            override fun decode(decoder: NoritoDecoder): ControlErrorFrame {
                val code = ERROR_CODE_ADAPTER.decode(decoder)
                val message = STRING.decode(decoder)
                return ControlErrorFrame(code, message)
            }
        }

    private data class ControlFrameEntry(
        val variant: ControlFrameVariant,
        val adapter: TypeAdapter<out ControlFramePayload>,
    )

    private val CONTROL_FRAME_ENTRIES: List<ControlFrameEntry> = listOf(
        ControlFrameEntry(ControlFrameVariant.MANIFEST_ANNOUNCE, MANIFEST_ANNOUNCE_FRAME_ADAPTER),
        ControlFrameEntry(ControlFrameVariant.CHUNK_REQUEST, CHUNK_REQUEST_FRAME_ADAPTER),
        ControlFrameEntry(ControlFrameVariant.CHUNK_ACKNOWLEDGE, CHUNK_ACK_FRAME_ADAPTER),
        ControlFrameEntry(ControlFrameVariant.TRANSPORT_CAPABILITIES, TRANSPORT_CAPABILITIES_FRAME_ADAPTER),
        ControlFrameEntry(ControlFrameVariant.CAPABILITY_REPORT, CAPABILITY_REPORT_ADAPTER),
        ControlFrameEntry(ControlFrameVariant.CAPABILITY_ACK, CAPABILITY_ACK_ADAPTER),
        ControlFrameEntry(ControlFrameVariant.FEEDBACK_HINT, FEEDBACK_HINT_FRAME_ADAPTER),
        ControlFrameEntry(ControlFrameVariant.RECEIVER_REPORT, RECEIVER_REPORT_ADAPTER),
        ControlFrameEntry(ControlFrameVariant.KEY_UPDATE, KEY_UPDATE_ADAPTER),
        ControlFrameEntry(ControlFrameVariant.CONTENT_KEY_UPDATE, CONTENT_KEY_UPDATE_ADAPTER),
        ControlFrameEntry(ControlFrameVariant.PRIVACY_ROUTE_UPDATE, PRIVACY_ROUTE_UPDATE_ADAPTER),
        ControlFrameEntry(ControlFrameVariant.PRIVACY_ROUTE_ACK, PRIVACY_ROUTE_ACK_FRAME_ADAPTER),
        ControlFrameEntry(ControlFrameVariant.ERROR, CONTROL_ERROR_FRAME_ADAPTER),
    )

    private val CONTROL_FRAME_ADAPTER: TypeAdapter<ControlFrame> = object : TypeAdapter<ControlFrame> {
        override fun encode(encoder: NoritoEncoder, value: ControlFrame) {
            for (i in CONTROL_FRAME_ENTRIES.indices) {
                val entry = CONTROL_FRAME_ENTRIES[i]
                if (entry.variant == value.variant) {
                    encoder.writeUInt(i.toLong(), 32)
                    @Suppress("UNCHECKED_CAST")
                    val adapter = entry.adapter as TypeAdapter<ControlFramePayload>
                    adapter.encode(encoder, value.payload)
                    return
                }
            }
            throw IllegalArgumentException("Unsupported ControlFrame variant: ${value.variant}")
        }

        override fun decode(decoder: NoritoDecoder): ControlFrame {
            val tag = decoder.readUInt(32)
            require(tag in 0 until CONTROL_FRAME_ENTRIES.size) {
                "Invalid ControlFrame discriminant: $tag"
            }
            val entry = CONTROL_FRAME_ENTRIES[tag.toInt()]
            @Suppress("UNCHECKED_CAST")
            val adapter = entry.adapter as TypeAdapter<ControlFramePayload>
            val payload = adapter.decode(decoder)
            return ControlFrame(entry.variant, payload)
        }
    }

    // --- Public adapter accessors ---

    @JvmStatic fun hashAdapter(): TypeAdapter<ByteArray> = HASH
    @JvmStatic fun signatureAdapter(): TypeAdapter<ByteArray> = SIGNATURE
    @JvmStatic fun bytesAdapter(): TypeAdapter<ByteArray> = BYTES
    @JvmStatic fun profileIdAdapter(): TypeAdapter<ProfileId> = PROFILE_ID_ADAPTER
    @JvmStatic fun capabilityFlagsAdapter(): TypeAdapter<CapabilityFlags> = CAPABILITY_FLAGS_ADAPTER
    @JvmStatic fun ticketCapabilitiesAdapter(): TypeAdapter<TicketCapabilities> = TICKET_CAPABILITIES_ADAPTER
    @JvmStatic fun ticketPolicyAdapter(): TypeAdapter<TicketPolicy> = TICKET_POLICY_ADAPTER
    @JvmStatic fun streamingTicketAdapter(): TypeAdapter<StreamingTicket> = STREAMING_TICKET_ADAPTER
    @JvmStatic fun ticketRevocationAdapter(): TypeAdapter<TicketRevocation> = TICKET_REVOCATION_ADAPTER
    @JvmStatic fun privacyCapabilitiesAdapter(): TypeAdapter<PrivacyCapabilities> = PRIVACY_CAPABILITIES_ADAPTER
    @JvmStatic fun hpkeSuiteMaskAdapter(): TypeAdapter<HpkeSuiteMask> = HPKE_SUITE_MASK_ADAPTER
    @JvmStatic fun encryptionSuiteAdapter(): TypeAdapter<EncryptionSuite> = ENCRYPTION_SUITE_ADAPTER
    @JvmStatic fun transportCapabilitiesAdapter(): TypeAdapter<TransportCapabilities> = TRANSPORT_CAPABILITIES_ADAPTER
    @JvmStatic fun streamMetadataAdapter(): TypeAdapter<StreamMetadata> = STREAM_METADATA_ADAPTER
    @JvmStatic fun privacyRelayAdapter(): TypeAdapter<PrivacyRelay> = PRIVACY_RELAY_ADAPTER
    @JvmStatic fun privacyRouteAdapter(): TypeAdapter<PrivacyRoute> = PRIVACY_ROUTE_ADAPTER
    @JvmStatic fun neuralBundleAdapter(): TypeAdapter<NeuralBundle> = NEURAL_BUNDLE_ADAPTER
    @JvmStatic fun chunkDescriptorAdapter(): TypeAdapter<ChunkDescriptor> = CHUNK_DESCRIPTOR_ADAPTER
    @JvmStatic fun manifestAdapter(): TypeAdapter<ManifestV1> = MANIFEST_ADAPTER
    @JvmStatic fun manifestAnnounceFrameAdapter(): TypeAdapter<ManifestAnnounceFrame> = MANIFEST_ANNOUNCE_FRAME_ADAPTER
    @JvmStatic fun chunkRequestFrameAdapter(): TypeAdapter<ChunkRequestFrame> = CHUNK_REQUEST_FRAME_ADAPTER
    @JvmStatic fun chunkAcknowledgeFrameAdapter(): TypeAdapter<ChunkAcknowledgeFrame> = CHUNK_ACK_FRAME_ADAPTER
    @JvmStatic fun transportCapabilitiesFrameAdapter(): TypeAdapter<TransportCapabilitiesFrame> = TRANSPORT_CAPABILITIES_FRAME_ADAPTER
    @JvmStatic fun capabilityReportAdapter(): TypeAdapter<CapabilityReport> = CAPABILITY_REPORT_ADAPTER
    @JvmStatic fun capabilityAckAdapter(): TypeAdapter<CapabilityAck> = CAPABILITY_ACK_ADAPTER
    @JvmStatic fun audioCapabilityAdapter(): TypeAdapter<AudioCapability> = AUDIO_CAPABILITY_ADAPTER
    @JvmStatic fun audioFrameAdapter(): TypeAdapter<AudioFrame> = AUDIO_FRAME_ADAPTER
    @JvmStatic fun resolutionAdapter(): TypeAdapter<Resolution> = RESOLUTION_ADAPTER
    @JvmStatic fun resolutionCustomAdapter(): TypeAdapter<ResolutionCustom> = RESOLUTION_CUSTOM_ADAPTER
    @JvmStatic fun feedbackHintFrameAdapter(): TypeAdapter<FeedbackHintFrame> = FEEDBACK_HINT_FRAME_ADAPTER
    @JvmStatic fun receiverReportAdapter(): TypeAdapter<ReceiverReport> = RECEIVER_REPORT_ADAPTER
    @JvmStatic fun telemetryEncodeStatsAdapter(): TypeAdapter<TelemetryEncodeStats> = TELEMETRY_ENCODE_STATS_ADAPTER
    @JvmStatic fun telemetryDecodeStatsAdapter(): TypeAdapter<TelemetryDecodeStats> = TELEMETRY_DECODE_STATS_ADAPTER
    @JvmStatic fun telemetryNetworkStatsAdapter(): TypeAdapter<TelemetryNetworkStats> = TELEMETRY_NETWORK_STATS_ADAPTER
    @JvmStatic fun telemetrySecurityStatsAdapter(): TypeAdapter<TelemetrySecurityStats> = TELEMETRY_SECURITY_STATS_ADAPTER
    @JvmStatic fun telemetryEnergyStatsAdapter(): TypeAdapter<TelemetryEnergyStats> = TELEMETRY_ENERGY_STATS_ADAPTER
    @JvmStatic fun telemetryEventAdapter(): TypeAdapter<TelemetryEvent> = TELEMETRY_EVENT_ADAPTER
    @JvmStatic fun keyUpdateAdapter(): TypeAdapter<KeyUpdate> = KEY_UPDATE_ADAPTER
    @JvmStatic fun contentKeyUpdateAdapter(): TypeAdapter<ContentKeyUpdate> = CONTENT_KEY_UPDATE_ADAPTER
    @JvmStatic fun privacyRouteUpdateAdapter(): TypeAdapter<PrivacyRouteUpdate> = PRIVACY_ROUTE_UPDATE_ADAPTER
    @JvmStatic fun privacyRouteAckFrameAdapter(): TypeAdapter<PrivacyRouteAckFrame> = PRIVACY_ROUTE_ACK_FRAME_ADAPTER
    @JvmStatic fun controlErrorFrameAdapter(): TypeAdapter<ControlErrorFrame> = CONTROL_ERROR_FRAME_ADAPTER
    @JvmStatic fun controlFrameAdapter(): TypeAdapter<ControlFrame> = CONTROL_FRAME_ADAPTER
}
