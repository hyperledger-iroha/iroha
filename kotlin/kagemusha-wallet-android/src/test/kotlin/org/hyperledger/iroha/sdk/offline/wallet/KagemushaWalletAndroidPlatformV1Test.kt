// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.wallet

import android.content.Context
import android.content.pm.ApplicationInfo
import java.io.File
import java.io.IOException
import java.nio.file.Files
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertNull
import kotlin.test.assertSame
import kotlin.test.assertTrue
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.io.TempDir
import org.hyperledger.iroha.sdk.testing.JvmApiInventory

class KagemushaWalletAndroidPlatformV1Test {
    @TempDir
    lateinit var directory: File

    private fun adapter(environment: KagemushaWalletAndroidEnvironmentV1, keyStore: TestKeyStoreV1 = TestKeyStoreV1()) =
        KagemushaWalletAndroidPlatformAdapterV1(environment, keyStore)

    @Test fun `creation is refused unless custody can be kept`() {
        val environment = TestEnvironmentV1(directory)
        environment.apiLevel = 25
        assertFailsWith<IllegalStateException> { adapter(environment) }
        environment.apiLevel = 31
        environment.flags = ApplicationInfo.FLAG_ALLOW_BACKUP or ApplicationInfo.FLAG_HAS_CODE
        assertFailsWith<IllegalStateException> { adapter(environment) }
        environment.flags = ApplicationInfo.FLAG_HAS_CODE
        environment.agent = "com.example.WalletBackupAgent"
        assertFailsWith<IllegalStateException> { adapter(environment) }
        environment.agent = null
        // A host resource of the same name that adds files to device transfer.
        val shipped = environment.dataExtraction
        val transfer = shipped.children.single { it.name == "device-transfer" }
        val include = KagemushaWalletAndroidXmlElementV1("include", mapOf("domain" to "file", "path" to "."), emptyList())
        environment.dataExtraction = KagemushaWalletAndroidXmlElementV1(shipped.name, shipped.attributes,
            shipped.children - transfer + KagemushaWalletAndroidXmlElementV1(transfer.name, transfer.attributes, transfer.children + include))
        assertFailsWith<IllegalStateException> { adapter(environment) }
        environment.dataExtraction = shipped
        environment.rulesFailure = IllegalStateException("Resources\$NotFoundException")
        assertFailsWith<IllegalStateException> { adapter(environment) }
        environment.rulesFailure = null
        environment.deviceProtected = true
        assertFailsWith<IllegalStateException> { adapter(environment) }
        environment.deviceProtected = false
        adapter(environment)
    }

    @Test fun `API 26 through 30 require fresh provenance and API 31 keeps definitive generation`() {
        for (api in listOf(26, 27, 28, 29, 30, 31, 33)) {
            val environment = TestEnvironmentV1(directory).apply { apiLevel = api }
            val platform = adapter(environment)
            assertEquals(if (api < 31) 1 else 0, platform.keyGenerationMode(), "API $api")
        }
    }

    @Test fun `fresh JNI generation refuses locked storage and unknown profiles without touching Keystore`() {
        val environment = TestEnvironmentV1(directory).apply { apiLevel = 26; unlocked = false }
        val keyStore = TestKeyStoreV1().apply { apiLevel = 26 }
        val platform = adapter(environment, keyStore)
        val slot = ByteArray(32) { 7 }
        val challenge = ByteArray(32) { 8 }
        assertEquals(
            KagemushaWalletAndroidUnavailableV1.BEFORE_FIRST_UNLOCK,
            assertIs<KagemushaWalletAndroidKeyGenerationV1.Unavailable>(platform.keyGenerateFreshFromNative(slot, challenge, 2)).reason,
        )
        assertEquals(0, keyStore.getKeyCalls)
        assertTrue(keyStore.generated.isEmpty())
        assertFailsWith<IllegalArgumentException> { platform.keyGenerateFreshFromNative(slot, challenge, 0) }
    }

    @Test fun `readback recovery preserves storage refusal and does not grant generation`() {
        val environment = TestEnvironmentV1(directory).apply { apiLevel = 26 }
        val keyStore = TestKeyStoreV1().apply { apiLevel = 26 }
        val platform = adapter(environment, keyStore)
        val slot = ByteArray(32) { 7 }
        val challenge = ByteArray(32) { 8 }
        assertNull(platform.keyGenerationRecovery(slot, challenge, 2))
        assertTrue(keyStore.generated.isEmpty())
        keyStore.factsFailure = java.security.ProviderException("KeyInfo unavailable")
        assertIs<KagemushaWalletAndroidKeyGenerationV1.Unavailable>(platform.keyGenerateFreshFromNative(slot, challenge, 2))
        val original = keyStore.entries.getValue(kagemushaWalletAndroidAliasV1(slot))
        keyStore.factsFailure = null
        environment.unlocked = false
        val reads = keyStore.getKeyCalls
        assertSame(KagemushaWalletAndroidUnavailableV1.BEFORE_FIRST_UNLOCK,
            assertIs<KagemushaWalletAndroidKeyGenerationV1.Unavailable>(platform.keyGenerationRecovery(slot, challenge, 2)).reason)
        assertEquals(reads, keyStore.getKeyCalls)
        assertFailsWith<IllegalArgumentException> { platform.keyGenerationRecovery(slot, challenge, 0) }
        environment.unlocked = true
        val recovered = assertIs<KagemushaWalletAndroidKeyGenerationV1.Generated>(platform.keyGenerationRecovery(slot, challenge, 2))
        assertContentEquals(testSec1V1(original.pair.public), recovered.publicKeySec1())
        assertEquals(1, keyStore.generated.size)
        assertEquals(0, keyStore.deleteCalls)
        assertEquals(0, keyStore.signCalls)
    }

    @Test fun `storage is available only after the first unlock`() {
        val environment = TestEnvironmentV1(directory)
        val adapter = adapter(environment)
        assertNull(adapter.storageState())
        environment.unlocked = false
        assertSame(KagemushaWalletAndroidUnavailableV1.BEFORE_FIRST_UNLOCK, adapter.storageState())
        environment.unlockFailure = IllegalStateException("UserManager died")
        assertEquals(
            KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_STORAGE),
            adapter.storageState(),
        )
    }

    @Test fun `the custody root is the canonical credential-encrypted no-backup directory`() {
        val linked = File(directory, "no_backup").apply { mkdirs() }
        val environment = TestEnvironmentV1(File(directory, "./no_backup/../no_backup"))
        val adapter = adapter(environment)
        val root = assertIs<KagemushaWalletAndroidCustodyRootV1.Present>(adapter.custodyRoot())
        assertEquals(File(linked.canonicalPath, "kagemusha-wallet-v1").path, root.path)
        assertEquals(false, File(root.path).exists())
        environment.unlocked = false
        assertSame(
            KagemushaWalletAndroidUnavailableV1.BEFORE_FIRST_UNLOCK,
            assertIs<KagemushaWalletAndroidCustodyRootV1.Unavailable>(adapter.custodyRoot()).reason,
        )
        environment.unlocked = true
        environment.directoryFailure = IOException("canonicalization failed")
        assertEquals(
            KagemushaWalletAndroidUnavailableV1.io(0),
            assertIs<KagemushaWalletAndroidCustodyRootV1.Unavailable>(adapter.custodyRoot()).reason,
        )
    }

    @Test fun `the custody root resolves symbolic links`() {
        val target = File(directory, "target").apply { mkdirs() }
        val link = File(directory, "link")
        Files.createSymbolicLink(link.toPath(), target.toPath())
        val root = assertIs<KagemushaWalletAndroidCustodyRootV1.Present>(adapter(TestEnvironmentV1(link)).custodyRoot())
        assertEquals(File(target.canonicalPath, "kagemusha-wallet-v1").path, root.path)
    }

    @Test fun `the handle exposes only creation and original certificate export to app code`() {
        val type = JvmApiInventory.read(KagemushaWalletAndroidPlatformV1::class.java)
        val declaredPublic = type.methods.filter { it.isPublic && !it.isSynthetic && it.name != "<init>" }
        assertEquals(setOf("create", "enrollmentCertificates"), declaredPublic.map { it.name }.toSet())
        val create = declaredPublic.single { it.name == "create" }
        assertEquals("(Landroid/content/Context;)L$JVM_OWNER/KagemushaWalletAndroidPlatformV1;", create.descriptor)
        assertTrue(create.isStatic)
        val export = declaredPublic.single { it.name == "enrollmentCertificates" }
        assertEquals("(L$JVM_OWNER/KagemushaWalletEnrollmentTargetV1;)Ljava/util/List;", export.descriptor)
        assertEquals(false, export.isStatic)
        for ((name, signature) in UPCALLS) {
            val method = type.methods.single { it.name == name }
            assertTrue(method.flags and 0x0002 != 0, name)
            assertEquals(signature, method.descriptor, name)
        }
        assertEquals(UPCALLS.keys, type.methods.filter {
            it.flags and 0x0002 != 0 && !it.isSynthetic && it.name != "<init>"
        }.map { it.name }.toSet())
    }

    @Test fun `enrollment exports only the selected key originals and preserves unavailable`() {
        val environment = TestEnvironmentV1(directory)
        val keyStore = TestKeyStoreV1()
        val slot = ByteArray(32) { 7 }
        val entry = keyStore.seed(kagemushaWalletAndroidAliasV1(slot))
        val target = KagemushaWalletEnrollmentTargetV1(slot + testSec1V1(entry.pair.public) + ByteArray(64) { 9 })
        val platform = KagemushaWalletAndroidPlatformV1.create(environment, keyStore)
        val expected = requireNotNull(entry.chain).map { it.encoded.toList() }
        val originals = platform.enrollmentCertificates(target)
        assertEquals(expected, originals.map { it.toList() })
        originals.first().fill(0)
        assertEquals(expected, platform.enrollmentCertificates(target).map { it.toList() })

        val changed = KagemushaWalletEnrollmentTargetV1(slot + testSec1V1(TestAttestationV1.p256().public) + ByteArray(64) { 9 })
        assertEquals(-7, assertFailsWith<KagemushaWalletExceptionV1> { platform.enrollmentCertificates(changed) }.status)
        keyStore.getKeyFailure = java.security.ProviderException("unavailable")
        assertEquals(-5, assertFailsWith<KagemushaWalletExceptionV1> { platform.enrollmentCertificates(target) }.status)
        keyStore.getKeyFailure = null
        environment.unlocked = false
        val locked = assertFailsWith<KagemushaWalletExceptionV1> { platform.enrollmentCertificates(target) }
        assertEquals(-5, locked.status)
        assertEquals(KagemushaWalletAndroidUnavailableV1.BEFORE_FIRST_UNLOCK.kind.tag, locked.reason)
        environment.unlocked = true
        keyStore.entries.clear()
        assertEquals(-13, assertFailsWith<KagemushaWalletExceptionV1> { platform.enrollmentCertificates(target) }.status)
        assertEquals(0, keyStore.signCalls)
        assertEquals(0, keyStore.deleteCalls)
        assertTrue(keyStore.generated.isEmpty())
    }

    @Test fun `the consumer rules keep every upcall the bridge binds`() {
        val rules = File("consumer-rules.pro").readText()
        val methods = JvmApiInventory.read(KagemushaWalletAndroidPlatformV1::class.java).methods
        for (name in UPCALLS.keys) {
            val signature = methods.single { it.name == name }.parameterTypes.joinToString(", ") {
                when (it) {
                    "[B" -> "byte[]"
                    "I" -> "int"
                    else -> error("unreviewed JNI upcall parameter descriptor: $it")
                }
            }
            assertTrue(rules.contains(" $name($signature);"), "consumer-rules.pro must keep $name($signature)")
        }
    }

    @Test fun `the platform adapter reaches the payment key through the slot alias`() {
        val keyStore = TestKeyStoreV1()
        val platform = adapter(TestEnvironmentV1(directory), keyStore)
        val slot = ByteArray(32) { (it + 1).toByte() }
        assertSame(KagemushaWalletAndroidKeyProbeV1.Absent, platform.keyProbe(slot))
        val generated = assertIs<KagemushaWalletAndroidKeyGenerationV1.Generated>(
            platform.keyGenerate(slot, ByteArray(32) { 3 }, KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT.tag),
        )
        assertEquals(kagemushaWalletAndroidAliasV1(slot), keyStore.generated.single().alias)
        assertEquals(
            generated.publicKeySec1().toList(),
            assertIs<KagemushaWalletAndroidKeyProbeV1.Present>(platform.keyProbe(slot)).publicKeySec1().toList(),
        )
        assertIs<KagemushaWalletAndroidSignatureV1.Der>(platform.keySign(slot, ByteArray(32) { (it + 1).toByte() }))
        assertContentEquals(ByteArray(32) { (it + 1).toByte() }, keyStore.signedMessages.single())
        assertFailsWith<IllegalArgumentException> { platform.keySign(slot, byteArrayOf(1, 2, 3)) }
        assertIs<KagemushaWalletAndroidAttestationChainV1.Present>(platform.attestationChain(slot))
        assertEquals(0, platform.anchorPolicyTag())
        assertNull(platform.storageState())
        assertIs<KagemushaWalletAndroidCustodyRootV1.Present>(platform.custodyRoot())
        assertSame(KagemushaWalletAndroidRemoveV1.Removed, platform.keyDelete(slot))
        assertSame(KagemushaWalletAndroidKeyProbeV1.Absent, platform.keyProbe(slot))
        assertFailsWith<IllegalArgumentException> { platform.keyGenerate(ByteArray(32) { 9 }, ByteArray(32) { 3 }, 0) }
        assertEquals(1, keyStore.generated.size)
    }

    private companion object {
        private const val JVM_OWNER = "org/hyperledger/iroha/sdk/offline/wallet"

        /** Exact private JNI names and descriptors, including return types. */
        val UPCALLS: Map<String, String> = linkedMapOf(
            "keyProbe" to "([B)L$JVM_OWNER/KagemushaWalletAndroidKeyProbeV1;",
            "keyGenerate" to "([B[BI)L$JVM_OWNER/KagemushaWalletAndroidKeyGenerationV1;",
            "keySign" to "([B[B)L$JVM_OWNER/KagemushaWalletAndroidSignatureV1;",
            "keyDelete" to "([B)L$JVM_OWNER/KagemushaWalletAndroidRemoveV1;",
            "attestationChain" to "([B)L$JVM_OWNER/KagemushaWalletAndroidAttestationChainV1;",
            "nativeCall" to "(I[B[BI)L$JVM_OWNER/KagemushaWalletNativeReplyV1;",
            "anchorPolicyTag" to "()I",
            "storageState" to "()L$JVM_OWNER/KagemushaWalletAndroidUnavailableV1;",
            "custodyRoot" to "()L$JVM_OWNER/KagemushaWalletAndroidCustodyRootV1;",
        )
    }
}
