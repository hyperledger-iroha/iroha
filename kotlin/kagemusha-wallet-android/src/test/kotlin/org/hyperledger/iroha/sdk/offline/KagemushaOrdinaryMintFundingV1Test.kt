package org.hyperledger.iroha.sdk.offline
import org.junit.jupiter.api.Test
import kotlin.test.*
import kotlinx.coroutines.runBlocking
/** TEST ONLY scripted transport tests sequence/retry. No fixture manufactures a Native owner. */
class KagemushaOrdinaryMintFundingV1Test {
 private class Native(var stage:Int=0):OrdinaryMintFundingWorkflowNativeV1 {
  val phases=mutableListOf<Int>();var closed=false;var approvals=0;var proofCalls=0;var current=true;var final=false
  override fun requireOpen(){check(!closed)};override fun revoke(){closed=true}
  override suspend fun recoverPlatformApproval():OrdinaryMintFundingApprovalStepV1 {phases+=120;return step()}
  private fun step()=object:OrdinaryMintFundingApprovalStepV1{override fun operationId()=ByteArray(32){1};override suspend fun approve(){approvals++}}
  override suspend fun prepare(amountLE128:ByteArray):OrdinaryMintFundingApprovalStepV1{phases+=1;check(current)
   return object:OrdinaryMintFundingApprovalStepV1{override fun operationId()=ByteArray(32){1};override suspend fun approve(){approvals++;check(current)}}}
  fun eight()=List(8){n->if(n==2)ByteArray(64){3}else if(n==4)ByteArray(0)else byteArrayOf((n+1).toByte())}
  override suspend fun invoke(phase:Int,fields:List<ByteArray>):List<ByteArray>{phases+=phase
   if(phase==13)current=true else if(phase !in listOf(4,12,14,15,17))check(current)
   return when(phase){17->listOf(ByteArray(32){1},byteArrayOf(stage.toByte()));4->{proofCalls++;current=false;listOf(ByteArray(32){4})};5->listOf(ByteArray(64){5});6,7,14->eight();9->listOf(ByteArray(32){9});11->listOf(byteArrayOf(if(final)1 else 0),if(final)byteArrayOf(1)else byteArrayOf());else->emptyList()}}
 }
 private fun response():ByteArray {val names=listOf("signed_decision_original_base64","decision_signed_clock_original_base64","decision_financial_control_original_base64","reserved_data_record_original_base64","node_submission_original_base64");return ("{"+names.joinToString(","){"\"$it\":\"AQ==\""}+"}").toByteArray()}
 @Test fun longProofReturnsDataThenActualRenewalBeforeConsent()=runBlocking {val n=Native();var pi=0;var fi=0
  val w=KagemushaOrdinaryMintFundingV1(n,{check(n.current);fi++},{check(n.current);pi++},KagemushaOrdinaryMintFundingOriginalTransportV1{response()},{})
  assertTrue(w.beginOrResume(java.math.BigInteger.TEN).pending());assertEquals(1,n.approvals);assertEquals(1,n.proofCalls)
  val proof=n.phases.indexOf(4);val consent=n.phases.indexOf(5);assertTrue(n.phases.subList(proof+1,consent).contains(13));assertTrue(pi>=5 && fi>=5)
 }
 @Test fun uncertainCoreOnlyReadsExactOriginalAndNeverSignsAgain()=runBlocking {val n=Native();var calls=0;var id="";var initial:ByteArray?=null
  val w=KagemushaOrdinaryMintFundingV1(n,{}, {},KagemushaOrdinaryMintFundingOriginalTransportV1{c->
   val out=java.io.ByteArrayOutputStream();c.writeBodyTo(out);calls++
   if(calls==1){id=c.requestId;initial=out.toByteArray();assertEquals(KagemushaOrdinaryMintFundingHttpCodecV1.ISSUE_PATH,c.path);error("TEST HTTP uncertainty")}
   assertEquals(id,c.requestId);assertContentEquals(initial,out.toByteArray());assertEquals(KagemushaOrdinaryMintFundingHttpCodecV1.RECOVER_PATH,c.path);response()},{})
  assertFails{w.beginOrResume(java.math.BigInteger.TEN)};assertTrue(w.beginOrResume(java.math.BigInteger.TEN).pending())
  assertEquals(1,n.approvals);assertEquals(1,n.proofCalls);assertEquals(1,n.phases.count{it==7});assertEquals(1,n.phases.count{it==14})
 }
 @Test fun pendingFinalityDoesNotResubmitOrReprove()=runBlocking {val n=Native();val w=KagemushaOrdinaryMintFundingV1(n,{}, {},KagemushaOrdinaryMintFundingOriginalTransportV1{response()},{})
  assertTrue(w.beginOrResume(java.math.BigInteger.ONE).pending());n.final=true
  val done=w.beginOrResume(java.math.BigInteger.ONE);assertFalse(done.pending());assertContentEquals(byteArrayOf(1),done.finalizedOriginalOrNull())
  assertEquals(1,n.phases.count{it==10});assertEquals(1,n.proofCalls);assertEquals(1,n.approvals)
 }
 @Test fun wrongAmountOrSoftwareUnavailableCannotReplaceOriginal()=runBlocking {val n=Native();val w=KagemushaOrdinaryMintFundingV1(n,{}, {},KagemushaOrdinaryMintFundingOriginalTransportV1{response()},{})
  w.beginOrResume(java.math.BigInteger.ONE);assertFails{w.beginOrResume(java.math.BigInteger.valueOf(2))}
  assertFails{w.beginOrResume(java.math.BigInteger.ZERO)};assertFails{w.beginOrResume(java.math.BigInteger.ONE.shiftLeft(128))};assertEquals(1,n.approvals)
 }
 @Test fun malformedCoreReplyClosesBeforeTransactionOrDebit()=runBlocking {val n=Native();val w=KagemushaOrdinaryMintFundingV1(n,{}, {},KagemushaOrdinaryMintFundingOriginalTransportV1{"{}".toByteArray()},{})
  assertFails{w.beginOrResume(java.math.BigInteger.ONE)};assertTrue(n.closed);assertFalse(n.phases.contains(9));assertFalse(n.phases.contains(10))
 }
 @Test fun coldAcknowledgedStagesNeverRepeatEarlierEffects()=runBlocking {
  for(stage in listOf(1,3,4,5,6,7,9,10,11,12)){val n=Native(stage);n.current=false;n.final=stage>=11;var path=""
   val w=KagemushaOrdinaryMintFundingV1(n,{}, {},KagemushaOrdinaryMintFundingOriginalTransportV1{path=it.path;response()},{})
   w.recoverRetained();assertEquals(17,n.phases.first());assertFalse(n.phases.contains(1));assertEquals(0,n.approvals);assertEquals(0,n.proofCalls)
   if(stage>=3)assertFalse(n.phases.contains(5));if(stage>=4)assertFalse(n.phases.contains(6))
   if(stage in listOf(5,6)){assertEquals(KagemushaOrdinaryMintFundingHttpCodecV1.RECOVER_PATH,path);assertTrue(n.phases.contains(14));assertFalse(n.phases.contains(7))}
   if(stage>=7)assertFalse(n.phases.contains(8));if(stage>=9)assertFalse(n.phases.contains(9));if(stage>=10)assertFalse(n.phases.contains(10))
   assertTrue(n.phases.contains(13));assertTrue(n.phases.contains(11))
  }
 }
 @Test fun unknownColdSignatureStagesStayClosedWithoutInvocation()=runBlocking {
  for(stage in listOf(2,8)){val n=Native(stage);val w=KagemushaOrdinaryMintFundingV1(n,{}, {},KagemushaOrdinaryMintFundingOriginalTransportV1{error("No Core issuance")},{})
   assertFails{w.recoverRetained()};assertTrue(n.closed);assertEquals(listOf(17),n.phases);assertEquals(0,n.approvals)}
 }
 @Test fun coldPlatformRecoveryUsesActualRetainedMessageAndRejectsOfferedAmount()=runBlocking {
  val n=Native(0);val w=KagemushaOrdinaryMintFundingV1(n,{}, {},KagemushaOrdinaryMintFundingOriginalTransportV1{response()},{})
  assertTrue(w.recoverRetained().pending());assertTrue(n.phases.contains(120));assertFalse(n.phases.contains(1));assertEquals(1,n.approvals)
  assertFails{w.beginOrResume(java.math.BigInteger.ONE)};assertEquals(1,n.approvals)
 }
}
