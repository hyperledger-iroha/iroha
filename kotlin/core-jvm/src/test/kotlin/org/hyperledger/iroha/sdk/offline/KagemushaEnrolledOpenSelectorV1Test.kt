// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.nio.file.Files
import java.nio.file.Paths
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import org.junit.jupiter.api.Test
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.MultisigMemberPayload
import org.hyperledger.iroha.sdk.address.MultisigPolicyPayload
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

class KagemushaEnrolledOpenSelectorV1Test {
    @Test fun `Rust canonical selector roundtrips exactly and commits to the full owner`() {
        val bytes = fixture("selector_canonical_hex")
        val value = KagemushaNoritoV1.decodeEnrolledOpenSelectorShapeExact(bytes)
        assertContentEquals(bytes, KagemushaNoritoV1.encodeEnrolledOpenSelectorShape(value))
        assertContentEquals(fixture("enrollment_id_hex"), value.enrollmentId())
        assertContentEquals(value.enrollmentId(), KagemushaNoritoV1.retailEnrollmentIdentityShape(value.owner))
        assertEquals("mibank", value.owner.runtime.fiId)
        assertEquals(10L, value.owner.runtime.ledgerDataspaceId)
        assertEquals("mibank.bpng", value.owner.runtime.authenticationNamespace)
        assertEquals(2, value.owner.runtime.scale)
        assertContentEquals(ByteArray(32) { 32 }, value.owner.laneId())
    }

    @Test fun `every immutable scope substitution invalidates the existing identity`() {
        val value = selector()
        val owner = value.owner
        val runtime = owner.runtime
        for (field in 0..8) {
            val changedRuntime = KagemushaRetailEnrollmentRuntimeV1(
                if (field == 1) "other-bank" else runtime.fiId,
                if (field == 2) 11L else runtime.ledgerDataspaceId,
                if (field == 3) "other.bpng" else runtime.authenticationNamespace,
                if (field == 4) NetworkId.fromBytes(ByteArray(32) { 3 }) else runtime.networkId,
                if (field == 5) KagemushaAssetDefinitionIdV1.fromCanonicalPayload(
                    runtime.asset.canonicalPayload().also { it[1] = (it[1].toInt() xor 1).toByte() }) else runtime.asset,
                if (field == 6) KagemushaAssetIncarnationV1(ByteArray(32) { 5 }) else runtime.assetIncarnation,
                if (field == 7) 3 else runtime.scale,
            )
            val changedOwner = KagemushaRetailEnrollmentOwnerV1(
                if (field == 0) differentAccount() else owner.accountId, changedRuntime,
                if (field == 8) ByteArray(32) { 33 } else owner.laneId())
            val changed = KagemushaEnrolledOpenSelectorV1(1, changedOwner, value.enrollmentId())
            assertFailsWith<IllegalArgumentException>("scope field $field") {
                KagemushaNoritoV1.encodeEnrolledOpenSelectorShape(changed)
            }
            assertFalse(value.enrollmentId().contentEquals(
                KagemushaEnrolledOpenSelectorV1.fromOwner(changedOwner).enrollmentId()))
        }
    }

    @Test fun `canonical decode rejects every truncation paths JSON tails oversized and other schema`() {
        val bytes = KagemushaNoritoV1.encodeEnrolledOpenSelectorShape(selector())
        for (length in bytes.indices) {
            assertFailsWith<IllegalArgumentException>("truncation $length") {
                KagemushaNoritoV1.decodeEnrolledOpenSelectorShapeExact(bytes.copyOf(length))
            }
        }
        listOf("/durable/wallet.db".toByteArray(), "{\"version\":1}".toByteArray(), bytes + 0,
            ByteArray(KagemushaNoritoV1.MAXIMUM_ENROLLED_OPEN_SELECTOR_BYTES + 1),
            bytes.copyOf().also { it[6] = (it[6].toInt() xor 1).toByte() },
        ).forEach { invalid -> assertFailsWith<IllegalArgumentException> {
            KagemushaNoritoV1.decodeEnrolledOpenSelectorShapeExact(invalid)
        } }
    }

    @Test fun `valid checksums cannot conceal invalid version identity or compression flags`() {
        val bytes = KagemushaNoritoV1.encodeEnrolledOpenSelectorShape(selector())
        val payload = NoritoHeader.decode(bytes, null).payload
        listOf(payload.copyOf().also { it[1] = 2 }, payload.copyOf().also { it[it.lastIndex] =
            (it.last().toInt() xor 1).toByte() }).forEach { changed ->
            assertFailsWith<IllegalArgumentException> {
                KagemushaNoritoV1.decodeEnrolledOpenSelectorShapeExact(frame(changed))
            }
        }
        listOf(bytes.copyOf().also { it[22] = 1 }, bytes.copyOf().also { it[39] = 0 },
            bytes.copyOf().also { for (index in 23..30) it[index] = 127 },
        ).forEach { invalid -> assertFailsWith<IllegalArgumentException> {
            KagemushaNoritoV1.decodeEnrolledOpenSelectorShapeExact(invalid)
        } }
    }

    @Test fun `projection byte arrays are copied on construction and access`() {
        val original = selector()
        val lane = original.owner.laneId()
        val id = original.enrollmentId()
        val copy = KagemushaEnrolledOpenSelectorV1(1,
            KagemushaRetailEnrollmentOwnerV1(original.owner.accountId, original.owner.runtime, lane), id)
        lane.fill(0); id.fill(0)
        copy.enrollmentId().fill(0); copy.owner.laneId().fill(0)
        assertContentEquals(KagemushaNoritoV1.encodeEnrolledOpenSelectorShape(original),
            KagemushaNoritoV1.encodeEnrolledOpenSelectorShape(copy))
    }

    @Test fun `runtime rejects invalid canonical names lane and scale while preserving unsigned dataspace`() {
        val value = selector().owner
        val runtime = value.runtime
        listOf("", "x".repeat(256), "bad name", "bad@name", "bad#name", "bad\$name",
            "bad\u0000name", "bad\u0085name", "bad\u202ename", "e\u0301", "bad\ud800",
        ).forEach { bad -> assertFailsWith<IllegalArgumentException> {
            KagemushaRetailEnrollmentRuntimeV1(bad, 10L, runtime.authenticationNamespace,
                runtime.networkId, runtime.asset, runtime.assetIncarnation, 2)
        } }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRetailEnrollmentOwnerV1(value.accountId, runtime, ByteArray(32))
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaRetailEnrollmentRuntimeV1(runtime.fiId, 10L, runtime.authenticationNamespace,
                runtime.networkId, runtime.asset, runtime.assetIncarnation, 39)
        }
        val high = KagemushaEnrolledOpenSelectorV1.fromOwner(KagemushaRetailEnrollmentOwnerV1(
            value.accountId, KagemushaRetailEnrollmentRuntimeV1(runtime.fiId, -1L,
                runtime.authenticationNamespace, runtime.networkId, runtime.asset, runtime.assetIncarnation, 2), value.laneId()))
        assertEquals(-1L, KagemushaNoritoV1.decodeEnrolledOpenSelectorShapeExact(
            KagemushaNoritoV1.encodeEnrolledOpenSelectorShape(high)).owner.runtime.ledgerDataspaceId)
    }

    @Test fun `selector rejects single non Ed25519 and multisig accounts`() {
        val value = selector().owner
        val unsupported = listOf(
            AccountAddress.fromAccount(hex("0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"), "secp256k1"),
            AccountAddress.fromMultisigPolicy(MultisigPolicyPayload.of(1, 1, listOf(
                MultisigMemberPayload(1, 1, hex("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"))))),
        )
        unsupported.forEach { address ->
            val account = KagemushaAccountIdV1.parse(address.toI105(0))
            assertFailsWith<IllegalArgumentException> {
                KagemushaEnrolledOpenSelectorV1.fromOwner(
                    KagemushaRetailEnrollmentOwnerV1(account, value.runtime, value.laneId()))
            }
        }
    }

    @Test fun `asset projection rejects non UUIDv4 version and variant`() {
        val payload = selector().owner.runtime.asset.canonicalPayload()
        listOf(payload.copyOf().also { it[13] = 0x30 }, payload.copyOf().also { it[17] = 0x40 },
            ByteArray(32) { if (it % 2 == 0) 1 else 0 },
        ).forEach { invalid -> assertFailsWith<IllegalArgumentException> {
            KagemushaAssetDefinitionIdV1.fromCanonicalPayload(invalid)
        } }
    }

    private fun selector(): KagemushaEnrolledOpenSelectorV1 {
        val asset = byteArrayOf(0x2f, 0x17, 0xc7.toByte(), 0x24, 0x66, 0xf8.toByte(), 0x4a, 0x4b,
            0xb8.toByte(), 0xa8.toByte(), 0xe2.toByte(), 0x48, 0x84.toByte(), 0xfd.toByte(), 0xcd.toByte(), 0x2f)
        val account = KagemushaAccountIdV1.parse(AccountAddress.fromAccount(hex(
            "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"), "ed25519").toI105(0))
        return KagemushaEnrolledOpenSelectorV1.fromOwner(KagemushaRetailEnrollmentOwnerV1(account,
            KagemushaRetailEnrollmentRuntimeV1("mibank", 10L, "mibank.bpng",
                NetworkId.fromBytes(ByteArray(32) { 1 }), KagemushaAssetDefinitionIdV1.fromCanonicalPayload(
                    asset.flatMap { listOf(1.toByte(), it) }.toByteArray()),
                KagemushaAssetIncarnationV1(ByteArray(32) { 1 }), 2), ByteArray(32) { 32 }))
    }

    private fun differentAccount(): KagemushaAccountIdV1 = KagemushaAccountIdV1.parse(
        AccountAddress.fromAccount(hex("3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c"),
            "ed25519").toI105(0))

    private fun frame(payload: ByteArray): ByteArray {
        val raw = NoritoCodec.encode(payload, "iroha.kagemusha.v1.enrolled-open-selector",
            object : TypeAdapter<ByteArray> {
                override fun encode(encoder: NoritoEncoder, value: ByteArray) = encoder.writeBytes(value)
                override fun decode(decoder: NoritoDecoder): ByteArray = decoder.readBytes(decoder.remaining())
            })
        val padding = (8 - NoritoHeader.HEADER_LENGTH % 8) % 8
        return raw.copyOfRange(0, NoritoHeader.HEADER_LENGTH) + ByteArray(padding) +
            raw.copyOfRange(NoritoHeader.HEADER_LENGTH, raw.size)
    }

    private fun fixture(field: String): ByteArray {
        val path = Paths.get("../../fixtures/offline/kagemusha_enrolled_open_selector_v1.json")
        val text = String(Files.readAllBytes(path), Charsets.UTF_8)
        return hex(Regex("\"$field\"\\s*:\\s*\"([^\"]+)\"").find(text)!!.groupValues[1])
    }
    private fun hex(text: String): ByteArray = text.chunked(2).map { it.toInt(16).toByte() }.toByteArray()
}
