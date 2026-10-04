// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.wallet

import android.content.pm.ApplicationInfo
import java.io.File
import java.io.IOException
import java.nio.file.Files
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertNull
import kotlin.test.assertSame
import org.junit.jupiter.api.Test
import org.junit.jupiter.api.io.TempDir

class KagemushaWalletAndroidPlatformV1Test {
    @TempDir
    lateinit var directory: File

    private fun platform(environment: KagemushaWalletAndroidEnvironmentV1, keyStore: TestKeyStoreV1 = TestKeyStoreV1()) =
        KagemushaWalletAndroidPlatformV1(environment, keyStore) { KagemushaWalletAndroidIntentStateV1.DURABLE }

    @Test fun `wallet creation is refused when the application allows backup`() {
        val environment = TestEnvironmentV1(directory)
        environment.flags = ApplicationInfo.FLAG_ALLOW_BACKUP or ApplicationInfo.FLAG_HAS_CODE
        assertFailsWith<IllegalStateException> { platform(environment) }
        environment.flags = ApplicationInfo.FLAG_HAS_CODE
        environment.agent = "com.example.WalletBackupAgent"
        assertFailsWith<IllegalStateException> { platform(environment) }
        environment.agent = null
        environment.deviceProtected = true
        assertFailsWith<IllegalStateException> { platform(environment) }
        environment.deviceProtected = false
        platform(environment)
    }

    @Test fun `storage is available only after the first unlock`() {
        val environment = TestEnvironmentV1(directory)
        val platform = platform(environment)
        assertNull(platform.storageState())
        environment.unlocked = false
        assertSame(KagemushaWalletAndroidUnavailableV1.BEFORE_FIRST_UNLOCK, platform.storageState())
        environment.unlockFailure = IllegalStateException("UserManager died")
        assertEquals(
            KagemushaWalletAndroidUnavailableV1.platform(KagemushaWalletAndroidUnavailableV1.PLATFORM_STORAGE),
            platform.storageState(),
        )
    }

    @Test fun `the custody root is the canonical credential-encrypted no-backup directory`() {
        val linked = File(directory, "no_backup").apply { mkdirs() }
        val environment = TestEnvironmentV1(File(directory, "./no_backup/../no_backup"))
        val platform = platform(environment)
        val root = assertIs<KagemushaWalletAndroidCustodyRootV1.Present>(platform.custodyRoot())
        assertEquals(File(linked.canonicalPath, "kagemusha-wallet-v1").path, root.path)
        assertEquals(KagemushaWalletAndroidPlatformV1.CUSTODY_ROOT_DIR_NAME, File(root.path).name)
        assertEquals(false, File(root.path).exists())
        environment.unlocked = false
        assertSame(
            KagemushaWalletAndroidUnavailableV1.BEFORE_FIRST_UNLOCK,
            assertIs<KagemushaWalletAndroidCustodyRootV1.Unavailable>(platform.custodyRoot()).reason,
        )
        environment.unlocked = true
        environment.directoryFailure = IOException("canonicalization failed")
        assertEquals(
            KagemushaWalletAndroidUnavailableV1.io(0),
            assertIs<KagemushaWalletAndroidCustodyRootV1.Unavailable>(platform.custodyRoot()).reason,
        )
    }

    @Test fun `the custody root resolves symbolic links`() {
        val target = File(directory, "target").apply { mkdirs() }
        val link = File(directory, "link")
        Files.createSymbolicLink(link.toPath(), target.toPath())
        val root = assertIs<KagemushaWalletAndroidCustodyRootV1.Present>(platform(TestEnvironmentV1(link)).custodyRoot())
        assertEquals(File(target.canonicalPath, "kagemusha-wallet-v1").path, root.path)
    }

    @Test fun `the boot identity is the validated lowercase boot UUID`() {
        val environment = TestEnvironmentV1(directory)
        val platform = platform(environment)
        environment.bootIdText = "6F1C2D3E-4A5B-6C7D-8E9F-A0B1C2D3E4F5\n"
        assertEquals("6f1c2d3e-4a5b-6c7d-8e9f-a0b1c2d3e4f5", assertIs<KagemushaWalletAndroidBootIdV1.Present>(platform.bootId()).uuid)
        for (malformed in listOf("", "6f1c2d3e4a5b6c7d8e9fa0b1c2d3e4f5", "6f1c2d3e-4a5b-6c7d-8e9f-a0b1c2d3e4fg", "6f1c2d3e-4a5b-6c7d-8e9f_a0b1c2d3e4f5")) {
            environment.bootIdText = malformed
            assertEquals(
                KagemushaWalletAndroidUnavailableV1.io(0),
                assertIs<KagemushaWalletAndroidBootIdV1.Unavailable>(platform.bootId()).reason,
            )
        }
        environment.bootIdText = "6f1c2d3e-4a5b-6c7d-8e9f-a0b1c2d3e4f5"
        environment.bootIdFailure = IOException("proc unavailable")
        assertIs<KagemushaWalletAndroidBootIdV1.Unavailable>(platform.bootId())
    }

    @Test fun `the clock and anchor policy match the Rust platform contract`() {
        val environment = TestEnvironmentV1(directory)
        val platform = platform(environment)
        environment.elapsed = 86_400_000L * 3
        assertEquals(86_400_000L * 3, platform.monotonicMillis())
        assertEquals(0, platform.anchorPolicyTag())
        assertEquals(0, KagemushaWalletAndroidPlatformV1.ANCHOR_POLICY_NOT_REQUIRED_TAG)
    }

    @Test fun `key operations reach the payment key through the slot alias`() {
        val keyStore = TestKeyStoreV1()
        val platform = platform(TestEnvironmentV1(directory), keyStore)
        val slot = ByteArray(32) { (it + 1).toByte() }
        assertSame(KagemushaWalletAndroidKeyProbeV1.Absent, platform.keyProbe(slot))
        val generated = assertIs<KagemushaWalletAndroidKeyGenerationV1.Generated>(
            platform.keyGenerate(slot, ByteArray(32) { 3 }, KagemushaWalletAndroidKeyProfileV1.SECURE_ELEMENT),
        )
        assertEquals(KagemushaWalletAndroidPlatformV1.paymentKeyAlias(slot), keyStore.generated.single().alias)
        assertEquals(
            generated.publicKeySec1().toList(),
            assertIs<KagemushaWalletAndroidKeyProbeV1.Present>(platform.keyProbe(slot)).publicKeySec1().toList(),
        )
        assertIs<KagemushaWalletAndroidSignatureV1.Der>(platform.keySign(slot, byteArrayOf(1, 2, 3)))
        assertIs<KagemushaWalletAndroidAttestationChainV1.Present>(platform.attestationChain(slot))
        assertSame(KagemushaWalletAndroidRemoveV1.Removed, platform.keyDelete(slot))
        assertSame(KagemushaWalletAndroidKeyProbeV1.Absent, platform.keyProbe(slot))
    }
}
