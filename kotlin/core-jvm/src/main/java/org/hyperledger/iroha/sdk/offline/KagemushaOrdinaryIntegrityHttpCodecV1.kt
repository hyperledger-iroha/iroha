// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline
import java.math.BigInteger
import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import java.security.MessageDigest
import java.util.Base64
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser
/** Pure strict Core transport grammar; neither a parsed challenge nor lease admits authority. */
object KagemushaOrdinaryIntegrityHttpCodecV1 {
 const val MAXIMUM_RESPONSE_BYTES=256*1024
 fun prepareBody(enrollment:ByteArray,nonce:ByteArray):ByteArray {
  digest(enrollment);digest(nonce)
  return json(linkedMapOf("enrollment_id_hex" to hex(enrollment),"client_nonce_hex" to hex(nonce)))
 }
 fun prepareResponse(raw:ByteArray,nonce:ByteArray):ByteArray {
  digest(nonce);val f=fields(raw);exact(f,"operation_id","signed_refresh_challenge_base64",
   "possession_signing_message_base64","play_integrity_request_hash_base64","expires_at_ms")
  val original=base64(f,"signed_refresh_challenge_base64",514,514)
  val c=KagemushaPlayIntegrityRefreshPreparationV1.parseOriginal(original)
  require(c.nonce().contentEquals(nonce) && text(f,"operation_id")==hex(c.operationId()))
  require(base64(f,"possession_signing_message_base64",8192).contentEquals(c.possessionSigningBytes()) &&
   base64(f,"play_integrity_request_hash_base64",32,32).contentEquals(c.playIntegrityRequestHash()))
  val expires=when(val x=f["expires_at_ms"]){is Number->BigInteger(x.toString());else->error("Expiry is not numeric")}
  require(expires==c.expiresAtMs);return original
 }
 fun finishBody(originals:List<ByteArray>):ByteArray {
  KagemushaOrdinaryIntegrityRefreshFrameV1.requireResponse(5,originals)
  return json(linkedMapOf("schema" to "iroha.kagemusha.play-integrity-refresh-request.v1","operation" to "issue",
   "operation_id" to hex(originals[0]),"signed_refresh_challenge_base64" to encode(originals[1]),
   "signature_der_base64" to encode(originals[2]),"play_integrity_token" to originals[3].toString(Charsets.US_ASCII)))
 }
 fun finishResponse(raw:ByteArray):ByteArray {
  val f=fields(raw);exact(f,"lease_base64","lease_sha256_hex");val lease=base64(f,"lease_base64",4096)
  require(text(f,"lease_sha256_hex")==hex(sha(lease)));return lease
 }
 fun requestId(body:ByteArray):String {
  require(body.size in 1..MAXIMUM_RESPONSE_BYTES)
  return KagemushaOrdinaryIdentityHttpCodecV1.rawAttestationRequestId(sha(
   "iroha:kagemusha:v1:ordinary-integrity-http\u0000".toByteArray(Charsets.US_ASCII)+body))
 }
 private fun json(value:Map<String,Any?>)=JsonEncoder.encode(value).toByteArray(Charsets.UTF_8).also{require(it.size<=MAXIMUM_RESPONSE_BYTES)}
 @Suppress("UNCHECKED_CAST") private fun fields(raw:ByteArray):Map<String,Any?> {
  require(raw.size in 1..MAXIMUM_RESPONSE_BYTES)
  val text=Charsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT).onUnmappableCharacter(CodingErrorAction.REPORT)
   .decode(ByteBuffer.wrap(raw)).toString()
  return JsonParser.parse(text) as? Map<String,Any?> ?:error("Integrity reply is not an object")
 }
 private fun exact(f:Map<String,Any?>,vararg names:String){require(f.keys==names.toSet())}
 private fun text(f:Map<String,Any?>,name:String)=f[name] as? String ?:error("Integrity field is not text")
 private fun base64(f:Map<String,Any?>,name:String,max:Int,min:Int=1):ByteArray {
  val encoded=text(f,name);require(encoded.length<=((max+2)/3)*4)
  return Base64.getDecoder().decode(encoded).also{require(it.size in min..max && encode(it)==encoded)}
 }
 private fun encode(raw:ByteArray)=Base64.getEncoder().encodeToString(raw)
 private fun digest(raw:ByteArray){require(raw.size==32 && raw.any{it!=0.toByte()})}
 private fun hex(raw:ByteArray)=raw.joinToString(""){"%02x".format(it.toInt() and 255)}
 private fun sha(raw:ByteArray)=MessageDigest.getInstance("SHA-256").digest(raw)
}
