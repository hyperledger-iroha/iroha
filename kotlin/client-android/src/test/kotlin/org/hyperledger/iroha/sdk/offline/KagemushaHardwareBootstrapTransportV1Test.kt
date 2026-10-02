package org.hyperledger.iroha.sdk.offline

import java.io.ByteArrayInputStream
import java.io.InputStream
import java.util.Base64
import org.junit.Assert.*
import org.junit.Test
import org.hyperledger.iroha.sdk.client.JsonParser

/** Synthetic byte originals exercise data-only framing; they are never issuer/owner authority. */
class KagemushaHardwareBootstrapTransportV1Test {
    private val codec=KagemushaHardwareBootstrapHttpCodecV1
    private fun fails(block:()->Unit) { try { block();fail("accepted invalid original") } catch(_:IllegalArgumentException){}
        catch(_:IllegalStateException){} }
    private fun carrier(stage:KagemushaHardwareBootstrapHttpOriginalV1.Stage, guard:()->Unit={}):KagemushaHardwareBootstrapHttpOriginalV1 =
        KagemushaHardwareBootstrapHttpOriginalV1(stage,"https://synthetic.example",byteArrayOf(1),guard,ByteArray(32){it.toByte()})
    @Test fun exactRoutesAndOperation() {
        val stages=KagemushaHardwareBootstrapHttpOriginalV1.Stage.entries
        val names=listOf("prepare","raw-attestation","finish")
        stages.forEachIndexed { i,s -> val value=carrier(s)
            assertEquals("/v1/kagemusha/hardware-evidence/first-device/"+names[i],value.path)
            assertEquals((0..31).joinToString(""){"%02x".format(it)},value.requestId)
        }
    }
    @Test fun strictOriginNoRedirectTargetOrCredentials() {
        codec.requireOrigin("https://synthetic.example:443")
        listOf("http://synthetic.example","https://synthetic.example/","https://user@synthetic.example",
            "https://synthetic.example?q=x","https://synthetic.example#x","https://synthetic.example/other").forEach { s->fails { codec.requireOrigin(s) } }
    }
    @Test fun prepareExactOriginalOAuthFields() {
        val reservation=byteArrayOf(1,2,3)
        val body=JsonParser.parse(codec.prepare(reservation,"actual.original.jwt".toByteArray()).toString(Charsets.UTF_8)) as Map<*,*>
        assertEquals(setOf("reservation_original_base64","google_id_token"),body.keys)
        assertArrayEquals(reservation,Base64.getDecoder().decode(body["reservation_original_base64"] as String))
        assertEquals("actual.original.jwt",body["google_id_token"])
        fails { codec.prepare(reservation,"token\n".toByteArray()) }
    }
    @Test fun rawAndFinishKeepCompleteOriginals() {
        val raw=JsonParser.parse(codec.raw(arrayOf(byteArrayOf(1),byteArrayOf(2,3))).toString(Charsets.UTF_8)) as Map<*,*>
        assertEquals(setOf("signed_challenge_original_base64","platform_attestation_original_base64"),raw.keys)
        val fields=Array(6){byteArrayOf((it+1).toByte())};fields[5]="opaque.pi.token".toByteArray()
        val finished=JsonParser.parse(codec.finish(fields).toString(Charsets.UTF_8)) as Map<*,*>
        assertEquals(6,finished.size);assertEquals("opaque.pi.token",finished["play_integrity_token"])
        assertFalse(finished.keys.any { it.toString().contains("wallet") || it.toString().contains("verdict") })
        fails { codec.finish(fields.copyOfRange(0,5)) }
    }
    @Test fun exactResponsePurposeAndBase64() {
        val original=byteArrayOf(1,2,3)
        val encoded=Base64.getEncoder().encodeToString(original)
        val reply="{\"signed_challenge_original_base64\":\"$encoded\"}".toByteArray()
        assertArrayEquals(original,carrier(KagemushaHardwareBootstrapHttpOriginalV1.Stage.PREPARE).decodeIssuerResponse(reply))
        fails { carrier(KagemushaHardwareBootstrapHttpOriginalV1.Stage.RECEIPT).decodeIssuerResponse(reply) }
        fails { codec.response(KagemushaHardwareBootstrapHttpOriginalV1.Stage.PREPARE,
            "{\"signed_challenge_original_base64\":\"AQ==\",\"accepted\":true}".toByteArray()) }
        fails { codec.response(KagemushaHardwareBootstrapHttpOriginalV1.Stage.PREPARE,
            "{\"signed_challenge_original_base64\":\"AQ==\",\"signed_challenge_original_base64\":\"Ag==\"}".toByteArray()) }
        fails { codec.response(KagemushaHardwareBootstrapHttpOriginalV1.Stage.PREPARE,
            "{\"signed_challenge_original_base64\":\"AQ\"}".toByteArray()) }
    }
    @Test fun maximumCompleteOriginalAndOversizeReply() {
        val value=ByteArray(codec.MAX_ORIGINAL){1}
        val b64=Base64.getEncoder().encodeToString(value)
        val reply="{\"signed_hardware_receipt_original_base64\":\"$b64\"}".toByteArray()
        assertArrayEquals(value,codec.response(KagemushaHardwareBootstrapHttpOriginalV1.Stage.RECEIPT,reply))
        fails { codec.response(KagemushaHardwareBootstrapHttpOriginalV1.Stage.RECEIPT,
            ByteArray(((codec.MAX_ORIGINAL+2)/3)*4+129){32}) }
    }
    @Test fun boundedChunkedStreamAndUnknownLength() {
        val original=ByteArray(37){it.toByte()}
        val chunked=object:InputStream(){ var p=0;override fun read():Int=if(p==original.size)-1 else original[p++].toInt() and 255
            override fun read(b:ByteArray,off:Int,len:Int):Int { if(p==original.size)return -1;val n=minOf(3,len,original.size-p)
                original.copyInto(b,off,p,p+n);p+=n;return n } }
        assertArrayEquals(original,readBoundedHardwareIssuerOriginal(chunked,37) {})
        fails { readBoundedHardwareIssuerOriginal(ByteArrayInputStream(ByteArray(38)),37){} }
        fails { readBoundedHardwareIssuerOriginal(ByteArrayInputStream(byteArrayOf()),37){} }
    }
    @Test fun ownerLossRefusesCompleteResponseAndStream() {
        var checks=0
        val guarded=carrier(KagemushaHardwareBootstrapHttpOriginalV1.Stage.RECEIPT) { checks++;check(checks<2) }
        fails { guarded.decodeIssuerResponse("{\"signed_hardware_receipt_original_base64\":\"AQ==\"}".toByteArray()) }
        checks=0
        fails { readBoundedHardwareIssuerOriginal(ByteArrayInputStream(byteArrayOf(1)),37) { checks++;check(checks<2) } }
    }
    @Test fun defaultFactoryRejectsMissingSubstitutedOrAmbiguousProvider() {
        fails { KagemushaFirstDeviceHardwareEvidenceServiceFactoryV1.select(emptyList<KagemushaFirstDeviceHardwareEvidenceServiceFactoryV1>().iterator()) }
        val real=KagemushaAndroidFirstDeviceHardwareEvidenceServiceFactoryV1()
        assertSame(real,KagemushaFirstDeviceHardwareEvidenceServiceFactoryV1.select(listOf(real).iterator()))
        fails { KagemushaFirstDeviceHardwareEvidenceServiceFactoryV1.select(listOf(real,real).iterator()) }
        val offered=object:KagemushaFirstDeviceHardwareEvidenceServiceFactoryV1 {
            override fun open(context:android.content.Context):KagemushaFirstDeviceHardwareEvidenceSelectionV1=error("no native authority") }
        fails { KagemushaFirstDeviceHardwareEvidenceServiceFactoryV1.select(listOf(offered).iterator()) }
    }
    @Test fun oneHttpInvocationAndSeparateLateOriginalGuard() {
        var effectCurrent=true
        val value=KagemushaHardwareBootstrapHttpOriginalV1(KagemushaHardwareBootstrapHttpOriginalV1.Stage.RECEIPT,
            "https://synthetic.example",byteArrayOf(1),{},ByteArray(32){1},{check(effectCurrent)})
        value.claimOriginalInvocation()
        fails { value.claimOriginalInvocation() }
        effectCurrent=false
        fails { value.requireInvocationCurrent() }
        // A real matching late result is data custody, while another effect remains denied.
        assertArrayEquals(byteArrayOf(1),value.decodeIssuerResponse(
            "{\"signed_hardware_receipt_original_base64\":\"AQ==\"}".toByteArray()))
    }
    @Test fun noFinancialOperationsInDedicatedEndpoint() {
        val methods=KagemushaHardwareBootstrapNativeEndpointV1::class.java.declaredMethods.map{it.name.lowercase()}
        assertFalse(methods.any { it.contains("mint") || it.contains("transfer") || it.contains("balance") || it.contains("fold") })
    }
}
