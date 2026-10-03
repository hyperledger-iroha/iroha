// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.util.Base64
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
import org.junit.jupiter.api.Test
import kotlin.test.*

/** Inert carrier data does not verify proof, signatures, finality or Native custody. */
class KagemushaOrdinaryLineageHttpCodecV1Test {
    @Test fun soleExistingServiceEnvelopePreservesEveryNativeOriginal() {
        val request=byteArrayOf(1,2);val signature=ByteArray(64) { 3 };val service=byteArrayOf(4,5,6)
        val body=body(request,signature,service)
        @Suppress("UNCHECKED_CAST") val parsed=JsonParser.parse(body.toString(Charsets.UTF_8)) as Map<String,Any?>
        assertEquals(setOf("schema","canonical_request_base64","account_signature_base64","proof_bundle_original_base64"),parsed.keys)
        assertEquals("iroha.kagemusha.ordinary-lineage-cas-request.v1",parsed["schema"])
        assertEquals(base64(request),parsed["canonical_request_base64"]);assertEquals(base64(signature),parsed["account_signature_base64"])
        assertEquals(base64(service),parsed["proof_bundle_original_base64"])
        assertEquals(KagemushaOrdinaryLineageHttpCodecV1.requestId(request),KagemushaOrdinaryLineageHttpCodecV1.requestId(request.copyOf()))
        assertNotEquals(KagemushaOrdinaryLineageHttpCodecV1.requestId(request),KagemushaOrdinaryLineageHttpCodecV1.requestId(byteArrayOf(1,3)))
        for (width in listOf(0,63,65)) assertFails { body(request,ByteArray(width),service) }
        assertFails { body(request,signature,ByteArray(0)) }
    }
    @Test fun fullDistinctResponseOriginalsAreReturnedWithoutAuthorityVerdict() {
        val authority=ByteArray(300*1024) { (it%251).toByte() }
        val fields=KagemushaOrdinaryLineageHttpCodecV1.responseOriginals(reply("AQ==","Ag==",base64(authority)))
        assertContentEquals(byteArrayOf(1),fields[0]);assertContentEquals(byteArrayOf(2),fields[1]);assertContentEquals(authority,fields[2])
    }
    @Test fun duplicateUnknownPartialMalformedAndAlternateBase64AreRefused() {
        val good=reply("AQ==","Ag==","Aw==").toString(Charsets.UTF_8)
        for (raw in listOf(good.dropLast(1)+",\"ready\":true}",
            good.dropLast(1)+",\"authority_original_base64\":\"BA==\"}",
            good.replace("AQ==","AQ"),good.replace("AQ==","AR=="),good.replace("Ag==",""),
            good.replace("\"Aw==\"","null"),"{}")) assertFails {
                KagemushaOrdinaryLineageHttpCodecV1.responseOriginals(raw.toByteArray(Charsets.UTF_8)) }
        assertFails { KagemushaOrdinaryLineageHttpCodecV1.responseOriginals(byteArrayOf(0xff.toByte())) }
        assertFails { KagemushaOrdinaryLineageHttpCodecV1.responseOriginals(reply(base64(ByteArray(32769)),"Ag==","Aw==")) }
    }
    @Test fun completeProofIsStreamedAndCallerOutputRemainsOpen() {
        val proof=ByteArray(3*1024*1024+1){(it%251).toByte()}
        val output=object:java.io.ByteArrayOutputStream(){var closed=false;var maximumChunk=0
            override fun write(b:ByteArray,off:Int,len:Int){maximumChunk=maxOf(maximumChunk,len);super.write(b,off,len)}
            override fun close(){closed=true;super.close()}}
        KagemushaOrdinaryLineageHttpCodecV1.writeRequestBody(byteArrayOf(1),ByteArray(64){2},proof,output)
        assertFalse(output.closed);assertTrue(output.maximumChunk<=16384)
        @Suppress("UNCHECKED_CAST") val parsed=JsonParser.parse(output.toString("UTF-8")) as Map<String,Any?>
        assertEquals(base64(proof),parsed["proof_bundle_original_base64"])
    }
    private fun body(request:ByteArray,signature:ByteArray,proof:ByteArray)=java.io.ByteArrayOutputStream().also {
        KagemushaOrdinaryLineageHttpCodecV1.writeRequestBody(request,signature,proof,it)
    }.toByteArray()
    private fun reply(a: String,b: String,c: String)=JsonEncoder.encode(linkedMapOf("signed_result_original_base64" to a,
        "data_record_original_base64" to b,"authority_original_base64" to c)).toByteArray(Charsets.UTF_8)
    private fun base64(raw: ByteArray)=Base64.getEncoder().encodeToString(raw)
}
