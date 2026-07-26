// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.io.ByteArrayOutputStream
import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.address.AssetDefinitionIdEncoder
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.instructions.TransferWirePayloadEncoder
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/** Package-private canonical Norito codec for the current registration instruction. */
internal object OfflineDeviceAttestationCodec {
    private val decodeChainDiscriminant = ThreadLocal<Int?>()
    const val REGISTRATION_SCHEMA: String =
        "iroha_data_model::offline::OfflineDeviceAttestationRegistration"
    const val CHALLENGE_SCHEMA: String =
        "iroha_data_model::offline::OfflineDeviceAttestationChallengePreimage"
    const val ANDROID_CHALLENGE_SCHEMA: String =
        "iroha_data_model::offline::OfflineAndroidKeyMintChallengePreimage"
    const val INSTRUCTION_SCHEMA: String =
        "iroha_data_model::isi::offline::RegisterOfflineDeviceAttestation"

    private val registrationAdapter = RegistrationAdapter()
    private val instructionAdapter = InstructionAdapter()
    private val challengeAdapter = ChallengeAdapter()
    private val androidChallengeAdapter = AndroidChallengeAdapter()

    fun encodeRegistration(registration: DeviceAttestationRegistration): ByteArray =
        NoritoCodec.encode(
            registration,
            REGISTRATION_SCHEMA,
            registrationAdapter,
            NoritoHeader.COMPACT_LEN,
        )

    fun decodeRegistrationCanonical(
        archive: ByteArray,
        chainDiscriminant: Int,
    ): DeviceAttestationRegistration =
        withDecodeChain(chainDiscriminant) {
            val snapshot = archive.copyOf()
            val view = NoritoCodec.fromBytesView(snapshot, REGISTRATION_SCHEMA)
            require(view.flags == NoritoHeader.COMPACT_LEN) {
                "registration archive must use canonical compact lengths"
            }
            val decoded = view.decode(registrationAdapter)
            require(snapshot.contentEquals(encodeRegistration(decoded))) {
                "registration archive is not canonically encoded"
            }
            decoded
        }

    fun encodeInstructionPayload(registration: DeviceAttestationRegistration): ByteArray =
        NoritoCodec.encode(
            registration,
            INSTRUCTION_SCHEMA,
            instructionAdapter,
            NoritoHeader.COMPACT_LEN,
        )

    fun decodeInstructionPayloadCanonical(
        archive: ByteArray,
        chainDiscriminant: Int,
    ): DeviceAttestationRegistration =
        withDecodeChain(chainDiscriminant) {
            val snapshot = archive.copyOf()
            val view = NoritoCodec.fromBytesView(snapshot, INSTRUCTION_SCHEMA)
            require(view.flags == NoritoHeader.COMPACT_LEN) {
                "instruction archive must use canonical compact lengths"
            }
            val decoded = view.decode(instructionAdapter)
            require(snapshot.contentEquals(encodeInstructionPayload(decoded))) {
                "instruction archive is not canonically encoded"
            }
            decoded
        }

    fun instruction(registration: DeviceAttestationRegistration): InstructionBox =
        InstructionBox.fromWirePayload(INSTRUCTION_SCHEMA, encodeInstructionPayload(registration))

    fun canonicalChallengeHash(value: DeviceAttestationRegistration): ByteArray {
        if (value.platform == DeviceAttestationRegistration.ANDROID_KEYMINT_PLATFORM) {
            return androidPreKeyGenerationChallengeHash(
                value.version,
                value.deviceId,
                value.accountId,
                value.assetDefinitionId,
                requireNotNull(value.androidPackageName),
                requireNotNull(value.androidSigningCertificateSha256),
                value.publicKey.sec1Bytes(),
                value.recentBlockHeight,
                value.recentBlockHash,
                value.expiresAtMs,
            )
        }
        return IrohaHash.prehash(
            NoritoCodec.encode(
                Challenge(value),
                CHALLENGE_SCHEMA,
                challengeAdapter,
                NoritoHeader.COMPACT_LEN,
            ),
        )
    }

    fun androidPreKeyGenerationChallengeHash(
        version: Int,
        deviceId: String,
        accountId: String,
        assetDefinitionId: String?,
        androidPackageName: String,
        androidSigningCertificateSha256: ByteArray,
        publicKey: ByteArray,
        recentBlockHeight: Long,
        recentBlockHash: ByteArray,
        expiresAtMs: Long,
    ): ByteArray {
        require(version == DeviceAttestationRegistration.REGISTRATION_VERSION) {
            "registration version must be exactly 1"
        }
        val challenge = AndroidChallenge(
            deviceId,
            accountId,
            assetDefinitionId,
            androidPackageName,
            androidSigningCertificateSha256,
            KagemushaP256Codec.requireUncompressedPublicKey(publicKey),
            recentBlockHeight,
            recentBlockHash,
            expiresAtMs,
        )
        return IrohaHash.prehash(
            NoritoCodec.encode(
                challenge,
                ANDROID_CHALLENGE_SCHEMA,
                androidChallengeAdapter,
                NoritoHeader.COMPACT_LEN,
            ),
        )
    }

    fun validateAccountId(accountId: String) {
        TransferWirePayloadEncoder.encodeAccountIdPayload(accountId)
    }

    private class RegistrationAdapter : TypeAdapter<DeviceAttestationRegistration> {
        override fun encode(encoder: NoritoEncoder, value: DeviceAttestationRegistration) {
            field(encoder) { it.writeUInt(value.version.toLong(), 16) }
            field(encoder) { string(it, value.platform) }
            field(encoder) { string(it, value.keyId) }
            field(encoder) { string(it, value.deviceId) }
            field(encoder) { it.writeBytes(TransferWirePayloadEncoder.encodeAccountIdPayload(value.accountId)) }
            field(encoder) { optionAssetDefinitionId(it, value.assetDefinitionId) }
            field(encoder) { optionString(it, value.iosTeamId) }
            field(encoder) { optionString(it, value.iosBundleId) }
            field(encoder) { optionString(it, value.iosEnvironment) }
            field(encoder) { optionString(it, value.androidPackageName) }
            field(encoder) { optionBytes(it, value.androidSigningCertificateSha256) }
            field(encoder) { p256PublicKey(it, value.publicKey.sec1Bytes()) }
            field(encoder) { string(it, value.assertionScheme) }
            field(encoder) { string(it, value.assertionKeyAlgorithm) }
            field(encoder) { bytes(it, value.assertionPublicKey) }
            field(encoder) { optionU32(it, value.assertionUsageCountLimit) }
            field(encoder) { it.writeByte(if (value.oneUse) 1 else 0) }
            field(encoder) { it.writeBytes(value.challengeHash) }
            field(encoder) { it.writeBytes(value.attestationReportHash) }
            field(encoder) { bytes(it, value.attestationReport) }
            field(encoder) { it.writeBytes(value.evidenceHash) }
            field(encoder) { bytes(it, value.evidence) }
            field(encoder) { it.writeUInt(value.recentBlockHeight, 64) }
            field(encoder) { it.writeBytes(value.recentBlockHash) }
            field(encoder) { it.writeUInt(value.expiresAtMs, 64) }
        }

        override fun decode(decoder: NoritoDecoder): DeviceAttestationRegistration =
            DeviceAttestationRegistration(
                version = readField(decoder) { checkedU16(it.readUInt(16)) },
                platform = readField(decoder, ::readString),
                keyId = readField(decoder, ::readString),
                deviceId = readField(decoder, ::readString),
                accountId = readField(decoder) {
                    TransferWirePayloadEncoder.decodeAccountIdPayload(
                        it.readBytes(it.remaining()),
                        requiredDecodeChainDiscriminant(),
                        it.flags,
                        it.flagsHint,
                    )
                },
                assetDefinitionId = readField(decoder, ::readOptionAssetDefinitionId),
                iosTeamId = readField(decoder, ::readOptionString),
                iosBundleId = readField(decoder, ::readOptionString),
                iosEnvironment = readField(decoder, ::readOptionString),
                androidPackageName = readField(decoder, ::readOptionString),
                androidSigningCertificateSha256 = readField(decoder, ::readOptionBytes),
                publicKey = KagemushaDevicePublicKeyV2(readField(decoder, ::readP256PublicKey)),
                assertionScheme = readField(decoder, ::readString),
                assertionKeyAlgorithm = readField(decoder, ::readString),
                assertionPublicKey = readField(decoder, ::readBytes),
                assertionUsageCountLimit = readField(decoder, ::readOptionU32),
                oneUse = readField(decoder, ::readBool),
                challengeHash = readField(decoder) { readHash(it, "challenge_hash") },
                attestationReportHash = readField(decoder) { readHash(it, "attestation_report_hash") },
                attestationReport = readField(decoder, ::readBytes),
                evidenceHash = readField(decoder) { readHash(it, "evidence_hash") },
                evidence = readField(decoder, ::readBytes),
                recentBlockHeight = readField(decoder) { it.readUInt(64) },
                recentBlockHash = readField(decoder) { readHash(it, "recent_block_hash") },
                expiresAtMs = readField(decoder) { it.readUInt(64) },
            )
    }

    private fun requiredDecodeChainDiscriminant(): Int =
        checkNotNull(decodeChainDiscriminant.get()) {
            "offline attestation decoding requires an explicit chainDiscriminant"
        }

    private fun <T> withDecodeChain(
        chainDiscriminant: Int,
        operation: () -> T,
    ): T {
        require(chainDiscriminant in 0..0xffff) {
            "chainDiscriminant must fit in u16"
        }
        val previous = decodeChainDiscriminant.get()
        check(previous == null || previous == chainDiscriminant) {
            "Conflicting nested chainDiscriminant context"
        }
        decodeChainDiscriminant.set(chainDiscriminant)
        return try {
            operation()
        } finally {
            if (previous == null) {
                decodeChainDiscriminant.remove()
            } else {
                decodeChainDiscriminant.set(previous)
            }
        }
    }

    private class InstructionAdapter : TypeAdapter<DeviceAttestationRegistration> {
        override fun encode(encoder: NoritoEncoder, value: DeviceAttestationRegistration) {
            field(encoder) { registrationAdapter.encode(it, value) }
        }

        override fun decode(decoder: NoritoDecoder): DeviceAttestationRegistration =
            readField(decoder, registrationAdapter::decode)
    }

    private class ChallengeAdapter : TypeAdapter<Challenge> {
        override fun encode(encoder: NoritoEncoder, value: Challenge) {
            val registration = value.registration
            field(encoder) { string(it, DeviceAttestationRegistration.DEVICE_ATTESTATION_CHALLENGE_DOMAIN) }
            field(encoder) { it.writeUInt(registration.version.toLong(), 16) }
            field(encoder) { string(it, registration.platform) }
            field(encoder) { string(it, registration.keyId) }
            field(encoder) { string(it, registration.deviceId) }
            field(encoder) { it.writeBytes(TransferWirePayloadEncoder.encodeAccountIdPayload(registration.accountId)) }
            field(encoder) { optionAssetDefinitionId(it, registration.assetDefinitionId) }
            field(encoder) { optionString(it, registration.iosTeamId) }
            field(encoder) { optionString(it, registration.iosBundleId) }
            field(encoder) { optionString(it, registration.iosEnvironment) }
            field(encoder) { optionString(it, registration.androidPackageName) }
            field(encoder) { optionBytes(it, registration.androidSigningCertificateSha256) }
            field(encoder) { p256PublicKey(it, registration.publicKey.sec1Bytes()) }
            field(encoder) { string(it, registration.assertionScheme) }
            field(encoder) { string(it, registration.assertionKeyAlgorithm) }
            field(encoder) { optionU32(it, registration.assertionUsageCountLimit) }
            field(encoder) { it.writeByte(if (registration.oneUse) 1 else 0) }
            field(encoder) { it.writeUInt(registration.recentBlockHeight, 64) }
            field(encoder) { it.writeBytes(registration.recentBlockHash) }
            field(encoder) { it.writeUInt(registration.expiresAtMs, 64) }
        }

        override fun decode(decoder: NoritoDecoder): Challenge =
            throw UnsupportedOperationException("challenge preimages are encode-only")
    }

    private class AndroidChallengeAdapter : TypeAdapter<AndroidChallenge> {
        override fun encode(encoder: NoritoEncoder, value: AndroidChallenge) {
            field(encoder) { string(it, DeviceAttestationRegistration.DEVICE_ATTESTATION_CHALLENGE_DOMAIN) }
            field(encoder) { it.writeUInt(DeviceAttestationRegistration.REGISTRATION_VERSION.toLong(), 16) }
            field(encoder) { string(it, DeviceAttestationRegistration.ANDROID_KEYMINT_PLATFORM) }
            field(encoder) { string(it, value.deviceId) }
            field(encoder) { it.writeBytes(TransferWirePayloadEncoder.encodeAccountIdPayload(value.accountId)) }
            field(encoder) { optionAssetDefinitionId(it, value.assetDefinitionId) }
            field(encoder) { optionString(it, null) }
            field(encoder) { optionString(it, null) }
            field(encoder) { optionString(it, null) }
            field(encoder) { optionString(it, value.androidPackageName) }
            field(encoder) { optionBytes(it, value.androidSigningCertificateSha256) }
            field(encoder) { p256PublicKey(it, value.publicKey) }
            field(encoder) { string(it, DeviceAttestationRegistration.ANDROID_KEYMINT_ASSERTION_SCHEME) }
            field(encoder) { string(it, DeviceAttestationRegistration.ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM) }
            field(encoder) { optionU32(it, 1) }
            field(encoder) { it.writeByte(1) }
            field(encoder) { it.writeUInt(value.recentBlockHeight, 64) }
            field(encoder) { it.writeBytes(value.recentBlockHash) }
            field(encoder) { it.writeUInt(value.expiresAtMs, 64) }
        }

        override fun decode(decoder: NoritoDecoder): AndroidChallenge =
            throw UnsupportedOperationException("challenge preimages are encode-only")
    }

    private class Challenge(val registration: DeviceAttestationRegistration)

    private class AndroidChallenge(
        val deviceId: String,
        val accountId: String,
        val assetDefinitionId: String?,
        val androidPackageName: String,
        androidSigningCertificateSha256: ByteArray,
        publicKey: ByteArray,
        val recentBlockHeight: Long,
        recentBlockHash: ByteArray,
        val expiresAtMs: Long,
    ) {
        val androidSigningCertificateSha256 = androidSigningCertificateSha256.copyOf()
        val publicKey = publicKey.copyOf()
        val recentBlockHash = recentBlockHash.copyOf()

        init {
            exact(deviceId, "device_id")
            exact(accountId, "account_id")
            exact(androidPackageName, "android_package_name")
            validateAccountId(accountId)
            assetDefinitionId?.let(AssetDefinitionIdEncoder::parseAddressBytes)
            require(this.androidSigningCertificateSha256.size == 32) {
                "android_signing_certificate_sha256 must be 32 bytes"
            }
            KagemushaP256Codec.requireUncompressedPublicKey(this.publicKey)
            require(recentBlockHeight > 0 && expiresAtMs > 0) {
                "challenge lifetime fields must be positive"
            }
            DeviceAttestationRegistration.requireHash(this.recentBlockHash, "recent_block_hash")
        }
    }

    private fun field(parent: NoritoEncoder, writer: (NoritoEncoder) -> Unit) {
        val child = parent.childEncoder()
        writer(child)
        val payload = child.toByteArray()
        parent.writeLength(payload.size.toLong(), compact(parent))
        parent.writeBytes(payload)
    }

    private fun <T> readField(parent: NoritoDecoder, reader: (NoritoDecoder) -> T): T {
        val length = checkedLength(parent.readLength(compact(parent)), "field")
        val child = NoritoDecoder(parent.readBytes(length), parent.flags, parent.flagsHint)
        val value = reader(child)
        require(child.remaining() == 0) { "field has trailing bytes" }
        return value
    }

    private fun string(encoder: NoritoEncoder, value: String) {
        val bytes = value.toByteArray(StandardCharsets.UTF_8)
        encoder.writeLength(bytes.size.toLong(), compact(encoder))
        encoder.writeBytes(bytes)
    }

    private fun readString(decoder: NoritoDecoder): String {
        val length = checkedLength(decoder.readLength(compact(decoder)), "string")
        val bytes = decoder.readBytes(length)
        val value = String(bytes, StandardCharsets.UTF_8)
        require(bytes.contentEquals(value.toByteArray(StandardCharsets.UTF_8))) {
            "string is not canonical UTF-8"
        }
        return value
    }

    private fun bytes(encoder: NoritoEncoder, value: ByteArray) {
        encoder.writeUInt(value.size.toLong(), 64)
        encoder.writeBytes(value)
    }

    private fun readBytes(decoder: NoritoDecoder): ByteArray =
        decoder.readBytes(checkedLength(decoder.readUInt(64), "byte vector"))

    private fun p256PublicKey(encoder: NoritoEncoder, value: ByteArray) {
        val key = KagemushaP256Codec.requireUncompressedPublicKey(value)
        encoder.writeBytes(key)
    }

    private fun readP256PublicKey(decoder: NoritoDecoder): ByteArray {
        require(decoder.remaining() == KagemushaP256Codec.PUBLIC_KEY_BYTES) {
            "P-256 public key must contain exactly 65 bytes"
        }
        return KagemushaP256Codec.requireUncompressedPublicKey(
            decoder.readBytes(KagemushaP256Codec.PUBLIC_KEY_BYTES),
        )
    }

    private fun optionString(encoder: NoritoEncoder, value: String?) {
        option(encoder, value) { string(it, requireNotNull(value)) }
    }

    private fun readOptionString(decoder: NoritoDecoder): String? = when (decoder.readByte()) {
        0 -> null
        1 -> readField(decoder, ::readString)
        else -> throw IllegalArgumentException("invalid option tag")
    }

    private fun optionBytes(encoder: NoritoEncoder, value: ByteArray?) {
        option(encoder, value) { bytes(it, requireNotNull(value)) }
    }

    private fun readOptionBytes(decoder: NoritoDecoder): ByteArray? = when (decoder.readByte()) {
        0 -> null
        1 -> readField(decoder, ::readBytes)
        else -> throw IllegalArgumentException("invalid option tag")
    }

    private fun optionU32(encoder: NoritoEncoder, value: Int?) {
        option(encoder, value) { it.writeUInt(requireNotNull(value).toLong(), 32) }
    }

    private fun readOptionU32(decoder: NoritoDecoder): Int? = when (decoder.readByte()) {
        0 -> null
        1 -> readField(decoder) { checkedU32(it.readUInt(32)) }
        else -> throw IllegalArgumentException("invalid option tag")
    }

    private fun optionAssetDefinitionId(encoder: NoritoEncoder, value: String?) {
        option(encoder, value) { child ->
            for (item in AssetDefinitionIdEncoder.parseAddressBytes(requireNotNull(value))) {
                child.writeLength(1, compact(child))
                child.writeByte(item.toInt())
            }
        }
    }

    private fun readOptionAssetDefinitionId(decoder: NoritoDecoder): String? =
        when (decoder.readByte()) {
            0 -> null
            1 -> readField(decoder) { child ->
                val out = ByteArrayOutputStream()
                while (child.remaining() > 0) {
                    require(child.readLength(compact(child)) == 1L) {
                        "asset definition byte length must be one"
                    }
                    out.write(child.readByte())
                }
                AssetDefinitionIdEncoder.encodeFromBytes(out.toByteArray())
            }
            else -> throw IllegalArgumentException("invalid option tag")
        }

    private fun option(
        encoder: NoritoEncoder,
        value: Any?,
        presentWriter: (NoritoEncoder) -> Unit,
    ) {
        if (value == null) {
            encoder.writeByte(0)
        } else {
            encoder.writeByte(1)
            field(encoder, presentWriter)
        }
    }

    private fun readBool(decoder: NoritoDecoder): Boolean = when (decoder.readByte()) {
        0 -> false
        1 -> true
        else -> throw IllegalArgumentException("invalid boolean tag")
    }

    private fun readHash(decoder: NoritoDecoder, field: String): ByteArray =
        decoder.readBytes(32).also { DeviceAttestationRegistration.requireHash(it, field) }

    private fun checkedU16(value: Long): Int {
        require(value in 0..0xffffL) { "u16 exceeds JVM range" }
        return value.toInt()
    }

    private fun checkedU32(value: Long): Int {
        require(value in 0..Int.MAX_VALUE.toLong()) { "u32 exceeds supported JVM range" }
        return value.toInt()
    }

    private fun checkedLength(value: Long, field: String): Int {
        require(value in 0..Int.MAX_VALUE.toLong()) { "$field length exceeds JVM range" }
        return value.toInt()
    }

    private fun compact(encoder: NoritoEncoder): Boolean =
        (encoder.flags and NoritoHeader.COMPACT_LEN) != 0

    private fun compact(decoder: NoritoDecoder): Boolean =
        (decoder.flags and NoritoHeader.COMPACT_LEN) != 0

    private fun exact(value: String, field: String) {
        require(value.isNotEmpty() && value == value.trim()) {
            "$field must be exact non-empty text"
        }
    }
}
