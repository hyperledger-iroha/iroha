package org.hyperledger.iroha.sdk.privacy

import java.io.File
import java.security.MessageDigest
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue

class PrivacyNativeBridgeTest {
    private val matrix = loadExact12Matrix()
    private val protocolRows = matrix.filter { it.first() == "protocol" }
    private val typedEnvelopeRows = matrix.filter { it.first() == "typed-envelope" }
    private val retired = matrix.filter { it.first() == "retired" }.map { it[1] }
    private val expected = protocolRows.map { it[2] }

    @Test
    fun exactClosedRegistryIsStable() {
        assertEquals(21, PrivacyNativeBridge.REQUIRED_BRIDGE_ABI_VERSION)
        assertEquals(expected, PrivacyNativeBridge.protocolsV1().map { it.canonicalLabel })
        assertEquals(12, PrivacyNativeBridge.protocolsV1().size)
        expected.forEachIndexed { index, label ->
            assertEquals(
                PrivacyNativeBridge.protocolsV1()[index],
                PrivacyNativeBridge.ProtocolIdV1.fromCanonicalLabel(label),
            )
        }
    }

    @Test
    fun sharedExact12MatrixBindsRoutesAndTypedEnvelopeDigests() {
        assertEquals(
            setOf("matrix-version", "registry-sha256", "protocol", "typed-envelope", "retired"),
            matrix.map { it[0] }.toSet(),
        )
        assertEquals(listOf(listOf("matrix-version", "1")), matrix.filter { it[0] == "matrix-version" })
        assertEquals((0 until 12).map(Int::toString), protocolRows.map { it[1] })
        assertEquals(12, expected.toSet().size)
        val registryPreimage = expected.joinToString(separator = "", postfix = "") { "$it\n" }
        val registryDigest =
            MessageDigest
                .getInstance("SHA-256")
                .digest(registryPreimage.toByteArray(Charsets.UTF_8))
                .joinToString("") { "%02x".format(it) }
        assertEquals(
            listOf(listOf("registry-sha256", registryDigest)),
            matrix.filter { it[0] == "registry-sha256" },
        )
        assertEquals(
            protocolRows.map { it.subList(2, 5) },
            typedEnvelopeRows.map { it.subList(1, 4) },
        )
        assertEquals(12, typedEnvelopeRows.size)
        typedEnvelopeRows.forEach { row ->
            assertEquals(6, row.size)
            row.drop(4).forEach { digest ->
                assertEquals(true, digest.matches(Regex("[0-9a-f]{64}")))
                assertEquals(false, digest.all { it == '0' })
            }
        }
        assertEquals(retired.size, retired.toSet().size)
        assertEquals(true, retired.none(expected::contains))
    }

    @Test
    fun aliasesAndNonCanonicalSpellingsAreRejected() {
        (
            retired +
                listOf(
                    "iroha-zk-ams-v1 ",
                    "Iroha-Zk-Ams-V1",
                    "",
                    "unknown-privacy-protocol-v1",
                )
        ).forEach { rejected ->
            assertFailsWith<IllegalArgumentException> {
                PrivacyNativeBridge.ProtocolIdV1.fromCanonicalLabel(rejected)
            }
        }
    }

    private fun loadExact12Matrix(): List<List<String>> {
        val fixture =
            generateSequence(File(".").canonicalFile) { it.parentFile }
                .map { File(it, "fixtures/privacy/exact12_v1.tsv") }
                .firstOrNull(File::isFile)
                ?: error("cannot locate fixtures/privacy/exact12_v1.tsv")
        val text = fixture.readText(Charsets.UTF_8)
        check(text.endsWith("\n") && '\r' !in text) { "exact12 fixture is not canonical LF text" }
        check(text.dropLast(1).lineSequence().none(String::isEmpty)) {
            "exact12 fixture contains an empty row"
        }
        return text
            .lineSequence()
            .filter { it.isNotEmpty() && !it.startsWith("#") }
            .map { it.split('\t') }
            .toList()
    }

    @Test
    fun sharedTypedValidatorStatusContractIsStable() {
        assertEquals(
            256 * 1024,
            PrivacyNativeBridge.COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_BYTES,
        )
        assertEquals(
            2 * 1024 * 1024,
            PrivacyNativeBridge.EXACT12_FIXTURE_BUNDLE_MAX_BYTES,
        )
        assertEquals(
            (0..8).toList(),
            PrivacyNativeBridge.CompiledProfileCatalogValidationStatusV1.values().map { it.code },
        )
        assertEquals(
            (0..8).toList(),
            PrivacyNativeBridge.Exact12FixtureValidationStatusV1.values().map { it.code },
        )
        val validator =
            PrivacyNativeBridge::class.java.declaredMethods.single {
                it.name == "nativeValidateCompiledProfileCatalog"
            }
        assertEquals(true, java.lang.reflect.Modifier.isNative(validator.modifiers))
        assertEquals(Int::class.javaPrimitiveType, validator.returnType)
        val fixtureQuery =
            PrivacyNativeBridge::class.java.declaredMethods.single {
                it.name == "nativeExact12FixtureBundle"
            }
        val fixtureValidator =
            PrivacyNativeBridge::class.java.declaredMethods.single {
                it.name == "nativeValidateExact12FixtureBundle"
            }
        assertEquals(true, java.lang.reflect.Modifier.isNative(fixtureQuery.modifiers))
        assertEquals(ByteArray::class.java, fixtureQuery.returnType)
        assertEquals(true, java.lang.reflect.Modifier.isNative(fixtureValidator.modifiers))
        assertEquals(Int::class.javaPrimitiveType, fixtureValidator.returnType)
    }

    @Test
    fun compiledProfileCatalogPreflightRejectsNullEmptyAndOversizeWithoutNativeCalls() {
        assertEquals(
            PrivacyNativeBridge.CompiledProfileCatalogValidationStatusV1.NULL_POINTER,
            PrivacyNativeBridge.validateCompiledProfileCatalogV1(null),
        )
        assertEquals(
            PrivacyNativeBridge.CompiledProfileCatalogValidationStatusV1.EMPTY,
            PrivacyNativeBridge.validateCompiledProfileCatalogV1(byteArrayOf()),
        )
        assertEquals(
            PrivacyNativeBridge.CompiledProfileCatalogValidationStatusV1.ARCHIVE_TOO_LARGE,
            PrivacyNativeBridge.validateCompiledProfileCatalogV1(
                ByteArray(PrivacyNativeBridge.COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_BYTES + 1),
            ),
        )
    }

    @Test
    fun compiledProfileCatalogRoundTripsAndRejectsAdversarialBytes() {
        assertTrue(
            PrivacyNativeBridge.isNativeAvailable(),
            "ABI-21 connect_norito_bridge with compiled-profile catalog JNI exports is required",
        )

        val canonical = PrivacyNativeBridge.compiledProfileCatalogV1()
        assertTrue(canonical.isNotEmpty())
        assertTrue(
            canonical.size <= PrivacyNativeBridge.COMPILED_PROFILE_CATALOG_ARCHIVE_MAX_BYTES,
        )
        assertEquals(
            PrivacyNativeBridge.CompiledProfileCatalogValidationStatusV1.VALID,
            PrivacyNativeBridge.validateCompiledProfileCatalogV1(canonical),
        )
        assertContentEquals(canonical, PrivacyNativeBridge.compiledProfileCatalogV1())

        listOf(
            canonical.copyOfRange(0, canonical.size - 1),
            canonical.copyOfRange(1, canonical.size),
            canonical.copyOfRange(0, canonical.size / 2),
        ).forEach { truncated ->
            assertNotEquals(
                PrivacyNativeBridge.CompiledProfileCatalogValidationStatusV1.VALID,
                PrivacyNativeBridge.validateCompiledProfileCatalogV1(truncated),
            )
            assertFailsWith<IllegalStateException> {
                PrivacyNativeBridge.requireCompiledProfileCatalog(truncated)
            }
        }
        assertNotEquals(
            PrivacyNativeBridge.CompiledProfileCatalogValidationStatusV1.VALID,
            PrivacyNativeBridge.validateCompiledProfileCatalogV1(canonical + byteArrayOf(0)),
        )
        setOf(0, canonical.size / 2, canonical.size - 1).forEach { index ->
            val mutated = canonical.copyOf()
            mutated[index] = (mutated[index].toInt() xor 0x80).toByte()
            assertNotEquals(
                PrivacyNativeBridge.CompiledProfileCatalogValidationStatusV1.VALID,
                PrivacyNativeBridge.validateCompiledProfileCatalogV1(mutated),
            )
        }
    }

    @Test
    fun exact12FixturePreflightRejectsNullEmptyAndOversizeWithoutNativeCalls() {
        assertEquals(
            PrivacyNativeBridge.Exact12FixtureValidationStatusV1.NULL_POINTER,
            PrivacyNativeBridge.validateExact12FixtureBundleV1(null),
        )
        assertEquals(
            PrivacyNativeBridge.Exact12FixtureValidationStatusV1.EMPTY,
            PrivacyNativeBridge.validateExact12FixtureBundleV1(byteArrayOf()),
        )
        assertEquals(
            PrivacyNativeBridge.Exact12FixtureValidationStatusV1.ARCHIVE_TOO_LARGE,
            PrivacyNativeBridge.validateExact12FixtureBundleV1(
                ByteArray(PrivacyNativeBridge.EXACT12_FIXTURE_BUNDLE_MAX_BYTES + 1),
            ),
        )
    }

    @Test
    fun exact12FixtureBundleRoundTripsAndRejectsAdversarialBytes() {
        assertTrue(
            PrivacyNativeBridge.isNativeAvailable(),
            "ABI-21 connect_norito_bridge with exact-12 fixture JNI exports is required",
        )

        val fetched = PrivacyNativeBridge.exact12FixtureBundleV1()
        val canonical = fetched.copyOf()
        assertTrue(canonical.isNotEmpty())
        assertTrue(canonical.size <= PrivacyNativeBridge.EXACT12_FIXTURE_BUNDLE_MAX_BYTES)
        assertEquals(
            PrivacyNativeBridge.Exact12FixtureValidationStatusV1.VALID,
            PrivacyNativeBridge.validateExact12FixtureBundleV1(canonical),
        )
        assertContentEquals(canonical, PrivacyNativeBridge.exact12FixtureBundleV1())

        fetched[0] = (fetched[0].toInt() xor 0xff).toByte()
        assertContentEquals(canonical, PrivacyNativeBridge.exact12FixtureBundleV1())

        listOf(
            canonical.copyOfRange(0, canonical.size - 1),
            canonical.copyOfRange(1, canonical.size),
            canonical.copyOfRange(0, canonical.size / 2),
        ).forEach { truncated ->
            assertNotEquals(
                PrivacyNativeBridge.Exact12FixtureValidationStatusV1.VALID,
                PrivacyNativeBridge.validateExact12FixtureBundleV1(truncated),
            )
            assertFailsWith<IllegalStateException> {
                PrivacyNativeBridge.requireExact12FixtureBundle(truncated)
            }
        }

        assertNotEquals(
            PrivacyNativeBridge.Exact12FixtureValidationStatusV1.VALID,
            PrivacyNativeBridge.validateExact12FixtureBundleV1(
                canonical + byteArrayOf(0),
            ),
        )
        setOf(0, canonical.size / 2, canonical.size - 1).forEach { index ->
            val mutated = canonical.copyOf()
            mutated[index] = (mutated[index].toInt() xor 0x80).toByte()
            assertNotEquals(
                PrivacyNativeBridge.Exact12FixtureValidationStatusV1.VALID,
                PrivacyNativeBridge.validateExact12FixtureBundleV1(mutated),
            )
        }

        assertNotEquals(
            PrivacyNativeBridge.Exact12FixtureValidationStatusV1.VALID,
            PrivacyNativeBridge.validateExact12FixtureBundleV1(
                PrivacyNativeBridge.compiledProfileCatalogV1(),
            ),
        )
    }

    @Test
    fun retiredGenericProofSurfaceIsAbsent() {
        PrivacyNativeBridge::class.java.declaredMethods.forEach { method ->
            val name = method.name.lowercase()
            check(!name.contains("proofrequest")) { method.name }
            check(!name.contains("buildproof")) { method.name }
            check(!name.contains("verifyproof")) { method.name }
        }
    }

}
