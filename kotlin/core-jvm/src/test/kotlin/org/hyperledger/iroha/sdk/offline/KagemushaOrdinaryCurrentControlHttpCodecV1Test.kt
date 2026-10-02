package org.hyperledger.iroha.sdk.offline

import java.util.Base64
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.junit.jupiter.api.Test
import kotlin.test.*

/** Carrier tests only; inert originals establish neither a signature nor FI admission. */
class KagemushaOrdinaryCurrentControlHttpCodecV1Test {
    @Test fun nativeOriginalBodyAndStableRequestCorrelation() {
        val request=byteArrayOf(1,2,3);val signature=ByteArray(64) { 4 }
        val body=KagemushaOrdinaryCurrentControlHttpCodecV1.requestBody(request,signature)
        @Suppress("UNCHECKED_CAST") val parsed=JsonParser.parse(body.toString(Charsets.UTF_8)) as Map<String,Any?>
        assertEquals(setOf("schema","canonical_request_base64","account_signature_base64"),parsed.keys)
        assertEquals("iroha.kagemusha.ordinary-current-fi-control-request.v1",parsed["schema"])
        assertEquals(base64(request),parsed["canonical_request_base64"]);assertEquals(base64(signature),parsed["account_signature_base64"])
        val id=KagemushaOrdinaryCurrentControlHttpCodecV1.requestId(request)
        assertEquals(id,java.util.UUID.fromString(id).toString())
        assertEquals(id,KagemushaOrdinaryCurrentControlHttpCodecV1.requestId(request.copyOf()))
        assertNotEquals(id,KagemushaOrdinaryCurrentControlHttpCodecV1.requestId(byteArrayOf(1,2,4)))
        for (width in listOf(0,63,65)) assertFails { KagemushaOrdinaryCurrentControlHttpCodecV1.requestBody(request,ByteArray(width)) }
        assertFails { KagemushaOrdinaryCurrentControlHttpCodecV1.requestBody(ByteArray(8193),signature) }
    }
    @Test fun fullCertifiedWorldUsesItsDistinctResponseBoundAndExactOriginals() {
        val world=ByteArray(300*1024) { (it % 251).toByte() };val signed=byteArrayOf(2,3)
        val fields=KagemushaOrdinaryCurrentControlHttpCodecV1.responseOriginals(reply(base64(signed),base64(world)))
        assertContentEquals(signed,fields[0]);assertContentEquals(world,fields[1])
        assertEquals(128*1024*1024,KagemushaOrdinaryCurrentControlHttpCodecV1.MAXIMUM_AUTHORITY_BYTES)
        assertTrue(KagemushaOrdinaryCurrentControlHttpCodecV1.MAXIMUM_RESPONSE_BYTES > 128*1024*1024)
    }
    @Test fun unknownDuplicateMalformedAndNoncanonicalOriginalEncodingsRefused() {
        val good=reply("AQ==","Ag==").toString(Charsets.UTF_8)
        for (raw in listOf(
            good.dropLast(1)+",\"status\":\"approved\"}",
            good.dropLast(1)+",\"authority_original_base64\":\"Aw==\"}",
            good.replace("AQ==","AQ"),good.replace("AQ==","AR=="),
            good.replace("Ag==",""),good.replace("\"Ag==\"","null"),
            "{\"signed_control_original_base64\":\"AQ==\"}",
        )) assertFails { KagemushaOrdinaryCurrentControlHttpCodecV1.responseOriginals(raw.toByteArray(Charsets.UTF_8)) }
        assertFails { KagemushaOrdinaryCurrentControlHttpCodecV1.responseOriginals(byteArrayOf(0xff.toByte())) }
        assertFails { KagemushaOrdinaryCurrentControlHttpCodecV1.responseOriginals(reply(base64(ByteArray(64*1024+1)),"Ag==")) }
    }
    private fun reply(signed:String,world:String)=JsonEncoder.encode(linkedMapOf(
        "signed_control_original_base64" to signed,"authority_original_base64" to world)).toByteArray(Charsets.UTF_8)
    private fun base64(raw:ByteArray)=Base64.getEncoder().encodeToString(raw)
}
