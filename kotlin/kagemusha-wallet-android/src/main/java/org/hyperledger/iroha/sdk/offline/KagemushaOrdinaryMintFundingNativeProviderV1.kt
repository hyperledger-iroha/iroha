// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline
/** Sole stateless packaging provider of the fixed measured wallet JNI. No account, clock,
 * platform callback or caller-original factory is accepted; Native owns every retained effect.
 */
class KagemushaOrdinaryMintFundingNativeProviderV1:KagemushaOrdinaryMintFundingNativeOwnerV1 {
 override fun fundingEndpoint():KagemushaOrdinaryNativeMintFundingEndpointV1=KagemushaOrdinaryRuntimeJniV1
 override fun completedMetadataEndpoint():KagemushaOrdinaryNativeIntegrityRefreshEndpointV1=KagemushaOrdinaryRuntimeJniV1
}
