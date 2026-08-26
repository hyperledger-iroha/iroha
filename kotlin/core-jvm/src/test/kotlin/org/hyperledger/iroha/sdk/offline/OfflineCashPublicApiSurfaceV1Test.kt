package org.hyperledger.iroha.sdk.offline

import java.lang.reflect.Modifier
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

    @Test
    fun `Offline Cash V1 Torii facade is public without Kagemusha type leakage`() {
        val sourceDirectory = repositoryRoot().resolve(
            "kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline",
        )
        val source = listOf(
            "OfflineCashToriiV1.kt",
            "OfflineCashReadinessBlockerV1.java",
            "OfflineCashReadinessV1.java",
            "OfflineCashOperationRejectionV1.java",
            "OfflineCashFinalizedTopUpV1.java",
            "OfflineCashOperationStatusProjectionV1.java",
        ).joinToString("\n") { name ->
            sourceDirectory.resolve(name).toFile().readText(Charsets.UTF_8)
        }
        for (declaration in listOf(
            "class OfflineCashTopUpRequestV1",
            "class OfflineCashRedeemRequestV1",
            "class OfflineCashReadinessBlockerV1",
            "class OfflineCashReadinessV1",
            "class OfflineCashOperationReferenceV1",
            "enum class OfflineCashOperationStateV1",
            "enum class OfflineCashOperationKindV1",
            "class OfflineCashOperationRejectionV1",
            "class OfflineCashFinalizedTopUpV1",
            "class OfflineCashOperationStatusProjectionV1",
            "class OfflineCashOperationStatusV1",
            "class OfflineCashToriiClientV1",
        )) {
            assertTrue(source.contains(declaration), "missing $declaration")
        }

        assertEquals("/v1/offline/readiness", OfflineCashToriiClientV1.READINESS_PATH)
        assertEquals("/v1/offline/top-up", OfflineCashToriiClientV1.TOP_UP_PATH)
        assertEquals("/v1/offline/redeem", OfflineCashToriiClientV1.REDEEM_PATH)
        assertEquals("/v1/offline/operations", OfflineCashToriiClientV1.OPERATIONS_PATH)

        val publicTypes = listOf(
            OfflineCashTopUpRequestV1::class.java,
            OfflineCashRedeemRequestV1::class.java,
            OfflineCashReadinessBlockerV1::class.java,
            OfflineCashReadinessV1::class.java,
            OfflineCashOperationReferenceV1::class.java,
            OfflineCashOperationStateV1::class.java,
            OfflineCashOperationKindV1::class.java,
            OfflineCashOperationRejectionV1::class.java,
            OfflineCashFinalizedTopUpV1::class.java,
            OfflineCashOperationStatusProjectionV1::class.java,
            OfflineCashOperationStatusV1::class.java,
            OfflineCashToriiClientV1::class.java,
        )
        val exposedSignatures = buildList {
            for (type in publicTypes) {
                type.constructors
                    .filter { constructor -> Modifier.isPublic(constructor.modifiers) }
                    .flatMapTo(this) { constructor ->
                        constructor.genericParameterTypes.map { parameter -> parameter.typeName }
                    }
                type.methods
                    .filter { method ->
                        Modifier.isPublic(method.modifiers) && method.declaringClass == type
                    }
                    .forEach { method ->
                        add(method.genericReturnType.typeName)
                        method.genericParameterTypes.mapTo(this) { parameter -> parameter.typeName }
                    }
                type.fields
                    .filter { field -> Modifier.isPublic(field.modifiers) }
                    .mapTo(this) { field -> field.genericType.typeName }
            }
        }
        val leakedSignatures = exposedSignatures.filter { signature ->
            signature.contains("KagemushaRecursiveSpendProver")
        }
        assertTrue(
            leakedSignatures.isEmpty(),
            "public Offline Cash V1 signatures expose internal types: $leakedSignatures",
        )

        val nativeValidatedProjectionTypes = listOf(
            OfflineCashReadinessBlockerV1::class.java,
            OfflineCashReadinessV1::class.java,
            OfflineCashOperationRejectionV1::class.java,
            OfflineCashFinalizedTopUpV1::class.java,
            OfflineCashOperationStatusProjectionV1::class.java,
        )
        for (type in nativeValidatedProjectionTypes) {
            assertTrue(
                type.constructors.isEmpty(),
                "${type.name} must not expose a public constructor that bypasses validation",
            )
        }
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
