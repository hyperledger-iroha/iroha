// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.attested

import java.util.Base64
import org.hyperledger.iroha.sdk.offline.IrohaPeerCanonicalPayload
import org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadKind
import org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile
import org.hyperledger.iroha.sdk.offline.IrohaPeerQRCodecV1
import org.hyperledger.iroha.sdk.offline.IrohaPeerQRScanSessionV1
import org.hyperledger.iroha.sdk.offline.IrohaPeerWireCompressionPolicyV1
import org.hyperledger.iroha.sdk.offline.IrohaPeerWireMessageV1

/** The three attested-app peer messages, bound to the KAGEMUSHA V1 IPM1 kinds. */
enum class KagemushaPeerMessageKind(
    @JvmField val ipm1Kind: IrohaPeerPayloadKind,
    @JvmField val schema: String,
) {
    REQUEST(IrohaPeerPayloadKind.REQUEST, KagemushaAttestedPaymentRequestV1.SCHEMA),
    PAYMENT(IrohaPeerPayloadKind.PAYMENT, KagemushaAttestedPaymentV1.SCHEMA),
    ACKNOWLEDGEMENT(IrohaPeerPayloadKind.ACKNOWLEDGEMENT, KagemushaAttestedAcknowledgementV1.SCHEMA),
}

/**
 * One opaque, canonically decoded attested-app peer message.
 *
 * It travels as IQR1 QR frames of an IPM1 profile-2 (`KAGEMUSHA_ATTESTED_V1`) message, as raw
 * IPM1 bytes for NFC or nearby transports, or as `kga1:` text (unpadded base64url of the
 * canonical Norito value). Decoding validates the exact type and canonical encoding; it grants no
 * authority by itself.
 */
class KagemushaPeerMessage private constructor(
    @JvmField val kind: KagemushaPeerMessageKind,
    canonical: ByteArray,
    private val value: Any,
) {
    private val bytes = canonical.copyOf()

    /** Exact canonical Norito bytes. */
    fun canonicalBytes(): ByteArray = bytes.copyOf()

    /** The decoded payment request; throws unless [kind] is REQUEST. */
    fun request(): KagemushaAttestedPaymentRequestV1 =
        value as? KagemushaAttestedPaymentRequestV1 ?: throw IllegalStateException("peer message is not a request")

    /** The decoded payment; throws unless [kind] is PAYMENT. */
    fun payment(): KagemushaAttestedPaymentV1 =
        value as? KagemushaAttestedPaymentV1 ?: throw IllegalStateException("peer message is not a payment")

    /** The decoded acknowledgement; throws unless [kind] is ACKNOWLEDGEMENT. */
    fun acknowledgement(): KagemushaAttestedAcknowledgementV1 =
        value as? KagemushaAttestedAcknowledgementV1 ?: throw IllegalStateException("peer message is not an acknowledgement")

    /** `kga1:` plus unpadded base64url of the canonical value. */
    fun text(): String = TEXT_PREFIX + Base64.getUrlEncoder().withoutPadding().encodeToString(bytes)

    /** Complete IPM1 profile-2 message bytes for NFC and nearby transports. */
    fun ipm1(): ByteArray = IrohaPeerWireMessageV1(canonicalPayload(), IrohaPeerWireCompressionPolicyV1.DISABLED).encode()

    /** IQR1 frame texts: one static frame when it fits, otherwise header, data and parity frames. */
    fun qrFrames(): List<String> = IrohaPeerQRCodecV1.encode(canonicalPayload())

    private fun canonicalPayload(): IrohaPeerCanonicalPayload =
        IrohaPeerCanonicalPayload(attestedProfile(), kind.ipm1Kind, SCHEMA_VERSION, bytes)

    override fun equals(other: Any?): Boolean = other is KagemushaPeerMessage && other.kind == kind &&
        other.bytes.contentEquals(bytes)

    override fun hashCode(): Int = bytes.contentHashCode()

    override fun toString(): String = "KagemushaPeerMessage($kind, ${bytes.size} bytes)"

    companion object {
        /** Text discriminator, distinct from the V1 `kgm1:` envelope. */
        const val TEXT_PREFIX: String = "kga1:"

        /** IPM1 application profile code `KAGEMUSHA_ATTESTED_V1`. */
        const val PROFILE_CODE: Int = 2

        /** IPM1 schema version of profile 2. */
        const val SCHEMA_VERSION: Int = 1

        /** Largest canonical peer message accepted from any transport. */
        const val MAXIMUM_CANONICAL_BYTES: Int = 4_096

        /** Decode exact canonical bytes, classifying the message by its Norito schema. */
        @JvmStatic
        fun fromCanonical(bytes: ByteArray): KagemushaPeerMessage {
            require(bytes.size <= MAXIMUM_CANONICAL_BYTES) { "KAGEMUSHA peer message is too large" }
            val schemaHash = KagemushaAttestedNorito.schemaHashOf(bytes)
                ?: throw IllegalArgumentException("KAGEMUSHA peer message is not a Norito frame")
            val kind = KagemushaPeerMessageKind.entries.firstOrNull {
                org.hyperledger.iroha.sdk.norito.SchemaHash.hash16(it.schema).contentEquals(schemaHash)
            } ?: throw IllegalArgumentException("KAGEMUSHA peer message has an unknown schema")
            val value: Any = when (kind) {
                KagemushaPeerMessageKind.REQUEST -> KagemushaAttestedPaymentRequestV1.decode(bytes)
                KagemushaPeerMessageKind.PAYMENT -> KagemushaAttestedPaymentV1.decode(bytes)
                KagemushaPeerMessageKind.ACKNOWLEDGEMENT -> KagemushaAttestedAcknowledgementV1.decode(bytes)
            }
            return KagemushaPeerMessage(kind, bytes, value)
        }

        /** Decode one strict `kga1:` text value. */
        @JvmStatic
        fun fromText(text: String): KagemushaPeerMessage {
            require(text.length <= TEXT_PREFIX.length + (MAXIMUM_CANONICAL_BYTES * 4 + 2) / 3) {
                "KAGEMUSHA peer text is too long"
            }
            require(text.startsWith(TEXT_PREFIX)) { "KAGEMUSHA peer text must start with $TEXT_PREFIX" }
            val body = text.substring(TEXT_PREFIX.length)
            require(body.isNotEmpty() && body.length % 4 != 1 && body.all(::isBase64Url)) {
                "KAGEMUSHA peer text is not unpadded base64url"
            }
            val raw = try {
                Base64.getUrlDecoder().decode(body)
            } catch (failure: IllegalArgumentException) {
                throw IllegalArgumentException("KAGEMUSHA peer text is not base64url", failure)
            }
            require(Base64.getUrlEncoder().withoutPadding().encodeToString(raw) == body) {
                "KAGEMUSHA peer text is not canonical base64url"
            }
            return fromCanonical(raw)
        }

        /** Decode complete IPM1 profile-2 message bytes. */
        @JvmStatic
        fun fromIpm1(bytes: ByteArray): KagemushaPeerMessage {
            val message = IrohaPeerWireMessageV1.decode(bytes, attestedProfile())
            return fromWire(message)
        }

        /** Decode a complete list of scanned IQR1 frames (any order, duplicates allowed). */
        @JvmStatic
        fun fromQrFrames(frames: List<String>): KagemushaPeerMessage {
            val scanner = KagemushaPeerQrScanner()
            frames.forEach { frame -> scanner.ingest(frame)?.let { return it } }
            throw IllegalArgumentException("KAGEMUSHA QR frames are incomplete")
        }

        internal fun fromWire(message: IrohaPeerWireMessageV1): KagemushaPeerMessage {
            val payload = message.canonicalPayload
            require(payload.profile.code == PROFILE_CODE && payload.schemaVersion == SCHEMA_VERSION) {
                "IPM1 message is not a KAGEMUSHA attested-app message"
            }
            val decoded = fromCanonical(payload.bytes)
            require(decoded.kind.ipm1Kind == payload.kind) { "IPM1 kind does not match the message schema" }
            return decoded
        }

        internal fun of(value: KagemushaAttestedPaymentRequestV1) =
            KagemushaPeerMessage(KagemushaPeerMessageKind.REQUEST, value.encode(), value)

        internal fun of(value: KagemushaAttestedPaymentV1) =
            KagemushaPeerMessage(KagemushaPeerMessageKind.PAYMENT, value.encode(), value)

        internal fun of(value: KagemushaAttestedAcknowledgementV1) =
            KagemushaPeerMessage(KagemushaPeerMessageKind.ACKNOWLEDGEMENT, value.encode(), value)

        /**
         * The IPM1 profile `KAGEMUSHA_ATTESTED_V1`. It is resolved from the shared IPM1 registry so
         * the attested suite never forks the IPM1 or IQR1 framing implementations.
         */
        internal fun attestedProfile(): IrohaPeerPayloadProfile =
            IrohaPeerPayloadProfile.fromCode(PROFILE_CODE) ?: throw UnsupportedOperationException(
                "IPM1 profile KAGEMUSHA_ATTESTED_V1 ($PROFILE_CODE) is not registered in this SDK build",
            )

        /** True when this SDK build registers the IPM1 profile used by QR, NFC and nearby transports. */
        @JvmStatic
        fun isIpm1ProfileAvailable(): Boolean = IrohaPeerPayloadProfile.fromCode(PROFILE_CODE) != null

        private fun isBase64Url(character: Char): Boolean =
            character in 'A'..'Z' || character in 'a'..'z' || character in '0'..'9' || character == '-' || character == '_'
    }
}

/**
 * Incremental IQR1 scanner for animated attested-app QR codes. Feed every decoded QR text; it
 * returns the message once enough data and parity frames have arrived.
 */
class KagemushaPeerQrScanner {
    private val session = IrohaPeerQRScanSessionV1(
        KagemushaPeerMessage.attestedProfile(),
        null,
        KagemushaPeerMessage.SCHEMA_VERSION,
    )

    /** Data frames received so far over the total, in `0.0..1.0`. */
    @Volatile
    var progress: Double = 0.0
        private set

    /** Ingest one frame text; returns the complete message or null while incomplete. */
    @Synchronized
    fun ingest(frame: String): KagemushaPeerMessage? {
        val result = session.ingest(frame)
        progress = result.progress
        val message = result.message ?: return null
        return try {
            KagemushaPeerMessage.fromWire(message)
        } catch (failure: RuntimeException) {
            session.quarantine(message.streamId)
            throw failure
        }
    }

    /** Forget every partial stream. */
    @Synchronized
    fun reset() {
        session.reset()
        progress = 0.0
    }
}
