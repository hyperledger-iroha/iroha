package org.hyperledger.iroha.sdk.offline

import kotlin.coroutines.Continuation
import kotlin.coroutines.EmptyCoroutineContext
import kotlin.coroutines.startCoroutine
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

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
    fun `offline cash lifecycle accepts SDK offline note wallet`() {
        val events = ArrayList<String>()
        val offlineWallet = OfflineNoteWallet(
            chainId = "00000042",
            accountId = "merchant",
            attestationProvider = UnusedAttestationProvider,
            proofProvider = UnusedProofProvider,
            proofVerifier = UnusedProofVerifier,
        )
        val controller = OfflineCashLifecycleController(
            wallet = offlineWallet,
            auditReceiptSynchronizer = RecordingSynchronizer(events, hasPending = true),
        )

        val error = assertFailsWith<IllegalStateException> {
            runSuspend { controller.load("pkr#sbp", "10") }
        }

        assertEquals("Offline Note issuer client is required for load", error.message)
        assertEquals(listOf("hasPending", "sync"), events)
    }

    @Test
    fun `configuration snapshot requires cached identity time issuer key and ABI`() {
        val issuerPublicKeyBase64 = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8"
        val issuerPublicKeyBase64Url = "__________________________________________8"
        val shortIssuerPublicKeyBase64 = "q6urq6urq6urq6urq6urq6urq6urq6urq6urq6urqw"
        val longIssuerPublicKeyBase64 = "zc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3Nzc3N"
        val snapshot = OfflineCashConfigurationSnapshot(
            chainId = "00000042",
            assetDefinitionId = "pkr#sbp",
            offlinePaymentsEnabled = true,
            issuerPublicKeyBase64 = issuerPublicKeyBase64,
            nativeBridgeAbiVersion = 7,
            artifactSetId = "artifact-set",
            circuitId = "kagemusha-recursive-compact-v1",
            createdAtMs = 100,
            expiresAtMs = 1_000,
        )
        snapshot.requireUsableForOfflineExchange(nowMs = 999, requiredNativeBridgeAbiVersion = 7)
        OfflineCashConfigurationSnapshot(
            chainId = "00000042",
            assetDefinitionId = "pkr#sbp",
            offlinePaymentsEnabled = true,
            issuerPublicKeyBase64 = issuerPublicKeyBase64Url,
            nativeBridgeAbiVersion = 7,
            artifactSetId = "artifact-set",
            circuitId = "kagemusha-recursive-compact-v1",
            createdAtMs = 100,
            expiresAtMs = 1_000,
        ).requireUsableForOfflineExchange(nowMs = 999, requiredNativeBridgeAbiVersion = 7)

        val malformedIdentitySnapshots = listOf(
            "chainId" to OfflineCashConfigurationSnapshot(
                chainId = "",
                assetDefinitionId = "pkr#sbp",
                offlinePaymentsEnabled = true,
                issuerPublicKeyBase64 = issuerPublicKeyBase64,
                nativeBridgeAbiVersion = 7,
            ),
            "chainId" to OfflineCashConfigurationSnapshot(
                chainId = "00000042\n",
                assetDefinitionId = "pkr#sbp",
                offlinePaymentsEnabled = true,
                issuerPublicKeyBase64 = issuerPublicKeyBase64,
                nativeBridgeAbiVersion = 7,
            ),
            "assetDefinitionId" to OfflineCashConfigurationSnapshot(
                chainId = "00000042",
                assetDefinitionId = "pkr sbp",
                offlinePaymentsEnabled = true,
                issuerPublicKeyBase64 = issuerPublicKeyBase64,
                nativeBridgeAbiVersion = 7,
            ),
            "artifactSetId" to OfflineCashConfigurationSnapshot(
                chainId = "00000042",
                assetDefinitionId = "pkr#sbp",
                offlinePaymentsEnabled = true,
                issuerPublicKeyBase64 = issuerPublicKeyBase64,
                nativeBridgeAbiVersion = 7,
                artifactSetId = "artifact set",
            ),
            "circuitId" to OfflineCashConfigurationSnapshot(
                chainId = "00000042",
                assetDefinitionId = "pkr#sbp",
                offlinePaymentsEnabled = true,
                issuerPublicKeyBase64 = issuerPublicKeyBase64,
                nativeBridgeAbiVersion = 7,
                circuitId = "kagemusha-recursive-compact-v1\n",
            ),
        )
        for ((fieldName, malformedSnapshot) in malformedIdentitySnapshots) {
            val malformedIdentity = assertFailsWith<OfflineCashConfigurationSnapshotException> {
                malformedSnapshot.requireUsableForOfflineExchange(
                    nowMs = 200,
                    requiredNativeBridgeAbiVersion = 7,
                )
            }
            assertEquals("malformed_snapshot", malformedIdentity.code)
            assertTrue(malformedIdentity.message?.contains(fieldName) == true)
        }

        val malformedTimeSnapshots = listOf(
            "createdAtMs" to OfflineCashConfigurationSnapshot(
                chainId = "00000042",
                assetDefinitionId = "pkr#sbp",
                offlinePaymentsEnabled = true,
                issuerPublicKeyBase64 = issuerPublicKeyBase64,
                nativeBridgeAbiVersion = 7,
                createdAtMs = -1,
            ),
            "expiresAtMs" to OfflineCashConfigurationSnapshot(
                chainId = "00000042",
                assetDefinitionId = "pkr#sbp",
                offlinePaymentsEnabled = true,
                issuerPublicKeyBase64 = issuerPublicKeyBase64,
                nativeBridgeAbiVersion = 7,
                expiresAtMs = -1,
            ),
            "expiresAtMs" to OfflineCashConfigurationSnapshot(
                chainId = "00000042",
                assetDefinitionId = "pkr#sbp",
                offlinePaymentsEnabled = true,
                issuerPublicKeyBase64 = issuerPublicKeyBase64,
                nativeBridgeAbiVersion = 7,
                createdAtMs = 100,
                expiresAtMs = 100,
            ),
        )
        for ((fieldName, malformedSnapshot) in malformedTimeSnapshots) {
            val malformedTime = assertFailsWith<OfflineCashConfigurationSnapshotException> {
                malformedSnapshot.requireUsableForOfflineExchange(
                    nowMs = 200,
                    requiredNativeBridgeAbiVersion = 7,
                )
            }
            assertEquals("malformed_snapshot", malformedTime.code)
            assertTrue(malformedTime.message?.contains(fieldName) == true)
        }

        val malformedNow = assertFailsWith<OfflineCashConfigurationSnapshotException> {
            snapshot.requireUsableForOfflineExchange(nowMs = -1, requiredNativeBridgeAbiVersion = 7)
        }
        assertEquals("malformed_snapshot", malformedNow.code)
        assertTrue(malformedNow.message?.contains("nowMs") == true)

        val error = assertFailsWith<OfflineCashConfigurationSnapshotException> {
            OfflineCashConfigurationSnapshot(
                chainId = "00000042",
                assetDefinitionId = "pkr#sbp",
                offlinePaymentsEnabled = true,
                issuerPublicKeyBase64 = " ",
            ).requireUsableForOfflineExchange(nowMs = 200)
        }
        assertEquals("missing_issuer_public_key", error.code)

        for (issuerKey in listOf(
            "",
            " $issuerPublicKeyBase64",
            "$issuerPublicKeyBase64 ",
            "not base64",
            "!!!!",
            "$issuerPublicKeyBase64=",
            shortIssuerPublicKeyBase64,
            longIssuerPublicKeyBase64,
            "issuer-key\n",
            "issuer-key\u2603",
        )) {
            val noncanonical = assertFailsWith<OfflineCashConfigurationSnapshotException> {
                OfflineCashConfigurationSnapshot(
                    chainId = "00000042",
                    assetDefinitionId = "pkr#sbp",
                    offlinePaymentsEnabled = true,
                    issuerPublicKeyBase64 = issuerKey,
                    nativeBridgeAbiVersion = 7,
                ).requireUsableForOfflineExchange(nowMs = 200, requiredNativeBridgeAbiVersion = 7)
            }
            assertEquals("missing_issuer_public_key", noncanonical.code)
        }

        val disabled = assertFailsWith<OfflineCashConfigurationSnapshotException> {
            OfflineCashConfigurationSnapshot(
                chainId = "00000042",
                assetDefinitionId = "pkr#sbp",
                offlinePaymentsEnabled = false,
                issuerPublicKeyBase64 = issuerPublicKeyBase64,
                nativeBridgeAbiVersion = 7,
            ).requireUsableForOfflineExchange(nowMs = 200, requiredNativeBridgeAbiVersion = 7)
        }
        assertEquals("offline_payments_disabled", disabled.code)

        val staleAbi = assertFailsWith<OfflineCashConfigurationSnapshotException> {
            OfflineCashConfigurationSnapshot(
                chainId = "00000042",
                assetDefinitionId = "pkr#sbp",
                offlinePaymentsEnabled = true,
                issuerPublicKeyBase64 = issuerPublicKeyBase64,
                nativeBridgeAbiVersion = 6,
            ).requireUsableForOfflineExchange(nowMs = 200, requiredNativeBridgeAbiVersion = 7)
        }
        assertEquals("unsupported_native_bridge_abi", staleAbi.code)

        val malformedNativeAbi = assertFailsWith<OfflineCashConfigurationSnapshotException> {
            OfflineCashConfigurationSnapshot(
                chainId = "00000042",
                assetDefinitionId = "pkr#sbp",
                offlinePaymentsEnabled = true,
                issuerPublicKeyBase64 = issuerPublicKeyBase64,
                nativeBridgeAbiVersion = 0,
            ).requireUsableForOfflineExchange(nowMs = 200, requiredNativeBridgeAbiVersion = 7)
        }
        assertEquals("malformed_snapshot", malformedNativeAbi.code)

        val malformedRequiredAbi = assertFailsWith<OfflineCashConfigurationSnapshotException> {
            snapshot.requireUsableForOfflineExchange(nowMs = 999, requiredNativeBridgeAbiVersion = 0)
        }
        assertEquals("malformed_snapshot", malformedRequiredAbi.code)

        val expired = assertFailsWith<OfflineCashConfigurationSnapshotException> {
            snapshot.requireUsableForOfflineExchange(nowMs = 1_000, requiredNativeBridgeAbiVersion = 7)
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

    private object UnusedAttestationProvider : OfflineNoteAttestationProvider {
        override fun currentKeyCertificate(): OfflineNote.KeyCertificate = error("not used")
    }

    private object UnusedProofProvider : OfflineNoteProofProvider {
        override fun proveAudit(audit: OfflineNote.AuditBundle): OfflineNote.RecursiveProof =
            error("not used")

        override fun proveRedeem(redemption: OfflineNote.Redeem): OfflineNote.RecursiveProof =
            error("not used")
    }

    private object UnusedProofVerifier : OfflineNoteProofVerifier {
        override fun verifyAudit(audit: OfflineNote.AuditBundle): Boolean = error("not used")

        override fun verifyRedeem(redemption: OfflineNote.Redeem): Boolean = error("not used")
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
