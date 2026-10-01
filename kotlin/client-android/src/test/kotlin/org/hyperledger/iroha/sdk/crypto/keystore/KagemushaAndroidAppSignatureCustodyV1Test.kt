package org.hyperledger.iroha.sdk.crypto.keystore

import java.io.File
import java.nio.file.Files
import java.security.KeyPairGenerator
import java.security.Signature
import java.security.spec.ECGenParameterSpec
import java.util.concurrent.Executors
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

/** Controlled software test signatures and memory fsync receipts; no native/hardware admission. */
class KagemushaAndroidAppSignatureCustodyV1Test {
    private class Io(val events: MutableList<String>) : KagemushaAndroidOriginalJournalIoV1 {
        val files = mutableMapOf<String, ByteArray>()
        var failIntent = false; var failResult = false; var failResultAfterWrite = false; var failResync = false
        override fun exists(file: File) = files.containsKey(file.name)
        override fun read(file: File, maximum: Int) = checkNotNull(files[file.name]).copyOf().also { check(it.size in 1..maximum) }
        override fun writeNew(file: File, bytes: ByteArray) {
            check(!files.containsKey(file.name)); val phase = file.extension
            events += "write:$phase"
            if (phase == "intent" && failIntent) { files[file.name] = byteArrayOf(0); error("inert torn intent") }
            if (phase == "der" && failResult && !failResultAfterWrite) error("inert missing DER")
            files[file.name] = bytes.copyOf()
            if (phase == "der" && failResult) error("inert result before fsync")
            events += "durable:$phase"
        }
        override fun syncExisting(file: File) {
            check(files.containsKey(file.name)); if (failResync) error("inert resync failure")
            events += "resync:${file.extension}"
        }
        override fun <T> withLock(file: File, action: () -> T): T = synchronized(this) { action() }
    }
    private class Device(val events: MutableList<String>) : KagemushaAndroidAppSignatureDeviceV1 {
        private val key = KeyPairGenerator.getInstance("EC").apply { initialize(ECGenParameterSpec("secp256r1")) }.generateKeyPair()
        var signs = 0; var failSign = false; var afterSign: () -> Unit = {}
        override fun signOriginal(original: KagemushaAndroidHeldAppSignatureV1): ByteArray {
            original.requireOriginal(); events += "sign"; signs++
            if (failSign) error("inert uncertain signing")
            val result = Signature.getInstance("SHA256withECDSA").run {
                initSign(key.private); update(original.signingBytes()); sign()
            }
            afterSign(); return result
        }
        override fun verifyOriginal(original: KagemushaAndroidHeldAppSignatureV1, signatureDer: ByteArray) {
            original.requireOriginal(); events += "verify"
            check(Signature.getInstance("SHA256withECDSA").run {
                initVerify(key.public); update(original.signingBytes()); verify(signatureDer)
            })
        }
    }
    private fun original(operation: ByteArray = ByteArray(32) { 1 }, key: ByteArray = ByteArray(32) { 2 },
        challenge: ByteArray = byteArrayOf(3, 4), message: ByteArray = byteArrayOf(5, 6),
        alias: String = "same-original-app-key", guard: () -> Unit = {}) =
        KagemushaAndroidHeldAppSignatureV1(alias, KagemushaAndroidAppKeyHardwarePolicyV1.TEE_OR_STRONGBOX,
            operation, key, ByteArray(32) { 7 }, challenge, message, guard)
    private fun failure(expected: KagemushaAndroidAppSignatureCustodyFailureV1, action: () -> Unit) {
        assertEquals(expected, assertThrows(KagemushaAndroidAppSignatureCustodyExceptionV1::class.java, action).failure)
    }
    private inline fun fixture(action: (KagemushaAndroidAppSignatureCustodyV1, Io, Device, MutableList<String>) -> Unit) {
        val directory = Files.createTempDirectory("iroha-app-signature-custody-test").toFile()
        try {
            val events = mutableListOf<String>(); val io = Io(events); val device = Device(events)
            action(KagemushaAndroidAppSignatureCustodyV1(directory, device, io), io, device, events)
        } finally { directory.delete() }
    }

    @Test fun intentIsDurableBeforeSigningAndExactDerIsDurableBeforeExposureAndRecovery() = fixture { runner, _, device, events ->
        val first = runner.signOrRecoverExact(original())
        assertEquals(listOf("write:intent", "durable:intent", "sign", "verify", "write:der", "durable:der"), events)
        val replay = runner.signOrRecoverExact(original())
        assertArrayEquals(first, replay); assertEquals(1, device.signs)
        assertEquals(listOf("verify", "resync:intent", "resync:der"), events.takeLast(3))
        first.fill(0); assertArrayEquals(replay, runner.signOrRecoverExact(original()))
    }

    @Test fun interruptedOrFailedSigningLeavesIntentAndNeverSignsAgain() = fixture { runner, io, device, _ ->
        device.failSign = true
        failure(KagemushaAndroidAppSignatureCustodyFailureV1.CUSTODY_UNAVAILABLE) { runner.signOrRecoverExact(original()) }
        device.failSign = false
        failure(KagemushaAndroidAppSignatureCustodyFailureV1.SIGNATURE_UNCERTAIN) { runner.signOrRecoverExact(original()) }
        assertEquals(1, device.signs); assertTrue(io.files.keys.single().endsWith(".intent"))
    }

    @Test fun failedIntentDurabilityPreventsSigningAndTornIntentCannotBeAdopted() = fixture { runner, io, device, _ ->
        io.failIntent = true
        failure(KagemushaAndroidAppSignatureCustodyFailureV1.CUSTODY_UNAVAILABLE) { runner.signOrRecoverExact(original()) }
        io.failIntent = false
        failure(KagemushaAndroidAppSignatureCustodyFailureV1.ORIGINAL_CHANGED) { runner.signOrRecoverExact(original()) }
        assertEquals(0, device.signs)
    }

    @Test fun missingResultAfterPhysicalSigningNeverTriggersASecondSignature() = fixture { runner, io, device, _ ->
        io.failResult = true
        failure(KagemushaAndroidAppSignatureCustodyFailureV1.CUSTODY_UNAVAILABLE) { runner.signOrRecoverExact(original()) }
        io.failResult = false
        failure(KagemushaAndroidAppSignatureCustodyFailureV1.SIGNATURE_UNCERTAIN) { runner.signOrRecoverExact(original()) }
        assertEquals(1, device.signs)
    }

    @Test fun completedBytesFromAnUncertainFsyncAreResyncedBeforeRecoveryWithoutResigning() = fixture { runner, io, device, events ->
        io.failResult = true; io.failResultAfterWrite = true
        failure(KagemushaAndroidAppSignatureCustodyFailureV1.CUSTODY_UNAVAILABLE) { runner.signOrRecoverExact(original()) }
        io.failResult = false; io.failResync = true
        failure(KagemushaAndroidAppSignatureCustodyFailureV1.CUSTODY_UNAVAILABLE) { runner.signOrRecoverExact(original()) }
        io.failResync = false
        val retained = runner.signOrRecoverExact(original()); requireCanonicalP256SignatureDerV1(retained)
        assertEquals(1, device.signs); assertEquals(listOf("verify", "resync:intent", "resync:der"), events.takeLast(3))
    }

    @Test fun changedKeyAliasChallengeOrSigningMessageCannotAdoptAnExistingOperation() = fixture { runner, _, device, _ ->
        runner.signOrRecoverExact(original())
        for (changed in listOf(original(key = ByteArray(32) { 8 }), original(alias = "different-app-key"),
            original(challenge = byteArrayOf(9)), original(message = byteArrayOf(10)))) {
            failure(KagemushaAndroidAppSignatureCustodyFailureV1.ORIGINAL_CHANGED) { runner.signOrRecoverExact(changed) }
        }
        assertEquals(1, device.signs)
    }

    @Test fun lostNativeOwnerCannotExposeOrRepeatItsSignedOperation() = fixture { runner, _, device, _ ->
        var current = true; val held = original { check(current) }
        device.afterSign = { current = false }
        failure(KagemushaAndroidAppSignatureCustodyFailureV1.CUSTODY_UNAVAILABLE) { runner.signOrRecoverExact(held) }
        current = true; device.afterSign = {}
        failure(KagemushaAndroidAppSignatureCustodyFailureV1.SIGNATURE_UNCERTAIN) { runner.signOrRecoverExact(held) }
        assertEquals(1, device.signs)
    }

    @Test fun simultaneousCallsShareOneSignedOriginal() = fixture { runner, _, device, _ ->
        val pool = Executors.newFixedThreadPool(2)
        try {
            val first = pool.submit<ByteArray> { runner.signOrRecoverExact(original()) }
            val second = pool.submit<ByteArray> { runner.signOrRecoverExact(original()) }
            assertArrayEquals(first.get(), second.get()); assertEquals(1, device.signs)
        } finally { pool.shutdownNow() }
    }

    @Test fun malformedOrForeignResultCannotBeExposedOrReplaced() = fixture { runner, io, device, _ ->
        runner.signOrRecoverExact(original())
        val name = io.files.keys.single { it.endsWith(".der") }
        val retained = checkNotNull(io.files[name]).copyOf()
        for (changed in listOf(retained + 0, retained.copyOfRange(0, retained.lastIndex),
            retained.copyOf().also { it[5] = (it[5].toInt() xor 1).toByte() },
            retained.copyOf().also { it[38] = 0x31 })) {
            io.files[name] = changed
            failure(KagemushaAndroidAppSignatureCustodyFailureV1.CORRUPT_ORIGINAL) { runner.signOrRecoverExact(original()) }
        }
        assertEquals(1, device.signs)
    }

    @Test fun originalHolderCopiesSelectorsAndDerRequiresMinimalPositiveP256Scalars() {
        val challenge = byteArrayOf(3, 4); val message = byteArrayOf(5, 6)
        val held = original(challenge = challenge, message = message); challenge.fill(0); message.fill(0)
        held.canonicalChallenge().fill(0); held.signingBytes().fill(0)
        assertArrayEquals(byteArrayOf(3, 4), held.canonicalChallenge()); assertArrayEquals(byteArrayOf(5, 6), held.signingBytes())
        val der = byteArrayOf(0x30, 6, 2, 1, 1, 2, 1, 1); requireCanonicalP256SignatureDerV1(der)
        for (bad in listOf(der + 0, der.copyOf().also { it[4] = 0 }, der.copyOf().also { it[7] = 0xff.toByte() },
            byteArrayOf(0x30, 7, 2, 2, 0, 1, 2, 1, 1))) {
            assertThrows(IllegalArgumentException::class.java) { requireCanonicalP256SignatureDerV1(bad) }
        }
    }
}
