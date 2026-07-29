package org.hyperledger.iroha.sdk.privacy

import java.io.File
import java.security.MessageDigest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith

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
        assertEquals(256 * 1024, PrivacyNativeBridge.PRIVACY_NATIVE_ARCHIVE_MAX_BYTES)
        assertEquals(
            (0..8).toList(),
            PrivacyNativeBridge.ValidationStatusV1.values().map { it.code },
        )
        val validator =
            PrivacyNativeBridge::class.java.declaredMethods.single {
                it.name == "nativeValidateCapabilities"
            }
        assertEquals(true, java.lang.reflect.Modifier.isNative(validator.modifiers))
        assertEquals(Int::class.javaPrimitiveType, validator.returnType)
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
