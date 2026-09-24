# Iroha SDK — consumer ProGuard/R8 rules
#
# These rules are bundled into the AAR and applied automatically
# when the consuming app enables minification.

# BouncyCastle is a direct core-jvm dependency. Its JCA provider discovers
# algorithm implementations dynamically, so retain the provider entry point.
-dontwarn org.bouncycastle.**
-keep class org.bouncycastle.jce.provider.BouncyCastleProvider { *; }

# These exact owner/method names bind the coordinator's exported JNI symbols.
-keep class org.hyperledger.iroha.sdk.offline.KagemushaDeviceLifecycleBridgeV1$NativeEndpoint {
    native <methods>;
}
-keep class org.hyperledger.iroha.sdk.offline.KagemushaCoreCoordinatorJniV1 {
    native <methods>;
}

# Raw testnet diagnostics have their own fixed Rust JNI symbol names. Keep their
# owners even when a consuming app minifies its release build.
-keep class org.hyperledger.iroha.sdk.offline.probe.KagemushaTestnetStateProofObservationJniV1 {
    native <methods>;
}
-keep class org.hyperledger.iroha.sdk.offline.probe.Pixel6TestnetDiagnosticSelectionJniV1 {
    native <methods>;
}
