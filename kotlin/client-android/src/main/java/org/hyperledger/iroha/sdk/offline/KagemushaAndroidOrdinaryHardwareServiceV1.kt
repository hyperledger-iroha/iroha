// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import android.content.Context
import java.util.ServiceLoader

/** Distinct ordinary app-key service bound to the installed Native W and activated member S.
 * Retail signatures come only from the retained Native account/session custody.
 * Results are detached originals: State/Guard publication and monetary ownership stay in Native.
 * This service never implements the applet/OEM hardware-provider interface.
 */
class KagemushaAndroidOrdinaryHardwareServiceV1 private constructor(
    private val workflow: Lazy<KagemushaAndroidOrdinaryEnrollmentV1>,
    private val requireOriginalOwner: () -> Unit,
) {
    suspend fun beginOrResumeEnrollment(): KagemushaOrdinaryEnrollmentOriginalsV1 {
        requireOriginalOwner()
        return workflow.value.beginOrResume().also { requireOriginalOwner() }
    }
    suspend fun beginOrResumeBootstrapApproval(): KagemushaOrdinaryBootstrapApprovalOriginalsV1 {
        requireOriginalOwner()
        return workflow.value.beginOrResumeBootstrapApproval().also { requireOriginalOwner() }
    }
    /** Actual Native publication through the same retained FI/W owner; acknowledgement only. */
    suspend fun beginOrResumeInitialStatePublication(): KagemushaOrdinaryInitialStatePublicationOriginalsV1 {
        requireOriginalOwner()
        return workflow.value.beginOrResumeInitialStatePublication().also { requireOriginalOwner() }
    }
    /** Retire Bootstrap after genuine publication before the same-owner wallet cash dispatch.
     * This only fences local lifecycle use; Native cash and current FI authentication remain mandatory.
     */
    suspend fun prepareCurrentFinancialControlHandoff() {
        requireOriginalOwner()
        workflow.value.prepareCurrentFinancialControlHandoff()
        requireOriginalOwner()
    }
    internal companion object {
        fun open(context: Context, coordinator: KagemushaNativeCoreCoordinatorAdapterV1,
            activatedSignatoryAccountId: String, transport: KagemushaOrdinaryIdentityOriginalTransportV1,
            selection: KagemushaNativeWalletAccountSelectionOriginalV1,
            requireOriginalOwner: () -> Unit): KagemushaAndroidOrdinaryHardwareServiceV1 {
            val selected = requireOriginalNativeAccount(coordinator, activatedSignatoryAccountId, selection, requireOriginalOwner)
            fun current() {
                requireOriginalOwner()
                selected.requireCurrent()
                requireOriginalOwner()
            }
            // Retain one original workflow through explicit retries. Native and the shared
            // platform adapters own alias/key attestation, PI, invocation fences and WAL.
            val workflow = lazy(LazyThreadSafetyMode.SYNCHRONIZED) {
                current()
                KagemushaAndroidOrdinaryEnrollmentV1(context, coordinator, transport, selected,
                    ::current).also { current() }
            }
            return KagemushaAndroidOrdinaryHardwareServiceV1(workflow, ::current)
        }
        /** Bind activated online S to actual Native S, then correlate the reservation with W.
         * The retained selection is finite session data, never an account-custody constructor.
         * Missing actual Native custody refuses before hardware collection or managed signing.
         */
        fun requireOriginalNativeAccount(coordinator: KagemushaNativeCoreCoordinatorAdapterV1,
            activatedSignatoryAccountId: String, selected: KagemushaNativeWalletAccountSelectionOriginalV1,
            requireOriginalOwner: () -> Unit):
            KagemushaNativeWalletAccountSelectionOriginalV1 {
            require(activatedSignatoryAccountId.isNotBlank()) { "The original activated signatory is required" }
            requireOriginalOwner()
            val native = coordinator.appIdentityOperations()
            native.requireCurrentWalletAccountSelection(selected)
            requireOriginalOwner()
            check(selected.signatoryAccountId() == activatedSignatoryAccountId) {
                "The installed ordinary Native owner controls another activated signatory"
            }
            requireOriginalOwner()
            val reserved = native.reserveOriginalIdentity()
            requireOriginalOwner()
            check(selected.walletAccountId() == reserved.accountId()) {
                "The original reservation differs from the installed Native wallet W"
            }
            selected.requireCurrent()
            requireOriginalOwner()
            return selected
        }
    }
}

/** Installed shared route, independent of historical applet capability discovery. */
interface KagemushaAndroidOrdinaryHardwareServiceFactoryV1 {
    fun open(context: Context, coordinator: KagemushaNativeCoreCoordinatorAdapterV1,
        activatedSignatoryAccountId: String, transport: KagemushaOrdinaryIdentityOriginalTransportV1,
        selection: KagemushaNativeWalletAccountSelectionOriginalV1,
        requireOriginalOwner: () -> Unit): KagemushaAndroidOrdinaryHardwareServiceV1

    companion object {
        @JvmStatic fun discover(classLoader: ClassLoader): KagemushaAndroidOrdinaryHardwareServiceFactoryV1 =
            selectOriginalFactory(ServiceLoader.load(KagemushaAndroidOrdinaryHardwareServiceFactoryV1::class.java,
                classLoader).iterator())

        internal fun selectOriginalFactory(providers: Iterator<KagemushaAndroidOrdinaryHardwareServiceFactoryV1>):
            KagemushaAndroidOrdinaryHardwareServiceFactoryV1 {
            check(providers.hasNext()) { "The ordinary shared hardware app-key service is not packaged" }
            val original = providers.next()
            check(!providers.hasNext()) { "Ambiguous ordinary hardware app-key service routes" }
            check(original.javaClass == KagemushaAndroidAppOwnedHardwareServiceFactoryV1::class.java) {
                "The original shared hardware app-key service was substituted"
            }
            return original
        }
    }
}

/** Uses the existing native C21/E20/FI and distinct C19 Bootstrap phase8 workflow.
 * Hardware policy is selected by Native; StrongBox preference and governed TEE admission
 * stay in KagemushaAndroidHardwareAppKeyStoreV1. No caller policy or software key route exists.
 */
class KagemushaAndroidAppOwnedHardwareServiceFactoryV1 : KagemushaAndroidOrdinaryHardwareServiceFactoryV1 {
    override fun open(context: Context, coordinator: KagemushaNativeCoreCoordinatorAdapterV1,
        activatedSignatoryAccountId: String, transport: KagemushaOrdinaryIdentityOriginalTransportV1,
        selection: KagemushaNativeWalletAccountSelectionOriginalV1,
        requireOriginalOwner: () -> Unit): KagemushaAndroidOrdinaryHardwareServiceV1 =
        KagemushaAndroidOrdinaryHardwareServiceV1.open(context, coordinator, activatedSignatoryAccountId,
            transport, selection, requireOriginalOwner)
}
