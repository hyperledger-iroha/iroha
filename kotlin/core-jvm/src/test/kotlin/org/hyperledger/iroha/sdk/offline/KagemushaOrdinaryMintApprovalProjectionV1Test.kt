package org.hyperledger.iroha.sdk.offline
import org.junit.jupiter.api.Test
import kotlin.test.*
import java.nio.ByteBuffer
import java.nio.ByteOrder
/** TEST ONLY Model210 layout with public fixture fields, not a signing holder/capability. */
class KagemushaOrdinaryMintApprovalProjectionV1Test {
 private fun original():ByteArray {val d=KagemushaOrdinaryMintApprovalProjectionV1.DOMAIN.toByteArray(Charsets.US_ASCII)
  val raw=d+ByteArray(8+210);val b=ByteBuffer.wrap(raw).order(ByteOrder.LITTLE_ENDIAN);b.putLong(d.size,210);b.putShort(d.size+8,1)
  repeat(6){n->java.util.Arrays.fill(raw,d.size+10+n*32,d.size+42+n*32,(n+1).toByte())};b.putLong(d.size+202,10);b.putLong(d.size+210,100);return raw}
 @Test fun soleModelDigestFieldsAnd210Version(){val p=KagemushaOrdinaryMintApprovalProjectionV1;val raw=original();p.requireOriginal(raw)
  assertContentEquals(ByteArray(32){1},p.operationId(raw));assertContentEquals(ByteArray(32){3},p.credentialDigest(raw))}
 @Test fun genericPurposeVersionAndZeroSelectorsRefuse(){val p=KagemushaOrdinaryMintApprovalProjectionV1;val d=p.DOMAIN.length
  for(raw in listOf(original().also{it[0]=0},original().also{it[d+8]=2},original().also{java.util.Arrays.fill(it,d+10,d+42,0.toByte())},ByteArray(325)))assertFails{p.requireOriginal(raw)}}
 @Test fun unchangedFiniteIntervalCannotWiden(){val p=KagemushaOrdinaryMintApprovalProjectionV1;val d=p.DOMAIN.length
  for(pair in listOf(0L to 100L,100L to 100L,1L to 120002L)){val raw=original();ByteBuffer.wrap(raw).order(ByteOrder.LITTLE_ENDIAN).putLong(d+202,pair.first).putLong(d+210,pair.second);assertFails{p.requireOriginal(raw)}}}
}
