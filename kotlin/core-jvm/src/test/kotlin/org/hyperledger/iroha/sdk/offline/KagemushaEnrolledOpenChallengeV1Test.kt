// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.file.Files
import java.nio.file.Paths
import java.security.MessageDigest
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import org.junit.jupiter.api.Test
import kotlin.test.assertTrue
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters
import org.bouncycastle.crypto.signers.Ed25519Signer
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/** Locally constructed structural projections; no fixture here claims native authority. */
class KagemushaEnrolledOpenChallengeV1Test {
    @Test fun `both actual Rust challenge and authority archives reencode identically`() {
        for (kind in listOf("initial", "recovery")) {
            val archive = fixture("${kind}_challenge_canonical_hex")
            val value = C.decodeAccountChallengeShapeExact(archive)
            assertContentEquals(archive, C.encodeAccountChallengeShape(value))
            val source = fixture("${kind}_authority_source_canonical_hex")
            assertContentEquals(source, C.encodeAuthoritySourceShape(value.authoritySource))
            assertContentEquals(source, C.encodeAuthoritySourceShape(C.decodeAuthoritySourceShapeExact(source)))
        }
    }

    @Test fun `named authority variants reject the invented enclosing tuple format`() {
        for (kind in listOf("initial", "recovery")) {
            val bare = fixture("${kind}_authority_source_payload_hex")
            val inner = bare.copyOfRange(4, bare.size)
            val writer = NoritoEncoder(NoritoHeader.COMPACT_LEN)
            writer.writeBytes(bare.copyOfRange(0, 4))
            writer.writeLength(inner.size.toLong(), true)
            writer.writeBytes(inner)
            assertFailsWith<IllegalArgumentException> {
                C.decodeAuthoritySourceShapeExact(frame("iroha.kagemusha.v1.enrolled-open-authority-source", writer.toByteArray()))
            }
        }
    }

    @Test fun `Rust signing hashes and Ed25519 signatures verify exactly without a second hash`() {
        for (kind in listOf("initial", "recovery")) {
            val value = C.decodeAccountChallengeShapeExact(fixture("${kind}_challenge_canonical_hex"))
            val message = signMessage(value)
            assertContentEquals(fixture("${kind}_account_signing_message_hex"), message)
            val signature = fixture("${kind}_account_signature_hex")
            val publicKey = fixture("account_public_key_hex", "kagemusha_enrolled_open_selector_v1.json")
            fun verify(bytes: ByteArray): Boolean = Ed25519Signer().run {
                init(false, Ed25519PublicKeyParameters(publicKey, 0))
                update(bytes, 0, bytes.size)
                verifySignature(signature)
            }
            assertTrue(verify(message))
            assertFalse(verify(IrohaHash.prehash(message)))
            assertFalse(verify(C.encodeAccountChallengeShape(value)))
        }
    }

    @Test fun `Rust recovery checkpoint fields retain exact values above unsigned 64 bits`() {
        val value = C.decodeAccountChallengeShapeExact(fixture("recovery_challenge_canonical_hex"))
        val anchor = (value.authoritySource as KagemushaEnrolledOpenAuthoritySourceV1.RecoveryCheckpoint).statement
        assertEquals(BigInteger.ONE.shiftLeft(80).add(BigInteger.valueOf(7)), anchor.metadataRevision)
        assertEquals(BigInteger.ONE.shiftLeft(72).add(BigInteger.ONE), anchor.hardwareEpoch.generation)
        assertEquals(BigInteger.ONE.shiftLeft(90).add(BigInteger.valueOf(19)), anchor.logicalSequence)
        assertEquals(BigInteger.ONE.shiftLeft(91).add(BigInteger.valueOf(20)), anchor.journalRevision)
        assertEquals(BigInteger.ONE.shiftLeft(92).add(BigInteger.valueOf(21)), anchor.inboxRevision)
        assertContentEquals(ByteArray(32) { 81 }, anchor.stateCommitment())
        assertContentEquals(ByteArray(32) { 85 }, anchor.stateNonceCommitment())
        assertContentEquals(ByteArray(32) { 86 }, anchor.snapshotCommitment())
    }

    @Test fun `both source variants roundtrip and recovery preserves all checkpoint fields`() {
        listOf(false, true).forEach { recovered ->
            val value = challenge(recovered)
            val bytes = C.encodeAccountChallengeShape(value)
            val decoded = C.decodeAccountChallengeShapeExact(bytes)
            assertContentEquals(bytes, C.encodeAccountChallengeShape(decoded))
            val source = C.encodeAuthoritySourceShape(value.authoritySource)
            assertContentEquals(source, C.encodeAuthoritySourceShape(C.decodeAuthoritySourceShapeExact(source)))
            if (recovered) {
                val anchor = (decoded.authoritySource as KagemushaEnrolledOpenAuthoritySourceV1.RecoveryCheckpoint).statement
                assertEquals(KAGEMUSHA_UINT128_MAX, anchor.metadataRevision)
                assertEquals(KAGEMUSHA_UINT128_MAX, anchor.hardwareEpoch.generation)
                assertEquals(KAGEMUSHA_UINT128_MAX, anchor.logicalSequence)
                assertEquals(KAGEMUSHA_UINT128_MAX.subtract(BigInteger.ONE), anchor.journalRevision)
                assertEquals(KAGEMUSHA_UINT128_MAX.subtract(BigInteger.valueOf(2)), anchor.inboxRevision)
                assertEquals(1, anchor.version)
                assertContentEquals(ByteArray(32) { 6 }, anchor.stateCommitment())
                assertContentEquals(ByteArray(32) { 7 }, anchor.hardwareEpoch.epochId())
                assertContentEquals(ByteArray(32) { 8 }, anchor.devicePolicyBinding.deviceKeyReference())
                assertContentEquals(ByteArray(32) { 9 }, anchor.devicePolicyBinding.hardwarePolicyId())
                assertContentEquals(ByteArray(32) { 10 }, anchor.stateNonceCommitment())
                assertContentEquals(ByteArray(32) { 11 }, anchor.snapshotCommitment())
                assertContentEquals(value.owner.laneId(), anchor.lane.deviceLaneId())
            }
        }
    }

    @Test fun `signing excludes archive header and uses exactly one marked Blake2b hash`() {
        val value = challenge(true)
        val archive = C.encodeAccountChallengeShape(value)
        val bare = NoritoHeader.decode(archive, null).payload
        val signing = signMessage(value)
        assertContentEquals(IrohaHash.prehash(bare), signing)
        assertEquals(1, signing.last().toInt() and 1)
        assertFalse(signing.contentEquals(IrohaHash.prehash(archive)))
        assertFalse(signing.contentEquals(IrohaHash.prehash(signing)))
        assertFalse(signing.contentEquals(MessageDigest.getInstance("SHA-256").digest(bare)))
    }

    @Test fun `signing rejects every substituted correlation pin and selected owner`() {
        val value = challenge(false)
        val fields = arrayOf(value.nonce(), value.releaseId(), value.hardwarePolicyDigest(), value.coreAuthorizationKeyReference())
        for (index in fields.indices) {
            val changed = fields.map { it.copyOf() }.toTypedArray()
            changed[index][0] = (changed[index][0].toInt() xor 1).toByte()
            assertFailsWith<IllegalArgumentException> {
                C.accountSigningMessageShape(value, selector(), changed[0], changed[1], changed[2], changed[3])
            }
        }
        val runtime = value.owner.runtime
        val other = KagemushaEnrolledOpenSelectorV1.fromOwner(KagemushaRetailEnrollmentOwnerV1(
            value.owner.accountId, KagemushaRetailEnrollmentRuntimeV1("other-bank", runtime.ledgerDataspaceId,
                runtime.authenticationNamespace, runtime.networkId, runtime.asset, runtime.assetIncarnation, runtime.scale), value.owner.laneId()))
        assertFailsWith<IllegalArgumentException> {
            C.accountSigningMessageShape(value, other, fields[0], fields[1], fields[2], fields[3])
        }
    }

    @Test fun `every truncation alternate schema malformed length compression and trailing bytes fail`() {
        for (recovered in listOf(false, true)) {
            val original = C.encodeAccountChallengeShape(challenge(recovered))
            for (length in original.indices) {
                assertFailsWith<IllegalArgumentException>("truncation $length") {
                    C.decodeAccountChallengeShapeExact(original.copyOf(length))
                }
            }
            listOf(original + 0, ByteArray(C.MAXIMUM_ARCHIVE_BYTES + 1), "{\"version\":1}".toByteArray(),
                "/durable/wallet.db".toByteArray(), original.copyOf().also { it[6] = (it[6].toInt() xor 1).toByte() },
                original.copyOf().also { it[22] = 1 }, original.copyOf().also { it[39] = 0 },
                original.copyOf().also { for (index in 23..30) it[index] = 127 },
            ).forEach { invalid -> assertFailsWith<IllegalArgumentException> { C.decodeAccountChallengeShapeExact(invalid) } }
        }
    }

    @Test fun `valid checksums cannot conceal unsupported version domain lifetime or authority tag`() {
        val value = challenge(false)
        val payload = NoritoHeader.decode(C.encodeAccountChallengeShape(value), null).payload
        val domainOffset = payload.indexOf('i'.code.toByte())
        listOf(payload.copyOf().also { it[1] = 2 }, payload.copyOf().also { it[domainOffset] = 'x'.code.toByte() },
            payload.copyOf().also { it[it.lastIndex] = 1 },
        ).forEach { invalid -> assertFailsWith<IllegalArgumentException> {
            C.decodeAccountChallengeShapeExact(frame("iroha.kagemusha.v1.enrolled-open-account-challenge", invalid))
        } }
        val source = NoritoHeader.decode(C.encodeAuthoritySourceShape(value.authoritySource), null).payload
        source[0] = 2
        assertFailsWith<IllegalArgumentException> {
            C.decodeAuthoritySourceShapeExact(frame("iroha.kagemusha.v1.enrolled-open-authority-source", source))
        }
    }

    @Test fun `arrays remain defensive throughout recovery source and challenge`() {
        val value = challenge(true)
        val original = C.encodeAccountChallengeShape(value)
        val source = value.authoritySource as KagemushaEnrolledOpenAuthoritySourceV1.RecoveryCheckpoint
        value.enrollmentId().fill(0); value.nonce().fill(0); value.releaseId().fill(0)
        value.hardwarePolicyDigest().fill(0); value.coreAuthorizationKeyReference().fill(0)
        source.terminalCertificateDigest().fill(0); source.statement.stateCommitment().fill(0)
        source.statement.stateNonceCommitment().fill(0); source.statement.snapshotCommitment().fill(0)
        assertContentEquals(original, C.encodeAccountChallengeShape(value))
        val certificate = ByteArray(32) { 4 }
        val first = KagemushaEnrolledOpenAuthoritySourceV1.InitialCertificate(certificate)
        certificate.fill(0); first.certificateDigest().fill(0)
        assertContentEquals(ByteArray(32) { 4 }, first.certificateDigest())
    }

    @Test fun `recovery checkpoint cannot substitute a different owner lane`() {
        val value = challenge(true)
        val old = (value.authoritySource as KagemushaEnrolledOpenAuthoritySourceV1.RecoveryCheckpoint).statement
        val wrong = KagemushaDurabilityAnchorStatementV1(old.metadataRevision, old.version,
            KagemushaDeviceLaneIdV1(old.lane.networkId(), ByteArray(32) { 99 }, old.lane.assetCanonicalPayload(), old.lane.scale),
            old.stateCommitment(), old.hardwareEpoch, old.devicePolicyBinding, old.stateNonceCommitment(), old.logicalSequence,
            old.journalRevision, old.inboxRevision, old.snapshotCommitment())
        assertFailsWith<IllegalArgumentException> {
            buildChallenge(KagemushaEnrolledOpenAuthoritySourceV1.RecoveryCheckpoint(wrong, ByteArray(32) { 12 }))
        }
    }

    private fun challenge(recovered: Boolean): KagemushaEnrolledOpenAccountChallengeV1 {
        if (!recovered) return buildChallenge(KagemushaEnrolledOpenAuthoritySourceV1.InitialCertificate(ByteArray(32) { 4 }))
        val owner = selector().owner
        val statement = KagemushaDurabilityAnchorStatementV1(KAGEMUSHA_UINT128_MAX, 1,
            KagemushaDeviceLaneIdV1(owner.runtime.networkId.bytes(), owner.laneId(), owner.runtime.asset.canonicalPayload(), owner.runtime.scale),
            ByteArray(32) { 6 }, KagemushaDeviceHardwareEpochV1(KAGEMUSHA_UINT128_MAX, ByteArray(32) { 7 }),
            KagemushaDevicePolicyBindingV1(ByteArray(32) { 8 }, ByteArray(32) { 9 }), ByteArray(32) { 10 },
            KAGEMUSHA_UINT128_MAX, KAGEMUSHA_UINT128_MAX.subtract(BigInteger.ONE), KAGEMUSHA_UINT128_MAX.subtract(BigInteger.valueOf(2)),
            ByteArray(32) { 11 })
        return buildChallenge(KagemushaEnrolledOpenAuthoritySourceV1.RecoveryCheckpoint(statement, ByteArray(32) { 12 }))
    }
    private fun buildChallenge(source: KagemushaEnrolledOpenAuthoritySourceV1): KagemushaEnrolledOpenAccountChallengeV1 {
        val selector = selector()
        return KagemushaEnrolledOpenAccountChallengeV1(1, KagemushaEnrolledOpenAccountChallengeV1.ACCOUNT_DOMAIN,
            selector.enrollmentId(), selector.owner, ByteArray(32) { 1 }, source, ByteArray(32) { 2 },
            ByteArray(32) { 3 }, ByteArray(32) { 4 }, 120_000)
    }
    private fun signMessage(value: KagemushaEnrolledOpenAccountChallengeV1): ByteArray = C.accountSigningMessageShape(
        value, selector(), value.nonce(), value.releaseId(), value.hardwarePolicyDigest(), value.coreAuthorizationKeyReference())
    private fun fixture(field: String, name: String = "kagemusha_enrolled_open_challenge_v1.json"): ByteArray {
        val text = String(Files.readAllBytes(Paths.get("../../fixtures/offline/$name")), Charsets.UTF_8)
        val hex = Regex("\"$field\"\\s*:\\s*\"([^\"]+)\"").find(text)!!.groupValues[1]
        return hex.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
    }
    private fun selector(): KagemushaEnrolledOpenSelectorV1 {
        val text = String(Files.readAllBytes(Paths.get("../../fixtures/offline/kagemusha_enrolled_open_selector_v1.json")), Charsets.UTF_8)
        val hex = Regex("\"selector_canonical_hex\"\\s*:\\s*\"([^\"]+)\"").find(text)!!.groupValues[1]
        return KagemushaNoritoV1.decodeEnrolledOpenSelectorShapeExact(hex.chunked(2).map { it.toInt(16).toByte() }.toByteArray())
    }
    private fun frame(schema: String, bytes: ByteArray): ByteArray {
        val archive = NoritoCodec.encode(bytes, schema, object : TypeAdapter<ByteArray> {
            override fun encode(encoder: NoritoEncoder, value: ByteArray) = encoder.writeBytes(value)
            override fun decode(decoder: NoritoDecoder): ByteArray = decoder.readBytes(decoder.remaining())
        })
        return archive.copyOfRange(0, 40) + ByteArray(8) + archive.copyOfRange(40, archive.size)
    }
    companion object { private val C = KagemushaEnrolledOpenChallengeCodecV1 }
}
