// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline
import org.junit.jupiter.api.Test
import kotlin.test.*
import kotlinx.coroutines.runBlocking
import java.security.MessageDigest
import java.io.ByteArrayOutputStream
/** Inert lifecycle controls. No Native owner, platform key, financial proof or grant is made. */
class KagemushaOrdinaryIncomingV1Test {
 private class Native:OrdinaryIncomingWorkflowNativeV1 {
  val calls=mutableListOf<Int>();var approves=0;var retired=false;var fail=0;var currentRequest=byteArrayOf(6)
  override suspend fun refreshAccount(){calls+=16;check(!retired)}
  private fun step()=object:OrdinaryIncomingApprovalStepV1 {override suspend fun approve(){approves++};override fun operationId()=ByteArray(32){1}}
  override suspend fun prepareMint(finalized:ByteArray,credit:ByteArray):OrdinaryIncomingApprovalStepV1 {calls+=1;return step()}
  override suspend fun prepareReceive(request:ByteArray,outgoing:ByteArray,assertion:ByteArray):OrdinaryIncomingApprovalStepV1 {calls+=17;return step()}
  override suspend fun selectTerminal(reserve:ByteArray):OrdinaryIncomingApprovalStepV1 {calls+=8;return step()}
  override suspend fun invoke(phase:Int,originals:List<ByteArray>):List<ByteArray> {
   calls+=phase;check(!retired);check(phase!=fail){"unknown Native result"}
   return when(phase){6,13->{currentRequest=byteArrayOf(phase.toByte());listOf(byteArrayOf(0),currentRequest,ByteArray(64){3},byteArrayOf(4),sha(currentRequest))}
    7->listOf(sha(currentRequest));14,15->emptyList();else->listOf(ByteArray(32){phase.toByte()})}
  }
  override fun requireOpen(){check(!retired)};override fun revoke(){retired=true}
  private fun sha(b:ByteArray)=MessageDigest.getInstance("SHA-256").digest(b)
 }
 private val response="{\"signed_result_original_base64\":\"AQ==\",\"data_record_original_base64\":\"Ag==\",\"authority_original_base64\":\"Aw==\"}".toByteArray()
 @Test fun realPhaseOrderIncludesRenewalsBeforeSignsAndEffects()=runBlocking {
  val n=Native();val flow=KagemushaOrdinaryIncomingV1(n,{}, {response},{})
  flow.beginOrResumeFinalizedMint(byteArrayOf(1),byteArrayOf(2))
  assertEquals(listOf(16,1,5,16,6,7,16,8,12,16,13,7,16,14,15),n.calls);assertEquals(2,n.approves)
  val count=n.calls.size;flow.beginOrResumeFinalizedMint(byteArrayOf(1),byteArrayOf(2));assertEquals(count,n.calls.size)
 }
 @Test fun httpRetryReusesBodyCarrierAndNeverRepeatsProofOrPlatform()=runBlocking {
  val n=Native();val originals=mutableListOf<KagemushaOrdinaryIncomingHttpOriginalV1>();var fail=true
  val flow=KagemushaOrdinaryIncomingV1(n,{}, { o->originals+=o;if(fail){fail=false;error("lost HTTP")}else response},{})
  assertFails { flow.beginOrResumeFinalizedMint(byteArrayOf(1),byteArrayOf(2)) }
  flow.beginOrResumeFinalizedMint(byteArrayOf(1),byteArrayOf(2));assertTrue(originals[0]===originals[1])
  assertEquals(1,n.calls.count {it==5});assertEquals(1,n.calls.count {it==6});assertEquals(2,n.approves)
 }
 @Test fun nativeUnknownOutcomeRetiresAndCannotRestart()=runBlocking {
  val n=Native().also{it.fail=12};val flow=KagemushaOrdinaryIncomingV1(n,{}, {response},{})
  assertFails {flow.beginOrResumeFinalizedMint(byteArrayOf(1),byteArrayOf(2))};assertTrue(n.retired)
  val count=n.calls.size;assertFails{flow.beginOrResumeFinalizedMint(byteArrayOf(1),byteArrayOf(2))};assertEquals(count,n.calls.size)
 }
 @Test fun sameUnfinishedOperationCannotBeReplacedOrReleased()=runBlocking {
  val n=Native();val flow=KagemushaOrdinaryIncomingV1(n,{}, {error("HTTP")},{})
  assertFails {flow.beginOrResumeFinalizedMint(byteArrayOf(1),byteArrayOf(2))}
  assertFails {flow.releaseCompletedCycle()};assertFails {flow.beginOrResumeReceive(ByteArray(32){1},byteArrayOf(3),byteArrayOf(4))};Unit
 }
 @Test fun completedCycleCanExplicitlyReleaseForNextReceive()=runBlocking {
  val n=Native();val flow=KagemushaOrdinaryIncomingV1(n,{}, {response},{})
  flow.beginOrResumeFinalizedMint(byteArrayOf(1),byteArrayOf(2));flow.releaseCompletedCycle()
  flow.beginOrResumeReceive(ByteArray(32){1},byteArrayOf(3),byteArrayOf(4));assertEquals(1,n.calls.count {it==17})
 }
 @Test fun financialHttpUncertaintyResumesSameFinancialOriginal()=runBlocking {
  val fresh=mutableListOf<Boolean>();var failed=false;val n=Native()
  val flow=KagemushaOrdinaryIncomingV1(n,{f->fresh+=f;if(!failed){failed=true;error("FI HTTP")}},{response},{})
  assertFails {flow.beginOrResumeFinalizedMint(byteArrayOf(1),byteArrayOf(2))};assertFalse(n.retired)
  flow.beginOrResumeFinalizedMint(byteArrayOf(1),byteArrayOf(2));assertEquals(listOf(true,false),fresh.take(2));assertEquals(1,n.calls.count {it==1})
 }
 @Test fun ownershipFailureCannotDispatchOrCreateAnotherCycle()=runBlocking {
  val n=Native();val flow=KagemushaOrdinaryIncomingV1(n,{}, {response},{error("selection retired")})
  assertFails {flow.beginOrResumeFinalizedMint(byteArrayOf(1),byteArrayOf(2))};assertTrue(n.calls.isEmpty())
 }
}
