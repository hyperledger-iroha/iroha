package org.hyperledger.iroha.sdk.nexus

import java.math.BigDecimal
import java.math.BigInteger
import java.util.Collections
import org.hyperledger.iroha.sdk.client.ClientResponse
import org.hyperledger.iroha.sdk.client.PipelineStatusOptions
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.numeric.KotodamaQuantity
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.SignedTransactionHasher

/** Static configuration for a SORA Nexus app facade instance. */
class NexusAppConfig @JvmOverloads constructor(
    @JvmField val networkId: NetworkId,
    @JvmField val chainId: String,
    @JvmField val chainDiscriminant: Int,
    @JvmField val appId: String? = null,
    @JvmField val relayUrl: String? = null,
    @JvmField val node: String? = null,
    @JvmField val authority: String? = null,
    signingPublicKey: ByteArray? = null,
    appMetadata: Map<String, String> = emptyMap(),
) {
    private val signingPublicKeyBytes: ByteArray? = signingPublicKey?.copyOf()

    /** Returns an owned copy of signingPublicKey. */
    val signingPublicKey: ByteArray? get() = signingPublicKeyBytes?.copyOf()
    @JvmField val appMetadata: Map<String, String> = immutableStrings(appMetadata)

    init {
        require(chainId.isNotBlank()) { "chainId must not be blank" }
        require(chainDiscriminant in 0..0xffff) {
            "chainDiscriminant must fit in u16"
        }
    }

    override fun equals(other: Any?): Boolean =
        this === other || other is NexusAppConfig &&
            networkId == other.networkId &&
            chainId == other.chainId &&
            chainDiscriminant == other.chainDiscriminant &&
            appId == other.appId &&
            relayUrl == other.relayUrl &&
            node == other.node &&
            authority == other.authority &&
            signingPublicKeyBytes.contentEquals(other.signingPublicKeyBytes) &&
            appMetadata == other.appMetadata

    override fun hashCode(): Int {
        var result = 1
        result = 31 * result + networkId.hashCode()
        result = 31 * result + chainId.hashCode()
        result = 31 * result + chainDiscriminant.hashCode()
        result = 31 * result + (appId?.hashCode() ?: 0)
        result = 31 * result + (relayUrl?.hashCode() ?: 0)
        result = 31 * result + (node?.hashCode() ?: 0)
        result = 31 * result + (authority?.hashCode() ?: 0)
        result = 31 * result + signingPublicKeyBytes.contentHashCode()
        result = 31 * result + appMetadata.hashCode()
        return result
    }
}

/** App-role Connect registration options. */
class NexusConnectOptions @JvmOverloads constructor(
    scopes: Set<String> = emptySet(),
    @JvmField val walletUriBase: String? = null,
    @JvmField val node: String? = null,
    metadata: Map<String, String> = emptyMap(),
    @JvmField val sessionId: String? = null,
) {
    @JvmField val scopes: Set<String> = Collections.unmodifiableSet(LinkedHashSet(scopes))
    @JvmField val metadata: Map<String, String> = immutableStrings(metadata)

    override fun equals(other: Any?): Boolean =
        this === other || other is NexusConnectOptions &&
            scopes == other.scopes &&
            walletUriBase == other.walletUriBase &&
            node == other.node &&
            metadata == other.metadata &&
            sessionId == other.sessionId

    override fun hashCode(): Int {
        var result = 1
        result = 31 * result + scopes.hashCode()
        result = 31 * result + (walletUriBase?.hashCode() ?: 0)
        result = 31 * result + (node?.hashCode() ?: 0)
        result = 31 * result + metadata.hashCode()
        result = 31 * result + (sessionId?.hashCode() ?: 0)
        return result
    }
}

/** Registered Connect session plus wallet launch metadata. */
class NexusConnectSession @JvmOverloads constructor(
    @JvmField val sessionId: String,
    @JvmField val walletLaunchUri: String,
    @JvmField val appId: String? = null,
    @JvmField val relayUrl: String? = null,
    @JvmField val node: String? = null,
    @JvmField val approvedAccount: String? = null,
    signingPublicKey: ByteArray? = null,
    metadata: Map<String, String> = emptyMap(),
) {
    private val signingPublicKeyBytes: ByteArray? = signingPublicKey?.copyOf()

    /** Returns an owned copy of signingPublicKey. */
    val signingPublicKey: ByteArray? get() = signingPublicKeyBytes?.copyOf()
    @JvmField val metadata: Map<String, String> = immutableStrings(metadata)

    init {
        require(sessionId.isNotBlank()) { "sessionId must not be blank" }
        require(walletLaunchUri.isNotBlank()) { "walletLaunchUri must not be blank" }
    }

    override fun equals(other: Any?): Boolean =
        this === other || other is NexusConnectSession &&
            sessionId == other.sessionId &&
            walletLaunchUri == other.walletLaunchUri &&
            appId == other.appId &&
            relayUrl == other.relayUrl &&
            node == other.node &&
            approvedAccount == other.approvedAccount &&
            signingPublicKeyBytes.contentEquals(other.signingPublicKeyBytes) &&
            metadata == other.metadata

    override fun hashCode(): Int {
        var result = 1
        result = 31 * result + sessionId.hashCode()
        result = 31 * result + walletLaunchUri.hashCode()
        result = 31 * result + (appId?.hashCode() ?: 0)
        result = 31 * result + (relayUrl?.hashCode() ?: 0)
        result = 31 * result + (node?.hashCode() ?: 0)
        result = 31 * result + (approvedAccount?.hashCode() ?: 0)
        result = 31 * result + signingPublicKeyBytes.contentHashCode()
        result = 31 * result + metadata.hashCode()
        return result
    }
}

/** Wallet approval result; transports leave [session] null and the facade supplies its caller copy. */
class NexusApprovedAccount @JvmOverloads constructor(
    @JvmField val accountId: String,
    signingPublicKey: ByteArray? = null,
    @JvmField val session: NexusConnectSession? = null,
) {
    private val signingPublicKeyBytes: ByteArray? = signingPublicKey?.copyOf()

    /** Returns an owned copy of signingPublicKey. */
    val signingPublicKey: ByteArray? get() = signingPublicKeyBytes?.copyOf()

    override fun equals(other: Any?): Boolean =
        this === other || other is NexusApprovedAccount &&
            accountId == other.accountId &&
            signingPublicKeyBytes.contentEquals(other.signingPublicKeyBytes) &&
            session == other.session

    override fun hashCode(): Int {
        var result = 1
        result = 31 * result + accountId.hashCode()
        result = 31 * result + signingPublicKeyBytes.contentHashCode()
        result = 31 * result + (session?.hashCode() ?: 0)
        return result
    }
}

/** Input for the V1 Quantity asset transfer flow. */
class NexusTransferInput @JvmOverloads constructor(
    @JvmField val sourceAssetId: String,
    @JvmField val quantity: String,
    @JvmField val destinationAccountId: String,
    @JvmField val feePayment: FeePaymentIntent,
    @JvmField val authority: String? = null,
    signingPublicKey: ByteArray? = null,
    @JvmField val creationTimeMs: Long? = null,
    @JvmField val ttlMs: Long? = null,
    @JvmField val nonce: Long? = null,
    metadata: Map<String, String> = emptyMap(),
) {
    private val signingPublicKeyBytes: ByteArray? = signingPublicKey?.copyOf()

    /** Returns an owned copy of signingPublicKey. */
    val signingPublicKey: ByteArray? get() = signingPublicKeyBytes?.copyOf()
    @JvmField val metadata: Map<String, String> = immutableStrings(metadata)

    init {
        require(sourceAssetId.isNotBlank()) { "sourceAssetId must not be blank" }
        require(destinationAccountId.isNotBlank()) { "destinationAccountId must not be blank" }
        KotodamaQuantity.parseCanonical(quantity)
    }

    /** Construct the minimal transfer input from a lossless validated quantity value. */
    constructor(
        sourceAssetId: String,
        quantity: KotodamaQuantity,
        destinationAccountId: String,
        feePayment: FeePaymentIntent,
    ) : this(sourceAssetId, quantity.toString(), destinationAccountId, feePayment)
    override fun equals(other: Any?): Boolean =
        this === other || other is NexusTransferInput &&
            sourceAssetId == other.sourceAssetId &&
            quantity == other.quantity &&
            destinationAccountId == other.destinationAccountId &&
            feePayment == other.feePayment &&
            authority == other.authority &&
            signingPublicKeyBytes.contentEquals(other.signingPublicKeyBytes) &&
            creationTimeMs == other.creationTimeMs &&
            ttlMs == other.ttlMs &&
            nonce == other.nonce &&
            metadata == other.metadata

    override fun hashCode(): Int {
        var result = 1
        result = 31 * result + sourceAssetId.hashCode()
        result = 31 * result + quantity.hashCode()
        result = 31 * result + destinationAccountId.hashCode()
        result = 31 * result + feePayment.hashCode()
        result = 31 * result + (authority?.hashCode() ?: 0)
        result = 31 * result + signingPublicKeyBytes.contentHashCode()
        result = 31 * result + (creationTimeMs?.hashCode() ?: 0)
        result = 31 * result + (ttlMs?.hashCode() ?: 0)
        result = 31 * result + (nonce?.hashCode() ?: 0)
        result = 31 * result + metadata.hashCode()
        return result
    }
}

/** Canonical transaction payload to be signed by a wallet. */
class NexusSignableTransaction @JvmOverloads constructor(
    payloadBytes: ByteArray,
    @JvmField val authority: String,
    signingPublicKey: ByteArray,
    @JvmField val signatureAlgorithm: String = NEXUS_SIGNATURE_ALGORITHM_ED25519,
) {
    private val ownedPayload: ByteArray = payloadBytes.copyOf()

    /** Returns an owned copy of payloadBytes. */
    val payloadBytes: ByteArray get() = ownedPayload.copyOf()
    private val signingPublicKeyBytes: ByteArray = signingPublicKey.copyOf()

    /** Returns an owned copy of signingPublicKey. */
    val signingPublicKey: ByteArray get() = signingPublicKeyBytes.copyOf()

    /** Hash of the exact owned payload; callers cannot supply a conflicting hash. */
    @JvmField val payloadHashHex: String = IrohaHash.prehash(ownedPayload)
        .joinToString("") { "%02x".format(it.toInt() and 0xff) }

    init {
        require(authority.isNotBlank()) { "authority must not be blank" }
        requireNexusEd25519(signatureAlgorithm)
    }

    override fun equals(other: Any?): Boolean =
        this === other || other is NexusSignableTransaction &&
            ownedPayload.contentEquals(other.ownedPayload) &&
            authority == other.authority &&
            signingPublicKeyBytes.contentEquals(other.signingPublicKeyBytes) &&
            signatureAlgorithm == other.signatureAlgorithm

    override fun hashCode(): Int {
        var result = 1
        result = 31 * result + ownedPayload.contentHashCode()
        result = 31 * result + authority.hashCode()
        result = 31 * result + signingPublicKeyBytes.contentHashCode()
        result = 31 * result + signatureAlgorithm.hashCode()
        return result
    }
}

/** Transfer draft containing both the normalized input and signable payload. */
class NexusTransferDraft(
    @JvmField val input: NexusTransferInput,
    @JvmField val signable: NexusSignableTransaction,
) {
    override fun equals(other: Any?): Boolean =
        this === other || other is NexusTransferDraft &&
            input == other.input &&
            signable == other.signable

    override fun hashCode(): Int {
        var result = 1
        result = 31 * result + input.hashCode()
        result = 31 * result + signable.hashCode()
        return result
    }
}

/** Wallet signature over [NexusSignableTransaction.payloadBytes]. */
class NexusWalletSignature @JvmOverloads constructor(
    signature: ByteArray,
    @JvmField val algorithm: String = NEXUS_SIGNATURE_ALGORITHM_ED25519,
) {
    private val signatureBytes: ByteArray = signature.copyOf()

    /** Returns an owned copy of signature. */
    val signature: ByteArray get() = signatureBytes.copyOf()

    init {
        requireNexusEd25519(algorithm)
    }

    override fun equals(other: Any?): Boolean =
        this === other || other is NexusWalletSignature &&
            signatureBytes.contentEquals(other.signatureBytes) &&
            algorithm == other.algorithm

    override fun hashCode(): Int {
        var result = 1
        result = 31 * result + signatureBytes.contentHashCode()
        result = 31 * result + algorithm.hashCode()
        return result
    }
}

/** Finalization policy. Instances retain identity semantics because status options can own observers. */
class NexusFinalizeOptions @JvmOverloads constructor(
    @JvmField val waitForFinalStatus: Boolean = true,
    @JvmField val pipelineStatusOptions: PipelineStatusOptions? = null,
)

/** Immutable submission observation. Receipts retain identity semantics; compare transactionHashHex for identity. */
class NexusTransferReceipt @JvmOverloads constructor(
    @JvmField val transactionHashHex: String,
    @JvmField val signedTransaction: SignedTransaction,
    @JvmField val submission: ClientResponse,
    finalStatus: Map<String, Any?>? = null,
) {
    /** Deeply immutable decoded JSON status, or null when no final status was requested. */
    @JvmField val finalStatus: Map<String, Any?>? = finalStatus?.let { immutableStatusObject(it, 0) }

    init {
        require(transactionHashHex.matches(Regex("[0-9a-f]{63}[13579bdf]"))) {
            "transactionHashHex must match [0-9a-f]{63}[13579bdf] with the Iroha HashOf marker"
        }
        require(transactionHashHex == SignedTransactionHasher.hashHex(signedTransaction)) {
            "transactionHashHex must identify the exact signed transaction"
        }
    }
}

private fun immutableStrings(values: Map<String, String>): Map<String, String> =
    Collections.unmodifiableMap(LinkedHashMap(values))

private fun requireNexusEd25519(algorithm: String) {
    if (algorithm != NEXUS_SIGNATURE_ALGORITHM_ED25519) {
        throw NexusAppError(
            "unsupported_signature_algorithm",
            "Nexus signatures must use the canonical ed25519 algorithm",
        )
    }
}

private fun immutableStatusObject(values: Map<*, *>, depth: Int): Map<String, Any?> {
    require(depth < 64) { "finalStatus exceeds 64 levels of JSON nesting" }
    val snapshot = LinkedHashMap<String, Any?>()
    for ((key, value) in values) {
        require(key is String) { "finalStatus JSON object keys must be strings" }
        snapshot[key] = immutableStatusValue(value, depth + 1)
    }
    return Collections.unmodifiableMap(snapshot)
}

private fun immutableStatusValue(value: Any?, depth: Int): Any? = when (value) {
    null, is String, is Boolean, is Byte, is Short, is Int, is Long,
    is BigInteger, is BigDecimal -> value
    is Float -> value.also { require(it.isFinite()) { "finalStatus numbers must be finite" } }
    is Double -> value.also { require(it.isFinite()) { "finalStatus numbers must be finite" } }
    is Map<*, *> -> immutableStatusObject(value, depth)
    is List<*> -> {
        require(depth < 64) { "finalStatus exceeds 64 levels of JSON nesting" }
        Collections.unmodifiableList(value.map { immutableStatusValue(it, depth + 1) })
    }
    else -> throw IllegalArgumentException("finalStatus must contain only JSON values")
}
