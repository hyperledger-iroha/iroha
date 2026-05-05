# Iroha SDK core-jvm — consumer ProGuard/R8 rules
#
# Bundled in the JAR at META-INF/proguard/ and applied automatically
# when consuming Android apps enable minification.

# Zstd-JNI — JNI linking requires class names to match native symbols.
# Renaming any class with native methods breaks native library loading.
-keep,allowoptimization class com.github.luben.zstd.Zstd { native <methods>; }
-keep,allowoptimization class com.github.luben.zstd.ZstdCompressCtx { native <methods>; }
-keep,allowoptimization class com.github.luben.zstd.ZstdDecompressCtx { native <methods>; }
-keep,allowoptimization class com.github.luben.zstd.ZstdDictCompress { native <methods>; }
-keep,allowoptimization class com.github.luben.zstd.ZstdDictDecompress { native <methods>; }
-keep,allowoptimization class com.github.luben.zstd.ZstdInputStreamNoFinalizer { native <methods>; }
-keep,allowoptimization class com.github.luben.zstd.ZstdOutputStreamNoFinalizer { native <methods>; }
-keep,allowoptimization class com.github.luben.zstd.util.Native { native <methods>; }
-dontwarn com.github.luben.zstd.**
