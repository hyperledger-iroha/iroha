// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline
import org.junit.jupiter.api.Test
import kotlin.test.*
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.Base64
import java.security.MessageDigest
import org.hyperledger.iroha.sdk.client.JsonEncoder
/** Unsigned model-shaped specimens only, never installed issuer/Native/key/Google authority. */
internal fun ordinaryIntegrityTestChallengeV1():ByteArray=ByteArray(514).also{
 it[0]=1;repeat(13){n->it.fill((n+1).toByte(),2+n*32,34+n*32)}
 ByteBuffer.wrap(it).order(ByteOrder.LITTLE_ENDIAN).putLong(418,1).putLong(426,1).putLong(434,1000).putLong(442,11000)
 it.fill(64,450,514)
}
internal fun ordinaryIntegrityPrepareTestResponseV1():ByteArray {
 val c=KagemushaPlayIntegrityRefreshPreparationV1.parseOriginal(ordinaryIntegrityTestChallengeV1())
 fun b(raw:ByteArray)=Base64.getEncoder().encodeToString(raw)
 return JsonEncoder.encode(linkedMapOf("operation_id" to c.operationId().joinToString(""){"%02x".format(it.toInt() and 255)},
  "signed_refresh_challenge_base64" to b(c.transportBytes()),"possession_signing_message_base64" to b(c.possessionSigningBytes()),
  "play_integrity_request_hash_base64" to b(c.playIntegrityRequestHash()),"expires_at_ms" to 11000)).toByteArray()
}
class KagemushaOrdinaryIntegrityRefreshV1FrameTest {
 @Test fun signedOriginalNonceHashMessageAndExpiryMustAllAgree(){
  val raw=ordinaryIntegrityPrepareTestResponseV1();val nonce=ByteArray(32){12}
  assertContentEquals(ordinaryIntegrityTestChallengeV1(),KagemushaOrdinaryIntegrityHttpCodecV1.prepareResponse(raw,nonce))
  assertFails{KagemushaOrdinaryIntegrityHttpCodecV1.prepareResponse(raw,ByteArray(32){13})}
  assertFails{KagemushaOrdinaryIntegrityHttpCodecV1.prepareResponse(raw.toString(Charsets.UTF_8).replace("11000","11001").toByteArray(),nonce)}
  assertFails{KagemushaOrdinaryIntegrityHttpCodecV1.prepareResponse(raw+byteArrayOf(0xc0.toByte()),nonce)}
 }
 @Test fun strictResponseFieldsDuplicateKeysAndLeaseHashRejectSubstitution(){
  val digest=MessageDigest.getInstance("SHA-256").digest(byteArrayOf(1)).joinToString(""){"%02x".format(it.toInt() and 255)}
  val raw="{\"lease_base64\":\"AQ==\",\"lease_sha256_hex\":\"$digest\"}".toByteArray()
  assertContentEquals(byteArrayOf(1),KagemushaOrdinaryIntegrityHttpCodecV1.finishResponse(raw))
  assertFails{KagemushaOrdinaryIntegrityHttpCodecV1.finishResponse(raw.toString(Charsets.UTF_8).replace("AQ==","Ag==").toByteArray())}
  assertFails{KagemushaOrdinaryIntegrityHttpCodecV1.finishResponse("{\"lease_base64\":\"AQ==\",\"lease_base64\":\"AQ==\",\"lease_sha256_hex\":\"$digest\"}".toByteArray())}
  assertFails{KagemushaOrdinaryIntegrityHttpCodecV1.finishResponse(raw.toString(Charsets.UTF_8).dropLast(1).plus(",\"ready\":true}").toByteArray())}
 }
 @Test fun pendingRecoveryMustKeepOriginalNonceAndAcknowledgedSignature(){
  val c=ordinaryIntegrityTestChallengeV1();val f=listOf(byteArrayOf(1),byteArrayOf(2),ByteArray(32){12},byteArrayOf(3),c,ByteArray(8){1},"token".toByteArray(),ByteArray(0))
  KagemushaOrdinaryIntegrityRefreshFrameV1.requireResponse(8,f)
  assertFails{KagemushaOrdinaryIntegrityRefreshFrameV1.requireResponse(8,f.toMutableList().also{it[2]=ByteArray(32){13}})}
  assertFails{KagemushaOrdinaryIntegrityRefreshFrameV1.requireResponse(8,f.toMutableList().also{it[3]=byteArrayOf(2)})}
  assertFails{KagemushaOrdinaryIntegrityRefreshFrameV1.requireResponse(8,f.toMutableList().also{it[5]=ByteArray(0)})}
 }
 @Test fun completedCustodyGrammarIsElevenFieldsAndCannotSelectGenerationOrSoftware(){
  val f=listOf(ByteArray(32){1},"alias".toByteArray(),ByteArray(32){2},ByteArray(65){if(it==0)4 else 1},ByteArray(32){3},byteArrayOf(3),
   byteArrayOf(1),byteArrayOf(2),byteArrayOf(3),ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(4).array(),ByteArray(32){5})
  KagemushaOrdinaryIntegrityRefreshFrameV1.requireResponse(10,f)
  assertFails{KagemushaOrdinaryIntegrityRefreshFrameV1.requireResponse(10,f.dropLast(1))}
  assertFails{KagemushaOrdinaryIntegrityRefreshFrameV1.requireResponse(10,f.toMutableList().also{it[5]=byteArrayOf(0)})}
  assertFails{KagemushaOrdinaryIntegrityRefreshFrameV1.requireRequest(10,byteArrayOf(1))}
  assertFails{KagemushaOrdinaryIntegrityRefreshFrameV1.responseFields(10,7,listOf(byteArrayOf(1,0),byteArrayOf(10),ByteArray(8){8})+f)}
 }
}
