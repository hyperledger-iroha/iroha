package org.hyperledger.iroha.sdk.offline

import kotlin.coroutines.Continuation
import kotlin.coroutines.EmptyCoroutineContext
import kotlin.coroutines.startCoroutine
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

class OfflineCashLifecycleTest {
    @Test
    fun `transport capabilities hide unsupported NFC`() {
        val capabilities = OfflineNoteTransferCapabilities.current(
            androidHceSupported = false,
            nearbyAvailable = true,
        )

        assertEquals(
            listOf(OfflineNoteTransferModality.QR_STREAMING, OfflineNoteTransferModality.NEARBY),
            capabilities.supportedModalities(),
        )
    }

    @Test
    fun `offline cash lifecycle syncs pending receipts before load`() {
        val events = ArrayList<String>()
        val controller = OfflineCashLifecycleController(
            wallet = RecordingWallet(events),
            auditReceiptSynchronizer = RecordingSynchronizer(events, hasPending = true),
        )

        val result = runSuspend { controller.load("pkr#sbp", "10") }

        assertEquals("ok", result)
        assertEquals(listOf("hasPending", "sync", "load:pkr#sbp:10"), events)
    }

    @Test
    fun `offline cash lifecycle does not load when audit sync fails`() {
        val events = ArrayList<String>()
        val controller = OfflineCashLifecycleController(
            wallet = RecordingWallet(events),
            auditReceiptSynchronizer = RecordingSynchronizer(
                events = events,
                hasPending = true,
                syncFailure = IllegalStateException("audit sync failed"),
            ),
        )

        val error = assertFailsWith<IllegalStateException> {
            runSuspend { controller.load("pkr#sbp", "10") }
        }

        assertEquals("audit sync failed", error.message)
        assertEquals(listOf("hasPending", "sync"), events)
    }

    @Test
    fun `configuration snapshot requires cached issuer key`() {
        val snapshot = OfflineCashConfigurationSnapshot(
            chainId = "00000042",
            assetDefinitionId = "pkr#sbp",
            offlinePaymentsEnabled = true,
            issuerPublicKeyBase64 = "issuer-key",
            bridgeAbiVersion = 7,
            createdAtMs = 100,
            expiresAtMs = 1_000,
        )
        snapshot.requireUsableForOfflineExchange(nowMs = 999, requiredBridgeAbiVersion = 7)

        val error = assertFailsWith<OfflineCashConfigurationSnapshotException> {
            OfflineCashConfigurationSnapshot(
                chainId = "00000042",
                assetDefinitionId = "pkr#sbp",
                offlinePaymentsEnabled = true,
                issuerPublicKeyBase64 = " ",
            ).requireUsableForOfflineExchange(nowMs = 200)
        }
        assertEquals("missing_issuer_public_key", error.code)

        val disabled = assertFailsWith<OfflineCashConfigurationSnapshotException> {
            OfflineCashConfigurationSnapshot(
                chainId = "00000042",
                assetDefinitionId = "pkr#sbp",
                offlinePaymentsEnabled = false,
                issuerPublicKeyBase64 = "issuer-key",
                bridgeAbiVersion = 7,
            ).requireUsableForOfflineExchange(nowMs = 200, requiredBridgeAbiVersion = 7)
        }
        assertEquals("offline_payments_disabled", disabled.code)

        val staleAbi = assertFailsWith<OfflineCashConfigurationSnapshotException> {
            OfflineCashConfigurationSnapshot(
                chainId = "00000042",
                assetDefinitionId = "pkr#sbp",
                offlinePaymentsEnabled = true,
                issuerPublicKeyBase64 = "issuer-key",
                bridgeAbiVersion = 6,
            ).requireUsableForOfflineExchange(nowMs = 200, requiredBridgeAbiVersion = 7)
        }
        assertEquals("unsupported_bridge_abi", staleAbi.code)

        val expired = assertFailsWith<OfflineCashConfigurationSnapshotException> {
            snapshot.requireUsableForOfflineExchange(nowMs = 1_000, requiredBridgeAbiVersion = 7)
        }
        assertEquals("expired", expired.code)
    }

    @Test
    fun `kagemusha wire name constants are canonical`() {
        assertEquals(
            "iroha_data_model::isi::offline::KagemushaTransfer",
            KagemushaWireNames.TRANSFER_INSTRUCTION,
        )
        assertEquals(
            "iroha_data_model::isi::offline::RedeemKagemushaRecursive",
            KagemushaWireNames.REDEEM_RECURSIVE_INSTRUCTION,
        )
        assertEquals(
            "iroha_data_model::offline::model::KagemushaRecursiveSpendRedeemRequestV1",
            KagemushaWireNames.RECURSIVE_REDEEM_REQUEST,
        )
        assertEquals(KagemushaWireNames.TRANSFER_INSTRUCTION, KagemushaInstructionType.TRANSFER.wireName)
    }

    private class RecordingSynchronizer(
        private val events: MutableList<String>,
        private val hasPending: Boolean,
        private val syncFailure: RuntimeException? = null,
    ) : OfflineCashAuditReceiptSynchronizer {
        override suspend fun hasPendingAuditReceipts(): Boolean {
            events.add("hasPending")
            return hasPending
        }

        override suspend fun syncPendingAuditReceipts() {
            events.add("sync")
            syncFailure?.let { throw it }
        }
    }

    private class RecordingWallet(
        private val events: MutableList<String>,
    ) : OfflineCashLifecycleWallet {
        override suspend fun load(assetDefinitionId: String, amount: String): Any {
            events.add("load:$assetDefinitionId:$amount")
            return "ok"
        }

        override fun prepareReceive(assetDefinitionId: String, amount: String): Any? =
            error("not used")

        override fun createPayment(receiveRequest: Any): Any? = error("not used")

        override fun acceptPayment(paymentToken: Any): Any? = error("not used")

        override suspend fun redeem(note: Any, recipient: String?): Any? = error("not used")
    }

    private fun <T> runSuspend(block: suspend () -> T): T {
        var outcome: Result<T>? = null
        block.startCoroutine(
            object : Continuation<T> {
                override val context = EmptyCoroutineContext

                override fun resumeWith(result: Result<T>) {
                    outcome = result
                }
            },
        )
        return outcome!!.getOrThrow()
    }
}
