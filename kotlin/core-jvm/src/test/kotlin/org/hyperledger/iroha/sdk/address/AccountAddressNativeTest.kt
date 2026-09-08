package org.hyperledger.iroha.sdk.address

import java.io.File
import java.nio.file.Files
import java.util.concurrent.TimeUnit
import org.bouncycastle.math.ec.rfc8032.Ed25519
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.norito.Varint
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** Shared positive vectors generated and independently decoded by the Rust model owner. */
internal object NativeAccountFixtures {
    val positives: List<Map<*, *>> by lazy {
        val file = generateSequence(File(".").canonicalFile) { it.parentFile }
            .map { File(it, "fixtures/account/multisig_wire_v1.json") }.first(File::isFile)
        val fixture = JsonParser.parse(file.readText(Charsets.UTF_8)) as Map<*, *>
        check(fixture["schema"] == "iroha.account.multisig-wire.v1")
        (fixture["positive"] as List<*>).map { it as Map<*, *> }.also { check(it.size == 16) }
    }

    fun bytes(value: String): ByteArray =
        value.removePrefix("0x").chunked(2).map { it.toInt(16).toByte() }.toByteArray()

    fun singleKey(name: String): ByteArray {
        val canonical = bytes(positives.single { it["name"] == name }["canonical_address_hex"] as String)
        check(canonical[1].toInt() == 0 || canonical[1].toInt() == 2)
        return canonical.copyOfRange(if (canonical[1].toInt() == 0) 4 else 5, canonical.size)
    }
}

/** Every successful public account/key boundary must execute the complete native validator. */
class AccountAddressNativeTest {
    @Test
    fun rawCanonicalInputBoundIsCheckedBeforeCopyingOrNativeAllocation() {
        for (size in listOf(0, -1, 64 * 1024 * 1024 + 1, Int.MAX_VALUE)) {
            val error = assertFailsWith<AccountAddressException> { AccountAddressNative.requireCanonicalSize(size) }
            assertEquals(AccountAddressErrorCode.INVALID_LENGTH, error.code)
        }
        for (size in listOf(1, 64 * 1024 * 1024)) AccountAddressNative.requireCanonicalSize(size)
        for (size in listOf(0, -1, 65536, Int.MAX_VALUE)) {
            val error = assertFailsWith<AccountAddressException> { AccountAddressNative.requireSingleSize(1, size) }
            assertEquals(AccountAddressErrorCode.INVALID_PUBLIC_KEY, error.code)
        }
        for (curve in listOf(-1, 0, 257, 65537)) {
            assertFailsWith<AccountAddressException> { AccountAddressNative.requireSingleSize(curve, 32) }
        }
        for (size in listOf(1, 65535)) AccountAddressNative.requireSingleSize(1, size)
    }

    @Test
    fun multihashLengthsCannotWrapOrTruncateAndMalformedVarintsReturnNull() {
        val keyHex = NativeAccountFixtures.singleKey("ed25519").joinToString("") { "%02x".format(it.toInt() and 255) }
        for (literal in listOf("", "ed01", "ed0180", "ed01a000$keyHex", "ed0100$keyHex",
            "ed01a0808080808080808001$keyHex", "ed01ffffffffffffffffff01$keyHex",
            "ed01a080808010$keyHex", "ed01" + "80".repeat(11) + keyHex,
            "0".repeat(2 * (65535 + 20) + 33))) {
            assertNull(decodePublicKeyLiteral(literal))
        }
        assertNull(decodeCompactPublicKeyPayload(ByteArray(65537)))
    }

    @Test
    fun completeControllersMatchRustThroughEveryPublicAddressEntryPoint() {
        for (vector in NativeAccountFixtures.positives) {
            val name = vector["name"] as String
            val raw = NativeAccountFixtures.bytes(vector["canonical_address_hex"] as String)
            val literal = vector["i105"] as String
            for (address in listOf(AccountAddress.fromCanonicalBytes(raw),
                AccountAddress.fromI105(literal, 753), AccountAddress.parseEncoded(literal, 753))) {
                assertContentEquals(raw, address.canonicalBytes, name)
                assertEquals(literal, address.toI105(753), name)
                val single = address.singleKeyPayload()
                if (single == null) {
                    val policy = assertNotNull(address.multisigPolicyPayload(), name)
                    assertContentEquals(raw, AccountAddress.fromMultisigPolicy(policy).canonicalBytes, name)
                } else {
                    val key = single.publicKey
                    val algorithm = assertNotNull(algorithmForCurveId(single.curveId), name)
                    assertContentEquals(raw, AccountAddress.fromAccount(key, algorithm).canonicalBytes, name)
                    assertContentEquals(key, PublicKeyPayload(single.curveId, key).keyBytes, name)
                    val encoded = encodePublicKeyMultihash(single.curveId, key)
                    assertContentEquals(key, assertNotNull(decodePublicKeyLiteral(encoded), name).keyBytes, name)
                    val compact = compactPublicKeyPayload(single.curveId, key)
                    assertContentEquals(key, assertNotNull(decodeCompactPublicKeyPayload(compact), name).keyBytes, name)
                }
            }
            val snapshot = raw.copyOf()
            val address = AccountAddress.fromCanonicalBytes(raw)
            raw.fill(0)
            address.canonicalBytes.fill(0)
            assertContentEquals(snapshot, address.canonicalBytes, name)
            assertFailsWith<AccountAddressException>(name) { AccountAddress.fromCanonicalBytes(snapshot + 0) }
        }
    }

    @Test
    fun malformedKeysCannotEnterAddressesPoliciesOrPublicKeyCodecs() {
        val badKeys = mutableListOf(
            1 to (byteArrayOf(1) + ByteArray(31)),
            1 to ByteArray(32) { 0xff.toByte() },
            4 to (byteArrayOf(2) + ByteArray(32) { 0xff.toByte() }),
            3 to ByteArray(48) { 0xff.toByte() },
            5 to ByteArray(96) { 0xff.toByte() },
            2 to ByteArray(1952),
            15 to (byteArrayOf(0, 0, 4) + ByteArray(64) { 0xff.toByte() }),
        )
        for (curve in 10..14) badKeys += curve to ByteArray(if (curve < 13) 64 else 128) { 0xff.toByte() }
        assertEquals(12, badKeys.size)
        val codes = mapOf(1 to 0xedL, 2 to 0xeeL, 3 to 0xeaL, 4 to 0xe7L, 5 to 0xebL,
            10 to 0x1200L, 11 to 0x1201L, 12 to 0x1202L, 13 to 0x1203L, 14 to 0x1204L, 15 to 0x1306L)
        val tags = mapOf(1 to 0, 4 to 1, 3 to 2, 5 to 3, 2 to 4, 10 to 5, 11 to 6,
            12 to 7, 13 to 8, 14 to 9, 15 to 10)
        for ((curve, key) in badKeys) {
            val length = if (key.size <= 255) byteArrayOf(key.size.toByte())
                else byteArrayOf((key.size ushr 8).toByte(), key.size.toByte())
            val canonical = byteArrayOf(2, if (key.size <= 255) 0 else 2, curve.toByte()) + length + key
            val algorithm = assertNotNull(algorithmForCurveId(curve))
            for (construct in listOf<() -> Unit>(
                { AccountAddress.fromCanonicalBytes(canonical) },
                { AccountAddress.fromAccount(key, algorithm) },
                { PublicKeyPayload(curve, key) },
                { encodePublicKeyMultihash(curve, key) },
                { compactPublicKeyPayload(curve, key) },
                { AccountAddress.fromMultisigPolicy(MultisigPolicyPayload.of(1, 1,
                    listOf(MultisigMemberPayload(curve, 1, key)))) },
            )) {
                val error = assertFailsWith<AccountAddressException>("curve $curve", construct)
                assertEquals(AccountAddressErrorCode.INVALID_PUBLIC_KEY, error.code, "curve $curve")
            }
            val multihash = Varint.encode(codes.getValue(curve)) + Varint.encode(key.size.toLong()) + key
            assertNull(decodePublicKeyLiteral(multihash.joinToString("") { "%02x".format(it.toInt() and 255) }))
            assertNull(decodeCompactPublicKeyPayload(byteArrayOf(tags.getValue(curve).toByte()) + key))
        }
    }

    @Test
    fun accountLiteralsRejectAsciiAndUnicodePaddingWithoutNormalization() {
        val literal = NativeAccountFixtures.positives.first()["i105"] as String
        for (padding in listOf(" ", "\t", "\n", "\u00a0", "\u2003", "\u3000")) {
            for (value in listOf(padding + literal, literal + padding)) {
                assertFailsWith<AccountAddressException> { AccountAddress.fromI105(value, 753) }
                assertFailsWith<AccountAddressException> { AccountAddress.parseEncoded(value, 753) }
            }
        }
    }
}

/** Tests native absence in a fresh JVM, independently of the parent JVM's loaded library. */
class AccountAddressNativeUnavailableTest {
    @Test
    fun everyPublicAdmissionBoundaryFailsExplicitlyWithoutNativeOwner() {
        val directory = Files.createTempDirectory("iroha-account-native-absent-").toFile()
        try {
            val output = File(directory, "probe.log")
            val classpath = listOf(AccountAddressNativeAbsenceProbe::class.java, AccountAddress::class.java,
                Unit::class.java, Ed25519::class.java).map {
                File(it.protectionDomain.codeSource.location.toURI()).absolutePath
            }.distinct().joinToString(File.pathSeparator)
            val process = ProcessBuilder(File(System.getProperty("java.home"), "bin/java").absolutePath,
                "-Djava.library.path=${directory.absolutePath}", "-cp", classpath,
                AccountAddressNativeAbsenceProbe::class.java.name)
                .redirectErrorStream(true).redirectOutput(output).start()
            assertTrue(process.waitFor(30, TimeUnit.SECONDS), "native absence probe did not exit")
            assertEquals(0, process.exitValue(), output.readText())
            assertEquals("8 native admission boundaries rejected missing owner", output.readText().trim())
        } finally {
            directory.deleteRecursively()
        }
    }
}

/** Fresh-process probe uses normal public SDK calls with a real valid Ed25519 fixture. */
object AccountAddressNativeAbsenceProbe {
    @JvmStatic
    fun main(args: Array<String>) {
        check(args.isEmpty())
        val key = "5f7cbf9a659cf123009e5b0c2cc0285dfa5ed76bedae07cd281af72706be9536"
            .chunked(2).map { it.toInt(16).toByte() }.toByteArray()
        val canonical = byteArrayOf(2, 0, 1, 32) + key
        val literal = "sorauﾛ1P2PMｲbjRｦ2jrLFﾁｽｸFjjBヱYﾜｴ3ﾋNRjﾌｸﾆｺNXcfﾒXSKXAW"
        val multihash = "ed0120" + key.joinToString("") { "%02x".format(it.toInt() and 255) }
        val boundaries = listOf<() -> Unit>(
            { AccountAddress.fromCanonicalBytes(canonical) },
            { AccountAddress.fromAccount(key, "ed25519") },
            { AccountAddress.parseEncoded(literal, 753) },
            { PublicKeyPayload(1, key) },
            { encodePublicKeyMultihash(1, key) },
            { compactPublicKeyPayload(1, key) },
            { decodePublicKeyLiteral(multihash) },
            { decodeCompactPublicKeyPayload(byteArrayOf(0) + key) },
        )
        for (operation in boundaries) {
            try {
                operation()
                error("native account admission unexpectedly succeeded")
            } catch (error: AccountAddressException) {
                check(error.code == AccountAddressErrorCode.NATIVE_BRIDGE_UNAVAILABLE) { error.codeValue }
            }
        }
        println("8 native admission boundaries rejected missing owner")
    }
}
