// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline
import org.junit.jupiter.api.Test
import kotlin.test.*
import java.nio.ByteBuffer
import java.nio.ByteOrder
/** TEST ONLY public Model210 data. This assertion belongs with the Android private signer;
 * it obtains no Native descriptor, approval holder or platform hardware permission.
 */
class KagemushaOrdinaryMintApprovalSigningPurposeV1Test {
 private fun original():ByteArray {val d=KagemushaOrdinaryMintApprovalProjectionV1.DOMAIN.toByteArray(Charsets.US_ASCII)
  val raw=d+ByteArray(8+210);val b=ByteBuffer.wrap(raw).order(ByteOrder.LITTLE_ENDIAN);b.putLong(d.size,210);b.putShort(d.size+8,1)
  repeat(6){n->java.util.Arrays.fill(raw,d.size+10+n*32,d.size+42+n*32,(n+1).toByte())};b.putLong(d.size+202,10);b.putLong(d.size+210,100);return raw}
 @Test fun privatePlatformPurposeCannotBeRelabeledGenericApproval(){
  val raw=original();org.hyperledger.iroha.sdk.crypto.keystore.requireAppPlatformSigningMessageV1(raw,
   org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidAppSignaturePurposeV1.ORDINARY_MINT_PRE_DEBIT_APPROVAL)
  for(purpose in listOf(org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidAppSignaturePurposeV1.OPERATION_APPROVAL,
   org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidAppSignaturePurposeV1.ORDINARY_PREPARATION_APPROVAL,
   org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidAppSignaturePurposeV1.IDENTITY_ENROLLMENT_POSSESSION))assertFails{
    org.hyperledger.iroha.sdk.crypto.keystore.requireAppPlatformSigningMessageV1(raw,purpose)}
 }
}
