plugins {
    id("com.android.application") version "9.0.1" apply false
    id("org.jetbrains.kotlin.android") version "2.4.21" apply false
}

subprojects {
    tasks.withType<Test>().configureEach {
        providers.environmentVariable("IROHA_NATIVE_LIBRARY_PATH").orNull?.let { nativeDirectory ->
            jvmArgs("-Djava.library.path=$nativeDirectory")
            inputs.files(fileTree(nativeDirectory) {
                include("*.dylib", "*.so", "*.dll")
            })
        }
    }
}
