package org.hyperledger.iroha.sdk.offline

import java.math.BigDecimal
import java.security.SecureRandom
import java.util.UUID
import java.util.concurrent.CompletableFuture
import java.util.function.LongSupplier
import org.hyperledger.iroha.sdk.client.ClientResponse
import org.hyperledger.iroha.sdk.client.IrohaClient
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
    REDEEM_PENDING,
    REDEEMED,
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

/** Submits direct Offline Note V2 audit/redeem transactions. */
interface OfflineNoteV2TransactionSubmitter {
    fun submitAudit(audit: OfflineNoteV2.AuditBundleV2): CompletableFuture<ClientResponse>
    fun submitRedeem(redemption: OfflineNoteV2.RedeemV2): CompletableFuture<ClientResponse>
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
        // TODO: Reconcile CHANGE_PENDING/SPEND_PENDING/REDEEM_PENDING notes against Torii
        // pipeline status once the SDK exposes a transaction-outcome index for Offline Note V2.
        return CompletableFuture.completedFuture(store.listNotes())
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
