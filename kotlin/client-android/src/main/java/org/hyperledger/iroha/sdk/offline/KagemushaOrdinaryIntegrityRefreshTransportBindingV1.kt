// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline
import java.security.MessageDigest
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidAppKeyHardwarePolicyV1
import org.hyperledger.iroha.sdk.crypto.keystore.requireOriginalP256DerV1
fun interface KagemushaOrdinaryNativeIntegrityRefreshEndpointV1 {
 fun integrityRefresh(phase:Int,coreHandle:Long,original:ByteArray):Array<ByteArray>?
}
/** Only the same measured wallet JNI can borrow the private descriptor. Metadata is read only. */
class KagemushaOrdinaryIntegrityRefreshTransportBindingV1 internal constructor(
 private val bridge:KagemushaCoreCoordinatorBridgeV1,private val guard:()->Unit) {
 fun requireOpen(){guard();bridge.requireOrdinaryDescriptorOpen();guard()}
 fun revoke()=bridge.close()
 fun refreshCurrentAccount(endpoint:KagemushaOrdinaryNativeStartupEndpointV1) {
  requireOpen();val reserved=bridge.invokeOrdinaryStartup(endpoint,1,0)
  require(reserved.size==6 && reserved[2].size==8)
  val id=java.nio.ByteBuffer.wrap(reserved[2]).order(java.nio.ByteOrder.LITTLE_ENDIAN).long
  check(id!=0L);bridge.invokeOrdinaryStartup(endpoint,6,id);requireOpen()
 }
 fun invoke(endpoint:KagemushaOrdinaryNativeIntegrityRefreshEndpointV1,phase:Int,original:ByteArray=ByteArray(0)):List<ByteArray> {
  requireOpen();return bridge.invokeOrdinaryIntegrity(endpoint,phase,original).also{requireOpen()}
 }
 internal fun completedKey(endpoint:KagemushaOrdinaryNativeIntegrityRefreshEndpointV1):KagemushaNativeCompletedIntegrityKeyOriginalsV1 =
  KagemushaNativeCompletedIntegrityKeyOriginalsV1(invoke(endpoint,10)) {
   invoke(endpoint,10)
  }
 fun acceptChallenge(endpoint:KagemushaOrdinaryNativeIntegrityRefreshEndpointV1,signedOriginal:ByteArray):KagemushaNativePreparedOrdinaryIntegrityRefreshV1 =
  prepared(endpoint,invoke(endpoint,2,signedOriginal),completedKey(endpoint))
 fun recoverPrepared(endpoint:KagemushaOrdinaryNativeIntegrityRefreshEndpointV1):KagemushaNativePreparedOrdinaryIntegrityRefreshV1 {
  val held=invoke(endpoint,8);require(held[4].size==514)
  val c=KagemushaPlayIntegrityRefreshPreparationV1.parseOriginal(held[4])
  return prepared(endpoint,listOf(held[4],c.possessionSigningBytes(),c.playIntegrityRequestHash(),c.attestedKeyId()),completedKey(endpoint))
 }
 private fun prepared(endpoint:KagemushaOrdinaryNativeIntegrityRefreshEndpointV1,fields:List<ByteArray>,
  key:KagemushaNativeCompletedIntegrityKeyOriginalsV1):KagemushaNativePreparedOrdinaryIntegrityRefreshV1 {
  KagemushaOrdinaryIntegrityRefreshFrameV1.requireResponse(2,fields);key.recheck()
  val c=KagemushaPlayIntegrityRefreshPreparationV1.parseOriginal(fields[0])
  check(same(c.attestedKeyId(),key.id()) && same(c.credentialDigest(),key.credentialDigest()))
  return KagemushaNativePreparedOrdinaryIntegrityRefreshV1.fromNative(this,endpoint,key,c)
 }
 private fun same(a:ByteArray,b:ByteArray)=MessageDigest.isEqual(a,b)
}
/** Original key metadata lent only by actual Native completed custody. No platform generation. */
internal class KagemushaNativeCompletedIntegrityKeyOriginalsV1(fields:List<ByteArray>,private val read:()->List<ByteArray>) {
 private val f=fields.map(ByteArray::copyOf)
 init{KagemushaOrdinaryIntegrityRefreshFrameV1.requireResponse(10,f)
  check(MessageDigest.isEqual(f[4],MessageDigest.getInstance("SHA-256").digest(f[3])))
  check(project()==KagemushaOrdinaryIdentityHttpCodecV1.playIntegrityCloudProjectOriginal(f[8]))}
 fun recheck(){val current=read();check(current.size==f.size && f.indices.all{MessageDigest.isEqual(f[it],current[it])})}
 fun id()=f[4].copyOf();fun credentialDigest()=f[10].copyOf()
 fun project()=java.nio.ByteBuffer.wrap(f[9]).order(java.nio.ByteOrder.LITTLE_ENDIAN).long.also{require(it>0)}
 fun incomingKey()=KagemushaNativeOrdinaryIncomingKeyOriginalsV1(f[1].toString(Charsets.UTF_8),f[2],f[3],policy(),::recheck)
 fun retail()=KagemushaNativeCompletedRetailBindingV1(f[10],f[7],f[0],::recheck)
 private fun policy()=when(f[5][0].toInt()){1->KagemushaAndroidAppKeyHardwarePolicyV1.TEE_ONLY
  2->KagemushaAndroidAppKeyHardwarePolicyV1.STRONGBOX_ONLY;3->KagemushaAndroidAppKeyHardwarePolicyV1.TEE_OR_STRONGBOX;else->error("Unknown Native hardware policy")}
 fun originalCertificate()=f[7].copyOf()
 fun originalCredential()=f[6].copyOf()
 fun signFields(sign:(String,ByteArray,ByteArray,ByteArray,KagemushaAndroidAppKeyHardwarePolicyV1)->ByteArray):ByteArray {
  recheck();return sign(f[1].toString(Charsets.UTF_8),f[2].copyOf(),f[3].copyOf(),f[4].copyOf(),policy()).also{recheck()}
 }
}
/** Separate PI possession capability. Only actual phase2 selects full C/message/key; raw DTOs
 * cannot sign, generate a key, renew C or turn Google evidence into a lease/financial grant.
 */
class KagemushaNativePreparedOrdinaryIntegrityRefreshV1 private constructor(
 private val binding:KagemushaOrdinaryIntegrityRefreshTransportBindingV1,
 private val endpoint:KagemushaOrdinaryNativeIntegrityRefreshEndpointV1,
 private val key:KagemushaNativeCompletedIntegrityKeyOriginalsV1,
 private val c:KagemushaPlayIntegrityRefreshPreparationV1) {
 internal companion object {
  fun fromNative(binding:KagemushaOrdinaryIntegrityRefreshTransportBindingV1,
   endpoint:KagemushaOrdinaryNativeIntegrityRefreshEndpointV1,key:KagemushaNativeCompletedIntegrityKeyOriginalsV1,
   c:KagemushaPlayIntegrityRefreshPreparationV1)=KagemushaNativePreparedOrdinaryIntegrityRefreshV1(binding,endpoint,key,c)
 }
 private var invoked=false
 private var raw:ByteArray?=null
 fun preparationOriginal()=c.transportBytes();fun requestHash()=c.playIntegrityRequestHash();fun operationId()=c.operationId()
 fun cloudProjectNumber()=key.project()
 fun requireCurrent(){binding.requireOpen();key.recheck()}
 @Synchronized internal fun performPlatformSigning(sign:(String,ByteArray,ByteArray,ByteArray,ByteArray,
  KagemushaAndroidAppKeyHardwarePolicyV1,()->Unit)->ByteArray):ByteArray {
  requireCurrent();val recovery=binding.invoke(endpoint,8)
  check(recovery[4].contentEquals(c.transportBytes()))
  if(recovery[3][0].toInt() in listOf(2,3)) raw=recovery[5].copyOf()
  if(raw==null){check(!invoked);val fence=binding.invoke(endpoint,3)
   check(fence[0].contentEquals(c.transportBytes()));invoked=true
   raw=key.signFields{alias,challenge,point,id,policy->sign(alias,challenge,point,id,c.possessionSigningBytes(),policy,::requireCurrent)}
  }
  val held=checkNotNull(raw);requireOriginalP256DerV1(held)
  if(recovery[3][0]!=3.toByte())binding.invoke(endpoint,4,held)
  val captured=binding.invoke(endpoint,8);check(captured[3][0]==3.toByte() && captured[5].contentEquals(held))
  requireCurrent();return held.copyOf()
 }
}
