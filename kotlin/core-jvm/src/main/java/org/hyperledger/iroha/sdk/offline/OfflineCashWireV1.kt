package org.hyperledger.iroha.sdk.offline

import java.util.Base64

/** Canonical Offline Cash V1 value whose opaque Norito bytes are transported. */
enum class OfflineCashWirePayloadKindV1(
    /** Maximum canonical Norito bytes for this value. */
    val maximumRawBytes: Int,
) {
    PAYMENT_REQUEST(OfflineCashWireV1.MAXIMUM_PAYMENT_REQUEST_BYTES),
    ACCEPTANCE_INTENT(OfflineCashWireV1.MAXIMUM_ACCEPTANCE_INTENT_BYTES),
    ACCEPTANCE_INTENT_AUTHORIZATION(OfflineCashWireV1.MAXIMUM_ACCEPTANCE_INTENT_AUTHORIZATION_BYTES),
    ACCEPTANCE_TICKET(OfflineCashWireV1.MAXIMUM_ACCEPTANCE_TICKET_BYTES),
    PAYMENT(OfflineCashWireV1.MAXIMUM_PAYMENT_BYTES),
    ACKNOWLEDGEMENT(OfflineCashWireV1.MAXIMUM_ACKNOWLEDGEMENT_BYTES),
    MINT_AUTHORIZATION(OfflineCashWireV1.MAXIMUM_MINT_AUTHORIZATION_BYTES),
    MINT_CREDIT(OfflineCashWireV1.MAXIMUM_MINT_CREDIT_BYTES),
    REDEMPTION_VOUCHER(OfflineCashWireV1.MAXIMUM_REDEMPTION_VOUCHER_BYTES),
    ;

    /** Maximum complete `oc1:` text bytes for this value. */
    val maximumTextBytes: Int
        get() = when (this) {
            PAYMENT_REQUEST -> OfflineCashWireV1.MAXIMUM_PAYMENT_REQUEST_TEXT_BYTES
            ACCEPTANCE_INTENT -> OfflineCashWireV1.MAXIMUM_ACCEPTANCE_INTENT_TEXT_BYTES
            ACCEPTANCE_INTENT_AUTHORIZATION ->
                OfflineCashWireV1.MAXIMUM_ACCEPTANCE_INTENT_AUTHORIZATION_TEXT_BYTES
            ACCEPTANCE_TICKET -> OfflineCashWireV1.MAXIMUM_ACCEPTANCE_TICKET_TEXT_BYTES
            PAYMENT -> OfflineCashWireV1.MAXIMUM_PAYMENT_TEXT_BYTES
            ACKNOWLEDGEMENT -> OfflineCashWireV1.MAXIMUM_ACKNOWLEDGEMENT_TEXT_BYTES
            MINT_AUTHORIZATION -> OfflineCashWireV1.MAXIMUM_MINT_AUTHORIZATION_TEXT_BYTES
            MINT_CREDIT -> OfflineCashWireV1.MAXIMUM_MINT_CREDIT_TEXT_BYTES
            REDEMPTION_VOUCHER -> OfflineCashWireV1.MAXIMUM_REDEMPTION_VOUCHER_TEXT_BYTES
        }
}

/**
 * Exact size contract and opaque text envelope for Offline Cash V1.
 *
 * This codec does not interpret or validate Norito. Callers must pass bytes produced by the
 * canonical typed encoder and must run the typed decoder and cryptographic verifier after text
 * decoding. Successful text decoding grants no monetary authority.
 */
object OfflineCashWireV1 {
    const val WIRE_VERSION: Int = 1
    const val DEVICE_LIFECYCLE_VERSION: Int = 1
    const val HANDOFF_CAPABILITY: String = "cash_handoff_v1"
    const val TEXT_PREFIX: String = "oc1:"
    const val MAXIMUM_ASSET_SCALE: Int = 28
    const val REQUEST_MAX_TTL_MS: Long = 5L * 60L * 1_000L

    const val MAXIMUM_AGGREGATE_STATE_BYTES: Int = 768
    const val MAXIMUM_PAYMENT_REQUEST_BYTES: Int = 1_024
    const val MAXIMUM_ACCEPTANCE_INTENT_BYTES: Int = 256
    const val MAXIMUM_ACCEPTANCE_INTENT_AUTHORIZATION_BYTES: Int = 7_936
    const val MAXIMUM_NO_COMMIT_CLOSURE_BYTES: Int = 16_384
    const val MAXIMUM_ACCEPTANCE_TICKET_BYTES: Int = 1_024
    const val MAXIMUM_PAYMENT_BYTES: Int = 7_936
    const val MAXIMUM_ACKNOWLEDGEMENT_BYTES: Int = 512
    const val MAXIMUM_MINT_AUTHORIZATION_BYTES: Int = 7_936
    const val MAXIMUM_MINT_CREDIT_BYTES: Int = 7_936
    const val MAXIMUM_REDEMPTION_VOUCHER_BYTES: Int = 7_936
    const val MAXIMUM_PAYMENT_REQUEST_TEXT_BYTES: Int = 1_370
    const val MAXIMUM_ACCEPTANCE_INTENT_TEXT_BYTES: Int = 346
    const val MAXIMUM_ACCEPTANCE_INTENT_AUTHORIZATION_TEXT_BYTES: Int = 10_586
    const val MAXIMUM_ACCEPTANCE_TICKET_TEXT_BYTES: Int = 1_370
    const val MAXIMUM_PAYMENT_TEXT_BYTES: Int = 10_586
    const val MAXIMUM_ACKNOWLEDGEMENT_TEXT_BYTES: Int = 687
    const val MAXIMUM_MINT_AUTHORIZATION_TEXT_BYTES: Int = 10_586
    const val MAXIMUM_MINT_CREDIT_TEXT_BYTES: Int = 10_586
    const val MAXIMUM_REDEMPTION_VOUCHER_TEXT_BYTES: Int = 10_586
    const val MAXIMUM_SESSION_RAW_BYTES: Int = 9_211
    const val MAXIMUM_SESSION_TEXT_BYTES: Int = 12_288
    const val MAXIMUM_PRE_TICKET_EXCHANGE_RAW_BYTES: Int = 9_984
    const val MAXIMUM_PRE_TICKET_EXCHANGE_TEXT_BYTES: Int = 13_326
    const val MAXIMUM_COMPLETE_EXCHANGE_RAW_BYTES: Int = 18_171
    const val MAXIMUM_COMPLETE_EXCHANGE_TEXT_BYTES: Int = 24_244

    const val MAXIMUM_PAIRED_PROOF_BYTES: Int = 6_528
    const val MAXIMUM_CURRENT_PROOFS_BYTES: Int = 4_990
    const val MAXIMUM_PARITY_PROOF_BYTES: Int = 2_495
    const val HISTORY_ACCUMULATOR_BYTES: Int = 544
    const val MAXIMUM_ENCRYPTED_CREDIT_BYTES: Int = 384
    const val MAXIMUM_CREDIT_OPENING_BYTES: Int = 256
    const val CREDIT_OPENING_CANONICAL_BYTES: Int = 200
    const val X25519_PUBLIC_KEY_BYTES: Int = 32
    const val XCHACHA20_POLY1305_NONCE_BYTES: Int = 24
    const val XCHACHA20_POLY1305_TAG_BYTES: Int = 16
    const val ENCRYPTED_CREDIT_CIPHERTEXT_AND_TAG_BYTES: Int =
        CREDIT_OPENING_CANONICAL_BYTES + XCHACHA20_POLY1305_TAG_BYTES
    const val ACCEPTANCE_TICKET_MIN_RESERVED_INBOX_BYTES: Int = 8_960
    const val PAYMENT_OUTBOX_MIN_BYTES: Int = 26_112
    const val REDEMPTION_OUTBOX_MIN_BYTES: Int = 26_112

    /** Encode bounded canonical bytes as exact unpadded base64url with the `oc1:` discriminator. */
    @JvmStatic
    fun encodeText(
        kind: OfflineCashWirePayloadKindV1,
        canonicalPayload: ByteArray,
    ): String {
        require(canonicalPayload.isNotEmpty()) { "Offline Cash V1 payload is empty" }
        require(canonicalPayload.size <= kind.maximumRawBytes) {
            "Offline Cash V1 payload exceeds ${kind.maximumRawBytes} bytes"
        }
        val body = Base64.getUrlEncoder().withoutPadding().encodeToString(canonicalPayload)
        val text = TEXT_PREFIX + body
        require(text.length <= kind.maximumTextBytes) {
            "Offline Cash V1 text exceeds ${kind.maximumTextBytes} bytes"
        }
        return text
    }

    /** Decode one strict `oc1:` envelope into opaque canonical bytes. */
    @JvmStatic
    fun decodeText(
        kind: OfflineCashWirePayloadKindV1,
        text: String,
    ): ByteArray {
        require(text.length <= kind.maximumTextBytes) {
            "Offline Cash V1 text exceeds ${kind.maximumTextBytes} bytes"
        }
        require(text.startsWith(TEXT_PREFIX)) { "Offline Cash V1 text prefix is invalid" }
        val body = text.substring(TEXT_PREFIX.length)
        require(body.isNotEmpty()) { "Offline Cash V1 payload is empty" }
        require(body.all(::isBase64UrlCharacter)) { "Offline Cash V1 text is invalid" }
        require(body.length % 4 != 1) { "Offline Cash V1 base64url is non-canonical" }
        val raw = try {
            Base64.getUrlDecoder().decode(body)
        } catch (error: IllegalArgumentException) {
            throw IllegalArgumentException("Offline Cash V1 base64url is invalid", error)
        }
        require(raw.size <= kind.maximumRawBytes) {
            "Offline Cash V1 payload exceeds ${kind.maximumRawBytes} bytes"
        }
        require(encodeText(kind, raw) == text) { "Offline Cash V1 base64url is non-canonical" }
        return raw
    }

    private fun isBase64UrlCharacter(character: Char): Boolean =
        character in 'A'..'Z' ||
            character in 'a'..'z' ||
            character in '0'..'9' ||
            character == '-' ||
            character == '_'
}
