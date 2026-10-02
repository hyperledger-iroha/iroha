package org.hyperledger.iroha.sdk.offline

import java.util.Base64
import java.security.MessageDigest
import kotlin.coroutines.*
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.junit.jupiter.api.Test
import kotlin.test.*

/** Scripted actual managed composition only; inert originals never become Native authority. */
class KagemushaOrdinaryCurrentControlV1Test {
    @Test fun canonicalJniOwnerHasExactFinalClassInClientDefiningLoaderWithoutNativeDispatch() {
        val ownerClass = KagemushaOrdinaryRuntimeJniV1.javaClass
        assertEquals("org.hyperledger.iroha.sdk.offline.KagemushaOrdinaryRuntimeJniV1", ownerClass.name)
        assertSame(KagemushaCoreCoordinatorBridgeV1::class.java.classLoader, ownerClass.classLoader)
        // JVM/Java class modifier FINAL is 0x0010; no reflective member lookup or construction.
        assertTrue((ownerClass.modifiers and 0x0010) != 0)
    }

    @Test fun actualFreshNativeSessionThenSoleNativeSignedRequestAndFullOriginalIntake() {
        val f=Fixture();var http=0
        run { f.holder { request ->
            http++;request.requireCurrent()
            assertEquals("/v1/kagemusha/enrollment/ordinary/current-control",request.path)
            assertTrue(request.maximumResponseBytes > 128*1024*1024)
            val body=request.body();request.body().fill(0);assertContentEquals(body,request.body())
            @Suppress("UNCHECKED_CAST") val objectFields=JsonParser.parse(body.toString(Charsets.UTF_8)) as Map<String,Any?>
            assertEquals(base64(f.request),objectFields["canonical_request_base64"])
            assertEquals(base64(f.signature),objectFields["account_signature_base64"])
            assertEquals(0,f.intakes)
            reply()
        }.refreshCurrentFinancialControl() }
        assertEquals(listOf(1,6),f.startupPhases)
        assertEquals(listOf(1,2,3),f.controlPhases)
        assertEquals(1,http);assertEquals(1,f.intakes)
        assertContentEquals(byteArrayOf(7),f.actualSigned);assertContentEquals(byteArrayOf(8),f.actualWorld)
    }
    @Test fun nativeSessionRefusalPreventsSignatureOrHttpAndNeverUsesBootstrap() {
        val f=Fixture().apply { badStartup=true }
        assertFails { run { f.holder { error("No HTTP without genuine refreshed session") }.refreshCurrentFinancialControl() } }
        assertTrue(f.controlPhases.isEmpty());assertEquals(listOf(1,6),f.startupPhases);assertEquals(0,f.intakes)
    }
    @Test fun substitutedNativeRequestOrRevokedOwnerCannotReachIntake() {
        val f=Fixture().apply { changedSigned=true }
        assertFails { run { f.holder { error("No HTTP with changed signing original") }.refreshCurrentFinancialControl() } }
        assertEquals(listOf(1,2),f.controlPhases);assertEquals(0,f.intakes)
        val revoked=Fixture()
        assertFails { run { revoked.holder { revoked.revoked=true;reply() }.refreshCurrentFinancialControl() } }
        assertEquals(0,revoked.intakes)
    }
    @Test fun malformedResponseFreezesWithoutFreshReadAndConcurrentRefreshCannotLendGrant() {
        val f=Fixture()
        val holder=f.holder { "{\"status\":\"approved\"}".toByteArray() }
        repeat(2) { assertFails { run { holder.refreshCurrentFinancialControl() } } }
        assertEquals(listOf(1,6),f.startupPhases);assertEquals(listOf(1,2),f.controlPhases);assertEquals(0,f.intakes);assertEquals(1,f.closes)
        val concurrent = Fixture()
        lateinit var same:KagemushaOrdinaryCurrentControlV1
        same=concurrent.holder {
            assertFailsWith<IllegalStateException> { run { same.refreshCurrentFinancialControl() } }
            reply()
        }
        run { same.refreshCurrentFinancialControl() };assertEquals(1,concurrent.intakes)
    }
    @Test fun httpOnlyRetryRetainsTheSameWalletRefreshFiRequestNativeSignatureAndUuid() {
        val f=Fixture();var attempts=0
        val bodies=mutableListOf<ByteArray>();val ids=mutableListOf<String>()
        var retained:KagemushaOrdinaryCurrentControlHttpOriginalV1?=null
        val holder=f.holder { request ->
            retained=request;bodies+=request.body();ids+=request.requestId
            if(++attempts==1)error("HTTP result lost without Native FI intake")
            reply()
        }
        assertFails { run { holder.beginOrResumeCurrentFinancialControl() } }
        // Explicit refresh cannot abandon/resign an unfinished original to repair HTTP expiry.
        assertFails { run { holder.refreshCurrentFinancialControl() } }
        var acknowledged:KagemushaOrdinaryCurrentControlOriginalsV1?=null
        run { acknowledged=holder.beginOrResumeCurrentFinancialControl() }
        val ack=checkNotNull(acknowledged)
        assertEquals(listOf(1,6),f.startupPhases);assertEquals(listOf(1,2,3),f.controlPhases)
        assertEquals(ids[0],ids[1]);assertContentEquals(bodies[0],bodies[1]);assertEquals(2,attempts)
        assertContentEquals(sha(f.request),ack.requestOriginalDigest())
        assertContentEquals(sha(byteArrayOf(7)),ack.signedControlOriginalDigest())
        assertContentEquals(sha(byteArrayOf(8)),ack.authorityOriginalDigest())
        ack.authorityOriginalDigest().fill(0)
        run { assertContentEquals(sha(byteArrayOf(8)),holder.beginOrResumeCurrentFinancialControl().authorityOriginalDigest()) }
        assertEquals(2,attempts);assertEquals(listOf(1,2,3),f.controlPhases)
        assertFails { checkNotNull(retained).body() };assertEquals(0,f.closes)
    }
    @Test fun explicitFreshReadAfterCompletionRepeatsGenuineStartupButDuplicateAckDoesNot() {
        val f=Fixture();var http=0
        val holder=f.holder { http++;reply() }
        run { holder.beginOrResumeCurrentFinancialControl() }
        run { holder.beginOrResumeCurrentFinancialControl() }
        assertEquals(1,http);assertEquals(listOf(1,6),f.startupPhases)
        run { holder.refreshCurrentFinancialControl() }
        assertEquals(2,http);assertEquals(listOf(1,6,1,6),f.startupPhases)
        assertEquals(listOf(1,2,3,1,2,3),f.controlPhases)
        // Fixture fields are inert: only actual Native would select/verify a new unique nonce.
    }
    @Test fun lostStartupAndEachCashPhaseFreezeWithoutColdFreshFallbackOrReopening() {
        for(phase in listOf(1,6)) {
            val f=Fixture().apply { loseStartupPhase=phase }
            val holder=f.holder { error("No HTTP after uncertain Native wallet refresh") }
            assertFails { run { holder.beginOrResumeCurrentFinancialControl() } }
            val before=f.startupPhases.toList();f.loseStartupPhase=null
            assertFails { run { holder.beginOrResumeCurrentFinancialControl() } }
            assertEquals(before,f.startupPhases);assertTrue(f.controlPhases.isEmpty());assertEquals(1,f.closes)
        }
        for(phase in 1..3) {
            val f=Fixture().apply { loseControlPhase=phase };val holder=f.holder { reply() }
            assertFails { run { holder.beginOrResumeCurrentFinancialControl() } }
            val before=f.controlPhases.toList();f.loseControlPhase=null
            assertFails { run { holder.beginOrResumeCurrentFinancialControl() } }
            assertEquals(before,f.controlPhases);assertEquals(listOf(1,6),f.startupPhases);assertEquals(1,f.closes)
        }
    }
    @Test fun duplicateOrConflictingFullResponseCannotBeReplacedByAnotherHttpReturn() {
        val raws=listOf("{}",reply().toString(Charsets.UTF_8).dropLast(1)+",\"authority_original_base64\":\"CQ==\"}",
            reply().toString(Charsets.UTF_8).replace("Bw==","Bw"))
        for(raw in raws) {
            val f=Fixture();var http=0;var offered=raw.toByteArray()
            val holder=f.holder { http++;offered }
            assertFails { run { holder.beginOrResumeCurrentFinancialControl() } }
            offered=reply()
            assertFails { run { holder.beginOrResumeCurrentFinancialControl() } }
            assertEquals(1,http);assertEquals(0,f.intakes);assertEquals(1,f.closes)
        }
    }
    @Test fun ownerChangedAfterNativeIntakeWithholdsAckAndCannotReplayThatIntake() {
        val f=Fixture().apply { afterIntake={revoked=true} };val holder=f.holder { reply() }
        assertFails { run { holder.beginOrResumeCurrentFinancialControl() } }
        assertEquals(1,f.intakes);assertEquals(1,f.closes)
        f.revoked=false
        assertFails { run { holder.beginOrResumeCurrentFinancialControl() } }
        assertEquals(1,f.intakes);assertEquals(listOf(1,2,3),f.controlPhases)
    }

    private class Fixture : KagemushaOrdinaryRuntimeCurrentControlEndpointV1 {
        val request=byteArrayOf(1,2,3);val signature=ByteArray(64) { 4 }
        val startupPhases=mutableListOf<Int>();val controlPhases=mutableListOf<Int>()
        var badStartup=false;var changedSigned=false;var revoked=false;var intakes=0;var closes=0;var closed=false
        var loseControlPhase:Int?=null;var loseStartupPhase:Int?=null;var afterIntake:(()->Unit)?=null
        var actualSigned=ByteArray(0);var actualWorld=ByteArray(0)
        override fun startup(phase:Int,readId:Long,original:ByteArray):Array<ByteArray>? {
            check(!closed);startupPhases+=phase;assertTrue(original.isEmpty());if(loseStartupPhase==phase)return null
            return when(phase) {
                1 -> arrayOf(byteArrayOf(1,0),byteArrayOf(1),le(17),ByteArray(32) { 2 },byteArrayOf(3),byteArrayOf(4))
                6 -> { assertEquals(17L,readId);if(badStartup)null else arrayOf(byteArrayOf(1,0),byteArrayOf(6),le(19)) }
                else -> error("No other startup phase")
            }
        }
        override fun invoke(phase:Int,coreHandle:Long,signedOriginal:ByteArray,authorityOriginal:ByteArray)=error("Test callback retains data only")
        fun holder(http:suspend(KagemushaOrdinaryCurrentControlHttpOriginalV1)->ByteArray)=KagemushaOrdinaryCurrentControlV1(
            { phase,signed,world ->
                check(!revoked && !closed);controlPhases+=phase;check(loseControlPhase!=phase) { "Lost original Native dispatch return" }
                when(phase) {
                    1 -> listOf(request.copyOf(), "iroha:kagemusha:v1:ordinary-current-fi-control-request\u0000".toByteArray(Charsets.US_ASCII)+request)
                    2 -> listOf(if(changedSigned)byteArrayOf(9) else request.copyOf(),signature.copyOf())
                    3 -> { actualSigned=signed.copyOf();actualWorld=world.copyOf();intakes++;afterIntake?.invoke();emptyList() }
                    else -> error("No monetary or Bootstrap phase")
                }
            },{check(!revoked && !closed)},{if(!closed){closed=true;closes++}},this,KagemushaOrdinaryCurrentControlOriginalTransportV1 { original -> http(original) },{check(!revoked)})
    }
    private fun run(block:suspend()->Unit) {
        var outcome:Result<Unit>?=null
        block.startCoroutine(object:Continuation<Unit> {
            override val context=EmptyCoroutineContext
            override fun resumeWith(result:Result<Unit>) { outcome=result }
        })
        checkNotNull(outcome) { "Fixture unexpectedly suspended" }.getOrThrow()
    }
    companion object {
        private fun sha(raw:ByteArray)=MessageDigest.getInstance("SHA-256").digest(raw)
        private fun le(value:Long)=ByteArray(8) { (value ushr(it*8)).toByte() }
        private fun base64(raw:ByteArray)=Base64.getEncoder().encodeToString(raw)
        private fun reply()=JsonEncoder.encode(linkedMapOf("signed_control_original_base64" to "Bw==",
            "authority_original_base64" to "CA==")).toByteArray(Charsets.UTF_8)
    }
}
