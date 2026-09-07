// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.ByteOrder
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/** Exact bounded public projections; native code separately authenticates every authority claim. */
object KagemushaEnrolledOpenChallengeCodecV1 {
    const val MAXIMUM_ARCHIVE_BYTES: Int = 16 * 1024
    private const val CHALLENGE_SCHEMA = "iroha.kagemusha.v1.enrolled-open-account-challenge"
    private const val SOURCE_SCHEMA = "iroha.kagemusha.v1.enrolled-open-authority-source"
    private const val ALIGNMENT = 16

    @JvmStatic
    fun encodeAccountChallengeShape(value: KagemushaEnrolledOpenAccountChallengeV1): ByteArray =
        frame(value, CHALLENGE_SCHEMA, CHALLENGE)

    @JvmStatic
    fun decodeAccountChallengeShapeExact(bytes: ByteArray): KagemushaEnrolledOpenAccountChallengeV1 =
        decodeExact(bytes, CHALLENGE_SCHEMA, CHALLENGE, ::encodeAccountChallengeShape)

    @JvmStatic
    fun encodeAuthoritySourceShape(value: KagemushaEnrolledOpenAuthoritySourceV1): ByteArray =
        frame(value, SOURCE_SCHEMA, SOURCE)

    @JvmStatic
    fun decodeAuthoritySourceShapeExact(bytes: ByteArray): KagemushaEnrolledOpenAuthoritySourceV1 =
        decodeExact(bytes, SOURCE_SCHEMA, SOURCE, ::encodeAuthoritySourceShape)

    /**
     * Correlate all caller-selected scope and native-request fields, then derive Rust HashOf.
     * Sign the returned 32 bytes directly with the account's Ed25519 key; do not prehash again.
     * Matching these untrusted projections supplies neither native possession nor KYC approval.
     */
    @JvmStatic
    fun accountSigningMessageShape(
        challenge: KagemushaEnrolledOpenAccountChallengeV1,
        expectedSelector: KagemushaEnrolledOpenSelectorV1,
        expectedNonce: ByteArray,
        expectedReleaseId: ByteArray,
        expectedHardwarePolicyDigest: ByteArray,
        expectedCoreAuthorizationKeyReference: ByteArray,
    ): ByteArray {
        val actualSelector = KagemushaEnrolledOpenSelectorV1(1, challenge.owner, challenge.enrollmentId())
        require(KagemushaNoritoV1.encodeEnrolledOpenSelectorShape(actualSelector).contentEquals(
            KagemushaNoritoV1.encodeEnrolledOpenSelectorShape(expectedSelector))) { "challenge owner does not match selection" }
        require(challenge.nonce().contentEquals(fixed32(expectedNonce, "expectedNonce"))) { "challenge nonce mismatch" }
        require(challenge.releaseId().contentEquals(fixed32(expectedReleaseId, "expectedReleaseId"))) { "challenge release mismatch" }
        require(challenge.hardwarePolicyDigest().contentEquals(fixed32(expectedHardwarePolicyDigest, "expectedHardwarePolicyDigest"))) {
            "challenge hardware policy mismatch"
        }
        require(challenge.coreAuthorizationKeyReference().contentEquals(
            fixed32(expectedCoreAuthorizationKeyReference, "expectedCoreAuthorizationKeyReference"))) { "challenge Core key mismatch" }
        val archive = encodeAccountChallengeShape(challenge)
        // Rust HashOf::new -> Encode::encode_to -> bare COMPACT_LEN payload. HashWriter uses
        // BLAKE2b-256 and sets the last byte's low bit; the canonical archive header is excluded.
        return IrohaHash.prehash(NoritoHeader.decode(archive, null).payload)
    }

    private val ANCHOR = adapter<KagemushaDurabilityAnchorStatementV1>(
        encode = { e, v ->
            uintField(e, v.metadataRevision)
            field(e) { it.writeUInt(v.version.toLong(), 16) }
            field(e) { it.writeBytes(KagemushaDeviceOperationCodecV1.lane(v.lane)) }
            digestAliasField(e, v.stateCommitment())
            field(e) { it.writeBytes(KagemushaDeviceOperationCodecV1.hardwareEpoch(v.hardwareEpoch)) }
            field(e) { it.writeBytes(KagemushaDeviceOperationCodecV1.policyBinding(v.devicePolicyBinding)) }
            digestAliasField(e, v.stateNonceCommitment())
            uintField(e, v.logicalSequence)
            uintField(e, v.journalRevision)
            uintField(e, v.inboxRevision)
            digestAliasField(e, v.snapshotCommitment())
        },
        decode = { d -> KagemushaDurabilityAnchorStatementV1(
            uintField(d), numberField(d, 16).toInt(),
            KagemushaDeviceOperationCodecV1.decodeLane(rawField(d)), digestAliasField(d),
            KagemushaDeviceOperationCodecV1.decodeHardwareEpoch(rawField(d)),
            KagemushaDeviceOperationCodecV1.decodePolicyBinding(rawField(d)), digestAliasField(d),
            uintField(d), uintField(d), uintField(d), digestAliasField(d)) },
    )

    private val SOURCE = adapter<KagemushaEnrolledOpenAuthoritySourceV1>(
        encode = { e, v -> when (v) {
            is KagemushaEnrolledOpenAuthoritySourceV1.InitialCertificate -> {
                e.writeUInt(0, 32)
                rawField(e, v.certificateDigest())
            }
            is KagemushaEnrolledOpenAuthoritySourceV1.RecoveryCheckpoint -> {
                e.writeUInt(1, 32)
                nestedField(e, ANCHOR, v.statement)
                rawField(e, v.terminalCertificateDigest())
            }
        } },
        decode = { d ->
            val tag = d.readUInt(32)
            require(tag == 0L || tag == 1L) { "unknown enrolled-open authority source" }
            if (tag == 0L) KagemushaEnrolledOpenAuthoritySourceV1.InitialCertificate(exactField(d, 32))
                else KagemushaEnrolledOpenAuthoritySourceV1.RecoveryCheckpoint(nestedField(d, ANCHOR), exactField(d, 32))
        },
    )

    private val CHALLENGE = adapter<KagemushaEnrolledOpenAccountChallengeV1>(
        encode = { e, v ->
            field(e) { it.writeUInt(v.version.toLong(), 16) }
            nestedField(e, NoritoAdapters.stringAdapter(), v.domain)
            rawField(e, v.enrollmentId())
            nestedField(e, KagemushaNoritoV1.RETAIL_ENROLLMENT_OWNER_ADAPTER, v.owner)
            rawField(e, v.nonce())
            nestedField(e, SOURCE, v.authoritySource)
            rawField(e, v.releaseId())
            rawField(e, v.hardwarePolicyDigest())
            rawField(e, v.coreAuthorizationKeyReference())
            field(e) { it.writeUInt(v.lifetimeMs, 64) }
        },
        decode = { d -> KagemushaEnrolledOpenAccountChallengeV1(numberField(d, 16).toInt(),
            nestedField(d, NoritoAdapters.stringAdapter()), exactField(d, 32),
            nestedField(d, KagemushaNoritoV1.RETAIL_ENROLLMENT_OWNER_ADAPTER), exactField(d, 32),
            nestedField(d, SOURCE), exactField(d, 32), exactField(d, 32), exactField(d, 32), numberField(d, 64)) },
    )

    private fun <T> adapter(encode: (NoritoEncoder, T) -> Unit, decode: (NoritoDecoder) -> T): TypeAdapter<T> =
        object : TypeAdapter<T> {
            override fun encode(encoder: NoritoEncoder, value: T) = encode.invoke(encoder, value)
            override fun decode(decoder: NoritoDecoder): T = decode.invoke(decoder)
        }

    private fun field(parent: NoritoEncoder, write: (NoritoEncoder) -> Unit) {
        val child = parent.childEncoder()
        write(child)
        val bytes = child.toByteArray()
        parent.writeLength(bytes.size.toLong(), true)
        parent.writeBytes(bytes)
    }
    private fun readField(parent: NoritoDecoder): NoritoDecoder {
        val length = parent.readLength(true)
        require(length >= 0 && length <= parent.remaining().toLong()) { "truncated challenge field" }
        return NoritoDecoder(parent.readBytes(length.toInt()), parent.flags)
    }
    private fun rawField(parent: NoritoEncoder, bytes: ByteArray) = field(parent) { it.writeBytes(bytes) }
    private fun rawField(parent: NoritoDecoder): ByteArray = readField(parent).let { it.readBytes(it.remaining()) }
    private fun exactField(parent: NoritoDecoder, width: Int): ByteArray = rawField(parent).also {
        require(it.size == width) { "challenge field width mismatch" }
    }
    private fun <T> nestedField(parent: NoritoEncoder, adapter: TypeAdapter<T>, value: T) = field(parent) { adapter.encode(it, value) }
    private fun <T> nestedField(parent: NoritoDecoder, adapter: TypeAdapter<T>): T {
        val child = readField(parent)
        return adapter.decode(child).also { require(child.remaining() == 0) { "trailing nested challenge bytes" } }
    }
    private fun numberField(parent: NoritoDecoder, bits: Int): Long = ByteBuffer.wrap(exactField(parent, bits / 8))
        .order(ByteOrder.LITTLE_ENDIAN).let { if (bits == 16) it.short.toLong() and 0xffffL else it.long }
    private fun uintField(parent: NoritoEncoder, value: BigInteger) = field(parent) { child ->
        val big = value.toByteArray()
        val little = ByteArray(16)
        repeat(minOf(big.size, 16)) { little[it] = big[big.lastIndex - it] }
        child.writeBytes(little)
    }
    private fun uintField(parent: NoritoDecoder): BigInteger = BigInteger(1, exactField(parent, 16).reversedArray())
    private fun digestAliasField(parent: NoritoEncoder, value: ByteArray) = field(parent) { alias ->
        value.forEach { byte -> field(alias) { it.writeBytes(byteArrayOf(byte)) } }
    }
    private fun digestAliasField(parent: NoritoDecoder): ByteArray {
        val alias = readField(parent)
        return ByteArray(32) { exactField(alias, 1)[0] }.also { require(alias.remaining() == 0) }
    }

    private fun <T> frame(value: T, schema: String, adapter: TypeAdapter<T>): ByteArray {
        val raw = NoritoCodec.encode(value, schema, adapter)
        val padding = (ALIGNMENT - NoritoHeader.HEADER_LENGTH % ALIGNMENT) % ALIGNMENT
        val archive = raw.copyOfRange(0, NoritoHeader.HEADER_LENGTH) + ByteArray(padding) +
            raw.copyOfRange(NoritoHeader.HEADER_LENGTH, raw.size)
        require(archive.size <= MAXIMUM_ARCHIVE_BYTES) { "enrolled-open challenge is oversized" }
        return archive
    }
    private fun <T> decodeExact(bytes: ByteArray, schema: String, adapter: TypeAdapter<T>, encode: (T) -> ByteArray): T {
        require(bytes.size in NoritoHeader.HEADER_LENGTH..MAXIMUM_ARCHIVE_BYTES) { "empty, truncated or oversized enrolled-open archive" }
        val canonical = bytes.copyOf()
        require(canonical[22].toInt() == NoritoHeader.COMPRESSION_NONE) { "compressed enrolled-open archive" }
        require(canonical[NoritoHeader.HEADER_LENGTH - 1].toInt() == NoritoHeader.COMPACT_LEN) { "noncanonical enrolled-open layout" }
        val payloadLength = ByteBuffer.wrap(canonical).order(ByteOrder.LITTLE_ENDIAN).getLong(23)
        val padding = (ALIGNMENT - NoritoHeader.HEADER_LENGTH % ALIGNMENT) % ALIGNMENT
        require(payloadLength >= 0 && payloadLength == (canonical.size - NoritoHeader.HEADER_LENGTH - padding).toLong()) {
            "enrolled-open payload length mismatch"
        }
        val result = NoritoCodec.decode(canonical, adapter, schema)
        require(encode(result).contentEquals(canonical)) { "enrolled-open archive is not canonical" }
        return result
    }
}
