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
import java.util.concurrent.CompletionException
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicInteger
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

/** State persisted for a wallet-owned Offline Note note. */
enum class OfflineNoteWalletNoteState {
    SPENDABLE,
    RECEIVE_PENDING,
    SPENT,
    REDEEM_PENDING,
    REDEEMED,
    CANCELLED,
}

/** Structured persisted note record; encrypted stores should serialize this shape. */
class OfflineNoteWalletNote @JvmOverloads constructor(
    val chainId: String,
    val accountId: String,
    val assetId: String,
    val amount: String,
    val keyCertificate: OfflineNote.KeyCertificate,
    noteCommitment: ByteArray,
    noteSecret: ByteArray,
    val origin: OfflineNote.CommitmentOrigin,
    bearerAuditTrail: List<OfflineNote.AuditBundle> = emptyList(),
    val state: OfflineNoteWalletNoteState,
    val createdAtMs: Long = 0,
    val updatedAtMs: Long = createdAtMs,
) {
    private val _noteCommitment = noteCommitment.copyOf()
    private val _noteSecret = noteSecret.copyOf()
    private val _bearerAuditTrail = bearerAuditTrail.toList()
    val canonicalAmount: String

    init {
        require(chainId.trim().isNotEmpty()) { "chainId must not be blank" }
        require(accountId.trim().isNotEmpty()) { "accountId must not be blank" }
        require(_noteSecret.size == 32) { "note_secret must be exactly 32 bytes" }
        canonicalAmount = OfflineNote.IssuedClaim(
            noteCommitment = _noteCommitment,
            keyCertificatePayloadHash = keyCertificate.payloadHash(),
            assetId = assetId,
            amount = amount,
        ).canonicalAmount
    }

    constructor(
        chainId: String,
        accountId: String,
        assetId: String,
        amount: String,
        keyCertificate: OfflineNote.KeyCertificate,
        noteCommitment: ByteArray,
        noteSecret: ByteArray,
        origin: OfflineNote.CommitmentOrigin,
        state: OfflineNoteWalletNoteState,
        createdAtMs: Long,
        updatedAtMs: Long,
    ) : this(
        chainId = chainId,
        accountId = accountId,
        assetId = assetId,
        amount = amount,
        keyCertificate = keyCertificate,
        noteCommitment = noteCommitment,
        noteSecret = noteSecret,
        origin = origin,
        bearerAuditTrail = emptyList(),
        state = state,
        createdAtMs = createdAtMs,
        updatedAtMs = updatedAtMs,
    )

    fun noteCommitment(): ByteArray = _noteCommitment.copyOf()
    fun noteSecret(): ByteArray = _noteSecret.copyOf()
    fun noteCommitmentHex(): String = hexLower(_noteCommitment)
    fun bearerAuditTrail(): List<OfflineNote.AuditBundle> = _bearerAuditTrail.toList()

    fun issuedClaim(): OfflineNote.IssuedClaim = OfflineNote.IssuedClaim(
        noteCommitment = noteCommitment(),
        keyCertificatePayloadHash = keyCertificate.payloadHash(),
        assetId = assetId,
        amount = canonicalAmount,
    )

    fun withState(state: OfflineNoteWalletNoteState, updatedAtMs: Long): OfflineNoteWalletNote =
        OfflineNoteWalletNote(
            chainId = chainId,
            accountId = accountId,
            assetId = assetId,
            amount = canonicalAmount,
            keyCertificate = keyCertificate,
            noteCommitment = noteCommitment(),
            noteSecret = noteSecret(),
            origin = origin,
            bearerAuditTrail = bearerAuditTrail(),
            state = state,
            createdAtMs = createdAtMs,
            updatedAtMs = updatedAtMs,
        )

    fun withBearerAuditTrail(
        bearerAuditTrail: List<OfflineNote.AuditBundle>,
        updatedAtMs: Long,
    ): OfflineNoteWalletNote =
        OfflineNoteWalletNote(
            chainId = chainId,
            accountId = accountId,
            assetId = assetId,
            amount = canonicalAmount,
            keyCertificate = keyCertificate,
            noteCommitment = noteCommitment(),
            noteSecret = noteSecret(),
            origin = origin,
            bearerAuditTrail = bearerAuditTrail,
            state = state,
            createdAtMs = createdAtMs,
            updatedAtMs = updatedAtMs,
        )
}

/** Minimal structured store API for Offline Note wallet notes. */
interface OfflineNoteStore {
    fun <T> mutateNotes(mutator: (MutableMap<String, OfflineNoteWalletNote>) -> T): T

    fun listNotes(): List<OfflineNoteWalletNote> = mutateNotes { it.values.toList() }

    fun findNote(noteCommitment: ByteArray): OfflineNoteWalletNote? =
        mutateNotes { it[hexLower(noteCommitment)] }

    fun upsert(note: OfflineNoteWalletNote) {
        mutateNotes { it[note.noteCommitmentHex()] = note }
    }
}

/** In-memory store for JVM tests and non-persistent tooling. */
class InMemoryOfflineNoteStore : OfflineNoteStore {
    private val notes = LinkedHashMap<String, OfflineNoteWalletNote>()

    @Synchronized
    override fun <T> mutateNotes(mutator: (MutableMap<String, OfflineNoteWalletNote>) -> T): T =
        mutator(notes)
}

/** Supplies wallet-bound Offline Note key certificates. */
interface OfflineNoteAttestationProvider {
    fun currentKeyCertificate(): OfflineNote.KeyCertificate
}

/** Supplies deterministic random material in tests and secure random material in production. */
interface OfflineNoteRandomSource {
    fun nextBytes(length: Int): ByteArray
}

/** Secure random source for note secrets and payment token nonces. */
class SecureOfflineNoteRandomSource : OfflineNoteRandomSource {
    private val secureRandom = SecureRandom()

    override fun nextBytes(length: Int): ByteArray {
        require(length > 0) { "random byte length must be positive" }
        val bytes = ByteArray(length)
        secureRandom.nextBytes(bytes)
        return bytes
    }
}

/** Generates wallet-local request and operation identifiers. */
interface OfflineNoteIdGenerator {
    fun nextId(prefix: String): String
}

/** UUID-backed identifier generator. */
class UuidOfflineNoteIdGenerator : OfflineNoteIdGenerator {
    override fun nextId(prefix: String): String = "$prefix-${UUID.randomUUID()}"
}

/** Builds recursive proofs for direct audit and redeem transactions. */
interface OfflineNoteProofProvider {
    fun proveAudit(audit: OfflineNote.AuditBundle): OfflineNote.RecursiveProof
    fun proveRedeem(redemption: OfflineNote.Redeem): OfflineNote.RecursiveProof
}

/** Verifies recursive proofs before accepting locally-final value. */
interface OfflineNoteProofVerifier {
    fun verifyAudit(audit: OfflineNote.AuditBundle): Boolean
    fun verifyRedeem(redemption: OfflineNote.Redeem): Boolean
}

/** Halo2-backed Offline Note proof verifier. */
class Halo2OfflineNoteProofVerifier : OfflineNoteProofVerifier {
    override fun verifyAudit(audit: OfflineNote.AuditBundle): Boolean =
        OfflineNoteHalo2Prover.verifyAudit(audit)

    override fun verifyRedeem(redemption: OfflineNote.Redeem): Boolean =
        OfflineNoteHalo2Prover.verifyRedeem(redemption)
}

/** Verifies issuer trust and attestation shape for Offline Note key certificates. */
interface OfflineNoteCertificateVerifier {
    fun verifyCertificate(certificate: OfflineNote.KeyCertificate): Boolean
}

/** Fails closed until a wallet is configured with trusted issuer roots. */
class RejectingOfflineNoteCertificateVerifier : OfflineNoteCertificateVerifier {
    override fun verifyCertificate(certificate: OfflineNote.KeyCertificate): Boolean = false
}

/** Ed25519 verifier for issuer-signed Offline Note key certificates. */
class Ed25519OfflineNoteCertificateVerifier(
    trustedIssuerPublicKeys: Collection<ByteArray>,
) : OfflineNoteCertificateVerifier {
    private val trustedIssuerPublicKeys = trustedIssuerPublicKeys.map { it.copyOf() }

    override fun verifyCertificate(certificate: OfflineNote.KeyCertificate): Boolean {
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

/** JVM Halo2 proof provider backed by the SDK's native Offline Note prover. */
class NativeOfflineNoteProofProvider : OfflineNoteProofProvider {
    override fun proveAudit(audit: OfflineNote.AuditBundle): OfflineNote.RecursiveProof =
        OfflineNoteHalo2Prover.proveAudit(audit)

    override fun proveRedeem(redemption: OfflineNote.Redeem): OfflineNote.RecursiveProof =
        OfflineNoteHalo2Prover.proveRedeem(redemption)
}

/** Torii issuer load context needed before deriving a wallet-owned issue commitment. */
class OfflineNoteLoadContext(
    val operationId: String,
    val lineageId: String,
    val localRevision: Long,
    val keyCertificate: OfflineNote.KeyCertificate,
)

/** Request sent to an issuer adapter after the wallet derives a note commitment. */
class OfflineNoteIssueRequest(
    val chainId: String,
    val accountId: String,
    val assetDefinitionId: String,
    val assetId: String,
    val amount: String,
    val loadContext: OfflineNoteLoadContext,
    noteCommitment: ByteArray,
) {
    private val _noteCommitment = noteCommitment.copyOf()

    fun noteCommitment(): ByteArray = _noteCommitment.copyOf()
    fun noteCommitmentHex(): String = hexLower(_noteCommitment)
}

/** Issuer response after Torii accepts the supplied note commitment. */
class OfflineNoteIssueResponse @JvmOverloads constructor(
    noteCommitment: ByteArray,
    val operationId: String,
    val lineageId: String,
    val localRevision: Long,
    val keyCertificate: OfflineNote.KeyCertificate? = null,
    val settlementEntryHashHex: String? = null,
) {
    private val _noteCommitment = noteCommitment.copyOf()

    fun noteCommitment(): ByteArray = _noteCommitment.copyOf()
}

/** Adapter boundary for Torii issuer key refill and note issue calls. */
interface OfflineNoteIssuerClient {
    fun prepareLoad(
        chainId: String,
        accountId: String,
        assetDefinitionId: String,
        amount: String,
    ): CompletableFuture<OfflineNoteLoadContext>

    fun issueNote(request: OfflineNoteIssueRequest): CompletableFuture<OfflineNoteIssueResponse>
}

/** Receiver request handed to a payer; it contains no note secret. */
class OfflineNoteReceiveRequest(
    val chainId: String,
    val paymentRequestId: String,
    val accountId: String,
    val assetDefinitionId: String,
    val assetId: String,
    val amount: String,
    val keyCertificate: OfflineNote.KeyCertificate,
    outputCommitment: ByteArray,
) {
    private val _outputCommitment = outputCommitment.copyOf()
    val canonicalAmount: String = OfflineNote.AuditOutputClaim(
        noteCommitment = _outputCommitment,
        keyCertificate = keyCertificate,
        assetId = assetId,
        amount = amount,
    ).canonicalAmount

    fun outputCommitment(): ByteArray = _outputCommitment.copyOf()
    fun outputCommitmentHex(): String = hexLower(_outputCommitment)
}

/** QR/Norito handoff codec for Offline Note receive requests. */
object OfflineNoteReceiveRequestCodec {
    const val TYPE: String = "offline_receive_request"
    const val TEXT_PREFIX: String = "wallet-offline-receive:"
    private const val RECEIVE_REQUEST_ENVELOPE_SCHEMA =
        "iroha_data_model::offline::model::OfflineNoteReceiveRequestEnvelope"

    @JvmStatic
    fun encodeNorito(request: OfflineNoteReceiveRequest): ByteArray =
        NoritoCodec.encode(request, RECEIVE_REQUEST_ENVELOPE_SCHEMA, ReceiveRequestAdapter, NoritoHeader.COMPACT_LEN)

    @JvmStatic
    fun decodeNorito(payload: ByteArray): OfflineNoteReceiveRequest =
        NoritoCodec.decode(payload, ReceiveRequestAdapter, RECEIVE_REQUEST_ENVELOPE_SCHEMA)

    @JvmStatic
    fun encodeText(request: OfflineNoteReceiveRequest): String =
        TEXT_PREFIX + Base64.getUrlEncoder().withoutPadding().encodeToString(encodeNorito(request))

    @JvmStatic
    fun decodeText(text: String): OfflineNoteReceiveRequest {
        val trimmed = text.trim()
        require(trimmed.startsWith(TEXT_PREFIX)) { "Offline Note receive request prefix missing" }
        return decodeNorito(Base64.getUrlDecoder().decode(trimmed.substring(TEXT_PREFIX.length)))
    }

    @JvmStatic
    fun encodeQrFrameBytes(request: OfflineNoteReceiveRequest): List<ByteArray> =
        encodeQrFrameBytes(request, OfflineQrStream.Options())

    @JvmStatic
    fun encodeQrFrameBytes(
        request: OfflineNoteReceiveRequest,
        options: OfflineQrStream.Options,
    ): List<ByteArray> =
        OfflineQrStream.Encoder.encodeFrameBytes(
            encodeNorito(request),
            OfflineQrStream.PayloadKind.OFFLINE_RECEIVE_REQUEST,
            options,
        )

    @JvmStatic
    fun decodeQrPayload(payload: ByteArray): OfflineNoteReceiveRequest = decodeNorito(payload)

    private object ReceiveRequestAdapter : TypeAdapter<OfflineNoteReceiveRequest> {
        override fun encode(encoder: NoritoEncoder, value: OfflineNoteReceiveRequest) {
            writeField(encoder) { writeString(it, value.chainId) }
            writeField(encoder) { writeString(it, value.paymentRequestId) }
            writeField(encoder) { writeString(it, value.accountId) }
            writeField(encoder) { writeString(it, value.assetDefinitionId) }
            writeField(encoder) { writeString(it, value.assetId) }
            writeField(encoder) { writeString(it, value.canonicalAmount) }
            writeField(encoder) { writeBytesVec(it, value.keyCertificate.noritoEncoded()) }
            writeField(encoder) { it.writeBytes(value.outputCommitment()) }
        }

        override fun decode(decoder: NoritoDecoder): OfflineNoteReceiveRequest {
            val chainId = readField(decoder) { readString(it) }
            val paymentRequestId = readField(decoder) { readString(it) }
            val accountId = readField(decoder) { readString(it) }
            val assetDefinitionId = readField(decoder) { readString(it) }
            val assetId = readField(decoder) { readString(it) }
            val amount = readField(decoder) { readString(it) }
            val keyCertificate = OfflineNote.decodeCertificate(readField(decoder) { readBytesVec(it) })
            val outputCommitment = readField(decoder) { it.readBytes(32) }
            return OfflineNoteReceiveRequest(
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
        require(length <= Int.MAX_VALUE) { "Offline Note receive request field length overflow" }
        val child = NoritoDecoder(decoder.readBytes(length.toInt()), decoder.flags, decoder.flagsHint)
        val value = read(child)
        require(child.remaining() == 0) { "Trailing bytes after Offline Note receive request field decode" }
        return value
    }

    private fun readString(decoder: NoritoDecoder): String {
        val length = decoder.readLength(true)
        require(length <= Int.MAX_VALUE) { "Offline Note receive request string length overflow" }
        val value = String(decoder.readBytes(length.toInt()), StandardCharsets.UTF_8)
        require(value.isNotBlank()) { "Offline Note receive request string must not be blank" }
        return value
    }

    private fun readBytesVec(decoder: NoritoDecoder): ByteArray {
        val length = decoder.readUInt(64)
        require(length <= Int.MAX_VALUE) { "Offline Note receive request bytes length overflow" }
        return decoder.readBytes(length.toInt())
    }
}

/** Payment token produced by a payer and accepted by the recipient. */
class OfflineNotePaymentToken(
    val chainId: String,
    val paymentRequestId: String,
    tokenNonce: ByteArray,
    tokenId: ByteArray,
    val audit: OfflineNote.AuditBundle,
    bearerAuditTrail: List<OfflineNote.AuditBundle> = listOf(audit),
    val createdAtMs: Long,
) {
    private val _tokenNonce = tokenNonce.copyOf()
    private val _tokenId = tokenId.copyOf()
    private val _bearerAuditTrail = bearerAuditTrail.toList()

    fun tokenNonce(): ByteArray = _tokenNonce.copyOf()
    fun tokenId(): ByteArray = _tokenId.copyOf()
    fun tokenIdHex(): String = hexLower(_tokenId)
    fun bearerAuditTrail(): List<OfflineNote.AuditBundle> = _bearerAuditTrail.toList()

    fun outputClaimForNoteCommitment(noteCommitment: ByteArray): OfflineNote.AuditOutputClaim? =
        audit.outputClaimForNoteCommitment(noteCommitment)

    fun outputClaimForNoteCommitmentHex(noteCommitmentHex: String): OfflineNote.AuditOutputClaim? =
        outputClaimForNoteCommitment(hexBytes(noteCommitmentHex.trim(), "note_commitment"))

    fun containsOutputNoteCommitment(noteCommitment: ByteArray): Boolean =
        outputClaimForNoteCommitment(noteCommitment) != null

    fun containsOutputNoteCommitmentHex(noteCommitmentHex: String): Boolean =
        try {
            outputClaimForNoteCommitmentHex(noteCommitmentHex) != null
        } catch (_: IllegalArgumentException) {
            false
        }
}

/** QR/Norito handoff codec for Offline Note payment tokens. */
object OfflineNotePaymentTokenCodec {
    const val TYPE: String = "offline_payment_token"
    const val TEXT_PREFIX: String = "wallet-offline-payment:"
    private const val TOKEN_ENVELOPE_SCHEMA =
        "iroha_data_model::offline::model::OfflineNotePaymentTokenEnvelope"

    @JvmStatic
    fun encodeNorito(token: OfflineNotePaymentToken): ByteArray =
        NoritoCodec.encode(token, TOKEN_ENVELOPE_SCHEMA, PaymentTokenAdapter, NoritoHeader.COMPACT_LEN)

    @JvmStatic
    fun decodeNorito(payload: ByteArray): OfflineNotePaymentToken =
        NoritoCodec.decode(payload, PaymentTokenAdapter, TOKEN_ENVELOPE_SCHEMA)

    @JvmStatic
    fun encodeJson(token: OfflineNotePaymentToken): ByteArray = encodeNorito(token)

    @JvmStatic
    fun decodeJson(payload: ByteArray): OfflineNotePaymentToken = decodeNorito(payload)

    @JvmStatic
    fun encodeText(token: OfflineNotePaymentToken): String =
        TEXT_PREFIX + Base64.getUrlEncoder().withoutPadding().encodeToString(encodeNorito(token))

    @JvmStatic
    fun decodeText(text: String): OfflineNotePaymentToken {
        val trimmed = text.trim()
        require(trimmed.startsWith(TEXT_PREFIX)) { "Offline Note payment token prefix missing" }
        return decodeNorito(Base64.getUrlDecoder().decode(trimmed.substring(TEXT_PREFIX.length)))
    }

    @JvmStatic
    fun encodeQrFrameBytes(token: OfflineNotePaymentToken): List<ByteArray> =
        encodeQrFrameBytes(token, OfflineQrStream.Options())

    @JvmStatic
    fun encodeQrFrameBytes(
        token: OfflineNotePaymentToken,
        options: OfflineQrStream.Options,
    ): List<ByteArray> =
        OfflineQrStream.Encoder.encodeFrameBytes(
            encodeNorito(token),
            OfflineQrStream.PayloadKind.OFFLINE_PAYMENT_TOKEN,
            options,
        )

    @JvmStatic
    fun decodeQrPayload(payload: ByteArray): OfflineNotePaymentToken = decodeNorito(payload)

    private object PaymentTokenAdapter : TypeAdapter<OfflineNotePaymentToken> {
        override fun encode(encoder: NoritoEncoder, value: OfflineNotePaymentToken) {
            writeField(encoder) { writeString(it, value.chainId) }
            writeField(encoder) { writeString(it, value.paymentRequestId) }
            writeField(encoder) { it.writeUInt(value.createdAtMs, 64) }
            writeField(encoder) { writeBytesVec(it, value.tokenNonce()) }
            writeField(encoder) { it.writeBytes(value.tokenId()) }
            writeField(encoder) { writeBytesVec(it, value.audit.noritoEncoded()) }
            writeField(encoder) { writeAuditTrail(it, value.bearerAuditTrail()) }
        }

        override fun decode(decoder: NoritoDecoder): OfflineNotePaymentToken {
            val chainId = readField(decoder) { readString(it) }
            val paymentRequestId = readField(decoder) { readString(it) }
            val createdAtMs = readField(decoder) { it.readUInt(64) }
            val tokenNonce = readField(decoder) { readBytesVec(it) }
            val tokenId = readField(decoder) { it.readBytes(32) }
            val audit = OfflineNote.decodeAudit(readField(decoder) { readBytesVec(it) })
            val bearerAuditTrail = readField(decoder) { readAuditTrail(it) }
            require(audit.tokenId().contentEquals(tokenId)) {
                "Offline Note payment token id does not match audit bundle"
            }
            return OfflineNotePaymentToken(
                chainId = chainId,
                paymentRequestId = paymentRequestId,
                tokenNonce = tokenNonce,
                tokenId = tokenId,
                audit = audit,
                bearerAuditTrail = bearerAuditTrail,
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

    private fun writeAuditTrail(encoder: NoritoEncoder, audits: List<OfflineNote.AuditBundle>) {
        encoder.writeUInt(audits.size.toLong(), 64)
        audits.forEach { audit ->
            writeField(encoder) { writeBytesVec(it, audit.noritoEncoded()) }
        }
    }

    private fun readAuditTrail(decoder: NoritoDecoder): List<OfflineNote.AuditBundle> {
        val count = decoder.readUInt(64)
        require(count <= Int.MAX_VALUE) { "Offline Note bearer audit trail length overflow" }
        return List(count.toInt()) {
            OfflineNote.decodeAudit(readField(decoder) { readBytesVec(it) })
        }
    }

    private fun <T> readField(decoder: NoritoDecoder, read: (NoritoDecoder) -> T): T {
        val length = decoder.readLength(true)
        require(length <= Int.MAX_VALUE) { "Offline Note payment token field length overflow" }
        val child = NoritoDecoder(decoder.readBytes(length.toInt()), decoder.flags, decoder.flagsHint)
        val value = read(child)
        require(child.remaining() == 0) { "Trailing bytes after Offline Note payment token field decode" }
        return value
    }

    private fun readString(decoder: NoritoDecoder): String {
        val length = decoder.readLength(true)
        require(length <= Int.MAX_VALUE) { "Offline Note payment token string length overflow" }
        val value = String(decoder.readBytes(length.toInt()), StandardCharsets.UTF_8)
        require(value.isNotBlank()) { "Offline Note payment token string must not be blank" }
        return value
    }

    private fun readBytesVec(decoder: NoritoDecoder): ByteArray {
        val length = decoder.readUInt(64)
        require(length <= Int.MAX_VALUE) { "Offline Note payment token bytes length overflow" }
        return decoder.readBytes(length.toInt())
    }
}

/** Receipt ACK returned by a recipient after accepting an Offline Note payment token. */
class OfflineNoteReceiptAck(
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
        require(acceptedAtMs > 0L) { "acceptedAtMs must be positive" }
    }

    fun tokenId(): ByteArray = _tokenId.copyOf()
    fun tokenIdHex(): String = hexLower(_tokenId)

    fun matchesPaymentToken(token: OfflineNotePaymentToken): Boolean =
        chainId == token.chainId &&
            paymentRequestId == token.paymentRequestId &&
            _tokenId.contentEquals(token.tokenId()) &&
            receiptAckTokenHasRecipientOutput(token, recipientAccountId)

    fun requireMatchesPaymentToken(token: OfflineNotePaymentToken) {
        require(matchesPaymentToken(token)) { "receipt ACK does not match payment token" }
    }

    companion object {
        @JvmStatic
        fun fromPaymentToken(
            token: OfflineNotePaymentToken,
            recipientAccountId: String,
            acceptedAtMs: Long,
        ): OfflineNoteReceiptAck {
            val checkedRecipient = recipientAccountId.trim()
            require(checkedRecipient.isNotEmpty()) { "recipientAccountId must not be blank" }
            require(receiptAckTokenHasRecipientOutput(token, checkedRecipient)) {
                "payment token does not contain recipient output"
            }
            return OfflineNoteReceiptAck(
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
    token: OfflineNotePaymentToken,
    recipientAccountId: String,
): Boolean =
    token.audit.outputClaims.any { it.keyCertificate.accountId == recipientAccountId }

/** QR/Norito handoff codec for Offline Note receipt ACKs. */
object OfflineNoteReceiptAckCodec {
    const val TYPE: String = "offline_receipt_ack"
    const val TEXT_PREFIX: String = "wallet-offline-ack:"
    private const val RECEIPT_ACK_ENVELOPE_SCHEMA =
        "iroha_data_model::offline::model::OfflineNoteReceiptAckEnvelope"

    @JvmStatic
    fun encodeNorito(ack: OfflineNoteReceiptAck): ByteArray =
        NoritoCodec.encode(ack, RECEIPT_ACK_ENVELOPE_SCHEMA, ReceiptAckAdapter, NoritoHeader.COMPACT_LEN)

    @JvmStatic
    fun decodeNorito(payload: ByteArray): OfflineNoteReceiptAck =
        NoritoCodec.decode(payload, ReceiptAckAdapter, RECEIPT_ACK_ENVELOPE_SCHEMA)

    @JvmStatic
    fun encodeText(ack: OfflineNoteReceiptAck): String =
        TEXT_PREFIX + Base64.getUrlEncoder().withoutPadding().encodeToString(encodeNorito(ack))

    @JvmStatic
    fun decodeText(text: String): OfflineNoteReceiptAck {
        val trimmed = text.trim()
        require(trimmed.startsWith(TEXT_PREFIX)) { "Offline Note receipt ACK prefix missing" }
        return decodeNorito(Base64.getUrlDecoder().decode(trimmed.substring(TEXT_PREFIX.length)))
    }

    @JvmStatic
    fun encodeQrFrameBytes(ack: OfflineNoteReceiptAck): List<ByteArray> =
        encodeQrFrameBytes(ack, OfflineQrStream.Options())

    @JvmStatic
    fun encodeQrFrameBytes(
        ack: OfflineNoteReceiptAck,
        options: OfflineQrStream.Options,
    ): List<ByteArray> =
        OfflineQrStream.Encoder.encodeFrameBytes(
            encodeNorito(ack),
            OfflineQrStream.PayloadKind.OFFLINE_RECEIPT_ACK,
            options,
        )

    @JvmStatic
    fun decodeQrPayload(payload: ByteArray): OfflineNoteReceiptAck = decodeNorito(payload)

    private object ReceiptAckAdapter : TypeAdapter<OfflineNoteReceiptAck> {
        override fun encode(encoder: NoritoEncoder, value: OfflineNoteReceiptAck) {
            writeField(encoder) { writeString(it, value.chainId) }
            writeField(encoder) { writeString(it, value.paymentRequestId) }
            writeField(encoder) { it.writeBytes(value.tokenId()) }
            writeField(encoder) { writeString(it, value.recipientAccountId) }
            writeField(encoder) { it.writeUInt(value.acceptedAtMs, 64) }
        }

        override fun decode(decoder: NoritoDecoder): OfflineNoteReceiptAck {
            val chainId = readField(decoder) { readString(it) }
            val paymentRequestId = readField(decoder) { readString(it) }
            val tokenId = readField(decoder) { it.readBytes(32) }
            val recipientAccountId = readField(decoder) { readString(it) }
            val acceptedAtMs = readField(decoder) { it.readUInt(64) }
            return OfflineNoteReceiptAck(
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
        require(length <= Int.MAX_VALUE) { "Offline Note receipt ACK field length overflow" }
        val child = NoritoDecoder(decoder.readBytes(length.toInt()), decoder.flags, decoder.flagsHint)
        val value = read(child)
        require(child.remaining() == 0) { "Trailing bytes after Offline Note receipt ACK field decode" }
        return value
    }

    private fun readString(decoder: NoritoDecoder): String {
        val length = decoder.readLength(true)
        require(length <= Int.MAX_VALUE) { "Offline Note receipt ACK string length overflow" }
        val value = String(decoder.readBytes(length.toInt()), StandardCharsets.UTF_8)
        require(value.isNotBlank()) { "Offline Note receipt ACK string must not be blank" }
        return value
    }
}

/** Submits direct Offline Note audit/redeem transactions. */
interface OfflineNoteTransactionSubmitter {
    fun submitAudit(audit: OfflineNote.AuditBundle): CompletableFuture<ClientResponse>
    fun submitRedeem(redemption: OfflineNote.Redeem): CompletableFuture<ClientResponse>
    fun submitDefund(
        redemption: OfflineNote.Redeem,
        bearerAuditTrail: List<OfflineNote.AuditBundle>,
    ): CompletableFuture<ClientResponse>
}

/** Resolution returned by a wallet sync resolver for one pending Offline Note note. */
class OfflineNoteSyncResolution @JvmOverloads constructor(
    val state: OfflineNoteWalletNoteState,
    val transactionHashHex: String? = null,
)

/** Looks up transaction-outcome state for pending wallet notes. */
interface OfflineNoteSyncResolver {
    fun resolvePendingNote(note: OfflineNoteWalletNote): CompletableFuture<OfflineNoteSyncResolution?>
}

/** One explorer instruction outcome used by Offline Note wallet reconciliation. */
class OfflineNoteExplorerInstructionOutcome @JvmOverloads constructor(
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

/** Supplies recent Offline Note explorer outcomes for resolver-backed wallet sync. */
interface OfflineNoteOutcomeProvider {
    fun listOutcomes(): CompletableFuture<List<OfflineNoteExplorerInstructionOutcome>>
}

/** Outcome index that maps committed/rejected Offline Note instructions to note states. */
class OfflineNoteOutcomeIndex {
    private val committedRedeems = LinkedHashMap<String, String?>()
    private val rejectedRedeems = LinkedHashMap<String, String?>()

    fun recordCommittedAudit(audit: OfflineNote.AuditBundle, transactionHashHex: String?): OfflineNoteOutcomeIndex {
        return this
    }

    fun recordRejectedAudit(audit: OfflineNote.AuditBundle, transactionHashHex: String?): OfflineNoteOutcomeIndex {
        return this
    }

    fun recordCommittedRedeem(redeem: OfflineNote.Redeem, transactionHashHex: String?): OfflineNoteOutcomeIndex {
        putFirst(committedRedeems, redeem.sourceNoteCommitment(), transactionHashHex)
        return this
    }

    fun recordRejectedRedeem(redeem: OfflineNote.Redeem, transactionHashHex: String?): OfflineNoteOutcomeIndex {
        putFirst(rejectedRedeems, redeem.sourceNoteCommitment(), transactionHashHex)
        return this
    }

    fun resolve(note: OfflineNoteWalletNote): OfflineNoteSyncResolution? =
        when (note.state) {
            OfflineNoteWalletNoteState.REDEEM_PENDING -> resolveRedeemPending(note)
            else -> null
        }

    private fun resolveRedeemPending(note: OfflineNoteWalletNote): OfflineNoteSyncResolution? {
        val commitmentKey = note.noteCommitmentHex()
        if (committedRedeems.containsKey(commitmentKey)) {
            return OfflineNoteSyncResolution(
                OfflineNoteWalletNoteState.REDEEMED,
                committedRedeems[commitmentKey],
            )
        }
        if (rejectedRedeems.containsKey(commitmentKey)) {
            return OfflineNoteSyncResolution(
                OfflineNoteWalletNoteState.SPENDABLE,
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
        const val KIND_ISSUE: String = "IssueOfflineNote"
        const val KIND_REDEEM: String = "RedeemOfflineNote"
        const val KIND_AUDIT: String = "AuditOfflineNote"

        @JvmStatic
        fun fromExplorerOutcomes(outcomes: List<OfflineNoteExplorerInstructionOutcome>): OfflineNoteOutcomeIndex {
            val index = OfflineNoteOutcomeIndex()
            for (outcome in outcomes) {
                val committed = outcome.transactionStatus.equals("committed", ignoreCase = true)
                val rejected = outcome.transactionStatus.equals("rejected", ignoreCase = true)
                if (!committed && !rejected) continue
                when {
                    outcome.kind.equals(KIND_AUDIT, ignoreCase = true) -> {
                        val audit = OfflineNote.decodeAuditInstruction(outcome.encodedInstruction())
                        if (committed) {
                            index.recordCommittedAudit(audit, outcome.transactionHashHex)
                        } else {
                            index.recordRejectedAudit(audit, outcome.transactionHashHex)
                        }
                    }
                    outcome.kind.equals(KIND_REDEEM, ignoreCase = true) -> {
                        val redeem = OfflineNote.decodeRedeemInstruction(outcome.encodedInstruction())
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
class OfflineNoteOutcomeIndexSyncResolver(
    private val provider: OfflineNoteOutcomeProvider,
) : OfflineNoteSyncResolver {
    override fun resolvePendingNote(
        note: OfflineNoteWalletNote,
    ): CompletableFuture<OfflineNoteSyncResolution?> =
        provider.listOutcomes().thenApply { OfflineNoteOutcomeIndex.fromExplorerOutcomes(it).resolve(note) }
}

/** Torii explorer-backed provider for Offline Note wallet reconciliation outcomes. */
class ToriiOfflineNoteOutcomeProvider @JvmOverloads constructor(
    private val executor: HttpTransportExecutor = PlatformHttpTransportExecutor.createDefault(),
    private val baseUri: URI = URI.create("http://localhost:8080"),
    private val timeout: Duration? = Duration.ofSeconds(15),
    defaultHeaders: Map<String, String> = emptyMap(),
    observers: List<ClientObserver> = emptyList(),
    private val perPage: Int = 100,
) : OfflineNoteOutcomeProvider {
    private val defaultHeaders: Map<String, String> = LinkedHashMap(defaultHeaders)
    private val observers: List<ClientObserver> = observers.toList()

    override fun listOutcomes(): CompletableFuture<List<OfflineNoteExplorerInstructionOutcome>> {
        val audit = fetchKind(OfflineNoteOutcomeIndex.KIND_AUDIT)
        val redeem = fetchKind(OfflineNoteOutcomeIndex.KIND_REDEEM)
        return CompletableFuture.allOf(audit, redeem).thenApply {
            audit.join() + redeem.join()
        }
    }

    private fun fetchKind(kind: String): CompletableFuture<List<OfflineNoteExplorerInstructionOutcome>> {
        val request = buildGetRequest(
            "/v1/explorer/instructions",
            linkedMapOf("kind" to kind, "per_page" to perPage.toString()),
        )
        notifyRequest(request)
        val future = CompletableFuture<List<OfflineNoteExplorerInstructionOutcome>>()
        executor.execute(request).whenComplete { response, throwable ->
            if (throwable != null) {
                val error = OfflineToriiException(
                    "Offline Note outcome lookup failed: ${throwable.message ?: throwable.javaClass.simpleName}",
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
                    "Offline Note outcome lookup failed with HTTP ${response.statusCode}",
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
                    "Failed to parse Offline Note explorer outcomes",
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
            "ToriiOfflineNoteOutcomeProvider",
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

    private fun parseExplorerOutcomes(payload: ByteArray): List<OfflineNoteExplorerInstructionOutcome> {
        val parsed = JsonParser.parse(String(payload, StandardCharsets.UTF_8))
        val root = requireObject(parsed, "explorer response")
        val items = root["items"] as? List<*> ?: throw IllegalArgumentException("items must be an array")
        return items.map { item ->
            val obj = requireObject(item, "instruction item")
            val box = requireObject(obj["r#box"] ?: obj["box"], "instruction box")
            val encoded = box["encoded"] as? String
                ?: requireNestedEncoded(box)
            OfflineNoteExplorerInstructionOutcome(
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

/** Transaction submitter that wraps Offline instructions in signed Iroha transactions. */
class IrohaOfflineNoteTransactionSubmitter @JvmOverloads constructor(
    private val client: IrohaClient,
    private val signer: Signer,
    private val chainId: String,
    private val authority: String,
    private val codecAdapter: NoritoCodecAdapter = NoritoJavaCodecAdapter(),
    private val clock: LongSupplier = LongSupplier { System.currentTimeMillis() },
) : OfflineNoteTransactionSubmitter {
    private val transactionBuilder = TransactionBuilder(codecAdapter)

    override fun submitAudit(audit: OfflineNote.AuditBundle): CompletableFuture<ClientResponse> =
        submit(OfflineNote.auditInstruction(audit))

    override fun submitRedeem(redemption: OfflineNote.Redeem): CompletableFuture<ClientResponse> =
        submit(OfflineNote.redeemInstruction(redemption))

    override fun submitDefund(
        redemption: OfflineNote.Redeem,
        bearerAuditTrail: List<OfflineNote.AuditBundle>,
    ): CompletableFuture<ClientResponse> =
        submit(bearerAuditTrail.map { OfflineNote.auditInstruction(it) } + OfflineNote.redeemInstruction(redemption))

    private fun submit(instruction: InstructionBox): CompletableFuture<ClientResponse> =
        submit(listOf(instruction))

    private fun submit(instructions: List<InstructionBox>): CompletableFuture<ClientResponse> {
        val payload = TransactionPayload(
            chainId = chainId,
            authority = authority,
            creationTimeMs = clock.getAsLong(),
            executable = Executable.instructions(instructions),
        )
        return client.submitTransaction(transactionBuilder.encodeAndSign(payload, signer))
    }
}

/** One-call Offline Note wallet facade for load, receive, pay, accept, redeem, and sync. */
class OfflineNoteWallet @JvmOverloads constructor(
    private val chainId: String,
    private val accountId: String,
    private val attestationProvider: OfflineNoteAttestationProvider,
    private val store: OfflineNoteStore = InMemoryOfflineNoteStore(),
    private val issuerClient: OfflineNoteIssuerClient? = null,
    private val transactionSubmitter: OfflineNoteTransactionSubmitter? = null,
    private val syncResolver: OfflineNoteSyncResolver? = null,
    private val proofProvider: OfflineNoteProofProvider = NativeOfflineNoteProofProvider(),
    private val proofVerifier: OfflineNoteProofVerifier = Halo2OfflineNoteProofVerifier(),
    private val certificateVerifier: OfflineNoteCertificateVerifier = RejectingOfflineNoteCertificateVerifier(),
    private val randomSource: OfflineNoteRandomSource = SecureOfflineNoteRandomSource(),
    private val idGenerator: OfflineNoteIdGenerator = UuidOfflineNoteIdGenerator(),
    private val clock: LongSupplier = LongSupplier { System.currentTimeMillis() },
) {
    private companion object {
        private val loadThreadIds = AtomicInteger()
        private val loadExecutor = Executors.newCachedThreadPool { task ->
            Thread(task, "iroha-offline-note-wallet-${loadThreadIds.incrementAndGet()}").apply {
                isDaemon = true
            }
        }
    }

    init {
        require(chainId.trim().isNotEmpty()) { "chainId must not be blank" }
        require(accountId.trim().isNotEmpty()) { "accountId must not be blank" }
    }

    fun listNotes(): List<OfflineNoteWalletNote> = store.listNotes()

    fun load(assetDefinitionId: String, amount: String): CompletableFuture<OfflineNoteWalletNote> {
        val issuer = issuerClient ?: return failedFuture(
            IllegalStateException("Offline Note issuer client is required for load")
        )
        val assetId = walletAssetId(assetDefinitionId, accountId)
        val result = CompletableFuture<OfflineNoteWalletNote>()
        issuer.prepareLoad(chainId, accountId, assetDefinition(assetId), amount)
            .whenComplete { context, prepareError ->
                loadExecutor.execute(prepareComplete@{
                    if (prepareError != null) {
                        result.completeExceptionally(unwrapCompletion(prepareError))
                        return@prepareComplete
                    }
                    if (context == null) {
                        result.completeExceptionally(
                            IllegalStateException("Offline Note issuer returned no load context")
                        )
                        return@prepareComplete
                    }
                    val noteSecret: ByteArray
                    val origin: OfflineNote.CommitmentOrigin.IssuerLoad
                    val noteCommitment: ByteArray
                    val request: OfflineNoteIssueRequest
                    try {
                        requireTrustedCertificate(context.keyCertificate, accountId)
                        noteSecret = random32()
                        origin = OfflineNote.CommitmentOrigin.IssuerLoad(
                            operationId = context.operationId,
                            lineageId = context.lineageId,
                            localRevision = context.localRevision,
                        )
                        noteCommitment = deriveNoteCommitment(
                            keyCertificate = context.keyCertificate,
                            assetId = assetId,
                            amount = amount,
                            noteSecret = noteSecret,
                            origin = origin,
                        )
                        request = OfflineNoteIssueRequest(
                            chainId = chainId,
                            accountId = accountId,
                            assetDefinitionId = assetDefinition(assetId),
                            assetId = assetId,
                            amount = amount,
                            loadContext = context,
                            noteCommitment = noteCommitment,
                        )
                    } catch (error: Throwable) {
                        result.completeExceptionally(error)
                        return@prepareComplete
                    }
                    val issueFuture = try {
                        issuer.issueNote(request)
                    } catch (error: Throwable) {
                        result.completeExceptionally(error)
                        return@prepareComplete
                    }
                    issueFuture.whenComplete { response, issueError ->
                        loadExecutor.execute(issueComplete@{
                            if (issueError != null) {
                                result.completeExceptionally(unwrapCompletion(issueError))
                                return@issueComplete
                            }
                            if (response == null) {
                                result.completeExceptionally(
                                    IllegalStateException("Offline Note issuer returned no issue response")
                                )
                                return@issueComplete
                            }
                            try {
                                require(response.noteCommitment().contentEquals(noteCommitment)) {
                                    "issuer returned a different Offline Note commitment"
                                }
                                val issuedCertificate = response.keyCertificate ?: context.keyCertificate
                                requireTrustedCertificate(issuedCertificate, accountId)
                                val now = clock.getAsLong()
                                val issued = OfflineNoteWalletNote(
                                    chainId = chainId,
                                    accountId = accountId,
                                    assetId = assetId,
                                    amount = amount,
                                    keyCertificate = issuedCertificate,
                                    noteCommitment = noteCommitment,
                                    noteSecret = noteSecret,
                                    origin = origin,
                                    state = OfflineNoteWalletNoteState.SPENDABLE,
                                    createdAtMs = now,
                                    updatedAtMs = now,
                                )
                                store.upsert(issued)
                                result.complete(issued)
                            } catch (error: Throwable) {
                                result.completeExceptionally(error)
                            }
                        })
                    }
                })
            }
        return result
    }

    fun prepareReceive(assetDefinitionId: String, amount: String): OfflineNoteReceiveRequest {
        val paymentRequestId = idGenerator.nextId("payment-request")
        val keyCertificate = attestationProvider.currentKeyCertificate()
        requireTrustedCertificate(keyCertificate, accountId)
        val assetId = walletAssetId(assetDefinitionId, accountId)
        val noteSecret = random32()
        val origin = OfflineNote.CommitmentOrigin.P2pOutput(
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
        val pending = OfflineNoteWalletNote(
            chainId = chainId,
            accountId = accountId,
            assetId = assetId,
            amount = amount,
            keyCertificate = keyCertificate,
            noteCommitment = outputCommitment,
            noteSecret = noteSecret,
            origin = origin,
            state = OfflineNoteWalletNoteState.RECEIVE_PENDING,
            createdAtMs = clock.getAsLong(),
            updatedAtMs = clock.getAsLong(),
        )
        store.upsert(pending)
        return OfflineNoteReceiveRequest(
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

    fun pay(receiveRequest: OfflineNoteReceiveRequest): OfflineNotePaymentToken {
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
            bearerAuditTrail(it)
            requireTrustedCertificate(it.keyCertificate, accountId)
            require(it.keyCertificate.payloadHash().contentEquals(senderCertificateHash)) {
                "selected input notes must use the same key certificate"
            }
        }
        val inputNullifiers = selected.map { note -> deriveInputNullifier(note) }
        val outputClaims = ArrayList<OfflineNote.AuditOutputClaim>()
        outputClaims.add(
            OfflineNote.AuditOutputClaim(
                noteCommitment = receiveRequest.outputCommitment(),
                keyCertificate = receiveRequest.keyCertificate,
                assetId = receiveRequest.assetId,
                amount = receiveRequest.canonicalAmount,
            )
        )
        val tokenNonce = random32()
        var changeNote: OfflineNoteWalletNote? = null
        if (changeAmount.signum() > 0) {
            val changeSecret = random32()
            val changeAssetId = walletAssetId(receiveRequest.assetDefinitionId, accountId)
            val changeOrigin = OfflineNote.CommitmentOrigin.P2pOutput(
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
            changeNote = OfflineNoteWalletNote(
                chainId = chainId,
                accountId = accountId,
                assetId = changeAssetId,
                amount = canonicalDecimal(changeAmount),
                keyCertificate = senderCertificate,
                noteCommitment = changeCommitment,
                noteSecret = changeSecret,
                origin = changeOrigin,
                state = OfflineNoteWalletNoteState.SPENDABLE,
                createdAtMs = createdAtMs,
                updatedAtMs = createdAtMs,
            )
            outputClaims.add(
                OfflineNote.AuditOutputClaim(
                    noteCommitment = changeCommitment,
                    keyCertificate = senderCertificate,
                    assetId = changeAssetId,
                    amount = changeNote.canonicalAmount,
                )
            )
        }
        val outputCommitments = outputClaims.map { it.noteCommitment() }
        val tokenId = OfflineNote.derivePaymentTokenId(
            OfflineNote.PaymentTokenIdPreimage(
                chainId = chainId,
                paymentRequestId = receiveRequest.paymentRequestId,
                createdAtMs = createdAtMs,
                tokenNonce = tokenNonce,
                senderKeyCertificatePayloadHash = senderCertificateHash,
                inputNullifiers = inputNullifiers,
                outputCommitments = outputCommitments,
            )
        )
        val draft = OfflineNote.AuditBundle(
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
        require(proofVerifier.verifyAudit(audit)) { "Offline Note recursive audit proof verification failed" }
        val outputBearerAuditTrail = bearerAuditTrail(selected, audit)
        store.mutateNotes { notes ->
            selected.forEach {
                require(notes[it.noteCommitmentHex()]?.state == OfflineNoteWalletNoteState.SPENDABLE) {
                    "selected Offline Note input changed state"
                }
            }
            if (changeNote != null) {
                require(!notes.containsKey(changeNote.noteCommitmentHex())) {
                    "Offline Note change note already exists"
                }
            }
            selected.forEach {
                notes[it.noteCommitmentHex()] = it.withState(OfflineNoteWalletNoteState.SPENT, createdAtMs)
            }
            if (changeNote != null) {
                notes[changeNote.noteCommitmentHex()] = changeNote.withBearerAuditTrail(
                    outputBearerAuditTrail,
                    createdAtMs,
                )
            }
        }
        return OfflineNotePaymentToken(
            chainId = chainId,
            paymentRequestId = receiveRequest.paymentRequestId,
            tokenNonce = tokenNonce,
            tokenId = tokenId,
            audit = audit,
            bearerAuditTrail = outputBearerAuditTrail,
            createdAtMs = createdAtMs,
        )
    }

    private fun rejectReusedReceiveRequest(paymentRequestId: String) {
        val reused = store.listNotes().any { note ->
            note.state != OfflineNoteWalletNoteState.RECEIVE_PENDING &&
                (note.origin as? OfflineNote.CommitmentOrigin.P2pOutput)?.paymentRequestId == paymentRequestId
        }
        require(!reused) { "Offline Note receive request has already been used locally" }
    }

    private fun bearerAuditTrail(note: OfflineNoteWalletNote): List<OfflineNote.AuditBundle> =
        when (note.origin) {
            is OfflineNote.CommitmentOrigin.IssuerLoad -> emptyList()
            is OfflineNote.CommitmentOrigin.P2pOutput -> {
                val trail = note.bearerAuditTrail()
                require(trail.isNotEmpty()) { "Offline Note bearer note is missing the audit trail required for defunding" }
                trail
            }
        }

    private fun bearerAuditTrail(
        inputNotes: List<OfflineNoteWalletNote>,
        audit: OfflineNote.AuditBundle,
    ): List<OfflineNote.AuditBundle> {
        val seen = LinkedHashSet<String>()
        val result = ArrayList<OfflineNote.AuditBundle>()
        inputNotes.forEach { note ->
            bearerAuditTrail(note).forEach { inputAudit ->
                if (seen.add(hexLower(inputAudit.tokenId()))) {
                    result.add(inputAudit)
                }
            }
        }
        if (seen.add(hexLower(audit.tokenId()))) {
            result.add(audit)
        }
        return result
    }

    fun accept(paymentToken: OfflineNotePaymentToken): OfflineNoteWalletNote {
        validatePaymentToken(paymentToken)
        require(proofVerifier.verifyAudit(paymentToken.audit)) {
            "Offline Note recursive audit proof verification failed"
        }
        return store.mutateNotes { notes ->
            paymentToken.audit.outputClaims.forEachIndexed { index, output ->
                val pending = notes[hexLower(output.noteCommitment())]
                if (pending == null || pending.state != OfflineNoteWalletNoteState.RECEIVE_PENDING) {
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
                val origin = pending.origin as? OfflineNote.CommitmentOrigin.P2pOutput
                    ?: throw IllegalArgumentException("payment token output origin must be P2P")
                require(origin.paymentRequestId == paymentToken.paymentRequestId && origin.outputIndex == index) {
                    "payment token output origin does not match receive request"
                }
                val now = clock.getAsLong()
                val accepted = pending
                    .withState(OfflineNoteWalletNoteState.SPENDABLE, now)
                    .withBearerAuditTrail(paymentToken.bearerAuditTrail(), now)
                notes[pending.noteCommitmentHex()] = accepted
                return@mutateNotes accepted
            }
            throw IllegalStateException("payment token has no pending output for this wallet")
        }
    }

    fun publishAudit(paymentToken: OfflineNotePaymentToken): CompletableFuture<ClientResponse> {
        val submitter = transactionSubmitter ?: return failedFuture(
            IllegalStateException("Offline Note transaction submitter is required for audit publication")
        )
        validatePaymentToken(paymentToken)
        require(proofVerifier.verifyAudit(paymentToken.audit)) {
            "Offline Note recursive audit proof verification failed"
        }
        return submitter.submitAudit(paymentToken.audit).thenApply { response ->
            ensureSuccess(response)
            response
        }
    }

    @JvmOverloads
    fun redeem(
        note: OfflineNoteWalletNote,
        recipient: String = accountId,
    ): CompletableFuture<OfflineNoteWalletNote> {
        val submitter = transactionSubmitter ?: return failedFuture(
            IllegalStateException("Offline Note transaction submitter is required for redeem")
        )
        val current = store.findNote(note.noteCommitment()) ?: note
        require(current.state == OfflineNoteWalletNoteState.SPENDABLE) {
            "only spendable Offline Note notes can be redeemed"
        }
        val bearerAuditTrail = bearerAuditTrail(current)
        requireTrustedCertificate(current.keyCertificate, current.accountId)
        val inputNullifier = deriveInputNullifier(current)
        val draft = OfflineNote.Redeem(
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
            "Offline Note recursive redeem proof verification failed"
        }
        val pending = store.mutateNotes { notes ->
            val latest = notes[current.noteCommitmentHex()] ?: current
            require(latest.state == OfflineNoteWalletNoteState.SPENDABLE) {
                "only spendable Offline Note notes can be redeemed"
            }
            val updated = latest.withState(
                OfflineNoteWalletNoteState.REDEEM_PENDING,
                clock.getAsLong(),
            )
            notes[latest.noteCommitmentHex()] = updated
            updated
        }
        val submitted = try {
            submitter.submitDefund(redemption, bearerAuditTrail)
        } catch (error: Throwable) {
            rollbackRedeemReservation(pending)
            return failedFuture(error)
        }
        return submitted.thenApply { response ->
            try {
                ensureSuccess(response)
            } catch (error: RuntimeException) {
                rollbackRedeemReservation(pending)
                throw error
            }
            pending
        }
    }

    private fun rollbackRedeemReservation(reserved: OfflineNoteWalletNote) {
        store.mutateNotes { notes ->
            val latest = notes[reserved.noteCommitmentHex()] ?: return@mutateNotes
            if (
                latest.state == OfflineNoteWalletNoteState.REDEEM_PENDING &&
                latest.updatedAtMs == reserved.updatedAtMs
            ) {
                notes[latest.noteCommitmentHex()] =
                    latest.withState(OfflineNoteWalletNoteState.SPENDABLE, clock.getAsLong())
            }
        }
    }

    fun sync(): CompletableFuture<List<OfflineNoteWalletNote>> {
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
    ): List<OfflineNoteWalletNote> {
        val selected = ArrayList<OfflineNoteWalletNote>()
        var total = BigDecimal.ZERO
        for (note in store.listNotes()) {
            if (note.state != OfflineNoteWalletNoteState.SPENDABLE) continue
            if (assetDefinition(note.assetId) != assetDefinition(assetDefinitionId)) continue
            selected.add(note)
            total = total.add(decimal(note.canonicalAmount))
            if (total.compareTo(requestedAmount) >= 0) break
            require(selected.size < 4) { "Offline Note payments support at most 4 input notes" }
        }
        require(selected.isNotEmpty() && total.compareTo(requestedAmount) >= 0) {
            "insufficient spendable Offline Note balance"
        }
        return selected
    }

    private fun deriveNoteCommitment(
        keyCertificate: OfflineNote.KeyCertificate,
        assetId: String,
        amount: String,
        noteSecret: ByteArray,
        origin: OfflineNote.CommitmentOrigin,
    ): ByteArray = OfflineNote.deriveNoteCommitment(
        OfflineNote.NoteCommitmentPreimage(
            chainId = chainId,
            ownerKeyCertificatePayloadHash = keyCertificate.payloadHash(),
            assetId = assetId,
            amount = amount,
            noteSecret = noteSecret,
            origin = origin,
        )
    )

    private fun deriveInputNullifier(note: OfflineNoteWalletNote): ByteArray =
        OfflineNote.deriveInputNullifier(
            OfflineNote.InputNullifierPreimage(
                chainId = chainId,
                sourceNoteCommitment = note.noteCommitment(),
                ownerKeyCertificatePayloadHash = note.keyCertificate.payloadHash(),
                noteSecret = note.noteSecret(),
            )
        )

    private fun validatePaymentToken(paymentToken: OfflineNotePaymentToken) {
        require(paymentToken.chainId == chainId) { "payment token chainId does not match wallet chainId" }
        paymentToken.audit.validateProofBinding()
        val expectedTokenId = OfflineNote.derivePaymentTokenId(
            OfflineNote.PaymentTokenIdPreimage(
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
            "Offline Note payment token id does not match bound token metadata"
        }
        requireTrustedAuditCertificates(paymentToken.audit)
        validateBearerAuditTrail(paymentToken.bearerAuditTrail(), paymentToken.audit)
    }

    private fun validateBearerAuditTrail(
        audits: List<OfflineNote.AuditBundle>,
        terminalAudit: OfflineNote.AuditBundle,
    ) {
        require(audits.isNotEmpty() && audits.last().noritoEncoded().contentEquals(terminalAudit.noritoEncoded())) {
            "Offline Note bearer audit trail must end with the payment token audit"
        }
        val tokenIds = LinkedHashSet<String>()
        val nullifiers = LinkedHashSet<String>()
        val outputs = LinkedHashSet<String>()
        val outputProducerIndex = LinkedHashMap<String, Int>()
        audits.forEachIndexed { index, audit ->
            audit.outputCommitments().forEach { output ->
                val key = hexLower(output)
                require(!outputProducerIndex.containsKey(key)) {
                    "Offline Note bearer audit trail has duplicate output commitment"
                }
                outputProducerIndex[key] = index
            }
        }
        audits.forEachIndexed { index, audit ->
            audit.validateProofBinding()
            require(tokenIds.add(hexLower(audit.tokenId()))) {
                "Offline Note bearer audit trail has duplicate token id"
            }
            audit.inputNullifiers().forEach { nullifier ->
                require(nullifiers.add(hexLower(nullifier))) {
                    "Offline Note bearer audit trail has duplicate input nullifier"
                }
            }
            audit.outputCommitments().forEach { output ->
                require(outputs.add(hexLower(output))) {
                    "Offline Note bearer audit trail has duplicate output commitment"
                }
            }
            audit.inputClaims.forEach { claim ->
                val producerIndex = outputProducerIndex[hexLower(claim.noteCommitment())]
                require(producerIndex == null || producerIndex < index) {
                    "Offline Note bearer audit trail input claims are out of order"
                }
            }
            requireTrustedAuditCertificates(audit)
            require(proofVerifier.verifyAudit(audit)) {
                "Offline Note recursive audit proof verification failed"
            }
        }
    }

    private fun requireTrustedAuditCertificates(audit: OfflineNote.AuditBundle) {
        requireTrustedCertificate(audit.senderKeyCertificate, null)
        val senderHash = audit.senderKeyCertificate.payloadHash()
        audit.inputClaims.forEach { input ->
            require(input.keyCertificatePayloadHash().contentEquals(senderHash)) {
                "Offline Note input claim certificate does not match sender certificate"
            }
            requireTrustedCertificate(audit.senderKeyCertificate, assetAccount(input.assetId))
        }
        audit.outputClaims.forEach { output ->
            requireTrustedCertificate(output.keyCertificate, assetAccount(output.assetId))
        }
    }

    private fun requireTrustedCertificate(
        certificate: OfflineNote.KeyCertificate,
        expectedAccountId: String?,
    ) {
        require(expectedAccountId == null || certificate.accountId == expectedAccountId) {
            "Offline Note key certificate account does not match wallet operation"
        }
        require(certificateVerifier.verifyCertificate(certificate)) {
            "Offline Note key certificate is not trusted for this wallet operation"
        }
    }

    private fun random32(): ByteArray {
        val bytes = randomSource.nextBytes(32)
        require(bytes.size == 32) { "Offline Note random source must return exactly 32 bytes" }
        return bytes
    }
}

private fun placeholderProof(): OfflineNote.RecursiveProof =
    OfflineNote.RecursiveProof(
        publicInputsHash = OfflineNote.hash("offline-note-draft-proof".toByteArray(Charsets.UTF_8)),
        proof = OfflineNote.ProofBox(OfflineNote.RECURSIVE_BACKEND, byteArrayOf(1)),
    )

private fun ensureSuccess(response: ClientResponse) {
    require(response.statusCode in 200..299) {
        "Offline Note transaction rejected with HTTP ${response.statusCode}: ${response.message}"
    }
}

private fun isPendingState(state: OfflineNoteWalletNoteState): Boolean = when (state) {
    OfflineNoteWalletNoteState.REDEEM_PENDING -> true
    OfflineNoteWalletNoteState.RECEIVE_PENDING,
    OfflineNoteWalletNoteState.SPENDABLE,
    OfflineNoteWalletNoteState.SPENT,
    OfflineNoteWalletNoteState.REDEEMED,
    OfflineNoteWalletNoteState.CANCELLED,
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

private fun unwrapCompletion(error: Throwable): Throwable =
    if (error is CompletionException && error.cause != null) error.cause!! else error

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
