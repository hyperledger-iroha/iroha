package org.hyperledger.iroha.sdk.offline

import java.util.EnumSet
import kotlin.test.*
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoHeader

class KagemushaOperationIntentV1Test {
    @Test fun `lost sync response retains exact ID and command across owner recreation`() {
        val store = TestOperationIntentStoreV1().apply { throwAfterSave = true }
        val original = KagemushaOperationIntentOwnerV1(store)
        assertFailsWith<IllegalStateException> {
            original.beginInternal(KagemushaDeviceControlCommandV1::BootstrapAggregateState)
        }
        store.throwAfterSave = false
        val saved = store.pendingInternal().single()
        val recovered = KagemushaOperationIntentOwnerV1(store)
            .beginInternal(KagemushaDeviceControlCommandV1::BootstrapAggregateState)
        assertContentEquals(saved.operationId(), recovered.operationId())
        assertContentEquals(saved.publicBinding(), recovered.publicBinding())
        assertEquals(1, store.pendingInternal().size)
    }

    @Test fun `pending fold resumes original selector and rejects different credit after restart`() {
        val store = TestOperationIntentStoreV1()
        val selector = KagemushaPendingCreditSelectorV1(KagemushaPendingCreditKindV1.RECEIVE, ByteArray(32) { 3 })
        val first = KagemushaOperationIntentOwnerV1(store).beginInternal {
            KagemushaDeviceControlCommandV1.FoldReceiveCredit(it, selector)
        }
        val resumed = KagemushaOperationIntentOwnerV1(store).beginInternal {
            KagemushaDeviceControlCommandV1.FoldReceiveCredit(it, selector)
        }
        assertContentEquals(first.operationId(), resumed.operationId())
        val decoded = KagemushaDeviceOperationCodecV1.decodeControlCommand(17, resumed.operationId(), resumed.publicBinding())
            as KagemushaDeviceControlCommandV1.FoldReceiveCredit
        assertContentEquals(selector.creditId(), decoded.selector.creditId())
        assertFailsWith<IllegalArgumentException> {
            KagemushaOperationIntentOwnerV1(store).beginInternal {
                KagemushaDeviceControlCommandV1.FoldReceiveCredit(it,
                    KagemushaPendingCreditSelectorV1(KagemushaPendingCreditKindV1.RECEIVE, ByteArray(32) { 4 }))
            }
        }
        assertEquals(1, store.pendingInternal().size)
    }

    @Test fun `identical new actions retain distinct IDs and conflicting reuse never mutates history`() {
        val store = TestOperationIntentStoreV1()
        val owner = KagemushaOperationIntentOwnerV1(store)
        val first = ByteArray(32) { 1 }
        val second = ByteArray(32) { 2 }
        owner.reserve(5, first, byteArrayOf(7))
        owner.reserve(5, second, byteArrayOf(7))
        assertFailsWith<IllegalArgumentException> { owner.reserve(5, first, byteArrayOf(8)) }
        assertContentEquals(byteArrayOf(7), store.load(5, first)!!.publicBinding())
        assertContentEquals(second, store.load(5, second)!!.operationId())
    }

    @Test fun `acknowledgement requires accepted reply and retains an immutable complete transcript`() {
        val store = TestOperationIntentStoreV1()
        val owner = KagemushaOperationIntentOwnerV1(store)
        val pending = owner.beginInternal(KagemushaDeviceControlCommandV1::BootstrapAggregateState)
        val id = pending.operationId()
        assertFailsWith<IllegalArgumentException> { owner.acknowledge(20, id) }
        owner.dispatched(20, id, pending.publicBinding(), byteArrayOf(5), KagemushaOperationIntentPurposeV1.INTERNAL)
        val signature = ByteArray(64).also { it[31] = 1; it[63] = 1 }
        owner.accepted(20, id, byteArrayOf(6), signature, byteArrayOf(5))
        val beforeAck = store.load(20, id)!!
        assertFalse(beforeAck.acknowledged)
        assertEquals(1, store.pendingInternal().size)
        assertFailsWith<IllegalArgumentException> { owner.acknowledge(20, id) }
        owner.reconciled(20, id, byteArrayOf(8))
        owner.acknowledge(20, id)
        assertTrue(store.load(20, id)!!.acknowledged)
        assertTrue(store.pendingInternal().isEmpty())
        assertContentEquals(signature, store.load(20, id)!!.responseAuthenticator())
        assertFailsWith<IllegalArgumentException> { owner.accepted(20, id, byteArrayOf(9), signature, byteArrayOf(5)) }
        val next = owner.beginInternal(KagemushaDeviceControlCommandV1::BootstrapAggregateState)
        assertFalse(next.operationId().contentEquals(id))
    }

    @Test fun `record codec rejects corrupt tails and retired schema while retaining defensive copies`() {
        val scope = byteArrayOf(1)
        val id = ByteArray(32) { 2 }
        val binding = byteArrayOf(3)
        val record = KagemushaOperationIntentV1(scope, 5, id, KagemushaOperationIntentPurposeV1.CALLER, binding)
        scope.fill(7); id.fill(7); binding.fill(7)
        assertContentEquals(byteArrayOf(1), record.scope())
        val bytes = KagemushaOperationIntentCodecV1.encode(record)
        val decoded = KagemushaOperationIntentCodecV1.decodeExact(bytes)
        assertContentEquals(ByteArray(32) { 2 }, decoded.operationId())
        decoded.operationId().fill(0)
        assertContentEquals(ByteArray(32) { 2 }, decoded.operationId())
        assertFailsWith<IllegalArgumentException> { KagemushaOperationIntentCodecV1.decodeExact(bytes + 0) }
        val corrupt = bytes.copyOf().also { it[it.lastIndex] = (it.last().toInt() xor 1).toByte() }
        assertFailsWith<IllegalArgumentException> { KagemushaOperationIntentCodecV1.decodeExact(corrupt) }
    }

    @Test fun `accepted retry rejects changed authenticator or historical qualification after owner recreation`() {
        val store = TestOperationIntentStoreV1()
        val first = KagemushaOperationIntentOwnerV1(store)
        val id = ByteArray(32) { 2 }
        val command = byteArrayOf(3)
        val qualification = byteArrayOf(4)
        val reply = byteArrayOf(5)
        val signature = ByteArray(64).also { it[31] = 1; it[63] = 1 }
        first.dispatched(5, id, command, qualification)
        first.accepted(5, id, reply, signature, qualification)
        val resumed = KagemushaOperationIntentOwnerV1(store)
        resumed.accepted(5, id, reply, signature, qualification)
        val saved = KagemushaOperationIntentCodecV1.encode(store.load(5, id)!!)
        assertFailsWith<IllegalArgumentException> {
            resumed.accepted(5, id, reply, signature.copyOf().also { it[63] = 2 }, qualification)
        }
        assertFailsWith<IllegalArgumentException> {
            resumed.accepted(5, id, reply, signature, byteArrayOf(6))
        }
        assertContentEquals(saved, KagemushaOperationIntentCodecV1.encode(store.load(5, id)!!))
    }

    @Test fun `scope replacement cannot admit old pending intents`() {
        val store = TestOperationIntentStoreV1()
        val owner = KagemushaOperationIntentOwnerV1(store)
        val retained = owner.beginInternal(KagemushaDeviceControlCommandV1::BootstrapAggregateState)
        store.scopeValue = byteArrayOf(43)
        assertFailsWith<IllegalArgumentException> { owner.reserve(5, ByteArray(32) { 2 }, byteArrayOf(3)) }
        assertFailsWith<IllegalArgumentException> { owner.load(20, retained.operationId()) }
        assertFailsWith<IllegalArgumentException> { owner.pendingInternal() }
        assertFailsWith<IllegalArgumentException> { KagemushaOperationIntentOwnerV1(store).pendingInternal() }
    }

    @Test fun `read operations cannot enter the durable operation store`() {
        for (operation in listOf(1, 13, 18, 21)) {
            assertFailsWith<IllegalArgumentException> {
                KagemushaOperationIntentV1(byteArrayOf(1), operation, ByteArray(32) { 2 },
                    KagemushaOperationIntentPurposeV1.CALLER, byteArrayOf(3))
            }
        }
    }

    @Test fun `Core recovered terminal result remains immutable when the original install reply was lost`() {
        val store = TestOperationIntentStoreV1()
        val owner = KagemushaOperationIntentOwnerV1(store)
        val id = ByteArray(32) { 12 }
        owner.reserve(9, id, byteArrayOf(9))
        owner.dispatched(9, id, byteArrayOf(9), byteArrayOf(5))
        owner.reserve(10, id, byteArrayOf(10))
        owner.dispatched(10, id, byteArrayOf(10), byteArrayOf(5))
        owner.accepted(10, id, byteArrayOf(6), ByteArray(64).also { it[31] = 1; it[63] = 1 }, byteArrayOf(5))
        owner.completedResult(10, id, byteArrayOf(7))
        owner.acknowledge(10, id)
        assertNull(store.load(9, id)!!.canonicalReply())
        assertContentEquals(byteArrayOf(7), store.load(10, id)!!.canonicalResult())
        assertFailsWith<IllegalArgumentException> { owner.completedResult(10, id, byteArrayOf(8)) }
    }

    @Test fun `intent codec rejects compression and nonzero layouts before decoding fields`() {
        val record = KagemushaOperationIntentV1(byteArrayOf(1), 5, ByteArray(32) { 2 },
            KagemushaOperationIntentPurposeV1.CALLER, byteArrayOf(3))
        val encoded = KagemushaOperationIntentCodecV1.encode(record)
        for (candidate in listOf(
            withArchiveHeader(encoded, compression = NoritoHeader.COMPRESSION_ZSTD),
            withArchiveHeader(encoded, flags = NoritoHeader.COMPACT_LEN),
        )) {
            val error = assertFailsWith<IllegalArgumentException> {
                KagemushaOperationIntentCodecV1.decodeExact(candidate)
            }
            assertEquals("operation intent requires an uncompressed archive with zero layout flags", error.message)
        }
        assertContentEquals(encoded, KagemushaOperationIntentCodecV1.encode(
            KagemushaOperationIntentCodecV1.decodeExact(encoded)))
    }

    @Test fun `qualification codec rejects compression and nonzero layouts before decoding fields`() {
        val encoded = KagemushaOperationIntentCodecV1.encodeQualification(structuralQualification())
        for (candidate in listOf(
            withArchiveHeader(encoded, compression = NoritoHeader.COMPRESSION_ZSTD),
            withArchiveHeader(encoded, flags = NoritoHeader.COMPACT_LEN),
        )) {
            val error = assertFailsWith<IllegalArgumentException> {
                KagemushaOperationIntentCodecV1.decodeQualification(candidate)
            }
            assertEquals("operation qualification requires an uncompressed archive with zero layout flags", error.message)
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaOperationIntentCodecV1.decodeQualification(ByteArray(0))
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaOperationIntentCodecV1.decodeQualification(ByteArray(4097))
        }
    }

    @Test fun `qualification codec retains exact canonical history and defensive copies`() {
        val encoded = KagemushaOperationIntentCodecV1.encodeQualification(structuralQualification())
        val expected = encoded.copyOf()
        val decoded = KagemushaOperationIntentCodecV1.decodeQualification(encoded)
        encoded.fill(0)
        decoded.releaseId().fill(0)
        assertContentEquals(expected, KagemushaOperationIntentCodecV1.encodeQualification(decoded))
        assertFailsWith<IllegalArgumentException> {
            KagemushaOperationIntentCodecV1.decodeQualification(expected + 0)
        }
        val corrupt = expected.copyOf().also { it[it.lastIndex] = (it.last().toInt() xor 1).toByte() }
        assertFailsWith<IllegalArgumentException> {
            KagemushaOperationIntentCodecV1.decodeQualification(corrupt)
        }
    }

    @Test fun `intent and qualification codecs retain enclosing decode flags on success and failure`() {
        val intent = KagemushaOperationIntentCodecV1.encode(KagemushaOperationIntentV1(
            byteArrayOf(1), 5, ByteArray(32) { 2 }, KagemushaOperationIntentPurposeV1.CALLER, byteArrayOf(3)))
        val qualification = KagemushaOperationIntentCodecV1.encodeQualification(structuralQualification())
        val previousFlags = NoritoCodec.effectiveDecodeFlags()
        NoritoCodec.DecodeFlagsGuard.enter(NoritoHeader.COMPACT_LEN).use {
            KagemushaOperationIntentCodecV1.decodeExact(intent)
            assertEquals(NoritoHeader.COMPACT_LEN, NoritoCodec.effectiveDecodeFlags())
            KagemushaOperationIntentCodecV1.decodeQualification(qualification)
            assertEquals(NoritoHeader.COMPACT_LEN, NoritoCodec.effectiveDecodeFlags())
            assertFailsWith<IllegalArgumentException> {
                KagemushaOperationIntentCodecV1.decodeExact(withArchiveHeader(intent,
                    compression = NoritoHeader.COMPRESSION_ZSTD))
            }
            assertEquals(NoritoHeader.COMPACT_LEN, NoritoCodec.effectiveDecodeFlags())
            assertFailsWith<IllegalArgumentException> {
                KagemushaOperationIntentCodecV1.decodeQualification(withArchiveHeader(qualification,
                    flags = NoritoHeader.COMPACT_LEN))
            }
            assertEquals(NoritoHeader.COMPACT_LEN, NoritoCodec.effectiveDecodeFlags())
        }
        assertEquals(previousFlags, NoritoCodec.effectiveDecodeFlags())
    }

    private fun withArchiveHeader(
        encoded: ByteArray,
        compression: Int = NoritoHeader.COMPRESSION_NONE,
        flags: Int = 0,
    ): ByteArray {
        val archive = NoritoHeader.decode(encoded, null)
        // Keep the actual and declared payload small; this tests rejection before decompression.
        return NoritoHeader(archive.header.schemaHash, archive.header.payloadLength,
            archive.header.checksum, flags, compression).encode() + archive.payload
    }

    private fun structuralQualification(): KagemushaHardwareQualificationV1 {
        // Public P-256 generator and shape-only r=s=1 signature; no hardware authentication.
        val deviceKey = KagemushaDevicePublicKeyV1(
            ("04" +
                "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296" +
                "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5")
                .chunked(2).map { it.toInt(16).toByte() }.toByteArray(),
        )
        val signature = KagemushaDeviceSignatureV1(ByteArray(64).also { it[31] = 1; it[63] = 1 })
        val profile = KagemushaHardwareProfileV1(
            version = 1, protocolVersion = 1,
            hardwareProfileId = digest(1), providerId = digest(3),
            platformClass = KagemushaHardwarePlatformClassV1.ANDROID_OEM_SERVICE,
            productClassDigest = digest(4), firmwarePolicyDigest = digest(5),
            enrollmentAttestationVerifierDigest = digest(6), attestationTrustRootsDigest = digest(7),
            allowedSuiteCommitment = digest(8), policyEpoch = 1,
            governanceCredentialPublicKey = deviceKey, capabilityMask = 0xffff,
            qualificationReportDigest = digest(9), validFromMs = 1, expiresAtMs = 20000,
        )
        val credential = KagemushaHardwareCredentialV1(
            version = 1, credentialId = digest(10), networkId = NetworkId.fromBytes(digest(11)),
            hardwareProfileId = profile.hardwareProfileId(), suiteId = digest(12),
            firmwarePolicyDigest = profile.firmwarePolicyDigest(), policyEpoch = profile.policyEpoch,
            laneCommitment = digest(13), hardwareEpochId = digest(14), hardwareEpochGeneration = 1,
            devicePublicKey = deviceKey, deviceKeyReference = digest(15),
            issuedAtMs = 10, expiresAtMs = 19000, governanceSignature = signature,
        )
        return KagemushaHardwareQualificationV1(
            protocolVersion = 1, profile = profile, credential = credential,
            releaseId = digest(16), hardwarePolicyDigest = digest(2),
            coreAuthorizationKeyReference = digest(17),
            capabilities = EnumSet.allOf(KagemushaHardwareCapabilityV1::class.java),
        )
    }

    private fun digest(value: Int): ByteArray = ByteArray(32) { value.toByte() }
}
