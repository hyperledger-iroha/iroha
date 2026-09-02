// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/**
 * Exact canonical codec and structural validation boundary for Offline Cash V1.
 *
 * Every method is deliberately named `Shape`: canonical decoding, digest binding, and field
 * consistency grant no monetary authority. Production signature, recursive-proof, release,
 * credential, X25519, HKDF, and AEAD operations must execute in the shared native core behind a
 * qualified non-forking hardware service.
 */
object OfflineCashNoritoV1 {
    private const val MODEL = "iroha_data_model::offline::offline_cash_v1::"
    private const val ACCEPTANCE_INTENT_TRANSCRIPT_BYTES = 114
    private const val INTENT_AUTHORIZATION_STATEMENT_TRANSCRIPT_BYTES = 244
    private const val NO_COMMIT_CLOSURE_STATEMENT_TRANSCRIPT_BYTES = 498
    private const val OUTBOX_RESERVATION_TRANSCRIPT_BYTES = 56
    private const val COMMIT_EVIDENCE_TRANSCRIPT_BYTES = 36
    private const val COMMIT_CERTIFICATE_ID_TRANSCRIPT_BYTES = 238
    private const val COMMIT_CERTIFICATE_TRANSCRIPT_BYTES = 270
    private const val AGGREGATE_SCHEMA = MODEL + "OfflineCashAggregateStateCommitmentV1"
    private const val PROOF_SCHEMA = MODEL + "OfflineCashPairedProofV1"
    private const val HARDWARE_PROFILE_SCHEMA = MODEL + "OfflineCashHardwareProfileV1"
    private const val HARDWARE_CREDENTIAL_SCHEMA = MODEL + "OfflineCashHardwareCredentialV1"
    private const val REQUEST_MODE_SCHEMA = MODEL + "OfflineCashPaymentRequestModeV1"
    private const val REQUEST_SCHEMA = MODEL + "OfflineCashPaymentRequestV1"
    private const val INTENT_SCHEMA = MODEL + "OfflineCashAcceptanceIntentV1"
    private const val INTENT_AUTH_SCHEMA = MODEL + "OfflineCashAcceptanceIntentAuthorizationV1"
    private const val NO_COMMIT_CLOSURE_STATEMENT_SCHEMA =
        MODEL + "OfflineCashNoCommitClosureStatementV1"
    private const val NO_COMMIT_CLOSURE_SCHEMA = MODEL + "OfflineCashNoCommitClosureV1"
    private const val TICKET_SCHEMA = MODEL + "OfflineCashAcceptanceTicketV1"
    private const val PEER_CREDIT_CONTEXT_SCHEMA = MODEL + "OfflineCashPeerCreditContextV1"
    private const val CREDIT_OPENING_SCHEMA = MODEL + "OfflineCashCreditOpeningV1"
    private const val CREDIT_AAD_SCHEMA = MODEL + "OfflineCashEncryptedCreditAadV1"
    private const val CREDIT_ENVELOPE_SCHEMA = MODEL + "OfflineCashEncryptedCreditEnvelopeV1"
    private const val LIFECYCLE_SCHEMA = MODEL + "OfflineCashLifecycleBindingV1"
    private const val COMMIT_CERTIFICATE_SCHEMA = MODEL + "OfflineCashCommitCertificateV1"
    private const val COMMIT_WRAPPER_SCHEMA = MODEL + "OfflineCashCommitWrapperProofV1"
    private const val STATEMENT_SCHEMA = MODEL + "OfflineCashTransferStatementV1"
    private const val PAYMENT_SCHEMA = MODEL + "OfflineCashPaymentV1"
    private const val ACK_SCHEMA = MODEL + "OfflineCashAcknowledgementV1"
    private const val MINT_AUTH_SCHEMA = MODEL + "OfflineCashMintAuthorizationV1"
    private const val MINT_STATEMENT_SCHEMA = MODEL + "OfflineCashMintCreditStatementV1"
    private const val MINT_SCHEMA = MODEL + "OfflineCashMintCreditV1"
    private const val REDEMPTION_STATEMENT_SCHEMA = MODEL + "OfflineCashRedemptionStatementV1"
    private const val REDEMPTION_SCHEMA = MODEL + "OfflineCashRedemptionVoucherV1"

    private val DEVICE_KEY_REFERENCE_DOMAIN = ascii("iroha:offline-cash:v1:device-key-reference")
    private val PASTA_STATE_COMMITMENT_DOMAIN = ascii("iroha:offline-cash:v1:pasta-state-commitment")
    private val LIABILITY_POOL_DOMAIN = ascii("iroha:offline-cash:v1:liability-pool")
    private val REQUEST_DIGEST_DOMAIN = ascii("iroha:offline-cash:v1:payment-request")
    private val INTENT_DIGEST_DOMAIN = ascii("iroha:offline-cash:v1:acceptance-intent")
    private val INTENT_AUTH_STATEMENT_DIGEST_DOMAIN =
        ascii("iroha:offline-cash:v1:acceptance-intent-authorization-statement")
    private val INTENT_AUTH_DIGEST_DOMAIN = ascii("iroha:offline-cash:v1:acceptance-intent-authorization")
    private val NO_COMMIT_CLOSURE_STATEMENT_DIGEST_DOMAIN =
        ascii("iroha:offline-cash:v1:no-commit-closure-statement")
    private val NO_COMMIT_CLOSURE_DIGEST_DOMAIN = ascii("iroha:offline-cash:v1:no-commit-closure")
    private val TICKET_DIGEST_DOMAIN = ascii("iroha:offline-cash:v1:acceptance-ticket")
    private val LIFECYCLE_DIGEST_DOMAIN = ascii("iroha:offline-cash:v1:lifecycle-binding")
    private val CIPHERTEXT_DIGEST_DOMAIN = ascii("iroha:offline-cash:v1:ciphertext")
    private val PEER_CREDIT_CONTEXT_DIGEST_DOMAIN = ascii("iroha:offline-cash:v1:peer-credit-context")
    private val PEER_CREDIT_LIFECYCLE_CONTEXT_DIGEST_DOMAIN =
        ascii("iroha:offline-cash:v1:peer-credit-lifecycle-context")
    private val CREDIT_ID_DOMAIN = ascii("iroha:offline-cash:v1:credit-id")
    private val STATEMENT_DIGEST_DOMAIN = ascii("iroha:offline-cash:v1:send-split-statement")
    private val OUTBOX_RESERVATION_COMMITMENT_DOMAIN = ascii("iroha:offline-cash:v1:outbox-reservation")
    private val COMMIT_CERTIFICATE_ID_DOMAIN = ascii("iroha:offline-cash:v1:commit-certificate-id")
    private val COMMIT_CERTIFICATE_DIGEST_DOMAIN = ascii("iroha:offline-cash:v1:commit-certificate")
    private val PAYMENT_DIGEST_DOMAIN = ascii("iroha:offline-cash:v1:payment")
    private val MINT_AUTH_CONTEXT_DIGEST_DOMAIN = ascii("iroha:offline-cash:v1:mint-authorization-context")
    private val MINT_AUTH_STATEMENT_DIGEST_DOMAIN = ascii("iroha:offline-cash:v1:mint-authorization-statement")
    private val MINT_AUTH_DIGEST_DOMAIN = ascii("iroha:offline-cash:v1:mint-authorization")
    private val MINT_CREDIT_ID_DOMAIN = ascii("iroha:offline-cash:v1:mint-credit-id")
    private val MINT_LIFECYCLE_CONTEXT_DOMAIN = ascii("iroha:offline-cash:v1:mint-lifecycle-context")
    private val MINT_STATEMENT_DIGEST_DOMAIN = ascii("iroha:offline-cash:v1:mint-statement")
    private val REDEMPTION_ID_DOMAIN = ascii("iroha:offline-cash:v1:redemption-id")
    private val REDEMPTION_STATEMENT_DIGEST_DOMAIN = ascii("iroha:offline-cash:v1:redemption-statement")
    private val ENCRYPTED_CREDIT_SALT_LABEL = ascii("iroha:offline-cash:v1:credit-envelope-salt\u0000")
    private val ENCRYPTED_CREDIT_INFO_LABEL = ascii("iroha:offline-cash:v1:credit-envelope-key\u0000")

    /** Encode exact bounded aggregate-state metadata after shape checks. */
    @JvmStatic
    fun encodeAggregateStateShape(value: OfflineCashAggregateStateCommitmentV1): ByteArray {
        validateAggregateStateShape(value)
        return bounded(raw(value, AGGREGATE_SCHEMA, AGGREGATE_ADAPTER), OfflineCashWireV1.MAXIMUM_AGGREGATE_STATE_BYTES)
    }

    /** Decode exact canonical bounded aggregate-state metadata after shape checks. */
    @JvmStatic
    fun decodeAggregateStateShapeExact(bytes: ByteArray): OfflineCashAggregateStateCommitmentV1 =
        decodeExact(
            bytes,
            OfflineCashWireV1.MAXIMUM_AGGREGATE_STATE_BYTES,
            AGGREGATE_SCHEMA,
            AGGREGATE_ADAPTER,
            ::encodeAggregateStateShape,
        )

    /** Encode a hardware profile for authenticated release transport. */
    @JvmStatic
    fun encodeHardwareProfileShape(value: OfflineCashHardwareProfileV1): ByteArray =
        bounded(raw(value, HARDWARE_PROFILE_SCHEMA, HARDWARE_PROFILE_ADAPTER), 512)

    /** Decode an exact canonical hardware profile without authenticating its governance state. */
    @JvmStatic
    fun decodeHardwareProfileShapeExact(bytes: ByteArray): OfflineCashHardwareProfileV1 =
        decodeExact(bytes, 512, HARDWARE_PROFILE_SCHEMA, HARDWARE_PROFILE_ADAPTER, ::encodeHardwareProfileShape)

    /** Encode a compact hardware credential without granting it authority. */
    @JvmStatic
    fun encodeHardwareCredentialShape(value: OfflineCashHardwareCredentialV1): ByteArray =
        bounded(raw(value, HARDWARE_CREDENTIAL_SCHEMA, HARDWARE_CREDENTIAL_ADAPTER), 768)

    /** Decode a compact hardware credential without authenticating its governance signature. */
    @JvmStatic
    fun decodeHardwareCredentialShapeExact(bytes: ByteArray): OfflineCashHardwareCredentialV1 =
        decodeExact(bytes, 768, HARDWARE_CREDENTIAL_SCHEMA, HARDWARE_CREDENTIAL_ADAPTER, ::encodeHardwareCredentialShape)

    /** Encode one canonical reusable request policy for the native hardware boundary. */
    @JvmStatic
    fun encodePaymentRequestModeShape(value: OfflineCashPaymentRequestModeV1): ByteArray =
        raw(value, REQUEST_MODE_SCHEMA, REQUEST_MODE_ADAPTER)

    /** Decode one exact reusable request policy. */
    @JvmStatic
    fun decodePaymentRequestModeShapeExact(bytes: ByteArray): OfflineCashPaymentRequestModeV1 =
        decodeExact(bytes, 256, REQUEST_MODE_SCHEMA, REQUEST_MODE_ADAPTER, ::encodePaymentRequestModeShape)

    /** Encode a signed request after shape and self-consistency checks only. */
    @JvmStatic
    fun encodePaymentRequestShape(value: OfflineCashPaymentRequestV1): ByteArray {
        validatePaymentRequestShape(value)
        return bounded(raw(value, REQUEST_SCHEMA, REQUEST_ADAPTER), OfflineCashWireV1.MAXIMUM_PAYMENT_REQUEST_BYTES)
    }

    /** Decode one exact canonical request without verifying its signature or credential. */
    @JvmStatic
    fun decodePaymentRequestShapeExact(bytes: ByteArray): OfflineCashPaymentRequestV1 =
        decodeExact(
            bytes,
            OfflineCashWireV1.MAXIMUM_PAYMENT_REQUEST_BYTES,
            REQUEST_SCHEMA,
            REQUEST_ADAPTER,
            ::encodePaymentRequestShape,
        )

    /** Encode a request as the sole `oc1:` text transport. */
    @JvmStatic
    fun encodePaymentRequestTextShape(value: OfflineCashPaymentRequestV1): String =
        OfflineCashWireV1.encodeText(OfflineCashWirePayloadKindV1.PAYMENT_REQUEST, encodePaymentRequestShape(value))

    /** Decode one exact `oc1:` request without granting it authority. */
    @JvmStatic
    fun decodePaymentRequestTextShapeExact(text: String): OfflineCashPaymentRequestV1 =
        decodePaymentRequestShapeExact(OfflineCashWireV1.decodeText(OfflineCashWirePayloadKindV1.PAYMENT_REQUEST, text))

    /** Encode a compact intent against the exact signed request. */
    @JvmStatic
    fun encodeAcceptanceIntentShape(
        value: OfflineCashAcceptanceIntentV1,
        request: OfflineCashPaymentRequestV1,
    ): ByteArray {
        validateAcceptanceIntentShape(value, request)
        return bounded(raw(value, INTENT_SCHEMA, INTENT_ADAPTER), OfflineCashWireV1.MAXIMUM_ACCEPTANCE_INTENT_BYTES)
    }

    /** Decode one exact compact intent against its request. */
    @JvmStatic
    fun decodeAcceptanceIntentShapeExact(
        bytes: ByteArray,
        request: OfflineCashPaymentRequestV1,
    ): OfflineCashAcceptanceIntentV1 = decodeExact(
        bytes,
        OfflineCashWireV1.MAXIMUM_ACCEPTANCE_INTENT_BYTES,
        INTENT_SCHEMA,
        INTENT_ADAPTER,
    ) { encodeAcceptanceIntentShape(it, request) }

    /** Encode one compact intent as strict `oc1:` text. */
    @JvmStatic
    fun encodeAcceptanceIntentTextShape(
        value: OfflineCashAcceptanceIntentV1,
        request: OfflineCashPaymentRequestV1,
    ): String = OfflineCashWireV1.encodeText(
        OfflineCashWirePayloadKindV1.ACCEPTANCE_INTENT,
        encodeAcceptanceIntentShape(value, request),
    )

    /** Decode one exact compact-intent text envelope against its request. */
    @JvmStatic
    fun decodeAcceptanceIntentTextShapeExact(
        text: String,
        request: OfflineCashPaymentRequestV1,
    ): OfflineCashAcceptanceIntentV1 = decodeAcceptanceIntentShapeExact(
        OfflineCashWireV1.decodeText(OfflineCashWirePayloadKindV1.ACCEPTANCE_INTENT, text),
        request,
    )

    /** Encode a proof-bearing sender authorization before any ticket is issued. */
    @JvmStatic
    fun encodeAcceptanceIntentAuthorizationShape(
        value: OfflineCashAcceptanceIntentAuthorizationV1,
        request: OfflineCashPaymentRequestV1,
    ): ByteArray {
        validateAcceptanceIntentAuthorizationShape(value, request)
        return bounded(
            raw(value, INTENT_AUTH_SCHEMA, INTENT_AUTH_ADAPTER),
            OfflineCashWireV1.MAXIMUM_ACCEPTANCE_INTENT_AUTHORIZATION_BYTES,
        )
    }

    /** Decode one exact proof-bearing sender authorization without verifying the proof. */
    @JvmStatic
    fun decodeAcceptanceIntentAuthorizationShapeExact(
        bytes: ByteArray,
        request: OfflineCashPaymentRequestV1,
    ): OfflineCashAcceptanceIntentAuthorizationV1 = decodeExact(
        bytes,
        OfflineCashWireV1.MAXIMUM_ACCEPTANCE_INTENT_AUTHORIZATION_BYTES,
        INTENT_AUTH_SCHEMA,
        INTENT_AUTH_ADAPTER,
    ) { encodeAcceptanceIntentAuthorizationShape(it, request) }

    /** Encode a sender authorization in strict `oc1:` form. */
    @JvmStatic
    fun encodeAcceptanceIntentAuthorizationTextShape(
        value: OfflineCashAcceptanceIntentAuthorizationV1,
        request: OfflineCashPaymentRequestV1,
    ): String = OfflineCashWireV1.encodeText(
        OfflineCashWirePayloadKindV1.ACCEPTANCE_INTENT_AUTHORIZATION,
        encodeAcceptanceIntentAuthorizationShape(value, request),
    )

    /** Decode an exact strict `oc1:` sender authorization without granting authority. */
    @JvmStatic
    fun decodeAcceptanceIntentAuthorizationTextShapeExact(
        text: String,
        request: OfflineCashPaymentRequestV1,
    ): OfflineCashAcceptanceIntentAuthorizationV1 = decodeAcceptanceIntentAuthorizationShapeExact(
        OfflineCashWireV1.decodeText(OfflineCashWirePayloadKindV1.ACCEPTANCE_INTENT_AUTHORIZATION, text),
        request,
    )

    /** Encode a ticket against the request and proof-bearing sender authorization that caused it. */
    @JvmStatic
    fun encodeAcceptanceTicketShape(
        value: OfflineCashAcceptanceTicketV1,
        request: OfflineCashPaymentRequestV1,
        authorization: OfflineCashAcceptanceIntentAuthorizationV1,
    ): ByteArray {
        validateAcceptanceTicketShape(value, request, authorization)
        return bounded(raw(value, TICKET_SCHEMA, TICKET_ADAPTER), OfflineCashWireV1.MAXIMUM_ACCEPTANCE_TICKET_BYTES)
    }

    /** Decode one exact ticket without verifying its signature or prior sender proof. */
    @JvmStatic
    fun decodeAcceptanceTicketShapeExact(
        bytes: ByteArray,
        request: OfflineCashPaymentRequestV1,
        authorization: OfflineCashAcceptanceIntentAuthorizationV1,
    ): OfflineCashAcceptanceTicketV1 = decodeExact(
        bytes,
        OfflineCashWireV1.MAXIMUM_ACCEPTANCE_TICKET_BYTES,
        TICKET_SCHEMA,
        TICKET_ADAPTER,
    ) { encodeAcceptanceTicketShape(it, request, authorization) }

    /** Encode a one-use ticket as strict `oc1:` text. */
    @JvmStatic
    fun encodeAcceptanceTicketTextShape(
        value: OfflineCashAcceptanceTicketV1,
        request: OfflineCashPaymentRequestV1,
        authorization: OfflineCashAcceptanceIntentAuthorizationV1,
    ): String = OfflineCashWireV1.encodeText(
        OfflineCashWirePayloadKindV1.ACCEPTANCE_TICKET,
        encodeAcceptanceTicketShape(value, request, authorization),
    )

    /** Decode one strict `oc1:` ticket without authenticating its receiver signature. */
    @JvmStatic
    fun decodeAcceptanceTicketTextShapeExact(
        text: String,
        request: OfflineCashPaymentRequestV1,
        authorization: OfflineCashAcceptanceIntentAuthorizationV1,
    ): OfflineCashAcceptanceTicketV1 = decodeAcceptanceTicketShapeExact(
        OfflineCashWireV1.decodeText(OfflineCashWirePayloadKindV1.ACCEPTANCE_TICKET, text),
        request,
        authorization,
    )

    /** Encode a self-contained proof that one prepared authorization was cancelled. */
    @JvmStatic
    fun encodeNoCommitClosureShape(value: OfflineCashNoCommitClosureV1): ByteArray {
        validateNoCommitClosureShape(value)
        return bounded(
            raw(value, NO_COMMIT_CLOSURE_SCHEMA, NO_COMMIT_CLOSURE_ADAPTER),
            OfflineCashWireV1.MAXIMUM_NO_COMMIT_CLOSURE_BYTES,
        )
    }

    /** Decode one exact no-commit closure without granting its proof monetary authority. */
    @JvmStatic
    fun decodeNoCommitClosureShapeExact(bytes: ByteArray): OfflineCashNoCommitClosureV1 =
        decodeExact(
            bytes,
            OfflineCashWireV1.MAXIMUM_NO_COMMIT_CLOSURE_BYTES,
            NO_COMMIT_CLOSURE_SCHEMA,
            NO_COMMIT_CLOSURE_ADAPTER,
            ::encodeNoCommitClosureShape,
        )

    /** Encode the exact pre-ID peer context carried by encrypted-credit AAD. */
    @JvmStatic
    fun encodePeerCreditContextShape(value: OfflineCashPeerCreditContextV1): ByteArray =
        raw(value, PEER_CREDIT_CONTEXT_SCHEMA, PEER_CREDIT_CONTEXT_ADAPTER)

    /** Decode one exact pre-ID peer context without opening an encrypted credit. */
    @JvmStatic
    fun decodePeerCreditContextShapeExact(bytes: ByteArray): OfflineCashPeerCreditContextV1 =
        decodeExact(
            bytes,
            512,
            PEER_CREDIT_CONTEXT_SCHEMA,
            PEER_CREDIT_CONTEXT_ADAPTER,
            ::encodePeerCreditContextShape,
        )

    /** Build the acyclic peer context from the exact request, intent, ticket, and statement. */
    @JvmStatic
    fun peerCreditContextShape(
        statement: OfflineCashTransferStatementV1,
        request: OfflineCashPaymentRequestV1,
        intent: OfflineCashAcceptanceIntentV1,
        ticket: OfflineCashAcceptanceTicketV1,
    ): OfflineCashPeerCreditContextV1 {
        validatePaymentRequestShape(request)
        validateAcceptanceIntentShape(intent, request)
        validateTicketWithoutAuthorizationProof(ticket, request, intent)
        validatePeerStatementContextShape(statement, request, ticket)
        return OfflineCashPeerCreditContextV1(
            OfflineCashWireV1.WIRE_VERSION,
            paymentRequestDigest(request),
            acceptanceIntentDigest(intent, request),
            digestEncoded(TICKET_DIGEST_DOMAIN, raw(ticket, TICKET_SCHEMA, TICKET_ADAPTER)),
            peerLifecycleContextDigest(statement.lifecycle),
            statement.recipientOneTimeKey,
        )
    }

    /** Return the canonical digest placed in peer-credit associated data. */
    @JvmStatic
    fun peerCreditContextDigestShape(value: OfflineCashPeerCreditContextV1): ByteArray =
        digestEncoded(
            PEER_CREDIT_CONTEXT_DIGEST_DOMAIN,
            encodePeerCreditContextShape(value),
        )

    /** Construct the exact typed AAD for a receiver-bound peer credit. */
    @JvmStatic
    fun encryptedCreditAadForPeerShape(
        statement: OfflineCashTransferStatementV1,
        request: OfflineCashPaymentRequestV1,
        intent: OfflineCashAcceptanceIntentV1,
        ticket: OfflineCashAcceptanceTicketV1,
    ): OfflineCashEncryptedCreditAadV1 {
        val context = peerCreditContextShape(statement, request, intent, ticket)
        return OfflineCashEncryptedCreditAadV1(
            OfflineCashWireV1.WIRE_VERSION,
            OfflineCashEncryptedCreditPurposeV1.PEER,
            peerCreditContextDigestShape(context),
            statement.ciphertextCommitment(),
            statement.lifecycle.creditId(),
            statement.amount,
        )
    }

    /** Construct the exact typed AAD authorized before a reserve-backed mint debit. */
    @JvmStatic
    fun encryptedCreditAadForMintShape(
        statement: OfflineCashMintAuthorizationStatementV1,
    ): OfflineCashEncryptedCreditAadV1 {
        validateMintAuthorizationStatementShape(statement)
        return OfflineCashEncryptedCreditAadV1(
            OfflineCashWireV1.WIRE_VERSION,
            OfflineCashEncryptedCreditPurposeV1.MINT,
            mintAuthorizationContextDigest(statement.context),
            statement.issuanceCommitment(),
            statement.creditId(),
            statement.context.amount,
        )
    }

    /** Encode a payment after exact request, intent, ticket, certificate, and envelope shape checks. */
    @JvmStatic
    fun encodePaymentShape(value: OfflineCashPaymentV1, request: OfflineCashPaymentRequestV1): ByteArray {
        validatePaymentShape(value, request)
        return bounded(raw(value, PAYMENT_SCHEMA, PAYMENT_ADAPTER), OfflineCashWireV1.MAXIMUM_PAYMENT_BYTES)
    }

    /** Decode a payment and validate all non-cryptographic bindings against its exact request. */
    @JvmStatic
    fun decodePaymentShapeExact(
        bytes: ByteArray,
        request: OfflineCashPaymentRequestV1,
    ): OfflineCashPaymentV1 = decodeExact(
        bytes,
        OfflineCashWireV1.MAXIMUM_PAYMENT_BYTES,
        PAYMENT_SCHEMA,
        PAYMENT_ADAPTER,
    ) { encodePaymentShape(it, request) }

    /** Encode a payment as strict `oc1:` text after shape checks. */
    @JvmStatic
    fun encodePaymentTextShape(value: OfflineCashPaymentV1, request: OfflineCashPaymentRequestV1): String =
        OfflineCashWireV1.encodeText(OfflineCashWirePayloadKindV1.PAYMENT, encodePaymentShape(value, request))

    /** Decode strict `oc1:` payment text without granting monetary authority. */
    @JvmStatic
    fun decodePaymentTextShapeExact(text: String, request: OfflineCashPaymentRequestV1): OfflineCashPaymentV1 =
        decodePaymentShapeExact(OfflineCashWireV1.decodeText(OfflineCashWirePayloadKindV1.PAYMENT, text), request)

    /** Encode an acknowledgement after exact structural binding checks. */
    @JvmStatic
    fun encodeAcknowledgementShape(
        value: OfflineCashAcknowledgementV1,
        request: OfflineCashPaymentRequestV1,
        payment: OfflineCashPaymentV1,
    ): ByteArray {
        validateAcknowledgementShape(value, request, payment)
        return bounded(raw(value, ACK_SCHEMA, ACK_ADAPTER), OfflineCashWireV1.MAXIMUM_ACKNOWLEDGEMENT_BYTES)
    }

    /** Decode an acknowledgement without verifying its receiver signature. */
    @JvmStatic
    fun decodeAcknowledgementShapeExact(
        bytes: ByteArray,
        request: OfflineCashPaymentRequestV1,
        payment: OfflineCashPaymentV1,
    ): OfflineCashAcknowledgementV1 = decodeExact(
        bytes,
        OfflineCashWireV1.MAXIMUM_ACKNOWLEDGEMENT_BYTES,
        ACK_SCHEMA,
        ACK_ADAPTER,
    ) { encodeAcknowledgementShape(it, request, payment) }

    /** Encode one durable-inbox acknowledgement as strict `oc1:` text. */
    @JvmStatic
    fun encodeAcknowledgementTextShape(
        value: OfflineCashAcknowledgementV1,
        request: OfflineCashPaymentRequestV1,
        payment: OfflineCashPaymentV1,
    ): String = OfflineCashWireV1.encodeText(
        OfflineCashWirePayloadKindV1.ACKNOWLEDGEMENT,
        encodeAcknowledgementShape(value, request, payment),
    )

    /** Decode strict acknowledgement text without authenticating its signature. */
    @JvmStatic
    fun decodeAcknowledgementTextShapeExact(
        text: String,
        request: OfflineCashPaymentRequestV1,
        payment: OfflineCashPaymentV1,
    ): OfflineCashAcknowledgementV1 = decodeAcknowledgementShapeExact(
        OfflineCashWireV1.decodeText(OfflineCashWirePayloadKindV1.ACKNOWLEDGEMENT, text),
        request,
        payment,
    )

    /** Encode a pre-debit mint authorization after shape checks only. */
    @JvmStatic
    fun encodeMintAuthorizationShape(value: OfflineCashMintAuthorizationV1): ByteArray {
        validateMintAuthorizationShape(value)
        return bounded(raw(value, MINT_AUTH_SCHEMA, MINT_AUTH_ADAPTER), OfflineCashWireV1.MAXIMUM_MINT_AUTHORIZATION_BYTES)
    }

    /** Decode one exact pre-debit mint authorization without verifying either proof parity. */
    @JvmStatic
    fun decodeMintAuthorizationShapeExact(bytes: ByteArray): OfflineCashMintAuthorizationV1 =
        decodeExact(
            bytes,
            OfflineCashWireV1.MAXIMUM_MINT_AUTHORIZATION_BYTES,
            MINT_AUTH_SCHEMA,
            MINT_AUTH_ADAPTER,
            ::encodeMintAuthorizationShape,
        )

    /** Encode one mint authorization as strict `oc1:` text after shape checks. */
    @JvmStatic
    fun encodeMintAuthorizationTextShape(value: OfflineCashMintAuthorizationV1): String =
        OfflineCashWireV1.encodeText(
            OfflineCashWirePayloadKindV1.MINT_AUTHORIZATION,
            encodeMintAuthorizationShape(value),
        )

    /** Decode one exact mint authorization text envelope without granting authority. */
    @JvmStatic
    fun decodeMintAuthorizationTextShapeExact(text: String): OfflineCashMintAuthorizationV1 =
        decodeMintAuthorizationShapeExact(
            OfflineCashWireV1.decodeText(OfflineCashWirePayloadKindV1.MINT_AUTHORIZATION, text),
        )

    /** Encode one finalized mint credit after standalone shape checks. */
    @JvmStatic
    fun encodeMintCreditShape(value: OfflineCashMintCreditV1): ByteArray {
        validateMintCreditShape(value)
        return bounded(raw(value, MINT_SCHEMA, MINT_ADAPTER), OfflineCashWireV1.MAXIMUM_MINT_CREDIT_BYTES)
    }

    /** Decode one exact standalone mint credit without granting monetary authority. */
    @JvmStatic
    fun decodeMintCreditShapeExact(bytes: ByteArray): OfflineCashMintCreditV1 =
        decodeExact(
            bytes,
            OfflineCashWireV1.MAXIMUM_MINT_CREDIT_BYTES,
            MINT_SCHEMA,
            MINT_ADAPTER,
            ::encodeMintCreditShape,
        )

    /** Encode one finalized mint credit as strict `oc1:` text after shape checks. */
    @JvmStatic
    fun encodeMintCreditTextShape(value: OfflineCashMintCreditV1): String =
        OfflineCashWireV1.encodeText(
            OfflineCashWirePayloadKindV1.MINT_CREDIT,
            encodeMintCreditShape(value),
        )

    /** Decode one exact standalone mint credit text envelope without granting authority. */
    @JvmStatic
    fun decodeMintCreditTextShapeExact(text: String): OfflineCashMintCreditV1 =
        decodeMintCreditShapeExact(
            OfflineCashWireV1.decodeText(OfflineCashWirePayloadKindV1.MINT_CREDIT, text),
        )

    /** Encode one finalized mint credit against its exact pre-debit authorization. */
    @JvmStatic
    fun encodeMintCreditShape(
        value: OfflineCashMintCreditV1,
        authorization: OfflineCashMintAuthorizationV1,
    ): ByteArray {
        validateMintCreditShapeAgainstAuthorization(value, authorization)
        return encodeMintCreditShape(value)
    }

    /** Decode a mint credit against its exact authorization without granting authority. */
    @JvmStatic
    fun decodeMintCreditShapeExact(
        bytes: ByteArray,
        authorization: OfflineCashMintAuthorizationV1,
    ): OfflineCashMintCreditV1 = decodeExact(
        bytes,
        OfflineCashWireV1.MAXIMUM_MINT_CREDIT_BYTES,
        MINT_SCHEMA,
        MINT_ADAPTER,
    ) { encodeMintCreditShape(it, authorization) }

    /** Encode one terminal redemption voucher after wrapper/certificate shape checks. */
    @JvmStatic
    fun encodeRedemptionVoucherShape(value: OfflineCashRedemptionVoucherV1): ByteArray {
        validateRedemptionVoucherShape(value)
        return bounded(raw(value, REDEMPTION_SCHEMA, REDEMPTION_ADAPTER), OfflineCashWireV1.MAXIMUM_REDEMPTION_VOUCHER_BYTES)
    }

    /** Decode one exact terminal redemption voucher without granting authority. */
    @JvmStatic
    fun decodeRedemptionVoucherShapeExact(bytes: ByteArray): OfflineCashRedemptionVoucherV1 =
        decodeExact(
            bytes,
            OfflineCashWireV1.MAXIMUM_REDEMPTION_VOUCHER_BYTES,
            REDEMPTION_SCHEMA,
            REDEMPTION_ADAPTER,
            ::encodeRedemptionVoucherShape,
        )

    /** Encode one terminal redemption voucher as strict `oc1:` text after shape checks. */
    @JvmStatic
    fun encodeRedemptionVoucherTextShape(value: OfflineCashRedemptionVoucherV1): String =
        OfflineCashWireV1.encodeText(
            OfflineCashWirePayloadKindV1.REDEMPTION_VOUCHER,
            encodeRedemptionVoucherShape(value),
        )

    /** Decode one exact redemption text envelope without granting authority. */
    @JvmStatic
    fun decodeRedemptionVoucherTextShapeExact(text: String): OfflineCashRedemptionVoucherV1 =
        decodeRedemptionVoucherShapeExact(
            OfflineCashWireV1.decodeText(OfflineCashWirePayloadKindV1.REDEMPTION_VOUCHER, text),
        )

    /** Encode the exact recipient-only credit-opening plaintext. */
    @JvmStatic
    fun encodeCreditOpeningShape(value: OfflineCashCreditOpeningV1): ByteArray {
        val canonical = bounded(
            raw(value, CREDIT_OPENING_SCHEMA, CREDIT_OPENING_ADAPTER),
            OfflineCashWireV1.MAXIMUM_CREDIT_OPENING_BYTES,
        )
        require(canonical.size == OfflineCashWireV1.CREDIT_OPENING_CANONICAL_BYTES)
        return canonical
    }

    /** Decode an exact canonical credit opening after authenticated decryption in native core. */
    @JvmStatic
    fun decodeCreditOpeningShapeExact(bytes: ByteArray): OfflineCashCreditOpeningV1 {
        require(bytes.size == OfflineCashWireV1.CREDIT_OPENING_CANONICAL_BYTES)
        return decodeExact(
            bytes,
            OfflineCashWireV1.MAXIMUM_CREDIT_OPENING_BYTES,
            CREDIT_OPENING_SCHEMA,
            CREDIT_OPENING_ADAPTER,
            ::encodeCreditOpeningShape,
        )
    }

    /** Decode the exact opening and bind it to its public credit identity and amount. */
    @JvmStatic
    fun decodeCreditOpeningShapeExactAgainst(
        bytes: ByteArray,
        creditId: ByteArray,
        amount: BigInteger,
    ): OfflineCashCreditOpeningV1 {
        val value = decodeCreditOpeningShapeExact(bytes)
        require(value.creditId().contentEquals(fixed32(creditId, "creditId")))
        require(value.amount == amount)
        return value
    }

    /** Encode canonical encrypted-credit associated data. */
    @JvmStatic
    fun encodeEncryptedCreditAadShape(value: OfflineCashEncryptedCreditAadV1): ByteArray =
        raw(value, CREDIT_AAD_SCHEMA, CREDIT_AAD_ADAPTER)

    /** Decode canonical encrypted-credit associated data. */
    @JvmStatic
    fun decodeEncryptedCreditAadShapeExact(bytes: ByteArray): OfflineCashEncryptedCreditAadV1 =
        decodeExact(bytes, 512, CREDIT_AAD_SCHEMA, CREDIT_AAD_ADAPTER, ::encodeEncryptedCreditAadShape)

    /** Encode the exact X25519/XChaCha recipient envelope. */
    @JvmStatic
    fun encodeEncryptedCreditEnvelopeShape(value: OfflineCashEncryptedCreditEnvelopeV1): ByteArray =
        bounded(raw(value, CREDIT_ENVELOPE_SCHEMA, CREDIT_ENVELOPE_ADAPTER), OfflineCashWireV1.MAXIMUM_ENCRYPTED_CREDIT_BYTES)

    /**
     * Decode one exact envelope and enforce its canonical nonzero 32-byte X25519 wire shape.
     * Native object/exchange validation authenticates the element before monetary use.
     */
    @JvmStatic
    fun decodeEncryptedCreditEnvelopeShapeExact(bytes: ByteArray): OfflineCashEncryptedCreditEnvelopeV1 =
        decodeExact(
            bytes,
            OfflineCashWireV1.MAXIMUM_ENCRYPTED_CREDIT_BYTES,
            CREDIT_ENVELOPE_SCHEMA,
            CREDIT_ENVELOPE_ADAPTER,
            ::encodeEncryptedCreditEnvelopeShape,
        )

    /** Derive the specified HKDF-SHA256 salt; no private key operation occurs on the JVM. */
    @JvmStatic
    fun encryptedCreditKdfSalt(
        recipientPublicKey: OfflineCashX25519PublicKeyV1,
        ephemeralPublicKey: OfflineCashX25519PublicKeyV1,
    ): ByteArray = sha256(ENCRYPTED_CREDIT_SALT_LABEL, recipientPublicKey.bytes(), ephemeralPublicKey.bytes())

    /** Derive the specified HKDF-SHA256 info; AEAD key derivation remains native-only. */
    @JvmStatic
    fun encryptedCreditKdfInfo(aad: OfflineCashEncryptedCreditAadV1): ByteArray =
        ENCRYPTED_CREDIT_INFO_LABEL + sha256(encodeEncryptedCreditAadShape(aad))

    /** Hash one public Pasta pair into the aggregate state's compact outer head. */
    @JvmStatic
    fun pastaStateCommitment(value: OfflineCashPastaStateCommitmentV1): ByteArray =
        sha256(PASTA_STATE_COMMITMENT_DOMAIN, byteArrayOf(0), value.eq(), value.ep())

    /** Derive the exact normalized device-key reference. */
    @JvmStatic
    fun deviceKeyReference(publicKey: OfflineCashDevicePublicKeyV1): ByteArray =
        sha256(DEVICE_KEY_REFERENCE_DOMAIN, byteArrayOf(0), publicKey.sec1Bytes())

    /** Derive the sole pooled reserve identity for one asset incarnation. */
    @JvmStatic
    fun liabilityPoolId(
        networkId: NetworkId,
        asset: OfflineCashAssetDefinitionIdV1,
        incarnation: OfflineCashAssetIncarnationV1,
    ): ByteArray = digestEncoded(
        LIABILITY_POOL_DOMAIN,
        frame("iroha.offline-cash.v1.liability-pool-preimage") { encoder ->
            field(encoder) { it.writeBytes(networkId.bytes()) }
            field(encoder) { it.writeBytes(asset.canonicalPayload()) }
            incarnationField(encoder, incarnation)
        },
    )

    /** Return the canonical digest of a shape-valid request. */
    @JvmStatic
    fun paymentRequestDigest(value: OfflineCashPaymentRequestV1): ByteArray {
        validatePaymentRequestShape(value)
        return digestEncoded(REQUEST_DIGEST_DOMAIN, raw(value, REQUEST_SCHEMA, REQUEST_ADAPTER))
    }

    /** Return the canonical digest of one request-bound sender intent. */
    @JvmStatic
    fun acceptanceIntentDigest(
        value: OfflineCashAcceptanceIntentV1,
        request: OfflineCashPaymentRequestV1,
    ): ByteArray {
        validateAcceptanceIntentShape(value, request)
        return digestEncoded(INTENT_DIGEST_DOMAIN, acceptanceIntentCircuitTranscript(value))
    }

    /** Return the canonical digest of one request-bound receiver ticket. */
    @JvmStatic
    fun acceptanceTicketDigest(
        value: OfflineCashAcceptanceTicketV1,
        request: OfflineCashPaymentRequestV1,
        authorization: OfflineCashAcceptanceIntentAuthorizationV1,
    ): ByteArray {
        validateAcceptanceTicketShape(value, request, authorization)
        return digestEncoded(TICKET_DIGEST_DOMAIN, raw(value, TICKET_SCHEMA, TICKET_ADAPTER))
    }

    /** Return the semantic digest a pre-ticket authorization proof must carry. */
    @JvmStatic
    fun acceptanceIntentAuthorizationStatementDigestShape(
        value: OfflineCashAcceptanceIntentAuthorizationStatementV1,
        request: OfflineCashPaymentRequestV1,
    ): ByteArray {
        validateAcceptanceIntentShape(value.intent, request)
        require(value.releaseId().contentEquals(request.releaseId()))
        require(value.suiteId().contentEquals(request.hardwareCredential.suiteId()))
        return digestEncoded(
            INTENT_AUTH_STATEMENT_DIGEST_DOMAIN,
            acceptanceIntentAuthorizationStatementCircuitTranscript(value),
        )
    }

    /** Return the digest of the complete proof-bearing sender authorization envelope. */
    @JvmStatic
    fun acceptanceIntentAuthorizationDigestShape(
        value: OfflineCashAcceptanceIntentAuthorizationV1,
        request: OfflineCashPaymentRequestV1,
    ): ByteArray {
        validateAcceptanceIntentAuthorizationShape(value, request)
        return digestEncoded(INTENT_AUTH_DIGEST_DOMAIN, raw(value, INTENT_AUTH_SCHEMA, INTENT_AUTH_ADAPTER))
    }

    /** Return the semantic digest constrained by both no-commit proof parities. */
    @JvmStatic
    fun noCommitClosureStatementDigestShape(value: OfflineCashNoCommitClosureStatementV1): ByteArray =
        digestEncoded(
            NO_COMMIT_CLOSURE_STATEMENT_DIGEST_DOMAIN,
            noCommitClosureStatementCircuitTranscript(value),
        )

    /** Return the canonical identity of one complete no-commit proof envelope. */
    @JvmStatic
    fun noCommitClosureDigestShape(value: OfflineCashNoCommitClosureV1): ByteArray {
        validateNoCommitClosureShape(value)
        return digestEncoded(
            NO_COMMIT_CLOSURE_DIGEST_DOMAIN,
            raw(value, NO_COMMIT_CLOSURE_SCHEMA, NO_COMMIT_CLOSURE_ADAPTER),
        )
    }

    /** Return the canonical public lifecycle digest used by terminal proofs. */
    @JvmStatic
    fun lifecycleDigestShape(value: OfflineCashLifecycleBindingV1): ByteArray = lifecycleDigest(value)

    /** Return the hiding sender-outbox commitment constrained by the terminal wrapper. */
    @JvmStatic
    fun outboxReservationCommitmentShape(value: OfflineCashOutboxReservationV1): ByteArray {
        val minimum = when (value.operationKind) {
            OfflineCashOperationKindV1.SEND_SPLIT -> OfflineCashWireV1.PAYMENT_OUTBOX_MIN_BYTES
            OfflineCashOperationKindV1.REDEEM_SPLIT -> OfflineCashWireV1.REDEMPTION_OUTBOX_MIN_BYTES
            else -> throw IllegalArgumentException("operation does not emit a recoverable terminal envelope")
        }
        require(value.reservedOutboxBytes >= minimum)
        return digestEncoded(
            OUTBOX_RESERVATION_COMMITMENT_DOMAIN,
            outboxReservationCircuitTranscript(value),
        )
    }

    /** Return the canonical ciphertext envelope digest without decrypting it. */
    @JvmStatic
    fun ciphertextDigestShape(bytes: ByteArray): ByteArray = ciphertextDigest(bytes.copyOf())

    /** Return the unique peer-credit identity implied by an unlinkable send statement. */
    @JvmStatic
    fun expectedPeerCreditIdShape(value: OfflineCashTransferStatementV1): ByteArray =
        expectedPeerCreditId(value)

    /** Return the semantic digest a payment wrapper must carry. */
    @JvmStatic
    fun transferStatementDigestShape(value: OfflineCashTransferStatementV1): ByteArray {
        validateTransferStatementShape(value)
        return statementDigest(value)
    }

    /** Return the canonical digest of a complete request-bound payment. */
    @JvmStatic
    fun paymentDigestShape(
        value: OfflineCashPaymentV1,
        request: OfflineCashPaymentRequestV1,
    ): ByteArray = paymentDigest(value, request)

    /** Return the canonical terminal certificate identity derived from its self-free fields. */
    @JvmStatic
    fun expectedCommitCertificateIdShape(value: OfflineCashCommitCertificateV1): ByteArray =
        expectedCommitCertificateId(value)

    /** Return the canonical digest of a terminal certificate. */
    @JvmStatic
    fun commitCertificateDigestShape(value: OfflineCashCommitCertificateV1): ByteArray =
        commitCertificateDigest(value)

    /** Return the exact pre-ID mint-authorization context digest. */
    @JvmStatic
    fun mintAuthorizationContextDigestShape(value: OfflineCashMintAuthorizationContextV1): ByteArray {
        validateMintAuthorizationContextShape(value)
        return mintAuthorizationContextDigest(value)
    }

    /** Return the semantic digest a mint-authorization proof must carry. */
    @JvmStatic
    fun mintAuthorizationStatementDigestShape(value: OfflineCashMintAuthorizationStatementV1): ByteArray {
        validateMintAuthorizationStatementShape(value)
        return digestEncoded(
            MINT_AUTH_STATEMENT_DIGEST_DOMAIN,
            raw(value, MODEL + "OfflineCashMintAuthorizationStatementV1", MINT_AUTH_STATEMENT_ADAPTER),
        )
    }

    /** Return the canonical digest of a complete pre-debit mint authorization. */
    @JvmStatic
    fun mintAuthorizationDigestShape(value: OfflineCashMintAuthorizationV1): ByteArray {
        validateMintAuthorizationShape(value)
        return mintAuthorizationDigest(value)
    }

    /** Return the unique mint-credit identity implied by its public statement. */
    @JvmStatic
    fun expectedMintCreditIdShape(value: OfflineCashMintCreditStatementV1): ByteArray {
        validateLifecycleShape(value.lifecycle)
        require(value.lifecycle.operationKind == OfflineCashOperationKindV1.MINT_FOLD)
        return expectedMintCreditId(value)
    }

    /** Return the semantic digest a mint-credit proof must carry. */
    @JvmStatic
    fun mintCreditStatementDigestShape(value: OfflineCashMintCreditStatementV1): ByteArray {
        validateLifecycleShape(value.lifecycle)
        require(value.lifecycle.creditId().contentEquals(expectedMintCreditId(value)))
        return mintStatementDigest(value)
    }

    /** Return the unique online redemption identity implied by its public statement. */
    @JvmStatic
    fun expectedRedemptionIdShape(value: OfflineCashRedemptionStatementV1): ByteArray {
        validateLifecycleShape(value.lifecycle)
        require(value.lifecycle.operationKind == OfflineCashOperationKindV1.REDEEM_SPLIT)
        return expectedRedemptionId(value)
    }

    /** Return the semantic digest a redemption wrapper must carry. */
    @JvmStatic
    fun redemptionStatementDigestShape(value: OfflineCashRedemptionStatementV1): ByteArray {
        validateRedemptionStatementShape(value)
        return redemptionStatementDigest(value)
    }

    /** Validate the terminal request/payment/ack delivery trio and return raw bytes. */
    @JvmStatic
    fun validateTerminalDeliveryShape(
        request: OfflineCashPaymentRequestV1,
        payment: OfflineCashPaymentV1,
        acknowledgement: OfflineCashAcknowledgementV1,
    ): Int {
        val sizes = intArrayOf(
            encodePaymentRequestShape(request).size,
            encodePaymentShape(payment, request).size,
            encodeAcknowledgementShape(acknowledgement, request, payment).size,
        )
        val raw = sizes.sum()
        require(raw <= OfflineCashWireV1.MAXIMUM_SESSION_RAW_BYTES)
        require(sizes.sumOf(::textEnvelopeLength) <= OfflineCashWireV1.MAXIMUM_SESSION_TEXT_BYTES)
        return raw
    }

    /** Validate request/authorization/ticket precommit transport before sender commitment. */
    @JvmStatic
    fun validatePreTicketExchangeShape(
        request: OfflineCashPaymentRequestV1,
        authorization: OfflineCashAcceptanceIntentAuthorizationV1,
        ticket: OfflineCashAcceptanceTicketV1,
    ): Int {
        val sizes = intArrayOf(
            encodePaymentRequestShape(request).size,
            encodeAcceptanceIntentAuthorizationShape(authorization, request).size,
            encodeAcceptanceTicketShape(ticket, request, authorization).size,
        )
        val raw = sizes.sum()
        require(raw <= OfflineCashWireV1.MAXIMUM_PRE_TICKET_EXCHANGE_RAW_BYTES)
        require(sizes.sumOf(::textEnvelopeLength) <= OfflineCashWireV1.MAXIMUM_PRE_TICKET_EXCHANGE_TEXT_BYTES)
        return raw
    }

    /** Validate all five transported messages and their exact pre-ticket-to-payment binding. */
    @JvmStatic
    fun validateCompleteExchangeShape(
        request: OfflineCashPaymentRequestV1,
        authorization: OfflineCashAcceptanceIntentAuthorizationV1,
        ticket: OfflineCashAcceptanceTicketV1,
        payment: OfflineCashPaymentV1,
        acknowledgement: OfflineCashAcknowledgementV1,
    ): Int {
        validatePreTicketExchangeShape(request, authorization, ticket)
        validateTerminalDeliveryShape(request, payment, acknowledgement)
        require(
            raw(payment.acceptanceIntent, INTENT_SCHEMA, INTENT_ADAPTER).contentEquals(
                raw(authorization.statement.intent, INTENT_SCHEMA, INTENT_ADAPTER),
            ),
        )
        require(
            raw(payment.acceptanceTicket, TICKET_SCHEMA, TICKET_ADAPTER).contentEquals(
                raw(ticket, TICKET_SCHEMA, TICKET_ADAPTER),
            ),
        )
        val sizes = intArrayOf(
            encodePaymentRequestShape(request).size,
            encodeAcceptanceIntentAuthorizationShape(authorization, request).size,
            encodeAcceptanceTicketShape(ticket, request, authorization).size,
            encodePaymentShape(payment, request).size,
            encodeAcknowledgementShape(acknowledgement, request, payment).size,
        )
        val raw = sizes.sum()
        require(raw <= OfflineCashWireV1.MAXIMUM_COMPLETE_EXCHANGE_RAW_BYTES)
        require(sizes.sumOf(::textEnvelopeLength) <= OfflineCashWireV1.MAXIMUM_COMPLETE_EXCHANGE_TEXT_BYTES)
        return raw
    }

    private fun validateAggregateStateShape(value: OfflineCashAggregateStateCommitmentV1) {
        require(value.liabilityPoolId().contentEquals(liabilityPoolId(value.networkId, value.asset, value.assetIncarnation)))
    }

    private fun validatePaymentRequestShape(value: OfflineCashPaymentRequestV1) {
        require(value.liabilityPoolId().contentEquals(liabilityPoolId(value.networkId, value.asset, value.assetIncarnation)))
        require(value.hardwareCredential.networkId == value.networkId)
        require(value.hardwareCredential.deviceKeyReference().contentEquals(deviceKeyReference(value.hardwareCredential.devicePublicKey)))
        require(value.issuedAtMs >= value.hardwareCredential.issuedAtMs)
        require(value.expiresAtMs <= value.hardwareCredential.expiresAtMs)
    }

    private fun validateAcceptanceIntentShape(
        value: OfflineCashAcceptanceIntentV1,
        request: OfflineCashPaymentRequestV1,
    ) {
        validatePaymentRequestShape(request)
        require(value.requestDigest().contentEquals(paymentRequestDigest(request)))
        require(request.requestMode.acceptsExactAmount(value.exactAmount))
    }

    private fun validateAcceptanceIntentAuthorizationShape(
        value: OfflineCashAcceptanceIntentAuthorizationV1,
        request: OfflineCashPaymentRequestV1,
    ) {
        validateAcceptanceIntentShape(value.statement.intent, request)
        require(value.statement.releaseId().contentEquals(request.releaseId()))
        require(value.statement.suiteId().contentEquals(request.hardwareCredential.suiteId()))
        val semantic = digestEncoded(
            INTENT_AUTH_STATEMENT_DIGEST_DOMAIN,
            acceptanceIntentAuthorizationStatementCircuitTranscript(value.statement),
        )
        require(value.proof.semanticDigest().contentEquals(semantic))
    }

    private fun validateAcceptanceTicketShape(
        value: OfflineCashAcceptanceTicketV1,
        request: OfflineCashPaymentRequestV1,
        authorization: OfflineCashAcceptanceIntentAuthorizationV1,
    ) {
        validateAcceptanceIntentAuthorizationShape(authorization, request)
        require(value.networkId == request.networkId)
        require(value.requestId().contentEquals(request.requestId()))
        require(value.requestDigest().contentEquals(paymentRequestDigest(request)))
        require(value.asset == request.asset && value.assetIncarnation == request.assetIncarnation)
        require(value.scale == request.scale)
        require(rawMode(value.requestMode).contentEquals(rawMode(request.requestMode)))
        require(value.intentDigest().contentEquals(acceptanceIntentDigest(authorization.statement.intent, request)))
        require(value.exactAmount == authorization.statement.intent.exactAmount)
        require(value.hardwareProfileId().contentEquals(request.hardwareCredential.hardwareProfileId()))
        require(value.policyEpoch == request.hardwareCredential.policyEpoch)
        require(value.issuedAtMs >= request.issuedAtMs && value.expiresAtMs <= request.expiresAtMs)
    }

    private fun validateNoCommitClosureShape(value: OfflineCashNoCommitClosureV1) {
        val statement = value.statement
        val request = value.request
        val authorization = value.intentAuthorization
        val intent = authorization.statement.intent
        val ticket = value.acceptanceTicket
        require(value.version == OfflineCashWireV1.WIRE_VERSION && statement.version == value.version)
        validateAcceptanceIntentAuthorizationShape(authorization, request)
        validateAcceptanceTicketShape(ticket, request, authorization)
        require(statement.requestId().contentEquals(request.requestId()))
        require(statement.requestDigest().contentEquals(paymentRequestDigest(request)))
        require(statement.acceptanceTicketId().contentEquals(ticket.acceptanceTicketId()))
        require(statement.ticketDigest().contentEquals(acceptanceTicketDigest(ticket, request, authorization)))
        require(
            statement.intentAuthorizationDigest().contentEquals(
                acceptanceIntentAuthorizationDigestShape(authorization, request),
            ),
        )
        require(statement.intentDigest().contentEquals(acceptanceIntentDigest(intent, request)))
        require(statement.exactAmount == intent.exactAmount && statement.exactAmount == ticket.exactAmount)
        require(statement.senderOneTimeCommitment().contentEquals(intent.senderOneTimeCommitment()))
        require(statement.releaseId().contentEquals(authorization.statement.releaseId()))
        require(statement.suiteId().contentEquals(authorization.statement.suiteId()))
        require(statement.vkDigest().contentEquals(authorization.statement.vkDigest()))
        require(
            statement.artifactManifestDigest().contentEquals(
                authorization.statement.artifactManifestDigest(),
            ),
        )
        require(value.proof.semanticDigest().contentEquals(noCommitClosureStatementDigestShape(statement)))
        require(
            raw(value, NO_COMMIT_CLOSURE_SCHEMA, NO_COMMIT_CLOSURE_ADAPTER).size <=
                OfflineCashWireV1.MAXIMUM_NO_COMMIT_CLOSURE_BYTES,
        )
    }

    private fun validateLifecycleShape(value: OfflineCashLifecycleBindingV1) {
        require(value.liabilityPoolId().contentEquals(liabilityPoolId(value.networkId, value.asset, value.assetIncarnation)))
    }

    private fun validateCommitCertificateShape(
        certificate: OfflineCashCommitCertificateV1,
        lifecycle: OfflineCashLifecycleBindingV1,
        transitionNullifier: ByteArray,
        evidence: OfflineCashCommitEvidenceV1,
    ) {
        require(certificate.lifecycleBindingDigest().contentEquals(lifecycleDigest(lifecycle)))
        require(certificate.transitionNullifier().contentEquals(transitionNullifier))
        require(certificate.hardwareProfileId().contentEquals(lifecycle.hardwareProfileId()))
        require(certificate.policyEpoch == lifecycle.policyEpoch)
        require(rawEvidence(certificate.commitEvidence).contentEquals(rawEvidence(evidence)))
        require(certificate.certificateId().contentEquals(expectedCommitCertificateId(certificate)))
    }

    private fun validateWrapperShape(
        proof: OfflineCashCommitWrapperProofV1,
        semanticDigest: ByteArray,
        certificate: OfflineCashCommitCertificateV1,
    ) {
        require(proof.semanticDigest().contentEquals(semanticDigest))
        require(proof.candidateEnvelopeDigest().contentEquals(certificate.candidateEnvelopeDigest()))
        require(proof.commitCertificateDigest().contentEquals(commitCertificateDigest(certificate)))
    }

    private fun validatePaymentShape(value: OfflineCashPaymentV1, request: OfflineCashPaymentRequestV1) {
        validatePaymentRequestShape(request)
        val requestDigest = paymentRequestDigest(request)
        validateAcceptanceIntentShape(value.acceptanceIntent, request)
        validateTicketWithoutAuthorizationProof(value.acceptanceTicket, request, value.acceptanceIntent)
        val ticketDigest = digestEncoded(TICKET_DIGEST_DOMAIN, raw(value.acceptanceTicket, TICKET_SCHEMA, TICKET_ADAPTER))
        val statement = value.statement
        val lifecycle = statement.lifecycle
        validateLifecycleShape(lifecycle)
        require(lifecycle.operationKind == OfflineCashOperationKindV1.SEND_SPLIT)
        require(lifecycle.networkId == request.networkId)
        require(lifecycle.asset == request.asset && lifecycle.assetIncarnation == request.assetIncarnation)
        require(lifecycle.scale == request.scale && lifecycle.requestId().contentEquals(request.requestId()))
        require(lifecycle.acceptanceTicketId().contentEquals(value.acceptanceTicket.acceptanceTicketId()))
        require(statement.amount == value.acceptanceTicket.exactAmount)
        require(statement.requestDigest().contentEquals(requestDigest))
        require(statement.acceptanceTicketDigest().contentEquals(ticketDigest))
        require(statement.recipientOneTimeKey == value.acceptanceTicket.recipientOneTimeKey)
        peerCreditContextShape(statement, request, value.acceptanceIntent, value.acceptanceTicket)
        val envelope = decodeEncryptedCreditEnvelopeShapeExact(value.encryptedCredit())
        require(envelope.version == value.version)
        require(lifecycle.ciphertextDigest().contentEquals(ciphertextDigest(value.encryptedCredit())))
        require(lifecycle.creditId().contentEquals(expectedPeerCreditId(statement)))
        validateCommitCertificateShape(value.commitCertificate, lifecycle, statement.transitionNullifier(), statement.commitEvidence)
        validateWrapperShape(value.proof, statementDigest(statement), value.commitCertificate)
    }

    private fun validatePeerStatementContextShape(
        statement: OfflineCashTransferStatementV1,
        request: OfflineCashPaymentRequestV1,
        ticket: OfflineCashAcceptanceTicketV1,
    ) {
        val lifecycle = statement.lifecycle
        validateLifecycleShape(lifecycle)
        require(statement.version == OfflineCashWireV1.WIRE_VERSION)
        require(statement.requestDigest().contentEquals(paymentRequestDigest(request)))
        require(
            statement.acceptanceTicketDigest().contentEquals(
                digestEncoded(TICKET_DIGEST_DOMAIN, raw(ticket, TICKET_SCHEMA, TICKET_ADAPTER)),
            ),
        )
        require(statement.recipientOneTimeKey == ticket.recipientOneTimeKey)
        require(statement.amount == ticket.exactAmount)
        require(lifecycle.releaseId().contentEquals(request.releaseId()))
        require(lifecycle.networkId == request.networkId)
        require(lifecycle.asset == request.asset && lifecycle.assetIncarnation == request.assetIncarnation)
        require(lifecycle.scale == request.scale)
        require(lifecycle.liabilityPoolId().contentEquals(request.liabilityPoolId()))
        require(lifecycle.suiteId().contentEquals(request.hardwareCredential.suiteId()))
        require(lifecycle.requestId().contentEquals(request.requestId()))
        require(lifecycle.acceptanceTicketId().contentEquals(ticket.acceptanceTicketId()))
        require(lifecycle.creditId().contentEquals(expectedPeerCreditId(statement)))
    }

    private fun peerLifecycleContextDigest(lifecycle: OfflineCashLifecycleBindingV1): ByteArray =
        digestEncoded(
            PEER_CREDIT_LIFECYCLE_CONTEXT_DIGEST_DOMAIN,
            frame("iroha.offline-cash.v1.peer-credit-lifecycle-context-preimage") { e ->
                u16Field(e, lifecycle.version)
                networkField(e, lifecycle.networkId)
                u16Field(e, lifecycle.protocolVersion)
                bytes32Field(e, lifecycle.suiteId())
                bytes32Field(e, lifecycle.vkDigest())
                bytes32Field(e, lifecycle.releaseId())
                assetField(e, lifecycle.asset)
                incarnationField(e, lifecycle.assetIncarnation)
                u32Field(e, lifecycle.scale)
                bytes32Field(e, lifecycle.liabilityPoolId())
                bytes32Field(e, lifecycle.hardwareProfileId())
                u64Field(e, lifecycle.policyEpoch)
                enumUnitField(e, lifecycle.operationKind.ordinal)
                bytes32Field(e, lifecycle.requestId())
                bytes32Field(e, lifecycle.acceptanceTicketId())
            },
        )

    private fun validateTicketWithoutAuthorizationProof(
        value: OfflineCashAcceptanceTicketV1,
        request: OfflineCashPaymentRequestV1,
        intent: OfflineCashAcceptanceIntentV1,
    ) {
        validateAcceptanceIntentShape(intent, request)
        require(value.networkId == request.networkId)
        require(value.requestId().contentEquals(request.requestId()))
        require(value.requestDigest().contentEquals(paymentRequestDigest(request)))
        require(value.asset == request.asset && value.assetIncarnation == request.assetIncarnation)
        require(value.scale == request.scale && rawMode(value.requestMode).contentEquals(rawMode(request.requestMode)))
        require(value.intentDigest().contentEquals(acceptanceIntentDigest(intent, request)))
        require(value.exactAmount == intent.exactAmount)
        require(value.hardwareProfileId().contentEquals(request.hardwareCredential.hardwareProfileId()))
        require(value.policyEpoch == request.hardwareCredential.policyEpoch)
        require(value.issuedAtMs >= request.issuedAtMs && value.expiresAtMs <= request.expiresAtMs)
    }

    private fun validateAcknowledgementShape(
        value: OfflineCashAcknowledgementV1,
        request: OfflineCashPaymentRequestV1,
        payment: OfflineCashPaymentV1,
    ) {
        validatePaymentShape(payment, request)
        require(value.requestDigest().contentEquals(paymentRequestDigest(request)))
        require(value.paymentDigest().contentEquals(paymentDigest(payment, request)))
        require(value.inboxReceipt.creditId().contentEquals(payment.statement.lifecycle.creditId()))
    }

    private fun validateMintAuthorizationStatementShape(
        value: OfflineCashMintAuthorizationStatementV1,
    ) {
        validateMintAuthorizationContextShape(value.context)
    }

    private fun validateMintAuthorizationContextShape(value: OfflineCashMintAuthorizationContextV1) {
        require(value.liabilityPoolId().contentEquals(liabilityPoolId(value.networkId, value.asset, value.assetIncarnation)))
    }

    private fun validateMintAuthorizationShape(value: OfflineCashMintAuthorizationV1) {
        validateMintAuthorizationStatementShape(value.statement)
        val semantic = digestEncoded(
            MINT_AUTH_STATEMENT_DIGEST_DOMAIN,
            raw(value.statement, MODEL + "OfflineCashMintAuthorizationStatementV1", MINT_AUTH_STATEMENT_ADAPTER),
        )
        require(value.proof.semanticDigest().contentEquals(semantic))
    }

    private fun validateMintCreditShape(value: OfflineCashMintCreditV1) {
        val statement = value.statement
        val lifecycle = statement.lifecycle
        validateLifecycleShape(lifecycle)
        require(lifecycle.operationKind == OfflineCashOperationKindV1.MINT_FOLD)
        require(lifecycle.creditId().contentEquals(expectedMintCreditId(statement)))
        require(value.proof.semanticDigest().contentEquals(mintStatementDigest(statement)))
        val envelope = decodeEncryptedCreditEnvelopeShapeExact(value.encryptedCredit())
        require(envelope.version == value.version)
        require(lifecycle.ciphertextDigest().contentEquals(ciphertextDigest(value.encryptedCredit())))
    }

    private fun validateMintCreditShapeAgainstAuthorization(
        value: OfflineCashMintCreditV1,
        authorization: OfflineCashMintAuthorizationV1,
    ) {
        validateMintCreditShape(value)
        validateMintAuthorizationShape(authorization)
        val statement = value.statement
        val lifecycle = statement.lifecycle
        val context = authorization.statement.context
        require(lifecycle.releaseId().contentEquals(context.releaseId()))
        require(lifecycle.suiteId().contentEquals(context.suiteId()))
        require(lifecycle.vkDigest().contentEquals(context.vkDigest()))
        require(lifecycle.networkId == context.networkId && lifecycle.asset == context.asset)
        require(lifecycle.assetIncarnation == context.assetIncarnation && lifecycle.scale == context.scale)
        require(lifecycle.liabilityPoolId().contentEquals(context.liabilityPoolId()))
        require(lifecycle.hardwareProfileId().contentEquals(context.hardwareProfileId()))
        require(lifecycle.policyEpoch == context.policyEpoch)
        require(statement.amount == context.amount && statement.recipient == context.recipient)
        require(statement.recipientCredentialCommitment().contentEquals(context.recipientCredentialCommitment()))
        require(statement.creditCommitment().contentEquals(context.creditCommitment()))
        require(statement.authorizationContextDigest().contentEquals(mintAuthorizationContextDigest(context)))
        require(statement.mintAuthorizationDigest().contentEquals(mintAuthorizationDigest(authorization)))
        require(statement.issuanceCommitment().contentEquals(authorization.statement.issuanceCommitment()))
        require(lifecycle.creditId().contentEquals(authorization.statement.creditId()))
        require(lifecycle.ciphertextDigest().contentEquals(authorization.statement.ciphertextDigest()))
        require(lifecycle.ciphertextDigest().contentEquals(ciphertextDigest(value.encryptedCredit())))
        require(value.artifactManifestDigest().contentEquals(context.artifactManifestDigest()))
        encryptedCreditAadForMintShape(authorization.statement)
    }

    private fun validateRedemptionVoucherShape(value: OfflineCashRedemptionVoucherV1) {
        val statement = value.statement
        validateRedemptionStatementShape(statement)
        val lifecycle = statement.lifecycle
        validateCommitCertificateShape(value.commitCertificate, lifecycle, statement.terminalNullifier(), statement.commitEvidence)
        validateWrapperShape(value.proof, redemptionStatementDigest(statement), value.commitCertificate)
    }

    private fun validateTransferStatementShape(value: OfflineCashTransferStatementV1) {
        validateLifecycleShape(value.lifecycle)
        require(value.lifecycle.operationKind == OfflineCashOperationKindV1.SEND_SPLIT)
        require(value.lifecycle.creditId().contentEquals(expectedPeerCreditId(value)))
    }

    private fun validateRedemptionStatementShape(value: OfflineCashRedemptionStatementV1) {
        val lifecycle = value.lifecycle
        validateLifecycleShape(lifecycle)
        require(lifecycle.operationKind == OfflineCashOperationKindV1.REDEEM_SPLIT)
        require(!value.terminalNullifier().contentEquals(value.redemptionCommitment()))
        require(!value.terminalNullifier().contentEquals(value.redemptionId()))
        require(!value.redemptionCommitment().contentEquals(value.redemptionId()))
        require(value.redemptionId().contentEquals(expectedRedemptionId(value)))
    }

    private fun lifecycleDigest(value: OfflineCashLifecycleBindingV1): ByteArray {
        validateLifecycleShape(value)
        return digestEncoded(LIFECYCLE_DIGEST_DOMAIN, raw(value, LIFECYCLE_SCHEMA, LIFECYCLE_ADAPTER))
    }

    private fun statementDigest(value: OfflineCashTransferStatementV1): ByteArray =
        digestEncoded(STATEMENT_DIGEST_DOMAIN, raw(value, STATEMENT_SCHEMA, STATEMENT_ADAPTER))

    private fun commitCertificateDigest(value: OfflineCashCommitCertificateV1): ByteArray =
        digestEncoded(COMMIT_CERTIFICATE_DIGEST_DOMAIN, commitCertificateCircuitTranscript(value))

    private fun expectedCommitCertificateId(value: OfflineCashCommitCertificateV1): ByteArray =
        digestEncoded(
            COMMIT_CERTIFICATE_ID_DOMAIN,
            commitCertificateIdCircuitTranscript(value),
        )

    private fun ciphertextDigest(bytes: ByteArray): ByteArray = digestBytes(CIPHERTEXT_DIGEST_DOMAIN, bytes)

    private fun paymentDigest(value: OfflineCashPaymentV1, request: OfflineCashPaymentRequestV1): ByteArray {
        validatePaymentShape(value, request)
        return digestEncoded(PAYMENT_DIGEST_DOMAIN, raw(value, PAYMENT_SCHEMA, PAYMENT_ADAPTER))
    }

    private fun mintAuthorizationContextDigest(value: OfflineCashMintAuthorizationContextV1): ByteArray =
        digestEncoded(
            MINT_AUTH_CONTEXT_DIGEST_DOMAIN,
            raw(value, MODEL + "OfflineCashMintAuthorizationContextV1", MINT_AUTH_CONTEXT_ADAPTER),
        )

    private fun mintAuthorizationDigest(value: OfflineCashMintAuthorizationV1): ByteArray =
        digestEncoded(MINT_AUTH_DIGEST_DOMAIN, raw(value, MINT_AUTH_SCHEMA, MINT_AUTH_ADAPTER))

    private fun mintStatementDigest(value: OfflineCashMintCreditStatementV1): ByteArray =
        digestEncoded(MINT_STATEMENT_DIGEST_DOMAIN, raw(value, MINT_STATEMENT_SCHEMA, MINT_STATEMENT_ADAPTER))

    private fun mintLifecycleContextDigest(value: OfflineCashLifecycleBindingV1): ByteArray =
        digestEncoded(
            MINT_LIFECYCLE_CONTEXT_DOMAIN,
            frame("iroha.offline-cash.v1.mint-lifecycle-context-preimage") { e ->
                u16Field(e, value.version)
                networkField(e, value.networkId)
                u16Field(e, value.protocolVersion)
                bytes32Field(e, value.suiteId())
                bytes32Field(e, value.vkDigest())
                bytes32Field(e, value.releaseId())
                assetField(e, value.asset)
                incarnationField(e, value.assetIncarnation)
                u32Field(e, value.scale)
                bytes32Field(e, value.liabilityPoolId())
                bytes32Field(e, value.hardwareProfileId())
                u64Field(e, value.policyEpoch)
                enumUnitField(e, value.operationKind.ordinal)
            },
        )

    private fun expectedMintCreditId(value: OfflineCashMintCreditStatementV1): ByteArray =
        digestEncoded(
            MINT_CREDIT_ID_DOMAIN,
            frame("iroha.offline-cash.v1.mint-credit-id-preimage", 16) { e ->
                bytes32Field(e, mintLifecycleContextDigest(value.lifecycle))
                bytes32Field(e, value.recipientCredentialCommitment())
                bytes32Field(e, value.authorizationContextDigest())
                u128Field(e, value.amount)
                bytes32Field(e, value.issuanceCommitment())
                accountField(e, value.recipient)
                bytes32Field(e, value.creditCommitment())
            },
        )

    private fun redemptionStatementDigest(value: OfflineCashRedemptionStatementV1): ByteArray =
        digestEncoded(
            REDEMPTION_STATEMENT_DIGEST_DOMAIN,
            raw(value, REDEMPTION_STATEMENT_SCHEMA, REDEMPTION_STATEMENT_ADAPTER),
        )

    private fun expectedRedemptionId(value: OfflineCashRedemptionStatementV1): ByteArray =
        digestEncoded(
            REDEMPTION_ID_DOMAIN,
            frame("iroha.offline-cash.v1.redemption-id-preimage", 16) { e ->
                bytes32Field(e, lifecycleDigest(value.lifecycle))
                bytes32Field(e, value.terminalNullifier())
                u128Field(e, value.amount)
                accountField(e, value.beneficiary)
                bytes32Field(e, value.redemptionCommitment())
            },
        )

    private fun expectedPeerCreditId(value: OfflineCashTransferStatementV1): ByteArray = digestEncoded(
        CREDIT_ID_DOMAIN,
        frame("iroha.offline-cash.v1.credit-id-preimage", 16) { encoder ->
            field(encoder) { fixedArray(it, value.transitionNullifier()) }
            field(encoder) { fixedArray(it, value.requestDigest()) }
            field(encoder) { fixedArray(it, value.acceptanceTicketDigest()) }
            field(encoder) { fixedArray(it, value.recipientOneTimeKey.bytes()) }
            field(encoder) { uint128(it, value.amount) }
            field(encoder) { fixedArray(it, value.ciphertextCommitment()) }
        },
    )

    private fun acceptanceIntentCircuitTranscript(value: OfflineCashAcceptanceIntentV1): ByteArray =
        fixedTranscript(
            ACCEPTANCE_INTENT_TRANSCRIPT_BYTES,
            u16Le(value.version),
            value.requestDigest(),
            value.intentId(),
            u128Le(value.exactAmount),
            value.senderOneTimeCommitment(),
        )

    private fun acceptanceIntentAuthorizationStatementCircuitTranscript(
        value: OfflineCashAcceptanceIntentAuthorizationStatementV1,
    ): ByteArray = fixedTranscript(
        INTENT_AUTHORIZATION_STATEMENT_TRANSCRIPT_BYTES,
        u16Le(value.version),
        acceptanceIntentCircuitTranscript(value.intent),
        value.releaseId(),
        value.suiteId(),
        value.vkDigest(),
        value.artifactManifestDigest(),
    )

    private fun noCommitClosureStatementCircuitTranscript(
        value: OfflineCashNoCommitClosureStatementV1,
    ): ByteArray = fixedTranscript(
        NO_COMMIT_CLOSURE_STATEMENT_TRANSCRIPT_BYTES,
        u16Le(value.version),
        value.releaseId(),
        value.suiteId(),
        value.vkDigest(),
        value.artifactManifestDigest(),
        value.senderHardwareBindingCommitment(),
        value.requestId(),
        value.requestDigest(),
        value.acceptanceTicketId(),
        value.ticketDigest(),
        value.intentAuthorizationDigest(),
        value.intentDigest(),
        u128Le(value.exactAmount),
        value.senderOneTimeCommitment(),
        value.recoveryId(),
        value.cancellationNullifier(),
        value.equivalentDeliverySlotCommitment(),
    )

    private fun outboxReservationCircuitTranscript(value: OfflineCashOutboxReservationV1): ByteArray =
        fixedTranscript(
            OUTBOX_RESERVATION_TRANSCRIPT_BYTES,
            value.reservationId(),
            u32Le(operationKindTag(value.operationKind)),
            u32Le(value.reservedOutboxBytes),
            u64Le(value.issuedAtMs),
            u64Le(value.expiresAtMs),
        )

    private fun commitEvidenceCircuitTranscript(value: OfflineCashCommitEvidenceV1): ByteArray = when (value) {
        is OfflineCashCommitEvidenceV1.TrustedTime ->
            fixedTranscript(
                COMMIT_EVIDENCE_TRANSCRIPT_BYTES,
                u32Le(0),
                value.timeEvidenceCommitment(),
            )
        is OfflineCashCommitEvidenceV1.MonotonicLease ->
            fixedTranscript(
                COMMIT_EVIDENCE_TRANSCRIPT_BYTES,
                u32Le(1),
                value.leaseEvidenceCommitment(),
            )
    }

    private fun commitCertificateIdCircuitTranscript(value: OfflineCashCommitCertificateV1): ByteArray =
        fixedTranscript(
            COMMIT_CERTIFICATE_ID_TRANSCRIPT_BYTES,
            u16Le(value.version),
            value.candidateEnvelopeDigest(),
            value.lifecycleBindingDigest(),
            value.transitionNullifier(),
            value.outboxReservationCommitment(),
            commitEvidenceCircuitTranscript(value.commitEvidence),
            value.hardwareProfileId(),
            u64Le(value.policyEpoch),
            value.hardwareTerminalCommitment(),
        )

    private fun commitCertificateCircuitTranscript(value: OfflineCashCommitCertificateV1): ByteArray =
        fixedTranscript(
            COMMIT_CERTIFICATE_TRANSCRIPT_BYTES,
            u16Le(value.version),
            value.certificateId(),
            value.candidateEnvelopeDigest(),
            value.lifecycleBindingDigest(),
            value.transitionNullifier(),
            value.outboxReservationCommitment(),
            commitEvidenceCircuitTranscript(value.commitEvidence),
            value.hardwareProfileId(),
            u64Le(value.policyEpoch),
            value.hardwareTerminalCommitment(),
        )

    private fun operationKindTag(value: OfflineCashOperationKindV1): Int = when (value) {
        OfflineCashOperationKindV1.BOOTSTRAP -> 0
        OfflineCashOperationKindV1.MINT_FOLD -> 1
        OfflineCashOperationKindV1.SEND_SPLIT -> 2
        OfflineCashOperationKindV1.RECEIVE_FOLD_BATCH -> 3
        OfflineCashOperationKindV1.REDEEM_SPLIT -> 4
        OfflineCashOperationKindV1.SUITE_UPGRADE -> 5
        OfflineCashOperationKindV1.ROTATE -> 6
    }

    private fun rawMode(value: OfflineCashPaymentRequestModeV1): ByteArray =
        raw(value, REQUEST_MODE_SCHEMA, REQUEST_MODE_ADAPTER)

    private fun rawEvidence(value: OfflineCashCommitEvidenceV1): ByteArray =
        frame(MODEL + "OfflineCashCommitEvidenceV1") { COMMIT_EVIDENCE_ADAPTER.encode(it, value) }

    private val AGGREGATE_ADAPTER = adapter<OfflineCashAggregateStateCommitmentV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            bytes32Field(e, v.releaseId())
            networkField(e, v.networkId)
            assetField(e, v.asset)
            incarnationField(e, v.assetIncarnation)
            u32Field(e, v.scale)
            bytes32Field(e, v.liabilityPoolId())
            bytes32Field(e, v.laneId())
            bytes32Field(e, v.hardwareEpochId())
            bytes32Field(e, v.keyReference())
            bytes32Field(e, v.hardwarePolicyId())
            u128Field(e, v.sequence)
            bytes32Field(e, v.stateCommitment())
        },
        decode = { d ->
            OfflineCashAggregateStateCommitmentV1(
                readU16(d), readFixed32(d), readNetwork(d), readAsset(d), readIncarnation(d),
                readU32(d), readFixed32(d), readFixed32(d), readFixed32(d), readFixed32(d),
                readFixed32(d), readU128(d), readFixed32(d),
            )
        },
    )

    private val PROOF_ADAPTER = adapter<OfflineCashPairedProofV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            bytes32Field(e, v.eqProtocolDigest())
            bytes32Field(e, v.epProtocolDigest())
            bytes32Field(e, v.semanticDigest())
            bytes32Field(e, v.guardEqCredentialAudit())
            bytes32Field(e, v.guardEpCredentialAudit())
            bytes32Field(e, v.eqDeferredAudit())
            bytes32Field(e, v.epDeferredAudit())
            vectorField(e, v.eqProof())
            vectorField(e, v.epProof())
            vectorField(e, v.eqHistory())
            vectorField(e, v.epHistory())
        },
        decode = { d ->
            OfflineCashPairedProofV1(
                readU16(d), readFixed32(d), readFixed32(d), readFixed32(d), readFixed32(d),
                readFixed32(d), readFixed32(d), readFixed32(d), readVector(d), readVector(d),
                readVector(d), readVector(d),
            )
        },
    )

    private val HARDWARE_PROFILE_ADAPTER = adapter<OfflineCashHardwareProfileV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            u16Field(e, v.protocolVersion)
            bytes32Field(e, v.hardwareProfileId())
            bytes32Field(e, v.providerId())
            enumUnitField(e, v.platformClass.ordinal)
            bytes32Field(e, v.productClassDigest())
            bytes32Field(e, v.firmwarePolicyDigest())
            bytes32Field(e, v.enrollmentAttestationVerifierDigest())
            bytes32Field(e, v.attestationTrustRootsDigest())
            bytes32Field(e, v.allowedSuiteCommitment())
            u64Field(e, v.policyEpoch)
            publicKeyField(e, v.governanceCredentialPublicKey)
            u16Field(e, v.capabilityMask)
            bytes32Field(e, v.qualificationReportDigest())
            u64Field(e, v.validFromMs)
            u64Field(e, v.expiresAtMs)
        },
        decode = { d ->
            OfflineCashHardwareProfileV1(
                readU16(d), readU16(d), readFixed32(d), readFixed32(d),
                OfflineCashHardwarePlatformClassV1.values()[readEnumUnit(d, 4)],
                readFixed32(d), readFixed32(d), readFixed32(d), readFixed32(d), readFixed32(d),
                readU64(d), readPublicKey(d), readU16(d), readFixed32(d), readU64(d), readU64(d),
            )
        },
    )

    private val HARDWARE_CREDENTIAL_ADAPTER = adapter<OfflineCashHardwareCredentialV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            bytes32Field(e, v.credentialId())
            networkField(e, v.networkId)
            bytes32Field(e, v.hardwareProfileId())
            bytes32Field(e, v.suiteId())
            bytes32Field(e, v.firmwarePolicyDigest())
            u64Field(e, v.policyEpoch)
            bytes32Field(e, v.laneCommitment())
            bytes32Field(e, v.hardwareEpochId())
            u64Field(e, v.hardwareEpochGeneration)
            publicKeyField(e, v.devicePublicKey)
            bytes32Field(e, v.deviceKeyReference())
            u64Field(e, v.issuedAtMs)
            u64Field(e, v.expiresAtMs)
            signatureField(e, v.governanceSignature)
        },
        decode = { d ->
            OfflineCashHardwareCredentialV1(
                readU16(d), readFixed32(d), readNetwork(d), readFixed32(d), readFixed32(d),
                readFixed32(d), readU64(d), readFixed32(d), readFixed32(d), readU64(d),
                readPublicKey(d), readFixed32(d), readU64(d), readU64(d), readSignature(d),
            )
        },
    )

    private val REQUEST_MODE_ADAPTER = adapter<OfflineCashPaymentRequestModeV1>(
        encode = { e, v ->
            when (v) {
                is OfflineCashPaymentRequestModeV1.SingleExact -> enumPayload(e, 0) { u128Field(it, v.amount) }
                is OfflineCashPaymentRequestModeV1.PartialUntilTotal -> enumPayload(e, 1) { u128Field(it, v.totalAmount) }
                is OfflineCashPaymentRequestModeV1.BoundedMultiPayment -> enumPayload(e, 2) {
                    u32Field(it, v.maxPayments)
                    amountPolicyFields(it, v.perPayment)
                }
                is OfflineCashPaymentRequestModeV1.OpenReceive -> enumPayload(e, 3) {
                    amountPolicyFields(it, v.perPayment)
                }
            }
        },
        decode = { d ->
            when (val tag = readEnumTag(d, 4)) {
                0 -> readEnumPayload(d) { OfflineCashPaymentRequestModeV1.SingleExact(readU128(it)) }
                1 -> readEnumPayload(d) { OfflineCashPaymentRequestModeV1.PartialUntilTotal(readU128(it)) }
                2 -> readEnumPayload(d) {
                    OfflineCashPaymentRequestModeV1.BoundedMultiPayment(readU32(it), readAmountPolicy(it))
                }
                3 -> readEnumPayload(d) { OfflineCashPaymentRequestModeV1.OpenReceive(readAmountPolicy(it)) }
                else -> throw IllegalArgumentException("unknown Offline Cash request mode tag: $tag")
            }
        },
    )

    private val REQUEST_ADAPTER = adapter<OfflineCashPaymentRequestV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            bytes32Field(e, v.releaseId())
            networkField(e, v.networkId)
            assetField(e, v.asset)
            incarnationField(e, v.assetIncarnation)
            u32Field(e, v.scale)
            bytes32Field(e, v.liabilityPoolId())
            accountField(e, v.recipient)
            nestedField(e, REQUEST_MODE_ADAPTER, v.requestMode)
            nestedField(e, HARDWARE_CREDENTIAL_ADAPTER, v.hardwareCredential)
            bytes32Field(e, v.requestId())
            u64Field(e, v.issuedAtMs)
            u64Field(e, v.expiresAtMs)
            signatureField(e, v.signature)
        },
        decode = { d ->
            OfflineCashPaymentRequestV1(
                readU16(d), readFixed32(d), readNetwork(d), readAsset(d), readIncarnation(d),
                readU32(d), readFixed32(d), readAccount(d), readNested(d, REQUEST_MODE_ADAPTER),
                readNested(d, HARDWARE_CREDENTIAL_ADAPTER), readFixed32(d), readU64(d), readU64(d),
                readSignature(d),
            )
        },
    )

    private val INTENT_ADAPTER = adapter<OfflineCashAcceptanceIntentV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            bytes32Field(e, v.requestDigest())
            bytes32Field(e, v.intentId())
            u128Field(e, v.exactAmount)
            bytes32Field(e, v.senderOneTimeCommitment())
        },
        decode = { d -> OfflineCashAcceptanceIntentV1(readU16(d), readFixed32(d), readFixed32(d), readU128(d), readFixed32(d)) },
    )

    private val INTENT_AUTH_STATEMENT_ADAPTER = adapter<OfflineCashAcceptanceIntentAuthorizationStatementV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            nestedField(e, INTENT_ADAPTER, v.intent)
            bytes32Field(e, v.releaseId())
            bytes32Field(e, v.suiteId())
            bytes32Field(e, v.vkDigest())
            bytes32Field(e, v.artifactManifestDigest())
        },
        decode = { d ->
            OfflineCashAcceptanceIntentAuthorizationStatementV1(
                readU16(d), readNested(d, INTENT_ADAPTER), readFixed32(d), readFixed32(d),
                readFixed32(d), readFixed32(d),
            )
        },
    )

    private val INTENT_AUTH_ADAPTER = adapter<OfflineCashAcceptanceIntentAuthorizationV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            nestedField(e, INTENT_AUTH_STATEMENT_ADAPTER, v.statement)
            nestedField(e, PROOF_ADAPTER, v.proof)
        },
        decode = { d -> OfflineCashAcceptanceIntentAuthorizationV1(readU16(d), readNested(d, INTENT_AUTH_STATEMENT_ADAPTER), readNested(d, PROOF_ADAPTER)) },
    )

    private val NO_COMMIT_CLOSURE_STATEMENT_ADAPTER = adapter<OfflineCashNoCommitClosureStatementV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            bytes32Field(e, v.releaseId())
            bytes32Field(e, v.suiteId())
            bytes32Field(e, v.vkDigest())
            bytes32Field(e, v.artifactManifestDigest())
            bytes32Field(e, v.senderHardwareBindingCommitment())
            bytes32Field(e, v.requestId())
            bytes32Field(e, v.requestDigest())
            bytes32Field(e, v.acceptanceTicketId())
            bytes32Field(e, v.ticketDigest())
            bytes32Field(e, v.intentAuthorizationDigest())
            bytes32Field(e, v.intentDigest())
            u128Field(e, v.exactAmount)
            bytes32Field(e, v.senderOneTimeCommitment())
            bytes32Field(e, v.recoveryId())
            bytes32Field(e, v.cancellationNullifier())
            bytes32Field(e, v.equivalentDeliverySlotCommitment())
        },
        decode = { d ->
            OfflineCashNoCommitClosureStatementV1(
                readU16(d), readFixed32(d), readFixed32(d), readFixed32(d), readFixed32(d),
                readFixed32(d), readFixed32(d), readFixed32(d), readFixed32(d), readFixed32(d),
                readFixed32(d), readFixed32(d), readU128(d), readFixed32(d), readFixed32(d),
                readFixed32(d), readFixed32(d),
            )
        },
    )

    private val TICKET_ADAPTER = adapter<OfflineCashAcceptanceTicketV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            networkField(e, v.networkId)
            bytes32Field(e, v.requestId())
            bytes32Field(e, v.requestDigest())
            bytes32Field(e, v.acceptanceTicketId())
            assetField(e, v.asset)
            incarnationField(e, v.assetIncarnation)
            u32Field(e, v.scale)
            nestedField(e, REQUEST_MODE_ADAPTER, v.requestMode)
            bytes32Field(e, v.intentDigest())
            u128Field(e, v.exactAmount)
            u32Field(e, v.reservedInboxBytes)
            bytes32Field(e, v.recipientOneTimeKey.bytes())
            bytes32Field(e, v.hardwareProfileId())
            u64Field(e, v.policyEpoch)
            u64Field(e, v.issuedAtMs)
            u64Field(e, v.expiresAtMs)
            signatureField(e, v.signature)
        },
        decode = { d ->
            OfflineCashAcceptanceTicketV1(
                readU16(d), readNetwork(d), readFixed32(d), readFixed32(d), readFixed32(d),
                readAsset(d), readIncarnation(d), readU32(d), readNested(d, REQUEST_MODE_ADAPTER),
                readFixed32(d), readU128(d), readU32(d), OfflineCashX25519PublicKeyV1(readRaw32(d)),
                readFixed32(d), readU64(d), readU64(d), readU64(d), readSignature(d),
            )
        },
    )

    private val NO_COMMIT_CLOSURE_ADAPTER = adapter<OfflineCashNoCommitClosureV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            nestedField(e, NO_COMMIT_CLOSURE_STATEMENT_ADAPTER, v.statement)
            nestedField(e, REQUEST_ADAPTER, v.request)
            nestedField(e, INTENT_AUTH_ADAPTER, v.intentAuthorization)
            nestedField(e, TICKET_ADAPTER, v.acceptanceTicket)
            nestedField(e, PROOF_ADAPTER, v.proof)
        },
        decode = { d ->
            OfflineCashNoCommitClosureV1(
                readU16(d), readNested(d, NO_COMMIT_CLOSURE_STATEMENT_ADAPTER),
                readNested(d, REQUEST_ADAPTER), readNested(d, INTENT_AUTH_ADAPTER),
                readNested(d, TICKET_ADAPTER), readNested(d, PROOF_ADAPTER),
            )
        },
    )

    private val PEER_CREDIT_CONTEXT_ADAPTER = adapter<OfflineCashPeerCreditContextV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            bytes32Field(e, v.requestDigest())
            bytes32Field(e, v.acceptanceIntentDigest())
            bytes32Field(e, v.acceptanceTicketDigest())
            bytes32Field(e, v.lifecycleContextDigest())
            bytes32Field(e, v.recipientOneTimeKey.bytes())
        },
        decode = { d ->
            OfflineCashPeerCreditContextV1(
                readU16(d), readFixed32(d), readFixed32(d), readFixed32(d), readFixed32(d),
                OfflineCashX25519PublicKeyV1(readRaw32(d)),
            )
        },
    )

    private val CREDIT_OPENING_ADAPTER = adapter<OfflineCashCreditOpeningV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            bytes32Field(e, v.creditId())
            u128Field(e, v.amount)
            bytes32Field(e, v.creditCommitmentOpening())
            bytes32Field(e, v.recipientBindingOpening())
            bytes32Field(e, v.recoveryNonce())
        },
        decode = { d ->
            OfflineCashCreditOpeningV1(
                readU16(d), readFixed32(d), readU128(d), readFixed32(d), readFixed32(d), readFixed32(d),
            )
        },
    )

    private val CREDIT_AAD_ADAPTER = adapter<OfflineCashEncryptedCreditAadV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            enumUnitField(e, v.purpose.ordinal)
            bytes32Field(e, v.contextDigest())
            bytes32Field(e, v.issuanceOrTransitionCommitment())
            bytes32Field(e, v.creditId())
            u128Field(e, v.amount)
        },
        decode = { d ->
            OfflineCashEncryptedCreditAadV1(
                readU16(d), OfflineCashEncryptedCreditPurposeV1.values()[readEnumUnit(d, 2)],
                readFixed32(d), readFixed32(d), readFixed32(d), readU128(d),
            )
        },
    )

    private val CREDIT_ENVELOPE_ADAPTER = adapter<OfflineCashEncryptedCreditEnvelopeV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            bytes32Field(e, v.ephemeralX25519PublicKey.bytes())
            field(e) { fixedArray(it, v.nonce()) }
            vectorField(e, v.ciphertextAndTag())
        },
        decode = { d ->
            OfflineCashEncryptedCreditEnvelopeV1(
                readU16(d), OfflineCashX25519PublicKeyV1(readRaw32(d)), readExactField(d, 24), readVector(d),
            )
        },
    )

    private val LIFECYCLE_ADAPTER = adapter<OfflineCashLifecycleBindingV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            networkField(e, v.networkId)
            u16Field(e, v.protocolVersion)
            bytes32Field(e, v.suiteId())
            bytes32Field(e, v.vkDigest())
            bytes32Field(e, v.releaseId())
            assetField(e, v.asset)
            incarnationField(e, v.assetIncarnation)
            u32Field(e, v.scale)
            bytes32Field(e, v.liabilityPoolId())
            bytes32Field(e, v.hardwareProfileId())
            u64Field(e, v.policyEpoch)
            enumUnitField(e, v.operationKind.ordinal)
            raw32Field(e, v.requestId())
            raw32Field(e, v.acceptanceTicketId())
            raw32Field(e, v.creditId())
            raw32Field(e, v.ciphertextDigest())
        },
        decode = { d ->
            OfflineCashLifecycleBindingV1(
                readU16(d), readNetwork(d), readU16(d), readFixed32(d), readFixed32(d), readFixed32(d),
                readAsset(d), readIncarnation(d), readU32(d), readFixed32(d), readFixed32(d), readU64(d),
                OfflineCashOperationKindV1.values()[readEnumUnit(d, 7)], readRaw32(d), readRaw32(d),
                readRaw32(d), readRaw32(d),
            )
        },
    )

    private val COMMIT_EVIDENCE_ADAPTER = adapter<OfflineCashCommitEvidenceV1>(
        encode = { e, v ->
            when (v) {
                is OfflineCashCommitEvidenceV1.TrustedTime -> enumPayload(e, 0) {
                    bytes32Field(it, v.timeEvidenceCommitment())
                }
                is OfflineCashCommitEvidenceV1.MonotonicLease -> enumPayload(e, 1) {
                    bytes32Field(it, v.leaseEvidenceCommitment())
                }
            }
        },
        decode = { d ->
            when (val tag = readEnumTag(d, 2)) {
                0 -> readEnumPayload(d) { OfflineCashCommitEvidenceV1.TrustedTime(readFixed32(it)) }
                1 -> readEnumPayload(d) { OfflineCashCommitEvidenceV1.MonotonicLease(readFixed32(it)) }
                else -> throw IllegalArgumentException("unknown Offline Cash commit-evidence tag: $tag")
            }
        },
    )

    private val COMMIT_CERTIFICATE_ADAPTER = adapter<OfflineCashCommitCertificateV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            bytes32Field(e, v.certificateId())
            bytes32Field(e, v.candidateEnvelopeDigest())
            bytes32Field(e, v.lifecycleBindingDigest())
            bytes32Field(e, v.transitionNullifier())
            bytes32Field(e, v.outboxReservationCommitment())
            nestedField(e, COMMIT_EVIDENCE_ADAPTER, v.commitEvidence)
            bytes32Field(e, v.hardwareProfileId())
            u64Field(e, v.policyEpoch)
            bytes32Field(e, v.hardwareTerminalCommitment())
        },
        decode = { d ->
            OfflineCashCommitCertificateV1(
                readU16(d), readFixed32(d), readFixed32(d), readFixed32(d), readFixed32(d),
                readFixed32(d), readNested(d, COMMIT_EVIDENCE_ADAPTER), readFixed32(d), readU64(d),
                readFixed32(d),
            )
        },
    )

    private val COMMIT_WRAPPER_ADAPTER = adapter<OfflineCashCommitWrapperProofV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            bytes32Field(e, v.eqProtocolDigest())
            bytes32Field(e, v.epProtocolDigest())
            bytes32Field(e, v.semanticDigest())
            bytes32Field(e, v.candidateEnvelopeDigest())
            bytes32Field(e, v.commitCertificateDigest())
            bytes32Field(e, v.eqDeferredAudit())
            bytes32Field(e, v.epDeferredAudit())
            vectorField(e, v.eqProof())
            vectorField(e, v.epProof())
            vectorField(e, v.eqHistory())
            vectorField(e, v.epHistory())
        },
        decode = { d ->
            OfflineCashCommitWrapperProofV1(
                readU16(d), readFixed32(d), readFixed32(d), readFixed32(d), readFixed32(d),
                readFixed32(d), readFixed32(d), readFixed32(d), readVector(d), readVector(d),
                readVector(d), readVector(d),
            )
        },
    )

    private val STATEMENT_ADAPTER = adapter<OfflineCashTransferStatementV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            nestedField(e, LIFECYCLE_ADAPTER, v.lifecycle)
            u128Field(e, v.amount)
            bytes32Field(e, v.transitionNullifier())
            bytes32Field(e, v.requestDigest())
            bytes32Field(e, v.acceptanceTicketDigest())
            bytes32Field(e, v.recipientOneTimeKey.bytes())
            bytes32Field(e, v.ciphertextCommitment())
            nestedField(e, COMMIT_EVIDENCE_ADAPTER, v.commitEvidence)
        },
        decode = { d ->
            OfflineCashTransferStatementV1(
                readU16(d), readNested(d, LIFECYCLE_ADAPTER), readU128(d), readFixed32(d),
                readFixed32(d), readFixed32(d), OfflineCashX25519PublicKeyV1(readRaw32(d)),
                readFixed32(d), readNested(d, COMMIT_EVIDENCE_ADAPTER),
            )
        },
    )

    private val PAYMENT_ADAPTER = adapter<OfflineCashPaymentV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            nestedField(e, STATEMENT_ADAPTER, v.statement)
            nestedField(e, INTENT_ADAPTER, v.acceptanceIntent)
            nestedField(e, TICKET_ADAPTER, v.acceptanceTicket)
            nestedField(e, COMMIT_CERTIFICATE_ADAPTER, v.commitCertificate)
            nestedField(e, COMMIT_WRAPPER_ADAPTER, v.proof)
            vectorField(e, v.encryptedCredit())
            bytes32Field(e, v.artifactManifestDigest())
        },
        decode = { d ->
            OfflineCashPaymentV1(
                readU16(d), readNested(d, STATEMENT_ADAPTER), readNested(d, INTENT_ADAPTER),
                readNested(d, TICKET_ADAPTER), readNested(d, COMMIT_CERTIFICATE_ADAPTER),
                readNested(d, COMMIT_WRAPPER_ADAPTER), readVector(d), readFixed32(d),
            )
        },
    )

    private val RECEIPT_ADAPTER = adapter<OfflineCashInboxReceiptV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            bytes32Field(e, v.creditId())
            bytes32Field(e, v.receiptCommitment())
        },
        decode = { d -> OfflineCashInboxReceiptV1(readU16(d), readFixed32(d), readFixed32(d)) },
    )

    private val ACK_ADAPTER = adapter<OfflineCashAcknowledgementV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            bytes32Field(e, v.requestDigest())
            bytes32Field(e, v.paymentDigest())
            nestedField(e, RECEIPT_ADAPTER, v.inboxReceipt)
            signatureField(e, v.signature)
        },
        decode = { d ->
            OfflineCashAcknowledgementV1(
                readU16(d), readFixed32(d), readFixed32(d), readNested(d, RECEIPT_ADAPTER), readSignature(d),
            )
        },
    )

    private val MINT_AUTH_CONTEXT_ADAPTER = adapter<OfflineCashMintAuthorizationContextV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            bytes32Field(e, v.operationId())
            bytes32Field(e, v.releaseId())
            bytes32Field(e, v.suiteId())
            bytes32Field(e, v.vkDigest())
            bytes32Field(e, v.artifactManifestDigest())
            networkField(e, v.networkId)
            assetField(e, v.asset)
            incarnationField(e, v.assetIncarnation)
            u32Field(e, v.scale)
            bytes32Field(e, v.liabilityPoolId())
            u128Field(e, v.amount)
            accountField(e, v.payer)
            accountField(e, v.recipient)
            bytes32Field(e, v.hardwareCredentialId())
            bytes32Field(e, v.hardwareProfileId())
            u64Field(e, v.policyEpoch)
            bytes32Field(e, v.recipientCredentialCommitment())
            bytes32Field(e, v.creditCommitment())
            bytes32Field(e, v.recipientOneTimeKey.bytes())
        },
        decode = { d ->
            OfflineCashMintAuthorizationContextV1(
                readU16(d), readFixed32(d), readFixed32(d), readFixed32(d), readFixed32(d),
                readFixed32(d), readNetwork(d), readAsset(d), readIncarnation(d), readU32(d),
                readFixed32(d), readU128(d), readAccount(d), readAccount(d), readFixed32(d),
                readFixed32(d), readU64(d), readFixed32(d), readFixed32(d),
                OfflineCashX25519PublicKeyV1(readRaw32(d)),
            )
        },
    )

    private val MINT_AUTH_STATEMENT_ADAPTER = adapter<OfflineCashMintAuthorizationStatementV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            nestedField(e, MINT_AUTH_CONTEXT_ADAPTER, v.context)
            bytes32Field(e, v.issuanceCommitment())
            bytes32Field(e, v.creditId())
            bytes32Field(e, v.ciphertextDigest())
        },
        decode = { d ->
            OfflineCashMintAuthorizationStatementV1(
                readU16(d), readNested(d, MINT_AUTH_CONTEXT_ADAPTER), readFixed32(d),
                readFixed32(d), readFixed32(d),
            )
        },
    )

    private val MINT_AUTH_ADAPTER = adapter<OfflineCashMintAuthorizationV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            nestedField(e, MINT_AUTH_STATEMENT_ADAPTER, v.statement)
            nestedField(e, PROOF_ADAPTER, v.proof)
        },
        decode = { d -> OfflineCashMintAuthorizationV1(readU16(d), readNested(d, MINT_AUTH_STATEMENT_ADAPTER), readNested(d, PROOF_ADAPTER)) },
    )

    private val MINT_STATEMENT_ADAPTER = adapter<OfflineCashMintCreditStatementV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            nestedField(e, LIFECYCLE_ADAPTER, v.lifecycle)
            bytes32Field(e, v.recipientCredentialCommitment())
            bytes32Field(e, v.authorizationContextDigest())
            bytes32Field(e, v.mintAuthorizationDigest())
            u128Field(e, v.amount)
            bytes32Field(e, v.issuanceCommitment())
            accountField(e, v.recipient)
            bytes32Field(e, v.creditCommitment())
            u64Field(e, v.mintedAtMs)
        },
        decode = { d ->
            OfflineCashMintCreditStatementV1(
                readU16(d), readNested(d, LIFECYCLE_ADAPTER), readFixed32(d), readFixed32(d),
                readFixed32(d), readU128(d), readFixed32(d), readAccount(d), readFixed32(d), readU64(d),
            )
        },
    )

    private val MINT_ADAPTER = adapter<OfflineCashMintCreditV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            nestedField(e, MINT_STATEMENT_ADAPTER, v.statement)
            nestedField(e, PROOF_ADAPTER, v.proof)
            bytes32Field(e, v.finalityCertificateBinding())
            bytes32Field(e, v.finalityAuthorityHead())
            bytes32Field(e, v.finalityGenesisRosterId())
            bytes32Field(e, v.finalityProofBindingDigest())
            vectorField(e, v.encryptedCredit())
            bytes32Field(e, v.artifactManifestDigest())
        },
        decode = { d ->
            OfflineCashMintCreditV1(
                readU16(d), readNested(d, MINT_STATEMENT_ADAPTER), readNested(d, PROOF_ADAPTER),
                readFixed32(d), readFixed32(d), readFixed32(d), readFixed32(d), readVector(d), readFixed32(d),
            )
        },
    )

    private val REDEMPTION_STATEMENT_ADAPTER = adapter<OfflineCashRedemptionStatementV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            nestedField(e, LIFECYCLE_ADAPTER, v.lifecycle)
            u128Field(e, v.amount)
            accountField(e, v.beneficiary)
            bytes32Field(e, v.terminalNullifier())
            bytes32Field(e, v.redemptionCommitment())
            bytes32Field(e, v.redemptionId())
            nestedField(e, COMMIT_EVIDENCE_ADAPTER, v.commitEvidence)
        },
        decode = { d ->
            OfflineCashRedemptionStatementV1(
                readU16(d), readNested(d, LIFECYCLE_ADAPTER), readU128(d), readAccount(d),
                readFixed32(d), readFixed32(d), readFixed32(d), readNested(d, COMMIT_EVIDENCE_ADAPTER),
            )
        },
    )

    private val REDEMPTION_ADAPTER = adapter<OfflineCashRedemptionVoucherV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            nestedField(e, REDEMPTION_STATEMENT_ADAPTER, v.statement)
            nestedField(e, COMMIT_CERTIFICATE_ADAPTER, v.commitCertificate)
            nestedField(e, COMMIT_WRAPPER_ADAPTER, v.proof)
            bytes32Field(e, v.artifactManifestDigest())
        },
        decode = { d ->
            OfflineCashRedemptionVoucherV1(
                readU16(d), readNested(d, REDEMPTION_STATEMENT_ADAPTER),
                readNested(d, COMMIT_CERTIFICATE_ADAPTER), readNested(d, COMMIT_WRAPPER_ADAPTER),
                readFixed32(d),
            )
        },
    )

    private fun <T> adapter(
        encode: (NoritoEncoder, T) -> Unit,
        decode: (NoritoDecoder) -> T,
    ): TypeAdapter<T> = object : TypeAdapter<T> {
        override fun encode(encoder: NoritoEncoder, value: T) = encode.invoke(encoder, value)
        override fun decode(decoder: NoritoDecoder): T = decode.invoke(decoder)
    }

    private fun <T> raw(value: T, schema: String, adapter: TypeAdapter<T>): ByteArray =
        encodeCanonical(value, schema, adapter, canonicalAlignment(schema))

    private fun canonicalAlignment(schema: String): Int = when (schema) {
        AGGREGATE_SCHEMA,
        REQUEST_MODE_SCHEMA,
        REQUEST_SCHEMA,
        INTENT_SCHEMA,
        MODEL + "OfflineCashAcceptanceIntentAuthorizationStatementV1",
        INTENT_AUTH_SCHEMA,
        NO_COMMIT_CLOSURE_STATEMENT_SCHEMA,
        NO_COMMIT_CLOSURE_SCHEMA,
        TICKET_SCHEMA,
        CREDIT_OPENING_SCHEMA,
        CREDIT_AAD_SCHEMA,
        STATEMENT_SCHEMA,
        PAYMENT_SCHEMA,
        MODEL + "OfflineCashMintAuthorizationContextV1",
        MODEL + "OfflineCashMintAuthorizationStatementV1",
        MINT_AUTH_SCHEMA,
        MINT_STATEMENT_SCHEMA,
        MINT_SCHEMA,
        REDEMPTION_STATEMENT_SCHEMA,
        REDEMPTION_SCHEMA,
        -> 16
        else -> 1
    }

    private fun frame(
        schema: String,
        alignment: Int = 1,
        write: (NoritoEncoder) -> Unit,
    ): ByteArray = encodeCanonical(
        Unit,
        schema,
        adapter(encode = { e, _ -> write(e) }, decode = { Unit }),
        alignment,
    )

    private fun <T> encodeCanonical(
        value: T,
        schema: String,
        adapter: TypeAdapter<T>,
        alignment: Int,
    ): ByteArray {
        require(alignment > 0 && alignment <= NoritoHeader.MAX_HEADER_PADDING && alignment and (alignment - 1) == 0)
        val archive = NoritoCodec.encode(value, schema, adapter)
        val padding = (alignment - NoritoHeader.HEADER_LENGTH % alignment) % alignment
        if (padding == 0) return archive
        return ByteArray(archive.size + padding).also { canonical ->
            archive.copyInto(canonical, endIndex = NoritoHeader.HEADER_LENGTH)
            archive.copyInto(
                canonical,
                destinationOffset = NoritoHeader.HEADER_LENGTH + padding,
                startIndex = NoritoHeader.HEADER_LENGTH,
            )
        }
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
        require(length <= parent.remaining().toLong()) { "truncated Offline Cash V1 field" }
        return NoritoDecoder(parent.readBytes(length.toInt()), parent.flags)
    }

    private fun <T> nestedField(parent: NoritoEncoder, adapter: TypeAdapter<T>, value: T) =
        field(parent) { adapter.encode(it, value) }

    private fun <T> readNested(parent: NoritoDecoder, adapter: TypeAdapter<T>): T {
        val child = readField(parent)
        val value = adapter.decode(child)
        require(child.remaining() == 0) { "trailing nested Offline Cash V1 bytes" }
        return value
    }

    private fun u16Field(e: NoritoEncoder, value: Int) = field(e) { it.writeUInt(value.toLong(), 16) }
    private fun u32Field(e: NoritoEncoder, value: Int) = field(e) { it.writeUInt(value.toLong(), 32) }
    private fun u64Field(e: NoritoEncoder, value: Long) = field(e) { it.writeUInt(value, 64) }
    private fun u128Field(e: NoritoEncoder, value: BigInteger) = field(e) { uint128(it, value) }
    private fun bytes32Field(e: NoritoEncoder, value: ByteArray) = field(e) { fixedArray(it, fixed32(value, "fixed32")) }
    private fun raw32Field(e: NoritoEncoder, value: ByteArray) = field(e) { fixedArray(it, raw32(value, "raw32")) }
    private fun networkField(e: NoritoEncoder, value: NetworkId) = field(e) { it.writeBytes(value.bytes()) }
    private fun assetField(e: NoritoEncoder, value: OfflineCashAssetDefinitionIdV1) = field(e) { it.writeBytes(value.canonicalPayload()) }
    private fun accountField(e: NoritoEncoder, value: OfflineCashAccountIdV1) = field(e) { it.writeBytes(value.canonicalPayload()) }
    private fun incarnationField(e: NoritoEncoder, value: OfflineCashAssetIncarnationV1) =
        field(e) { raw32Field(it, value.bytes()) }
    private fun publicKeyField(e: NoritoEncoder, value: OfflineCashDevicePublicKeyV1) = field(e) { it.writeBytes(value.sec1Bytes()) }
    private fun signatureField(e: NoritoEncoder, value: OfflineCashDeviceSignatureV1) = field(e) { it.writeBytes(value.rawBytes()) }
    private fun vectorField(e: NoritoEncoder, value: ByteArray) = field(e) { vector(it, value) }
    private fun enumUnitField(e: NoritoEncoder, ordinal: Int) = field(e) { it.writeUInt(ordinal.toLong(), 32) }

    private fun enumPayload(e: NoritoEncoder, ordinal: Int, payload: (NoritoEncoder) -> Unit) {
        e.writeUInt(ordinal.toLong(), 32)
        field(e, payload)
    }

    private fun <T> readEnumPayload(d: NoritoDecoder, decode: (NoritoDecoder) -> T): T {
        val child = readField(d)
        val value = decode(child)
        require(child.remaining() == 0)
        return value
    }

    private fun readEnumTag(d: NoritoDecoder, variants: Int): Int {
        val value = d.readUInt(32)
        require(value in 0 until variants.toLong())
        return value.toInt()
    }

    private fun readEnumUnit(d: NoritoDecoder, variants: Int): Int {
        val child = readField(d)
        val value = readEnumTag(child, variants)
        require(child.remaining() == 0)
        return value
    }

    private fun amountPolicyFields(e: NoritoEncoder, value: OfflineCashAmountPolicyV1) {
        u128Field(e, value.minimumAmount)
        u128Field(e, value.maximumAmount)
    }

    private fun readAmountPolicy(d: NoritoDecoder): OfflineCashAmountPolicyV1 =
        OfflineCashAmountPolicyV1(readU128(d), readU128(d))

    private fun fixedArray(encoder: NoritoEncoder, bytes: ByteArray) = encoder.writeBytes(bytes)

    private fun readRaw32(decoder: NoritoDecoder): ByteArray = readExactField(decoder, 32)
    private fun readFixed32(decoder: NoritoDecoder): ByteArray = fixed32(readRaw32(decoder), "fixed32")

    private fun readExactField(decoder: NoritoDecoder, width: Int): ByteArray {
        val child = readField(decoder)
        require(child.remaining() == width)
        return child.readBytes(width)
    }

    private fun vector(encoder: NoritoEncoder, bytes: ByteArray) {
        encoder.writeUInt(bytes.size.toLong(), 64)
        encoder.writeBytes(bytes)
    }

    private fun readVector(decoder: NoritoDecoder): ByteArray {
        val child = readField(decoder)
        val length = child.readUInt(64)
        require(length in 0..child.remaining().toLong())
        val result = child.readBytes(length.toInt())
        require(child.remaining() == 0)
        return result
    }

    private fun uint128(encoder: NoritoEncoder, value: BigInteger) {
        requireUnsigned128(value, "u128")
        val bigEndian = value.toByteArray()
        val source = if (bigEndian.size == 17 && bigEndian[0].toInt() == 0) {
            bigEndian.copyOfRange(1, bigEndian.size)
        } else {
            bigEndian
        }
        require(source.size <= 16)
        val littleEndian = ByteArray(16)
        source.reversedArray().copyInto(littleEndian)
        encoder.writeBytes(littleEndian)
    }

    private fun readU128(decoder: NoritoDecoder): BigInteger =
        BigInteger(1, readExactField(decoder, 16).reversedArray())

    private fun readU16(decoder: NoritoDecoder): Int {
        val child = readField(decoder)
        val value = child.readUInt(16).toInt()
        require(child.remaining() == 0)
        return value
    }

    private fun readU32(decoder: NoritoDecoder): Int {
        val child = readField(decoder)
        val value = child.readUInt(32)
        require(value <= Int.MAX_VALUE && child.remaining() == 0)
        return value.toInt()
    }

    private fun readU64(decoder: NoritoDecoder): Long {
        val child = readField(decoder)
        val value = child.readUInt(64)
        require(value >= 0 && child.remaining() == 0)
        return value
    }

    private fun readNetwork(decoder: NoritoDecoder): NetworkId =
        NetworkId.fromBytes(readExactField(decoder, 32))

    private fun readIncarnation(decoder: NoritoDecoder): OfflineCashAssetIncarnationV1 {
        val child = readField(decoder)
        val value = OfflineCashAssetIncarnationV1(readRaw32(child))
        require(child.remaining() == 0)
        return value
    }

    private fun readAsset(decoder: NoritoDecoder): OfflineCashAssetDefinitionIdV1 {
        val child = readField(decoder)
        return OfflineCashAssetDefinitionIdV1.fromCanonicalPayload(child.readBytes(child.remaining()))
    }

    private fun readAccount(decoder: NoritoDecoder): OfflineCashAccountIdV1 {
        val child = readField(decoder)
        return OfflineCashAccountIdV1.fromCanonicalPayload(child.readBytes(child.remaining()))
    }

    private fun readPublicKey(decoder: NoritoDecoder): OfflineCashDevicePublicKeyV1 =
        OfflineCashDevicePublicKeyV1(readExactField(decoder, 65))

    private fun readSignature(decoder: NoritoDecoder): OfflineCashDeviceSignatureV1 =
        OfflineCashDeviceSignatureV1(readExactField(decoder, 64))

    private fun <T> decodeExact(
        bytes: ByteArray,
        maximum: Int,
        schema: String,
        adapter: TypeAdapter<T>,
        encode: (T) -> ByteArray,
    ): T {
        require(bytes.isNotEmpty() && bytes.size <= maximum) { "Offline Cash V1 archive is empty or oversized" }
        val value = NoritoCodec.decode(bytes, adapter, schema)
        require(encode(value).contentEquals(bytes)) { "Offline Cash V1 archive is not canonical" }
        return value
    }

    private fun bounded(bytes: ByteArray, maximum: Int): ByteArray {
        require(bytes.size <= maximum) { "Offline Cash V1 archive exceeds $maximum bytes" }
        return bytes
    }

    private fun fixedTranscript(expectedSize: Int, vararg parts: ByteArray): ByteArray {
        val transcript = ByteArray(expectedSize)
        var offset = 0
        parts.forEach { part ->
            require(offset + part.size <= transcript.size) { "Offline Cash V1 circuit transcript overflow" }
            part.copyInto(transcript, offset)
            offset += part.size
        }
        require(offset == transcript.size) { "Offline Cash V1 circuit transcript width mismatch" }
        return transcript
    }

    private fun u16Le(value: Int): ByteArray =
        byteArrayOf(value.toByte(), (value ushr 8).toByte())

    private fun u32Le(value: Int): ByteArray =
        ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN).putInt(value).array()

    private fun u64Le(value: Long): ByteArray =
        ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(value).array()

    private fun u128Le(value: BigInteger): ByteArray {
        require(value.signum() >= 0 && value.bitLength() <= 128) { "value is outside the u128 domain" }
        val bigEndian = value.toByteArray()
        return ByteArray(16).also { littleEndian ->
            val width = minOf(littleEndian.size, bigEndian.size)
            repeat(width) { index -> littleEndian[index] = bigEndian[bigEndian.lastIndex - index] }
        }
    }

    private fun digestEncoded(domain: ByteArray, transcript: ByteArray): ByteArray =
        sha256(
            domain,
            byteArrayOf(0),
            ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(transcript.size.toLong()).array(),
            transcript,
        )

    private fun digestBytes(domain: ByteArray, bytes: ByteArray): ByteArray = digestEncoded(domain, bytes)

    private fun sha256(vararg parts: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").run {
        parts.forEach(::update)
        digest()
    }

    private fun ascii(value: String): ByteArray = value.toByteArray(StandardCharsets.US_ASCII)

    private fun textEnvelopeLength(rawBytes: Int): Int =
        OfflineCashWireV1.TEXT_PREFIX.length + (rawBytes / 3 * 4) + when (rawBytes % 3) {
            0 -> 0
            1 -> 2
            else -> 3
        }
}
