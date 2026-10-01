// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.file.Files
import java.nio.file.Paths
import java.security.KeyFactory
import java.security.MessageDigest
import java.security.Signature
import java.security.spec.X509EncodedKeySpec
import java.util.Base64
import org.hyperledger.iroha.sdk.crypto.keystore.requireAppPlatformSigningMessageV1
import org.hyperledger.iroha.sdk.crypto.keystore.KagemushaAndroidAppSignaturePurposeV1
import org.junit.jupiter.api.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/** Actual mounted Rust model goldens only: no issuer root, hardware or native owner admission. */
class KagemushaOrdinaryAppIdentityNativeVectorsV1Test {
    @Test fun `actual Android and Apple Rust C515 E371 aliases and integrity equations match shared projections`() {
        for ((name, nativeTag) in listOf("android" to 5, "apple" to 4)) {
            val c = vector(name + "_challenge_signing")
            val transport = vector(name + "_challenge_transport")
            val cBody = c.copyOfRange(c.size - 451, c.size)
            val id = cBody.copyOfRange(3, 35)
            assertEquals(if (nativeTag == 5) 1 else 2, cBody[2].toInt())
            assertEquals(515, transport.size)
            assertContentEquals(cBody, transport.copyOfRange(0, 451))
            assertContentEquals(vector(name + "_issuer_signature"), transport.copyOfRange(451, 515))
            val challenge = sha(c)
            assertContentEquals(vector(name + "_attestation_challenge"), challenge)
            val alias = vector(name + "_key_alias_utf8").toString(Charsets.UTF_8)
            val point = vector(name + "_attested_public_key_sec1")
            val keyId = vector(name + "_attested_key_id")
            assertContentEquals(sha(point), keyId)
            if (nativeTag == 5) assertEquals(alias, KagemushaOrdinaryAppKeyAliasV1.originalAlias(c))
            else assertEquals(alias, Base64.getEncoder().encodeToString(keyId))
            val fields = listOf(le64(7), transport, c, challenge, byteArrayOf(nativeTag.toByte()),
                if (nativeTag == 5) alias.toByteArray(Charsets.UTF_8) else byteArrayOf(),
                byteArrayOf(if (nativeTag == 5) 3 else 0), ByteArray(32) { 0x42 })
            val request = KagemushaCoreCoordinatorFrameV1.encodeRequest(KagemushaCoreCoordinatorMethodV1.PREPARED_ORDINARY_APP_IDENTITY,
                listOf(KagemushaCoreCoordinatorFrameV1.u32(13), le64(6), transport))
            KagemushaCoreCoordinatorFrameV1.encodeResponse(KagemushaCoreCoordinatorMethodV1.PREPARED_ORDINARY_APP_IDENTITY, request, fields)

            val e = vector(name + "_possession_signing")
            requireAppPlatformSigningMessageV1(e, KagemushaAndroidAppSignaturePurposeV1.IDENTITY_ENROLLMENT_POSSESSION)
            val eBody = e.copyOfRange(e.size - 371, e.size)
            val originalSelectors = listOf(challenge) + listOf(1, 2, 3, 4, 10, 6, 7, 5).map {
                cBody.copyOfRange(3 + it * 32, 35 + it * 32)
            } + listOf(keyId, vector(name + "_raw_platform_evidence_digest"))
            val exactE = "iroha:kagemusha:v1:app-enrollment-possession\u0000".toByteArray(Charsets.US_ASCII) +
                le64(371) + byteArrayOf(1, 0, 1) + originalSelectors.fold(byteArrayOf()) { result, field -> result + field } +
                cBody.copyOfRange(435, 451)
            assertContentEquals(exactE, e)
            assertContentEquals(cBody.copyOfRange(435, 451), eBody.copyOfRange(355, 371))
            assertContentEquals(vector(name + "_possession_sha256"), sha(e))
            val eProjection = listOf(le64(7), e, byteArrayOf(nativeTag.toByte()), alias.toByteArray(Charsets.UTF_8),
                challenge, point, keyId, c, byteArrayOf(), ByteArray(32) { 0x43 },
                if (nativeTag == 5) byteArrayOf() else KagemushaCoreCoordinatorFrameV1.u32(0),
                byteArrayOf(if (nativeTag == 5) 3 else 0), ByteArray(32) { 0x44 }, byteArrayOf())
            val eRequest = KagemushaCoreCoordinatorFrameV1.encodeRequest(KagemushaCoreCoordinatorMethodV1.PREPARED_APP_ENROLLMENT_POSSESSION,
                listOf(KagemushaCoreCoordinatorFrameV1.u32(1), challenge))
            KagemushaCoreCoordinatorFrameV1.encodeResponse(KagemushaCoreCoordinatorMethodV1.PREPARED_APP_ENROLLMENT_POSSESSION, eRequest, eProjection)
            val integrity = sha("iroha:kagemusha:v1:play-integrity-enrollment\u0000".toByteArray(Charsets.US_ASCII) + c + keyId)
            assertContentEquals(vector(name + "_play_integrity_request_hash"), integrity)
            assertEquals(vector(name + "_play_integrity_request_hash_base64url_utf8").toString(Charsets.UTF_8),
                Base64.getUrlEncoder().withoutPadding().encodeToString(integrity))

            val publicKey = KeyFactory.getInstance("Ed25519").generatePublic(X509EncodedKeySpec(
                hex("302a300506032b6570032100") + vector(name + "_issuer_public_key")))
            fun verifies(message: ByteArray) = Signature.getInstance("Ed25519").run {
                initVerify(publicKey); update(message); verify(vector(name + "_issuer_signature"))
            }
            assertTrue(verifies(c))
            assertFalse(verifies(c.copyOf().also { it[it.size - 451 + 35] = (it[it.size - 451 + 35].toInt() xor 1).toByte() }))
        }
    }

    private fun vector(name: String): ByteArray {
        val path = sequenceOf("fixtures", "../fixtures", "../../fixtures").map {
            Paths.get(it, "offline/kagemusha_app_platform_messages_v1.tsv")
        }.first { Files.isRegularFile(it) }
        return hex(Files.readAllLines(path).first { it.startsWith(name + '\t') }.substringAfter('\t'))
    }
    private fun hex(value: String) = value.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
    private fun sha(bytes: ByteArray) = MessageDigest.getInstance("SHA-256").digest(bytes)
    private fun le64(value: Long) = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(value).array()
}
