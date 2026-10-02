// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidAppKeyHardwarePolicyV1

/** Dedicated Native view; eighteen hardware-only methods, no financial coordinator fallback. */
internal class KagemushaFirstDeviceHardwareEvidenceNativeV1 private constructor(private val handle: Long) :
    KagemushaHardwareBootstrapNativeEndpointV1 {
    private val original = status()
    private val operation = original[0].copyOf()
    private fun invoke(method: Int, vararg fields: ByteArray): Array<ByteArray> {
        require(method in 1..18 && fields.size <= 7 && fields.all { it.size <= 192 * 1024 })
        val result = checkNotNull(KagemushaFirstDeviceHardwareEvidenceJniV1.invoke(handle,method,fields.map { it.copyOf() }.toTypedArray())) {
            "Original Native hardware-evidence operation refused; recover its retained original"
        }
        check(result.size <= 7 && result.all { it.size <= 192 * 1024 })
        return result.map { it.copyOf() }.toTypedArray()
    }
    private fun status(): Array<ByteArray> = invoke(1).also { fields ->
        check(fields.size == 7 && fields[0].size == 32 && fields[0].any { it != 0.toByte() }
            && fields[1].size == 8 && fields[2].size == 1 && fields[2][0].toInt() in 0..6
            && fields[3].size == 1 && fields[3][0].toInt() in 0..6)
        KagemushaHardwareBootstrapHttpCodecV1.requireOrigin(fields[4].toString(Charsets.UTF_8))
        check(fields[5].isNotEmpty() && fields[5].size <= 512 && fields[5].all { it.toInt() in 33..126 })
        check(fields[6].toString(Charsets.US_ASCII) in setOf("accounts.google.com","https://accounts.google.com"))
    }
    override fun requireOriginalCustody() { check(MessageDigest.isEqual(operation,status()[0])) }
    override fun requirePendingEffect(step: Int) { requireOriginalCustody(); invoke(2,byteArrayOf(step.toByte())).also { check(it.isEmpty()) }; requireOriginalCustody() }
    override fun originalOperationId(): ByteArray = status()[0].copyOf()
    override fun authoritativeDeadlineMs(): ULong = ByteBuffer.wrap(status()[1]).order(ByteOrder.LITTLE_ENDIAN).long.toULong()
    override fun googleOAuthClientId(): String = status()[5].toString(Charsets.US_ASCII)
    override fun completedStep(): Int = status()[2][0].toInt()
    override fun pendingStep(): Int? = status()[3][0].toInt().takeIf { it != 0 }
    private fun acknowledged(method: Int, vararg fields: ByteArray) { requireOriginalCustody(); check(invoke(method,*fields).isEmpty()); requireOriginalCustody() }
    private fun http(stage: KagemushaHardwareBootstrapHttpOriginalV1.Stage, body: ByteArray): KagemushaHardwareBootstrapHttpOriginalV1 {
        requireOriginalCustody()
        return KagemushaHardwareBootstrapHttpOriginalV1(stage,original[4].toString(Charsets.UTF_8),body,::requireOriginalCustody,operation,
            { requirePendingEffect(when(stage) {
                KagemushaHardwareBootstrapHttpOriginalV1.Stage.PREPARE->1
                KagemushaHardwareBootstrapHttpOriginalV1.Stage.RAW_ATTESTATION->3
                KagemushaHardwareBootstrapHttpOriginalV1.Stage.RECEIPT->6
            }) })
    }
    override fun fencePrepare(originalGoogleIdToken: ByteArray): KagemushaHardwareBootstrapHttpOriginalV1 {
        requireOriginalCustody(); val fields=invoke(3,originalGoogleIdToken); check(fields.size==1)
        return http(KagemushaHardwareBootstrapHttpOriginalV1.Stage.PREPARE,
            KagemushaHardwareBootstrapHttpCodecV1.prepare(fields[0],originalGoogleIdToken))
    }
    override fun acceptChallenge(original: ByteArray) = acknowledged(4,original)
    private fun key(fields: Array<ByteArray>): HardwareKeySelection {
        check(fields.size==3 && fields[2].size==1)
        val policy=when(fields[2][0].toInt()) { 1->KagemushaAndroidAppKeyHardwarePolicyV1.TEE_ONLY
            2->KagemushaAndroidAppKeyHardwarePolicyV1.STRONGBOX_ONLY
            3->KagemushaAndroidAppKeyHardwarePolicyV1.TEE_OR_STRONGBOX
            else->error("Native hardware policy differs") }
        return HardwareKeySelection(fields[0].toString(Charsets.US_ASCII),fields[1],policy)
    }
    override fun fenceKey(): HardwareKeySelection { requireOriginalCustody(); return key(invoke(5)).also { requireOriginalCustody() } }
    override fun recoverKeySelection(): HardwareKeySelection { requireOriginalCustody(); return key(invoke(6)).also { requireOriginalCustody() } }
    override fun captureKey(point: ByteArray, rawArchive: ByteArray) = acknowledged(7,point,rawArchive)
    override fun fenceRawIssuer(): KagemushaHardwareBootstrapHttpOriginalV1 {
        requireOriginalCustody(); return http(KagemushaHardwareBootstrapHttpOriginalV1.Stage.RAW_ATTESTATION,
            KagemushaHardwareBootstrapHttpCodecV1.raw(invoke(8)))
    }
    override fun acceptRawAdmission(original: ByteArray) = acknowledged(9,original)
    override fun fencePossession(): HardwarePossessionSelection {
        requireOriginalCustody(); val f=invoke(10); check(f.size==6)
        return HardwarePossessionSelection(key(f.copyOfRange(0,3)),f[3],f[4],f[5]).also { requireOriginalCustody() }
    }
    override fun capturePossession(originalDer: ByteArray) = acknowledged(11,originalDer)
    override fun fenceIntegrity(): HardwareIntegritySelection {
        requireOriginalCustody(); val f=invoke(12); check(f.size==2 && f[0].size==8)
        return HardwareIntegritySelection(ByteBuffer.wrap(f[0]).order(ByteOrder.LITTLE_ENDIAN).long,f[1]).also { requireOriginalCustody() }
    }
    override fun captureIntegrityOriginal(opaqueOriginal: ByteArray) = acknowledged(13,opaqueOriginal)
    override fun fenceReceipt(): KagemushaHardwareBootstrapHttpOriginalV1 {
        requireOriginalCustody(); return http(KagemushaHardwareBootstrapHttpOriginalV1.Stage.RECEIPT,
            KagemushaHardwareBootstrapHttpCodecV1.finish(invoke(14)))
    }
    override fun acceptHardwareReceipt(original: ByteArray) = acknowledged(15,original)
    override fun originalReceipt(): ByteArray? { requireOriginalCustody(); val f=invoke(16); check(f.size<=1); return f.firstOrNull()?.copyOf() }
    override fun requestCancel() = acknowledged(17)
    override fun disposeTerminal() = acknowledged(18)
    companion object {
        fun openInstalled(context: android.content.Context, storage: String): KagemushaFirstDeviceHardwareEvidenceNativeV1? {
            check(KagemushaFirstDeviceHardwareEvidenceJniV1.contract().contentEquals(intArrayOf(1,1,18,7,192*1024))) {
                "Dedicated Native hardware-evidence contract differs"
            }
            val handle=KagemushaFirstDeviceHardwareEvidenceJniV1.open(context,storage)
            if(handle==0L) return null
            return try { KagemushaFirstDeviceHardwareEvidenceNativeV1(handle) }
            catch(failure: Throwable) { KagemushaFirstDeviceHardwareEvidenceJniV1.close(handle); throw failure }
        }
    }
}
/** Exact final packaged JNI class. There is no managed authority installer. */
internal object KagemushaFirstDeviceHardwareEvidenceJniV1 {
    private val loaded: Unit by lazy { System.loadLibrary("connect_norito_bridge") }
    fun contract(): IntArray { loaded; return checkNotNull(nativeContractV1()) }
    fun open(context: android.content.Context, storage: String): Long { loaded; return nativeOpenV1(context,storage) }
    fun invoke(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray>? { loaded; return nativeInvokeV1(handle,method,fields) }
    fun close(handle: Long): Int { loaded; return nativeCloseV1(handle) }
    @JvmStatic private external fun nativeContractV1(): IntArray?
    @JvmStatic private external fun nativeOpenV1(context: android.content.Context, storage: String): Long
    @JvmStatic private external fun nativeInvokeV1(handle: Long, method: Int, fields: Array<ByteArray>): Array<ByteArray>?
    @JvmStatic private external fun nativeCloseV1(handle: Long): Int
}
