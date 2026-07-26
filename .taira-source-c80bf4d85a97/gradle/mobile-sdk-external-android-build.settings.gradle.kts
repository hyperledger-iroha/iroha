import java.nio.file.Files
import java.nio.file.LinkOption
import java.nio.file.Path

/*
 * A reviewed mobile Release supplies one canonical external Android artifact
 * root. Redirect every project in each included Iroha build below that root so
 * a read-only reviewed source mount never receives Gradle or native outputs.
 * Debug/developer builds retain Gradle's normal local build directories when
 * the variable is absent.
 */
val mobileSdkAndroidArtifactDirectory =
    providers.environmentVariable("MOBILE_SDK_ANDROID_ARTIFACT_DIR").orNull

if (mobileSdkAndroidArtifactDirectory != null) {
    require(mobileSdkAndroidArtifactDirectory.isNotEmpty()) {
        "MOBILE_SDK_ANDROID_ARTIFACT_DIR must not be empty"
    }
    val suppliedRoot = Path.of(mobileSdkAndroidArtifactDirectory)
    require(suppliedRoot.isAbsolute) {
        "MOBILE_SDK_ANDROID_ARTIFACT_DIR must be absolute"
    }
    val normalizedRoot = suppliedRoot.normalize()
    require(normalizedRoot.toString() == mobileSdkAndroidArtifactDirectory) {
        "MOBILE_SDK_ANDROID_ARTIFACT_DIR must be normalized and canonical"
    }
    val canonicalRoot = normalizedRoot.toRealPath()
    require(canonicalRoot == normalizedRoot) {
        "MOBILE_SDK_ANDROID_ARTIFACT_DIR must not traverse symbolic links"
    }
    require(
        Files.isDirectory(canonicalRoot, LinkOption.NOFOLLOW_LINKS) &&
            !Files.isSymbolicLink(canonicalRoot) &&
            Files.isWritable(canonicalRoot),
    ) {
        "MOBILE_SDK_ANDROID_ARTIFACT_DIR must be a writable non-symbolic directory"
    }

    var reviewedSourceRoot = settingsDir.toPath().toRealPath()
    while (
        !Files.exists(reviewedSourceRoot.resolve(".git"), LinkOption.NOFOLLOW_LINKS) &&
        reviewedSourceRoot.parent != null
    ) {
        reviewedSourceRoot = reviewedSourceRoot.parent
    }
    require(Files.exists(reviewedSourceRoot.resolve(".git"), LinkOption.NOFOLLOW_LINKS)) {
        "Unable to locate the reviewed Iroha source root"
    }
    require(
        canonicalRoot != reviewedSourceRoot &&
            !canonicalRoot.startsWith(reviewedSourceRoot),
    ) {
        "MOBILE_SDK_ANDROID_ARTIFACT_DIR must be outside the reviewed Iroha source tree"
    }

    val buildNamespace = rootProject.name
    require(Regex("^[A-Za-z0-9._-]+$").matches(buildNamespace)) {
        "Iroha Gradle build namespace is not path-safe: $buildNamespace"
    }
    val externalProjectRoot = canonicalRoot
        .resolve("gradle-build")
        .resolve(buildNamespace)
    gradle.beforeProject {
        val relativeProjectPath = if (path == ":") {
            "root"
        } else {
            path.removePrefix(":").replace(':', '/')
        }
        layout.buildDirectory.set(
            externalProjectRoot.resolve(relativeProjectPath).toFile()
        )
    }
}
