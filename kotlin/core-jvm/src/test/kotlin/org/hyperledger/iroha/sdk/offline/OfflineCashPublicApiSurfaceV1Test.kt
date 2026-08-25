package org.hyperledger.iroha.sdk.offline

import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/** Locks the Kotlin source API to Offline Cash V1 while retaining Kagemusha internals. */
class OfflineCashPublicApiSurfaceV1Test {
    @Test
    fun `legacy Kagemusha and Offline V2 declarations are internal`() {
        val sourceDirectory = repositoryRoot()
            .resolve("kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline")
        val legacyFiles = listOf(
            "DeviceAttestationRegistration.kt",
            "KagemushaDeviceAuthority.kt",
            "KagemushaNearby.kt",
            "KagemushaNfcProtocol.kt",
            "KagemushaPeerTransport.kt",
            "KagemushaQrStream.kt",
            "KagemushaRecursiveSpendProver.kt",
            "KagemushaScaledAmount.kt",
            "OfflineAndroidAttestedDevicePropertiesV2.kt",
            "RegisterOfflineDeviceAttestation.kt",
        )
        val defaultPublicDeclaration = Regex(
            "(?m)^(?:class|data class|enum class|sealed class|object|interface|" +
                "typealias|value class)\\s+",
        )

        for (name in legacyFiles) {
            val source = sourceDirectory.resolve(name).toFile().readText(Charsets.UTF_8)
            assertFalse(
                defaultPublicDeclaration.containsMatchIn(source),
                "$name must not contain default-public top-level declarations",
            )
        }
        val peerWire = sourceDirectory.resolve("IrohaPeerWireV1.kt")
            .toFile()
            .readText(Charsets.UTF_8)
        assertTrue(peerWire.contains("internal object IrohaPeerKagemushaAdapterV1"))
    }

    @Test
    fun `kgm2 Offline Cash V1 remains the public Kotlin peer API`() {
        val source = repositoryRoot().resolve(
                "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/OfflineCashV1.kt",
            )
            .toFile()
            .readText(Charsets.UTF_8)
        for (declaration in listOf(
            "class OfflineCashPaymentRequestV1",
            "class OfflineCashPaymentV1",
            "class OfflineCashAcknowledgementV1",
            "class OfflineCashReleaseStatusV1",
            "class OfflineCashWalletSessionV1",
            "object OfflineCashPeerAdapterV1",
        )) {
            assertTrue(source.contains(declaration), "missing $declaration")
        }
        assertEquals("kgm2:", OfflineCashPeerAdapterV1.TEXT_PREFIX)
        assertFalse(source.contains("PKK2"))
        assertFalse(source.contains("PKKQ1"))
    }

    private fun repositoryRoot(): Path {
        var cursor: Path? = Paths.get(System.getProperty("user.dir")).toAbsolutePath()
        while (cursor != null) {
            if (Files.isDirectory(cursor.resolve("kotlin/core-jvm"))) return cursor
            cursor = cursor.parent
        }
        error("repository root containing kotlin/core-jvm was not found")
    }
}
