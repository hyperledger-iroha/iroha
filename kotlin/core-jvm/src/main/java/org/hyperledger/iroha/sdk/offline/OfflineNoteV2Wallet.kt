package org.hyperledger.iroha.sdk.offline

import java.math.BigDecimal
import java.net.URI
import java.net.URLEncoder
import java.nio.charset.StandardCharsets
import java.security.SecureRandom
import java.time.Duration
import java.util.Base64
import java.util.Locale
import java.util.UUID
import java.util.concurrent.CompletableFuture
import java.util.function.LongSupplier
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters
import org.bouncycastle.crypto.signers.Ed25519Signer
import org.hyperledger.iroha.sdk.client.ClientObserver
import org.hyperledger.iroha.sdk.client.ClientResponse
import org.hyperledger.iroha.sdk.client.HttpErrorMessageExtractor
import org.hyperledger.iroha.sdk.client.HttpTransportExecutor
import org.hyperledger.iroha.sdk.client.IrohaClient
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.client.PlatformHttpTransportExecutor
import org.hyperledger.iroha.sdk.client.TransportSecurity
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.crypto.Signer
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter
import org.hyperledger.iroha.sdk.tx.TransactionBuilder
import org.hyperledger.iroha.sdk.tx.norito.NoritoCodecAdapter
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter

/** State persisted for a wallet-owned Offline Note V2 note. */
enum class OfflineNoteV2WalletNoteState {
    SPENDABLE,
    RECEIVE_PENDING,
    SPENT,
    REDEEM_PENDING,
    REDEEMED,
    CANCELLED,
}

/** Structured persisted note record; encrypted stores should serialize this shape. */
class OfflineNoteV2WalletNote @JvmOverloads constructor(
    val chainId: String,
    val accountId: String,
    val assetId: String,
    val amount: String,
    val keyCertificate: OfflineNoteV2.KeyCertificateV2,
    noteCommitment: ByteArray,
    noteSecret: ByteArray,
    val origin: OfflineNoteV2.CommitmentOriginV2,
    val state: OfflineNoteV2WalletNoteState,
    val createdAtMs: Long = 0,
    val updatedAtMs: Long = createdAtMs,
) {
    private val _noteCommitment = noteCommitment.copyOf()
    private val _noteSecret = noteSecret.copyOf()
    val canonicalAmount: String

    init {
        require(chainId.trim().isNotEmpty()) { "chainId must not be blank" }
        require(accountId.trim().isNotEmpty()) { "accountId must not be blank" }
        require(_noteSecret.size == 32) { "note_secret must be exactly 32 bytes" }
        canonicalAmount = OfflineNoteV2.IssuedClaimV2(
            noteCommitment = _noteCommitment,
            keyCertificatePayloadHash = keyCertificate.payloadHash(),
            assetId = assetId,
            amount = amount,
        ).canonicalAmount
    }

    fun noteCommitment(): ByteArray = _noteCommitment.copyOf()
    fun noteSecret(): ByteArray = _noteSecret.copyOf()
    fun noteCommitmentHex(): String = hexLower(_noteCommitment)

    fun issuedClaim(): OfflineNoteV2.IssuedClaimV2 = OfflineNoteV2.IssuedClaimV2(
        noteCommitment = noteCommitment(),
        keyCertificatePayloadHash = keyCertificate.payloadHash(),
        assetId = assetId,
        amount = canonicalAmount,
    )

    fun withState(state: OfflineNoteV2WalletNoteState, updatedAtMs: Long): OfflineNoteV2WalletNote =
        OfflineNoteV2WalletNote(
            chainId = chainId,
            accountId = accountId,
            assetId = assetId,
            amount = canonicalAmount,
            keyCertificate = keyCertificate,
            noteCommitment = noteCommitment(),
            noteSecret = noteSecret(),
            origin = origin,
            state = state,
            createdAtMs = createdAtMs,
            updatedAtMs = updatedAtMs,
        )
}

/** Minimal structured store API for Offline Note V2 wallet notes. */
interface OfflineNoteV2Store {
    fun <T> mutateNotes(mutator: (MutableMap<String, OfflineNoteV2WalletNote>) -> T): T

    fun listNotes(): List<OfflineNoteV2WalletNote> = mutateNotes { it.values.toList() }

    fun findNote(noteCommitment: ByteArray): OfflineNoteV2WalletNote? =
        mutateNotes { it[hexLower(noteCommitment)] }

    fun upsert(note: OfflineNoteV2WalletNote) {
        mutateNotes { it[note.noteCommitmentHex()] = note }
    }
}

/** In-memory store for JVM tests and non-persistent tooling. */
class InMemoryOfflineNoteV2Store : OfflineNoteV2Store {
    private val notes = LinkedHashMap<String, OfflineNoteV2WalletNote>()

    @Synchronized
    override fun <T> mutateNotes(mutator: (MutableMap<String, OfflineNoteV2WalletNote>) -> T): T =
        mutator(notes)
}

/** Supplies wallet-bound Offline Note V2 key certificates. */
interface OfflineNoteV2AttestationProvider {
    fun currentKeyCertificate(): OfflineNoteV2.KeyCertificateV2
}

/** Supplies deterministic random material in tests and secure random material in production. */
interface OfflineNoteV2RandomSource {
    fun nextBytes(length: Int): ByteArray
}

/** Secure random source for note secrets and payment token nonces. */
class SecureOfflineNoteV2RandomSource : OfflineNoteV2RandomSource {
    private val secureRandom = SecureRandom()

    override fun nextBytes(length: Int): ByteArray {
        require(length > 0) { "random byte length must be positive" }
        val bytes = ByteArray(length)
        secureRandom.nextBytes(bytes)
        return bytes
    }
}

/** Generates wallet-local request and operation identifiers. */
interface OfflineNoteV2IdGenerator {
    fun nextId(prefix: String): String
}

/** UUID-backed identifier generator. */
class UuidOfflineNoteV2IdGenerator : OfflineNoteV2IdGenerator {
    override fun nextId(prefix: String): String = "$prefix-${UUID.randomUUID()}"
}

/** Builds recursive proofs for direct audit and redeem transactions. */
interface OfflineNoteV2ProofProvider {
    fun proveAudit(audit: OfflineNoteV2.AuditBundleV2): OfflineNoteV2.RecursiveProofV2
    fun proveRedeem(redemption: OfflineNoteV2.RedeemV2): OfflineNoteV2.RecursiveProofV2
}

/** Verifies recursive proofs before accepting locally-final value. */
interface OfflineNoteV2ProofVerifier {
    fun verifyAudit(audit: OfflineNoteV2.AuditBundleV2): Boolean
    fun verifyRedeem(redemption: OfflineNoteV2.RedeemV2): Boolean
}

/** Halo2-backed Offline Note V2 proof verifier. */
class Halo2OfflineNoteV2ProofVerifier : OfflineNoteV2ProofVerifier {
    override fun verifyAudit(audit: OfflineNoteV2.AuditBundleV2): Boolean =
        OfflineNoteV2Halo2Prover.verifyAudit(audit)

    override fun verifyRedeem(redemption: OfflineNoteV2.RedeemV2): Boolean =
        OfflineNoteV2Halo2Prover.verifyRedeem(redemption)
}

/** Verifies issuer trust and attestation shape for Offline Note V2 key certificates. */
interface OfflineNoteV2CertificateVerifier {
    fun verifyCertificate(certificate: OfflineNoteV2.KeyCertificateV2): Boolean
}

/** Fails closed until a wallet is configured with trusted issuer roots. */
class RejectingOfflineNoteV2CertificateVerifier : OfflineNoteV2CertificateVerifier {
    override fun verifyCertificate(certificate: OfflineNoteV2.KeyCertificateV2): Boolean = false
}

/** Ed25519 verifier for issuer-signed Offline Note V2 key certificates. */
class Ed25519OfflineNoteV2CertificateVerifier(
    trustedIssuerPublicKeys: Collection<ByteArray>,
) : OfflineNoteV2CertificateVerifier {
    private val trustedIssuerPublicKeys = trustedIssuerPublicKeys.map { it.copyOf() }

    override fun verifyCertificate(certificate: OfflineNoteV2.KeyCertificateV2): Boolean {
        if (trustedIssuerPublicKeys.isEmpty()) return false
        if (certificate.platform.trim().isEmpty()) return false
        if (certificate.keyId.trim().isEmpty()) return false
        if (certificate.deviceId.trim().isEmpty()) return false
        if (certificate.assertionScheme.trim().isEmpty()) return false
        if (certificate.assertionKeyAlgorithm.trim().isEmpty()) return false
        if (certificate.assertionPublicKey().isEmpty()) return false
        val message = certificate.signingBytes()
        val signature = certificate.issuerSignature()
        return trustedIssuerPublicKeys.any { root ->
            root.size == 32 && verifyEd25519(root, message, signature)
        }
    }

    private fun verifyEd25519(publicKey: ByteArray, message: ByteArray, signature: ByteArray): Boolean =
        try {
            val verifier = Ed25519Signer()
            verifier.init(false, Ed25519PublicKeyParameters(publicKey, 0))
            verifier.update(message, 0, message.size)
            verifier.verifySignature(signature)
        } catch (ex: RuntimeException) {
            false
        }
}

/** JVM Halo2 proof provider backed by the SDK's native Offline Note V2 prover. */
class NativeOfflineNoteV2ProofProvider : OfflineNoteV2ProofProvider {
    override fun proveAudit(audit: OfflineNoteV2.AuditBundleV2): OfflineNoteV2.RecursiveProofV2 =
        OfflineNoteV2Halo2Prover.proveAudit(audit)

    override fun proveRedeem(redemption: OfflineNoteV2.RedeemV2): OfflineNoteV2.RecursiveProofV2 =
        OfflineNoteV2Halo2Prover.proveRedeem(redemption)
}

/** Torii issuer load context needed before deriving a wallet-owned issue commitment. */
class OfflineNoteV2LoadContext(
    val operationId: String,
    val lineageId: String,
    val localRevision: Long,
    val keyCertificate: OfflineNoteV2.KeyCertificateV2,
)

/** Request sent to an issuer adapter after the wallet derives a note commitment. */
class OfflineNoteV2IssueRequest(
    val chainId: String,
    val accountId: String,
    val assetDefinitionId: String,
    val assetId: String,
    val amount: String,
    val loadContext: OfflineNoteV2LoadContext,
    noteCommitment: ByteArray,
) {
    private val _noteCommitment = noteCommitment.copyOf()

    fun noteCommitment(): ByteArray = _noteCommitment.copyOf()
    fun noteCommitmentHex(): String = hexLower(_noteCommitment)
}

/** Issuer response after Torii accepts the supplied note commitment. */
class OfflineNoteV2IssueResponse @JvmOverloads constructor(
    noteCommitment: ByteArray,
    val operationId: String,
    val lineageId: String,
    val localRevision: Long,
    val keyCertificate: OfflineNoteV2.KeyCertificateV2? = null,
    val settlementEntryHashHex: String? = null,
) {
    private val _noteCommitment = noteCommitment.copyOf()

    fun noteCommitment(): ByteArray = _noteCommitment.copyOf()
}

/** Adapter boundary for Torii issuer key refill and note issue calls. */
interface OfflineNoteV2IssuerClient {
    fun prepareLoad(
        chainId: String,
        accountId: String,
        assetDefinitionId: String,
        amount: String,
    ): CompletableFuture<OfflineNoteV2LoadContext>

    fun issueNote(request: OfflineNoteV2IssueRequest): CompletableFuture<OfflineNoteV2IssueResponse>
}

/** Receiver request handed to a payer; it contains no note secret. */
class OfflineNoteV2ReceiveRequest(
    val chainId: String,
    val paymentRequestId: String,
    val accountId: String,
    val assetDefinitionId: String,
    val assetId: String,
    val amount: String,
    val keyCertificate: OfflineNoteV2.KeyCertificateV2,
    outputCommitment: ByteArray,
) {
    private val _outputCommitment = outputCommitment.copyOf()
    val canonicalAmount: String = OfflineNoteV2.AuditOutputClaimV2(
        noteCommitment = _outputCommitment,
        keyCertificate = keyCertificate,
        assetId = assetId,
        amount = amount,
    ).canonicalAmount

    fun outputCommitment(): ByteArray = _outputCommitment.copyOf()
    fun outputCommitmentHex(): String = hexLower(_outputCommitment)
}

/** QR/Norito handoff codec for Offline Note V2 receive requests. */
object OfflineNoteV2ReceiveRequestCodec {
    const val TYPE: String = "offline_receive_request_v2"
    const val VERSION: Long = 2
    const val TEXT_PREFIX: String = "wallet-offline-receive-v2:"
    private const val RECEIVE_REQUEST_ENVELOPE_SCHEMA =
        "iroha_data_model::offline::model::OfflineNoteReceiveRequestEnvelopeV2"

    @JvmStatic
    fun encodeNorito(request: OfflineNoteV2ReceiveRequest): ByteArray =
        NoritoCodec.encode(request, RECEIVE_REQUEST_ENVELOPE_SCHEMA, ReceiveRequestAdapter, NoritoHeader.COMPACT_LEN)

    @JvmStatic
    fun decodeNorito(payload: ByteArray): OfflineNoteV2ReceiveRequest =
        NoritoCodec.decode(payload, ReceiveRequestAdapter, RECEIVE_REQUEST_ENVELOPE_SCHEMA)

    @JvmStatic
    fun encodeText(request: OfflineNoteV2ReceiveRequest): String =
        TEXT_PREFIX + Base64.getUrlEncoder().withoutPadding().encodeToString(encodeNorito(request))

    @JvmStatic
    fun decodeText(text: String): OfflineNoteV2ReceiveRequest {
        val trimmed = text.trim()
        require(trimmed.startsWith(TEXT_PREFIX)) { "Offline Note V2 receive request prefix missing" }
        return decodeNorito(Base64.getUrlDecoder().decode(trimmed.substring(TEXT_PREFIX.length)))
    }

    @JvmStatic
    fun encodeQrFrameBytes(request: OfflineNoteV2ReceiveRequest): List<ByteArray> =
        encodeQrFrameBytes(request, OfflineQrStream.Options())

    @JvmStatic
    fun encodeQrFrameBytes(
        request: OfflineNoteV2ReceiveRequest,
        options: OfflineQrStream.Options,
    ): List<ByteArray> =
        OfflineQrStream.Encoder.encodeFrameBytes(
            encodeNorito(request),
            OfflineQrStream.PayloadKind.OFFLINE_RECEIVE_REQUEST_V2,
            options,
        )

    @JvmStatic
    fun decodeQrPayload(payload: ByteArray): OfflineNoteV2ReceiveRequest = decodeNorito(payload)

    private object ReceiveRequestAdapter : TypeAdapter<OfflineNoteV2ReceiveRequest> {
        override fun encode(encoder: NoritoEncoder, value: OfflineNoteV2ReceiveRequest) {
            writeField(encoder) { it.writeUInt(VERSION, 64) }
            writeField(encoder) { writeString(it, value.chainId) }
            writeField(encoder) { writeString(it, value.paymentRequestId) }
            writeField(encoder) { writeString(it, value.accountId) }
            writeField(encoder) { writeString(it, value.assetDefinitionId) }
            writeField(encoder) { writeString(it, value.assetId) }
            writeField(encoder) { writeString(it, value.canonicalAmount) }
            writeField(encoder) { writeBytesVec(it, value.keyCertificate.noritoEncoded()) }
            writeField(encoder) { it.writeBytes(value.outputCommitment()) }
        }

        override fun decode(decoder: NoritoDecoder): OfflineNoteV2ReceiveRequest {
            val version = readField(decoder) { it.readUInt(64) }
            require(version == VERSION) { "Offline Note V2 receive request Norito version must be $VERSION" }
            val chainId = readField(decoder) { readString(it) }
            val paymentRequestId = readField(decoder) { readString(it) }
            val accountId = readField(decoder) { readString(it) }
            val assetDefinitionId = readField(decoder) { readString(it) }
            val assetId = readField(decoder) { readString(it) }
            val amount = readField(decoder) { readString(it) }
            val keyCertificate = OfflineNoteV2.decodeCertificate(readField(decoder) { readBytesVec(it) })
            val outputCommitment = readField(decoder) { it.readBytes(32) }
            return OfflineNoteV2ReceiveRequest(
                chainId = chainId,
                paymentRequestId = paymentRequestId,
                accountId = accountId,
                assetDefinitionId = assetDefinitionId,
                assetId = assetId,
                amount = amount,
                keyCertificate = keyCertificate,
                outputCommitment = outputCommitment,
            )
        }
    }

    private fun writeField(encoder: NoritoEncoder, write: (NoritoEncoder) -> Unit) {
        val child = encoder.childEncoder()
        write(child)
        val payload = child.toByteArray()
        encoder.writeLength(payload.size.toLong(), true)
        encoder.writeBytes(payload)
    }

    private fun writeString(encoder: NoritoEncoder, value: String) {
        val bytes = value.toByteArray(StandardCharsets.UTF_8)
        encoder.writeLength(bytes.size.toLong(), true)
        encoder.writeBytes(bytes)
    }

    private fun writeBytesVec(encoder: NoritoEncoder, value: ByteArray) {
        encoder.writeUInt(value.size.toLong(), 64)
        encoder.writeBytes(value)
    }

    private fun <T> readField(decoder: NoritoDecoder, read: (NoritoDecoder) -> T): T {
        val length = decoder.readLength(true)
        require(length <= Int.MAX_VALUE) { "Offline Note V2 receive request field length overflow" }
        val child = NoritoDecoder(decoder.readBytes(length.toInt()), decoder.flags, decoder.flagsHint)
        val value = read(child)
        require(child.remaining() == 0) { "Trailing bytes after Offline Note V2 receive request field decode" }
        return value
    }

    private fun readString(decoder: NoritoDecoder): String {
        val length = decoder.readLength(true)
        require(length <= Int.MAX_VALUE) { "Offline Note V2 receive request string length overflow" }
        val value = String(decoder.readBytes(length.toInt()), StandardCharsets.UTF_8)
        require(value.isNotBlank()) { "Offline Note V2 receive request string must not be blank" }
        return value
    }

    private fun readBytesVec(decoder: NoritoDecoder): ByteArray {
        val length = decoder.readUInt(64)
        require(length <= Int.MAX_VALUE) { "Offline Note V2 receive request bytes length overflow" }
        return decoder.readBytes(length.toInt())
    }
}

/** Payment token produced by a payer and accepted by the recipient. */
class OfflineNoteV2PaymentToken(
    val chainId: String,
    val paymentRequestId: String,
    tokenNonce: ByteArray,
    tokenId: ByteArray,
    val audit: OfflineNoteV2.AuditBundleV2,
    val createdAtMs: Long,
) {
    private val _tokenNonce = tokenNonce.copyOf()
    private val _tokenId = tokenId.copyOf()

    fun tokenNonce(): ByteArray = _tokenNonce.copyOf()
    fun tokenId(): ByteArray = _tokenId.copyOf()
    fun tokenIdHex(): String = hexLower(_tokenId)
}

/** QR/Norito handoff codec for Offline Note V2 payment tokens. */
object OfflineNoteV2PaymentTokenCodec {
    const val TYPE: String = "offline_payment_token_v2"
    const val VERSION: Long = 2
    const val TEXT_PREFIX: String = "wallet-offline-payment-v2:"
    private const val TOKEN_ENVELOPE_SCHEMA =
        "iroha_data_model::offline::model::OfflineNotePaymentTokenEnvelopeV2"

    @JvmStatic
    fun encodeNorito(token: OfflineNoteV2PaymentToken): ByteArray =
        NoritoCodec.encode(token, TOKEN_ENVELOPE_SCHEMA, PaymentTokenAdapter, NoritoHeader.COMPACT_LEN)

    @JvmStatic
    fun decodeNorito(payload: ByteArray): OfflineNoteV2PaymentToken =
        NoritoCodec.decode(payload, PaymentTokenAdapter, TOKEN_ENVELOPE_SCHEMA)

    @JvmStatic
    fun encodeJson(token: OfflineNoteV2PaymentToken): ByteArray = encodeNorito(token)

    @JvmStatic
    fun decodeJson(payload: ByteArray): OfflineNoteV2PaymentToken = decodeNorito(payload)

    @JvmStatic
    fun encodeText(token: OfflineNoteV2PaymentToken): String =
        TEXT_PREFIX + Base64.getUrlEncoder().withoutPadding().encodeToString(encodeNorito(token))

    @JvmStatic
    fun decodeText(text: String): OfflineNoteV2PaymentToken {
        val trimmed = text.trim()
        require(trimmed.startsWith(TEXT_PREFIX)) { "Offline Note V2 payment token prefix missing" }
        return decodeNorito(Base64.getUrlDecoder().decode(trimmed.substring(TEXT_PREFIX.length)))
    }

    @JvmStatic
    fun encodeQrFrameBytes(token: OfflineNoteV2PaymentToken): List<ByteArray> =
        encodeQrFrameBytes(token, OfflineQrStream.Options())

    @JvmStatic
    fun encodeQrFrameBytes(
        token: OfflineNoteV2PaymentToken,
        options: OfflineQrStream.Options,
    ): List<ByteArray> =
        OfflineQrStream.Encoder.encodeFrameBytes(
            encodeNorito(token),
            OfflineQrStream.PayloadKind.OFFLINE_PAYMENT_TOKEN_V2,
            options,
        )

    @JvmStatic
    fun decodeQrPayload(payload: ByteArray): OfflineNoteV2PaymentToken = decodeNorito(payload)

    private object PaymentTokenAdapter : TypeAdapter<OfflineNoteV2PaymentToken> {
        override fun encode(encoder: NoritoEncoder, value: OfflineNoteV2PaymentToken) {
            writeField(encoder) { it.writeUInt(VERSION, 64) }
            writeField(encoder) { writeString(it, value.chainId) }
            writeField(encoder) { writeString(it, value.paymentRequestId) }
            writeField(encoder) { it.writeUInt(value.createdAtMs, 64) }
            writeField(encoder) { writeBytesVec(it, value.tokenNonce()) }
            writeField(encoder) { it.writeBytes(value.tokenId()) }
            writeField(encoder) { writeBytesVec(it, value.audit.noritoEncoded()) }
        }

        override fun decode(decoder: NoritoDecoder): OfflineNoteV2PaymentToken {
            val version = readField(decoder) { it.readUInt(64) }
            require(version == VERSION) { "Offline Note V2 payment token Norito version must be $VERSION" }
            val chainId = readField(decoder) { readString(it) }
            val paymentRequestId = readField(decoder) { readString(it) }
            val createdAtMs = readField(decoder) { it.readUInt(64) }
            val tokenNonce = readField(decoder) { readBytesVec(it) }
            val tokenId = readField(decoder) { it.readBytes(32) }
            val audit = OfflineNoteV2.decodeAudit(readField(decoder) { readBytesVec(it) })
            require(audit.tokenId().contentEquals(tokenId)) {
                "Offline Note V2 payment token id does not match audit bundle"
            }
            return OfflineNoteV2PaymentToken(
                chainId = chainId,
                paymentRequestId = paymentRequestId,
                tokenNonce = tokenNonce,
                tokenId = tokenId,
                audit = audit,
                createdAtMs = createdAtMs,
            )
        }
    }

    private fun writeField(encoder: NoritoEncoder, write: (NoritoEncoder) -> Unit) {
        val child = encoder.childEncoder()
        write(child)
        val payload = child.toByteArray()
        encoder.writeLength(payload.size.toLong(), true)
        encoder.writeBytes(payload)
    }

    private fun writeString(encoder: NoritoEncoder, value: String) {
        val bytes = value.toByteArray(StandardCharsets.UTF_8)
        encoder.writeLength(bytes.size.toLong(), true)
        encoder.writeBytes(bytes)
    }

    private fun writeBytesVec(encoder: NoritoEncoder, value: ByteArray) {
        encoder.writeUInt(value.size.toLong(), 64)
        encoder.writeBytes(value)
    }

    private fun <T> readField(decoder: NoritoDecoder, read: (NoritoDecoder) -> T): T {
        val length = decoder.readLength(true)
        require(length <= Int.MAX_VALUE) { "Offline Note V2 payment token field length overflow" }
        val child = NoritoDecoder(decoder.readBytes(length.toInt()), decoder.flags, decoder.flagsHint)
        val value = read(child)
        require(child.remaining() == 0) { "Trailing bytes after Offline Note V2 payment token field decode" }
        return value
    }

    private fun readString(decoder: NoritoDecoder): String {
        val length = decoder.readLength(true)
        require(length <= Int.MAX_VALUE) { "Offline Note V2 payment token string length overflow" }
        val value = String(decoder.readBytes(length.toInt()), StandardCharsets.UTF_8)
        require(value.isNotBlank()) { "Offline Note V2 payment token string must not be blank" }
        return value
    }

    private fun readBytesVec(decoder: NoritoDecoder): ByteArray {
        val length = decoder.readUInt(64)
        require(length <= Int.MAX_VALUE) { "Offline Note V2 payment token bytes length overflow" }
        return decoder.readBytes(length.toInt())
    }
}

/** Receipt ACK returned by a recipient after accepting an Offline Note V2 payment token. */
class OfflineNoteV2ReceiptAck(
    val chainId: String,
    val paymentRequestId: String,
    tokenId: ByteArray,
    val recipientAccountId: String,
    val acceptedAtMs: Long,
) {
    private val _tokenId = tokenId.copyOf()

    init {
        require(chainId.isNotBlank()) { "chainId must not be blank" }
        require(paymentRequestId.isNotBlank()) { "paymentRequestId must not be blank" }
        require(_tokenId.size == 32) { "tokenId must be 32 bytes" }
        require(recipientAccountId.isNotBlank()) { "recipientAccountId must not be blank" }
        require(acceptedAtMs >= 0L) { "acceptedAtMs must be non-negative" }
    }

    fun tokenId(): ByteArray = _tokenId.copyOf()
    fun tokenIdHex(): String = hexLower(_tokenId)

    fun matchesPaymentToken(token: OfflineNoteV2PaymentToken): Boolean =
        chainId == token.chainId &&
            paymentRequestId == token.paymentRequestId &&
            _tokenId.contentEquals(token.tokenId()) &&
            receiptAckTokenHasRecipientOutput(token, recipientAccountId)

    fun requireMatchesPaymentToken(token: OfflineNoteV2PaymentToken) {
        require(matchesPaymentToken(token)) { "receipt ACK does not match payment token" }
    }

    companion object {
        @JvmStatic
        fun fromPaymentToken(
            token: OfflineNoteV2PaymentToken,
            recipientAccountId: String,
            acceptedAtMs: Long,
        ): OfflineNoteV2ReceiptAck {
            val checkedRecipient = recipientAccountId.trim()
            require(checkedRecipient.isNotEmpty()) { "recipientAccountId must not be blank" }
            require(receiptAckTokenHasRecipientOutput(token, checkedRecipient)) {
                "payment token does not contain recipient output"
            }
            return OfflineNoteV2ReceiptAck(
                chainId = token.chainId,
                paymentRequestId = token.paymentRequestId,
                tokenId = token.tokenId(),
                recipientAccountId = checkedRecipient,
                acceptedAtMs = acceptedAtMs,
            )
        }
    }
}

private fun receiptAckTokenHasRecipientOutput(
    token: OfflineNoteV2PaymentToken,
    recipientAccountId: String,
): Boolean =
    token.audit.outputClaims.any { it.keyCertificate.accountId == recipientAccountId }

/** QR/Norito handoff codec for Offline Note V2 receipt ACKs. */
object OfflineNoteV2ReceiptAckCodec {
    const val TYPE: String = "offline_receipt_ack_v2"
    const val VERSION: Long = 2
    const val TEXT_PREFIX: String = "wallet-offline-ack-v2:"
    private const val RECEIPT_ACK_ENVELOPE_SCHEMA =
        "iroha_data_model::offline::model::OfflineNoteReceiptAckEnvelopeV2"

    @JvmStatic
    fun encodeNorito(ack: OfflineNoteV2ReceiptAck): ByteArray =
        NoritoCodec.encode(ack, RECEIPT_ACK_ENVELOPE_SCHEMA, ReceiptAckAdapter, NoritoHeader.COMPACT_LEN)

    @JvmStatic
    fun decodeNorito(payload: ByteArray): OfflineNoteV2ReceiptAck =
        NoritoCodec.decode(payload, ReceiptAckAdapter, RECEIPT_ACK_ENVELOPE_SCHEMA)

    @JvmStatic
    fun encodeText(ack: OfflineNoteV2ReceiptAck): String =
        TEXT_PREFIX + Base64.getUrlEncoder().withoutPadding().encodeToString(encodeNorito(ack))

    @JvmStatic
    fun decodeText(text: String): OfflineNoteV2ReceiptAck {
        val trimmed = text.trim()
        require(trimmed.startsWith(TEXT_PREFIX)) { "Offline Note V2 receipt ACK prefix missing" }
        return decodeNorito(Base64.getUrlDecoder().decode(trimmed.substring(TEXT_PREFIX.length)))
    }

    @JvmStatic
    fun encodeQrFrameBytes(ack: OfflineNoteV2ReceiptAck): List<ByteArray> =
        encodeQrFrameBytes(ack, OfflineQrStream.Options())

    @JvmStatic
    fun encodeQrFrameBytes(
        ack: OfflineNoteV2ReceiptAck,
        options: OfflineQrStream.Options,
    ): List<ByteArray> =
        OfflineQrStream.Encoder.encodeFrameBytes(
            encodeNorito(ack),
            OfflineQrStream.PayloadKind.OFFLINE_RECEIPT_ACK_V2,
            options,
        )

    @JvmStatic
    fun decodeQrPayload(payload: ByteArray): OfflineNoteV2ReceiptAck = decodeNorito(payload)

    private object ReceiptAckAdapter : TypeAdapter<OfflineNoteV2ReceiptAck> {
        override fun encode(encoder: NoritoEncoder, value: OfflineNoteV2ReceiptAck) {
            writeField(encoder) { it.writeUInt(VERSION, 64) }
            writeField(encoder) { writeString(it, value.chainId) }
            writeField(encoder) { writeString(it, value.paymentRequestId) }
            writeField(encoder) { it.writeBytes(value.tokenId()) }
            writeField(encoder) { writeString(it, value.recipientAccountId) }
            writeField(encoder) { it.writeUInt(value.acceptedAtMs, 64) }
        }

        override fun decode(decoder: NoritoDecoder): OfflineNoteV2ReceiptAck {
            val version = readField(decoder) { it.readUInt(64) }
            require(version == VERSION) { "Offline Note V2 receipt ACK Norito version must be $VERSION" }
            val chainId = readField(decoder) { readString(it) }
            val paymentRequestId = readField(decoder) { readString(it) }
            val tokenId = readField(decoder) { it.readBytes(32) }
            val recipientAccountId = readField(decoder) { readString(it) }
            val acceptedAtMs = readField(decoder) { it.readUInt(64) }
            return OfflineNoteV2ReceiptAck(
                chainId = chainId,
                paymentRequestId = paymentRequestId,
                tokenId = tokenId,
                recipientAccountId = recipientAccountId,
                acceptedAtMs = acceptedAtMs,
            )
        }
    }

    private fun writeField(encoder: NoritoEncoder, write: (NoritoEncoder) -> Unit) {
        val child = encoder.childEncoder()
        write(child)
        val payload = child.toByteArray()
        encoder.writeLength(payload.size.toLong(), true)
        encoder.writeBytes(payload)
    }

    private fun writeString(encoder: NoritoEncoder, value: String) {
        val bytes = value.toByteArray(StandardCharsets.UTF_8)
        encoder.writeLength(bytes.size.toLong(), true)
        encoder.writeBytes(bytes)
    }

    private fun <T> readField(decoder: NoritoDecoder, read: (NoritoDecoder) -> T): T {
        val length = decoder.readLength(true)
        require(length <= Int.MAX_VALUE) { "Offline Note V2 receipt ACK field length overflow" }
        val child = NoritoDecoder(decoder.readBytes(length.toInt()), decoder.flags, decoder.flagsHint)
        val value = read(child)
        require(child.remaining() == 0) { "Trailing bytes after Offline Note V2 receipt ACK field decode" }
        return value
    }

    private fun readString(decoder: NoritoDecoder): String {
        val length = decoder.readLength(true)
        require(length <= Int.MAX_VALUE) { "Offline Note V2 receipt ACK string length overflow" }
        val value = String(decoder.readBytes(length.toInt()), StandardCharsets.UTF_8)
        require(value.isNotBlank()) { "Offline Note V2 receipt ACK string must not be blank" }
        return value
    }
}

/** Submits direct Offline Note V2 audit/redeem transactions. */
interface OfflineNoteV2TransactionSubmitter {
    fun submitAudit(audit: OfflineNoteV2.AuditBundleV2): CompletableFuture<ClientResponse>
    fun submitRedeem(redemption: OfflineNoteV2.RedeemV2): CompletableFuture<ClientResponse>
}

/** Resolution returned by a wallet sync resolver for one pending Offline Note V2 note. */
class OfflineNoteV2SyncResolution @JvmOverloads constructor(
    val state: OfflineNoteV2WalletNoteState,
    val transactionHashHex: String? = null,
)

/** Looks up transaction-outcome state for pending wallet notes. */
interface OfflineNoteV2SyncResolver {
    fun resolvePendingNote(note: OfflineNoteV2WalletNote): CompletableFuture<OfflineNoteV2SyncResolution?>
}

/** One explorer instruction outcome used by Offline Note V2 wallet reconciliation. */
class OfflineNoteV2ExplorerInstructionOutcome @JvmOverloads constructor(
    val kind: String,
    val transactionStatus: String,
    val transactionHashHex: String? = null,
    encodedInstruction: ByteArray,
) {
    private val _encodedInstruction = encodedInstruction.copyOf()

    init {
        require(kind.trim().isNotEmpty()) { "kind must not be blank" }
        require(transactionStatus.trim().isNotEmpty()) { "transactionStatus must not be blank" }
        require(_encodedInstruction.isNotEmpty()) { "encodedInstruction must not be empty" }
    }

    fun encodedInstruction(): ByteArray = _encodedInstruction.copyOf()
}

/** Supplies recent Offline Note V2 explorer outcomes for resolver-backed wallet sync. */
interface OfflineNoteV2OutcomeProvider {
    fun listOutcomes(): CompletableFuture<List<OfflineNoteV2ExplorerInstructionOutcome>>
}

/** Outcome index that maps committed/rejected Offline Note V2 instructions to note states. */
class OfflineNoteV2OutcomeIndex {
    private val committedRedeems = LinkedHashMap<String, String?>()
    private val rejectedRedeems = LinkedHashMap<String, String?>()

    fun recordCommittedAudit(audit: OfflineNoteV2.AuditBundleV2, transactionHashHex: String?): OfflineNoteV2OutcomeIndex {
        return this
    }

    fun recordRejectedAudit(audit: OfflineNoteV2.AuditBundleV2, transactionHashHex: String?): OfflineNoteV2OutcomeIndex {
        return this
    }

    fun recordCommittedRedeem(redeem: OfflineNoteV2.RedeemV2, transactionHashHex: String?): OfflineNoteV2OutcomeIndex {
        putFirst(committedRedeems, redeem.sourceNoteCommitment(), transactionHashHex)
        return this
    }

    fun recordRejectedRedeem(redeem: OfflineNoteV2.RedeemV2, transactionHashHex: String?): OfflineNoteV2OutcomeIndex {
        putFirst(rejectedRedeems, redeem.sourceNoteCommitment(), transactionHashHex)
        return this
    }

    fun resolve(note: OfflineNoteV2WalletNote): OfflineNoteV2SyncResolution? =
        when (note.state) {
            OfflineNoteV2WalletNoteState.REDEEM_PENDING -> resolveRedeemPending(note)
            else -> null
        }

    private fun resolveRedeemPending(note: OfflineNoteV2WalletNote): OfflineNoteV2SyncResolution? {
        val commitmentKey = note.noteCommitmentHex()
        if (committedRedeems.containsKey(commitmentKey)) {
            return OfflineNoteV2SyncResolution(
                OfflineNoteV2WalletNoteState.REDEEMED,
                committedRedeems[commitmentKey],
            )
        }
        if (rejectedRedeems.containsKey(commitmentKey)) {
            return OfflineNoteV2SyncResolution(
                OfflineNoteV2WalletNoteState.SPENDABLE,
                rejectedRedeems[commitmentKey],
            )
        }
        return null
    }

    private fun putFirst(target: MutableMap<String, String?>, bytes: ByteArray, transactionHashHex: String?) {
        val key = hexLower(bytes)
        if (!target.containsKey(key)) {
            target[key] = transactionHashHex
        }
    }

    companion object {
        const val KIND_ISSUE: String = "IssueOfflineNoteV2"
        const val KIND_REDEEM: String = "RedeemOfflineNoteV2"
        const val KIND_AUDIT: String = "AuditOfflineNoteV2"

        @JvmStatic
        fun fromExplorerOutcomes(outcomes: List<OfflineNoteV2ExplorerInstructionOutcome>): OfflineNoteV2OutcomeIndex {
            val index = OfflineNoteV2OutcomeIndex()
            for (outcome in outcomes) {
                val committed = outcome.transactionStatus.equals("committed", ignoreCase = true)
                val rejected = outcome.transactionStatus.equals("rejected", ignoreCase = true)
                if (!committed && !rejected) continue
                when {
                    outcome.kind.equals(KIND_AUDIT, ignoreCase = true) -> {
                        val audit = OfflineNoteV2.decodeAuditInstruction(outcome.encodedInstruction())
                        if (committed) {
                            index.recordCommittedAudit(audit, outcome.transactionHashHex)
                        } else {
                            index.recordRejectedAudit(audit, outcome.transactionHashHex)
                        }
                    }
                    outcome.kind.equals(KIND_REDEEM, ignoreCase = true) -> {
                        val redeem = OfflineNoteV2.decodeRedeemInstruction(outcome.encodedInstruction())
                        if (committed) {
                            index.recordCommittedRedeem(redeem, outcome.transactionHashHex)
                        } else {
                            index.recordRejectedRedeem(redeem, outcome.transactionHashHex)
                        }
                    }
                }
            }
            return index
        }
    }
}

/** Sync resolver that rebuilds an outcome index from a provider for each wallet sync pass. */
class OfflineNoteV2OutcomeIndexSyncResolver(
    private val provider: OfflineNoteV2OutcomeProvider,
) : OfflineNoteV2SyncResolver {
    override fun resolvePendingNote(
        note: OfflineNoteV2WalletNote,
    ): CompletableFuture<OfflineNoteV2SyncResolution?> =
        provider.listOutcomes().thenApply { OfflineNoteV2OutcomeIndex.fromExplorerOutcomes(it).resolve(note) }
}

/** Torii explorer-backed provider for Offline Note V2 wallet reconciliation outcomes. */
class ToriiOfflineNoteV2OutcomeProvider @JvmOverloads constructor(
    private val executor: HttpTransportExecutor = PlatformHttpTransportExecutor.createDefault(),
    private val baseUri: URI = URI.create("http://localhost:8080"),
    private val timeout: Duration? = Duration.ofSeconds(15),
    defaultHeaders: Map<String, String> = emptyMap(),
    observers: List<ClientObserver> = emptyList(),
    private val perPage: Int = 100,
) : OfflineNoteV2OutcomeProvider {
    private val defaultHeaders: Map<String, String> = LinkedHashMap(defaultHeaders)
    private val observers: List<ClientObserver> = observers.toList()

    override fun listOutcomes(): CompletableFuture<List<OfflineNoteV2ExplorerInstructionOutcome>> {
        val audit = fetchKind(OfflineNoteV2OutcomeIndex.KIND_AUDIT)
        val redeem = fetchKind(OfflineNoteV2OutcomeIndex.KIND_REDEEM)
        return CompletableFuture.allOf(audit, redeem).thenApply {
            audit.join() + redeem.join()
        }
    }

    private fun fetchKind(kind: String): CompletableFuture<List<OfflineNoteV2ExplorerInstructionOutcome>> {
        val request = buildGetRequest(
            "/v1/explorer/instructions",
            linkedMapOf("kind" to kind, "per_page" to perPage.toString()),
        )
        notifyRequest(request)
        val future = CompletableFuture<List<OfflineNoteV2ExplorerInstructionOutcome>>()
        executor.execute(request).whenComplete { response, throwable ->
            if (throwable != null) {
                val error = OfflineToriiException(
                    "Offline Note V2 outcome lookup failed: ${throwable.message ?: throwable.javaClass.simpleName}",
                    throwable,
                    null,
                    null,
                    null,
                )
                notifyFailure(request, error)
                future.completeExceptionally(error)
                return@whenComplete
            }
            val clientResponse = ClientResponse(
                response.statusCode,
                response.body,
                response.message,
                null,
                HttpErrorMessageExtractor.extractRejectCode(
                    response.headers,
                    "x-iroha-reject-code",
                    response.body,
                ),
            )
            if (response.statusCode < 200 || response.statusCode >= 300) {
                val error = OfflineToriiException(
                    "Offline Note V2 outcome lookup failed with HTTP ${response.statusCode}",
                    response.statusCode,
                    clientResponse.rejectCode,
                    HttpErrorMessageExtractor.extractMessage(response.body),
                )
                notifyFailure(request, error)
                future.completeExceptionally(error)
                return@whenComplete
            }
            try {
                val parsed = parseExplorerOutcomes(response.body)
                notifyResponse(request, clientResponse)
                future.complete(parsed)
            } catch (ex: RuntimeException) {
                val error = OfflineToriiException(
                    "Failed to parse Offline Note V2 explorer outcomes",
                    ex,
                    response.statusCode,
                    clientResponse.rejectCode,
                    HttpErrorMessageExtractor.extractMessage(response.body),
                )
                notifyFailure(request, error)
                future.completeExceptionally(error)
            }
        }
        return future
    }

    private fun buildGetRequest(path: String, queryParams: Map<String, String>): TransportRequest {
        val target = appendQuery(resolvePath(path), queryParams)
        val headers = mergeHeaders()
        TransportSecurity.requireHttpRequestAllowed(
            "ToriiOfflineNoteV2OutcomeProvider",
            baseUri,
            target,
            headers,
            null,
        )
        val builder = TransportRequest.builder().setUri(target).setMethod("GET").setTimeout(timeout)
        headers.forEach { (name, value) -> builder.addHeader(name, value) }
        return builder.build()
    }

    private fun resolvePath(path: String): URI {
        val normalized = if (path.startsWith("/")) path.substring(1) else path
        val base = baseUri.toString()
        return URI.create(if (base.endsWith("/")) base + normalized else "$base/$normalized")
    }

    private fun appendQuery(uri: URI, params: Map<String, String>): URI {
        if (params.isEmpty()) return uri
        val query = params.entries.joinToString("&") {
            "${urlEncode(it.key)}=${urlEncode(it.value)}"
        }
        val base = uri.toString()
        val separator = if (base.contains("?")) "&" else "?"
        return URI.create(base + separator + query)
    }

    private fun urlEncode(value: String): String =
        URLEncoder.encode(value, StandardCharsets.UTF_8.name())

    private fun mergeHeaders(): Map<String, String> {
        val headers = LinkedHashMap(defaultHeaders)
        headers[findHeader(headers, "Accept") ?: "Accept"] = "application/json"
        return headers
    }

    private fun parseExplorerOutcomes(payload: ByteArray): List<OfflineNoteV2ExplorerInstructionOutcome> {
        val parsed = JsonParser.parse(String(payload, StandardCharsets.UTF_8))
        val root = requireObject(parsed, "explorer response")
        val items = root["items"] as? List<*> ?: throw IllegalArgumentException("items must be an array")
        return items.map { item ->
            val obj = requireObject(item, "instruction item")
            val box = requireObject(obj["r#box"] ?: obj["box"], "instruction box")
            val encoded = box["encoded"] as? String
                ?: requireNestedEncoded(box)
            OfflineNoteV2ExplorerInstructionOutcome(
                kind = requiredString(obj, "kind"),
                transactionStatus = requiredString(obj, "transaction_status"),
                transactionHashHex = obj["transaction_hash"] as? String,
                encodedInstruction = hexBytes(encoded, "encoded"),
            )
        }
    }

    private fun requireNestedEncoded(box: Map<String, Any?>): String {
        val json = box["json"] as? Map<*, *> ?: throw IllegalArgumentException("instruction box encoded payload missing")
        val encoded = json["encoded"] as? String ?: throw IllegalArgumentException("instruction box encoded payload missing")
        return encoded
    }

    private fun notifyRequest(request: TransportRequest) {
        observers.forEach { it.onRequest(request) }
    }

    private fun notifyResponse(request: TransportRequest, response: ClientResponse) {
        observers.forEach { it.onResponse(request, response) }
    }

    private fun notifyFailure(request: TransportRequest, error: Throwable) {
        observers.forEach { it.onFailure(request, error) }
    }

    private fun findHeader(headers: Map<String, String>, name: String): String? =
        headers.keys.firstOrNull { it.equals(name, ignoreCase = true) }
}

/** Transaction submitter that wraps Offline V2 instructions in signed Iroha transactions. */
class IrohaOfflineNoteV2TransactionSubmitter @JvmOverloads constructor(
    private val client: IrohaClient,
    private val signer: Signer,
    private val chainId: String,
    private val authority: String,
    private val codecAdapter: NoritoCodecAdapter = NoritoJavaCodecAdapter(),
    private val clock: LongSupplier = LongSupplier { System.currentTimeMillis() },
) : OfflineNoteV2TransactionSubmitter {
    private val transactionBuilder = TransactionBuilder(codecAdapter)

    override fun submitAudit(audit: OfflineNoteV2.AuditBundleV2): CompletableFuture<ClientResponse> =
        submit(OfflineNoteV2.auditInstruction(audit))

    override fun submitRedeem(redemption: OfflineNoteV2.RedeemV2): CompletableFuture<ClientResponse> =
        submit(OfflineNoteV2.redeemInstruction(redemption))

    private fun submit(instruction: InstructionBox): CompletableFuture<ClientResponse> {
        val payload = TransactionPayload(
            chainId = chainId,
            authority = authority,
            creationTimeMs = clock.getAsLong(),
            executable = Executable.instructions(listOf(instruction)),
        )
        return client.submitTransaction(transactionBuilder.encodeAndSign(payload, signer))
    }
}

/** One-call Offline Note V2 wallet facade for load, receive, pay, accept, redeem, and sync. */
class OfflineNoteV2Wallet @JvmOverloads constructor(
    private val chainId: String,
    private val accountId: String,
    private val attestationProvider: OfflineNoteV2AttestationProvider,
    private val store: OfflineNoteV2Store = InMemoryOfflineNoteV2Store(),
    private val issuerClient: OfflineNoteV2IssuerClient? = null,
    private val transactionSubmitter: OfflineNoteV2TransactionSubmitter? = null,
    private val syncResolver: OfflineNoteV2SyncResolver? = null,
    private val proofProvider: OfflineNoteV2ProofProvider = NativeOfflineNoteV2ProofProvider(),
    private val proofVerifier: OfflineNoteV2ProofVerifier = Halo2OfflineNoteV2ProofVerifier(),
    private val certificateVerifier: OfflineNoteV2CertificateVerifier = RejectingOfflineNoteV2CertificateVerifier(),
    private val randomSource: OfflineNoteV2RandomSource = SecureOfflineNoteV2RandomSource(),
    private val idGenerator: OfflineNoteV2IdGenerator = UuidOfflineNoteV2IdGenerator(),
    private val clock: LongSupplier = LongSupplier { System.currentTimeMillis() },
) {
    init {
        require(chainId.trim().isNotEmpty()) { "chainId must not be blank" }
        require(accountId.trim().isNotEmpty()) { "accountId must not be blank" }
    }

    fun listNotes(): List<OfflineNoteV2WalletNote> = store.listNotes()

    fun load(assetDefinitionId: String, amount: String): CompletableFuture<OfflineNoteV2WalletNote> {
        val issuer = issuerClient ?: return failedFuture(
            IllegalStateException("Offline Note V2 issuer client is required for load")
        )
        val assetId = walletAssetId(assetDefinitionId, accountId)
        return issuer.prepareLoad(chainId, accountId, assetDefinition(assetId), amount)
            .thenCompose { context ->
                requireTrustedCertificate(context.keyCertificate, accountId)
                val noteSecret = random32()
                val origin = OfflineNoteV2.CommitmentOriginV2.IssuerLoad(
                    operationId = context.operationId,
                    lineageId = context.lineageId,
                    localRevision = context.localRevision,
                )
                val noteCommitment = deriveNoteCommitment(
                    keyCertificate = context.keyCertificate,
                    assetId = assetId,
                    amount = amount,
                    noteSecret = noteSecret,
                    origin = origin,
                )
                val request = OfflineNoteV2IssueRequest(
                    chainId = chainId,
                    accountId = accountId,
                    assetDefinitionId = assetDefinition(assetId),
                    assetId = assetId,
                    amount = amount,
                    loadContext = context,
                    noteCommitment = noteCommitment,
                )
                issuer.issueNote(request).thenApply { response ->
                    require(response.noteCommitment().contentEquals(noteCommitment)) {
                        "issuer returned a different Offline Note V2 commitment"
                    }
                    val issuedCertificate = response.keyCertificate ?: context.keyCertificate
                    requireTrustedCertificate(issuedCertificate, accountId)
                    val issued = OfflineNoteV2WalletNote(
                        chainId = chainId,
                        accountId = accountId,
                        assetId = assetId,
                        amount = amount,
                        keyCertificate = issuedCertificate,
                        noteCommitment = noteCommitment,
                        noteSecret = noteSecret,
                        origin = origin,
                        state = OfflineNoteV2WalletNoteState.SPENDABLE,
                        createdAtMs = clock.getAsLong(),
                        updatedAtMs = clock.getAsLong(),
                    )
                    store.upsert(issued)
                    issued
                }
            }
    }

    fun prepareReceive(assetDefinitionId: String, amount: String): OfflineNoteV2ReceiveRequest {
        val paymentRequestId = idGenerator.nextId("payment-request")
        val keyCertificate = attestationProvider.currentKeyCertificate()
        requireTrustedCertificate(keyCertificate, accountId)
        val assetId = walletAssetId(assetDefinitionId, accountId)
        val noteSecret = random32()
        val origin = OfflineNoteV2.CommitmentOriginV2.P2pOutput(
            paymentRequestId = paymentRequestId,
            outputIndex = 0,
        )
        val outputCommitment = deriveNoteCommitment(
            keyCertificate = keyCertificate,
            assetId = assetId,
            amount = amount,
            noteSecret = noteSecret,
            origin = origin,
        )
        val pending = OfflineNoteV2WalletNote(
            chainId = chainId,
            accountId = accountId,
            assetId = assetId,
            amount = amount,
            keyCertificate = keyCertificate,
            noteCommitment = outputCommitment,
            noteSecret = noteSecret,
            origin = origin,
            state = OfflineNoteV2WalletNoteState.RECEIVE_PENDING,
            createdAtMs = clock.getAsLong(),
            updatedAtMs = clock.getAsLong(),
        )
        store.upsert(pending)
        return OfflineNoteV2ReceiveRequest(
            chainId = chainId,
            paymentRequestId = paymentRequestId,
            accountId = accountId,
            assetDefinitionId = assetDefinition(assetId),
            assetId = assetId,
            amount = pending.canonicalAmount,
            keyCertificate = keyCertificate,
            outputCommitment = outputCommitment,
        )
    }

    fun pay(receiveRequest: OfflineNoteV2ReceiveRequest): OfflineNoteV2PaymentToken {
        require(receiveRequest.chainId == chainId) { "receive request chainId does not match wallet chainId" }
        requireTrustedCertificate(receiveRequest.keyCertificate, receiveRequest.accountId)
        rejectReusedReceiveRequest(receiveRequest.paymentRequestId)
        val createdAtMs = clock.getAsLong()
        val requestedAmount = decimal(receiveRequest.canonicalAmount)
        val selected = selectSpendableNotes(receiveRequest.assetDefinitionId, requestedAmount)
        val inputAmount = selected.fold(BigDecimal.ZERO) { acc, note -> acc.add(decimal(note.canonicalAmount)) }
        val changeAmount = inputAmount.subtract(requestedAmount)
        require(changeAmount.signum() >= 0) { "selected input amount is below requested amount" }

        val senderCertificate = selected.first().keyCertificate
        requireTrustedCertificate(senderCertificate, accountId)
        val senderCertificateHash = senderCertificate.payloadHash()
        selected.forEach {
            requireTrustedCertificate(it.keyCertificate, accountId)
            require(it.keyCertificate.payloadHash().contentEquals(senderCertificateHash)) {
                "selected input notes must use the same key certificate"
            }
        }
        val inputNullifiers = selected.map { note -> deriveInputNullifier(note) }
        val outputClaims = ArrayList<OfflineNoteV2.AuditOutputClaimV2>()
        outputClaims.add(
            OfflineNoteV2.AuditOutputClaimV2(
                noteCommitment = receiveRequest.outputCommitment(),
                keyCertificate = receiveRequest.keyCertificate,
                assetId = receiveRequest.assetId,
                amount = receiveRequest.canonicalAmount,
            )
        )
        val tokenNonce = random32()
        var changeNote: OfflineNoteV2WalletNote? = null
        if (changeAmount.signum() > 0) {
            val changeSecret = random32()
            val changeAssetId = walletAssetId(receiveRequest.assetDefinitionId, accountId)
            val changeOrigin = OfflineNoteV2.CommitmentOriginV2.P2pOutput(
                paymentRequestId = receiveRequest.paymentRequestId,
                outputIndex = 1,
            )
            val changeCommitment = deriveNoteCommitment(
                keyCertificate = senderCertificate,
                assetId = changeAssetId,
                amount = canonicalDecimal(changeAmount),
                noteSecret = changeSecret,
                origin = changeOrigin,
            )
            changeNote = OfflineNoteV2WalletNote(
                chainId = chainId,
                accountId = accountId,
                assetId = changeAssetId,
                amount = canonicalDecimal(changeAmount),
                keyCertificate = senderCertificate,
                noteCommitment = changeCommitment,
                noteSecret = changeSecret,
                origin = changeOrigin,
                state = OfflineNoteV2WalletNoteState.SPENDABLE,
                createdAtMs = createdAtMs,
                updatedAtMs = createdAtMs,
            )
            outputClaims.add(
                OfflineNoteV2.AuditOutputClaimV2(
                    noteCommitment = changeCommitment,
                    keyCertificate = senderCertificate,
                    assetId = changeAssetId,
                    amount = changeNote.canonicalAmount,
                )
            )
        }
        val outputCommitments = outputClaims.map { it.noteCommitment() }
        val tokenId = OfflineNoteV2.derivePaymentTokenId(
            OfflineNoteV2.PaymentTokenIdPreimageV2(
                chainId = chainId,
                paymentRequestId = receiveRequest.paymentRequestId,
                createdAtMs = createdAtMs,
                tokenNonce = tokenNonce,
                senderKeyCertificatePayloadHash = senderCertificateHash,
                inputNullifiers = inputNullifiers,
                outputCommitments = outputCommitments,
            )
        )
        val draft = OfflineNoteV2.AuditBundleV2(
            tokenId = tokenId,
            senderKeyCertificate = senderCertificate,
            inputNullifiers = inputNullifiers,
            inputClaims = selected.map { it.issuedClaim() },
            outputCommitments = outputCommitments,
            outputClaims = outputClaims,
            recursiveProof = placeholderProof(),
        )
        val audit = draft.replacingRecursiveProof(proofProvider.proveAudit(draft))
        audit.validateProofBinding()
        requireTrustedAuditCertificates(audit)
        require(proofVerifier.verifyAudit(audit)) { "Offline Note V2 recursive audit proof verification failed" }
        store.mutateNotes { notes ->
            selected.forEach {
                require(notes[it.noteCommitmentHex()]?.state == OfflineNoteV2WalletNoteState.SPENDABLE) {
                    "selected Offline Note V2 input changed state"
                }
            }
            if (changeNote != null) {
                require(!notes.containsKey(changeNote.noteCommitmentHex())) {
                    "Offline Note V2 change note already exists"
                }
            }
            selected.forEach {
                notes[it.noteCommitmentHex()] = it.withState(OfflineNoteV2WalletNoteState.SPENT, createdAtMs)
            }
            if (changeNote != null) {
                notes[changeNote.noteCommitmentHex()] = changeNote
            }
        }
        return OfflineNoteV2PaymentToken(
            chainId = chainId,
            paymentRequestId = receiveRequest.paymentRequestId,
            tokenNonce = tokenNonce,
            tokenId = tokenId,
            audit = audit,
            createdAtMs = createdAtMs,
        )
    }

    private fun rejectReusedReceiveRequest(paymentRequestId: String) {
        val reused = store.listNotes().any { note ->
            note.state != OfflineNoteV2WalletNoteState.RECEIVE_PENDING &&
                (note.origin as? OfflineNoteV2.CommitmentOriginV2.P2pOutput)?.paymentRequestId == paymentRequestId
        }
        require(!reused) { "Offline Note V2 receive request has already been used locally" }
    }

    fun accept(paymentToken: OfflineNoteV2PaymentToken): OfflineNoteV2WalletNote {
        validatePaymentToken(paymentToken)
        require(proofVerifier.verifyAudit(paymentToken.audit)) {
            "Offline Note V2 recursive audit proof verification failed"
        }
        return store.mutateNotes { notes ->
            paymentToken.audit.outputClaims.forEachIndexed { index, output ->
                val pending = notes[hexLower(output.noteCommitment())]
                if (pending == null || pending.state != OfflineNoteV2WalletNoteState.RECEIVE_PENDING) {
                    return@forEachIndexed
                }
                require(pending.assetId == output.assetId) {
                    "payment token output asset does not match receive request"
                }
                require(pending.canonicalAmount == output.canonicalAmount) {
                    "payment token output amount does not match receive request"
                }
                require(output.keyCertificate.payloadHash().contentEquals(pending.keyCertificate.payloadHash())) {
                    "payment token output key certificate does not match receive request"
                }
                val origin = pending.origin as? OfflineNoteV2.CommitmentOriginV2.P2pOutput
                    ?: throw IllegalArgumentException("payment token output origin must be P2P")
                require(origin.paymentRequestId == paymentToken.paymentRequestId && origin.outputIndex == index) {
                    "payment token output origin does not match receive request"
                }
                val accepted = pending.withState(OfflineNoteV2WalletNoteState.SPENDABLE, clock.getAsLong())
                notes[pending.noteCommitmentHex()] = accepted
                return@mutateNotes accepted
            }
            throw IllegalStateException("payment token has no pending output for this wallet")
        }
    }

    fun publishAudit(paymentToken: OfflineNoteV2PaymentToken): CompletableFuture<ClientResponse> {
        val submitter = transactionSubmitter ?: return failedFuture(
            IllegalStateException("Offline Note V2 transaction submitter is required for audit publication")
        )
        validatePaymentToken(paymentToken)
        require(proofVerifier.verifyAudit(paymentToken.audit)) {
            "Offline Note V2 recursive audit proof verification failed"
        }
        return submitter.submitAudit(paymentToken.audit).thenApply { response ->
            ensureSuccess(response)
            response
        }
    }

    @JvmOverloads
    fun redeem(
        note: OfflineNoteV2WalletNote,
        recipient: String = accountId,
    ): CompletableFuture<OfflineNoteV2WalletNote> {
        val submitter = transactionSubmitter ?: return failedFuture(
            IllegalStateException("Offline Note V2 transaction submitter is required for redeem")
        )
        val current = store.findNote(note.noteCommitment()) ?: note
        require(current.state == OfflineNoteV2WalletNoteState.SPENDABLE) {
            "only spendable Offline Note V2 notes can be redeemed"
        }
        requireTrustedCertificate(current.keyCertificate, current.accountId)
        val inputNullifier = deriveInputNullifier(current)
        val draft = OfflineNoteV2.RedeemV2(
            sourceNoteCommitment = current.noteCommitment(),
            inputNullifiers = listOf(inputNullifier),
            senderKeyCertificate = current.keyCertificate,
            recipient = recipient,
            assetId = current.assetId,
            amount = current.canonicalAmount,
            recursiveProof = placeholderProof(),
        )
        val redemption = draft.replacingRecursiveProof(proofProvider.proveRedeem(draft))
        redemption.validateProofBinding()
        requireTrustedCertificate(redemption.senderKeyCertificate, current.accountId)
        require(proofVerifier.verifyRedeem(redemption)) {
            "Offline Note V2 recursive redeem proof verification failed"
        }
        val pending = store.mutateNotes { notes ->
            val latest = notes[current.noteCommitmentHex()] ?: current
            require(latest.state == OfflineNoteV2WalletNoteState.SPENDABLE) {
                "only spendable Offline Note V2 notes can be redeemed"
            }
            val updated = latest.withState(OfflineNoteV2WalletNoteState.REDEEM_PENDING, clock.getAsLong())
            notes[latest.noteCommitmentHex()] = updated
            updated
        }
        return submitter.submitRedeem(redemption).thenApply { response ->
            ensureSuccess(response)
            pending
        }
    }

    fun sync(): CompletableFuture<List<OfflineNoteV2WalletNote>> {
        val resolver = syncResolver ?: return CompletableFuture.completedFuture(store.listNotes())
        var chain = CompletableFuture.completedFuture(Unit)
        for (snapshot in store.listNotes()) {
            if (!isPendingState(snapshot.state)) continue
            chain = chain.thenCompose {
                val current = store.findNote(snapshot.noteCommitment())
                    ?: return@thenCompose CompletableFuture.completedFuture(Unit)
                if (!isPendingState(current.state)) {
                    return@thenCompose CompletableFuture.completedFuture(Unit)
                }
                resolver.resolvePendingNote(current).thenApply { resolution ->
                    if (resolution != null && resolution.state != current.state) {
                        store.upsert(current.withState(resolution.state, clock.getAsLong()))
                    }
                    Unit
                }
            }
        }
        return chain.thenApply { store.listNotes() }
    }

    private fun selectSpendableNotes(
        assetDefinitionId: String,
        requestedAmount: BigDecimal,
    ): List<OfflineNoteV2WalletNote> {
        val selected = ArrayList<OfflineNoteV2WalletNote>()
        var total = BigDecimal.ZERO
        for (note in store.listNotes()) {
            if (note.state != OfflineNoteV2WalletNoteState.SPENDABLE) continue
            if (assetDefinition(note.assetId) != assetDefinition(assetDefinitionId)) continue
            selected.add(note)
            total = total.add(decimal(note.canonicalAmount))
            if (total.compareTo(requestedAmount) >= 0) break
            require(selected.size < 4) { "Offline Note V2 payments support at most 4 input notes" }
        }
        require(selected.isNotEmpty() && total.compareTo(requestedAmount) >= 0) {
            "insufficient spendable Offline Note V2 balance"
        }
        return selected
    }

    private fun deriveNoteCommitment(
        keyCertificate: OfflineNoteV2.KeyCertificateV2,
        assetId: String,
        amount: String,
        noteSecret: ByteArray,
        origin: OfflineNoteV2.CommitmentOriginV2,
    ): ByteArray = OfflineNoteV2.deriveNoteCommitment(
        OfflineNoteV2.NoteCommitmentPreimageV2(
            chainId = chainId,
            ownerKeyCertificatePayloadHash = keyCertificate.payloadHash(),
            assetId = assetId,
            amount = amount,
            noteSecret = noteSecret,
            origin = origin,
        )
    )

    private fun deriveInputNullifier(note: OfflineNoteV2WalletNote): ByteArray =
        OfflineNoteV2.deriveInputNullifier(
            OfflineNoteV2.InputNullifierPreimageV2(
                chainId = chainId,
                sourceNoteCommitment = note.noteCommitment(),
                ownerKeyCertificatePayloadHash = note.keyCertificate.payloadHash(),
                noteSecret = note.noteSecret(),
            )
        )

    private fun validatePaymentToken(paymentToken: OfflineNoteV2PaymentToken) {
        require(paymentToken.chainId == chainId) { "payment token chainId does not match wallet chainId" }
        paymentToken.audit.validateProofBinding()
        val expectedTokenId = OfflineNoteV2.derivePaymentTokenId(
            OfflineNoteV2.PaymentTokenIdPreimageV2(
                chainId = paymentToken.chainId,
                paymentRequestId = paymentToken.paymentRequestId,
                createdAtMs = paymentToken.createdAtMs,
                tokenNonce = paymentToken.tokenNonce(),
                senderKeyCertificatePayloadHash = paymentToken.audit.senderKeyCertificate.payloadHash(),
                inputNullifiers = paymentToken.audit.inputNullifiers(),
                outputCommitments = paymentToken.audit.outputCommitments(),
            )
        )
        require(paymentToken.audit.tokenId().contentEquals(paymentToken.tokenId()) &&
            paymentToken.tokenId().contentEquals(expectedTokenId)) {
            "Offline Note V2 payment token id does not match bound token metadata"
        }
        requireTrustedAuditCertificates(paymentToken.audit)
    }

    private fun requireTrustedAuditCertificates(audit: OfflineNoteV2.AuditBundleV2) {
        requireTrustedCertificate(audit.senderKeyCertificate, null)
        val senderHash = audit.senderKeyCertificate.payloadHash()
        audit.inputClaims.forEach { input ->
            require(input.keyCertificatePayloadHash().contentEquals(senderHash)) {
                "Offline Note V2 input claim certificate does not match sender certificate"
            }
            requireTrustedCertificate(audit.senderKeyCertificate, assetAccount(input.assetId))
        }
        audit.outputClaims.forEach { output ->
            requireTrustedCertificate(output.keyCertificate, assetAccount(output.assetId))
        }
    }

    private fun requireTrustedCertificate(
        certificate: OfflineNoteV2.KeyCertificateV2,
        expectedAccountId: String?,
    ) {
        require(expectedAccountId == null || certificate.accountId == expectedAccountId) {
            "Offline Note V2 key certificate account does not match wallet operation"
        }
        require(certificateVerifier.verifyCertificate(certificate)) {
            "Offline Note V2 key certificate is not trusted for this wallet operation"
        }
    }

    private fun random32(): ByteArray {
        val bytes = randomSource.nextBytes(32)
        require(bytes.size == 32) { "Offline Note V2 random source must return exactly 32 bytes" }
        return bytes
    }
}

private fun placeholderProof(): OfflineNoteV2.RecursiveProofV2 =
    OfflineNoteV2.RecursiveProofV2(
        publicInputsHash = OfflineNoteV2.hash("offline-note-v2-draft-proof".toByteArray(Charsets.UTF_8)),
        proof = OfflineNoteV2.ProofBox(OfflineNoteV2.RECURSIVE_BACKEND, byteArrayOf(1)),
    )

private fun ensureSuccess(response: ClientResponse) {
    require(response.statusCode in 200..299) {
        "Offline Note V2 transaction rejected with HTTP ${response.statusCode}: ${response.message}"
    }
}

private fun isPendingState(state: OfflineNoteV2WalletNoteState): Boolean = when (state) {
    OfflineNoteV2WalletNoteState.REDEEM_PENDING -> true
    OfflineNoteV2WalletNoteState.RECEIVE_PENDING,
    OfflineNoteV2WalletNoteState.SPENDABLE,
    OfflineNoteV2WalletNoteState.SPENT,
    OfflineNoteV2WalletNoteState.REDEEMED,
    OfflineNoteV2WalletNoteState.CANCELLED,
    -> false
}

private fun walletAssetId(assetDefinitionId: String, accountId: String): String =
    "${assetDefinition(assetDefinitionId)}#$accountId"

private fun assetDefinition(assetIdOrDefinition: String): String {
    val definition = assetIdOrDefinition.substringBefore('#')
    require(definition.trim().isNotEmpty()) { "asset definition id must not be blank" }
    return definition
}

private fun assetAccount(assetId: String): String? {
    val parts = assetId.split("#", limit = 2)
    if (parts.size != 2) return null
    return parts[1].substringBefore("#dataspace:")
}

private fun decimal(value: String): BigDecimal = BigDecimal(value)

private fun canonicalDecimal(value: BigDecimal): String {
    var normalized = value.stripTrailingZeros()
    if (normalized.scale() < 0) {
        normalized = normalized.setScale(0)
    }
    return normalized.toPlainString()
}

private fun <T> failedFuture(error: Throwable): CompletableFuture<T> {
    val future = CompletableFuture<T>()
    future.completeExceptionally(error)
    return future
}

@Suppress("UNCHECKED_CAST")
private fun requireObject(value: Any?, path: String): Map<String, Any?> {
    require(value is Map<*, *>) { "$path must be an object" }
    return value as Map<String, Any?>
}

private fun requiredString(value: Map<String, Any?>, field: String): String {
    val raw = value[field]
    require(raw is String && raw.isNotBlank()) { "$field must be a non-empty string" }
    return raw
}

private fun hexBytes(value: String, field: String): ByteArray {
    val normalized = value.removePrefix("0x").removePrefix("0X").lowercase(Locale.ROOT)
    require(normalized.length % 2 == 0) { "$field must have an even hex length" }
    val out = ByteArray(normalized.length / 2)
    for (index in out.indices) {
        val hi = Character.digit(normalized[index * 2], 16)
        val lo = Character.digit(normalized[index * 2 + 1], 16)
        require(hi >= 0 && lo >= 0) { "$field must be hex" }
        out[index] = ((hi shl 4) or lo).toByte()
    }
    return out
}

private fun hexLower(bytes: ByteArray): String {
    val chars = CharArray(bytes.size * 2)
    val alphabet = "0123456789abcdef"
    for (i in bytes.indices) {
        val value = bytes[i].toInt() and 0xff
        chars[i * 2] = alphabet[value ushr 4]
        chars[i * 2 + 1] = alphabet[value and 0x0f]
    }
    return String(chars)
}
