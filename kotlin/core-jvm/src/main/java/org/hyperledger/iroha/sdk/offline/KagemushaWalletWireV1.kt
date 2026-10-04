// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.security.MessageDigest
import java.util.Base64
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.norito.Varint

/**
 * Exact role label of one domain-separated KAGEMUSHA wallet V1 digest `H(role, body)`.
 *
 * Mirrors Rust `iroha_data_model::kagemusha::KagemushaWalletDigestRoleV1` in declaration order.
 * A `*-body` role names the signed transcript of an object; the matching role without the suffix
 * names that signed object's digest `H(role, e || signature)`.
 */
enum class KagemushaWalletDigestRoleV1(
    /** Exact ASCII role label hashed after [KagemushaWalletWireV1.DIGEST_PREFIX]. */
    @JvmField val label: String,
) {
    SCHEME("scheme"),
    RELATION("relation"),
    PROVIDER_CONTRACT("provider-contract"),
    ASSET_SCOPE("asset-scope"),
    ACCOUNT("account"),
    ENROLLMENT_CHALLENGE("enrollment-challenge"),
    ENROLLMENT_ID("enrollment-id"),
    ENROLLMENT_KEY_BINDING("enrollment-key-binding"),
    WALLET_ID("wallet-id"),
    CERTIFICATE_BODY("certificate-body"),
    CERTIFICATE("certificate"),
    CERTIFICATE_SET("certificate-set"),
    CREDENTIAL_BODY("credential-body"),
    CREDENTIAL("credential"),
    SCHEME_POLICY_BODY("scheme-policy-body"),
    SCHEME_POLICY("scheme-policy"),
    FEE_SCHEDULE_BODY("fee-schedule-body"),
    FEE_SCHEDULE("fee-schedule"),
    BLACKLIST_BODY("blacklist-body"),
    BLACKLIST("blacklist"),
    BLACKLIST_LEAF("blacklist-leaf"),
    BLACKLIST_NODE("blacklist-node"),
    QUOTA_SHARE_BODY("quota-share-body"),
    QUOTA_SHARE("quota-share"),
    QUOTA_WINDOW("quota-window"),
    QUOTA_NODE("quota-node"),
    TIME_ANCHOR_BODY("time-anchor-body"),
    TIME_ANCHOR("time-anchor"),
    OFFER_BODY("offer-body"),
    SESSION_CONTROL_BODY("session-control-body"),
    REQUEST_BODY("request-body"),
    REQUEST("request"),
    CREDIT("credit"),
    DEPENDENCIES("dependencies"),
    STATEMENT("statement"),
    PROOF("proof"),
    RECEIPT_BODY("receipt-body"),
    RECEIPT("receipt"),
    PACKAGE("package"),
    PAYMENT("payment"),
    CREDIT_STATUS_STATEMENT("credit-status-statement"),
    CREDITED("credited"),
    OPERATION_ID("operation-id"),
    OUTPUT("output"),
    CAPSULE("capsule"),
    MARKER("marker"),
    COMPLETION("completion"),
    VOUCHER_BODY("voucher-body"),
    VOUCHER("voucher"),
    UNLOAD_NULLIFIER("unload-nullifier"),
    LEDGER_CONTROL_BODY("ledger-control-body"),
    RENEWAL_CHALLENGE("renewal-challenge"),
    RENEWAL_KEY_BINDING("renewal-key-binding"),
    RENEWAL_ASSERTION("renewal-assertion"),
    ARTIFACT_MANIFEST_BODY("artifact-manifest-body"),
    ARTIFACT_MANIFEST("artifact-manifest"),
    CHARGE_QUOTE_BODY("charge-quote-body"),
    CHARGE_QUOTE("charge-quote"),
    EVIDENCE("evidence"),
    ;

    companion object {
        /** Role whose label is exactly [label], or `null` for an unknown label. */
        @JvmStatic
        fun fromLabel(label: String): KagemushaWalletDigestRoleV1? = entries.firstOrNull { it.label == label }
    }
}

/** Peer message kind carried by a KAGEMUSHA wallet V1 envelope; its tag selects the frame bound. */
enum class KagemushaWalletMessageKindV1(
    /** Norito `u32` enum tag of `KagemushaWalletMessageV1`. */
    @JvmField val wireTag: Int,
    /** Rust variant name of `KagemushaWalletMessageV1`. */
    @JvmField val label: String,
    /** Maximum complete canonical envelope frame, header and padding included. */
    @JvmField val maximumFrameBytes: Int,
    /** Maximum complete `kgm1:` text of an envelope of this kind. */
    @JvmField val maximumTextBytes: Int,
) {
    OFFER(
        1,
        "Offer",
        KagemushaWalletWireV1.SESSION_MAX_BYTES,
        KagemushaWalletWireV1.SESSION_TEXT_MAX_BYTES,
    ),
    REQUEST(
        2,
        "Request",
        KagemushaWalletWireV1.MESSAGE_MAX_BYTES,
        KagemushaWalletWireV1.MESSAGE_TEXT_MAX_BYTES,
    ),
    PAYMENT(
        3,
        "Payment",
        KagemushaWalletWireV1.MESSAGE_MAX_BYTES,
        KagemushaWalletWireV1.MESSAGE_TEXT_MAX_BYTES,
    ),
    CREDITED(
        4,
        "Credited",
        KagemushaWalletWireV1.MESSAGE_MAX_BYTES,
        KagemushaWalletWireV1.MESSAGE_TEXT_MAX_BYTES,
    ),
    SESSION_CONTROL(
        5,
        "SessionControl",
        KagemushaWalletWireV1.SESSION_MAX_BYTES,
        KagemushaWalletWireV1.SESSION_TEXT_MAX_BYTES,
    ),
    POLICY_DATA(
        6,
        "PolicyData",
        KagemushaWalletWireV1.MESSAGE_MAX_BYTES,
        KagemushaWalletWireV1.MESSAGE_TEXT_MAX_BYTES,
    ),
    ;

    companion object {
        /** Kind with exactly [tag], or `null` for an unknown tag. */
        @JvmStatic
        fun fromWireTag(tag: Int): KagemushaWalletMessageKindV1? = entries.firstOrNull { it.wireTag == tag }

        /** Kind whose Rust variant name is exactly [label], or `null` for an unknown name. */
        @JvmStatic
        fun fromLabel(label: String): KagemushaWalletMessageKindV1? = entries.firstOrNull { it.label == label }
    }
}

/**
 * One canonical KAGEMUSHA wallet V1 envelope frame whose Norito header, envelope version and
 * message tag were validated by [KagemushaWalletWireV1.inspectEnvelope].
 *
 * This is a structural transport check only: the carried message is not decoded or verified
 * and grants no monetary authority. Equality is identity; compare [frame] bytes instead.
 */
class KagemushaWalletEnvelopeFrameV1 internal constructor(
    frame: ByteArray,
    /** Message kind read from the envelope payload tag. */
    @JvmField val kind: KagemushaWalletMessageKindV1,
    /** Norito layout flags; always [KagemushaWalletWireV1.ENVELOPE_FLAGS]. */
    @JvmField val flags: Int,
    /** Zero padding between the header and the payload. */
    @JvmField val paddingLength: Int,
    /** Payload bytes after the header and padding. */
    @JvmField val payloadLength: Int,
    /** CRC64-XZ of the payload, as stored little-endian in the header. */
    @JvmField val crc64: Long,
) {
    private val bytes: ByteArray = frame.copyOf()

    /** Complete frame length, header and padding included. */
    val frameLength: Int
        get() = bytes.size

    /** 16-byte Norito schema hash of the envelope frame name; a defensive copy. */
    val schemaHash: ByteArray
        get() = bytes.copyOfRange(SCHEMA_HASH_OFFSET, SCHEMA_HASH_OFFSET + SCHEMA_HASH_BYTES)

    /** Complete canonical frame bytes; a defensive copy. */
    fun frame(): ByteArray = bytes.copyOf()

    /** Strict `kgm1:` text of this frame, within [KagemushaWalletMessageKindV1.maximumTextBytes]. */
    fun toText(): String = KagemushaWalletWireV1.encodeText(bytes).also { text ->
        check(text.length <= kind.maximumTextBytes) {
            "KAGEMUSHA wallet V1 ${kind.label} text exceeds ${kind.maximumTextBytes} bytes"
        }
    }

    private companion object {
        const val SCHEMA_HASH_OFFSET = 6
        const val SCHEMA_HASH_BYTES = 16
    }
}

/**
 * Bounds, domain digests, envelope header validation and strict `kgm1:` text for the KAGEMUSHA
 * wallet V1 protocol, matching the Rust owner `iroha_data_model::kagemusha::kagemusha_wallet_v1`
 * and `fixtures/kagemusha/wallet_v1_vectors.json`.
 *
 * Every peer message travels as one canonical Norito frame of `KagemushaWalletEnvelopeV1`: a
 * 40-byte header with flags `0x02`, eight zero padding bytes (the envelope archive alignment is
 * 16), then the payload. Bounds count the complete frame.
 */
object KagemushaWalletWireV1 {
    /** Version carried by every wallet V1 object and transcript. */
    const val VERSION: Int = 1

    /** Text transport discriminator. */
    const val TEXT_PREFIX: String = "kgm1:"

    /** ASCII prefix of every wallet V1 digest preimage. */
    const val DIGEST_PREFIX: String = "iroha:kagemusha:wallet:v1:"

    /** Length of every wallet V1 digest. */
    const val DIGEST_BYTES: Int = 32

    /** Maximum complete envelope frame for Offer and SessionControl. */
    const val SESSION_MAX_BYTES: Int = 2_048

    /** Maximum complete envelope frame for Request, Payment, Credited and PolicyData. */
    const val MESSAGE_MAX_BYTES: Int = 10_000

    /** Maximum complete `kgm1:` text for a session-bounded envelope. */
    const val SESSION_TEXT_MAX_BYTES: Int = 2_736

    /** Maximum complete `kgm1:` text for a message-bounded envelope. */
    const val MESSAGE_TEXT_MAX_BYTES: Int = 13_339

    /** Provisional G1 decode cap for one transition proof (design C2). */
    // TODO(G3): replace with the measured cap of the frozen relation, together with Rust.
    const val PROOF_MAX_BYTES: Int = 6_016

    /** Provisional G1 decode cap for one CreditStatus proof (design C2). */
    // TODO(G3): replace with the measured cap of the frozen relation, together with Rust.
    const val CREDIT_STATUS_PROOF_MAX_BYTES: Int = 2_000

    /** Maximum certificates in one certificate set. */
    const val CERTIFICATE_SET_MAX: Int = 3

    /** Norito frame name of the envelope; its schema hash is derived from this name. */
    const val ENVELOPE_FRAME_NAME: String =
        "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletEnvelopeV1"

    /** Archive alignment of the envelope payload (it contains `u128`). */
    const val ENVELOPE_PAYLOAD_ALIGNMENT: Int = 16

    /** Exact zero padding between the 40-byte header and the envelope payload. */
    const val ENVELOPE_PADDING_BYTES: Int =
        (ENVELOPE_PAYLOAD_ALIGNMENT - NoritoHeader.HEADER_LENGTH % ENVELOPE_PAYLOAD_ALIGNMENT) %
            ENVELOPE_PAYLOAD_ALIGNMENT

    /** Exact header flags of every canonical envelope frame (`COMPACT_LEN`). */
    const val ENVELOPE_FLAGS: Int = NoritoHeader.COMPACT_LEN

    private const val PAYLOAD_OFFSET: Int = NoritoHeader.HEADER_LENGTH + ENVELOPE_PADDING_BYTES
    private const val SCHEMA_HASH_OFFSET: Int = 6
    private const val COMPRESSION_OFFSET: Int = 22
    private const val PAYLOAD_LENGTH_OFFSET: Int = 23
    private const val CRC64_OFFSET: Int = 31
    private const val FLAGS_OFFSET: Int = 39
    private const val VERSION_FIELD_BYTES: Long = 2L
    private const val TAG_BYTES: Int = 4

    private val DIGEST_PREFIX_BYTES: ByteArray = DIGEST_PREFIX.toByteArray(Charsets.US_ASCII)
    private val ENVELOPE_SCHEMA_HASH: ByteArray = SchemaHash.hash16(ENVELOPE_FRAME_NAME)

    /** 16-byte Norito schema hash of [ENVELOPE_FRAME_NAME]; a defensive copy. */
    @JvmStatic
    fun envelopeSchemaHash(): ByteArray = ENVELOPE_SCHEMA_HASH.copyOf()

    /** Exact `kgm1:` text length of a frame of [frameBytes]: the prefix and unpadded base64url. */
    @JvmStatic
    fun textBytesForFrame(frameBytes: Int): Int {
        require(frameBytes in 0..MESSAGE_MAX_BYTES) {
            "KAGEMUSHA wallet V1 frame length $frameBytes is outside 0..$MESSAGE_MAX_BYTES"
        }
        val remainder = frameBytes % 3
        val tail = if (remainder == 0) 0 else remainder + 1
        return TEXT_PREFIX.length + frameBytes / 3 * 4 + tail
    }

    /**
     * Exact SHA-256 preimage of `H(role, body)`, which is also the ECDSA message of a signed body:
     * `prefix || role || 0x00 || LE64(len(body)) || body`.
     */
    @JvmStatic
    fun preimage(role: KagemushaWalletDigestRoleV1, body: ByteArray): ByteArray {
        val label = role.label.toByteArray(Charsets.US_ASCII)
        val out = ByteArray(DIGEST_PREFIX_BYTES.size + label.size + 1 + 8 + body.size)
        var cursor = 0
        DIGEST_PREFIX_BYTES.copyInto(out, cursor)
        cursor += DIGEST_PREFIX_BYTES.size
        label.copyInto(out, cursor)
        cursor += label.size
        out[cursor] = 0
        cursor += 1
        val length = body.size.toLong()
        for (index in 0 until 8) {
            out[cursor + index] = (length ushr (8 * index)).toByte()
        }
        cursor += 8
        body.copyInto(out, cursor)
        return out
    }

    /** Domain-separated digest `H(role, body) = SHA-256(preimage(role, body))`. */
    @JvmStatic
    fun digest(role: KagemushaWalletDigestRoleV1, body: ByteArray): ByteArray =
        MessageDigest.getInstance("SHA-256").digest(preimage(role, body))

    /**
     * Digest of one signed object, `H(role, e || signature)`, where `e` is its signed body digest
     * and the signature is the canonical 64-byte low-S `r || s`.
     *
     * @throws IllegalArgumentException for a body digest that is not 32 bytes or a non-canonical
     * signature.
     */
    @JvmStatic
    fun signedObjectDigest(
        role: KagemushaWalletDigestRoleV1,
        bodyDigest: ByteArray,
        rawSignature: ByteArray,
    ): ByteArray {
        require(bodyDigest.size == DIGEST_BYTES) {
            "KAGEMUSHA wallet V1 body digest must be exactly $DIGEST_BYTES bytes"
        }
        val signature = KagemushaP256Codec.requireRawLowSSignature(rawSignature)
        return digest(role, bodyDigest + signature)
    }

    /**
     * Encode one canonical frame as `kgm1:` and unpadded base64url.
     *
     * The frame must be non-empty and at most [MESSAGE_MAX_BYTES], the domain that [decodeText]
     * accepts, so every produced text decodes back to the same bytes.
     */
    @JvmStatic
    fun encodeText(frame: ByteArray): String {
        require(frame.isNotEmpty()) { "KAGEMUSHA wallet V1 frame is empty" }
        require(frame.size <= MESSAGE_MAX_BYTES) {
            "KAGEMUSHA wallet V1 frame exceeds $MESSAGE_MAX_BYTES bytes"
        }
        return TEXT_PREFIX + Base64.getUrlEncoder().withoutPadding().encodeToString(frame)
    }

    /**
     * Strictly decode `kgm1:` text into frame bytes, with the rules of the Rust codec in order:
     * at most [MESSAGE_TEXT_MAX_BYTES], the exact prefix, a non-empty body of only the base64url
     * alphabet (no padding or whitespace), a body length that is not `1 mod 4`, and re-encoding
     * equality (which rejects nonzero trailing bits).
     */
    @JvmStatic
    fun decodeText(text: String): ByteArray {
        require(text.length <= MESSAGE_TEXT_MAX_BYTES) {
            "KAGEMUSHA wallet V1 text exceeds $MESSAGE_TEXT_MAX_BYTES bytes"
        }
        require(text.startsWith(TEXT_PREFIX)) { "KAGEMUSHA wallet V1 text prefix is invalid" }
        val body = text.substring(TEXT_PREFIX.length)
        require(body.isNotEmpty()) { "KAGEMUSHA wallet V1 text body is empty" }
        require(body.all(::isBase64UrlCharacter)) { "KAGEMUSHA wallet V1 text alphabet is invalid" }
        require(body.length % 4 != 1) { "KAGEMUSHA wallet V1 text length is invalid" }
        val frame = try {
            Base64.getUrlDecoder().decode(body)
        } catch (error: IllegalArgumentException) {
            throw IllegalArgumentException("KAGEMUSHA wallet V1 base64url is invalid", error)
        }
        require(Base64.getUrlEncoder().withoutPadding().encodeToString(frame) == body) {
            "KAGEMUSHA wallet V1 base64url is non-canonical"
        }
        return frame
    }

    /**
     * Validate one canonical envelope frame header and its per-kind bound.
     *
     * Checks, in the Rust decode order: the largest bound before parsing; magic, major and minor
     * version; the schema hash of [ENVELOPE_FRAME_NAME]; no compression; flags exactly
     * [ENVELOPE_FLAGS]; exactly [ENVELOPE_PADDING_BYTES] zero padding bytes; the payload length;
     * the CRC64; the envelope version field and the exact field spans of the message; a known
     * message tag; and finally the bound of the kind that tag selects.
     *
     * The carried message itself is not decoded or verified.
     * TODO(G4): decode and validate the typed wallet message once the Kotlin exchange
     * integration owns the typed codec.
     *
     * @throws IllegalArgumentException for any violation.
     */
    @JvmStatic
    fun inspectEnvelope(frame: ByteArray): KagemushaWalletEnvelopeFrameV1 {
        val bytes = frame.copyOf()
        require(bytes.size <= MESSAGE_MAX_BYTES) {
            "KAGEMUSHA wallet V1 envelope exceeds $MESSAGE_MAX_BYTES bytes"
        }
        require(bytes.size > PAYLOAD_OFFSET) { "KAGEMUSHA wallet V1 envelope is truncated" }
        require(bytes.copyOfRange(0, NoritoHeader.MAGIC.size).contentEquals(NoritoHeader.MAGIC)) {
            "KAGEMUSHA wallet V1 envelope magic is invalid"
        }
        require(
            unsigned(bytes[4]) == NoritoHeader.MAJOR_VERSION &&
                unsigned(bytes[5]) == NoritoHeader.MINOR_VERSION,
        ) { "KAGEMUSHA wallet V1 envelope Norito version is unsupported" }
        require(
            bytes.copyOfRange(SCHEMA_HASH_OFFSET, SCHEMA_HASH_OFFSET + ENVELOPE_SCHEMA_HASH.size)
                .contentEquals(ENVELOPE_SCHEMA_HASH),
        ) { "KAGEMUSHA wallet V1 envelope schema hash does not match $ENVELOPE_FRAME_NAME" }
        require(unsigned(bytes[COMPRESSION_OFFSET]) == NoritoHeader.COMPRESSION_NONE) {
            "KAGEMUSHA wallet V1 envelope must not be compressed"
        }
        val flags = unsigned(bytes[FLAGS_OFFSET])
        require(flags == ENVELOPE_FLAGS) { "KAGEMUSHA wallet V1 envelope flags are not canonical" }
        for (index in NoritoHeader.HEADER_LENGTH until PAYLOAD_OFFSET) {
            require(bytes[index].toInt() == 0) { "KAGEMUSHA wallet V1 envelope padding is not zero" }
        }
        val payloadLength = bytes.size - PAYLOAD_OFFSET
        require(readLongLe(bytes, PAYLOAD_LENGTH_OFFSET) == payloadLength.toLong()) {
            "KAGEMUSHA wallet V1 envelope payload length does not match the frame"
        }
        val payload = bytes.copyOfRange(PAYLOAD_OFFSET, bytes.size)
        val crc64 = readLongLe(bytes, CRC64_OFFSET)
        require(CRC64.compute(payload) == crc64) { "KAGEMUSHA wallet V1 envelope CRC64 mismatch" }
        val kind = envelopeKind(payload)
        require(bytes.size <= kind.maximumFrameBytes) {
            "KAGEMUSHA wallet V1 ${kind.label} envelope exceeds ${kind.maximumFrameBytes} bytes"
        }
        return KagemushaWalletEnvelopeFrameV1(
            bytes,
            kind,
            flags,
            ENVELOPE_PADDING_BYTES,
            payloadLength,
            crc64,
        )
    }

    /** Strictly decode `kgm1:` text and validate the envelope frame it carries. */
    @JvmStatic
    fun decodeEnvelopeText(text: String): KagemushaWalletEnvelopeFrameV1 {
        val envelope = inspectEnvelope(decodeText(text))
        require(text.length <= envelope.kind.maximumTextBytes) {
            "KAGEMUSHA wallet V1 ${envelope.kind.label} text exceeds ${envelope.kind.maximumTextBytes} bytes"
        }
        return envelope
    }

    /**
     * Read the envelope payload `[len] version: u16 [len] (u32 tag [len] variant)` with compact
     * lengths, requiring version 1, exact field spans and a known tag.
     */
    private fun envelopeKind(payload: ByteArray): KagemushaWalletMessageKindV1 {
        val versionField = Varint.decode(payload, 0)
        require(versionField.value == VERSION_FIELD_BYTES) {
            "KAGEMUSHA wallet V1 envelope version field is not two bytes"
        }
        var cursor = versionField.nextOffset
        require(cursor + VERSION_FIELD_BYTES.toInt() <= payload.size) {
            "KAGEMUSHA wallet V1 envelope is truncated"
        }
        val version = unsigned(payload[cursor]) or (unsigned(payload[cursor + 1]) shl 8)
        require(version == VERSION) { "KAGEMUSHA wallet V1 envelope version $version is unsupported" }
        cursor += VERSION_FIELD_BYTES.toInt()
        val messageField = Varint.decode(payload, cursor)
        cursor = messageField.nextOffset
        require(messageField.value == (payload.size - cursor).toLong()) {
            "KAGEMUSHA wallet V1 envelope message field does not span the payload"
        }
        require(payload.size - cursor > TAG_BYTES) { "KAGEMUSHA wallet V1 envelope message is truncated" }
        val tag = readIntLe(payload, cursor)
        val kind = KagemushaWalletMessageKindV1.fromWireTag(tag)
            ?: throw IllegalArgumentException("KAGEMUSHA wallet V1 envelope message tag $tag is unknown")
        cursor += TAG_BYTES
        val variantField = Varint.decode(payload, cursor)
        cursor = variantField.nextOffset
        require(variantField.value > 0L && variantField.value == (payload.size - cursor).toLong()) {
            "KAGEMUSHA wallet V1 envelope ${kind.label} field does not span the message"
        }
        return kind
    }

    private fun readLongLe(bytes: ByteArray, offset: Int): Long {
        var value = 0L
        for (index in 7 downTo 0) {
            value = (value shl 8) or unsigned(bytes[offset + index]).toLong()
        }
        return value
    }

    private fun readIntLe(bytes: ByteArray, offset: Int): Int {
        var value = 0
        for (index in 3 downTo 0) {
            value = (value shl 8) or unsigned(bytes[offset + index])
        }
        return value
    }

    private fun unsigned(value: Byte): Int = value.toInt() and 0xff

    private fun isBase64UrlCharacter(character: Char): Boolean =
        character in 'A'..'Z' ||
            character in 'a'..'z' ||
            character in '0'..'9' ||
            character == '-' ||
            character == '_'
}
