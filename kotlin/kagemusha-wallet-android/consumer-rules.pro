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

# The maintained native reserve verifier resolves this exact Kotlin JNI class.
-keep class org.hyperledger.iroha.sdk.offline.wallet.KagemushaReserveFinalityJniV1 {
    native <methods>;
}

-keep class org.hyperledger.iroha.sdk.offline.wallet.KagemushaTopUpSubmissionJniV1 { *; }

-keep class org.hyperledger.iroha.sdk.offline.wallet.KagemushaTestnetNativeStartupJniV1 {
    native <methods>;
}

# Exact shared ordinary startup/current-FI and Application/retirement JNI names must survive R8.
# nativeBindApplicationV1(android.app.Application): boolean retains identity only;
# nativeRetireOriginalV1(): boolean is deny-only and accepts no account/root/secret frame.
-keep class org.hyperledger.iroha.sdk.offline.KagemushaOrdinaryRuntimeJniV1 {
    native <methods>;
}
