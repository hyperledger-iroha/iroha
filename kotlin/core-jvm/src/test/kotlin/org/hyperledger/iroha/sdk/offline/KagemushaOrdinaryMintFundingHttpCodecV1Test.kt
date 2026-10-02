package org.hyperledger.iroha.sdk.offline
import org.junit.jupiter.api.Test
import kotlin.test.*
import java.io.ByteArrayOutputStream
/** TEST ONLY data/carrier bytes. No decoder or fixture grants Native authority. */
class KagemushaOrdinaryMintFundingHttpCodecV1Test {
 private fun fields()=List(8){n->if(n==2)ByteArray(64){3}else if(n==4)ByteArray(0)else byteArrayOf((n+1).toByte())}
 @Test fun eightWholeOriginalsAndExplicitAbsentPi(){val c=KagemushaOrdinaryMintFundingHttpCodecV1;val out=ByteArrayOutputStream();c.writeRequest(fields(),out);val s=out.toString("UTF-8")
  assertTrue(s.contains("\"preparation_integrity_original_base64\":null"));assertTrue(s.contains("\"current_financial_control_original_base64\":\"CA==\""))
  assertEquals(106503516,c.MAXIMUM_RESPONSE_BYTES);assertTrue(out.size()<c.MAXIMUM_REQUEST_BYTES)
 }
 @Test fun requestIdPinsEveryFullOriginal(){val c=KagemushaOrdinaryMintFundingHttpCodecV1;val f=fields();val id=c.requestId(f)
  assertEquals(id,c.requestId(f.map(ByteArray::copyOf)));f.indices.forEach{n->val changed=f.map(ByteArray::copyOf).toMutableList();changed[n]=if(n==4)byteArrayOf(1)else changed[n].also{it[0]=99};assertNotEquals(id,c.requestId(changed))}}
 @Test fun exactFiveResponseOriginalsAndCanonicalBase64(){val c=KagemushaOrdinaryMintFundingHttpCodecV1
  val names=listOf("signed_decision_original_base64","decision_signed_clock_original_base64","decision_financial_control_original_base64","reserved_data_record_original_base64","node_submission_original_base64")
  val body="{"+names.joinToString(","){"\"$it\":\"AQ==\""}+"}";assertEquals(5,c.responseOriginals(body.toByteArray()).size)
  for(raw in listOf(body.replace("AQ==","AQ"),body.dropLast(1)+",\"extra\":1}",body.replace("\"AQ==\"","null"),body.replace(names[4],"retired_packet")))assertFails{c.responseOriginals(raw.toByteArray())}
 }
}
