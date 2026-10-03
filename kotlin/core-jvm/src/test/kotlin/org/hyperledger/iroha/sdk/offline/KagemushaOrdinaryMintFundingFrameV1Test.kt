package org.hyperledger.iroha.sdk.offline
import org.junit.jupiter.api.Test
import kotlin.test.*
/** TEST ONLY fixed public bytes; no Native owner, proof or funding authority. */
class KagemushaOrdinaryMintFundingFrameV1Test {
 @Test fun soleSeventeenPhaseRequestGrammar(){val f=KagemushaOrdinaryMintFundingFrameV1
  for(p in 1..17){val r=when(p){1->listOf(ByteArray(16){1});3->listOf(byteArrayOf(1));8->List(5){byteArrayOf(1)};else->emptyList()};f.requireRequest(p,r)}
  for(p in listOf(0,18,255))assertFails{f.requireRequest(p,emptyList())}
  assertFails{f.requireRequest(1,listOf(ByteArray(16)))};assertFails{f.requireRequest(13,listOf(byteArrayOf(1)))}
 }
 @Test fun progressDataIsBoundedAndUnknownStageRefuses(){val f=KagemushaOrdinaryMintFundingFrameV1;f.requireResponse(17,listOf(ByteArray(32){1},byteArrayOf(12)));assertFails{f.requireResponse(17,listOf(ByteArray(32){1},byteArrayOf(13)))};assertFails{f.requireResponse(17,listOf(ByteArray(32),byteArrayOf(0)))}}
 @Test fun headerCannotChangeDescriptorOrPhase(){val f=KagemushaOrdinaryMintFundingFrameV1;val h=ByteArray(8){if(it==0)7 else 0}
  assertTrue(f.responseFields(13,7,listOf(byteArrayOf(1,0),byteArrayOf(13),h)).isEmpty())
  assertFails{f.responseFields(13,8,listOf(byteArrayOf(1,0),byteArrayOf(13),h))};assertFails{f.responseFields(4,7,listOf(byteArrayOf(1,0),byteArrayOf(13),h))}
 }
 @Test fun pendingFinalityAndPrivateFloorNeverCreateOriginal(){val f=KagemushaOrdinaryMintFundingFrameV1
  f.requireResponse(11,listOf(byteArrayOf(0),byteArrayOf()));assertFails{f.requireResponse(11,listOf(byteArrayOf(0),byteArrayOf(1)))}
  f.requireResponse(15,listOf(byteArrayOf(5),byteArrayOf()));assertFails{f.requireResponse(15,listOf(byteArrayOf(5),ByteArray(4)))}
  assertFails{f.requireResponse(12,listOf(byteArrayOf(1),ByteArray(32){1},byteArrayOf(1),byteArrayOf(1),byteArrayOf(1)))}
 }
 @Test fun completeNodePacketAndNoSurrogateResponse(){val f=KagemushaOrdinaryMintFundingFrameV1
  assertFails{f.requireRequest(8,List(4){byteArrayOf(1)})};f.requireRequest(8,List(5){byteArrayOf(1)})
  assertFails{f.requireResponse(7,List(8){byteArrayOf(1)})} // Ed consent must be complete64.
 }
}
