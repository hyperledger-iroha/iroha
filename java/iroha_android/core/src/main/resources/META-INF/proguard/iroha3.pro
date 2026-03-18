# ==================== Iroha3 Android SDK (i23) ====================
# This module uses reflection extensively. All reflection targets must be preserved.

# Keep all SDK public APIs and internal classes
-keep class org.hyperledger.iroha.android.** { *; }
-keep interface org.hyperledger.iroha.android.** { *; }

# Keep Norito codec library (uses reflection for field extraction in NoritoAdapters)
-keep class org.hyperledger.iroha.norito.** { *; }
-keep interface org.hyperledger.iroha.norito.** { *; }

# ==================== Reflection Targets ====================

# Android Keystore - accessed via reflection in SystemAndroidKeystoreBackend.java
# Classes: KeyGenParameterSpec$Builder, KeyProperties (getField: PURPOSE_SIGN, PURPOSE_VERIFY, DIGEST_NONE)
-keep class android.security.keystore.KeyGenParameterSpec$Builder { *; }
-keep class android.security.keystore.KeyProperties { *; }

# Android Build - accessed via reflection for runtime detection
# Files: SystemAndroidKeystoreBackend.java:75, AndroidDeviceProfileProvider.java:24
-keep class android.os.Build { *; }

# Android Network APIs - accessed via reflection in AndroidNetworkContextProvider.java
-keep class android.content.Context {
    public java.lang.Object getSystemService(java.lang.String);
    public static final java.lang.String CONNECTIVITY_SERVICE;
}
-keep class android.net.ConnectivityManager {
    public android.net.NetworkInfo getActiveNetworkInfo();
}
-keep class android.net.NetworkInfo {
    public boolean isConnected();
    public java.lang.String getTypeName();
    public boolean isRoaming();
}

# BouncyCastle provider - accessed via reflection in SoftwareKeyProvider.java and Blake2b.java
-keep class org.bouncycastle.jce.provider.BouncyCastleProvider {
    public <init>();
}

# BouncyCastle Argon2 KDF - accessed via reflection in DeterministicKeyExporter.java
-keep class org.bouncycastle.crypto.params.Argon2Parameters { *; }
-keep class org.bouncycastle.crypto.params.Argon2Parameters$Builder { *; }
-keep class org.bouncycastle.crypto.generators.Argon2BytesGenerator { *; }

# ==================== Suppress Warnings ====================

# Optional Android dependencies (SDK can run on desktop JVM without these)
-dontwarn android.security.**
-dontwarn android.content.**
-dontwarn android.net.**
-dontwarn android.os.**

# BouncyCastle (optional fallback crypto provider)
-dontwarn org.bouncycastle.**

# Zstd compression (optional, loaded via reflection in NoritoCompression.java)
-keep class com.github.luben.zstd.Zstd { *; }
-dontwarn com.github.luben.zstd.**

# ==================== Notes for SDK Consumers ====================
#
# The rules above keep the SDK’s own reflection targets. If your app passes custom
# POJOs into Norito adapters or other SDK APIs that use reflection, add app-level
# keeps for those model classes so R8 does not rename/strip their getters/fields.
