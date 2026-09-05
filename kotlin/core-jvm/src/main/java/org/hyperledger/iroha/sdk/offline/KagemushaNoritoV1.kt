// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import java.util.Optional
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/**
 * Exact canonical codec and structural validation boundary for KAGEMUSHA V1.
 *
 * Every method is deliberately named `Shape`: canonical decoding, digest binding, and field
 * consistency grant no monetary authority. Production signature, recursive-proof, release,
 * credential, X25519, HKDF, and AEAD operations must execute in the shared native core behind a
 * qualified non-forking hardware service.
 */
object KagemushaNoritoV1 {
    /** Maximum canonical bytes for the request embedded in `TopUpKagemushaV1`. */
    const val MAXIMUM_TOP_UP_REQUEST_BYTES: Int = 16 * 1024

    private const val MODEL = "iroha_data_model::kagemusha::kagemusha_v1::"
    private const val AGGREGATE_SCHEMA = MODEL + "KagemushaAggregateStateCommitmentV1"
    private const val PASTA_STATE_SCHEMA = MODEL + "KagemushaPastaStateCommitmentV1"
    private const val PROOF_SCHEMA = MODEL + "KagemushaPairedProofV1"
    private const val HARDWARE_PROFILE_SCHEMA = MODEL + "KagemushaHardwareProfileV1"
    private const val HARDWARE_CREDENTIAL_SCHEMA = MODEL + "KagemushaHardwareCredentialV1"
    private const val REQUEST_SCHEMA = MODEL + "KagemushaPaymentRequestV1"
    private const val PEER_CREDIT_CONTEXT_SCHEMA = MODEL + "KagemushaPeerCreditContextV1"
    private const val CREDIT_OPENING_SCHEMA = MODEL + "KagemushaCreditOpeningV1"
    private const val CREDIT_AAD_SCHEMA = MODEL + "KagemushaEncryptedCreditAadV1"
    private const val CREDIT_ENVELOPE_SCHEMA = MODEL + "KagemushaEncryptedCreditEnvelopeV1"
    private const val LIFECYCLE_SCHEMA = MODEL + "KagemushaLifecycleBindingV1"
    private const val COMMIT_CERTIFICATE_SCHEMA = MODEL + "KagemushaCommitCertificateV1"
    private const val PAYMENT_PROOF_SCHEMA = MODEL + "KagemushaPaymentProofV1"
    private const val REDEMPTION_PROOF_SCHEMA = MODEL + "KagemushaRedemptionProofV1"
    private const val PAYMENT_OUTPUT_SCHEMA = MODEL + "KagemushaPaymentOutputV1"
    private const val PAYMENT_SCHEMA = MODEL + "KagemushaPaymentV1"
    private const val ACK_SCHEMA = MODEL + "KagemushaAcknowledgementV1"
    private const val MINT_AUTH_SCHEMA = MODEL + "KagemushaMintAuthorizationV1"
    private const val MINT_STATEMENT_SCHEMA = MODEL + "KagemushaMintCreditStatementV1"
    private const val MINT_SCHEMA = MODEL + "KagemushaMintCreditV1"
    private const val DEVICE_MODEL =
        "iroha_data_model::kagemusha::kagemusha_device_v1::"
    private const val DEVICE_MINT_STAGE_COMMAND_SCHEMA =
        DEVICE_MODEL + "KagemushaDeviceMintStageCommandV1"
    private const val DEVICE_MINT_STAGE_RESULT_SCHEMA =
        DEVICE_MODEL + "KagemushaDeviceMintStageResultV1"
    private const val REDEMPTION_STATEMENT_SCHEMA = MODEL + "KagemushaRedemptionStatementV1"
    private const val REDEMPTION_SCHEMA = MODEL + "KagemushaRedemptionVoucherV1"
    private const val TOP_UP_REQUEST_SCHEMA = "iroha.torii.v1.kagemusha.top_up.request"
    private const val TOP_UP_INSTRUCTION_SCHEMA =
        "iroha_data_model::isi::kagemusha_v1::TopUpKagemushaV1"
    private const val REDEMPTION_REQUEST_SCHEMA = "iroha.torii.v1.kagemusha.redeem.request"
    private const val MAXIMUM_TOP_UP_INSTRUCTION_PAYLOAD_BYTES =
        MAXIMUM_TOP_UP_REQUEST_BYTES + 1024

    /** Maximum canonical bytes in the public body of secure-device operation 16. */
    const val MAXIMUM_DEVICE_MINT_STAGE_COMMAND_BYTES: Int = 64 * 1024

    /** Maximum canonical bytes in the fixed public result of secure-device operation 16. */
    const val MAXIMUM_DEVICE_MINT_STAGE_RESULT_BYTES: Int = 128

    private val DEVICE_KEY_REFERENCE_DOMAIN = ascii("iroha:kagemusha:v1:device-key-reference")
    private val PASTA_STATE_COMMITMENT_DOMAIN = ascii("iroha:kagemusha:v1:pasta-state-commitment")
    private val LIABILITY_POOL_DOMAIN = ascii("iroha:kagemusha:v1:liability-pool")
    private val REQUEST_DIGEST_DOMAIN = ascii("iroha:kagemusha:v1:payment-request")
    private val PREPARED_TRANSFER_DIGEST_DOMAIN = ascii("iroha:kagemusha:v1:prepared-transfer")
    private val LIFECYCLE_DIGEST_DOMAIN = ascii("iroha:kagemusha:v1:lifecycle-binding")
    private val CIPHERTEXT_DIGEST_DOMAIN = ascii("iroha:kagemusha:v1:ciphertext")
    private val PEER_CREDIT_CONTEXT_DIGEST_DOMAIN = ascii("iroha:kagemusha:v1:peer-credit-context")
    private val CREDIT_ID_DOMAIN = ascii("iroha:kagemusha:v1:credit-id")
    private val PEER_CREDIT_OPENING_COMMITMENT_DOMAIN =
        ascii("iroha:kagemusha:v1:peer-credit-opening-commitment")
    private val STATEMENT_DIGEST_DOMAIN = ascii("iroha:kagemusha:v1:send-split-statement")
    private val PAYMENT_DIGEST_DOMAIN = ascii("iroha:kagemusha:v1:payment")
    private val MINT_AUTH_CONTEXT_DIGEST_DOMAIN = ascii("iroha:kagemusha:v1:mint-authorization-context")
    private val MINT_AUTH_STATEMENT_DIGEST_DOMAIN = ascii("iroha:kagemusha:v1:mint-authorization-statement")
    private val MINT_AUTH_DIGEST_DOMAIN = ascii("iroha:kagemusha:v1:mint-authorization")
    private val MINT_CREDIT_ID_DOMAIN = ascii("iroha:kagemusha:v1:mint-credit-id")
    private val MINT_LIFECYCLE_CONTEXT_DOMAIN = ascii("iroha:kagemusha:v1:mint-lifecycle-context")
    private val MINT_STATEMENT_DIGEST_DOMAIN = ascii("iroha:kagemusha:v1:mint-statement")
    private val REDEMPTION_ID_DOMAIN = ascii("iroha:kagemusha:v1:redemption-id")
    private val REDEMPTION_STATEMENT_DIGEST_DOMAIN = ascii("iroha:kagemusha:v1:redemption-statement")
    private val COMMIT_CERTIFICATE_ID_DOMAIN = ascii("iroha:kagemusha:v1:commit-certificate-id")
    private val COMMIT_CERTIFICATE_DIGEST_DOMAIN = ascii("iroha:kagemusha:v1:commit-certificate")
    private val ENCRYPTED_CREDIT_SALT_LABEL = ascii("iroha:kagemusha:v1:credit-envelope-salt\u0000")
    private val ENCRYPTED_CREDIT_INFO_LABEL = ascii("iroha:kagemusha:v1:credit-envelope-key\u0000")

    /** Encode exact bounded aggregate-state metadata after shape checks. */
    @JvmStatic
    fun encodeAggregateStateShape(value: KagemushaAggregateStateCommitmentV1): ByteArray {
        validateAggregateStateShape(value)
        return bounded(raw(value, AGGREGATE_SCHEMA, AGGREGATE_ADAPTER), KagemushaWireV1.MAXIMUM_AGGREGATE_STATE_BYTES)
    }

    /** Decode exact canonical bounded aggregate-state metadata after shape checks. */
    @JvmStatic
    fun decodeAggregateStateShapeExact(bytes: ByteArray): KagemushaAggregateStateCommitmentV1 =
        decodeExact(
            bytes,
            KagemushaWireV1.MAXIMUM_AGGREGATE_STATE_BYTES,
            AGGREGATE_SCHEMA,
            AGGREGATE_ADAPTER,
            ::encodeAggregateStateShape,
        )

    /** Encode a hardware profile for authenticated release transport. */
    @JvmStatic
    fun encodeHardwareProfileShape(value: KagemushaHardwareProfileV1): ByteArray =
        bounded(raw(value, HARDWARE_PROFILE_SCHEMA, HARDWARE_PROFILE_ADAPTER), 512)

    /** Decode an exact canonical hardware profile without authenticating its governance state. */
    @JvmStatic
    fun decodeHardwareProfileShapeExact(bytes: ByteArray): KagemushaHardwareProfileV1 =
        decodeExact(bytes, 512, HARDWARE_PROFILE_SCHEMA, HARDWARE_PROFILE_ADAPTER, ::encodeHardwareProfileShape)

    /** Encode a compact hardware credential without granting it authority. */
    @JvmStatic
    fun encodeHardwareCredentialShape(value: KagemushaHardwareCredentialV1): ByteArray =
        bounded(raw(value, HARDWARE_CREDENTIAL_SCHEMA, HARDWARE_CREDENTIAL_ADAPTER), 768)

    /** Decode a compact hardware credential without authenticating its governance signature. */
    @JvmStatic
    fun decodeHardwareCredentialShapeExact(bytes: ByteArray): KagemushaHardwareCredentialV1 =
        decodeExact(bytes, 768, HARDWARE_CREDENTIAL_SCHEMA, HARDWARE_CREDENTIAL_ADAPTER, ::encodeHardwareCredentialShape)

    /** Encode one terminal hardware certificate. */
    @JvmStatic
    fun encodeCommitCertificateShape(value: KagemushaCommitCertificateV1): ByteArray {
        require(value.certificateId().contentEquals(commitCertificateId(value))) {
            "KAGEMUSHA V1 certificate identity does not match its canonical body"
        }
        return bounded(raw(value, COMMIT_CERTIFICATE_SCHEMA, COMMIT_CERTIFICATE_ADAPTER), KagemushaWireV1.MAXIMUM_COMMIT_CERTIFICATE_BYTES)
    }

    /** Decode one exact bounded terminal hardware certificate. */
    @JvmStatic
    fun decodeCommitCertificateShapeExact(bytes: ByteArray): KagemushaCommitCertificateV1 =
        decodeExact(
            bytes,
            KagemushaWireV1.MAXIMUM_COMMIT_CERTIFICATE_BYTES,
            COMMIT_CERTIFICATE_SCHEMA,
            COMMIT_CERTIFICATE_ADAPTER,
            ::encodeCommitCertificateShape,
        )

    /** Encode one final paired proof authorizing an offline payment. */
    @JvmStatic
    fun encodePaymentProofShape(value: KagemushaPaymentProofV1): ByteArray =
        bounded(raw(value, PAYMENT_PROOF_SCHEMA, PAYMENT_PROOF_ADAPTER), KagemushaWireV1.MAXIMUM_PAIRED_PROOF_BYTES)

    /** Decode one exact bounded final payment proof. */
    @JvmStatic
    fun decodePaymentProofShapeExact(bytes: ByteArray): KagemushaPaymentProofV1 =
        decodeExact(
            bytes,
            KagemushaWireV1.MAXIMUM_PAIRED_PROOF_BYTES,
            PAYMENT_PROOF_SCHEMA,
            PAYMENT_PROOF_ADAPTER,
            ::encodePaymentProofShape,
        )

    /** Encode one final paired proof authorizing an online redemption. */
    @JvmStatic
    fun encodeRedemptionProofShape(value: KagemushaRedemptionProofV1): ByteArray =
        bounded(raw(value, REDEMPTION_PROOF_SCHEMA, REDEMPTION_PROOF_ADAPTER), KagemushaWireV1.MAXIMUM_REDEMPTION_PROOF_BYTES)

    /** Decode one exact bounded final redemption proof. */
    @JvmStatic
    fun decodeRedemptionProofShapeExact(bytes: ByteArray): KagemushaRedemptionProofV1 =
        decodeExact(
            bytes,
            KagemushaWireV1.MAXIMUM_REDEMPTION_PROOF_BYTES,
            REDEMPTION_PROOF_SCHEMA,
            REDEMPTION_PROOF_ADAPTER,
            ::encodeRedemptionProofShape,
        )

    /** Encode a signed request after shape and self-consistency checks only. */
    @JvmStatic
    fun encodePaymentRequestShape(value: KagemushaPaymentRequestV1): ByteArray {
        validatePaymentRequestShape(value)
        return bounded(raw(value, REQUEST_SCHEMA, REQUEST_ADAPTER), KagemushaWireV1.MAXIMUM_PAYMENT_REQUEST_BYTES)
    }

    /** Decode one exact canonical request without verifying its signature or credential. */
    @JvmStatic
    fun decodePaymentRequestShapeExact(bytes: ByteArray): KagemushaPaymentRequestV1 =
        decodeExact(
            bytes,
            KagemushaWireV1.MAXIMUM_PAYMENT_REQUEST_BYTES,
            REQUEST_SCHEMA,
            REQUEST_ADAPTER,
            ::encodePaymentRequestShape,
        )

    /** Encode a request as the sole `kgm1:` text transport. */
    @JvmStatic
    fun encodePaymentRequestTextShape(value: KagemushaPaymentRequestV1): String =
        KagemushaWireV1.encodeText(KagemushaWirePayloadKindV1.PAYMENT_REQUEST, encodePaymentRequestShape(value))

    /** Decode one exact `kgm1:` request without granting it authority. */
    @JvmStatic
    fun decodePaymentRequestTextShapeExact(text: String): KagemushaPaymentRequestV1 =
        decodePaymentRequestShapeExact(KagemushaWireV1.decodeText(KagemushaWirePayloadKindV1.PAYMENT_REQUEST, text))

    /** Encode the exact pre-ID peer context carried by encrypted-credit AAD. */
    @JvmStatic
    fun encodePeerCreditContextShape(value: KagemushaPeerCreditContextV1): ByteArray =
        raw(value, PEER_CREDIT_CONTEXT_SCHEMA, PEER_CREDIT_CONTEXT_ADAPTER)

    /** Decode one exact pre-ID peer context without opening an encrypted credit. */
    @JvmStatic
    fun decodePeerCreditContextShapeExact(bytes: ByteArray): KagemushaPeerCreditContextV1 =
        decodeExact(
            bytes,
            512,
            PEER_CREDIT_CONTEXT_SCHEMA,
            PEER_CREDIT_CONTEXT_ADAPTER,
            ::encodePeerCreditContextShape,
        )

    /** Build the acyclic peer context from the exact request and payment output. */
    @JvmStatic
    fun peerCreditContextShape(
        request: KagemushaPaymentRequestV1,
        output: KagemushaPaymentOutputV1,
    ): KagemushaPeerCreditContextV1 {
        validatePaymentOutputShape(output, request)
        return KagemushaPeerCreditContextV1(
            KagemushaWireV1.WIRE_VERSION,
            paymentRequestDigest(request),
            output.amount,
            output.senderBeforeCommitment(),
            output.senderAfterCommitment(),
            preparedTransferDigest(
                request,
                output.senderBeforeCommitment(),
                output.senderAfterCommitment(),
                output.transitionNullifier(),
                output.ciphertextCommitment(),
            ),
            request.recipientEncryptionKey,
        )
    }

    /** Return the canonical digest placed in peer-credit associated data. */
    @JvmStatic
    fun peerCreditContextDigestShape(value: KagemushaPeerCreditContextV1): ByteArray =
        digestEncoded(
            PEER_CREDIT_CONTEXT_DIGEST_DOMAIN,
            encodePeerCreditContextShape(value),
        )

    /** Construct the exact typed AAD for a receiver-bound peer credit. */
    @JvmStatic
    fun encryptedCreditAadForPeerShape(
        output: KagemushaPaymentOutputV1,
        request: KagemushaPaymentRequestV1,
    ): KagemushaEncryptedCreditAadV1 {
        validatePaymentOutputShape(output, request)
        val context = peerCreditContextShape(request, output)
        return KagemushaEncryptedCreditAadV1(
            KagemushaWireV1.WIRE_VERSION,
            KagemushaEncryptedCreditPurposeV1.PEER,
            peerCreditContextDigestShape(context),
            output.ciphertextCommitment(),
            output.creditId(),
            output.amount,
        )
    }

    /** Construct the exact typed AAD authorized before a reserve-backed mint debit. */
    @JvmStatic
    fun encryptedCreditAadForMintShape(
        statement: KagemushaMintAuthorizationStatementV1,
    ): KagemushaEncryptedCreditAadV1 {
        validateMintAuthorizationStatementShape(statement)
        return KagemushaEncryptedCreditAadV1(
            KagemushaWireV1.WIRE_VERSION,
            KagemushaEncryptedCreditPurposeV1.MINT,
            mintAuthorizationContextDigest(statement.context),
            statement.issuanceCommitment(),
            statement.creditId(),
            statement.context.amount,
        )
    }

    /** Encode a payment after exact request, statement, recursive-proof, and envelope shape checks. */
    @JvmStatic
    fun encodePaymentShape(
        value: KagemushaPaymentV1,
        request: KagemushaPaymentRequestV1,
    ): ByteArray {
        validatePaymentShape(value, request)
        return bounded(raw(value, PAYMENT_SCHEMA, PAYMENT_ADAPTER), KagemushaWireV1.MAXIMUM_PAYMENT_BYTES)
    }

    /** Decode a payment and validate all non-cryptographic bindings against its exact request. */
    @JvmStatic
    fun decodePaymentShapeExact(
        bytes: ByteArray,
        request: KagemushaPaymentRequestV1,
    ): KagemushaPaymentV1 = decodeExact(
        bytes,
        KagemushaWireV1.MAXIMUM_PAYMENT_BYTES,
        PAYMENT_SCHEMA,
        PAYMENT_ADAPTER,
    ) { encodePaymentShape(it, request) }

    /** Encode a payment as strict `kgm1:` text after shape checks. */
    @JvmStatic
    fun encodePaymentTextShape(
        value: KagemushaPaymentV1,
        request: KagemushaPaymentRequestV1,
    ): String = KagemushaWireV1.encodeText(
        KagemushaWirePayloadKindV1.PAYMENT,
        encodePaymentShape(value, request),
    )

    /** Decode strict `kgm1:` payment text without granting monetary authority. */
    @JvmStatic
    fun decodePaymentTextShapeExact(
        text: String,
        request: KagemushaPaymentRequestV1,
    ): KagemushaPaymentV1 = decodePaymentShapeExact(
        KagemushaWireV1.decodeText(KagemushaWirePayloadKindV1.PAYMENT, text),
        request,
    )

    /** Encode an acknowledgement after exact structural binding checks. */
    @JvmStatic
    fun encodeAcknowledgementShape(
        value: KagemushaAcknowledgementV1,
        request: KagemushaPaymentRequestV1,
        payment: KagemushaPaymentV1,
    ): ByteArray {
        validateAcknowledgementShape(value, request, payment)
        return bounded(raw(value, ACK_SCHEMA, ACK_ADAPTER), KagemushaWireV1.MAXIMUM_ACKNOWLEDGEMENT_BYTES)
    }

    /** Decode an acknowledgement without verifying its receiver signature. */
    @JvmStatic
    fun decodeAcknowledgementShapeExact(
        bytes: ByteArray,
        request: KagemushaPaymentRequestV1,
        payment: KagemushaPaymentV1,
    ): KagemushaAcknowledgementV1 = decodeExact(
        bytes,
        KagemushaWireV1.MAXIMUM_ACKNOWLEDGEMENT_BYTES,
        ACK_SCHEMA,
        ACK_ADAPTER,
    ) { encodeAcknowledgementShape(it, request, payment) }

    /** Encode one durable-inbox acknowledgement as strict `kgm1:` text. */
    @JvmStatic
    fun encodeAcknowledgementTextShape(
        value: KagemushaAcknowledgementV1,
        request: KagemushaPaymentRequestV1,
        payment: KagemushaPaymentV1,
    ): String = KagemushaWireV1.encodeText(
        KagemushaWirePayloadKindV1.ACKNOWLEDGEMENT,
        encodeAcknowledgementShape(value, request, payment),
    )

    /** Decode strict acknowledgement text without authenticating its signature. */
    @JvmStatic
    fun decodeAcknowledgementTextShapeExact(
        text: String,
        request: KagemushaPaymentRequestV1,
        payment: KagemushaPaymentV1,
    ): KagemushaAcknowledgementV1 = decodeAcknowledgementShapeExact(
        KagemushaWireV1.decodeText(KagemushaWirePayloadKindV1.ACKNOWLEDGEMENT, text),
        request,
        payment,
    )

    /** Encode a pre-debit mint authorization after shape checks only. */
    @JvmStatic
    fun encodeMintAuthorizationShape(value: KagemushaMintAuthorizationV1): ByteArray {
        validateMintAuthorizationShape(value)
        return bounded(raw(value, MINT_AUTH_SCHEMA, MINT_AUTH_ADAPTER), KagemushaWireV1.MAXIMUM_MINT_AUTHORIZATION_BYTES)
    }

    /** Decode one exact pre-debit mint authorization without verifying either proof parity. */
    @JvmStatic
    fun decodeMintAuthorizationShapeExact(bytes: ByteArray): KagemushaMintAuthorizationV1 =
        decodeExact(
            bytes,
            KagemushaWireV1.MAXIMUM_MINT_AUTHORIZATION_BYTES,
            MINT_AUTH_SCHEMA,
            MINT_AUTH_ADAPTER,
            ::encodeMintAuthorizationShape,
        )

    /** Encode one mint authorization as strict `kgm1:` text after shape checks. */
    @JvmStatic
    fun encodeMintAuthorizationTextShape(value: KagemushaMintAuthorizationV1): String =
        KagemushaWireV1.encodeText(
            KagemushaWirePayloadKindV1.MINT_AUTHORIZATION,
            encodeMintAuthorizationShape(value),
        )

    /** Decode one exact mint authorization text envelope without granting authority. */
    @JvmStatic
    fun decodeMintAuthorizationTextShapeExact(text: String): KagemushaMintAuthorizationV1 =
        decodeMintAuthorizationShapeExact(
            KagemushaWireV1.decodeText(KagemushaWirePayloadKindV1.MINT_AUTHORIZATION, text),
        )

    /** Encode one finalized mint credit after standalone shape checks. */
    @JvmStatic
    fun encodeMintCreditShape(value: KagemushaMintCreditV1): ByteArray {
        validateMintCreditShape(value)
        return bounded(raw(value, MINT_SCHEMA, MINT_ADAPTER), KagemushaWireV1.MAXIMUM_MINT_CREDIT_BYTES)
    }

    /** Decode one exact standalone mint credit without granting monetary authority. */
    @JvmStatic
    fun decodeMintCreditShapeExact(bytes: ByteArray): KagemushaMintCreditV1 =
        decodeExact(
            bytes,
            KagemushaWireV1.MAXIMUM_MINT_CREDIT_BYTES,
            MINT_SCHEMA,
            MINT_ADAPTER,
            ::encodeMintCreditShape,
        )

    /** Encode one finalized mint credit as strict `kgm1:` text after shape checks. */
    @JvmStatic
    fun encodeMintCreditTextShape(value: KagemushaMintCreditV1): String =
        KagemushaWireV1.encodeText(
            KagemushaWirePayloadKindV1.MINT_CREDIT,
            encodeMintCreditShape(value),
        )

    /** Decode one exact standalone mint credit text envelope without granting authority. */
    @JvmStatic
    fun decodeMintCreditTextShapeExact(text: String): KagemushaMintCreditV1 =
        decodeMintCreditShapeExact(
            KagemushaWireV1.decodeText(KagemushaWirePayloadKindV1.MINT_CREDIT, text),
        )

    /** Encode one finalized mint credit against its exact pre-debit authorization. */
    @JvmStatic
    fun encodeMintCreditShape(
        value: KagemushaMintCreditV1,
        authorization: KagemushaMintAuthorizationV1,
    ): ByteArray {
        validateMintCreditShapeAgainstAuthorization(value, authorization)
        return encodeMintCreditShape(value)
    }

    /** Decode a mint credit against its exact authorization without granting authority. */
    @JvmStatic
    fun decodeMintCreditShapeExact(
        bytes: ByteArray,
        authorization: KagemushaMintAuthorizationV1,
    ): KagemushaMintCreditV1 = decodeExact(
        bytes,
        KagemushaWireV1.MAXIMUM_MINT_CREDIT_BYTES,
        MINT_SCHEMA,
        MINT_ADAPTER,
    ) { encodeMintCreditShape(it, authorization) }

    /** Encode a canonical operation-16 body after validating both nested public archives. */
    @JvmStatic
    fun encodeDeviceMintStageCommandShape(
        value: KagemushaDeviceMintStageCommandV1,
    ): ByteArray {
        validateDeviceMintStageCommandShape(value)
        return bounded(
            raw(value, DEVICE_MINT_STAGE_COMMAND_SCHEMA, DEVICE_MINT_STAGE_COMMAND_ADAPTER),
            MAXIMUM_DEVICE_MINT_STAGE_COMMAND_BYTES,
        )
    }

    /** Build and encode a canonical operation-16 body from exact nested public archives. */
    @JvmStatic
    fun encodeDeviceMintStageCommandShape(
        canonicalAuthorization: ByteArray,
        canonicalMintCredit: ByteArray,
    ): ByteArray = encodeDeviceMintStageCommandShape(
        KagemushaDeviceMintStageCommandV1(
            KagemushaWireV1.WIRE_VERSION,
            canonicalAuthorization,
            canonicalMintCredit,
        ),
    )

    /** Decode one exact operation-16 body without granting staging or monetary authority. */
    @JvmStatic
    fun decodeDeviceMintStageCommandShapeExact(
        bytes: ByteArray,
    ): KagemushaDeviceMintStageCommandV1 = decodeExact(
        bytes,
        MAXIMUM_DEVICE_MINT_STAGE_COMMAND_BYTES,
        DEVICE_MINT_STAGE_COMMAND_SCHEMA,
        DEVICE_MINT_STAGE_COMMAND_ADAPTER,
        ::encodeDeviceMintStageCommandShape,
    )

    /** Encode the fixed public operation-16 result. */
    @JvmStatic
    fun encodeDeviceMintStageResultShape(
        value: KagemushaDeviceMintStageResultV1,
    ): ByteArray = bounded(
        raw(value, DEVICE_MINT_STAGE_RESULT_SCHEMA, DEVICE_MINT_STAGE_RESULT_ADAPTER),
        MAXIMUM_DEVICE_MINT_STAGE_RESULT_BYTES,
    )

    /** Decode one exact public operation-16 result. */
    @JvmStatic
    fun decodeDeviceMintStageResultShapeExact(
        bytes: ByteArray,
    ): KagemushaDeviceMintStageResultV1 = decodeExact(
        bytes,
        MAXIMUM_DEVICE_MINT_STAGE_RESULT_BYTES,
        DEVICE_MINT_STAGE_RESULT_SCHEMA,
        DEVICE_MINT_STAGE_RESULT_ADAPTER,
        ::encodeDeviceMintStageResultShape,
    )

    /** Decode and bind an operation-16 result to the exact command credit identity. */
    @JvmStatic
    fun decodeDeviceMintStageResultShapeExact(
        bytes: ByteArray,
        command: KagemushaDeviceMintStageCommandV1,
    ): KagemushaDeviceMintStageResultV1 {
        val result = decodeDeviceMintStageResultShapeExact(bytes)
        val credit = validateDeviceMintStageCommandShape(command)
        require(result.creditId().contentEquals(credit.statement.lifecycle.creditId())) {
            "KAGEMUSHA V1 mint-stage result substituted the credit identity"
        }
        return result
    }

    /** Encode one terminal redemption voucher after direct-proof shape checks. */
    @JvmStatic
    fun encodeRedemptionVoucherShape(value: KagemushaRedemptionVoucherV1): ByteArray {
        validateRedemptionVoucherShape(value)
        return bounded(raw(value, REDEMPTION_SCHEMA, REDEMPTION_ADAPTER), KagemushaWireV1.MAXIMUM_REDEMPTION_VOUCHER_BYTES)
    }

    /** Decode one exact terminal redemption voucher without granting authority. */
    @JvmStatic
    fun decodeRedemptionVoucherShapeExact(bytes: ByteArray): KagemushaRedemptionVoucherV1 =
        decodeExact(
            bytes,
            KagemushaWireV1.MAXIMUM_REDEMPTION_VOUCHER_BYTES,
            REDEMPTION_SCHEMA,
            REDEMPTION_ADAPTER,
            ::encodeRedemptionVoucherShape,
        )

    /** Encode one terminal redemption voucher as strict `kgm1:` text after shape checks. */
    @JvmStatic
    fun encodeRedemptionVoucherTextShape(value: KagemushaRedemptionVoucherV1): String =
        KagemushaWireV1.encodeText(
            KagemushaWirePayloadKindV1.REDEMPTION_VOUCHER,
            encodeRedemptionVoucherShape(value),
        )

    /** Decode one exact redemption text envelope without granting authority. */
    @JvmStatic
    fun decodeRedemptionVoucherTextShapeExact(text: String): KagemushaRedemptionVoucherV1 =
        decodeRedemptionVoucherShapeExact(
            KagemushaWireV1.decodeText(KagemushaWirePayloadKindV1.REDEMPTION_VOUCHER, text),
        )

    /** Encode one exact reserve-backed request for a `TopUpKagemushaV1` instruction. */
    @JvmStatic
    fun encodeTopUpRequestShape(value: KagemushaTopUpRequestV1): ByteArray {
        validateTopUpRequestShape(value)
        return bounded(
            raw(value, TOP_UP_REQUEST_SCHEMA, TOP_UP_REQUEST_ADAPTER),
            MAXIMUM_TOP_UP_REQUEST_BYTES,
        )
    }

    /** Decode one exact top-up request without granting the embedded proof monetary authority. */
    @JvmStatic
    fun decodeTopUpRequestShapeExact(bytes: ByteArray): KagemushaTopUpRequestV1 =
        decodeExact(
            bytes,
            MAXIMUM_TOP_UP_REQUEST_BYTES,
            TOP_UP_REQUEST_SCHEMA,
            TOP_UP_REQUEST_ADAPTER,
            ::encodeTopUpRequestShape,
        )

    /** Encode the concrete registered payload for one native `TopUpKagemushaV1` instruction. */
    @JvmStatic
    fun encodeTopUpInstructionPayloadShape(value: KagemushaTopUpRequestV1): ByteArray {
        encodeTopUpRequestShape(value)
        return bounded(
            frame(TOP_UP_INSTRUCTION_SCHEMA, alignment = 16) { encoder ->
                nestedField(encoder, TOP_UP_REQUEST_ADAPTER, value)
            },
            MAXIMUM_TOP_UP_INSTRUCTION_PAYLOAD_BYTES,
        )
    }

    /** Encode one exact full or partial redemption request for the generic Torii route. */
    @JvmStatic
    fun encodeRedemptionRequestShape(value: KagemushaRedemptionRequestV1): ByteArray {
        validateRedemptionRequestShape(value)
        return bounded(raw(value, REDEMPTION_REQUEST_SCHEMA, REDEMPTION_REQUEST_ADAPTER), 8 * 1024)
    }

    /** Decode one exact redemption request without granting the voucher monetary authority. */
    @JvmStatic
    fun decodeRedemptionRequestShapeExact(bytes: ByteArray): KagemushaRedemptionRequestV1 =
        decodeExact(
            bytes,
            8 * 1024,
            REDEMPTION_REQUEST_SCHEMA,
            REDEMPTION_REQUEST_ADAPTER,
            ::encodeRedemptionRequestShape,
        )

    /** Encode the exact recipient-only credit-opening plaintext. */
    @JvmStatic
    fun encodeCreditOpeningShape(value: KagemushaCreditOpeningV1): ByteArray {
        val canonical = bounded(
            raw(value, CREDIT_OPENING_SCHEMA, CREDIT_OPENING_ADAPTER),
            KagemushaWireV1.MAXIMUM_CREDIT_OPENING_BYTES,
        )
        require(canonical.size == KagemushaWireV1.CREDIT_OPENING_CANONICAL_BYTES)
        return canonical
    }

    /** Decode an exact canonical credit opening after authenticated decryption in native core. */
    @JvmStatic
    fun decodeCreditOpeningShapeExact(bytes: ByteArray): KagemushaCreditOpeningV1 {
        require(bytes.size == KagemushaWireV1.CREDIT_OPENING_CANONICAL_BYTES)
        return decodeExact(
            bytes,
            KagemushaWireV1.MAXIMUM_CREDIT_OPENING_BYTES,
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
    ): KagemushaCreditOpeningV1 {
        val value = decodeCreditOpeningShapeExact(bytes)
        require(value.creditId().contentEquals(fixed32(creditId, "creditId")))
        require(value.amount == amount)
        return value
    }

    /** Encode canonical encrypted-credit associated data. */
    @JvmStatic
    fun encodeEncryptedCreditAadShape(value: KagemushaEncryptedCreditAadV1): ByteArray =
        raw(value, CREDIT_AAD_SCHEMA, CREDIT_AAD_ADAPTER)

    /** Decode canonical encrypted-credit associated data. */
    @JvmStatic
    fun decodeEncryptedCreditAadShapeExact(bytes: ByteArray): KagemushaEncryptedCreditAadV1 =
        decodeExact(bytes, 512, CREDIT_AAD_SCHEMA, CREDIT_AAD_ADAPTER, ::encodeEncryptedCreditAadShape)

    /** Encode the exact X25519/XChaCha recipient envelope. */
    @JvmStatic
    fun encodeEncryptedCreditEnvelopeShape(value: KagemushaEncryptedCreditEnvelopeV1): ByteArray =
        bounded(raw(value, CREDIT_ENVELOPE_SCHEMA, CREDIT_ENVELOPE_ADAPTER), KagemushaWireV1.MAXIMUM_ENCRYPTED_CREDIT_BYTES)

    /**
     * Decode one exact envelope and enforce its canonical nonzero 32-byte X25519 wire shape.
     * Native object/exchange validation authenticates the element before monetary use.
     */
    @JvmStatic
    fun decodeEncryptedCreditEnvelopeShapeExact(bytes: ByteArray): KagemushaEncryptedCreditEnvelopeV1 =
        decodeExact(
            bytes,
            KagemushaWireV1.MAXIMUM_ENCRYPTED_CREDIT_BYTES,
            CREDIT_ENVELOPE_SCHEMA,
            CREDIT_ENVELOPE_ADAPTER,
            ::encodeEncryptedCreditEnvelopeShape,
        )

    /** Derive the specified HKDF-SHA256 salt; no private key operation occurs on the JVM. */
    @JvmStatic
    fun encryptedCreditKdfSalt(
        recipientPublicKey: KagemushaX25519PublicKeyV1,
        ephemeralPublicKey: KagemushaX25519PublicKeyV1,
    ): ByteArray = sha256(ENCRYPTED_CREDIT_SALT_LABEL, recipientPublicKey.bytes(), ephemeralPublicKey.bytes())

    /** Derive the specified HKDF-SHA256 info; AEAD key derivation remains native-only. */
    @JvmStatic
    fun encryptedCreditKdfInfo(aad: KagemushaEncryptedCreditAadV1): ByteArray =
        ENCRYPTED_CREDIT_INFO_LABEL + sha256(encodeEncryptedCreditAadShape(aad))

    /** Hash one public Pasta pair into the aggregate state's compact outer head. */
    @JvmStatic
    fun pastaStateCommitment(value: KagemushaPastaStateCommitmentV1): ByteArray =
        sha256(PASTA_STATE_COMMITMENT_DOMAIN, byteArrayOf(0), value.eq(), value.ep())

    /** Derive the exact normalized device-key reference. */
    @JvmStatic
    fun deviceKeyReference(publicKey: KagemushaDevicePublicKeyV1): ByteArray =
        sha256(DEVICE_KEY_REFERENCE_DOMAIN, byteArrayOf(0), publicKey.sec1Bytes())

    /** Derive the sole pooled reserve identity for one asset incarnation. */
    @JvmStatic
    fun liabilityPoolId(
        networkId: NetworkId,
        asset: KagemushaAssetDefinitionIdV1,
        incarnation: KagemushaAssetIncarnationV1,
    ): ByteArray = digestEncoded(
        LIABILITY_POOL_DOMAIN,
        frame("iroha.kagemusha.v1.liability-pool-preimage") { encoder ->
            field(encoder) { it.writeBytes(networkId.bytes()) }
            field(encoder) { it.writeBytes(asset.canonicalPayload()) }
            incarnationField(encoder, incarnation)
        },
    )

    /** Return the canonical digest of a shape-valid request. */
    @JvmStatic
    fun paymentRequestDigest(value: KagemushaPaymentRequestV1): ByteArray {
        validatePaymentRequestShape(value)
        return digestBytes(REQUEST_DIGEST_DOMAIN, paymentRequestTranscript(value))
    }

    /** Return the canonical public lifecycle digest used by terminal proofs. */
    @JvmStatic
    fun lifecycleDigestShape(value: KagemushaLifecycleBindingV1): ByteArray = lifecycleDigest(value)

    /** Expected digest-derived identity for a terminal certificate body. */
    @JvmStatic
    fun expectedCommitCertificateIdShape(value: KagemushaCommitCertificateV1): ByteArray =
        commitCertificateId(value)

    /** Canonical certificate digest after exact lifecycle/evidence/nullifier binding. */
    @JvmStatic
    fun commitCertificateDigestShape(
        value: KagemushaCommitCertificateV1,
        lifecycle: KagemushaLifecycleBindingV1,
        commitEvidence: KagemushaCommitEvidenceV1,
        transitionNullifier: ByteArray,
    ): ByteArray {
        validateCommitCertificateShape(value, lifecycle, commitEvidence, transitionNullifier)
        return commitCertificateDigest(value)
    }

    /** Return the canonical ciphertext envelope digest without decrypting it. */
    @JvmStatic
    fun ciphertextDigestShape(bytes: ByteArray): ByteArray = ciphertextDigest(bytes.copyOf())

    /** Derive the direct request-bound transfer digest before encryption, proving and commit. */
    @JvmStatic
    fun preparedTransferDigestShape(
        request: KagemushaPaymentRequestV1,
        senderBeforeCommitment: ByteArray,
        senderAfterCommitment: ByteArray,
        transitionNullifier: ByteArray,
        ciphertextCommitment: ByteArray,
    ): ByteArray = preparedTransferDigest(
        request,
        senderBeforeCommitment,
        senderAfterCommitment,
        transitionNullifier,
        ciphertextCommitment,
    )

    /** Return the unique peer-credit identity implied by the compact output and context. */
    @JvmStatic
    fun expectedPeerCreditIdShape(
        output: KagemushaPaymentOutputV1,
        request: KagemushaPaymentRequestV1,
    ): ByteArray = expectedPeerCreditId(output, request)

    /** Commit one private peer-credit opening before deriving its credit ID. */
    @JvmStatic
    fun peerCreditOpeningCommitmentShape(
        requestDigest: ByteArray,
        recipientOneTimeKey: KagemushaX25519PublicKeyV1,
        amount: BigInteger,
        creditCommitmentOpening: ByteArray,
        recipientBindingOpening: ByteArray,
        recoveryNonce: ByteArray,
    ): ByteArray {
        require(amount.signum() > 0 && amount.bitLength() <= 128) {
            "KAGEMUSHA V1 peer credit opening amount must be positive and fit u128"
        }
        return sha256(
            PEER_CREDIT_OPENING_COMMITMENT_DOMAIN,
            byteArrayOf(0),
            u16Le(KagemushaWireV1.WIRE_VERSION),
            fixed32(requestDigest, "requestDigest"),
            recipientOneTimeKey.bytes(),
            u128Le(amount),
            fixed32(creditCommitmentOpening, "creditCommitmentOpening"),
            fixed32(recipientBindingOpening, "recipientBindingOpening"),
            fixed32(recoveryNonce, "recoveryNonce"),
        )
    }

    /** Return the canonical digest of a complete request-bound payment. */
    @JvmStatic
    fun paymentDigestShape(
        value: KagemushaPaymentV1,
        request: KagemushaPaymentRequestV1,
    ): ByteArray = paymentDigest(value, request)

    /** Return the exact pre-ID mint-authorization context digest. */
    @JvmStatic
    fun mintAuthorizationContextDigestShape(value: KagemushaMintAuthorizationContextV1): ByteArray {
        validateMintAuthorizationContextShape(value)
        return mintAuthorizationContextDigest(value)
    }

    /** Return the semantic digest a mint-authorization proof must carry. */
    @JvmStatic
    fun mintAuthorizationStatementDigestShape(value: KagemushaMintAuthorizationStatementV1): ByteArray {
        validateMintAuthorizationStatementShape(value)
        return digestEncoded(
            MINT_AUTH_STATEMENT_DIGEST_DOMAIN,
            raw(value, MODEL + "KagemushaMintAuthorizationStatementV1", MINT_AUTH_STATEMENT_ADAPTER),
        )
    }

    /** Return the canonical digest of a complete pre-debit mint authorization. */
    @JvmStatic
    fun mintAuthorizationDigestShape(value: KagemushaMintAuthorizationV1): ByteArray {
        validateMintAuthorizationShape(value)
        return mintAuthorizationDigest(value)
    }

    /** Return the unique mint-credit identity implied by its public statement. */
    @JvmStatic
    fun expectedMintCreditIdShape(value: KagemushaMintCreditStatementV1): ByteArray {
        validateLifecycleShape(value.lifecycle)
        require(value.lifecycle.operationKind == KagemushaOperationKindV1.MINT_FOLD)
        return expectedMintCreditId(value)
    }

    /** Return the semantic digest a mint-credit proof must carry. */
    @JvmStatic
    fun mintCreditStatementDigestShape(value: KagemushaMintCreditStatementV1): ByteArray {
        validateLifecycleShape(value.lifecycle)
        require(value.lifecycle.creditId().contentEquals(expectedMintCreditId(value)))
        return mintStatementDigest(value)
    }

    /** Return the unique online redemption identity implied by its public statement. */
    @JvmStatic
    fun expectedRedemptionIdShape(value: KagemushaRedemptionStatementV1): ByteArray {
        validateLifecycleShape(value.lifecycle)
        require(value.lifecycle.operationKind == KagemushaOperationKindV1.REDEEM_SPLIT)
        return expectedRedemptionId(value)
    }

    /** Return the semantic digest constrained by both redemption-proof parities. */
    @JvmStatic
    fun redemptionStatementDigestShape(value: KagemushaRedemptionStatementV1): ByteArray {
        validateRedemptionStatementShape(value)
        return redemptionStatementDigest(value)
    }

    /** Validate all three separately framed peer lifecycle messages and return total raw bytes. */
    @JvmStatic
    fun validateCompleteExchangeShape(
        request: KagemushaPaymentRequestV1,
        payment: KagemushaPaymentV1,
        acknowledgement: KagemushaAcknowledgementV1,
    ): Int {
        val sizes = intArrayOf(
            encodePaymentRequestShape(request).size,
            encodePaymentShape(payment, request).size,
            encodeAcknowledgementShape(acknowledgement, request, payment).size,
        )
        val raw = sizes.sum()
        require(raw <= KagemushaWireV1.MAXIMUM_COMPLETE_EXCHANGE_RAW_BYTES)
        require(sizes.sumOf(::textEnvelopeLength) <= KagemushaWireV1.MAXIMUM_COMPLETE_EXCHANGE_TEXT_BYTES)
        return raw
    }

    private fun validateAggregateStateShape(value: KagemushaAggregateStateCommitmentV1) {
        require(value.liabilityPoolId().contentEquals(liabilityPoolId(value.networkId, value.asset, value.assetIncarnation)))
    }

    private fun validatePaymentRequestShape(value: KagemushaPaymentRequestV1) {
        require(value.liabilityPoolId().contentEquals(liabilityPoolId(value.networkId, value.asset, value.assetIncarnation)))
        require(value.hardwareCredential.networkId == value.networkId)
        requirePositiveU128(value.amount, "amount")
        require(value.hardwareCredential.deviceKeyReference().contentEquals(deviceKeyReference(value.hardwareCredential.devicePublicKey)))
        require(java.lang.Long.compareUnsigned(value.issuedAtMs, value.hardwareCredential.issuedAtMs) >= 0)
        require(java.lang.Long.compareUnsigned(value.expiresAtMs, value.hardwareCredential.expiresAtMs) <= 0)
    }

    /** Canonical framed typed asset identity used by the fixed request transcript. */
    @JvmStatic
    fun assetIdentityCanonicalShape(value: KagemushaAssetDefinitionIdV1): ByteArray =
        frame("iroha_data_model::asset::id::model::AssetDefinitionId", 1) { it.writeBytes(value.canonicalPayload()) }

    /** Canonical framed typed account identity used by the fixed request transcript. */
    @JvmStatic
    fun accountIdentityCanonicalShape(value: KagemushaAccountIdV1): ByteArray =
        frame("iroha_data_model::account::model::AccountId", 8) { it.writeBytes(value.canonicalPayload()) }

    private fun paymentRequestTranscript(value: KagemushaPaymentRequestV1): ByteArray =
        fixedTranscript(
            390, u16Le(value.version), value.releaseId(), value.networkId.bytes(),
            digestEncoded(ascii("iroha:kagemusha:v1:asset-identity"), assetIdentityCanonicalShape(value.asset)),
            value.assetIncarnation.bytes(), u32Le(value.scale), value.liabilityPoolId(),
            digestEncoded(ascii("iroha:kagemusha:v1:account-identity"), accountIdentityCanonicalShape(value.recipient)),
            u128Le(value.amount), value.recipientEncryptionKey.bytes(),
            value.hardwareCredential.credentialId(), value.requestId(),
            u64Le(value.issuedAtMs), u64Le(value.expiresAtMs), value.signature.rawBytes(),
        )

    /** Fixed semantic output digest, independent of ciphertext, certificate and proof bytes. */
    @JvmStatic
    fun paymentOutputDigestShape(value: KagemushaPaymentOutputV1): ByteArray =
        digestBytes(STATEMENT_DIGEST_DOMAIN, fixedTranscript(
            254, u16Le(value.version), value.requestDigest(), u128Le(value.amount),
            value.senderBeforeCommitment(), value.senderAfterCommitment(),
            value.transitionNullifier(), value.creditId(), value.ciphertextCommitment(),
            commitEvidenceTranscript(value.commitEvidence), u64Le(value.committedAtMs),
        ))

    /** Acyclic body digest committed before hardware consumes the predecessor. */
    @JvmStatic
    fun paymentBodyDigestShape(output: KagemushaPaymentOutputV1, encryptedCredit: ByteArray): ByteArray {
        decodeEncryptedCreditEnvelopeShapeExact(encryptedCredit)
        return digestBytes(ascii("iroha:kagemusha:v1:payment-body"),
            fixedTranscript(64, paymentOutputDigestShape(output), ciphertextDigest(encryptedCredit)))
    }

    private fun validateLifecycleShape(value: KagemushaLifecycleBindingV1) {
        require(value.liabilityPoolId().contentEquals(liabilityPoolId(value.networkId, value.asset, value.assetIncarnation)))
    }

    private fun validatePaymentOutputShape(
        output: KagemushaPaymentOutputV1,
        request: KagemushaPaymentRequestV1,
    ) {
        validatePaymentRequestShape(request)
        require(output.version == KagemushaWireV1.WIRE_VERSION)
        require(output.requestDigest().contentEquals(paymentRequestDigest(request)))
        require(output.amount == request.amount)
        require(output.creditId().contentEquals(expectedPeerCreditId(output, request)))
        require(java.lang.Long.compareUnsigned(output.committedAtMs, request.issuedAtMs) >= 0)
        require(java.lang.Long.compareUnsigned(output.committedAtMs, request.expiresAtMs) < 0)
    }

    private fun validatePaymentShape(
        value: KagemushaPaymentV1,
        request: KagemushaPaymentRequestV1,
    ) {
        require(value.version == KagemushaWireV1.WIRE_VERSION && value.output.version == value.version)
        validatePaymentOutputShape(value.output, request)
        val envelope = decodeEncryptedCreditEnvelopeShapeExact(value.encryptedCredit())
        require(envelope.version == value.version)
        encryptedCreditAadForPeerShape(value.output, request)
        val certificate = value.commitCertificate
        require(certificate.certificateId().contentEquals(commitCertificateId(certificate)))
        require(certificate.transitionNullifier().contentEquals(value.output.transitionNullifier()))
        require(commitEvidenceTranscript(certificate.commitEvidence).contentEquals(commitEvidenceTranscript(value.output.commitEvidence)))
        require(raw(certificate, COMMIT_CERTIFICATE_SCHEMA, COMMIT_CERTIFICATE_ADAPTER).size <= KagemushaWireV1.MAXIMUM_COMMIT_CERTIFICATE_BYTES)
        require(value.proof.semanticDigest().contentEquals(paymentBodyDigestShape(value.output, value.encryptedCredit())))
        require(value.proof.candidateEnvelopeDigest().contentEquals(certificate.candidateEnvelopeDigest()))
        require(value.proof.commitCertificateDigest().contentEquals(commitCertificateDigest(certificate)))
        require(raw(value.proof, PAYMENT_PROOF_SCHEMA, PAYMENT_PROOF_ADAPTER).size <= KagemushaWireV1.MAXIMUM_PAIRED_PROOF_BYTES)
        require(envelope.ephemeralX25519PublicKey != request.recipientEncryptionKey)
    }

    private fun validateAcknowledgementShape(
        value: KagemushaAcknowledgementV1,
        request: KagemushaPaymentRequestV1,
        payment: KagemushaPaymentV1,
    ) {
        validatePaymentShape(payment, request)
        require(value.requestDigest().contentEquals(paymentRequestDigest(request)))
        require(value.paymentDigest().contentEquals(paymentDigest(payment, request)))
        require(value.inboxReceipt.creditId().contentEquals(payment.output.creditId()))
    }

    private fun commitEvidenceTranscript(value: KagemushaCommitEvidenceV1): ByteArray =
        fixedTranscript(36, u32Le(value.wireTag), value.evidenceCommitment())

    private fun commitCertificateId(value: KagemushaCommitCertificateV1): ByteArray =
        digestBytes(
            COMMIT_CERTIFICATE_ID_DOMAIN,
            fixedTranscript(
                238,
                u16Le(value.version),
                value.candidateEnvelopeDigest(),
                value.lifecycleBindingDigest(),
                value.transitionNullifier(),
                value.outboxReservationCommitment(),
                commitEvidenceTranscript(value.commitEvidence),
                value.hardwareProfileId(),
                u64Le(value.policyEpoch),
                value.hardwareTerminalCommitment(),
            ),
        )

    private fun commitCertificateDigest(value: KagemushaCommitCertificateV1): ByteArray =
        digestBytes(
            COMMIT_CERTIFICATE_DIGEST_DOMAIN,
            fixedTranscript(
                270,
                u16Le(value.version),
                value.certificateId(),
                value.candidateEnvelopeDigest(),
                value.lifecycleBindingDigest(),
                value.transitionNullifier(),
                value.outboxReservationCommitment(),
                commitEvidenceTranscript(value.commitEvidence),
                value.hardwareProfileId(),
                u64Le(value.policyEpoch),
                value.hardwareTerminalCommitment(),
            ),
        )

    private fun validateCommitCertificateShape(
        value: KagemushaCommitCertificateV1,
        lifecycle: KagemushaLifecycleBindingV1,
        expectedEvidence: KagemushaCommitEvidenceV1,
        expectedNullifier: ByteArray,
    ) {
        validateLifecycleShape(lifecycle)
        require(value.lifecycleBindingDigest().contentEquals(lifecycleDigest(lifecycle)))
        require(value.transitionNullifier().contentEquals(expectedNullifier))
        require(commitEvidenceTranscript(value.commitEvidence).contentEquals(commitEvidenceTranscript(expectedEvidence)))
        require(value.hardwareProfileId().contentEquals(lifecycle.hardwareProfileId()))
        require(value.policyEpoch == lifecycle.policyEpoch)
        require(value.certificateId().contentEquals(commitCertificateId(value)))
    }

    private fun validateMintAuthorizationStatementShape(
        value: KagemushaMintAuthorizationStatementV1,
    ) {
        validateMintAuthorizationContextShape(value.context)
    }

    private fun validateMintAuthorizationContextShape(value: KagemushaMintAuthorizationContextV1) {
        require(value.liabilityPoolId().contentEquals(liabilityPoolId(value.networkId, value.asset, value.assetIncarnation)))
    }

    private fun validateMintAuthorizationShape(value: KagemushaMintAuthorizationV1) {
        validateMintAuthorizationStatementShape(value.statement)
        val semantic = digestEncoded(
            MINT_AUTH_STATEMENT_DIGEST_DOMAIN,
            raw(value.statement, MODEL + "KagemushaMintAuthorizationStatementV1", MINT_AUTH_STATEMENT_ADAPTER),
        )
        require(value.proof.semanticDigest().contentEquals(semantic))
    }

    private fun validateMintCreditShape(value: KagemushaMintCreditV1) {
        val statement = value.statement
        val lifecycle = statement.lifecycle
        validateLifecycleShape(lifecycle)
        require(lifecycle.operationKind == KagemushaOperationKindV1.MINT_FOLD)
        require(lifecycle.creditId().contentEquals(expectedMintCreditId(statement)))
        require(value.proof.semanticDigest().contentEquals(mintStatementDigest(statement)))
        val envelope = decodeEncryptedCreditEnvelopeShapeExact(value.encryptedCredit())
        require(envelope.version == value.version)
        require(lifecycle.ciphertextDigest().contentEquals(ciphertextDigest(value.encryptedCredit())))
    }

    private fun validateMintCreditShapeAgainstAuthorization(
        value: KagemushaMintCreditV1,
        authorization: KagemushaMintAuthorizationV1,
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

    private fun validateDeviceMintStageCommandShape(
        value: KagemushaDeviceMintStageCommandV1,
    ): KagemushaMintCreditV1 {
        require(value.version == KagemushaWireV1.WIRE_VERSION)
        val authorization = decodeMintAuthorizationShapeExact(value.canonicalAuthorization())
        return decodeMintCreditShapeExact(value.canonicalMintCredit(), authorization)
    }

    private fun validateRedemptionVoucherShape(value: KagemushaRedemptionVoucherV1) {
        val statement = value.statement
        validateRedemptionStatementShape(statement)
        validateCommitCertificateShape(
            value.commitCertificate,
            statement.lifecycle,
            statement.commitEvidence,
            statement.terminalNullifier(),
        )
        require(value.proof.semanticDigest().contentEquals(redemptionStatementDigest(statement)))
        require(value.proof.candidateEnvelopeDigest().contentEquals(value.commitCertificate.candidateEnvelopeDigest()))
        require(value.proof.commitCertificateDigest().contentEquals(commitCertificateDigest(value.commitCertificate)))
    }

    private fun validateTopUpRequestShape(value: KagemushaTopUpRequestV1) {
        validateMintAuthorizationShape(value.mintAuthorization)
        val statement = value.mintAuthorization.statement
        val context = statement.context
        require(value.liabilityPoolId().contentEquals(liabilityPoolId(value.networkId, value.asset, value.assetIncarnation)))
        require(value.operationId().contentEquals(context.operationId()))
        require(value.issuanceCommitment().contentEquals(statement.issuanceCommitment()))
        require(value.creditId().contentEquals(statement.creditId()))
        require(value.releaseId().contentEquals(context.releaseId()))
        require(value.suiteId().contentEquals(context.suiteId()))
        require(value.vkDigest().contentEquals(context.vkDigest()))
        require(value.networkId == context.networkId)
        require(value.asset == context.asset && value.assetIncarnation == context.assetIncarnation)
        require(value.scale == context.scale && value.amount == context.amount)
        require(value.liabilityPoolId().contentEquals(context.liabilityPoolId()))
        require(value.payer == context.payer && value.recipient == context.recipient)
        require(value.hardwareCredential.credentialId().contentEquals(context.hardwareCredentialId()))
        require(value.hardwareCredential.hardwareProfileId().contentEquals(context.hardwareProfileId()))
        require(value.hardwareCredential.policyEpoch == context.policyEpoch)
        require(value.recipientCredentialCommitment().contentEquals(context.recipientCredentialCommitment()))
        require(value.creditCommitment().contentEquals(context.creditCommitment()))
        require(value.recipientOneTimeKey == context.recipientOneTimeKey)
        require(value.artifactManifestDigest().contentEquals(context.artifactManifestDigest()))
        decodeEncryptedCreditEnvelopeShapeExact(value.encryptedCredit())
        require(ciphertextDigest(value.encryptedCredit()).contentEquals(statement.ciphertextDigest()))
    }

    private fun validateRedemptionRequestShape(value: KagemushaRedemptionRequestV1) {
        validateRedemptionVoucherShape(value.voucher)
    }

    private fun validateRedemptionStatementShape(value: KagemushaRedemptionStatementV1) {
        val lifecycle = value.lifecycle
        validateLifecycleShape(lifecycle)
        require(lifecycle.operationKind == KagemushaOperationKindV1.REDEEM_SPLIT)
        require(!value.terminalNullifier().contentEquals(value.redemptionCommitment()))
        require(!value.terminalNullifier().contentEquals(value.redemptionId()))
        require(!value.redemptionCommitment().contentEquals(value.redemptionId()))
        require(value.redemptionId().contentEquals(expectedRedemptionId(value)))
    }

    private fun lifecycleDigest(value: KagemushaLifecycleBindingV1): ByteArray {
        validateLifecycleShape(value)
        return digestEncoded(LIFECYCLE_DIGEST_DOMAIN, raw(value, LIFECYCLE_SCHEMA, LIFECYCLE_ADAPTER))
    }

    private fun ciphertextDigest(bytes: ByteArray): ByteArray = digestBytes(CIPHERTEXT_DIGEST_DOMAIN, bytes)

    private fun paymentDigest(
        value: KagemushaPaymentV1,
        request: KagemushaPaymentRequestV1,
    ): ByteArray {
        validatePaymentShape(value, request)
        return digestEncoded(PAYMENT_DIGEST_DOMAIN, raw(value, PAYMENT_SCHEMA, PAYMENT_ADAPTER))
    }

    private fun mintAuthorizationContextDigest(value: KagemushaMintAuthorizationContextV1): ByteArray =
        digestEncoded(
            MINT_AUTH_CONTEXT_DIGEST_DOMAIN,
            raw(value, MODEL + "KagemushaMintAuthorizationContextV1", MINT_AUTH_CONTEXT_ADAPTER),
        )

    private fun mintAuthorizationDigest(value: KagemushaMintAuthorizationV1): ByteArray =
        digestEncoded(MINT_AUTH_DIGEST_DOMAIN, raw(value, MINT_AUTH_SCHEMA, MINT_AUTH_ADAPTER))

    private fun mintStatementDigest(value: KagemushaMintCreditStatementV1): ByteArray =
        digestEncoded(MINT_STATEMENT_DIGEST_DOMAIN, raw(value, MINT_STATEMENT_SCHEMA, MINT_STATEMENT_ADAPTER))

    private fun mintLifecycleContextDigest(value: KagemushaLifecycleBindingV1): ByteArray =
        digestEncoded(
            MINT_LIFECYCLE_CONTEXT_DOMAIN,
            frame("iroha.kagemusha.v1.mint-lifecycle-context-preimage") { e ->
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
                enumUnitField(e, value.operationKind.wireTag)
            },
        )

    private fun expectedMintCreditId(value: KagemushaMintCreditStatementV1): ByteArray =
        digestEncoded(
            MINT_CREDIT_ID_DOMAIN,
            frame("iroha.kagemusha.v1.mint-credit-id-preimage", 16) { e ->
                bytes32Field(e, mintLifecycleContextDigest(value.lifecycle))
                bytes32Field(e, value.recipientCredentialCommitment())
                bytes32Field(e, value.authorizationContextDigest())
                u128Field(e, value.amount)
                bytes32Field(e, value.issuanceCommitment())
                accountField(e, value.recipient)
                bytes32Field(e, value.creditCommitment())
            },
        )

    private fun redemptionStatementDigest(value: KagemushaRedemptionStatementV1): ByteArray =
        digestEncoded(
            REDEMPTION_STATEMENT_DIGEST_DOMAIN,
            raw(value, REDEMPTION_STATEMENT_SCHEMA, REDEMPTION_STATEMENT_ADAPTER),
        )

    private fun expectedRedemptionId(value: KagemushaRedemptionStatementV1): ByteArray =
        digestEncoded(
            REDEMPTION_ID_DOMAIN,
            frame("iroha.kagemusha.v1.redemption-id-preimage", 16) { e ->
                bytes32Field(e, lifecycleDigest(value.lifecycle))
                bytes32Field(e, value.terminalNullifier())
                u128Field(e, value.amount)
                accountField(e, value.beneficiary)
                bytes32Field(e, value.redemptionCommitment())
            },
        )

    private fun preparedTransferDigest(
        request: KagemushaPaymentRequestV1,
        senderBeforeCommitment: ByteArray,
        senderAfterCommitment: ByteArray,
        transitionNullifier: ByteArray,
        ciphertextCommitment: ByteArray,
    ): ByteArray {
        validatePaymentRequestShape(request)
        val before = fixed32(senderBeforeCommitment, "senderBeforeCommitment")
        val after = fixed32(senderAfterCommitment, "senderAfterCommitment")
        require(!before.contentEquals(after))
        return digestBytes(
            PREPARED_TRANSFER_DIGEST_DOMAIN,
            fixedTranscript(
                210, u16Le(KagemushaWireV1.WIRE_VERSION), paymentRequestDigest(request),
                u128Le(request.amount), before, after,
                fixed32(transitionNullifier, "transitionNullifier"),
                request.recipientEncryptionKey.bytes(), fixed32(ciphertextCommitment, "ciphertextCommitment"),
            ),
        )
    }

    private fun expectedPeerCreditId(
        output: KagemushaPaymentOutputV1,
        request: KagemushaPaymentRequestV1,
    ): ByteArray = sha256(
        CREDIT_ID_DOMAIN, byteArrayOf(0), output.transitionNullifier(),
        paymentRequestDigest(request),
    )

    private val AGGREGATE_ADAPTER = adapter<KagemushaAggregateStateCommitmentV1>(
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
            KagemushaAggregateStateCommitmentV1(
                readU16(d), readFixed32(d), readNetwork(d), readAsset(d), readIncarnation(d),
                readU32(d), readFixed32(d), readFixed32(d), readFixed32(d), readFixed32(d),
                readFixed32(d), readU128(d), readFixed32(d),
            )
        },
    )

    private val PASTA_STATE_ADAPTER = adapter<KagemushaPastaStateCommitmentV1>(
        encode = { e, v ->
            bytes32Field(e, v.eq())
            bytes32Field(e, v.ep())
        },
        decode = { d -> KagemushaPastaStateCommitmentV1(readFixed32(d), readFixed32(d)) },
    )

    private val PROOF_ADAPTER = adapter<KagemushaPairedProofV1>(
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
            KagemushaPairedProofV1(
                readU16(d), readFixed32(d), readFixed32(d), readFixed32(d), readFixed32(d),
                readFixed32(d), readFixed32(d), readFixed32(d), readVector(d), readVector(d),
                readVector(d), readVector(d),
            )
        },
    )

    private val HARDWARE_PROFILE_ADAPTER = adapter<KagemushaHardwareProfileV1>(
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
            KagemushaHardwareProfileV1(
                readU16(d), readU16(d), readFixed32(d), readFixed32(d),
                KagemushaHardwarePlatformClassV1.values()[readEnumUnit(d, 4)],
                readFixed32(d), readFixed32(d), readFixed32(d), readFixed32(d), readFixed32(d),
                readU64(d), readPublicKey(d), readU16(d), readFixed32(d), readU64(d), readU64(d),
            )
        },
    )

    private val HARDWARE_CREDENTIAL_ADAPTER = adapter<KagemushaHardwareCredentialV1>(
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
            KagemushaHardwareCredentialV1(
                readU16(d), readFixed32(d), readNetwork(d), readFixed32(d), readFixed32(d),
                readFixed32(d), readU64(d), readFixed32(d), readFixed32(d), readU64(d),
                readPublicKey(d), readFixed32(d), readU64(d), readU64(d), readSignature(d),
            )
        },
    )

    private val REQUEST_ADAPTER = adapter<KagemushaPaymentRequestV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            bytes32Field(e, v.releaseId())
            networkField(e, v.networkId)
            assetField(e, v.asset)
            incarnationField(e, v.assetIncarnation)
            u32Field(e, v.scale)
            bytes32Field(e, v.liabilityPoolId())
            accountField(e, v.recipient)
            u128Field(e, v.amount)
            bytes32Field(e, v.recipientEncryptionKey.bytes())
            nestedField(e, HARDWARE_CREDENTIAL_ADAPTER, v.hardwareCredential)
            bytes32Field(e, v.requestId())
            u64Field(e, v.issuedAtMs)
            u64Field(e, v.expiresAtMs)
            signatureField(e, v.signature)
        },
        decode = { d ->
            KagemushaPaymentRequestV1(
                readU16(d), readFixed32(d), readNetwork(d), readAsset(d), readIncarnation(d),
                readU32(d), readFixed32(d), readAccount(d), readU128(d),
                KagemushaX25519PublicKeyV1(readRaw32(d)),
                readNested(d, HARDWARE_CREDENTIAL_ADAPTER),
                readFixed32(d), readU64(d), readU64(d), readSignature(d),
            )
        },
    )

    private val PEER_CREDIT_CONTEXT_ADAPTER = adapter<KagemushaPeerCreditContextV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            bytes32Field(e, v.requestDigest())
            u128Field(e, v.amount)
            bytes32Field(e, v.senderBeforeCommitment())
            bytes32Field(e, v.senderAfterCommitment())
            bytes32Field(e, v.preparedTransferDigest())
            bytes32Field(e, v.recipientEncryptionKey.bytes())
        },
        decode = { d ->
            KagemushaPeerCreditContextV1(
                readU16(d), readFixed32(d), readU128(d), readFixed32(d), readFixed32(d), readFixed32(d),
                KagemushaX25519PublicKeyV1(readRaw32(d)),
            )
        },
    )

    private val CREDIT_OPENING_ADAPTER = adapter<KagemushaCreditOpeningV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            bytes32Field(e, v.creditId())
            u128Field(e, v.amount)
            bytes32Field(e, v.creditCommitmentOpening())
            bytes32Field(e, v.recipientBindingOpening())
            bytes32Field(e, v.recoveryNonce())
        },
        decode = { d ->
            KagemushaCreditOpeningV1(
                readU16(d), readFixed32(d), readU128(d), readFixed32(d), readFixed32(d), readFixed32(d),
            )
        },
    )

    private val CREDIT_AAD_ADAPTER = adapter<KagemushaEncryptedCreditAadV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            enumUnitField(e, v.purpose.ordinal)
            bytes32Field(e, v.contextDigest())
            bytes32Field(e, v.issuanceOrTransitionCommitment())
            bytes32Field(e, v.creditId())
            u128Field(e, v.amount)
        },
        decode = { d ->
            KagemushaEncryptedCreditAadV1(
                readU16(d), KagemushaEncryptedCreditPurposeV1.values()[readEnumUnit(d, 2)],
                readFixed32(d), readFixed32(d), readFixed32(d), readU128(d),
            )
        },
    )

    private val CREDIT_ENVELOPE_ADAPTER = adapter<KagemushaEncryptedCreditEnvelopeV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            bytes32Field(e, v.ephemeralX25519PublicKey.bytes())
            field(e) { fixedArray(it, v.nonce()) }
            vectorField(e, v.ciphertextAndTag())
        },
        decode = { d ->
            KagemushaEncryptedCreditEnvelopeV1(
                readU16(d), KagemushaX25519PublicKeyV1(readRaw32(d)), readExactField(d, 24), readVector(d),
            )
        },
    )

    private val LIFECYCLE_ADAPTER = adapter<KagemushaLifecycleBindingV1>(
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
            enumUnitField(e, v.operationKind.wireTag)
            raw32Field(e, v.requestId())
            raw32Field(e, v.receiverLaneCommitment())
            raw32Field(e, v.creditId())
            raw32Field(e, v.ciphertextDigest())
        },
        decode = { d ->
            KagemushaLifecycleBindingV1(
                readU16(d), readNetwork(d), readU16(d), readFixed32(d), readFixed32(d), readFixed32(d),
                readAsset(d), readIncarnation(d), readU32(d), readFixed32(d), readFixed32(d), readU64(d),
                KagemushaOperationKindV1.values()[readEnumUnit(d, 7)], readRaw32(d), readRaw32(d),
                readRaw32(d), readRaw32(d),
            )
        },
    )

    private val COMMIT_EVIDENCE_ADAPTER = adapter<KagemushaCommitEvidenceV1>(
        encode = { e, v ->
            enumPayload(e, v.wireTag) { payload ->
                bytes32Field(payload, v.evidenceCommitment())
            }
        },
        decode = { d ->
            when (readEnumTag(d, 2)) {
                0 -> readEnumPayload(d) { KagemushaTrustedCommitTimeV1(readFixed32(it)) }
                else -> readEnumPayload(d) { KagemushaMonotonicLeaseV1(readFixed32(it)) }
            }
        },
    )

    private val COMMIT_CERTIFICATE_ADAPTER = adapter<KagemushaCommitCertificateV1>(
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
            KagemushaCommitCertificateV1(
                readU16(d), readFixed32(d), readFixed32(d), readFixed32(d), readFixed32(d),
                readFixed32(d), readNested(d, COMMIT_EVIDENCE_ADAPTER), readFixed32(d),
                readU64(d), readFixed32(d),
            )
        },
    )

    private val PAYMENT_PROOF_ADAPTER = adapter<KagemushaPaymentProofV1>(
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
            KagemushaPaymentProofV1(
                readU16(d), readFixed32(d), readFixed32(d), readFixed32(d), readFixed32(d),
                readFixed32(d), readFixed32(d), readFixed32(d), readVector(d), readVector(d),
                readVector(d), readVector(d),
            )
        },
    )

    private val REDEMPTION_PROOF_ADAPTER = adapter<KagemushaRedemptionProofV1>(
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
            KagemushaRedemptionProofV1(
                readU16(d), readFixed32(d), readFixed32(d), readFixed32(d), readFixed32(d),
                readFixed32(d), readFixed32(d), readFixed32(d), readVector(d), readVector(d),
                readVector(d), readVector(d),
            )
        },
    )

    private val PAYMENT_OUTPUT_ADAPTER = adapter<KagemushaPaymentOutputV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            bytes32Field(e, v.requestDigest())
            u128Field(e, v.amount)
            bytes32Field(e, v.senderBeforeCommitment())
            bytes32Field(e, v.senderAfterCommitment())
            bytes32Field(e, v.transitionNullifier())
            bytes32Field(e, v.creditId())
            bytes32Field(e, v.ciphertextCommitment())
            nestedField(e, COMMIT_EVIDENCE_ADAPTER, v.commitEvidence)
            u64Field(e, v.committedAtMs)
        },
        decode = { d ->
            KagemushaPaymentOutputV1(
                readU16(d), readFixed32(d), readU128(d), readFixed32(d), readFixed32(d),
                readFixed32(d), readFixed32(d), readFixed32(d),
                readNested(d, COMMIT_EVIDENCE_ADAPTER), readU64(d),
            )
        },
    )

    private val PAYMENT_ADAPTER = adapter<KagemushaPaymentV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            nestedField(e, PAYMENT_OUTPUT_ADAPTER, v.output)
            vectorField(e, v.encryptedCredit())
            nestedField(e, COMMIT_CERTIFICATE_ADAPTER, v.commitCertificate)
            nestedField(e, PAYMENT_PROOF_ADAPTER, v.proof)
        },
        decode = { d ->
            KagemushaPaymentV1(
                readU16(d), readNested(d, PAYMENT_OUTPUT_ADAPTER), readVector(d),
                readNested(d, COMMIT_CERTIFICATE_ADAPTER), readNested(d, PAYMENT_PROOF_ADAPTER),
            )
        },
    )

    private val RECEIPT_ADAPTER = adapter<KagemushaInboxReceiptV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            bytes32Field(e, v.creditId())
            bytes32Field(e, v.receiptCommitment())
        },
        decode = { d -> KagemushaInboxReceiptV1(readU16(d), readFixed32(d), readFixed32(d)) },
    )

    private val ACK_ADAPTER = adapter<KagemushaAcknowledgementV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            bytes32Field(e, v.requestDigest())
            bytes32Field(e, v.paymentDigest())
            nestedField(e, RECEIPT_ADAPTER, v.inboxReceipt)
            signatureField(e, v.signature)
        },
        decode = { d ->
            KagemushaAcknowledgementV1(
                readU16(d), readFixed32(d), readFixed32(d), readNested(d, RECEIPT_ADAPTER), readSignature(d),
            )
        },
    )

    private val MINT_AUTH_CONTEXT_ADAPTER = adapter<KagemushaMintAuthorizationContextV1>(
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
            KagemushaMintAuthorizationContextV1(
                readU16(d), readFixed32(d), readFixed32(d), readFixed32(d), readFixed32(d),
                readFixed32(d), readNetwork(d), readAsset(d), readIncarnation(d), readU32(d),
                readFixed32(d), readU128(d), readAccount(d), readAccount(d), readFixed32(d),
                readFixed32(d), readU64(d), readFixed32(d), readFixed32(d),
                KagemushaX25519PublicKeyV1(readRaw32(d)),
            )
        },
    )

    private val MINT_AUTH_STATEMENT_ADAPTER = adapter<KagemushaMintAuthorizationStatementV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            nestedField(e, MINT_AUTH_CONTEXT_ADAPTER, v.context)
            bytes32Field(e, v.issuanceCommitment())
            bytes32Field(e, v.creditId())
            bytes32Field(e, v.ciphertextDigest())
        },
        decode = { d ->
            KagemushaMintAuthorizationStatementV1(
                readU16(d), readNested(d, MINT_AUTH_CONTEXT_ADAPTER), readFixed32(d),
                readFixed32(d), readFixed32(d),
            )
        },
    )

    private val MINT_AUTH_ADAPTER = adapter<KagemushaMintAuthorizationV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            nestedField(e, MINT_AUTH_STATEMENT_ADAPTER, v.statement)
            nestedField(e, PROOF_ADAPTER, v.proof)
        },
        decode = { d -> KagemushaMintAuthorizationV1(readU16(d), readNested(d, MINT_AUTH_STATEMENT_ADAPTER), readNested(d, PROOF_ADAPTER)) },
    )

    private val MINT_STATEMENT_ADAPTER = adapter<KagemushaMintCreditStatementV1>(
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
            KagemushaMintCreditStatementV1(
                readU16(d), readNested(d, LIFECYCLE_ADAPTER), readFixed32(d), readFixed32(d),
                readFixed32(d), readU128(d), readFixed32(d), readAccount(d), readFixed32(d), readU64(d),
            )
        },
    )

    private val MINT_ADAPTER = adapter<KagemushaMintCreditV1>(
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
            KagemushaMintCreditV1(
                readU16(d), readNested(d, MINT_STATEMENT_ADAPTER), readNested(d, PROOF_ADAPTER),
                readFixed32(d), readFixed32(d), readFixed32(d), readFixed32(d), readVector(d), readFixed32(d),
            )
        },
    )

    private val DEVICE_MINT_STAGE_COMMAND_ADAPTER =
        adapter<KagemushaDeviceMintStageCommandV1>(
            encode = { e, v ->
                u16Field(e, v.version)
                vectorField(e, v.canonicalAuthorization())
                vectorField(e, v.canonicalMintCredit())
            },
            decode = { d ->
                KagemushaDeviceMintStageCommandV1(
                    readU16(d),
                    readVector(d),
                    readVector(d),
                )
            },
        )

    private val DEVICE_MINT_STAGE_RESULT_ADAPTER =
        adapter<KagemushaDeviceMintStageResultV1>(
            encode = { e, v ->
                u16Field(e, v.version)
                u8Field(e, v.disposition)
                bytes32Field(e, v.creditId())
            },
            decode = { d ->
                KagemushaDeviceMintStageResultV1(
                    readU16(d),
                    readU8(d),
                    readFixed32(d),
                )
            },
        )

    private val REDEMPTION_STATEMENT_ADAPTER = adapter<KagemushaRedemptionStatementV1>(
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
            KagemushaRedemptionStatementV1(
                readU16(d), readNested(d, LIFECYCLE_ADAPTER), readU128(d), readAccount(d),
                readFixed32(d), readFixed32(d), readFixed32(d),
                readNested(d, COMMIT_EVIDENCE_ADAPTER),
            )
        },
    )

    private val REDEMPTION_ADAPTER = adapter<KagemushaRedemptionVoucherV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            nestedField(e, REDEMPTION_STATEMENT_ADAPTER, v.statement)
            nestedField(e, COMMIT_CERTIFICATE_ADAPTER, v.commitCertificate)
            nestedField(e, REDEMPTION_PROOF_ADAPTER, v.proof)
            bytes32Field(e, v.artifactManifestDigest())
        },
        decode = { d ->
            KagemushaRedemptionVoucherV1(
                readU16(d), readNested(d, REDEMPTION_STATEMENT_ADAPTER),
                readNested(d, COMMIT_CERTIFICATE_ADAPTER), readNested(d, REDEMPTION_PROOF_ADAPTER),
                readFixed32(d),
            )
        },
    )

    private val OPTIONAL_MINT_AUTH_ADAPTER = NoritoAdapters.option(MINT_AUTH_ADAPTER)

    private val TOP_UP_REQUEST_ADAPTER = adapter<KagemushaTopUpRequestV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            bytes32Field(e, v.operationId())
            bytes32Field(e, v.issuanceCommitment())
            bytes32Field(e, v.creditId())
            bytes32Field(e, v.releaseId())
            bytes32Field(e, v.suiteId())
            bytes32Field(e, v.vkDigest())
            networkField(e, v.networkId)
            assetField(e, v.asset)
            incarnationField(e, v.assetIncarnation)
            u32Field(e, v.scale)
            u128Field(e, v.amount)
            bytes32Field(e, v.liabilityPoolId())
            accountField(e, v.payer)
            accountField(e, v.recipient)
            nestedField(e, HARDWARE_CREDENTIAL_ADAPTER, v.hardwareCredential)
            bytes32Field(e, v.recipientCredentialCommitment())
            bytes32Field(e, v.creditCommitment())
            bytes32Field(e, v.recipientOneTimeKey.bytes())
            vectorField(e, v.encryptedCredit())
            bytes32Field(e, v.artifactManifestDigest())
            nestedField(e, OPTIONAL_MINT_AUTH_ADAPTER, Optional.of(v.mintAuthorization))
        },
        decode = { d ->
            val version = readU16(d)
            val operationId = readFixed32(d)
            val issuanceCommitment = readFixed32(d)
            val creditId = readFixed32(d)
            val releaseId = readFixed32(d)
            val suiteId = readFixed32(d)
            val vkDigest = readFixed32(d)
            val networkId = readNetwork(d)
            val asset = readAsset(d)
            val assetIncarnation = readIncarnation(d)
            val scale = readU32(d)
            val amount = readU128(d)
            val liabilityPoolId = readFixed32(d)
            val payer = readAccount(d)
            val recipient = readAccount(d)
            val credential = readNested(d, HARDWARE_CREDENTIAL_ADAPTER)
            val recipientCredentialCommitment = readFixed32(d)
            val creditCommitment = readFixed32(d)
            val recipientOneTimeKey = KagemushaX25519PublicKeyV1(readRaw32(d))
            val encryptedCredit = readVector(d)
            val artifactManifestDigest = readFixed32(d)
            val authorization = readNested(d, OPTIONAL_MINT_AUTH_ADAPTER).orElseThrow {
                IllegalArgumentException("canonical KAGEMUSHA V1 top-up requires mint authorization")
            }
            KagemushaTopUpRequestV1(
                version,
                operationId,
                issuanceCommitment,
                creditId,
                releaseId,
                suiteId,
                vkDigest,
                networkId,
                asset,
                assetIncarnation,
                scale,
                amount,
                liabilityPoolId,
                payer,
                recipient,
                credential,
                recipientCredentialCommitment,
                creditCommitment,
                recipientOneTimeKey,
                encryptedCredit,
                artifactManifestDigest,
                authorization,
            )
        },
    )

    private val REDEMPTION_REQUEST_ADAPTER = adapter<KagemushaRedemptionRequestV1>(
        encode = { e, v ->
            u16Field(e, v.version)
            bytes32Field(e, v.operationId())
            nestedField(e, REDEMPTION_ADAPTER, v.voucher)
        },
        decode = { d ->
            KagemushaRedemptionRequestV1(
                readU16(d),
                readFixed32(d),
                readNested(d, REDEMPTION_ADAPTER),
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
        PEER_CREDIT_CONTEXT_SCHEMA,
        LIFECYCLE_SCHEMA,
        COMMIT_CERTIFICATE_SCHEMA,
        REDEMPTION_PROOF_SCHEMA,
        PAYMENT_PROOF_SCHEMA,
        DEVICE_MINT_STAGE_COMMAND_SCHEMA,
        -> 8
        ACK_SCHEMA,
        -> 2
        AGGREGATE_SCHEMA,
        PASTA_STATE_SCHEMA,
        REQUEST_SCHEMA,
        PAYMENT_SCHEMA,
        CREDIT_OPENING_SCHEMA,
        CREDIT_AAD_SCHEMA,
        PAYMENT_OUTPUT_SCHEMA,
        MODEL + "KagemushaMintAuthorizationContextV1",
        MODEL + "KagemushaMintAuthorizationStatementV1",
        MINT_AUTH_SCHEMA,
        MINT_STATEMENT_SCHEMA,
        MINT_SCHEMA,
        REDEMPTION_STATEMENT_SCHEMA,
        REDEMPTION_SCHEMA,
        TOP_UP_REQUEST_SCHEMA,
        REDEMPTION_REQUEST_SCHEMA,
        -> 16
        DEVICE_MINT_STAGE_RESULT_SCHEMA,
        -> 2
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
        require(length <= parent.remaining().toLong()) { "truncated KAGEMUSHA V1 field" }
        return NoritoDecoder(parent.readBytes(length.toInt()), parent.flags)
    }

    private fun <T> nestedField(parent: NoritoEncoder, adapter: TypeAdapter<T>, value: T) =
        field(parent) { adapter.encode(it, value) }

    private fun <T> readNested(parent: NoritoDecoder, adapter: TypeAdapter<T>): T {
        val child = readField(parent)
        val value = adapter.decode(child)
        require(child.remaining() == 0) { "trailing nested KAGEMUSHA V1 bytes" }
        return value
    }

    private fun u16Field(e: NoritoEncoder, value: Int) = field(e) { it.writeUInt(value.toLong(), 16) }
    private fun u8Field(e: NoritoEncoder, value: Int) = field(e) { it.writeUInt(value.toLong(), 8) }
    private fun u32Field(e: NoritoEncoder, value: Int) = field(e) { it.writeUInt(value.toLong(), 32) }
    private fun u64Field(e: NoritoEncoder, value: Long) = field(e) { it.writeUInt(value, 64) }
    private fun u128Field(e: NoritoEncoder, value: BigInteger) = field(e) { uint128(it, value) }
    private fun bytes32Field(e: NoritoEncoder, value: ByteArray) = field(e) { fixedArray(it, fixed32(value, "fixed32")) }
    private fun raw32Field(e: NoritoEncoder, value: ByteArray) = field(e) { fixedArray(it, raw32(value, "raw32")) }
    private fun networkField(e: NoritoEncoder, value: NetworkId) = field(e) { it.writeBytes(value.bytes()) }
    private fun assetField(e: NoritoEncoder, value: KagemushaAssetDefinitionIdV1) = field(e) { it.writeBytes(value.canonicalPayload()) }
    private fun accountField(e: NoritoEncoder, value: KagemushaAccountIdV1) = field(e) { it.writeBytes(value.canonicalPayload()) }
    private fun incarnationField(e: NoritoEncoder, value: KagemushaAssetIncarnationV1) =
        field(e) { raw32Field(it, value.bytes()) }
    private fun publicKeyField(e: NoritoEncoder, value: KagemushaDevicePublicKeyV1) = field(e) { it.writeBytes(value.sec1Bytes()) }
    private fun signatureField(e: NoritoEncoder, value: KagemushaDeviceSignatureV1) = field(e) { it.writeBytes(value.rawBytes()) }
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

    private fun readU8(decoder: NoritoDecoder): Int {
        val child = readField(decoder)
        val value = child.readUInt(8).toInt()
        require(child.remaining() == 0)
        return value
    }

    private fun readU32(decoder: NoritoDecoder): Int {
        val child = readField(decoder)
        val value = child.readUInt(32)
        require(child.remaining() == 0)
        return value.toInt()
    }

    private fun readU64(decoder: NoritoDecoder): Long {
        val child = readField(decoder)
        val value = child.readUInt(64)
        require(child.remaining() == 0)
        return value
    }

    private fun readNetwork(decoder: NoritoDecoder): NetworkId =
        NetworkId.fromBytes(readExactField(decoder, 32))

    private fun readIncarnation(decoder: NoritoDecoder): KagemushaAssetIncarnationV1 {
        val child = readField(decoder)
        val value = KagemushaAssetIncarnationV1(readRaw32(child))
        require(child.remaining() == 0)
        return value
    }

    private fun readAsset(decoder: NoritoDecoder): KagemushaAssetDefinitionIdV1 {
        val child = readField(decoder)
        return KagemushaAssetDefinitionIdV1.fromCanonicalPayload(child.readBytes(child.remaining()))
    }

    private fun readAccount(decoder: NoritoDecoder): KagemushaAccountIdV1 {
        val child = readField(decoder)
        return KagemushaAccountIdV1.fromCanonicalPayload(child.readBytes(child.remaining()))
    }

    private fun readPublicKey(decoder: NoritoDecoder): KagemushaDevicePublicKeyV1 =
        KagemushaDevicePublicKeyV1(readExactField(decoder, 65))

    private fun readSignature(decoder: NoritoDecoder): KagemushaDeviceSignatureV1 =
        KagemushaDeviceSignatureV1(readExactField(decoder, 64))

    private fun <T> decodeExact(
        bytes: ByteArray,
        maximum: Int,
        schema: String,
        adapter: TypeAdapter<T>,
        encode: (T) -> ByteArray,
    ): T {
        require(bytes.isNotEmpty() && bytes.size <= maximum) { "KAGEMUSHA V1 archive is empty or oversized" }
        val canonical = bytes.copyOf()
        require(canonical.size >= NoritoHeader.HEADER_LENGTH) {
            "KAGEMUSHA V1 archive has a truncated Norito header"
        }
        // The generic codec can decompress to the untrusted declared length. Reject noncanonical
        // headers and bound the exact schema-aligned framing before entering that allocation path.
        val compressionOffset = 4 + 1 + 1 + 16
        require(canonical[compressionOffset].toInt() and 0xff == NoritoHeader.COMPRESSION_NONE) {
            "KAGEMUSHA V1 canonical archive must be uncompressed"
        }
        require(canonical[NoritoHeader.HEADER_LENGTH - 1].toInt() and 0xff == NoritoHeader.COMPACT_LEN) {
            "KAGEMUSHA V1 canonical archive has noncanonical layout flags"
        }
        val alignment = canonicalAlignment(schema)
        val padding = (alignment - NoritoHeader.HEADER_LENGTH % alignment) % alignment
        val framingBytes = NoritoHeader.HEADER_LENGTH + padding
        val payloadLength = ByteBuffer.wrap(canonical).order(ByteOrder.LITTLE_ENDIAN)
            .getLong(compressionOffset + 1)
        require(payloadLength >= 0 && payloadLength <= (maximum - framingBytes).toLong()) {
            "KAGEMUSHA V1 declared payload is oversized"
        }
        require(payloadLength == (canonical.size - framingBytes).toLong()) {
            "KAGEMUSHA V1 payload length does not match canonical framing"
        }
        val value = NoritoCodec.decode(canonical, adapter, schema)
        require(encode(value).contentEquals(canonical)) { "KAGEMUSHA V1 archive is not canonical" }
        return value
    }

    private fun bounded(bytes: ByteArray, maximum: Int): ByteArray {
        require(bytes.size <= maximum) { "KAGEMUSHA V1 archive exceeds $maximum bytes" }
        return bytes
    }

    private fun fixedTranscript(expectedSize: Int, vararg parts: ByteArray): ByteArray {
        val transcript = ByteArray(expectedSize)
        var offset = 0
        parts.forEach { part ->
            require(offset + part.size <= transcript.size) { "KAGEMUSHA V1 circuit transcript overflow" }
            part.copyInto(transcript, offset)
            offset += part.size
        }
        require(offset == transcript.size) { "KAGEMUSHA V1 circuit transcript width mismatch" }
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
        KagemushaWireV1.TEXT_PREFIX.length + (rawBytes / 3 * 4) + when (rawBytes % 3) {
            0 -> 0
            1 -> 2
            else -> 3
        }
}
