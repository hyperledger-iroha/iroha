# Iroha KAGEMUSHA Wallet SDK — consumer ProGuard/R8 rules
#
# ServiceLoader resolves these provider names from META-INF/services. Preserve the generic
# adapter and every qualified runtime-supplied native-Core coordinator factory constructor.
-keep class org.hyperledger.iroha.sdk.offline.wallet.KagemushaAndroidAuthenticatedHardwareProviderFactoryV1 {
    public <init>();
}
-keepnames interface org.hyperledger.iroha.sdk.offline.wallet.KagemushaAndroidHardwareProviderFactoryV1
-keepnames interface org.hyperledger.iroha.sdk.offline.KagemushaNativeCoreCoordinatorFactoryV1
-keep class * implements org.hyperledger.iroha.sdk.offline.KagemushaNativeCoreCoordinatorFactoryV1 {
    public <init>();
}
