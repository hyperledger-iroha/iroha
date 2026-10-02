// Synthetic Native endpoint exercises managed custody only; it grants no release/hardware authority.
package org.hyperledger.iroha.sdk.offline
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest
import java.util.concurrent.CompletableFuture
import kotlin.test.*
import org.junit.jupiter.api.Test
import org.hyperledger.iroha.sdk.crypto.keystore.*

class KagemushaFirstDeviceHardwareEvidenceV1Test {
    private class Endpoint : KagemushaHardwareBootstrapNativeEndpointV1 {
        var receipt:ByteArray?=null;var receiptCalls=0;var current=true; var operation=ByteArray(32){1}; var pending:Int?=null;var complete=0;var cancel=false;var disposed=false
        override fun requireOriginalCustody(){check(current)}
        override fun requirePendingEffect(step:Int){requireOriginalCustody();check(pending==step && !cancel && !disposed)}
        override fun originalOperationId()=operation.copyOf()
        override fun googleOAuthClientId():String="synthetic-client.apps.googleusercontent.com"
        override fun authoritativeDeadlineMs()=120_001uL
        override fun completedStep()=complete
        override fun pendingStep()=pending
        override fun fencePrepare(originalGoogleIdToken:ByteArray):KagemushaHardwareBootstrapHttpOriginalV1 {error("pure test never invokes HTTP")}
        override fun acceptChallenge(original:ByteArray){error("not invoked")}
        override fun fenceKey():HardwareKeySelection=error("not invoked")
        override fun recoverKeySelection():HardwareKeySelection=error("not invoked")
        override fun captureKey(point:ByteArray,rawArchive:ByteArray){error("not invoked")}
        override fun fenceRawIssuer():KagemushaHardwareBootstrapHttpOriginalV1=error("not invoked")
        override fun acceptRawAdmission(original:ByteArray){error("not invoked")}
        override fun fencePossession():HardwarePossessionSelection=error("not invoked")
        override fun capturePossession(originalDer:ByteArray){error("not invoked")}
        override fun fenceIntegrity():HardwareIntegritySelection=error("not invoked")
        override fun captureIntegrityOriginal(opaqueOriginal:ByteArray){error("not invoked")}
        override fun fenceReceipt():KagemushaHardwareBootstrapHttpOriginalV1 {
            requireOriginalCustody();check(complete==5 && pending==null && !cancel);pending=6;receiptCalls++
            return KagemushaHardwareBootstrapHttpOriginalV1(KagemushaHardwareBootstrapHttpOriginalV1.Stage.RECEIPT,"https://synthetic.example",byteArrayOf(1),{requireOriginalCustody()},ByteArray(32){1})
        }
        override fun acceptHardwareReceipt(original:ByteArray){requireOriginalCustody();check(pending==6);receipt=original.copyOf();pending=null;complete=6}
        override fun originalReceipt():ByteArray?=receipt?.copyOf()
        override fun requestCancel(){check(!cancel);cancel=true}
        override fun disposeTerminal(){check(pending==null && (cancel || complete==6));disposed=true}
    }
    @Test fun sameStrongServiceSessionAcrossUiRecreation(){var factories=0;val e=Endpoint();val service=KagemushaFirstDeviceHardwareEvidenceServiceOwnerV1{factories++;e};val a=service.recoverOriginalOrReserve();assertSame(a,service.recoverOriginalOrReserve());a.requestCancel();assertSame(a,service.recoverOriginalOrReserve());assertEquals(1,factories)}
    @Test fun failedStartupReleasesOnlyTheUnpublishedNativeView() {
        val endpoint=Endpoint();endpoint.pending=5;endpoint.complete=4;endpoint.current=false
        var releases=0
        assertFailsWith<IllegalStateException> {
            KagemushaFirstDeviceHardwareEvidenceServiceOwnerV1.fromOwnedNativeView(endpoint) { releases++ }
        }
        assertEquals(1,releases)
        assertEquals(5,endpoint.pending);assertEquals(4,endpoint.complete)
        assertFalse(endpoint.cancel);assertFalse(endpoint.disposed)
    }
    @Test fun authenticatedStartupRetainsTheSameViewAndSession() {
        val endpoint=Endpoint();var releases=0
        val owner=KagemushaFirstDeviceHardwareEvidenceServiceOwnerV1.fromOwnedNativeView(endpoint) { releases++ }
        assertSame(owner.recoverOriginalOrReserve(),owner.recoverOriginalOrReserve())
        assertEquals(0,releases);assertFalse(endpoint.cancel);assertFalse(endpoint.disposed)
    }
    @Test fun cleanupFailureCannotReplaceTheStartupFailure() {
        val endpoint=Endpoint();endpoint.current=false
        val cleanup=IllegalArgumentException("synthetic view release failed")
        val failure=assertFailsWith<IllegalStateException> {
            KagemushaFirstDeviceHardwareEvidenceServiceOwnerV1.fromOwnedNativeView(endpoint) { throw cleanup }
        }
        assertEquals("Check failed.",failure.message)
        assertEquals(listOf(cleanup),failure.suppressed.toList())
        assertFalse(endpoint.cancel);assertFalse(endpoint.disposed)
    }
    @Test fun viewCancellationCannotCancelOriginalFuture(){val original=CompletableFuture<ByteArray>();val first=detachedOriginalView(original){it.copyOf()};assertTrue(first.cancel(true));assertFalse(original.isCancelled);val second=detachedOriginalView(original){it.copyOf()};original.complete(byteArrayOf(7));assertContentEquals(byteArrayOf(7),second.get())}
    @Test fun viewsCannotMutateOriginalBytes(){val raw=byteArrayOf(9);val original=CompletableFuture.completedFuture(raw);val a=detachedOriginalView(original){it.copyOf()}.get();a[0]=1;assertContentEquals(byteArrayOf(9),detachedOriginalView(original){it.copyOf()}.get());assertContentEquals(byteArrayOf(9),raw)}
    @Test fun exceptionViewsPreserveOriginalFailure(){val original=CompletableFuture<ByteArray>();val view=detachedOriginalView(original){it.copyOf()};val failure=IllegalStateException("synthetic");original.completeExceptionally(failure);try{view.get();fail()}catch(e:java.util.concurrent.ExecutionException){assertSame(failure,e.cause)}}
    @Test fun nativeDeadlineReadDoesNotDisposeOrCancel(){val e=Endpoint();val s=KagemushaFirstDeviceHardwareEvidenceSessionV1.fromNativeEndpoint(e);assertEquals(120_001uL,s.authoritativeDeadlineMs());assertFalse(e.cancel);assertFalse(e.disposed)}
    @Test fun unknownNativeInvocationPreventsDisposal(){val e=Endpoint();e.pending=5;val s=KagemushaFirstDeviceHardwareEvidenceSessionV1.fromNativeEndpoint(e);s.requestCancel();try{s.disposeTerminal();fail()}catch(_:IllegalStateException){};assertEquals(5,e.pending);assertFalse(e.disposed)}
    @Test fun changedNativeOperationRefusesDataReads(){val e=Endpoint();val s=KagemushaFirstDeviceHardwareEvidenceSessionV1.fromNativeEndpoint(e);e.operation[0]=2;try{s.authoritativeDeadlineMs();fail()}catch(_:IllegalStateException){}}
    @Test fun operationOriginalCannotBeChangedByCaller(){val e=Endpoint();val s=KagemushaFirstDeviceHardwareEvidenceSessionV1.fromNativeEndpoint(e);s.operationId()[0]=3;assertEquals(1,s.operationId()[0].toInt())}
    @Test fun noMoneyOrFinancialReservationApi(){val names=KagemushaFirstDeviceHardwareEvidenceSessionV1::class.java.methods.map{it.name};for(name in listOf("reserveOriginalIdentity","prepareApproval","send","mint","receive","publishOriginalInitialState")){assertFalse(names.contains(name),name)}}
    @Test fun coldCapturedPiPrefixFinishesOnlyOneReceiptOriginal(){val e=Endpoint();e.complete=5;val service=KagemushaFirstDeviceHardwareEvidenceServiceOwnerV1{e};val nativeHttp=CompletableFuture<ByteArray>();var transports=0;val transport=KagemushaHardwareBootstrapOriginalTransportV1{transports++;it.requireCurrent();nativeHttp};val a=service.recoverOriginalOrReserve().completeCapturedOriginalReceipt(transport);a.cancel(true);val b=service.recoverOriginalOrReserve().completeCapturedOriginalReceipt(transport);assertEquals(1,e.receiptCalls);assertEquals(1,transports);assertFalse(nativeHttp.isCancelled);nativeHttp.complete(byteArrayOf(7));assertContentEquals(byteArrayOf(7),b.get());assertEquals(6,e.complete)}
    @Test fun unknownReceiptCannotInvokeTransportAgain(){val e=Endpoint();e.complete=5;e.pending=6;val s=KagemushaFirstDeviceHardwareEvidenceSessionV1.fromNativeEndpoint(e);var calls=0;try{s.completeCapturedOriginalReceipt{calls++;CompletableFuture.completedFuture(byteArrayOf(1))};fail()}catch(_:IllegalStateException){};assertEquals(0,calls);assertEquals(0,e.receiptCalls)}
    @Test fun responseBoundOrOwnerLossCannotReachNativeIntake(){for(bound in listOf(true,false)){val e=Endpoint();e.complete=5;val s=KagemushaFirstDeviceHardwareEvidenceSessionV1.fromNativeEndpoint(e);val raw=CompletableFuture<ByteArray>();val result=s.completeCapturedOriginalReceipt{raw};if(!bound)e.current=false;raw.complete(if(bound)ByteArray(192*1024+1) else byteArrayOf(7));assertTrue(result.isCompletedExceptionally);assertNull(e.receipt);assertEquals(6,e.pending)}}
    @Test fun failedViewCopyTerminatesViewInsteadOfHanging(){val original=CompletableFuture.completedFuture(byteArrayOf(1));val view=detachedOriginalView(original){error("copy rejected")};assertTrue(view.isCompletedExceptionally)}
    @Test fun pendingOriginalKeyNeverRequestsReplacementGoogleIdentity() {
        val e=Endpoint();e.complete=1;e.pending=2
        val s=KagemushaFirstDeviceHardwareEvidenceSessionV1.fromNativeEndpoint(e)
        repeat(3){assertFalse(s.originalNeedsGoogleIdentity())}
        assertEquals(1,e.complete);assertEquals(2,e.pending)
        assertEquals(0,e.receiptCalls);assertFalse(e.cancel);assertFalse(e.disposed)
    }
    @Test fun otherUnknownOriginalEffectsStillRefuseGoogleIdentity() {
        for(step in listOf(1,3,4,5,6)) {
            val e=Endpoint();e.complete=step-1;e.pending=step
            val s=KagemushaFirstDeviceHardwareEvidenceSessionV1.fromNativeEndpoint(e)
            try{s.originalNeedsGoogleIdentity();fail("unknown original step $step accepted")}
            catch(_:IllegalStateException){}
            assertEquals(step,e.pending);assertEquals(step-1,e.complete)
            assertEquals(0,e.receiptCalls);assertFalse(e.cancel);assertFalse(e.disposed)
        }
    }
    @Test fun identitySelectionRequiresExactOriginalCapturedPrefix() {
        val fresh=Endpoint();assertTrue(KagemushaFirstDeviceHardwareEvidenceSessionV1.fromNativeEndpoint(fresh).originalNeedsGoogleIdentity())
        for(step in 1..5) {
            val e=Endpoint();e.complete=step
            assertFalse(KagemushaFirstDeviceHardwareEvidenceSessionV1.fromNativeEndpoint(e).originalNeedsGoogleIdentity())
        }
        val invalid=Endpoint();invalid.complete=0;invalid.pending=2
        try{KagemushaFirstDeviceHardwareEvidenceSessionV1.fromNativeEndpoint(invalid).originalNeedsGoogleIdentity();fail()}
        catch(_:IllegalStateException){}
        assertEquals(2,invalid.pending);assertEquals(0,invalid.complete)
    }
    private fun e():ByteArray {
        val domain="iroha:kagemusha:v1:hardware-evidence-possession\u0000".toByteArray(Charsets.US_ASCII)
        val body=ByteArray(308);body[0]=1;body[2]=1;repeat(7){field->java.util.Arrays.fill(body,3+field*32,35+field*32,(field+1).toByte())}
        // Real canonical P256 generator point, no synthetic software fallback key use.
        val point="046b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c2964fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5".chunked(2).map{it.toInt(16).toByte()}.toByteArray()
        point.copyInto(body,227);MessageDigest.getInstance("SHA-256").digest(point).copyInto(body,163)
        ByteBuffer.wrap(body).order(ByteOrder.LITTLE_ENDIAN).putLong(292,1).putLong(300,120001)
        return domain+ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(308).array()+body
    }
    @Test fun distinctHardwareEAcceptedOnlyByHardwarePurpose(){val b=e();requireAppPlatformSigningMessageV1(b,KagemushaAndroidAppSignaturePurposeV1.FIRST_DEVICE_HARDWARE_POSSESSION);for(p in KagemushaAndroidAppSignaturePurposeV1.entries.filter{it!=KagemushaAndroidAppSignaturePurposeV1.FIRST_DEVICE_HARDWARE_POSSESSION}){try{requireAppPlatformSigningMessageV1(b,p);fail()}catch(_:IllegalArgumentException){}}}
    @Test fun hardwareERejectsMoneyPurposeAndMissingSelector(){val b=e();val body=b.size-308;b[body+2]=2;try{requireAppPlatformSigningMessageV1(b,KagemushaAndroidAppSignaturePurposeV1.FIRST_DEVICE_HARDWARE_POSSESSION);fail()}catch(_:IllegalArgumentException){};b[body+2]=1;java.util.Arrays.fill(b,body+3,body+35,0.toByte());try{requireAppPlatformSigningMessageV1(b,KagemushaAndroidAppSignaturePurposeV1.FIRST_DEVICE_HARDWARE_POSSESSION);fail()}catch(_:IllegalArgumentException){}}
    @Test fun hardwareERejectsPointKeyHashAndIntervalSubstitution(){for(index in listOf(163,227,307)){val b=e();b[b.size-308+index]=(b[b.size-308+index].toInt() xor 1).toByte();try{requireAppPlatformSigningMessageV1(b,KagemushaAndroidAppSignaturePurposeV1.FIRST_DEVICE_HARDWARE_POSSESSION);fail("accepted mutation $index")}catch(_:IllegalArgumentException){}}}
}
