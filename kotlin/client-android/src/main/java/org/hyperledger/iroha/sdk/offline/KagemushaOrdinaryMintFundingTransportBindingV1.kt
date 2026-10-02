// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline
import java.security.MessageDigest
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidAppKeyHardwarePolicyV1
import org.hyperledger.iroha.sdk.crypto.keystore.requireOriginalP256DerV1
fun interface KagemushaOrdinaryNativeMintFundingEndpointV1 {fun mintFunding(phase:Int,coreHandle:Long,originals:Array<ByteArray>):Array<ByteArray>?}
/** Only the measured JNI may borrow this exact descriptor. No offered account/key/time fields. */
class KagemushaOrdinaryMintFundingTransportBindingV1 internal constructor(private val bridge:KagemushaCoreCoordinatorBridgeV1,private val guard:()->Unit){
 fun requireOpen(){guard();bridge.requireOrdinaryDescriptorOpen();guard()};fun revoke()=bridge.close()
 fun invoke(endpoint:KagemushaOrdinaryNativeMintFundingEndpointV1,phase:Int,fields:List<ByteArray> = emptyList()):List<ByteArray>{requireOpen();return bridge.invokeOrdinaryMintFunding(endpoint,phase,fields).also{requireOpen()}}
 internal fun prepared(endpoint:KagemushaOrdinaryNativeMintFundingEndpointV1,metadata:KagemushaOrdinaryNativeIntegrityRefreshEndpointV1,fields:List<ByteArray>):KagemushaNativePreparedOrdinaryMintApprovalV1 {
  KagemushaOrdinaryMintFundingFrameV1.requireResponse(1,fields)
  val key=KagemushaOrdinaryIntegrityRefreshTransportBindingV1(bridge,guard).completedKey(metadata)
  return KagemushaNativePreparedOrdinaryMintApprovalV1.fromNative(this,endpoint,key,fields)
 }
}
/** Dedicated pre-debit approval; only actual Native1 or retained12 may create this holder. */
class KagemushaNativePreparedOrdinaryMintApprovalV1 private constructor(
 private val binding:KagemushaOrdinaryMintFundingTransportBindingV1,private val endpoint:KagemushaOrdinaryNativeMintFundingEndpointV1,
 private val key:KagemushaNativeCompletedIntegrityKeyOriginalsV1,fields:List<ByteArray>){
 private val operation=fields[0].copyOf();private val message=fields[1].copyOf();private val credential=fields[2].copyOf()
 private var invoked=false
 init{KagemushaOrdinaryMintApprovalProjectionV1.requireOriginal(message)
  check(same(operation,KagemushaOrdinaryMintApprovalProjectionV1.operationId(message)) && same(credential,key.originalCredential()) &&
   same(key.credentialDigest(),KagemushaOrdinaryMintApprovalProjectionV1.credentialDigest(message)))}
 internal companion object{fun fromNative(binding:KagemushaOrdinaryMintFundingTransportBindingV1,endpoint:KagemushaOrdinaryNativeMintFundingEndpointV1,
  key:KagemushaNativeCompletedIntegrityKeyOriginalsV1,fields:List<ByteArray>)=KagemushaNativePreparedOrdinaryMintApprovalV1(binding,endpoint,key,fields)}
 fun operationId()=operation.copyOf();fun signingBytes()=message.copyOf()
 fun requireCurrent(){binding.requireOpen();key.recheck()}
 @Synchronized internal fun performPlatformSigning(sign:(String,ByteArray,ByteArray,ByteArray,ByteArray,KagemushaAndroidAppKeyHardwarePolicyV1,()->Unit)->ByteArray):ByteArray {
  requireCurrent();val r=binding.invoke(endpoint,12)
  check(same(operation,r[1]) && same(message,r[2]) && same(credential,r[3]))
  val floor=binding.invoke(endpoint,15);check(floor[0].contentEquals(byteArrayOf(5)) && floor[1].isEmpty())
  val status=r[0][0].toInt();check(status!=1){"The original OS invocation is uncertain"}
  var raw=if(status>=2)r[4].copyOf()else null
  if(raw==null){check(!invoked);binding.invoke(endpoint,2);invoked=true
   raw=key.signFields{alias,challenge,point,id,policy->sign(alias,challenge,point,id,message.copyOf(),policy,::requireCurrent)}
   requireOriginalP256DerV1(raw);binding.invoke(endpoint,3,listOf(raw))
  }else if(status==2)binding.invoke(endpoint,16)
  val acknowledged=binding.invoke(endpoint,12);check(acknowledged[0].contentEquals(byteArrayOf(3)) && same(acknowledged[4],raw))
  requireOriginalP256DerV1(raw);requireCurrent();return raw.copyOf()
 }
 private fun same(a:ByteArray,b:ByteArray)=MessageDigest.isEqual(a,b)
}
