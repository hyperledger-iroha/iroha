// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline
import android.content.Context
import java.math.BigInteger
import java.io.OutputStream
import java.util.concurrent.atomic.AtomicBoolean
import java.security.MessageDigest
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidHardwareAppKeyStoreV1
fun interface KagemushaOrdinaryMintFundingOriginalTransportV1 {suspend fun exchange(original:KagemushaOrdinaryMintFundingHttpOriginalV1):ByteArray}
/** Closed immutable eight-original carrier, selected only by Native7 or Native14. */
class KagemushaOrdinaryMintFundingHttpOriginalV1 private constructor(fields:List<ByteArray>,recover:Boolean,private val guard:()->Unit){
 private val f=fields.map(ByteArray::copyOf)
 val path=if(recover)KagemushaOrdinaryMintFundingHttpCodecV1.RECOVER_PATH else KagemushaOrdinaryMintFundingHttpCodecV1.ISSUE_PATH
 val requestId=KagemushaOrdinaryMintFundingHttpCodecV1.requestId(f)
 val maximumResponseBytes=KagemushaOrdinaryMintFundingHttpCodecV1.MAXIMUM_RESPONSE_BYTES
 fun requireCurrent(){guard()}
 fun writeBodyTo(output:OutputStream){requireCurrent();KagemushaOrdinaryMintFundingHttpCodecV1.writeRequest(f,output);requireCurrent()}
 internal fun clear(){f.forEach{it.fill(0)}}
 internal fun exactOriginals()=f.map(ByteArray::copyOf)
 internal companion object{fun fromNative(fields:List<ByteArray>,recover:Boolean,guard:()->Unit):KagemushaOrdinaryMintFundingHttpOriginalV1{
  KagemushaOrdinaryMintFundingFrameV1.requireResponse(if(recover)14 else 7,fields)
  return KagemushaOrdinaryMintFundingHttpOriginalV1(fields,recover,guard)}}
}
/** Pending/finalized correlation data only. Native retains the genuine source and financial grant. */
class KagemushaOrdinaryMintFundingStatusV1 internal constructor(finalized:ByteArray?){
 private val original=finalized?.copyOf()
 fun pending()=original==null
 fun finalizedOriginalOrNull()=original?.copyOf()
}
internal interface OrdinaryMintFundingApprovalStepV1{suspend fun approve();fun operationId():ByteArray}
internal interface OrdinaryMintFundingWorkflowNativeV1 {
 suspend fun prepare(amountLE128:ByteArray):OrdinaryMintFundingApprovalStepV1
 suspend fun recoverPlatformApproval():OrdinaryMintFundingApprovalStepV1
 suspend fun invoke(phase:Int,fields:List<ByteArray> = emptyList()):List<ByteArray>
 fun requireOpen();fun revoke()
}
/** Dedicated Native funding, not a regular transfer-to-reserve. Keep one object through UI
 * cancellation/HTTP uncertainty. Native signs consent/transaction, submits and independently
 * admits finality. A pending/HTTP/submission response creates no balance or Mint/Receive grant.
 */
class KagemushaOrdinaryMintFundingV1 internal constructor(private val native:OrdinaryMintFundingWorkflowNativeV1,
 private val financial:suspend(Boolean)->Unit,private val integrity:suspend()->Unit,
 private val transport:KagemushaOrdinaryMintFundingOriginalTransportV1,private val owner:()->Unit){
 companion object {
  fun open(context:Context,coordinator:KagemushaNativeCoreCoordinatorAdapterV1,currentControl:KagemushaOrdinaryCurrentControlV1,
   integrity:KagemushaOrdinaryIntegrityRefreshV1,transport:KagemushaOrdinaryMintFundingOriginalTransportV1,
   requireOriginalOwner:()->Unit):KagemushaOrdinaryMintFundingV1 {
   requireOriginalOwner();val binding=coordinator.ordinaryMintFundingTransportBinding(requireOriginalOwner)
   requireOriginalOwner();return KagemushaOrdinaryMintFundingV1(ActualNative(binding,KagemushaAndroidHardwareAppKeyStoreV1(context.applicationContext)),
    {fresh->if(fresh)currentControl.refreshCurrentFinancialControl() else currentControl.beginOrResumeCurrentFinancialControl()},
    {integrity.refreshCurrentIntegrity();Unit},transport,requireOriginalOwner)
  }
 }
 private class ActualNative(private val binding:KagemushaOrdinaryMintFundingTransportBindingV1,private val keys:KagemushaAndroidHardwareAppKeyStoreV1):OrdinaryMintFundingWorkflowNativeV1 {
  private val io=KagemushaRetainedNativeIoV1()
  override fun requireOpen()=binding.requireOpen()
  override fun revoke(){try{binding.revoke()}finally{io.retire()}}
  override suspend fun invoke(phase:Int,fields:List<ByteArray>):List<ByteArray> = io.call{binding.invoke(KagemushaOrdinaryRuntimeJniV1,phase,fields)}
  override suspend fun recoverPlatformApproval():OrdinaryMintFundingApprovalStepV1=io.call {
   val retained=binding.invoke(KagemushaOrdinaryRuntimeJniV1,12)
   val prepared=binding.prepared(KagemushaOrdinaryRuntimeJniV1,KagemushaOrdinaryRuntimeJniV1,retained.subList(1,4))
   approval(prepared)
  }
  override suspend fun prepare(amountLE128:ByteArray):OrdinaryMintFundingApprovalStepV1=io.call {
   val prepared=binding.prepared(KagemushaOrdinaryRuntimeJniV1,KagemushaOrdinaryRuntimeJniV1,binding.invoke(KagemushaOrdinaryRuntimeJniV1,1,listOf(amountLE128)))
   approval(prepared)
  }
  private fun approval(prepared:KagemushaNativePreparedOrdinaryMintApprovalV1)=object:OrdinaryMintFundingApprovalStepV1 {
   override fun operationId()=prepared.operationId()
   override suspend fun approve(){io.call{keys.approveOrdinaryMintFunding(prepared).fill(0)}}
  }
 }
 private class Cycle(val amount:ByteArray?){var approval:OrdinaryMintFundingApprovalStepV1?=null;var captured=false;var proven=false
  var consent=false;var predebit=false;var httpInvoked=false;var carrier:KagemushaOrdinaryMintFundingHttpOriginalV1?=null
  var response:List<ByteArray>?=null;var accepted=false;var transaction=false;var dispatched=false;var finalized:KagemushaOrdinaryMintFundingStatusV1?=null;var operation:ByteArray?=null}
 private val running=AtomicBoolean(false);private var frozen=false;private var financialPending=false;private var cycle:Cycle?=null
 suspend fun beginOrResume(amount:BigInteger):KagemushaOrdinaryMintFundingStatusV1 {
  require(amount.signum()>0 && amount.bitLength()<=128)
  val bytes=amount.toByteArray();val le=ByteArray(16);repeat(minOf(16,bytes.size)){le[it]=bytes[bytes.size-1-it]}
  check(running.compareAndSet(false,true)){"The original Mint funding attempt is active"}
  try{current();val held=cycle?.also{check(it.amount!=null && same(it.amount,le)){"An unfinished funding operation cannot be replaced"}}
    ?:Cycle(le.copyOf()).also{cycle=it};held.finalized?.let{return it}
   return runHeld(held)
  }finally{le.fill(0);bytes.fill(0);running.set(false)}
 }
 /** Read only actual Native17 to recover an existing durable attempt. No amount, stage,
  * offered signature, nonce, decision or transaction can select a recovery disposition. */
 suspend fun recoverRetained():KagemushaOrdinaryMintFundingStatusV1 {
  check(running.compareAndSet(false,true)){"The original Mint funding attempt is active"}
  try{current();val progress=invoke(17);val op=progress[0];val stage=progress[1][0].toInt()
   if(stage==2 || stage==8)freeze(IllegalStateException("The original Native signing invocation is uncertain"))
   val held=cycle ?: Cycle(null).also{cycle=it}
   held.operation?.let{check(same(it,op)){"The retained Native operation changed"}};held.operation=op.copyOf()
   // These flags are derived exclusively from authenticated acknowledged Native WAL chronology.
   held.captured=stage>=1;held.proven=stage>=1;held.consent=stage>=3;held.predebit=stage>=4
   held.httpInvoked=stage>=5;held.accepted=stage>=7;held.transaction=stage>=9;held.dispatched=stage>=10
   return runHeld(held)
  }finally{running.set(false)}
 }
 private suspend fun runHeld(held:Cycle):KagemushaOrdinaryMintFundingStatusV1 {
  held.finalized?.let{return it}
   refreshDependencies()
   if(held.approval==null && !held.captured){held.approval=actual{
    if(held.amount==null)native.recoverPlatformApproval()else native.prepare(held.amount.copyOf())}
    val op=checkNotNull(held.approval).operationId();held.operation?.let{check(same(it,op))};held.operation=op}
   if(!held.captured){actual{checkNotNull(held.approval).approve()};held.captured=true}
   if(!held.proven){invoke(4);held.proven=true} // Historical proof data survives finite current S/W expiry.
   if(!held.consent){refreshDependencies();invoke(5);held.consent=true}
   if(!held.predebit){refreshDependencies();invoke(6);held.predebit=true}
   if(!held.accepted){exchangeDecision(held);refreshDependencies();invoke(8,checkNotNull(held.response));held.accepted=true
    held.response?.forEach{it.fill(0)};held.response=null;held.carrier?.clear();held.carrier=null}
   if(!held.transaction){refreshDependencies();invoke(9);held.transaction=true}
   if(!held.dispatched){refreshDependencies();invoke(10);held.dispatched=true}
   refreshDependencies();val final=invoke(11)
   val result=KagemushaOrdinaryMintFundingStatusV1(if(final[0][0]==1.toByte())final[1]else null)
   if(!result.pending())held.finalized=result
   current();return result
 }
 private suspend fun exchangeDecision(h:Cycle){
  if(h.response!=null)return
  val recover=h.httpInvoked
  val fields=invoke(if(recover)14 else 7)
  val next=KagemushaOrdinaryMintFundingHttpOriginalV1.fromNative(fields,recover,::current)
  h.carrier?.let{old->val a=old.exactOriginals();val b=next.exactOriginals()
   check(a.indices.all{same(a[it],b[it])}){"Native recovery replaced the exact pre-debit request"}
   a.forEach{it.fill(0)};b.forEach{it.fill(0)};check(old.requestId==next.requestId);old.clear()}
  h.carrier=next;h.httpInvoked=true // A lost HTTP return may only read the same original through /recover.
  val raw=transport.exchange(next) // Real HTTP failure retains carrier/Native fence; no automatic issue retry.
  h.response=try{KagemushaOrdinaryMintFundingHttpCodecV1.responseOriginals(raw)}catch(e:Throwable){freeze(e)}finally{raw.fill(0)}
  current()
 }
 private suspend fun refreshDependencies(){current();invoke(13);integrity();current()
  val fresh=!financialPending;financialPending=true;financial(fresh);financialPending=false;current()}
 private suspend fun invoke(phase:Int,f:List<ByteArray> = emptyList()):List<ByteArray> = actual {
  KagemushaOrdinaryMintFundingFrameV1.requireRequest(phase,f);native.invoke(phase,f).also{KagemushaOrdinaryMintFundingFrameV1.requireResponse(phase,it)}}
 private suspend fun <T> actual(block:suspend()->T):T {current();return try{block().also{current()}}catch(e:Throwable){freeze(e)}}
 private fun current(){check(!frozen){"The original Native funding owner is closed"};owner();native.requireOpen();owner()}
 private fun freeze(e:Throwable):Nothing{frozen=true;cycle?.carrier?.clear();cycle?.response?.forEach{it.fill(0)}
  try{native.revoke()}catch(retired:Throwable){e.addSuppressed(retired)};throw e}
 private fun same(a:ByteArray,b:ByteArray)=MessageDigest.isEqual(a,b)
}
