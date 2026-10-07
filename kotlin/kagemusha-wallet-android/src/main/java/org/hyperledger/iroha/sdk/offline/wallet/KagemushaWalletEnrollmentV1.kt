// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline.wallet

/** Exact retained canonical E1/policy/account originals and original Core timestamps.
 * Native authenticates their installed scope and persists intent before its original fresh
 * generation grant. Neither this DATA carrier nor restore grants key freshness.
 */
class KagemushaWalletEnrollmentOriginalsV1(challenge:ByteArray,policy:ByteArray,account:ByteArray,
    val issuedAtMs:Long,val expiresAtMs:Long) {
    private val retained:List<ByteArray>
    init {
        val values=listOf(challenge,policy,account)
        require(values.zip(listOf(1024,1024,4096)).all{(bytes,bound)->bytes.isNotEmpty() && bytes.size<=bound})
        require(issuedAtMs>0 && expiresAtMs>issuedAtMs && expiresAtMs-issuedAtMs<=600_000)
        retained=values.map{it.copyOf()}
    }
    internal fun frames():List<ByteArray> = retained.map{it.copyOf()}
    override fun toString()="KagemushaWalletEnrollmentOriginalsV1(originals=[REDACTED])"
}

/** Native durable enrollment progress alone grants no monetary admission. */
enum class KagemushaWalletEnrollmentStateV1 { ENROLLED,PENDING,SLOT_ABANDONED }

/** Public authentic evidence DATA. The custody slot remains exclusively inside Native. */
class KagemushaWalletEnrollmentProgressV1 private constructor(reply:KagemushaWalletEnrollmentReplyV1) {
    val state=when(reply.status){
        0->KagemushaWalletEnrollmentStateV1.ENROLLED
        1->KagemushaWalletEnrollmentStateV1.PENDING
        2->KagemushaWalletEnrollmentStateV1.SLOT_ABANDONED
        else->throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
    }
    private val payment=reply.paymentKey()
    private val account=reply.accountSigningOriginal()
    private val binding=reply.playIntegrityRequestHash()
    private val chain=reply.attestationCertificatesDer()
    private val original=reply.bytes()
    fun paymentKey():ByteArray=payment.copyOf()
    /** Exact385 original from selected Native network/account/E1/key and retained Core times. */
    fun accountSigningOriginal():ByteArray=account.copyOf()
    /** Native authentic selected operation/key binding; no app-decoded Integrity verdict. */
    fun playIntegrityRequestHash():ByteArray=binding.copyOf()
    fun attestationCertificatesDer():List<ByteArray> = chain.map{it.copyOf()}
    fun markerOriginal():ByteArray=original.copyOf()
    override fun toString()="KagemushaWalletEnrollmentProgressV1(state=$state,originals=[REDACTED])"
    internal companion object {fun fromNative(reply:KagemushaWalletEnrollmentReplyV1)=KagemushaWalletEnrollmentProgressV1(reply.checked())}
}

/** Literal JNI `(III[B[B[B[[B[B)V`; never reused for ordinary financial completions. */
internal class KagemushaWalletEnrollmentReplyV1(
    @JvmField val status:Int,@JvmField val reason:Int,@JvmField val platformCode:Int,
    paymentKey:ByteArray,accountOriginal:ByteArray,binding:ByteArray,chain:Array<ByteArray>,bytes:ByteArray,
) {
    private val payment:ByteArray
    private val account:ByteArray
    private val requestHash:ByteArray
    private val certificates:List<ByteArray>
    private val original:ByteArray
    init {
        val evidenceEmpty=paymentKey.isEmpty() && accountOriginal.isEmpty() && binding.isEmpty() && chain.isEmpty()
        val valid=when {
            status<0->evidenceEmpty && bytes.isEmpty()
            status==0->reason == -1 && platformCode==0 && paymentKey.size==65 && paymentKey[0]==4.toByte() &&
                accountOriginal.size==385 && binding.size==32 && binding.any{it!=0.toByte()} &&
                chain.size in 2..8 && chain.all{it.size in 1..16_384} && bytes.size in 1..1024
            status in 1..2->reason == -1 && platformCode==0 && evidenceEmpty && bytes.isEmpty()
            status==3->reason == -1 && platformCode==0 && evidenceEmpty && bytes.size in 1..524288
            status==4->reason == -1 && platformCode==0 && evidenceEmpty && bytes.size in 1..1024
            else->false
        }
        if(!valid)invalid()
        payment=paymentKey.copyOf();account=accountOriginal.copyOf();requestHash=binding.copyOf()
        certificates=chain.map{it.copyOf()};original=bytes.copyOf()
    }
    fun checked():KagemushaWalletEnrollmentReplyV1 {
        if(status<0)throw KagemushaWalletExceptionV1(status,reason,platformCode)
        return this
    }
    fun progress():KagemushaWalletEnrollmentProgressV1=KagemushaWalletEnrollmentProgressV1.fromNative(this)
    fun original(expectedStatus:Int):ByteArray{checked();if(status!=expectedStatus)invalid();return bytes()}
    fun paymentKey():ByteArray=payment.copyOf()
    fun accountSigningOriginal():ByteArray=account.copyOf()
    fun playIntegrityRequestHash():ByteArray=requestHash.copyOf()
    fun attestationCertificatesDer():List<ByteArray> = certificates.map{it.copyOf()}
    fun bytes():ByteArray=original.copyOf()
    private fun invalid():Nothing=throw KagemushaWalletExceptionV1(KagemushaWalletExceptionV1.INVALID_NATIVE_OUTPUT)
    override fun toString()="KagemushaWalletEnrollmentReplyV1(status=$status,originals=[REDACTED])"
}

/** Finite original-only request. No slot or freshness flag crosses this managed ABI. */
internal class KagemushaWalletEnrollmentInputV1(val selector:Int,
    originals:KagemushaWalletEnrollmentOriginalsV1,original:ByteArray=byteArrayOf(),certificates:ByteArray=byteArrayOf()) {
    val issuedAtMs=originals.issuedAtMs;val expiresAtMs=originals.expiresAtMs
    private val retained:List<ByteArray>
    init {
        require(selector in 0..3)
        require(when(selector){0,1->original.isEmpty();2->original.size in 1..524288;else->original.size in 1..1024})
        require(if(selector==3)certificates.size in 1..10000 else certificates.isEmpty())
        retained=originals.frames()+listOf(original.copyOf(),certificates.copyOf())
    }
    fun frames():List<ByteArray> = retained.map{it.copyOf()}
}

internal object KagemushaWalletEnrollmentNativeV1 {
    @JvmStatic external fun enroll(runtime:Long,selector:Int,challenge:ByteArray,policy:ByteArray,account:ByteArray,
        original:ByteArray,certificates:ByteArray,issuedAtMs:Long,expiresAtMs:Long):KagemushaWalletEnrollmentReplyV1?
}

/** Managed read-only evidence fencing. Reconciliation callbacks retain actual Native authority. */
internal fun readEnrollmentAttestationChainV1(
    resume:()->KagemushaWalletEnrollmentProgressV1,
    read:(ByteArray)->KagemushaWalletAndroidAttestationChainV1,
):List<ByteArray> {
    fun unavailable(reason:KagemushaWalletAndroidUnavailableV1):Nothing =
        throw KagemushaWalletExceptionV1(-5,reason.kind.tag,reason.code)
    fun enrolled(progress:KagemushaWalletEnrollmentProgressV1) {
        if(progress.state!=KagemushaWalletEnrollmentStateV1.ENROLLED)unavailable(KagemushaWalletAndroidUnavailableV1.BUSY)
    }
    val before=resume().also(::enrolled)
    val chain=read(before.paymentKey())
    val after=resume().also(::enrolled)
    if(!after.paymentKey().contentEquals(before.paymentKey()) ||
        !after.markerOriginal().contentEquals(before.markerOriginal()) ||
        !after.accountSigningOriginal().contentEquals(before.accountSigningOriginal()) ||
        !after.playIntegrityRequestHash().contentEquals(before.playIntegrityRequestHash()))unavailable(KagemushaWalletAndroidUnavailableV1.BUSY)
    val originals=when(chain) {
        is KagemushaWalletAndroidAttestationChainV1.Present->chain.certificatesDer()
        KagemushaWalletAndroidAttestationChainV1.Absent->unavailable(KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_KEY_ABSENT))
        is KagemushaWalletAndroidAttestationChainV1.Unavailable->unavailable(chain.reason)
    }
    if(originals.size !in 2..8 || originals.any{it.size !in 1..16384})unavailable(
        KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_CERTIFICATE))
    return originals.map{it.copyOf()}
}
