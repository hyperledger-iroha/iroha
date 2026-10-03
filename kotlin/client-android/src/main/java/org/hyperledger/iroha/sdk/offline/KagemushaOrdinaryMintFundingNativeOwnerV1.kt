// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

/** Packaging SPI only. The funding binding accepts solely the exact fixed wallet provider,
 * original classloader and final JNI class; implementing this interface grants no descriptor.
 */
interface KagemushaOrdinaryMintFundingNativeOwnerV1 {
 fun fundingEndpoint():KagemushaOrdinaryNativeMintFundingEndpointV1
 fun completedMetadataEndpoint():KagemushaOrdinaryNativeIntegrityRefreshEndpointV1
}
/** Private transport selection; Native independently checks Application/release/account custody. */
internal class OriginalOrdinaryMintFundingNativeV1(provider:KagemushaOrdinaryMintFundingNativeOwnerV1) {
 private val original=provider
 val funding=provider.fundingEndpoint()
 val metadata=provider.completedMetadataEndpoint()
 init{recheck()}
 fun recheck(){
  requireOriginalOrdinaryMintFundingProviderV1(original)
  requireOrdinaryRuntimeJniOwnerClassV1(funding.javaClass)
  requireOrdinaryRuntimeJniOwnerClassV1(metadata.javaClass)
  check(funding===metadata){"Mint funding and completed key metadata have different original JNI owners"}
 }
}
private fun requireOriginalOrdinaryMintFundingProviderV1(provider:KagemushaOrdinaryMintFundingNativeOwnerV1) {
 val actual=provider.javaClass
 check(actual.name=="org.hyperledger.iroha.sdk.offline.KagemushaOrdinaryMintFundingNativeProviderV1" &&
  (actual.modifiers and 0x0010)!=0 && actual.classLoader===KagemushaCoreCoordinatorBridgeV1::class.java.classLoader){
  "The original Mint funding Native provider was substituted"
 }
}
internal fun originalOrdinaryMintFundingNativeV1(providers:Iterator<KagemushaOrdinaryMintFundingNativeOwnerV1>):OriginalOrdinaryMintFundingNativeV1 {
 check(providers.hasNext()){"The original Mint funding Native provider is not packaged"}
 val original=providers.next()
 check(!providers.hasNext()){"Ambiguous Mint funding Native providers"}
 requireOriginalOrdinaryMintFundingProviderV1(original) // Before invoking either SPI method.
 return OriginalOrdinaryMintFundingNativeV1(original)
}
