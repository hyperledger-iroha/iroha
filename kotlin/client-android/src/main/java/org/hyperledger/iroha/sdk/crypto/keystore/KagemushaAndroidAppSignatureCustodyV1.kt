package org.hyperledger.iroha.sdk.crypto.keystore

import java.io.ByteArrayOutputStream
import java.io.DataOutputStream
import java.io.File
import java.math.BigInteger
import java.security.MessageDigest

/** SDK-internal native selection. No public constructor can authorize an offered signing frame.
 * Only the actual held native owner may supply this original and its current-owner check.
 * The platform signs the original bytes; it never constructs an approval or financial subject.
 */
internal class KagemushaAndroidHeldAppSignatureV1(
    val alias: String,
    val hardwarePolicy: KagemushaAndroidAppKeyHardwarePolicyV1,
    operationId: ByteArray,
    attestedKeyId: ByteArray,
    attestationChallenge: ByteArray,
    canonicalChallenge: ByteArray,
    signingBytes: ByteArray,
    private val requireCurrent: () -> Unit,
) {
    private val operation = operationId.copyOf()
    private val key = attestedKeyId.copyOf()
    private val attestation = attestationChallenge.copyOf()
    private val challenge = canonicalChallenge.copyOf()
    private val message = signingBytes.copyOf()
    init {
        require(alias.matches(Regex("[a-zA-Z0-9._-]{1,128}")))
        for (field in listOf(operation, key, attestation)) require(field.size == 32 && field.any { it != 0.toByte() })
        require(challenge.size in 1..128 * 1024 && message.size in 1..4096)
    }
    fun operationId(): ByteArray = operation.copyOf()
    fun attestedKeyId(): ByteArray = key.copyOf()
    fun attestationChallenge(): ByteArray = attestation.copyOf()
    fun canonicalChallenge(): ByteArray = challenge.copyOf()
    fun signingBytes(): ByteArray = message.copyOf()
    fun requireOriginal() = requireCurrent()
}

internal enum class KagemushaAndroidAppSignatureCustodyFailureV1 {
    ORIGINAL_CHANGED, SIGNATURE_UNCERTAIN, CORRUPT_ORIGINAL, CUSTODY_UNAVAILABLE,
}
internal class KagemushaAndroidAppSignatureCustodyExceptionV1(
    val failure: KagemushaAndroidAppSignatureCustodyFailureV1,
    cause: Throwable? = null,
) : IllegalStateException("Original hardware app signature custody: ${failure.name}", cause)

internal interface KagemushaAndroidAppSignatureDeviceV1 {
    fun signOriginal(original: KagemushaAndroidHeldAppSignatureV1): ByteArray
    /** Recover only under the same actual nonexportable key, verifying its original signature. */
    fun verifyOriginal(original: KagemushaAndroidHeldAppSignatureV1, signatureDer: ByteArray)
}

/** Crash custody only, never a hardware monotonic counter, StateGuard or native replay grant.
 * Native admission must independently reserve its challenge, own its financial state and verify
 * the original enrolled key and signature. An intent without a DER result always freezes locally.
 */
internal class KagemushaAndroidAppSignatureCustodyV1(
    private val directory: File,
    private val device: KagemushaAndroidAppSignatureDeviceV1,
    private val io: KagemushaAndroidOriginalJournalIoV1 = AndroidOriginalJournalIoV1,
) {
    fun signOrRecoverExact(original: KagemushaAndroidHeldAppSignatureV1): ByteArray {
        original.requireOriginal()
        val slot = original.operationId().joinToString("") { "%02x".format(it.toInt() and 0xff) }
        fun file(suffix: String): File {
            check(directory.isDirectory) { "Private hardware signature directory is unavailable" }
            return File(directory, "kagemusha-app-signature-v1-$slot.$suffix")
        }
        try {
            return io.withLock(file("lock")) {
                original.requireOriginal()
                val expected = intentBytes(original)
                val intent = file("intent"); val result = file("der")
                if (io.exists(intent)) {
                    if (!MessageDigest.isEqual(io.read(intent, MAXIMUM_INTENT_BYTES), expected)) {
                        fail(KagemushaAndroidAppSignatureCustodyFailureV1.ORIGINAL_CHANGED)
                    }
                    original.requireOriginal()
                    if (!io.exists(result)) fail(KagemushaAndroidAppSignatureCustodyFailureV1.SIGNATURE_UNCERTAIN)
                    val signature = decodeResult(io.read(result, MAXIMUM_RESULT_BYTES), expected)
                    original.requireOriginal(); device.verifyOriginal(original, signature.copyOf())
                    // A prior write could have reached the file before fsync failed. Reestablish
                    // durability of both originals and their directory before exposing recovery.
                    original.requireOriginal(); io.syncExisting(intent); io.syncExisting(result)
                    original.requireOriginal(); signature.copyOf()
                } else {
                    if (io.exists(result)) fail(KagemushaAndroidAppSignatureCustodyFailureV1.CORRUPT_ORIGINAL)
                    original.requireOriginal(); io.writeNew(intent, expected)
                    original.requireOriginal()
                    val signature = device.signOriginal(original).copyOf()
                    requireCanonicalP256SignatureDerV1(signature)
                    device.verifyOriginal(original, signature.copyOf())
                    original.requireOriginal(); io.writeNew(result, resultBytes(expected, signature))
                    original.requireOriginal(); signature.copyOf()
                }
            }
        } catch (error: KagemushaAndroidAppSignatureCustodyExceptionV1) { throw error }
        catch (error: Exception) {
            throw KagemushaAndroidAppSignatureCustodyExceptionV1(
                KagemushaAndroidAppSignatureCustodyFailureV1.CUSTODY_UNAVAILABLE, error)
        }
    }

    private fun intentBytes(original: KagemushaAndroidHeldAppSignatureV1): ByteArray {
        val bytes = ByteArrayOutputStream()
        DataOutputStream(bytes).use { output ->
            output.write(INTENT_MAGIC)
            output.write(original.operationId()); output.write(original.attestedKeyId()); output.write(original.attestationChallenge())
            output.writeByte(original.hardwarePolicy.ordinal)
            for (part in listOf(original.alias.toByteArray(Charsets.US_ASCII), original.canonicalChallenge(), original.signingBytes())) {
                output.writeInt(part.size); output.write(part)
            }
        }
        return bytes.toByteArray().also { check(it.size <= MAXIMUM_INTENT_BYTES) }
    }

    private fun resultBytes(intent: ByteArray, signature: ByteArray): ByteArray =
        RESULT_MAGIC + sha(intent) + byteArrayOf(signature.size.toByte()) + signature.copyOf()

    private fun decodeResult(bytes: ByteArray, intent: ByteArray): ByteArray {
        val offset = RESULT_MAGIC.size + 32 + 1
        if (bytes.size !in offset + 8..offset + 72 || !bytes.copyOfRange(0, RESULT_MAGIC.size).contentEquals(RESULT_MAGIC) ||
            !MessageDigest.isEqual(bytes.copyOfRange(RESULT_MAGIC.size, RESULT_MAGIC.size + 32), sha(intent)) ||
            (bytes[offset - 1].toInt() and 0xff) != bytes.size - offset) {
            fail(KagemushaAndroidAppSignatureCustodyFailureV1.CORRUPT_ORIGINAL)
        }
        return bytes.copyOfRange(offset, bytes.size).also {
            try { requireCanonicalP256SignatureDerV1(it) }
            catch (_: IllegalArgumentException) { fail(KagemushaAndroidAppSignatureCustodyFailureV1.CORRUPT_ORIGINAL) }
        }
    }
    private fun sha(bytes: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(bytes)
    private fun fail(reason: KagemushaAndroidAppSignatureCustodyFailureV1): Nothing =
        throw KagemushaAndroidAppSignatureCustodyExceptionV1(reason)

    companion object {
        private val INTENT_MAGIC = byteArrayOf(0x4b, 0x41, 0x53, 0x49, 1)
        private val RESULT_MAGIC = byteArrayOf(0x4b, 0x41, 0x53, 0x44, 1)
        private const val MAXIMUM_INTENT_BYTES = 136 * 1024
        private const val MAXIMUM_RESULT_BYTES = 5 + 32 + 1 + 72
    }
}

/** Preserve the exact minimal DER, including a genuine high-S original; native crypto normalizes S. */
internal fun requireCanonicalP256SignatureDerV1(bytes: ByteArray) {
    require(bytes.size in 8..72 && bytes[0] == 0x30.toByte() && (bytes[1].toInt() and 0xff) == bytes.size - 2)
    val order = BigInteger("ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551", 16)
    var offset = 2
    repeat(2) {
        require(offset + 2 <= bytes.size && bytes[offset] == 2.toByte())
        val length = bytes[offset + 1].toInt() and 0xff; offset += 2
        require(length in 1..33 && length <= bytes.size - offset && (bytes[offset].toInt() and 0x80) == 0)
        require(length == 1 || bytes[offset] != 0.toByte() || (bytes[offset + 1].toInt() and 0x80) != 0)
        val integer = BigInteger(1, bytes.copyOfRange(offset, offset + length))
        require(integer.signum() > 0 && integer < order); offset += length
    }
    require(offset == bytes.size)
}
