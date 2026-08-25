package org.hyperledger.iroha.sdk.offline

import java.nio.charset.StandardCharsets
import java.security.MessageDigest
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.core.model.TransactionAdmissionIntent
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.core.model.WirePayload
import org.hyperledger.iroha.sdk.core.model.instructions.FixtureGeneratorRunner
import org.hyperledger.iroha.sdk.core.model.instructions.ProofAttachment
import org.hyperledger.iroha.sdk.core.model.instructions.ProofVerifierKeyRef
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.TypeAdapter
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertIs

/** Exact Rust/Kotlin parity and adversarial coverage for the native bridge ABI 22 path. */
class RegisterOfflineDeviceAttestationTest {

    @Test
    fun `omitted ttl uses canonical transaction default`() {
        val accountId = AccountAddress
            .fromAccount(TestEd25519Keys.publicKey(0x42), "ed25519")
            .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT)
        val registration = registration(accountId)
        val request = RegisterOfflineDeviceAttestation(
            networkId = TEST_NETWORK_ID,
            authority = accountId,
            registration = registration,
            creationTimeMs = 1_900_000_000_000,
            nonce = 7,
            feePayment = TEST_FEE_PAYMENT,
        )
        val canonicalPayload = TransactionPayload(
            networkId = TEST_NETWORK_ID,
            authority = accountId,
            creationTimeMs = 1_900_000_000_000,
            executable = Executable.instructions(listOf(request.instruction())),
            nonce = 7,
            feePayment = TEST_FEE_PAYMENT,
            admissionIntent = TransactionAdmissionIntent.QUEUE_PLAN_SYNCED,
        )

        assertEquals(100_000L, canonicalPayload.timeToLiveMs)
        assertEquals(canonicalPayload.timeToLiveMs, request.timeToLiveMs)
        assertEquals(canonicalPayload.timeToLiveMs, request.transactionPayload().timeToLiveMs)
        assertFailsWith<IllegalArgumentException> {
            RegisterOfflineDeviceAttestation(
                networkId = TEST_NETWORK_ID,
                authority = accountId,
                registration = registration,
                creationTimeMs = registration.expiresAtMs - 99_999,
                nonce = 7,
                feePayment = TEST_FEE_PAYMENT,
            )
        }
    }

    @Test
    fun `registration and instruction exactly match Rust current model`() {
        val rust = FixtureGeneratorRunner.run("offline-device-attestation")
        assertEquals(5, rust.size)
        val registration = registration(rust[3])

        assertEquals(22, DeviceAttestationRegistration.REQUIRED_NATIVE_BRIDGE_ABI_VERSION)
        assertContentEquals(hexToBytes(rust[0]), registration.noritoEncoded())
        assertContentEquals(hexToBytes(rust[2]), registration.challengeHash)
        assertContentEquals(hexToBytes(rust[4]), registration.canonicalRegistrationHash())
        assertFalse(
            registration.canonicalRegistrationHash().contentEquals(
                MessageDigest.getInstance("SHA-256").digest(registration.noritoEncoded()),
            ),
            "registration ID is canonical Iroha Hash, not raw SHA-256",
        )

        val request = request(registration)
        val instruction = request.instruction()
        assertEquals(OfflineDeviceAttestationCodec.INSTRUCTION_SCHEMA, instruction.name)
        val wire = assertIs<WirePayload>(instruction.payload)
        assertContentEquals(hexToBytes(rust[1]), wire.payloadBytes)
        assertEquals(
            registration,
            DeviceAttestationRegistration.decodeCanonical(
                registration.noritoEncoded(),
                AccountAddress.DEFAULT_I105_DISCRIMINANT,
            ),
        )
        assertEquals(
            registration,
            RegisterOfflineDeviceAttestation.decodeInstructionPayloadCanonical(
                wire.payloadBytes,
                AccountAddress.DEFAULT_I105_DISCRIMINANT,
            ),
        )
        request.validateExactPayload(request.transactionPayload())
    }

    @Test
    fun `Android attested properties Some round trips without changing pre-key challenge`() {
        val accountId = FixtureGeneratorRunner.run("offline-device-attestation")[3]
        val withoutProperties = registration(accountId)
        val properties = OfflineAndroidAttestedDevicePropertiesV2(
            version = OfflineAndroidAttestedDevicePropertiesV2.VERSION,
            attestationVersion = 400,
            keymintVersion = 300,
            securityLevel = OfflineAndroidDeviceSecurityLevelV2.STRONG_BOX,
            brand = "Iroha",
            device = "abi22-device",
            product = "abi22-product",
            manufacturer = "Hyperledger",
            model = "Kagemusha",
            osVersion = 14,
            osPatchLevel = 202608,
            vendorPatchLevel = 202608,
            bootPatchLevel = 202608,
            verifiedBootKey = bytes("abi22-verified-boot-key"),
            verifiedBootHash = sha256(bytes("abi22-verified-boot-hash")),
        )
        val withProperties = registration(
            accountId,
            attestedProperties = properties,
        )

        assertContentEquals(withoutProperties.challengeHash, withProperties.challengeHash)
        assertFalse(withoutProperties.noritoEncoded().contentEquals(withProperties.noritoEncoded()))
        assertEquals(properties, withProperties.androidAttestedDeviceProperties)
        assertEquals(
            withProperties,
            DeviceAttestationRegistration.decodeCanonical(
                withProperties.noritoEncoded(),
                AccountAddress.DEFAULT_I105_DISCRIMINANT,
            ),
        )
    }

    @Test
    fun `hash and app identity substitutions fail closed`() {
        val accountId = FixtureGeneratorRunner.run("offline-device-attestation")[3]
        val canonical = registration(accountId)
        assertFailsWith<IllegalArgumentException> {
            registration(accountId, challengeHash = IrohaHash.prehash(bytes("wrong-challenge")))
        }
        assertFailsWith<IllegalArgumentException> {
            registration(accountId, reportHash = IrohaHash.prehash(bytes("wrong-report")))
        }
        assertFailsWith<IllegalArgumentException> {
            registration(accountId, evidenceHash = IrohaHash.prehash(bytes("wrong-evidence")))
        }
        assertFailsWith<IllegalArgumentException> {
            registration(
                accountId,
                evidence = bytes(DeviceAttestationRegistration.DEVICE_ATTESTATION_EVIDENCE_PREFIX) +
                    IrohaHash.prehash(bytes("different-report")),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            registration(
                accountId,
                packageName = "org.hyperledger.iroha.substituted",
                challengeHash = canonical.challengeHash,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            val substitutedDigest = sha256(bytes("substituted-signing-certificate"))
            registration(
                accountId,
                signingCertificate = substitutedDigest,
                challengeHash = canonical.challengeHash,
            )
        }
    }

    @Test
    fun `unknown malformed and noncanonical registration bytes fail closed`() {
        val accountId = FixtureGeneratorRunner.run("offline-device-attestation")[3]
        val canonical = registration(accountId).noritoEncoded()
        val malformed = canonical.copyOf().also {
            it[it.lastIndex] = (it.last().toInt() xor 1).toByte()
        }
        assertFailsWith<IllegalArgumentException> {
            DeviceAttestationRegistration.decodeCanonical(
                malformed,
                AccountAddress.DEFAULT_I105_DISCRIMINANT,
            )
        }

        val payload = NoritoCodec.fromBytesView(
            canonical,
            OfflineDeviceAttestationCodec.REGISTRATION_SCHEMA,
        ).asBytes()
        val unknownField = NoritoCodec.encode(
            payload,
            OfflineDeviceAttestationCodec.REGISTRATION_SCHEMA,
            RawPayloadAdapter(appendUnknownField = true),
        )
        assertFailsWith<IllegalArgumentException> {
            DeviceAttestationRegistration.decodeCanonical(
                unknownField,
                AccountAddress.DEFAULT_I105_DISCRIMINANT,
            )
        }

        val alternateFlags = NoritoCodec.encode(
            payload,
            OfflineDeviceAttestationCodec.REGISTRATION_SCHEMA,
            RawPayloadAdapter(appendUnknownField = false),
            0,
        )
        assertFailsWith<IllegalArgumentException> {
            DeviceAttestationRegistration.decodeCanonical(
                alternateFlags,
                AccountAddress.DEFAULT_I105_DISCRIMINANT,
            )
        }
    }

    @Test
    fun `transaction rejects extra instructions and invalid ttl nonce`() {
        val accountId = FixtureGeneratorRunner.run("offline-device-attestation")[3]
        val registration = registration(accountId)
        val request = request(registration)
        val extraInstructionPayload = request.transactionPayload().copy(
            executable = Executable.instructions(
                listOf(request.instruction(), request.instruction()),
            ),
        )
        assertFailsWith<IllegalArgumentException> {
            request.validateExactPayload(extraInstructionPayload)
        }
        val attachedPayload = request.transactionPayload().copy(
            attachments = listOf(
                ProofAttachment(
                    "halo2",
                    byteArrayOf(0x01),
                    ProofVerifierKeyRef("halo2", "vk1"),
                ),
            ),
        )
        assertFailsWith<IllegalArgumentException> {
            request.validateExactPayload(attachedPayload)
        }
        assertFailsWith<IllegalArgumentException> {
            RegisterOfflineDeviceAttestation(
                TEST_NETWORK_ID,
                accountId,
                registration,
                1_900_000_000_000,
                0,
                1,
                TEST_FEE_PAYMENT,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            RegisterOfflineDeviceAttestation(
                TEST_NETWORK_ID,
                accountId,
                registration,
                1_900_000_000_000,
                1,
                0,
                TEST_FEE_PAYMENT,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            RegisterOfflineDeviceAttestation(
                TEST_NETWORK_ID,
                accountId,
                registration,
                Long.MAX_VALUE - 1,
                2,
                1,
                TEST_FEE_PAYMENT,
            )
        }
        assertFailsWith<IllegalArgumentException> {
            RegisterOfflineDeviceAttestation(
                TEST_NETWORK_ID,
                accountId,
                registration,
                registration.expiresAtMs - 1,
                2,
                1,
                TEST_FEE_PAYMENT,
            )
        }
    }

    private class RawPayloadAdapter(
        private val appendUnknownField: Boolean,
    ) : TypeAdapter<ByteArray> {
        override fun encode(encoder: NoritoEncoder, value: ByteArray) {
            encoder.writeBytes(value)
            if (appendUnknownField) {
                encoder.writeLength(1, true)
                encoder.writeByte(0)
            }
        }

        override fun decode(decoder: NoritoDecoder): ByteArray =
            decoder.readBytes(decoder.remaining())
    }

    companion object {
        private val TEST_NETWORK_ID = NetworkId.parse(
            "32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149",
        )
        private const val P256_GENERATOR =
            "04" +
                "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296" +
                "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"
        private val TEST_FEE_PAYMENT = FeePaymentIntent.authority(emptyList())

        private fun request(registration: DeviceAttestationRegistration) =
            RegisterOfflineDeviceAttestation(
                networkId = TEST_NETWORK_ID,
                authority = registration.accountId,
                registration = registration,
                creationTimeMs = 1_900_000_000_000,
                timeToLiveMs = 60_000,
                nonce = 7,
                feePayment = TEST_FEE_PAYMENT,
            )

        private fun registration(
            accountId: String,
            packageName: String = "org.hyperledger.iroha.abi22.fixture",
            signingCertificate: ByteArray =
                sha256(bytes("abi22-unit-test-signing-certificate")),
            attestedProperties: OfflineAndroidAttestedDevicePropertiesV2? = null,
            challengeHash: ByteArray? = null,
            reportHash: ByteArray? = null,
            evidenceHash: ByteArray? = null,
            evidence: ByteArray? = null,
        ): DeviceAttestationRegistration {
            val assertionPublicKey = hexToBytes(P256_GENERATOR)
            return DeviceAttestationRegistration(
                version = 1,
                platform = DeviceAttestationRegistration.ANDROID_KEYMINT_PLATFORM,
                keyId = hexLower(sha256(assertionPublicKey)),
                deviceId = "abi22-android-unit-test-device",
                accountId = accountId,
                assetDefinitionId = null,
                iosTeamId = null,
                iosBundleId = null,
                iosEnvironment = null,
                androidPackageName = packageName,
                androidSigningCertificateSha256 = signingCertificate,
                androidAttestedDeviceProperties = attestedProperties,
                publicKey = KagemushaDevicePublicKeyV2(hexToBytes(P256_GENERATOR)),
                assertionScheme =
                    DeviceAttestationRegistration.ANDROID_KEYMINT_ASSERTION_SCHEME,
                assertionKeyAlgorithm =
                    DeviceAttestationRegistration.ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM,
                assertionPublicKey = assertionPublicKey,
                assertionUsageCountLimit = 1,
                oneUse = true,
                challengeHash = challengeHash,
                attestationReportHash = reportHash,
                attestationReport =
                    bytes("abi22-unit-test-not-physical-attestation-evidence"),
                evidenceHash = evidenceHash,
                evidence = evidence,
                recentBlockHeight = 42,
                recentBlockHash = IrohaHash.prehash(bytes("abi22-unit-test-block")),
                expiresAtMs = 2_000_000_000_000,
            )
        }

        private fun bytes(value: String): ByteArray =
            value.toByteArray(StandardCharsets.UTF_8)

        private fun sha256(value: ByteArray): ByteArray =
            MessageDigest.getInstance("SHA-256").digest(value)

        private fun hexLower(value: ByteArray): String =
            value.joinToString(separator = "") { "%02x".format(it.toInt() and 0xff) }

        private fun hexToBytes(value: String): ByteArray {
            require(value.length % 2 == 0)
            return ByteArray(value.length / 2) { index ->
                val high = Character.digit(value[index * 2], 16)
                val low = Character.digit(value[index * 2 + 1], 16)
                ((high shl 4) or low).toByte()
            }
        }
    }
}
