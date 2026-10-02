package org.hyperledger.iroha.sdk.crypto.keystore
import java.util.concurrent.CompletableFuture
import org.junit.Assert.*
import org.junit.Test
import org.hyperledger.iroha.sdk.offline.detachedOriginalView
/** Real shared provider against a synthetic Google future; no decoded verdict/device claim. */
class KagemushaFirstDeviceHardwareIntegrityV1Test {
    private class Backend:KagemushaPlayIntegrityBackendV1 {
        var preparations=0;var requests=0;val warm=CompletableFuture<KagemushaPlayIntegrityPreparedV1>();val token=CompletableFuture<String>()
        override fun prepare(cloudProjectNumber:Long):CompletableFuture<KagemushaPlayIntegrityPreparedV1>{preparations++;return warm}
        fun prepared(){warm.complete(object:KagemushaPlayIntegrityPreparedV1{override fun request(originalHashText:String):CompletableFuture<String>{requests++;assertEquals(43,originalHashText.length);return token}})}
    }
    @Test fun beforeInvocationRefusalDoesNotReachGoogle(){val b=Backend();val p=KagemushaAndroidPlayIntegrityProviderV1(b);val f=p.requestHardwareBootstrapOriginal(1,ByteArray(32){1},{error("Native refused")},{});assertTrue(f.isCompletedExceptionally);assertEquals(0,b.preparations);assertEquals(0,b.requests)}
    @Test fun cancellationDuringWarmupCannotStartGoogleRequest(){val b=Backend();val p=KagemushaAndroidPlayIntegrityProviderV1(b);var valid=true;val f=p.requestHardwareBootstrapOriginal(1,ByteArray(32){1},{check(valid)},{});valid=false;b.prepared();assertTrue(f.isCompletedExceptionally);assertEquals(0,b.requests)}
    @Test fun deadlineAfterOriginalInvocationStillRetainsOriginalToken(){val b=Backend();val p=KagemushaAndroidPlayIntegrityProviderV1(b);var valid=true;var captures=0;val f=p.requestHardwareBootstrapOriginal(1,ByteArray(32){2},{check(valid)},{captures++});b.prepared();valid=false;b.token.complete("actual-original-synthetic-token");assertEquals("actual-original-synthetic-token",f.get().opaqueToken());assertEquals(1,b.requests);assertEquals(2,captures)}
    @Test fun cancellingUiViewPreservesOriginalGoogleAndCapture(){val b=Backend();val p=KagemushaAndroidPlayIntegrityProviderV1(b);val f=p.requestHardwareBootstrapOriginal(1,ByteArray(32){3},{},{});val view=detachedOriginalView(f){it};view.cancel(true);b.prepared();assertEquals(1,b.requests);assertFalse(f.isCancelled);assertFalse(b.token.isCancelled);b.token.complete("retained-original");assertEquals("retained-original",f.get().opaqueToken())}
    @Test fun lostNativeCustodyAfterGoogleDoesNotReturnOriginal(){val b=Backend();val p=KagemushaAndroidPlayIntegrityProviderV1(b);var owned=true;val f=p.requestHardwareBootstrapOriginal(1,ByteArray(32){4},{check(owned)},{check(owned)});b.prepared();owned=false;b.token.complete("retained-original");assertTrue(f.isCompletedExceptionally);assertEquals(1,b.requests)}
    @Test fun genericExistingFinancialProviderRetainsItsStrictGuard(){val b=Backend();val p=KagemushaAndroidPlayIntegrityProviderV1(b);var valid=true;val f=p.requestOriginal(1,ByteArray(32){5}){check(valid)};b.prepared();valid=false;b.token.complete("retained-original");assertTrue(f.isCompletedExceptionally);assertEquals(1,b.requests)}
}
