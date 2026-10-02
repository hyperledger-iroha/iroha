// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline
import org.junit.jupiter.api.Test
import kotlin.test.*
import java.io.ByteArrayOutputStream
import java.util.Base64
import java.security.MessageDigest
class KagemushaOrdinaryIncomingFrameV1Test {
 @Test fun requestsAreExactAndReceiveIsDistinct() {
  val d=ByteArray(32){1};KagemushaOrdinaryIncomingFrameV1.requireRequest(17,listOf(d,byteArrayOf(2),byteArrayOf(3)))
  for(p in listOf(2,4,5,6,9,11,12,13,16)) { KagemushaOrdinaryIncomingFrameV1.requireRequest(p,emptyList())
   assertFailsWith<IllegalArgumentException> { KagemushaOrdinaryIncomingFrameV1.requireRequest(p,listOf(d)) } }
  for(f in listOf(listOf(d,byteArrayOf(1)),listOf(ByteArray(31),byteArrayOf(1),byteArrayOf(1))))
   assertFailsWith<IllegalArgumentException> { KagemushaOrdinaryIncomingFrameV1.requireRequest(17,f) }
  assertFailsWith<IllegalStateException> { KagemushaOrdinaryIncomingFrameV1.requireRequest(18,emptyList()) }
 }
 @Test fun responseIdentityAndSignedRequestDigestCannotBeSubstituted() {
  val f=listOf(byteArrayOf(0),byteArrayOf(4),ByteArray(64){3},byteArrayOf(5),MessageDigest.getInstance("SHA-256").digest(byteArrayOf(4)))
  val h=listOf(byteArrayOf(1,0),byteArrayOf(6),byteArrayOf(7,0,0,0,0,0,0,0))
  assertEquals(5,KagemushaOrdinaryIncomingFrameV1.responseFields(6,7,h+f).size)
  assertFailsWith<IllegalArgumentException> { KagemushaOrdinaryIncomingFrameV1.responseFields(6,8,h+f) }
  assertFailsWith<IllegalArgumentException> { KagemushaOrdinaryIncomingFrameV1.requireResponse(6,f.dropLast(1)+ByteArray(32){9}) }
 }
 @Test fun recoveryStatusesRequireExactCapturedOriginals() {
  KagemushaOrdinaryIncomingFrameV1.requireResponse(4,listOf(byteArrayOf(0),byteArrayOf(),byteArrayOf()))
  KagemushaOrdinaryIncomingFrameV1.requireResponse(11,listOf(byteArrayOf(2),ByteArray(8){1},byteArrayOf(2)))
  for(f in listOf(listOf(byteArrayOf(3),byteArrayOf(),byteArrayOf()),listOf(byteArrayOf(1),byteArrayOf(),byteArrayOf(2)),
   listOf(byteArrayOf(0),ByteArray(8),byteArrayOf()))) assertFailsWith<IllegalArgumentException> { KagemushaOrdinaryIncomingFrameV1.requireResponse(4,f) }
 }
 @Test fun streamedBodyUsesExactFullOriginalsAndDoesNotCloseCallerOutput() {
  val proof=ByteArray(3*1024*1024){it.toByte()};var closed=false
  val out=object:ByteArrayOutputStream(){ override fun close(){closed=true} }
  KagemushaOrdinaryIncomingHttpCodecV1.writeRequestBody(byteArrayOf(1),ByteArray(64){2},proof,false,out)
  assertFalse(closed);val text=out.toString("US-ASCII")
  assertTrue(text.contains("\"proof_bundle_original_base64\":\""+Base64.getEncoder().encodeToString(proof)+"\""))
  assertEquals(KagemushaOrdinaryIncomingHttpCodecV1.requestId(byteArrayOf(1)),KagemushaOrdinaryIncomingHttpCodecV1.requestId(byteArrayOf(1)))
 }
 @Test fun responseRejectsExtraDuplicateMissingNoncanonicalAndMalformedUtf8() {
  val good="{\"signed_result_original_base64\":\"AQ==\",\"data_record_original_base64\":\"Ag==\",\"authority_original_base64\":\"Aw==\"}"
  assertEquals(3,KagemushaOrdinaryIncomingHttpCodecV1.responseOriginals(good.toByteArray()).size)
  for(bad in listOf(good.replace("AQ==","AQ"),good.replace("AQ==","AR=="),good.dropLast(1)+",\"extra\":true}",
   good.dropLast(1)+",\"authority_original_base64\":\"Aw==\"}","{}"))
    assertFails { KagemushaOrdinaryIncomingHttpCodecV1.responseOriginals(bad.toByteArray()) }
  assertFails { KagemushaOrdinaryIncomingHttpCodecV1.responseOriginals(byteArrayOf(0xc3.toByte(),0x28)) }
 }
}
