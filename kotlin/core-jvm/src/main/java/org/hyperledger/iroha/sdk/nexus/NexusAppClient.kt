package org.hyperledger.iroha.sdk.nexus

import java.util.concurrent.CompletionException
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters
import org.bouncycastle.crypto.signers.Ed25519Signer
import org.hyperledger.iroha.sdk.client.ClientResponse
import org.hyperledger.iroha.sdk.client.IrohaClient
import org.hyperledger.iroha.sdk.client.PipelineStatusOptions
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.core.model.instructions.TransferWirePayloadEncoder
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.SignedTransactionHasher
import org.hyperledger.iroha.sdk.tx.norito.NoritoCodecAdapter
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter

const val NEXUS_SIGNATURE_ALGORITHM_ED25519: String = "ed25519"

/** Typed error raised by [NexusAppClient]. */
class NexusAppError(
    @JvmField val code: String,
    message: String,
    cause: Throwable? = null,
) : RuntimeException(message, cause)

/** Static configuration for a SORA Nexus app facade instance. */
data class NexusAppConfig @JvmOverloads constructor(
    @JvmField val chainId: String,
    @JvmField val appId: String? = null,
    @JvmField val relayUrl: String? = null,
    @JvmField val node: String? = null,
    @JvmField val authority: String? = null,
    @JvmField val signingPublicKey: ByteArray? = null,
    @JvmField val appMetadata: Map<String, String> = emptyMap(),
)

/** App-role Connect registration options. */
data class NexusConnectOptions @JvmOverloads constructor(
    @JvmField val scopes: Set<String> = emptySet(),
    @JvmField val walletUriBase: String? = null,
    @JvmField val node: String? = null,
    @JvmField val metadata: Map<String, String> = emptyMap(),
    @JvmField val sessionId: String? = null,
)

/** Registered Connect session plus wallet launch metadata. */
data class NexusConnectSession @JvmOverloads constructor(
    @JvmField val sessionId: String,
    @JvmField val walletLaunchUri: String,
    @JvmField val appId: String? = null,
    @JvmField val relayUrl: String? = null,
    @JvmField val node: String? = null,
    @JvmField val approvedAccount: String? = null,
    @JvmField val signingPublicKey: ByteArray? = null,
    @JvmField val metadata: Map<String, String> = emptyMap(),
)

/** Wallet approval result for an app-role Connect session. */
data class NexusApprovedAccount @JvmOverloads constructor(
    @JvmField val accountId: String,
    @JvmField val signingPublicKey: ByteArray? = null,
    @JvmField val session: NexusConnectSession? = null,
)

/** Input for the V1 numeric asset transfer flow. */
data class NexusTransferInput @JvmOverloads constructor(
    @JvmField val sourceAssetId: String,
    @JvmField val quantity: String,
    @JvmField val destinationAccountId: String,
    @JvmField val authority: String? = null,
    @JvmField val signingPublicKey: ByteArray? = null,
    @JvmField val creationTimeMs: Long? = null,
    @JvmField val ttlMs: Long? = null,
    @JvmField val nonce: Int? = null,
    @JvmField val metadata: Map<String, String> = emptyMap(),
)

/** Canonical transaction payload to be signed by a wallet. */
data class NexusSignableTransaction(
    @JvmField val payloadBytes: ByteArray,
    @JvmField val payloadHashHex: String,
    @JvmField val authority: String,
    @JvmField val signingPublicKey: ByteArray,
    @JvmField val signatureAlgorithm: String = NEXUS_SIGNATURE_ALGORITHM_ED25519,
)

/** Transfer draft containing both the normalized input and signable payload. */
data class NexusTransferDraft(
    @JvmField val input: NexusTransferInput,
    @JvmField val signable: NexusSignableTransaction,
)

/** Wallet signature over [NexusSignableTransaction.payloadBytes]. */
data class NexusWalletSignature @JvmOverloads constructor(
    @JvmField val signature: ByteArray,
    @JvmField val algorithm: String = NEXUS_SIGNATURE_ALGORITHM_ED25519,
)

/** Options for signing finalization and Torii pipeline waiting. */
data class NexusFinalizeOptions @JvmOverloads constructor(
    @JvmField val waitForFinalStatus: Boolean = true,
    @JvmField val pipelineStatusOptions: PipelineStatusOptions? = null,
)

/** Receipt returned after a signed transfer is finalized and submitted. */
data class NexusTransferReceipt(
    @JvmField val transactionHashHex: String,
    @JvmField val signedTransaction: SignedTransaction,
    @JvmField val submission: ClientResponse,
    @JvmField val finalStatus: Map<String, Any>? = null,
)

/** App-role Connect dependency used by [NexusAppClient]. */
interface NexusConnectTransport {
    fun startConnect(options: NexusConnectOptions, config: NexusAppConfig): NexusConnectSession

    fun awaitApproval(session: NexusConnectSession, config: NexusAppConfig): NexusApprovedAccount

    fun requestSignature(
        session: NexusConnectSession,
        signable: NexusSignableTransaction,
        config: NexusAppConfig,
    ): NexusWalletSignature
}

/** High-level SORA Nexus app facade for Connect wallet transfer flows. */
class NexusAppClient @JvmOverloads constructor(
    private val config: NexusAppConfig,
    private val connectTransport: NexusConnectTransport? = null,
    private val codecAdapter: NoritoCodecAdapter = NoritoJavaCodecAdapter(),
    private val toriiClient: IrohaClient? = null,
) {

    fun startConnect(options: NexusConnectOptions = NexusConnectOptions()): NexusConnectSession {
        val transport = connectTransport ?: throw NexusAppError(
            "connect_transport_unavailable",
            "Connect transport is required to start a Nexus Connect session",
        )
        return transport.startConnect(options, config)
    }

    fun awaitApproval(session: NexusConnectSession): NexusApprovedAccount {
        val transport = connectTransport ?: throw NexusAppError(
            "connect_transport_unavailable",
            "Connect transport is required to await wallet approval",
        )
        val approved = transport.awaitApproval(session, config)
        if (approved.accountId.isBlank()) {
            throw NexusAppError("approval_missing_account", "wallet approval did not include an account")
        }
        val publicKey = approved.signingPublicKey
            ?: session.signingPublicKey
            ?: config.signingPublicKey
            ?: throw NexusAppError(
                "missing_signing_public_key",
                "wallet approval did not include a signing public key",
            )
        if (publicKey.isEmpty()) {
            throw NexusAppError("missing_signing_public_key", "signing public key must not be empty")
        }
        validateEd25519PublicKey(publicKey)
        val approvedSession = approved.session ?: session.copy(
            approvedAccount = approved.accountId,
            signingPublicKey = publicKey.copyOf(),
        )
        return approved.copy(signingPublicKey = publicKey.copyOf(), session = approvedSession)
    }

    fun buildTransferDraft(input: NexusTransferInput): NexusTransferDraft {
        val authority = input.authority ?: config.authority ?: throw NexusAppError(
            "missing_authority",
            "transfer authority is required",
        )
        val signingPublicKey = input.signingPublicKey ?: config.signingPublicKey ?: throw NexusAppError(
            "missing_signing_public_key",
            "signing public key is required for an externally signed transfer",
        )
        validateEd25519PublicKey(signingPublicKey)
        val normalized = input.copy(
            authority = authority,
            signingPublicKey = signingPublicKey.copyOf(),
        )
        val instruction = TransferWirePayloadEncoder.encodeAssetTransfer(
            normalized.sourceAssetId,
            normalized.quantity,
            normalized.destinationAccountId,
        )
        val payload = TransactionPayload(
            chainId = config.chainId,
            authority = authority,
            creationTimeMs = normalized.creationTimeMs ?: System.currentTimeMillis(),
            executable = Executable.instructions(listOf(instruction)),
            timeToLiveMs = normalized.ttlMs,
            nonce = normalized.nonce,
            metadata = normalized.metadata,
        )
        val payloadBytes = codecAdapter.encodeTransaction(payload)
        val signable = NexusSignableTransaction(
            payloadBytes = payloadBytes,
            payloadHashHex = toHex(IrohaHash.prehash(payloadBytes)),
            authority = authority,
            signingPublicKey = signingPublicKey.copyOf(),
        )
        return NexusTransferDraft(normalized, signable)
    }

    fun requestSignature(
        session: NexusConnectSession,
        signable: NexusSignableTransaction,
    ): NexusWalletSignature {
        val transport = connectTransport ?: throw NexusAppError(
            "connect_transport_unavailable",
            "Connect transport is required to request a wallet signature",
        )
        ensureEd25519(signable.signatureAlgorithm)
        val signature = transport.requestSignature(session, signable, config)
        ensureEd25519(signature.algorithm)
        validateEd25519Signature(signature.signature)
        return NexusWalletSignature(signature.signature.copyOf(), NEXUS_SIGNATURE_ALGORITHM_ED25519)
    }

    fun finalizeAndSubmit(
        signable: NexusSignableTransaction,
        signature: NexusWalletSignature,
        options: NexusFinalizeOptions = NexusFinalizeOptions(),
    ): NexusTransferReceipt {
        ensureEd25519(signable.signatureAlgorithm)
        ensureEd25519(signature.algorithm)
        validateEd25519PublicKey(signable.signingPublicKey)
        validateEd25519Signature(signature.signature)
        validateEd25519SignatureForPayload(
            signable.signingPublicKey,
            signable.payloadBytes,
            signature.signature,
        )

        val signed = SignedTransaction.builder()
            .setEncodedPayload(signable.payloadBytes)
            .setSignature(signature.signature)
            .setPublicKey(signable.signingPublicKey)
            .setSchemaName(codecAdapter.schemaName())
            .build()
        val transactionHashHex = SignedTransactionHasher.hashHex(signed)
        val client = toriiClient ?: throw NexusAppError(
            "torii_client_unavailable",
            "Torii client is required to submit a signed Nexus transfer",
        )
        val submission = joinClientFuture("submit_failed", "failed to submit signed transfer to Torii") {
            client.submitTransaction(signed).join()
        }
        val submittedHash = submission.hashHex()
        if (submittedHash != null && submittedHash != transactionHashHex) {
            throw NexusAppError(
                "transaction_hash_mismatch",
                "Torii returned transaction hash $submittedHash but local hash is $transactionHashHex",
            )
        }
        val finalStatus = if (options.waitForFinalStatus) {
            joinClientFuture("status_wait_failed", "failed while waiting for Torii pipeline status") {
                client.waitForTransactionStatus(transactionHashHex, options.pipelineStatusOptions).join()
            }
        } else {
            null
        }
        return NexusTransferReceipt(transactionHashHex, signed, submission, finalStatus)
    }

    fun transferWithWallet(
        session: NexusConnectSession,
        input: NexusTransferInput,
        options: NexusFinalizeOptions = NexusFinalizeOptions(),
    ): NexusTransferReceipt {
        val authority = input.authority
            ?: session.approvedAccount
            ?: config.authority
            ?: throw NexusAppError("missing_authority", "transfer authority is required")
        if (session.approvedAccount != null && input.authority != null && session.approvedAccount != input.authority) {
            throw NexusAppError(
                "approval_account_mismatch",
                "transfer authority does not match the approved wallet account",
            )
        }
        val signingPublicKey = input.signingPublicKey
            ?: session.signingPublicKey
            ?: config.signingPublicKey
            ?: throw NexusAppError(
                "missing_signing_public_key",
                "approved account did not provide a signing public key",
            )
        val draft = buildTransferDraft(
            input.copy(
                authority = authority,
                signingPublicKey = signingPublicKey.copyOf(),
            ),
        )
        val walletSignature = requestSignature(session, draft.signable)
        return finalizeAndSubmit(draft.signable, walletSignature, options)
    }
}

private fun ensureEd25519(algorithm: String) {
    if (
        !algorithm.all { it.code in 0x20..0x7E } ||
        !(algorithm.equals(NEXUS_SIGNATURE_ALGORITHM_ED25519, ignoreCase = true) || algorithm == "0")
    ) {
        throw NexusAppError(
            "unsupported_signature_algorithm",
            "Nexus App Facade V1 supports Ed25519 signatures only",
        )
    }
}

private fun validateEd25519PublicKey(publicKey: ByteArray) {
    if (publicKey.size != 32) {
        throw NexusAppError(
            "invalid_signing_public_key",
            "Ed25519 signing public key must be 32 bytes",
        )
    }
}

private fun validateEd25519Signature(signature: ByteArray) {
    if (signature.size != 64) {
        throw NexusAppError("invalid_signature", "Ed25519 signature must be 64 bytes")
    }
}

private fun validateEd25519SignatureForPayload(
    publicKey: ByteArray,
    payloadBytes: ByteArray,
    signature: ByteArray,
) {
    val message = IrohaHash.prehash(payloadBytes)
    val verified = try {
        val verifier = Ed25519Signer()
        verifier.init(false, Ed25519PublicKeyParameters(publicKey, 0))
        verifier.update(message, 0, message.size)
        verifier.verifySignature(signature)
    } catch (ex: RuntimeException) {
        false
    }
    if (!verified) {
        throw NexusAppError(
            "invalid_signature",
            "Ed25519 signature does not verify for the signable payload",
        )
    }
}

private fun <T> joinClientFuture(code: String, message: String, block: () -> T): T {
    try {
        return block()
    } catch (ex: CompletionException) {
        val cause = ex.cause ?: ex
        throw NexusAppError(code, "$message: ${cause.message ?: cause.javaClass.simpleName}", cause)
    } catch (ex: RuntimeException) {
        throw NexusAppError(code, "$message: ${ex.message ?: ex.javaClass.simpleName}", ex)
    }
}

private fun toHex(data: ByteArray): String {
    val builder = StringBuilder(data.size * 2)
    for (byte in data) {
        builder.append(String.format("%02x", byte))
    }
    return builder.toString()
}
