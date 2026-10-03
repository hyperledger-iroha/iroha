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

# Exact shared ordinary startup/current-FI, incoming, Mint funding and Application/retirement JNI names must survive R8.
# nativeBindApplicationV1(android.app.Application): boolean retains identity only;
# nativeRetireOriginalV1(): boolean is deny-only and accepts no account/root/secret frame.
-keep class org.hyperledger.iroha.sdk.offline.KagemushaOrdinaryRuntimeJniV1 {
    native <methods>;
}

# Native creates this exact private final continuation through the measured product loader.
# It is not a provider or a caller-supplied key constructor. Keep only its fixed loan grammar.
-keep class org.hyperledger.iroha.sdk.offline.KagemushaOrdinaryExistingAccountIntakeV1 {
    private <init>();
    public void consumeOriginal(java.lang.String, byte[]);
}

# Sole module-safe funding provider. No alternate endpoint/fields constructor is public.
-keepnames interface org.hyperledger.iroha.sdk.offline.KagemushaOrdinaryMintFundingNativeOwnerV1
-keep class org.hyperledger.iroha.sdk.offline.KagemushaOrdinaryMintFundingNativeProviderV1 {
    public <init>();
}
