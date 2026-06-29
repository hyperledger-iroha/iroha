package org.hyperledger.iroha.sdk.offline

import java.util.Base64
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlin.coroutines.suspendCoroutine

object OfflineCashTransportKinds {
    const val QR: String = "qr"
    const val NFC: String = "nfc"
    const val NEARBY: String = "nearby"
}

class OfflineCashConfigurationSnapshotException(
    val code: String,
    message: String,
) : IllegalStateException(message)

class OfflineCashConfigurationSnapshot(
    val chainId: String,
    val assetDefinitionId: String,
    val offlinePaymentsEnabled: Boolean,
    val issuerPublicKeyBase64: String?,
    val nativeBridgeAbiVersion: Int? = null,
    val artifactSetId: String? = null,
    val circuitId: String? = null,
    val createdAtMs: Long = 0L,
    val expiresAtMs: Long? = null,
) {
    fun requireUsableForOfflineExchange(
        nowMs: Long,
        requiredNativeBridgeAbiVersion: Int? = null,
    ) {
        val checkedNowMs = nonnegativeOfflineCashSnapshotTimestamp(nowMs, "nowMs")
        if (!offlinePaymentsEnabled) {
            throw OfflineCashConfigurationSnapshotException(
                "offline_payments_disabled",
                "Offline cash is disabled in the cached configuration snapshot.",
            )
        }
        requireCanonicalOfflineCashSnapshotText(chainId, "chainId")
        requireCanonicalOfflineCashSnapshotText(assetDefinitionId, "assetDefinitionId")
        requireOptionalCanonicalOfflineCashSnapshotText(artifactSetId, "artifactSetId")
        requireOptionalCanonicalOfflineCashSnapshotText(circuitId, "circuitId")
        if (!isValidOfflineIssuerPublicKeyBase64(issuerPublicKeyBase64)) {
            throw OfflineCashConfigurationSnapshotException(
                "missing_issuer_public_key",
                "Offline cash requires a cached issuer public key before offline exchange.",
            )
        }
        val checkedCreatedAtMs = nonnegativeOfflineCashSnapshotTimestamp(createdAtMs, "createdAtMs")
        val checkedExpiresAtMs = optionalNonnegativeOfflineCashSnapshotTimestamp(
            expiresAtMs,
            "expiresAtMs",
        )
        if (checkedExpiresAtMs != null && checkedExpiresAtMs <= checkedCreatedAtMs) {
            throw OfflineCashConfigurationSnapshotException(
                "malformed_snapshot",
                "Offline cash configuration snapshot field expiresAtMs must be after createdAtMs.",
            )
        }
        if (checkedExpiresAtMs != null && checkedExpiresAtMs <= checkedNowMs) {
            throw OfflineCashConfigurationSnapshotException(
                "expired",
                "Offline cash configuration snapshot expired at $checkedExpiresAtMs.",
            )
        }
        val checkedNativeBridgeAbiVersion = positiveNativeBridgeAbiVersion(
            nativeBridgeAbiVersion,
            "nativeBridgeAbiVersion",
        )
        val checkedRequiredNativeBridgeAbiVersion = positiveNativeBridgeAbiVersion(
            requiredNativeBridgeAbiVersion,
            "requiredNativeBridgeAbiVersion",
        )
        if (checkedRequiredNativeBridgeAbiVersion != null &&
            (
                checkedNativeBridgeAbiVersion == null ||
                    checkedNativeBridgeAbiVersion < checkedRequiredNativeBridgeAbiVersion
                )
        ) {
            throw OfflineCashConfigurationSnapshotException(
                "unsupported_native_bridge_abi",
                "Offline cash requires native bridge ABI $checkedRequiredNativeBridgeAbiVersion.",
            )
        }
    }
}

private fun positiveNativeBridgeAbiVersion(value: Int?, fieldName: String): Int? {
    if (value == null) return null
    if (value <= 0) {
        throw OfflineCashConfigurationSnapshotException(
            "malformed_snapshot",
            "Offline cash configuration snapshot field $fieldName must be a positive integer.",
        )
    }
    return value
}

private fun nonnegativeOfflineCashSnapshotTimestamp(value: Long, fieldName: String): Long {
    if (value < 0L) {
        throw OfflineCashConfigurationSnapshotException(
            "malformed_snapshot",
            "Offline cash configuration snapshot field $fieldName must be a nonnegative integer timestamp.",
        )
    }
    return value
}

private fun optionalNonnegativeOfflineCashSnapshotTimestamp(value: Long?, fieldName: String): Long? {
    if (value == null) return null
    return nonnegativeOfflineCashSnapshotTimestamp(value, fieldName)
}

private fun requireCanonicalOfflineCashSnapshotText(value: String?, fieldName: String) {
    if (!isCanonicalOfflineCashSnapshotText(value)) {
        throw OfflineCashConfigurationSnapshotException(
            "malformed_snapshot",
            "Offline cash configuration snapshot field $fieldName must be a non-empty printable ASCII string with no whitespace.",
        )
    }
}

private fun requireOptionalCanonicalOfflineCashSnapshotText(value: String?, fieldName: String) {
    if (value == null) return
    requireCanonicalOfflineCashSnapshotText(value, fieldName)
}

private fun isCanonicalOfflineCashSnapshotText(value: String?): Boolean =
    value != null && value.isNotEmpty() && value.all { scalar ->
        scalar.code > 0x20 && scalar.code <= 0x7E
    }

private fun isValidOfflineIssuerPublicKeyBase64(value: String?): Boolean {
    if (!isCanonicalOfflineCashSnapshotText(value)) return false
    if (value == null || value.contains("=")) return false
    val normalized = value.replace('-', '+').replace('_', '/')
    if (normalized.length % 4 == 1 || normalized.any { it !in BASE64_ALPHABET }) return false
    val padded = normalized + "=".repeat((4 - (normalized.length % 4)) % 4)
    val raw = try {
        Base64.getDecoder().decode(padded)
    } catch (_: IllegalArgumentException) {
        return false
    }
    return raw.size == 32 && Base64.getEncoder().withoutPadding().encodeToString(raw) == normalized
}

private const val BASE64_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"

interface OfflineCashAuditReceiptSynchronizer {
    suspend fun hasPendingAuditReceipts(): Boolean

    suspend fun syncPendingAuditReceipts()
}

interface OfflineCashLifecycleWallet {
    suspend fun load(assetDefinitionId: String, amount: String): Any?

    fun prepareReceive(assetDefinitionId: String, amount: String): Any?

    fun createPayment(receiveRequest: Any): Any?

    fun acceptPayment(paymentToken: Any): Any?

    suspend fun redeem(note: Any, recipient: String? = null): Any?
}

class OfflineCashLifecycleController(
    private val wallet: OfflineCashLifecycleWallet,
    private val auditReceiptSynchronizer: OfflineCashAuditReceiptSynchronizer? = null,
) {
    constructor(
        wallet: OfflineNoteWallet,
        auditReceiptSynchronizer: OfflineCashAuditReceiptSynchronizer? = null,
    ) : this(OfflineNoteWalletLifecycleAdapter(wallet), auditReceiptSynchronizer)

    suspend fun syncPendingAuditReceiptsIfNeeded(): Boolean {
        val synchronizer = auditReceiptSynchronizer ?: return false
        if (!synchronizer.hasPendingAuditReceipts()) return false
        synchronizer.syncPendingAuditReceipts()
        return true
    }

    suspend fun load(assetDefinitionId: String, amount: String): Any? {
        syncPendingAuditReceiptsIfNeeded()
        return wallet.load(assetDefinitionId, amount)
    }

    fun prepareReceive(assetDefinitionId: String, amount: String): Any? =
        wallet.prepareReceive(assetDefinitionId, amount)

    fun createPayment(receiveRequest: Any): Any? = wallet.createPayment(receiveRequest)

    fun acceptPayment(paymentToken: Any): Any? = wallet.acceptPayment(paymentToken)

    suspend fun redeem(note: Any, recipient: String? = null): Any? = wallet.redeem(note, recipient)
}

private class OfflineNoteWalletLifecycleAdapter(
    private val wallet: OfflineNoteWallet,
) : OfflineCashLifecycleWallet {
    override suspend fun load(assetDefinitionId: String, amount: String): Any? =
        wallet.load(assetDefinitionId, amount).awaitOfflineCash()

    override fun prepareReceive(assetDefinitionId: String, amount: String): Any? =
        wallet.prepareReceive(assetDefinitionId, amount)

    override fun createPayment(receiveRequest: Any): Any? {
        require(receiveRequest is OfflineNoteReceiveRequest) {
            "receiveRequest must be OfflineNoteReceiveRequest"
        }
        return wallet.pay(receiveRequest)
    }

    override fun acceptPayment(paymentToken: Any): Any? {
        require(paymentToken is OfflineNotePaymentToken) {
            "paymentToken must be OfflineNotePaymentToken"
        }
        return wallet.accept(paymentToken)
    }

    override suspend fun redeem(note: Any, recipient: String?): Any? {
        require(note is OfflineNoteWalletNote) {
            "note must be OfflineNoteWalletNote"
        }
        return if (recipient == null) {
            wallet.redeem(note).awaitOfflineCash()
        } else {
            wallet.redeem(note, recipient).awaitOfflineCash()
        }
    }
}

private suspend fun <T> CompletableFuture<T>.awaitOfflineCash(): T =
    suspendCoroutine { continuation ->
        whenComplete { value, error ->
            if (error != null) {
                continuation.resumeWithException(unwrapOfflineCashCompletion(error))
            } else {
                continuation.resume(value)
            }
        }
    }

private fun unwrapOfflineCashCompletion(error: Throwable): Throwable =
    if (error is CompletionException && error.cause != null) error.cause!! else error
