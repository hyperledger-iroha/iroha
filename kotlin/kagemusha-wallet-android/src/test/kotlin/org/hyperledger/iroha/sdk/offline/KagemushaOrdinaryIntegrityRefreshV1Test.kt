// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline
import org.junit.jupiter.api.Test
import kotlin.test.*
import kotlinx.coroutines.*
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.security.MessageDigest
/** Inert lifecycle/Google controls only. No Native issuer/platform/proof capability. */
class KagemushaOrdinaryIntegrityRefreshV1Test {
 private companion object {
  fun challengeFixture():ByteArray=ByteArray(514).also{
   it[0]=1;repeat(13){n->it.fill((n+1).toByte(),2+n*32,34+n*32)}
   java.nio.ByteBuffer.wrap(it).order(java.nio.ByteOrder.LITTLE_ENDIAN).putLong(418,1).putLong(426,1).putLong(434,1000).putLong(442,11000)
   it.fill(64,450,514)
  }
  fun prepareResponseFixture():ByteArray {
   val c=KagemushaPlayIntegrityRefreshPreparationV1.parseOriginal(challengeFixture())
   fun b(raw:ByteArray)=java.util.Base64.getEncoder().encodeToString(raw)
   return org.hyperledger.iroha.sdk.client.JsonEncoder.encode(linkedMapOf("operation_id" to c.operationId().joinToString(""){"%02x".format(it.toInt() and 255)},
    "signed_refresh_challenge_base64" to b(c.transportBytes()),"possession_signing_message_base64" to b(c.possessionSigningBytes()),
    "play_integrity_request_hash_base64" to b(c.playIntegrityRequestHash()),"expires_at_ms" to 11000)).toByteArray()
  }
 }
 private class Native:OrdinaryIntegrityWorkflowNativeV1 {
  val calls=mutableListOf<Int>();var retired=false;var prepared=false;var captured=false;var fail=0
  var signatures=0;var requests=0;var token:ByteArray?=null
  val future=CompletableFuture<String>();val c=challengeFixture();val nonce=ByteArray(32){12};val der=ByteArray(8){1}
  override suspend fun refreshAccount(){calls+=16;check(!retired)}
  override fun requireOpen(){check(!retired)};override fun revoke(){retired=true}
  override suspend fun invoke(phase:Int,original:ByteArray):List<ByteArray>{
   calls+=phase;check(phase!=fail);return when(phase){
    1->listOf(ByteArray(32){1},nonce)
    8->listOf(byteArrayOf(1),byteArrayOf(2),if(prepared)nonce else ByteArray(0),byteArrayOf(if(captured)3 else 0),
     if(prepared)c else ByteArray(0),if(captured)der else ByteArray(0),token ?:ByteArray(0),ByteArray(0))
    10->listOf(ByteArray(32){1},"alias".toByteArray(),ByteArray(32){2},ByteArray(65){if(it==0)4 else 1},ByteArray(32){3},byteArrayOf(3),byteArrayOf(1),byteArrayOf(2),byteArrayOf(3),ByteArray(8){1},ByteArray(32){4})
    5->{token=original.copyOf();listOf(KagemushaPlayIntegrityRefreshPreparationV1.parseOriginal(c).operationId(),c,der,original)}
    6->listOf(ByteArray(32){6});7->{prepared=false;captured=false;token=null;emptyList()};9->listOf(byteArrayOf(1));else->error("unknown")}
  }
  private fun step()=object:OrdinaryIntegrityPreparedStepV1 {
   override fun original()=c;override fun requestHash()=KagemushaPlayIntegrityRefreshPreparationV1.parseOriginal(c).playIntegrityRequestHash()
   override suspend fun approve():ByteArray {if(!captured){signatures++;captured=true};return der}
   override fun requestToken():CompletableFuture<String>{requests++;return future}
  }
  override suspend fun acceptChallenge(raw:ByteArray):OrdinaryIntegrityPreparedStepV1{calls+=2;check(raw.contentEquals(c));prepared=true;return step()}
  override suspend fun recoverPrepared():OrdinaryIntegrityPreparedStepV1{check(prepared);return step()}
 }
 private val finishResponse=run{val hash=MessageDigest.getInstance("SHA-256").digest(byteArrayOf(1)).joinToString(""){"%02x".format(it.toInt() and 255)}
  "{\"lease_base64\":\"AQ==\",\"lease_sha256_hex\":\"$hash\"}".toByteArray()}
 private fun response(o:KagemushaOrdinaryIntegrityHttpOriginalV1)=if(o.path.endsWith("/prepare"))prepareResponseFixture()else finishResponse
 @Test fun exactSequenceSeparatesNativeRenewalHardwareGoogleAndLease()=runBlocking {
  val n=Native().also{it.future.complete("opaque-original")};val flow=KagemushaOrdinaryIntegrityRefreshV1(n,{response(it)},{})
  assertContentEquals(ByteArray(32){6},flow.beginOrResumeRefresh().leaseDigest())
  assertEquals(listOf(16,8,1,2,5,6),n.calls);assertEquals(1,n.signatures);assertEquals(1,n.requests)
  val count=n.calls.size;flow.beginOrResumeRefresh();assertEquals(count,n.calls.size)
 }
 @Test fun finishHttpUncertaintyKeepsSameCarrierNonceDerAndGoogleFuture()=runBlocking {
  val n=Native().also{it.future.complete("opaque-original")};val originals=mutableListOf<KagemushaOrdinaryIntegrityHttpOriginalV1>();var failed=false
  val flow=KagemushaOrdinaryIntegrityRefreshV1(n,{o->if(o.path.endsWith("/finish")){originals+=o;if(!failed){failed=true;error("HTTP lost")}};response(o)},{})
  assertFails{flow.beginOrResumeRefresh()};flow.beginOrResumeRefresh()
  assertTrue(originals[0]===originals[1]);assertEquals(1,n.calls.count{it==1});assertEquals(1,n.calls.count{it==5})
  assertEquals(1,n.signatures);assertEquals(1,n.requests)
 }
 @Test fun coldPendingReusesCapturedSignatureAndTokenWithoutNewInvocation()=runBlocking {
  val n=Native().also{it.prepared=true;it.captured=true;it.token="old-token".toByteArray()}
  val flow=KagemushaOrdinaryIntegrityRefreshV1(n,{o->assertTrue(o.path.endsWith("/finish"));response(o)},{})
  flow.beginOrResumeRefresh();assertEquals(0,n.signatures);assertEquals(0,n.requests);assertFalse(1 in n.calls);assertTrue(10 in n.calls)
 }
 @Test fun cancelledUiDoesNotCancelTheHeldGoogleFutureOrStartAnother()=runBlocking {
  val n=Native();val flow=KagemushaOrdinaryIntegrityRefreshV1(n,{response(it)},{})
  val task=launch{flow.beginOrResumeRefresh()};yield();assertEquals(1,n.requests);task.cancel()
  assertFalse(n.future.isCancelled);n.future.complete("opaque-original");task.join()
  flow.beginOrResumeRefresh();assertEquals(1,n.requests);assertEquals(1,n.signatures)
 }
 @Test fun unknownNativeOutcomeCannotReplaceNonceOrRepeatHardware()=runBlocking {
  val n=Native().also{it.future.complete("opaque-original");it.fail=5};val flow=KagemushaOrdinaryIntegrityRefreshV1(n,{response(it)},{})
  assertFails{flow.beginOrResumeRefresh()};assertTrue(n.retired);val count=n.calls.size
  assertFails{flow.beginOrResumeRefresh()};assertEquals(count,n.calls.size)
 }
 @Test fun failedGoogleFutureIsRetainedUntilExplicitAbandonment()=runBlocking {
  val n=Native().also{it.future.completeExceptionally(IllegalStateException("Google outcome"))}
  val flow=KagemushaOrdinaryIntegrityRefreshV1(n,{response(it)},{})
  assertFails{flow.beginOrResumeRefresh()};assertFails{flow.beginOrResumeRefresh()}
  assertEquals(1,n.requests);assertFalse(n.retired);flow.abandonRetainedAttempt();assertTrue(7 in n.calls)
 }
 @Test fun workflowOwnedNativeIoKeepsActualResultAfterCancellation()=runBlocking {
  val io=KagemushaRetainedNativeIoV1();val entered=CountDownLatch(1);val release=CountDownLatch(1);var retained=0
  val task=launch{retained=io.call{entered.countDown();check(release.await(5,TimeUnit.SECONDS));7}}
  assertTrue(withContext(Dispatchers.IO){entered.await(5,TimeUnit.SECONDS)});task.cancel();release.countDown();task.join()
  assertEquals(7,retained);io.retire()
 }
}
