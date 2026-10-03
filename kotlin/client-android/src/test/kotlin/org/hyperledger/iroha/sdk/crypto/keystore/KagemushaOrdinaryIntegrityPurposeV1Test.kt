// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.crypto.keystore
import org.junit.jupiter.api.Test
import kotlin.test.*
import org.hyperledger.iroha.sdk.offline.KagemushaPlayIntegrityRefreshPreparationV1
/** Exact public signing grammar only; no platform key or holder is constructed. */
class KagemushaOrdinaryIntegrityPurposeV1Test {
 private fun fixture()=ByteArray(514).also {
  it[0]=1;repeat(13){n->it.fill((n+1).toByte(),2+n*32,34+n*32)}
  java.nio.ByteBuffer.wrap(it).order(java.nio.ByteOrder.LITTLE_ENDIAN).putLong(418,1).putLong(426,1).putLong(434,1000).putLong(442,11000)
 }
 @Test fun distinctRefreshPossessionRequiresFullSoleModelOriginal(){
  val c=KagemushaPlayIntegrityRefreshPreparationV1.parseOriginal(fixture())
  val m=c.possessionSigningBytes()
  requireAppPlatformSigningMessageV1(m,KagemushaAndroidAppSignaturePurposeV1.ORDINARY_INTEGRITY_REFRESH_POSSESSION)
  assertFails{requireAppPlatformSigningMessageV1(c.canonicalSigningBytes(),KagemushaAndroidAppSignaturePurposeV1.ORDINARY_INTEGRITY_REFRESH_POSSESSION)}
  assertFails{requireAppPlatformSigningMessageV1(m+byteArrayOf(0),KagemushaAndroidAppSignaturePurposeV1.ORDINARY_INTEGRITY_REFRESH_POSSESSION)}
  assertFails{requireAppPlatformSigningMessageV1(m,KagemushaAndroidAppSignaturePurposeV1.OPERATION_APPROVAL)}
 }
}
