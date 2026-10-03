package org.hyperledger.iroha.sdk.offline
import java.math.BigInteger
import java.security.MessageDigest
import kotlinx.coroutines.runBlocking
import org.junit.jupiter.api.Test
import kotlin.test.*
/** TEST ONLY scripted sequencing; no fixture owns a Native proof, key or monetary authority. */
class KagemushaOrdinaryOutgoingCashV1Test {
 private class Native:OrdinaryOutgoingWorkflowNativeV1 {
  val events=mutableListOf<String>();var closed=false;var current=true;var approvals=0;var last=ByteArray(32);var kind=0
  var acknowledged=false
  override fun requireOpen(){check(!closed)};override fun revoke(){closed=true}
  private fun step()=object:OrdinaryOutgoingApprovalStepV1 {override suspend fun approve(){check(current);approvals++;events+="approve"}}
  override suspend fun prepare(kind:Int,business:ByteArray,amount:BigInteger?):OrdinaryOutgoingApprovalStepV1 {
   check(current);this.kind=kind;events+="prepare";if(kind==4)assertNotNull(amount);return step()
  }
  override suspend fun terminal(reserveKey:ByteArray):OrdinaryOutgoingApprovalStepV1 {check(current);events+="terminal";assertContentEquals(last,reserveKey);return step()}
  override suspend fun invoke(phase:Int,fields:List<ByteArray>):List<ByteArray> {
   events+=phase.toString();if(phase==16)current=true else if(phase !in listOf(5,12,17))check(current)
   return when(phase){5,12->{current=false;listOf(ByteArray(32){phase.toByte()})};6,13->{
    val req=byteArrayOf(phase.toByte());last=MessageDigest.getInstance("SHA-256").digest(req)
    listOf(byteArrayOf(if(acknowledged)2 else 0),req,ByteArray(64){phase.toByte()},byteArrayOf(3),last.copyOf())}
    7->listOf(last.copyOf());17->listOf(byteArrayOf(17));else->emptyList()}
  }
 }
 private fun response()="{\"signed_result_original_base64\":\"AQ==\",\"data_record_original_base64\":\"Ag==\",\"authority_original_base64\":\"Aw==\"}".toByteArray()
 private fun workflow(n:Native,http:KagemushaOrdinaryLineageOriginalTransportV1=KagemushaOrdinaryLineageOriginalTransportV1{response()},owner:()->Unit={}):KagemushaOrdinaryOutgoingCashV1 =
  KagemushaOrdinaryOutgoingCashV1(n,{check(n.current);n.events+="FI"},{check(n.current);n.events+="PI"},http,owner)
 @Test fun bothLongProofsRetainResultAndRenewSameOwnerBeforeEachSignature():Unit=runBlocking {
  val n=Native();val done=workflow(n).beginOrResumeSend(byteArrayOf(1));assertContentEquals(byteArrayOf(17),done.completeOutgoingOriginal());assertEquals(2,n.approvals)
  for((proof,sign)in listOf("5" to "6","12" to "13")){val between=n.events.subList(n.events.indexOf(proof)+1,n.events.indexOf(sign))
   assertEquals(listOf("16","PI","FI"),between)}
  assertTrue(n.events.indexOf("14")<n.events.indexOf("15"));assertEquals(2,n.events.count{it=="7"})
 }
 @Test fun exactHttpUncertaintyReusesWholeBodyAndNeverRepeatsProofOrApproval():Unit=runBlocking {
  val n=Native();var calls=0;var bytes=byteArrayOf();var id=""
  val w=workflow(n,KagemushaOrdinaryLineageOriginalTransportV1{c->val out=java.io.ByteArrayOutputStream();c.writeBodyTo(out);calls++
   if(calls==1){bytes=out.toByteArray();id=c.requestId;error("TEST HTTP outcome unknown")}
   if(calls==2){assertContentEquals(bytes,out.toByteArray());assertEquals(id,c.requestId)};response()})
  assertFails{w.beginOrResumeSend(byteArrayOf(1))};assertFalse(n.closed);w.beginOrResumeSend(byteArrayOf(1))
  assertEquals(1,n.events.count{it=="5"});assertEquals(1,n.events.count{it=="12"});assertEquals(1,n.events.count{it=="6"});assertEquals(2,n.approvals)
 }
 @Test fun financialHttpUncertaintyKeepsProofAndResumesSameFinancialOriginal():Unit=runBlocking {
  val n=Native();var failed=false;val choices=mutableListOf<Boolean>()
  val w=KagemushaOrdinaryOutgoingCashV1(n,{fresh->choices+=fresh;check(n.current)
   if(n.events.contains("5")&&!failed){failed=true;error("TEST FI HTTP uncertain")}}, {},KagemushaOrdinaryLineageOriginalTransportV1{response()}, {})
  assertFails{w.beginOrResumeSend(byteArrayOf(1))};assertFalse(n.closed);w.beginOrResumeSend(byteArrayOf(1))
  assertEquals(listOf(false,true,true,false),choices.take(4));assertEquals(1,n.events.count{it=="5"});assertEquals(2,n.approvals)
 }
 @Test fun differentBusinessCannotReplacePendingOriginal():Unit=runBlocking {
  val n=Native();val w=workflow(n,KagemushaOrdinaryLineageOriginalTransportV1{error("TEST pending HTTP")})
  assertFails{w.beginOrResumeSend(byteArrayOf(1))};assertFails{w.beginOrResumeSend(byteArrayOf(2))}
  assertFails{w.beginOrResumeRedemption(BigInteger.ONE)};assertFails{w.releaseCompletedCycle()};assertEquals(1,n.approvals)
 }
 @Test fun malformedReplyAndOwnerRetirementRefuseCommit():Unit=runBlocking {
  val n=Native();val w=workflow(n,KagemushaOrdinaryLineageOriginalTransportV1{"{}".toByteArray()})
  assertFails{w.beginOrResumeSend(byteArrayOf(1))};assertTrue(n.closed);assertFalse(n.events.contains("13"))
  val other=Native();var current=true;val held=workflow(other,KagemushaOrdinaryLineageOriginalTransportV1{current=false;response()}){check(current)}
  assertFails{held.beginOrResumeSend(byteArrayOf(1))};assertTrue(other.closed);assertFalse(other.events.contains("7"))
 }
 @Test fun acknowledgedCycleReleaseAllowsNextRealOperationWithoutChangingOldDelivery():Unit=runBlocking {
  val n=Native();val w=workflow(n);val old=w.beginOrResumeRedemption(BigInteger.TEN);assertEquals(4,n.kind)
  w.releaseCompletedCycle();w.beginOrResumeSend(byteArrayOf(1));assertEquals(2,n.kind);assertEquals(4,n.approvals)
  assertContentEquals(byteArrayOf(17),old.completeOutgoingOriginal());assertFails{w.beginOrResumeRedemption(BigInteger.ZERO)}
  assertFails{w.beginOrResumeRedemption(BigInteger.ONE.shiftLeft(128))}
 }
 @Test fun genuineNativeAcknowledgedRequestDoesNotRedispatchHttp():Unit=runBlocking {
  val n=Native();n.acknowledged=true;workflow(n,KagemushaOrdinaryLineageOriginalTransportV1{error("No HTTP")}).beginOrResumeSend(byteArrayOf(1))
  assertFalse(n.events.contains("7"));assertTrue(n.events.contains("15"))
 }
 @Test fun actualRetainedNativeWorkerRunsOutsideUiThread():Unit=runBlocking {
  val io=KagemushaRetainedNativeIoV1();val caller=Thread.currentThread()
  try {val worker=io.call {Thread.currentThread()};assertNotSame(caller,worker)
   assertEquals("iroha-ordinary-native-workflow",worker.name)
  }finally{io.retire()}
 }

}
