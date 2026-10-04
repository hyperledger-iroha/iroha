// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.security.GeneralSecurityException
import java.security.MessageDigest
import java.security.Signature
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.bouncycastle.crypto.ec.CustomNamedCurves
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.norito.Varint
import org.junit.jupiter.api.Test

/**
 * Consumes the Rust-written `fixtures/kagemusha/wallet_v1_vectors.json` (design §8, C11):
 * recomputes every digest, checks every signature verdict with the raw low-S check before JCA,
 * validates every envelope header and per-kind bound against the Kotlin constants, and round-trips
 * the strict `kgm1:` text.
 */
class KagemushaWalletVectorsV1Test {
    @Test fun `bounds prefixes and Norito header facts match the fixture`() {
        val bounds = vectors.obj("bounds")
        assertEquals(KagemushaWalletWireV1.VERSION, bounds.int("version"))
        assertEquals(KagemushaWalletWireV1.TEXT_PREFIX, bounds.text("text_prefix"))
        assertEquals(KagemushaWalletWireV1.SESSION_MAX_BYTES, bounds.int("session_max_bytes"))
        assertEquals(KagemushaWalletWireV1.MESSAGE_MAX_BYTES, bounds.int("message_max_bytes"))
        assertEquals(KagemushaWalletWireV1.SESSION_TEXT_MAX_BYTES, bounds.int("session_text_max_bytes"))
        assertEquals(KagemushaWalletWireV1.MESSAGE_TEXT_MAX_BYTES, bounds.int("message_text_max_bytes"))
        assertEquals(KagemushaWalletWireV1.PROOF_MAX_BYTES, bounds.int("proof_max_bytes"))
        assertEquals(
            KagemushaWalletWireV1.CREDIT_STATUS_PROOF_MAX_BYTES,
            bounds.int("credit_status_proof_max_bytes"),
        )
        assertEquals(KagemushaWalletWireV1.CERTIFICATE_SET_MAX, bounds.int("certificate_set_max"))
        assertEquals(KagemushaWalletWireV1.DIGEST_PREFIX, vectors.text("domain_prefix"))
        assertContentEquals(
            KagemushaWalletWireV1.DIGEST_PREFIX.toByteArray(Charsets.US_ASCII),
            vectors.hex("domain_prefix_hex"),
        )

        val header = vectors.obj("norito_header")
        assertContentEquals(NoritoHeader.MAGIC, header.hex("magic_hex"))
        assertEquals(NoritoHeader.HEADER_LENGTH, header.int("header_bytes"))
        assertEquals(NoritoHeader.MAJOR_VERSION, header.int("major"))
        assertEquals(NoritoHeader.MINOR_VERSION, header.int("minor"))
        assertEquals(NoritoHeader.COMPRESSION_NONE, header.int("compression"))
        assertEquals(8, KagemushaWalletWireV1.ENVELOPE_PADDING_BYTES)
        assertEquals(NoritoHeader.COMPACT_LEN, KagemushaWalletWireV1.ENVELOPE_FLAGS)
    }

    @Test fun `text maxima are the exact kgm1 length of the frame bounds`() {
        assertEquals(5, KagemushaWalletWireV1.textBytesForFrame(0))
        assertEquals(7, KagemushaWalletWireV1.textBytesForFrame(1))
        assertEquals(8, KagemushaWalletWireV1.textBytesForFrame(2))
        assertEquals(9, KagemushaWalletWireV1.textBytesForFrame(3))
        assertEquals(
            KagemushaWalletWireV1.SESSION_TEXT_MAX_BYTES,
            KagemushaWalletWireV1.textBytesForFrame(KagemushaWalletWireV1.SESSION_MAX_BYTES),
        )
        assertEquals(
            KagemushaWalletWireV1.MESSAGE_TEXT_MAX_BYTES,
            KagemushaWalletWireV1.textBytesForFrame(KagemushaWalletWireV1.MESSAGE_MAX_BYTES),
        )
        for (length in listOf(1, 2, 3, 4, 47, 48, 2_048, 9_999, 10_000)) {
            assertEquals(
                KagemushaWalletWireV1.textBytesForFrame(length),
                KagemushaWalletWireV1.encodeText(ByteArray(length) { it.toByte() }).length,
            )
        }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletWireV1.textBytesForFrame(-1) }
        assertFailsWith<IllegalArgumentException> {
            KagemushaWalletWireV1.textBytesForFrame(KagemushaWalletWireV1.MESSAGE_MAX_BYTES + 1)
        }
    }

    @Test fun `message kinds and tags match the Rust enum tags`() {
        val tags = vectors.obj("enum_tags").array("KagemushaWalletMessageV1").map { it.jsonObject }
        assertEquals(KagemushaWalletMessageKindV1.entries.size, tags.size)
        for (entry in tags) {
            val kind = assertNotNull(KagemushaWalletMessageKindV1.fromLabel(entry.text("variant")))
            assertEquals(entry.int("tag"), kind.wireTag)
            assertEquals(kind, KagemushaWalletMessageKindV1.fromWireTag(entry.int("tag")))
        }
        assertNull(KagemushaWalletMessageKindV1.fromWireTag(0))
        assertNull(KagemushaWalletMessageKindV1.fromWireTag(7))
        assertNull(KagemushaWalletMessageKindV1.fromLabel("offer"))
        for (kind in KagemushaWalletMessageKindV1.entries) {
            assertEquals(KagemushaWalletWireV1.textBytesForFrame(kind.maximumFrameBytes), kind.maximumTextBytes)
        }
    }

    @Test fun `every digest vector recomputes and every role is covered`() {
        val seen = mutableSetOf<KagemushaWalletDigestRoleV1>()
        for (vector in vectors.array("digests").map { it.jsonObject }) {
            val role = assertNotNull(KagemushaWalletDigestRoleV1.fromLabel(vector.text("role")))
            seen += role
            val body = vector.hex("body_hex")
            val preimage = KagemushaWalletWireV1.preimage(role, body)
            assertContentEquals(vector.hex("preimage_hex"), preimage, vector.text("object"))
            assertContentEquals(vector.hex("digest_hex"), KagemushaWalletWireV1.digest(role, body), vector.text("object"))
            assertContentEquals(sha256(preimage), KagemushaWalletWireV1.digest(role, body))
            if (role == KagemushaWalletDigestRoleV1.PROOF) {
                assertTrue(vector.bool("stand_in_proof"))
                assertTrue(body.indices.all { index -> unsigned(body[index]) == index % 251 })
            }
        }
        assertEquals(KagemushaWalletDigestRoleV1.entries.toSet(), seen)
        assertEquals(
            KagemushaWalletDigestRoleV1.entries.size,
            KagemushaWalletDigestRoleV1.entries.map { it.label }.toSet().size,
        )
        assertNull(KagemushaWalletDigestRoleV1.fromLabel("policy-chunk"))
        assertNull(KagemushaWalletDigestRoleV1.fromLabel(""))
    }

    @Test fun `signed object digests recompute from the signature vectors`() {
        val digests = vectors.array("digests").map { it.jsonObject }
        var matched = 0
        for (vector in vectors.array("signatures").map { it.jsonObject }) {
            val bodyRole = vector.text("role")
            if (!bodyRole.endsWith("-body")) continue
            // Offer, session control and ledger control bodies have no separate object digest role.
            val role = KagemushaWalletDigestRoleV1.fromLabel(bodyRole.removeSuffix("-body")) ?: continue
            val expected = digests.firstOrNull {
                it.text("role") == role.label && it.text("object") == vector.text("object")
            } ?: continue
            val digest = KagemushaWalletWireV1.signedObjectDigest(
                role,
                vector.hex("e_hex"),
                vector.hex("signature_hex"),
            )
            assertContentEquals(expected.hex("digest_hex"), digest, vector.text("object"))
            matched += 1
        }
        assertTrue(matched >= 10, "only $matched signed-object digest vectors matched")

        val first = vectors.array("signatures").first().jsonObject
        assertFailsWith<IllegalArgumentException> {
            KagemushaWalletWireV1.signedObjectDigest(
                KagemushaWalletDigestRoleV1.CERTIFICATE,
                first.hex("e_hex").copyOf(31),
                first.hex("signature_hex"),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            KagemushaWalletWireV1.signedObjectDigest(
                KagemushaWalletDigestRoleV1.CERTIFICATE,
                first.hex("e_hex"),
                first.obj("high_s_twin").hex("signature_hex"),
            )
        }
    }

    @Test fun `every signature vector has its codec and verify verdicts`() {
        val signatures = vectors.array("signatures").map { it.jsonObject }
        assertTrue(signatures.isNotEmpty())
        for (vector in signatures) {
            val label = vector.text("object")
            val role = assertNotNull(KagemushaWalletDigestRoleV1.fromLabel(vector.text("role")))
            val preimage = vector.hex("preimage_hex")
            val bodyOffset = KagemushaWalletWireV1.preimage(role, ByteArray(0)).size
            val body = preimage.copyOfRange(bodyOffset, preimage.size)
            assertContentEquals(preimage, KagemushaWalletWireV1.preimage(role, body), label)
            assertContentEquals(vector.hex("e_hex"), KagemushaWalletWireV1.digest(role, body), label)

            val key = vector.hex("public_key_hex")
            val signature = vector.hex("signature_hex")
            assertTrue(vector.bool("codec_ok") && vector.bool("verify_ok"), label)
            assertVerdicts(label, key, preimage, signature, vector.bool("codec_ok"), vector.bool("verify_ok"))

            val twin = vector.obj("high_s_twin")
            val twinRaw = twin.hex("signature_hex")
            assertFalse(twin.bool("codec_ok"), label)
            assertTrue(twin.bool("verify_ok"), label)
            assertVerdicts("$label high-S twin", key, preimage, twinRaw, twin.bool("codec_ok"), twin.bool("verify_ok"))
            assertContentEquals(signature.copyOfRange(0, 32), twinRaw.copyOfRange(0, 32))
            assertEquals(
                P256_ORDER.subtract(BigInteger(1, signature.copyOfRange(32, 64))),
                BigInteger(1, twinRaw.copyOfRange(32, 64)),
            )
            val twinDer = twin.hex("der_hex")
            assertTrue(jcaVerifyDer(key, preimage, twinDer), "JCA accepts the high-S twin of $label")
            val frozen = KagemushaP256Codec.rawLowSFromStrictDer(twinDer)
            assertContentEquals(twin.hex("frozen_signature_hex"), frozen, label)
            assertContentEquals(signature, frozen, label)
            assertTrue(KagemushaP256Codec.verifyRawLowS(key, preimage, frozen), label)
        }
    }

    @Test fun `low-S boundary scalars have their verdicts`() {
        val boundaries = vectors.obj("signature_boundaries")
        val order = BigInteger(boundaries.text("order_hex"), 16)
        assertEquals(P256_ORDER, order)
        assertEquals(order.shiftRight(1), BigInteger(boundaries.text("half_order_hex"), 16))
        assertEquals(order.shiftRight(1).add(BigInteger.ONE), BigInteger(boundaries.text("half_order_plus_one_hex"), 16))

        val role = assertNotNull(KagemushaWalletDigestRoleV1.fromLabel(boundaries.text("role")))
        val body = boundaries.hex("body_hex")
        val preimage = KagemushaWalletWireV1.preimage(role, body)
        assertContentEquals(boundaries.hex("preimage_hex"), preimage)
        assertContentEquals(boundaries.hex("e_hex"), KagemushaWalletWireV1.digest(role, body))

        // The documented derivation: r = x(kG) mod n, s = floor(n/2), d = (s*k - e) * r^-1 mod n.
        val curve = CustomNamedCurves.getByName("secp256r1")
        val k = BigInteger(boundaries.text("k_hex"), 16)
        val r = curve.g.multiply(k).normalize().affineXCoord.toBigInteger().mod(order)
        assertEquals(BigInteger(boundaries.text("r_hex"), 16), r)
        val e = BigInteger(1, sha256(preimage)).mod(order)
        val d = order.shiftRight(1).multiply(k).subtract(e).multiply(r.modInverse(order)).mod(order)
        assertEquals(BigInteger(boundaries.text("d_hex"), 16), d)
        val key = boundaries.hex("public_key_hex")
        assertContentEquals(curve.g.multiply(d).normalize().getEncoded(false), key)

        val cases = boundaries.array("cases").map { it.jsonObject }
        assertEquals(
            setOf("s_half_order", "s_half_order_plus_one_high_s_twin", "r_zero", "s_zero", "r_order", "s_order"),
            cases.map { it.text("name") }.toSet(),
        )
        for (case in cases) {
            assertVerdicts(
                case.text("name"),
                key,
                preimage,
                case.hex("signature_hex"),
                case.bool("codec_ok"),
                case.bool("verify_ok"),
            )
        }
        val accepted = cases.single { it.text("name") == "s_half_order" }.hex("signature_hex")
        assertTrue(KagemushaP256Codec.verifyRawLowS(key, preimage, accepted))
    }

    @Test fun `fixture public keys convert to JCA keys and malformed keys are rejected`() {
        val curve = CustomNamedCurves.getByName("secp256r1")
        val keys = vectors.array("keys").map { it.jsonObject }
        assertTrue(keys.isNotEmpty())
        for (entry in keys) {
            val sec1 = entry.hex("public_key_hex")
            val scalar = BigInteger(entry.text("scalar_hex"), 16)
            assertContentEquals(curve.g.multiply(scalar).normalize().getEncoded(false), sec1, entry.text("name"))
            val key = KagemushaP256Codec.publicKeyFromSec1(sec1)
            assertEquals(BigInteger(1, sec1.copyOfRange(1, 33)), key.w.affineX)
            assertEquals(BigInteger(1, sec1.copyOfRange(33, 65)), key.w.affineY)
            assertEquals(P256_ORDER, key.params.order)
            assertEquals(256, key.params.curve.field.fieldSize)
        }
        val valid = keys.first().hex("public_key_hex")
        val offCurve = valid.copyOf().also { it[64] = (it[64].toInt() xor 1).toByte() }
        val compressedPrefix = valid.copyOf().also { it[0] = 0x02 }
        for (bad in listOf(offCurve, compressedPrefix, valid.copyOf(64), valid + byteArrayOf(0))) {
            assertFailsWith<IllegalArgumentException> { KagemushaP256Codec.publicKeyFromSec1(bad) }
        }
    }

    @Test fun `verifyRawLowS rejects wrong preimages keys and malformed inputs without mutation`() {
        val signatures = vectors.array("signatures").map { it.jsonObject }
        val vector = signatures.first()
        val key = vector.hex("public_key_hex")
        val preimage = vector.hex("preimage_hex")
        val signature = vector.hex("signature_hex")
        val keyCopy = key.copyOf()
        val preimageCopy = preimage.copyOf()
        val signatureCopy = signature.copyOf()
        assertTrue(KagemushaP256Codec.verifyRawLowS(key, preimage, signature))
        assertContentEquals(keyCopy, key)
        assertContentEquals(preimageCopy, preimage)
        assertContentEquals(signatureCopy, signature)

        val otherKey = signatures.map { it.hex("public_key_hex") }.first { !it.contentEquals(key) }
        assertFalse(KagemushaP256Codec.verifyRawLowS(otherKey, preimage, signature))
        assertFalse(KagemushaP256Codec.verifyRawLowS(key, preimage + byteArrayOf(0), signature))
        assertFalse(KagemushaP256Codec.verifyRawLowS(key, preimage, signature.copyOf(63)))
        assertFalse(KagemushaP256Codec.verifyRawLowS(key, preimage, signature + byteArrayOf(0)))
        val flipped = signature.copyOf().also { it[10] = (it[10].toInt() xor 1).toByte() }
        assertFalse(KagemushaP256Codec.verifyRawLowS(key, preimage, flipped))
        val offCurve = key.copyOf().also { it[64] = (it[64].toInt() xor 1).toByte() }
        assertFalse(KagemushaP256Codec.verifyRawLowS(offCurve, preimage, signature))
        assertFalse(KagemushaP256Codec.verifyRawLowS(key.copyOf(33), preimage, signature))
    }

    @Test fun `frame schema hashes recompute from frame names`() {
        val frames = vectors.array("frames").map { it.jsonObject }
        assertTrue(frames.isNotEmpty())
        for (frame in frames) {
            val name = frame.text("frame_name")
            assertTrue(name.endsWith("::" + frame.text("type")), name)
            assertContentEquals(frame.hex("frame_hash_hex"), SchemaHash.hash16(name), name)
        }
        val envelope = frames.single { it.text("type") == "KagemushaWalletEnvelopeV1" }
        assertEquals(KagemushaWalletWireV1.ENVELOPE_FRAME_NAME, envelope.text("frame_name"))
        assertEquals(KagemushaWalletWireV1.MESSAGE_MAX_BYTES, envelope.int("max_bytes"))
        assertContentEquals(KagemushaWalletWireV1.envelopeSchemaHash(), envelope.hex("frame_hash_hex"))
    }

    @Test fun `every envelope vector header and bound validate against the Kotlin constants`() {
        val envelopes = vectors.array("envelopes").map { it.jsonObject }
        val schemeId = vectors.array("digests").map { it.jsonObject }
            .single { it.text("role") == "scheme" }.hex("digest_hex")
        val kinds = mutableSetOf<KagemushaWalletMessageKindV1>()
        for (vector in envelopes) {
            val label = vector.text("variant")
            val frame = vector.hex("canonical_hex")
            assertEquals(KagemushaWalletWireV1.ENVELOPE_FRAME_NAME, vector.text("frame_name"), label)
            val schemaHash = vector.hex("schema_hash_hex")
            assertContentEquals(SchemaHash.hash16(vector.text("frame_name")), schemaHash, label)
            assertContentEquals(KagemushaWalletWireV1.envelopeSchemaHash(), schemaHash, label)
            assertContentEquals(schemeId, vector.hex("scheme_id_hex"), label)

            val envelope = KagemushaWalletWireV1.inspectEnvelope(frame)
            val expectedKind = assertNotNull(KagemushaWalletMessageKindV1.fromLabel(vector.text("kind")), label)
            kinds += expectedKind
            assertEquals(expectedKind, envelope.kind, label)
            assertEquals(vector.int("tag"), envelope.kind.wireTag, label)
            assertEquals(vector.int("bound"), envelope.kind.maximumFrameBytes, label)
            assertEquals(vector.int("text_bound"), envelope.kind.maximumTextBytes, label)
            assertEquals(vector.int("flags"), envelope.flags, label)
            assertEquals(vector.int("padding_len"), envelope.paddingLength, label)
            assertEquals(vector.int("payload_len"), envelope.payloadLength, label)
            assertEquals(vector.int("frame_len"), envelope.frameLength, label)
            assertEquals(frame.size, envelope.frameLength, label)
            assertEquals(java.lang.Long.parseUnsignedLong(vector.text("crc64_hex"), 16), envelope.crc64, label)
            assertEquals(CRC64.compute(frame.copyOfRange(48, frame.size)), envelope.crc64, label)
            assertContentEquals(schemaHash, envelope.schemaHash, label)
            assertContentEquals(frame, envelope.frame(), label)
            assertTrue(frame.size <= envelope.kind.maximumFrameBytes, label)

            val text = vector.text("text")
            assertTrue(text.length <= envelope.kind.maximumTextBytes, label)
            assertEquals(KagemushaWalletWireV1.textBytesForFrame(frame.size), text.length, label)
            assertEquals(text, KagemushaWalletWireV1.encodeText(frame), label)
            assertEquals(text, envelope.toText(), label)
            assertContentEquals(frame, KagemushaWalletWireV1.decodeText(text), label)
            val decoded = KagemushaWalletWireV1.decodeEnvelopeText(text)
            assertEquals(expectedKind, decoded.kind, label)
            assertContentEquals(frame, decoded.frame(), label)
        }
        assertEquals(KagemushaWalletMessageKindV1.entries.toSet(), kinds)
    }

    @Test fun `every kind accepts its bound and rejects bound plus one`() {
        for (kind in KagemushaWalletMessageKindV1.entries) {
            val atBound = envelopeFrame(kind.wireTag, kind.maximumFrameBytes)
            assertEquals(kind.maximumFrameBytes, atBound.size)
            val accepted = KagemushaWalletWireV1.inspectEnvelope(atBound)
            assertEquals(kind, accepted.kind)
            val text = accepted.toText()
            assertEquals(kind.maximumTextBytes, text.length)
            assertContentEquals(atBound, KagemushaWalletWireV1.decodeEnvelopeText(text).frame())

            val overBound = envelopeFrame(kind.wireTag, kind.maximumFrameBytes + 1)
            assertEquals(kind.maximumFrameBytes + 1, overBound.size)
            val frameError = assertFailsWith<IllegalArgumentException> {
                KagemushaWalletWireV1.inspectEnvelope(overBound)
            }
            assertTrue(frameError.message.orEmpty().contains("exceeds ${kind.maximumFrameBytes} bytes"), frameError.message)
            val overText = KagemushaWalletWireV1.TEXT_PREFIX + base64Url(overBound)
            assertEquals(kind.maximumTextBytes + 1, overText.length)
            assertFailsWith<IllegalArgumentException> { KagemushaWalletWireV1.decodeEnvelopeText(overText) }
        }
        // The session bound is enforced after the header parse, by the kind the payload tag selects.
        val sessionError = assertFailsWith<IllegalArgumentException> {
            KagemushaWalletWireV1.inspectEnvelope(envelopeFrame(1, KagemushaWalletWireV1.SESSION_MAX_BYTES + 1))
        }
        assertTrue(sessionError.message.orEmpty().contains("Offer"), sessionError.message)
        assertEquals(
            KagemushaWalletMessageKindV1.REQUEST,
            KagemushaWalletWireV1.inspectEnvelope(envelopeFrame(2, KagemushaWalletWireV1.SESSION_MAX_BYTES + 1)).kind,
        )
        assertFailsWith<IllegalArgumentException> {
            KagemushaWalletWireV1.inspectEnvelope(ByteArray(KagemushaWalletWireV1.MESSAGE_MAX_BYTES + 1))
        }
    }

    @Test fun `envelope header violations are rejected`() {
        val frame = vectors.array("envelopes").first().jsonObject.hex("canonical_hex")
        KagemushaWalletWireV1.inspectEnvelope(frame)
        val payload = frame.copyOfRange(48, frame.size)
        fun mutated(index: Int, value: Int) = frame.copyOf().also { it[index] = value.toByte() }
        fun reframedPayload(edit: (ByteArray) -> ByteArray) = frameOf(edit(payload.copyOf()))

        val rejected = listOf(
            "empty" to ByteArray(0),
            "header only" to frame.copyOf(48),
            "truncated payload" to frame.copyOf(frame.size - 1),
            "trailing byte" to frame + byteArrayOf(0),
            "magic" to mutated(0, 'X'.code),
            "major" to mutated(4, 1),
            "minor" to mutated(5, 1),
            "schema hash" to mutated(6, frame[6].toInt() xor 1),
            "compression" to mutated(22, 1),
            "payload length" to mutated(23, frame[23].toInt() + 1),
            "payload length high byte" to mutated(30, 1),
            "crc64" to mutated(31, frame[31].toInt() xor 1),
            "flags zero" to mutated(39, 0x00),
            "flags reserved" to mutated(39, 0x03),
            "flags high" to mutated(39, 0x82),
            "padding" to mutated(40, 1),
            "padding last" to mutated(47, 0x80),
            "payload byte" to mutated(frame.size - 1, frame[frame.size - 1].toInt() xor 1),
            "version field length" to reframedPayload { it.also { bytes -> bytes[0] = 3 } },
            "version" to reframedPayload { it.also { bytes -> bytes[1] = 2 } },
            "noncanonical varint" to reframedPayload { byteArrayOf(0x82.toByte(), 0x00) + it.copyOfRange(1, it.size) },
            "tag zero" to reframedPayload { it.also { bytes -> bytes[5] = 0 } },
            "tag seven" to reframedPayload { it.also { bytes -> bytes[5] = 7 } },
            "tag high byte" to reframedPayload { it.also { bytes -> bytes[8] = 1 } },
            "message longer than its field" to reframedPayload { it + byteArrayOf(0) },
            "message shorter than its field" to reframedPayload { it.copyOf(it.size - 1) },
            "variant span" to reframedPayload { bytes ->
                // Shorten the message field by one byte and its length prefix with it, so only
                // the variant length prefix disagrees with the remaining span.
                val shortened = bytes.copyOf(bytes.size - 1)
                val messageLength = Varint.decode(shortened, 3)
                check(messageLength.nextOffset == 5)
                val reencoded = Varint.encode(messageLength.value - 1)
                check(reencoded.size == 2)
                reencoded.copyInto(shortened, 3)
                shortened
            },
        )
        for ((name, bytes) in rejected) {
            assertFailsWith<IllegalArgumentException>(name) { KagemushaWalletWireV1.inspectEnvelope(bytes) }
        }
        // The rebuilt payload is the original vector, so reframing alone is accepted.
        assertContentEquals(frame, frameOf(payload))
    }

    @Test fun `kgm1 text decoding is strict`() {
        val text = vectors.array("envelopes").first().jsonObject.text("text")
        val body = text.removePrefix(KagemushaWalletWireV1.TEXT_PREFIX)
        val rejected = listOf(
            body,
            "KGM1:$body",
            "kgm2:$body",
            "kgm1:",
            "$text=",
            "$text==",
            text.replaceRange(10, 11, "+"),
            text.replaceRange(10, 11, "/"),
            text.replaceRange(10, 11, " "),
            "$text\n",
            " $text",
            text.replaceRange(10, 11, "é"),
            "${text}A",
            "kgm1:A",
            "kgm1:_x",
            "kgm1:AB",
            KagemushaWalletWireV1.TEXT_PREFIX + "A".repeat(13_335),
        )
        for (candidate in rejected) {
            assertFailsWith<IllegalArgumentException>(candidate.take(24)) {
                KagemushaWalletWireV1.decodeText(candidate)
            }
        }
        assertContentEquals(byteArrayOf(0xff.toByte()), KagemushaWalletWireV1.decodeText("kgm1:_w"))
        assertContentEquals(byteArrayOf(0), KagemushaWalletWireV1.decodeText("kgm1:AA"))
        val largest = KagemushaWalletWireV1.TEXT_PREFIX + "A".repeat(13_334)
        assertEquals(KagemushaWalletWireV1.MESSAGE_TEXT_MAX_BYTES, largest.length)
        assertContentEquals(
            ByteArray(KagemushaWalletWireV1.MESSAGE_MAX_BYTES),
            KagemushaWalletWireV1.decodeText(largest),
        )
        assertFailsWith<IllegalArgumentException> { KagemushaWalletWireV1.decodeEnvelopeText(largest) }
        assertFailsWith<IllegalArgumentException> { KagemushaWalletWireV1.encodeText(ByteArray(0)) }
        assertFailsWith<IllegalArgumentException> {
            KagemushaWalletWireV1.encodeText(ByteArray(KagemushaWalletWireV1.MESSAGE_MAX_BYTES + 1))
        }
        assertEquals(largest, KagemushaWalletWireV1.encodeText(ByteArray(KagemushaWalletWireV1.MESSAGE_MAX_BYTES)))
    }

    private fun assertVerdicts(
        label: String,
        key: ByteArray,
        preimage: ByteArray,
        raw: ByteArray,
        codecOk: Boolean,
        verifyOk: Boolean,
    ) {
        assertEquals(codecOk, codecAccepts(raw), "$label codec_ok")
        assertEquals(verifyOk, jcaVerifyDer(key, preimage, anyDer(raw)), "$label verify_ok")
        assertEquals(codecOk && verifyOk, KagemushaP256Codec.verifyRawLowS(key, preimage, raw), "$label accepted")
    }

    private companion object {
        val P256_ORDER: BigInteger =
            BigInteger("FFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551", 16)

        val vectors: JsonObject by lazy {
            Json.parseToJsonElement(
                String(Files.readAllBytes(fixturePath()), StandardCharsets.UTF_8),
            ).jsonObject
        }

        /** Walks up from the working directory to `fixtures/kagemusha/wallet_v1_vectors.json`. */
        fun fixturePath(): Path {
            var current: Path? = Paths.get("").toAbsolutePath().normalize()
            while (current != null) {
                val candidate = current.resolve("fixtures/kagemusha/wallet_v1_vectors.json")
                if (Files.isRegularFile(candidate)) return candidate
                current = current.parent
            }
            error("fixtures/kagemusha/wallet_v1_vectors.json was not found above the test working directory")
        }

        fun JsonObject.obj(key: String): JsonObject = getValue(key).jsonObject

        fun JsonObject.array(key: String): JsonArray = getValue(key).jsonArray

        fun JsonObject.text(key: String): String = getValue(key).jsonPrimitive.content

        fun JsonObject.int(key: String): Int = getValue(key).jsonPrimitive.int

        fun JsonObject.bool(key: String): Boolean = getValue(key).jsonPrimitive.boolean

        fun JsonObject.hex(key: String): ByteArray = hexBytes(text(key))

        fun hexBytes(text: String): ByteArray {
            require(text.length % 2 == 0) { "odd hex length" }
            return ByteArray(text.length / 2) { index ->
                ((Character.digit(text[2 * index], 16) shl 4) or Character.digit(text[2 * index + 1], 16)).toByte()
            }
        }

        fun unsigned(value: Byte): Int = value.toInt() and 0xff

        fun sha256(bytes: ByteArray): ByteArray = MessageDigest.getInstance("SHA-256").digest(bytes)

        fun base64Url(bytes: ByteArray): String =
            java.util.Base64.getUrlEncoder().withoutPadding().encodeToString(bytes)

        fun codecAccepts(raw: ByteArray): Boolean = try {
            KagemushaP256Codec.requireRawLowSSignature(raw)
            true
        } catch (rejected: IllegalArgumentException) {
            false
        }

        /** DER of arbitrary `r || s` scalars (including zero, `n` and high S) for raw JCA checks. */
        fun anyDer(raw: ByteArray): ByteArray {
            val r = BigInteger(1, raw.copyOfRange(0, 32)).toByteArray()
            val s = BigInteger(1, raw.copyOfRange(32, 64)).toByteArray()
            return byteArrayOf(0x30, (4 + r.size + s.size).toByte(), 0x02, r.size.toByte()) +
                r + byteArrayOf(0x02, s.size.toByte()) + s
        }

        /** Plain JCA ECDSA-P256-SHA256 over DER; it accepts high S, unlike the wallet verifier. */
        fun jcaVerifyDer(key: ByteArray, preimage: ByteArray, der: ByteArray): Boolean = try {
            Signature.getInstance("SHA256withECDSA").run {
                initVerify(KagemushaP256Codec.publicKeyFromSec1(key))
                update(preimage)
                verify(der)
            }
        } catch (rejected: GeneralSecurityException) {
            false
        }

        /** A canonical envelope frame around [payload] with the exact header and padding. */
        fun frameOf(payload: ByteArray): ByteArray =
            NoritoHeader(
                KagemushaWalletWireV1.envelopeSchemaHash(),
                payload.size,
                CRC64.compute(payload),
                NoritoHeader.COMPACT_LEN,
                NoritoHeader.COMPRESSION_NONE,
            ).encode() + ByteArray(KagemushaWalletWireV1.ENVELOPE_PADDING_BYTES) + payload

        /**
         * A structurally valid envelope of exactly [frameLength] bytes whose message tag is [tag]
         * and whose variant field is zero filler; only the header-level checks apply to it.
         */
        fun envelopeFrame(tag: Int, frameLength: Int): ByteArray {
            val overhead = NoritoHeader.HEADER_LENGTH + KagemushaWalletWireV1.ENVELOPE_PADDING_BYTES
            var variantLength = 1
            while (true) {
                val messageLength = 4 + Varint.encode(variantLength.toLong()).size + variantLength
                val total = overhead + 3 + Varint.encode(messageLength.toLong()).size + messageLength
                if (total == frameLength) break
                check(total < frameLength) { "no envelope of exactly $frameLength bytes" }
                variantLength += 1
            }
            val variant = Varint.encode(variantLength.toLong()) + ByteArray(variantLength)
            val message = byteArrayOf(tag.toByte(), 0, 0, 0) + variant
            return frameOf(byteArrayOf(2, 1, 0) + Varint.encode(message.size.toLong()) + message)
        }
    }
}
