# Iroha SDK Offline Wallet — consumer ProGuard/R8 rules

# JNI natives (loaded via System.loadLibrary)
-keep class org.hyperledger.iroha.sdk.offline.OfflineBalanceProof { native <methods>; }
-keep class org.hyperledger.iroha.sdk.offline.OfflineReceiptChallenge { native <methods>; }
-keep class org.hyperledger.iroha.sdk.offline.OfflineSpendReceiptPayloadEncoder { native <methods>; }
-keep class org.hyperledger.iroha.sdk.offline.OfflineBuildClaimPayloadEncoder { native <methods>; }
