package org.hyperledger.iroha.sdk.offline

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
    val bridgeAbiVersion: Int? = null,
    val artifactSetId: String? = null,
    val circuitId: String? = null,
    val createdAtMs: Long = 0L,
    val expiresAtMs: Long? = null,
) {
    fun requireUsableForOfflineExchange(
        nowMs: Long,
        requiredBridgeAbiVersion: Int? = null,
    ) {
        if (!offlinePaymentsEnabled) {
            throw OfflineCashConfigurationSnapshotException(
                "offline_payments_disabled",
                "Offline cash is disabled in the cached configuration snapshot.",
            )
        }
        if (issuerPublicKeyBase64.isNullOrBlank()) {
            throw OfflineCashConfigurationSnapshotException(
                "missing_issuer_public_key",
                "Offline cash requires a cached issuer public key before offline exchange.",
            )
        }
        if (expiresAtMs != null && expiresAtMs <= nowMs) {
            throw OfflineCashConfigurationSnapshotException(
                "expired",
                "Offline cash configuration snapshot expired at $expiresAtMs.",
            )
        }
        if (requiredBridgeAbiVersion != null &&
            (bridgeAbiVersion == null || bridgeAbiVersion < requiredBridgeAbiVersion)
        ) {
            throw OfflineCashConfigurationSnapshotException(
                "unsupported_bridge_abi",
                "Offline cash requires bridge ABI $requiredBridgeAbiVersion.",
            )
        }
    }
}

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
