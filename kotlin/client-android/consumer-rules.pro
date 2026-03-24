# Iroha SDK — consumer ProGuard/R8 rules
#
# These rules are bundled into the AAR and applied automatically
# when the consuming app enables minification.

# BouncyCastle (optional — used by SoftwareKeyProvider and DeterministicKeyExporter)
# Only needed if the app uses BouncyCastle as a JCE provider or Argon2 KDF.
-dontwarn org.bouncycastle.**
-keep class org.bouncycastle.jce.provider.BouncyCastleProvider { *; }
-keep class org.bouncycastle.crypto.generators.Argon2BytesGenerator { *; }
-keep class org.bouncycastle.crypto.params.Argon2Parameters { *; }
-keep class org.bouncycastle.crypto.params.Argon2Parameters$Builder { *; }

# Platform transport discovery (optional — loaded via Class.forName)
-keep class org.hyperledger.iroha.sdk.client.okhttp.OkHttpTransportExecutorFactory { *; }
-keep class org.hyperledger.iroha.sdk.client.JavaHttpExecutorFactory { *; }
-keep class org.hyperledger.iroha.sdk.client.okhttp.OkHttpWebSocketConnectorFactory { *; }
-keep class org.hyperledger.iroha.sdk.client.websocket.JdkWebSocketConnectorFactory { *; }
