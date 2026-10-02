// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import android.content.Context
import java.io.File
import java.util.ServiceLoader
import java.util.concurrent.CompletableFuture
import java.util.concurrent.ForkJoinPool

/** Evidence setup is enabled independently of a monetary runtime catalog. */
sealed interface KagemushaFirstDeviceHardwareEvidenceSelectionV1 {
    class Available internal constructor(val service: KagemushaFirstDeviceHardwareEvidenceServiceV1) : KagemushaFirstDeviceHardwareEvidenceSelectionV1
    class Declined internal constructor(val reason: Reason) : KagemushaFirstDeviceHardwareEvidenceSelectionV1
    enum class Reason { NATIVE_NOT_PACKAGED, AUTHENTIC_BOOTSTRAP_NOT_INSTALLED }
}
/** Distinct shared app-owned hardware route; never discovers an OEM or monetary provider. */
interface KagemushaFirstDeviceHardwareEvidenceServiceFactoryV1 {
    /** Blocking native startup; call only on an owned worker. */
    fun open(context: Context): KagemushaFirstDeviceHardwareEvidenceSelectionV1
    /** Shipping UI uses this asynchronous entry, retaining one original startup across views. */
    fun openOriginalAsync(context: Context): CompletableFuture<KagemushaFirstDeviceHardwareEvidenceSelectionV1> =
        CompletableFuture.supplyAsync { open(context.applicationContext) }
    companion object {
        @JvmStatic fun discover(classLoader: ClassLoader): KagemushaFirstDeviceHardwareEvidenceServiceFactoryV1 =
            select(ServiceLoader.load(KagemushaFirstDeviceHardwareEvidenceServiceFactoryV1::class.java,classLoader).iterator())
        internal fun select(providers: Iterator<KagemushaFirstDeviceHardwareEvidenceServiceFactoryV1>): KagemushaFirstDeviceHardwareEvidenceServiceFactoryV1 {
            check(providers.hasNext()) { "Shared first-device hardware service is not packaged" }
            val original=providers.next()
            check(!providers.hasNext() && original.javaClass == KagemushaAndroidFirstDeviceHardwareEvidenceServiceFactoryV1::class.java) {
                "Shared first-device hardware service was substituted"
            }
            return original
        }
    }
}
/** The shipping default route exists for all nineteen required Android targets. Actual key
 * eligibility/admission remains OS attestation + independently verified PI under Native policy.
 * There is no runtime-catalog toggle, software key, applet capability or device provisioning gate.
 */
class KagemushaAndroidFirstDeviceHardwareEvidenceServiceFactoryV1 : KagemushaFirstDeviceHardwareEvidenceServiceFactoryV1 {
    override fun open(context: Context): KagemushaFirstDeviceHardwareEvidenceSelectionV1 =
        InstalledOwner.open(context.applicationContext)
    override fun openOriginalAsync(context: Context): CompletableFuture<KagemushaFirstDeviceHardwareEvidenceSelectionV1> =
        InstalledOwner.openOriginalAsync(context.applicationContext)
    private object InstalledOwner {
        private val startup = KagemushaFirstDeviceHardwareOriginalStartupV1<KagemushaFirstDeviceHardwareEvidenceSelectionV1>(ForkJoinPool.commonPool())
        @Synchronized fun openOriginalAsync(context: Context): CompletableFuture<KagemushaFirstDeviceHardwareEvidenceSelectionV1> {
            val storage=File(context.noBackupFilesDir,"kagemusha-first-device-hardware-v1").absolutePath
            originalRoot?.let { check(it==storage) { "Installed hardware storage owner changed" } }
            originalRoot=storage
            return startup.openOriginal { open(context) }
        }
        private var originalRoot: String? = null
        private var original: KagemushaFirstDeviceHardwareEvidenceServiceV1? = null
        @Synchronized fun open(context: Context): KagemushaFirstDeviceHardwareEvidenceSelectionV1 {
            val storage=File(context.noBackupFilesDir,"kagemusha-first-device-hardware-v1").absolutePath
            original?.let {
                check(storage == originalRoot) { "Installed hardware storage owner changed" }
                return KagemushaFirstDeviceHardwareEvidenceSelectionV1.Available(it)
            }
            check(android.os.Looper.myLooper() != android.os.Looper.getMainLooper()) {
                "Native hardware startup requires its original worker"
            }
            val native=try { KagemushaFirstDeviceHardwareEvidenceNativeV1.openInstalled(context,storage) }
                catch (_: UnsatisfiedLinkError) { return KagemushaFirstDeviceHardwareEvidenceSelectionV1.Declined(
                    KagemushaFirstDeviceHardwareEvidenceSelectionV1.Reason.NATIVE_NOT_PACKAGED) }
            if(native==null) return KagemushaFirstDeviceHardwareEvidenceSelectionV1.Declined(
                KagemushaFirstDeviceHardwareEvidenceSelectionV1.Reason.AUTHENTIC_BOOTSTRAP_NOT_INSTALLED)
            val owner=KagemushaFirstDeviceHardwareEvidenceServiceOwnerV1 { native }
            // Authenticate its exact original identity before publishing the service to the app.
            owner.recoverOriginalOrReserve()
            originalRoot=storage;original=owner
            return KagemushaFirstDeviceHardwareEvidenceSelectionV1.Available(owner)
        }
    }
}
