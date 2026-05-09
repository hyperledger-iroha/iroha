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
import org.hyperledger.iroha.sdk.tx.TransactionBuilder
import org.hyperledger.iroha.sdk.tx.norito.NoritoCodecAdapter
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter

/** State persisted for a wallet-owned Offline Note V2 note. */
enum class OfflineNoteV2WalletNoteState {
    SPENDABLE,
    RECEIVE_PENDING,
    CHANGE_PENDING,
    SPEND_PENDING,
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
    fun listNotes(): List<OfflineNoteV2WalletNote>
    fun findNote(noteCommitment: ByteArray): OfflineNoteV2WalletNote?
    fun upsert(note: OfflineNoteV2WalletNote)
}

/** In-memory store for JVM tests and non-persistent tooling. */
class InMemoryOfflineNoteV2Store : OfflineNoteV2Store {
    private val notes = LinkedHashMap<String, OfflineNoteV2WalletNote>()

    @Synchronized
    override fun listNotes(): List<OfflineNoteV2WalletNote> = notes.values.toList()

    @Synchronized
    override fun findNote(noteCommitment: ByteArray): OfflineNoteV2WalletNote? =
        notes[hexLower(noteCommitment)]

    @Synchronized
    override fun upsert(note: OfflineNoteV2WalletNote) {
        notes[note.noteCommitmentHex()] = note
    }
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

/** Payment token produced by a payer and accepted by the recipient. */
class OfflineNoteV2PaymentToken(
    val paymentRequestId: String,
    tokenId: ByteArray,
    val audit: OfflineNoteV2.AuditBundleV2,
    val createdAtMs: Long,
) {
    private val _tokenId = tokenId.copyOf()

    fun tokenId(): ByteArray = _tokenId.copyOf()
    fun tokenIdHex(): String = hexLower(_tokenId)
}

/** QR/JSON handoff codec for Offline Note V2 payment tokens. */
object OfflineNoteV2PaymentTokenCodec {
    const val TYPE: String = "offline_payment_token_v2"
    const val VERSION: Long = 2
    const val TEXT_PREFIX: String = "wallet-offline-payment-v2:"

    @JvmStatic
    fun encodeJson(token: OfflineNoteV2PaymentToken): ByteArray {
        val payload = linkedMapOf<String, Any?>(
            "version" to VERSION,
            "type" to TYPE,
            "invoice_id" to token.paymentRequestId,
            "token_id" to token.tokenIdHex(),
            "audit_norito_base64" to Base64.getEncoder().encodeToString(token.audit.noritoEncoded()),
            "created_at_ms" to token.createdAtMs,
        )
        return JsonEncoder.encode(payload).toByteArray(StandardCharsets.UTF_8)
    }

    @JvmStatic
    fun decodeJson(payload: ByteArray): OfflineNoteV2PaymentToken {
        val obj = parseObject(payload)
        val version = asLong(obj["version"], "version")
        require(version == VERSION) { "Offline Note V2 payment token JSON version must be $VERSION" }
        require(asString(obj["type"], "type") == TYPE) { "Offline Note V2 payment token JSON type mismatch" }
        val paymentRequestId = asOptionalString(obj["invoice_id"])
            ?: asString(obj["payment_request_id"], "payment_request_id")
        val tokenId = hexBytes(asString(obj["token_id"], "token_id"), "token_id")
        val auditBytes = Base64.getDecoder().decode(asString(obj["audit_norito_base64"], "audit_norito_base64"))
        val audit = OfflineNoteV2.decodeAudit(auditBytes)
        require(audit.tokenId().contentEquals(tokenId)) {
            "Offline Note V2 payment token id does not match audit bundle"
        }
        return OfflineNoteV2PaymentToken(
            paymentRequestId = paymentRequestId,
            tokenId = tokenId,
            audit = audit,
            createdAtMs = asLong(obj["created_at_ms"], "created_at_ms"),
        )
    }

    @JvmStatic
    fun encodeText(token: OfflineNoteV2PaymentToken): String =
        TEXT_PREFIX + Base64.getEncoder().encodeToString(encodeJson(token))

    @JvmStatic
    fun decodeText(text: String): OfflineNoteV2PaymentToken {
        val trimmed = text.trim()
        require(trimmed.startsWith(TEXT_PREFIX)) { "Offline Note V2 payment token prefix missing" }
        return decodeJson(Base64.getDecoder().decode(trimmed.substring(TEXT_PREFIX.length)))
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
            encodeJson(token),
            OfflineQrStream.PayloadKind.OFFLINE_PAYMENT_TOKEN_V2,
            options,
        )

    @JvmStatic
    fun decodeQrPayload(payload: ByteArray): OfflineNoteV2PaymentToken = decodeJson(payload)

    @Suppress("UNCHECKED_CAST")
    private fun parseObject(payload: ByteArray): Map<String, Any?> {
        val parsed = JsonParser.parse(String(payload, StandardCharsets.UTF_8))
        require(parsed is Map<*, *>) { "Offline Note V2 payment token JSON root must be an object" }
        return parsed as Map<String, Any?>
    }

    private fun asString(value: Any?, field: String): String {
        require(value is String && value.isNotBlank()) { "$field must be a non-empty string" }
        return value
    }

    private fun asOptionalString(value: Any?): String? {
        if (value == null) return null
        require(value is String && value.isNotBlank()) { "optional string field must be non-empty when present" }
        return value
    }

    private fun asLong(value: Any?, field: String): Long = when (value) {
        is Number -> value.toLong()
        is String -> value.toLong()
        else -> throw IllegalArgumentException("$field must be an integer")
    }

    private fun hexBytes(value: String, field: String): ByteArray {
        val normalized = value.lowercase(Locale.ROOT)
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
    private val spentInputNullifiers = LinkedHashMap<String, String?>()
    private val rejectedAuditInputs = LinkedHashMap<String, String?>()
    private val committedAuditOutputs = LinkedHashMap<String, String?>()
    private val rejectedAuditOutputs = LinkedHashMap<String, String?>()
    private val committedRedeems = LinkedHashMap<String, String?>()
    private val rejectedRedeems = LinkedHashMap<String, String?>()

    fun recordCommittedAudit(audit: OfflineNoteV2.AuditBundleV2, transactionHashHex: String?): OfflineNoteV2OutcomeIndex {
        audit.inputNullifiers().forEach { putFirst(spentInputNullifiers, it, transactionHashHex) }
        audit.outputCommitments().forEach { putFirst(committedAuditOutputs, it, transactionHashHex) }
        return this
    }

    fun recordRejectedAudit(audit: OfflineNoteV2.AuditBundleV2, transactionHashHex: String?): OfflineNoteV2OutcomeIndex {
        audit.inputClaims.forEach { putFirst(rejectedAuditInputs, it.noteCommitment(), transactionHashHex) }
        audit.outputCommitments().forEach { putFirst(rejectedAuditOutputs, it, transactionHashHex) }
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
            OfflineNoteV2WalletNoteState.SPEND_PENDING -> resolveSpendPending(note)
            OfflineNoteV2WalletNoteState.CHANGE_PENDING,
            OfflineNoteV2WalletNoteState.RECEIVE_PENDING -> resolveOutputPending(note)
            OfflineNoteV2WalletNoteState.REDEEM_PENDING -> resolveRedeemPending(note)
            else -> null
        }

    private fun resolveSpendPending(note: OfflineNoteV2WalletNote): OfflineNoteV2SyncResolution? {
        val inputNullifier = OfflineNoteV2.deriveInputNullifier(
            OfflineNoteV2.InputNullifierPreimageV2(
                chainId = note.chainId,
                sourceNoteCommitment = note.noteCommitment(),
                ownerKeyCertificatePayloadHash = note.keyCertificate.payloadHash(),
                noteSecret = note.noteSecret(),
            )
        )
        val nullifierKey = hexLower(inputNullifier)
        if (spentInputNullifiers.containsKey(nullifierKey)) {
            return OfflineNoteV2SyncResolution(
                OfflineNoteV2WalletNoteState.SPENT,
                spentInputNullifiers[nullifierKey],
            )
        }
        val commitmentKey = note.noteCommitmentHex()
        if (rejectedAuditInputs.containsKey(commitmentKey)) {
            return OfflineNoteV2SyncResolution(
                OfflineNoteV2WalletNoteState.SPENDABLE,
                rejectedAuditInputs[commitmentKey],
            )
        }
        return null
    }

    private fun resolveOutputPending(note: OfflineNoteV2WalletNote): OfflineNoteV2SyncResolution? {
        val commitmentKey = note.noteCommitmentHex()
        if (committedAuditOutputs.containsKey(commitmentKey)) {
            return OfflineNoteV2SyncResolution(
                OfflineNoteV2WalletNoteState.SPENDABLE,
                committedAuditOutputs[commitmentKey],
            )
        }
        if (rejectedAuditOutputs.containsKey(commitmentKey)) {
            return OfflineNoteV2SyncResolution(
                OfflineNoteV2WalletNoteState.CANCELLED,
                rejectedAuditOutputs[commitmentKey],
            )
        }
        return null
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
                HttpErrorMessageExtractor.extractRejectCode(response.headers, "x-iroha-reject-code"),
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
                    val issued = OfflineNoteV2WalletNote(
                        chainId = chainId,
                        accountId = accountId,
                        assetId = assetId,
                        amount = amount,
                        keyCertificate = response.keyCertificate ?: context.keyCertificate,
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
        val requestedAmount = decimal(receiveRequest.canonicalAmount)
        val selected = selectSpendableNotes(receiveRequest.assetDefinitionId, requestedAmount)
        val inputAmount = selected.fold(BigDecimal.ZERO) { acc, note -> acc.add(decimal(note.canonicalAmount)) }
        val changeAmount = inputAmount.subtract(requestedAmount)
        require(changeAmount.signum() >= 0) { "selected input amount is below requested amount" }

        val senderCertificate = selected.first().keyCertificate
        val senderCertificateHash = senderCertificate.payloadHash()
        require(selected.all { it.keyCertificate.payloadHash().contentEquals(senderCertificateHash) }) {
            "selected input notes must use the same key certificate"
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
                state = OfflineNoteV2WalletNoteState.CHANGE_PENDING,
                createdAtMs = clock.getAsLong(),
                updatedAtMs = clock.getAsLong(),
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
        val now = clock.getAsLong()
        selected.forEach { store.upsert(it.withState(OfflineNoteV2WalletNoteState.SPEND_PENDING, now)) }
        if (changeNote != null) {
            store.upsert(changeNote)
        }
        return OfflineNoteV2PaymentToken(
            paymentRequestId = receiveRequest.paymentRequestId,
            tokenId = tokenId,
            audit = audit,
            createdAtMs = now,
        )
    }

    fun accept(paymentToken: OfflineNoteV2PaymentToken): CompletableFuture<OfflineNoteV2WalletNote> {
        val submitter = transactionSubmitter ?: return failedFuture(
            IllegalStateException("Offline Note V2 transaction submitter is required for accept")
        )
        paymentToken.audit.validateProofBinding()
        val output = paymentToken.audit.outputClaims.firstOrNull { claim ->
            store.findNote(claim.noteCommitment())?.state == OfflineNoteV2WalletNoteState.RECEIVE_PENDING
        } ?: return failedFuture(IllegalStateException("payment token has no pending output for this wallet"))
        val pending = store.findNote(output.noteCommitment())
            ?: return failedFuture(IllegalStateException("pending receive note is missing"))
        require(pending.assetId == output.assetId) { "payment token output asset does not match receive request" }
        require(pending.canonicalAmount == output.canonicalAmount) {
            "payment token output amount does not match receive request"
        }
        require(output.keyCertificate.payloadHash().contentEquals(pending.keyCertificate.payloadHash())) {
            "payment token output key certificate does not match receive request"
        }
        return submitter.submitAudit(paymentToken.audit).thenApply { response ->
            ensureSuccess(response)
            val accepted = pending.withState(OfflineNoteV2WalletNoteState.SPENDABLE, clock.getAsLong())
            store.upsert(accepted)
            accepted
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
        val pending = current.withState(OfflineNoteV2WalletNoteState.REDEEM_PENDING, clock.getAsLong())
        store.upsert(pending)
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
    OfflineNoteV2WalletNoteState.RECEIVE_PENDING,
    OfflineNoteV2WalletNoteState.CHANGE_PENDING,
    OfflineNoteV2WalletNoteState.SPEND_PENDING,
    OfflineNoteV2WalletNoteState.REDEEM_PENDING,
    -> true
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
