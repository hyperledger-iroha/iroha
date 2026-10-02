package org.hyperledger.iroha.sdk.offline
import org.junit.jupiter.api.Test
import kotlin.test.*
/** TEST ONLY discovery/descriptor specimens. They create no Native or hardware authority. */
class KagemushaOrdinaryMintFundingTransportBindingV1Test {
 private class Offered:KagemushaOrdinaryMintFundingNativeOwnerV1 {
  var calls=0
  override fun fundingEndpoint():KagemushaOrdinaryNativeMintFundingEndpointV1{calls++;error("Must not invoke offered SPI")}
  override fun completedMetadataEndpoint():KagemushaOrdinaryNativeIntegrityRefreshEndpointV1{calls++;error("Must not invoke offered SPI")}
 }
 @Test fun offeredProviderRefusesBeforeEitherEndpointMethod() {
  val offered=Offered();assertFails{originalOrdinaryMintFundingNativeV1(listOf(offered).iterator())};assertEquals(0,offered.calls)
 }
 @Test fun missingOrAmbiguousProviderCannotSelectAnotherRoute() {
  assertFails{originalOrdinaryMintFundingNativeV1(emptyList<KagemushaOrdinaryMintFundingNativeOwnerV1>().iterator())}
  val a=Offered();val b=Offered();assertFails{originalOrdinaryMintFundingNativeV1(listOf(a,b).iterator())}
  assertEquals(0,a.calls);assertEquals(0,b.calls)
 }
 @Test fun retiredProductGuardCannotAcquirePreparationOrRecoveryHolder() {
  var closed=0;val endpoint=object:KagemushaCoreCoordinatorEndpointV1 {
   override fun contract()=intArrayOf(2,25,3,6,54,8,7,22,16,0xffff,1,21)
   override fun install(storagePath:String)=0;override fun open(storagePath:String)=1L
   override fun close(handle:Long):Int{closed++;return 0}
   override fun invoke(handle:Long,method:Int,fields:Array<ByteArray>):Array<ByteArray>?=error("No Native authority")
  }
  val bridge=KagemushaCoreCoordinatorBridgeV1.openEndpoint("/test/untrusted-funding-module",endpoint)
  val binding=KagemushaOrdinaryMintFundingTransportBindingV1(bridge){error("Original owner retired")}
  assertFails{binding.prepareNative(ByteArray(16){1})};assertFails{binding.recoverNative()};assertEquals(1,closed)
 }
}
