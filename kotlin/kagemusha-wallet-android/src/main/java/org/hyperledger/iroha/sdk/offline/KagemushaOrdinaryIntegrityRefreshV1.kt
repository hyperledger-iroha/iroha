// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline
import android.content.Context
import java.util.concurrent.CompletableFuture
import java.util.concurrent.atomic.AtomicBoolean
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlin.coroutines.suspendCoroutine
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidHardwareAppKeyStoreV1
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidPlayIntegrityProviderV1
/** Protected enrolled HTTP only; Native retains and independently authenticates the originals. */
fun interface KagemushaOrdinaryIntegrityOriginalTransportV1 {
 suspend fun exchange(original:KagemushaOrdinaryIntegrityHttpOriginalV1):ByteArray
}
class KagemushaOrdinaryIntegrityHttpOriginalV1 private constructor(
 kind:String,body:ByteArray,private val guard:()->Unit) {
 val path="/v1/kagemusha/enrollment/ordinary/integrity/"+kind
 val maximumResponseBytes=KagemushaOrdinaryIntegrityHttpCodecV1.MAXIMUM_RESPONSE_BYTES
 private val original=body.copyOf()
 val requestId=KagemushaOrdinaryIntegrityHttpCodecV1.requestId(original)
 init{require(kind=="prepare"||kind=="finish")}
 fun requireCurrent()=guard()
 fun body():ByteArray {requireCurrent();return original.copyOf().also{requireCurrent()}}
 internal fun clear()=original.fill(0)
 internal companion object {
  fun prepare(enrollment:ByteArray,nonce:ByteArray,guard:()->Unit)=KagemushaOrdinaryIntegrityHttpOriginalV1(
   "prepare",KagemushaOrdinaryIntegrityHttpCodecV1.prepareBody(enrollment,nonce),guard)
  fun finish(fields:List<ByteArray>,guard:()->Unit)=KagemushaOrdinaryIntegrityHttpOriginalV1(
   "finish",KagemushaOrdinaryIntegrityHttpCodecV1.finishBody(fields),guard)
 }
}
/** A Native-admitted lease digest is historical evidence, never a monetary readiness flag. */
class KagemushaOrdinaryIntegrityAcknowledgementV1 internal constructor(digest:ByteArray) {
 private val original=digest.copyOf();fun leaseDigest():ByteArray=original.copyOf()
}
internal interface OrdinaryIntegrityPreparedStepV1 {
 fun original():ByteArray;fun requestHash():ByteArray;suspend fun approve():ByteArray
 fun requestToken():CompletableFuture<String>
}
internal interface OrdinaryIntegrityWorkflowNativeV1 {
 suspend fun refreshAccount();fun requireOpen();fun revoke()
 suspend fun invoke(phase:Int,original:ByteArray=ByteArray(0)):List<ByteArray>
 suspend fun acceptChallenge(raw:ByteArray):OrdinaryIntegrityPreparedStepV1
 suspend fun recoverPrepared():OrdinaryIntegrityPreparedStepV1
}
/** Keep this workflow across UI recreation/cancellation. Google and OS invocation futures and
 * full Native pending originals are retained, while HTTP uncertainty reuses only the same body/ID.
 * No timer renews a challenge or creates a replacement nonce; abandonment is an explicit call.
 */
class KagemushaOrdinaryIntegrityRefreshV1 internal constructor(
 private val native:OrdinaryIntegrityWorkflowNativeV1,
 private val transport:KagemushaOrdinaryIntegrityOriginalTransportV1,
 private val requireOriginalOwner:()->Unit) {
 constructor(context:Context,coordinator:KagemushaNativeCoreCoordinatorAdapterV1,
  transport:KagemushaOrdinaryIntegrityOriginalTransportV1,requireOriginalOwner:()->Unit):this(
   ActualIntegrityNativeV1(coordinator.ordinaryIntegrityRefreshTransportBinding(requireOriginalOwner),
    KagemushaAndroidHardwareAppKeyStoreV1(context.applicationContext),KagemushaAndroidPlayIntegrityProviderV1(context.applicationContext)),
   transport,requireOriginalOwner)
 private val running=AtomicBoolean(false)
 private var frozen=false
 private var nonce:ByteArray?=null
 private var enrollment:ByteArray?=null
 private var prepared:OrdinaryIntegrityPreparedStepV1?=null
 private var der:ByteArray?=null
 private var token:ByteArray?=null
 private var future:CompletableFuture<String>?=null
 private var tokenCaptured:List<ByteArray>?=null
 private var carrier:KagemushaOrdinaryIntegrityHttpOriginalV1?=null
 private var pendingResponse:ByteArray?=null
 private var lease:ByteArray?=null
 private var acknowledged:KagemushaOrdinaryIntegrityAcknowledgementV1?=null
 suspend fun beginOrResumeRefresh():KagemushaOrdinaryIntegrityAcknowledgementV1 =perform(false)
 /** Explicit dependency refresh for an authorised current operation; never an expiry timer. */
 suspend fun refreshCurrentIntegrity():KagemushaOrdinaryIntegrityAcknowledgementV1=perform(true)
 private suspend fun perform(fresh:Boolean):KagemushaOrdinaryIntegrityAcknowledgementV1 {
  check(running.compareAndSet(false,true))
  try{
   current();if(fresh && acknowledged!=null)acknowledged=null
   acknowledged?.let{return it}
   actual{native.refreshAccount()}
   val recovery=invoke(8)
   if(nonce==null){
    if(recovery[2].isEmpty()) {val reserved=invoke(1);enrollment=reserved[0].copyOf();nonce=reserved[1].copyOf()}
    else {nonce=recovery[2].copyOf();enrollment=invoke(10)[0].copyOf()}
   }
   // Completed custody exposes the genuine enrollment ID even when the old baseline PI expired.
   if(enrollment==null) enrollment=invoke(10)[0].copyOf()
   if(recovery[2].isNotEmpty()) check(recovery[2].contentEquals(checkNotNull(nonce)))
   if(prepared==null){
    if(recovery[4].isNotEmpty()) prepared=actual{native.recoverPrepared()}
    else {
     if(carrier==null)carrier=KagemushaOrdinaryIntegrityHttpOriginalV1.prepare(checkNotNull(enrollment),checkNotNull(nonce),::current)
     val response=exchange()
     val signed=try{KagemushaOrdinaryIntegrityHttpCodecV1.prepareResponse(response,checkNotNull(nonce))}
      catch(failure:Throwable){freeze(failure)}
     prepared=actual{native.acceptChallenge(signed)};clearTransport()
    }
   }
   val held=checkNotNull(prepared)
   if(der==null)der=actual{held.approve()}
   if(token==null){
    if(recovery[6].isNotEmpty()) token=recovery[6].copyOf()
    else {
     val pending=future ?: actual{held.requestToken()}.also{future=it}
     val raw=pending.retainedOriginalAwait()
     current();require(raw.isNotEmpty() && raw.length<=65536 && raw.all{it.code in 0x21..0x7e})
     token=raw.toByteArray(Charsets.US_ASCII)
    }
   }
   if(tokenCaptured==null)tokenCaptured=invoke(5,checkNotNull(token)).also {
    check(it[1].contentEquals(held.original()) && it[2].contentEquals(checkNotNull(der)) && it[3].contentEquals(checkNotNull(token)))
   }
   if(lease==null){
    if(recovery[7].isNotEmpty())lease=recovery[7].copyOf()
    else {
     if(carrier==null)carrier=KagemushaOrdinaryIntegrityHttpOriginalV1.finish(checkNotNull(tokenCaptured),::current)
     val response=exchange()
     lease=try{KagemushaOrdinaryIntegrityHttpCodecV1.finishResponse(response)}catch(failure:Throwable){freeze(failure)}
    }
   }
   val digest=invoke(6,checkNotNull(lease)).single()
   return KagemushaOrdinaryIntegrityAcknowledgementV1(digest).also{acknowledged=it;clearTransport();wipeAttempt()}
  }finally{running.set(false)}
 }
 /** Separately invokes Native's actual recovered Bootstrap activation; no managed grant. */
 suspend fun activateRecoveredBootstrap():Boolean {
  check(running.compareAndSet(false,true));try{current();actual{native.refreshAccount()};return invoke(9).single()[0]==1.toByte()}
  finally{running.set(false)}
 }
 /** Explicit user-directed retirement of a pending PI nonce. Never called by expiry/retry timers. */
 suspend fun abandonRetainedAttempt() {
  check(running.compareAndSet(false,true));try{current();actual{native.refreshAccount()};invoke(7);clearTransport();wipeAttempt();acknowledged=null}
  finally{running.set(false)}
 }
 fun releaseCompletedRefresh(){check(running.compareAndSet(false,true));try{current();checkNotNull(acknowledged);acknowledged=null}finally{running.set(false)}}
 private suspend fun exchange():ByteArray {
  pendingResponse?.let{return it.copyOf()}
  val body=checkNotNull(carrier)
  val response=try{transport.exchange(body)}catch(failure:Throwable){current();throw failure}
  current();require(response.size in 1..body.maximumResponseBytes)
  pendingResponse=response.copyOf();return response
 }
 private suspend fun invoke(phase:Int,original:ByteArray=ByteArray(0))=actual {
  KagemushaOrdinaryIntegrityRefreshFrameV1.requireRequest(phase,original)
  native.invoke(phase,original).also{KagemushaOrdinaryIntegrityRefreshFrameV1.requireResponse(phase,it)}
 }
 private fun current(){check(!frozen);requireOriginalOwner();native.requireOpen();requireOriginalOwner()}
 private suspend fun<T> actual(op:suspend ()->T):T=try{current();op().also{current()}}catch(failure:Throwable){freeze(failure)}
 private fun freeze(failure:Throwable):Nothing{frozen=true;clearTransport();wipeAttempt();try{native.revoke()}catch(_:Throwable){};throw failure}
 private fun clearTransport(){carrier?.clear();carrier=null;pendingResponse?.fill(0);pendingResponse=null}
 private fun wipeAttempt(){nonce?.fill(0);nonce=null;enrollment?.fill(0);enrollment=null;der?.fill(0);der=null;token?.fill(0);token=null
  tokenCaptured?.forEach{it.fill(0)};tokenCaptured=null;lease?.fill(0);lease=null;prepared=null;future=null}
}
// The future is owned by the retained workflow, not by the cancelled UI coroutine.
private suspend fun<T> CompletableFuture<T>.retainedOriginalAwait():T=suspendCoroutine{continuation->
 whenComplete{value,failure->if(failure!=null)continuation.resumeWithException(failure)else continuation.resume(value)}
}
private class ActualIntegrityNativeV1(private val binding:KagemushaOrdinaryIntegrityRefreshTransportBindingV1,
 private val hardware:KagemushaAndroidHardwareAppKeyStoreV1,private val google:KagemushaAndroidPlayIntegrityProviderV1):OrdinaryIntegrityWorkflowNativeV1 {
 private val io=KagemushaRetainedNativeIoV1()
 override suspend fun refreshAccount()=io.call{binding.refreshCurrentAccount(KagemushaOrdinaryRuntimeJniV1)}
 override fun requireOpen()=binding.requireOpen();override fun revoke(){try{binding.revoke()}finally{io.retire()}}
 override suspend fun invoke(phase:Int,original:ByteArray)=io.call{binding.invoke(KagemushaOrdinaryRuntimeJniV1,phase,original)}
 override suspend fun acceptChallenge(raw:ByteArray)=io.call{step(binding.acceptChallenge(KagemushaOrdinaryRuntimeJniV1,raw))}
 override suspend fun recoverPrepared()=io.call{step(binding.recoverPrepared(KagemushaOrdinaryRuntimeJniV1))}
 private fun step(prepared:KagemushaNativePreparedOrdinaryIntegrityRefreshV1)=object:OrdinaryIntegrityPreparedStepV1 {
  override fun original()=prepared.preparationOriginal();override fun requestHash()=prepared.requestHash()
  override suspend fun approve()=io.call{hardware.approveOrdinaryIntegrityRefresh(prepared)}
  override fun requestToken()=google.requestOriginal(prepared.cloudProjectNumber(),prepared.requestHash(),prepared::requireCurrent).thenApply{original->
   check(original.cloudProjectNumber==prepared.cloudProjectNumber() && original.requestHash().contentEquals(prepared.requestHash()))
   original.opaqueToken()}
 }
}
