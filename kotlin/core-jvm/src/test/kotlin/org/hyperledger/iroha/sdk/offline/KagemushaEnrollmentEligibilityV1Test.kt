// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

import java.io.File
import java.math.BigInteger
import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters
import org.bouncycastle.crypto.signers.Ed25519Signer
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.junit.jupiter.api.Test
import kotlin.test.*
import org.hyperledger.iroha.sdk.offline.KagemushaEnrollmentEligibilityV1 as E

class KagemushaEnrollmentEligibilityV1Test {
    private fun rows(): List<Map<*, *>> {
        val file = generateSequence(File(".").canonicalFile) { it.parentFile }.map {
            File(it, "fixtures/kagemusha/enrollment_eligibility_v1_vectors.json") }.first(File::isFile)
        val root = JsonParser.parse(file.readText()) as Map<*, *>
        assertEquals(1, (root["version"] as Number).toInt())
        return (root["cases"] as List<*>).map { it as Map<*, *> }.also { assertEquals(36, it.size) }
    }
    private fun hex(value: Any?): ByteArray = (value as String).chunked(2).map { it.toInt(16).toByte() }.toByteArray()
    private fun n(value: Long) = BigInteger.valueOf(value)
    private fun clock(vararg times: Long): E.Clock { var i = 0; return E.Clock { check(i < times.size); n(times[i++]) } }
    private fun lookup(approved: Boolean = true, frozen: Boolean = false, revision: Long = 6, observed: Long = 1100) =
        E.CurrentLookup { _, _ -> E.Current(approved, frozen, n(revision), n(observed)) }
    private fun failure(code: E.FailureCode, run: () -> Unit) { assertEquals(code, assertFailsWith<E.Failure>(block = run).code) }

    @Test fun all36RustFramesDigestsSignaturesAndExactResponses() {
        for (row in rows()) {
            val p = hex(row["policy_hex"]); val q = hex(row["request_hex"]); val r = hex(row["response_hex"])
            val policy = E.decodePolicy(p); val request = E.decodeRequest(policy, q)
            val response = E.decodeResponse(policy, request, n(1100), r)
            assertContentEquals(p, policy.originalBytes()); assertContentEquals(q, request.originalBytes()); assertContentEquals(r, response.originalBytes())
            assertContentEquals(hex(row["policy_digest_hex"]), policy.digest()); assertContentEquals(hex(row["request_digest_hex"]), request.digest())
            assertContentEquals(hex(row["signing_message_hex"]), response.signingMessage()); assertContentEquals(hex(row["signature_hex"]), response.signature())
            assertEquals((row["decision"] as String).replace('-', '_').uppercase(), response.decision.name)
            assertEquals((row["purpose"] as String).replace('-', '_').uppercase(), request.purpose.name)
            var reads = 0; var signs = 0; var times = 0
            val output = E.answer(policy, q, E.Clock { times++; n(1100) }, E.CurrentLookup { selected, actual ->
                reads++; assertContentEquals(p, selected.originalBytes()); assertContentEquals(q, actual.originalBytes())
                E.Current(response.decision == E.Decision.APPROVED_UNFROZEN, response.decision == E.Decision.FROZEN, n(6), n(1100))
            }, E.Signer { message -> signs++; assertContentEquals(hex(row["signing_message_hex"]), message); hex(row["signature_hex"]) })
            assertContentEquals(r, output); assertEquals(1, reads); assertEquals(1, signs); assertEquals(3, times)
        }
    }
    @Test fun ordinaryEd25519CustodySignsExactMessageWithoutTransactionPrehash() {
        val row = rows().first()
        val privateKey = Ed25519PrivateKeyParameters(ByteArray(32) { 42 }, 0)
        val policy = E.decodePolicy(mutate(hex(row["policy_hex"])) { this[7] = privateKey.generatePublicKey().encoded })
        val requestOriginal = mutate(hex(row["request_hex"])) { this[1] = policy.digest() }
        val request = E.decodeRequest(policy, requestOriginal)
        fun sign(message: ByteArray): ByteArray = Ed25519Signer().run {
            init(true, privateKey); update(message, 0, message.size); generateSignature()
        }
        val original = E.answer(policy, requestOriginal, clock(1100, 1100, 1100), lookup(), E.Signer(::sign))
        val response = E.decodeResponse(policy, request, n(1100), original)
        assertEquals(E.Decision.APPROVED_UNFROZEN, response.decision)
        assertContentEquals(request.digest(), response.requestDigest())
        failure(E.FailureCode.SIGNATURE) {
            E.answer(policy, requestOriginal, clock(1100, 1100, 1100), lookup(),
                E.Signer { sign(IrohaHash.prehash(it)) })
        }
    }
    @Test fun frozenPrecedesApprovalAndEveryRequestReadsAgain() {
        val row = rows().first { it["decision"] == "frozen" }; val policy = E.decodePolicy(hex(row["policy_hex"]))
        var reads = 0
        repeat(2) { assertContentEquals(hex(row["response_hex"]), E.answer(policy, hex(row["request_hex"]), clock(1100, 1100, 1100),
            E.CurrentLookup { _, _ -> reads++; E.Current(true, true, n(6), n(1100)) }, E.Signer { hex(row["signature_hex"]) })) }
        assertEquals(2, reads)
    }
    @Test fun failuresExpiryRollbackAndObservationsNeverEmitResponse() {
        val row = rows().first(); val policy = E.decodePolicy(hex(row["policy_hex"])); val request = hex(row["request_hex"])
        val signer = E.Signer { hex(row["signature_hex"]) }
        for (times in listOf(longArrayOf(999), longArrayOf(2000), longArrayOf(1100, 1099), longArrayOf(1100, 2000), longArrayOf(1100, 1100, 1099), longArrayOf(1100, 1100, 2000)))
            failure(E.FailureCode.CLOCK) { E.answer(policy, request, clock(*times), lookup(), signer) }
        failure(E.FailureCode.CLOCK) { E.answer(policy, request, E.Clock { error("clock unavailable") }, lookup(), signer) }
        for (value in listOf(lookup(revision = 0), lookup(observed = 1099), lookup(observed = 1101)))
            failure(E.FailureCode.INVALID_OBSERVATION) { E.answer(policy, request, clock(1100, 1100, 1100), value, signer) }
        failure(E.FailureCode.UNAVAILABLE) { E.answer(policy, request, clock(1100), E.CurrentLookup { _, _ -> error("unknown subject") }, signer) }
        failure(E.FailureCode.SIGNING) { E.answer(policy, request, clock(1100, 1100), lookup(), E.Signer { error("signer unavailable") }) }
        val foreign = rows().first { it["decision"] == "frozen" }
        failure(E.FailureCode.SIGNATURE) { E.answer(policy, request, clock(1100, 1100, 1100), lookup(), E.Signer { hex(foreign["signature_hex"]) }) }
    }
    @Test fun canonicalBoundsSchemasFlagsTruncationAndTrailingBytesReject() {
        val row = rows().first(); val policy = E.decodePolicy(hex(row["policy_hex"])); val request = E.decodeRequest(policy, hex(row["request_hex"]))
        for ((key, decode) in listOf<Pair<String, (ByteArray) -> Unit>>("policy_hex" to { E.decodePolicy(it) },
            "request_hex" to { E.decodeRequest(policy, it) }, "response_hex" to { E.decodeResponse(policy, request, n(1100), it) })) {
            val original = hex(row[key])
            for (bad in listOf(ByteArray(0), ByteArray(2049), original.copyOf(original.size - 1), original + byteArrayOf(0),
                original.copyOf().also { it[6] = (it[6].toInt() xor 1).toByte() }, original.copyOf().also { it[22] = 1 },
                original.copyOf().also { it[39] = 0 }, original.copyOf().also { it[it.lastIndex] = (it.last().toInt() xor 1).toByte() }))
                assertFailsWith<E.Failure> { decode(bad) }
        }
    }
    @Test fun foreignAuthorityRegulatedParliamentUnknownArmsAndWeakKeysReject() {
        val row = rows().first(); val original = hex(row["policy_hex"]); val policy = E.decodePolicy(original)
        for (index in listOf(1, 2, 3, 4, 7, 8)) assertFailsWith<E.Failure> { E.decodePolicy(mutate(original) { this[index] = ByteArray(this[index].size) }) }
        val parliament = hex(rows().first { it["authority"] == "parliament-non-regulated" }["policy_hex"])
        assertFailsWith<E.Failure> { E.decodePolicy(mutate(parliament) { this[5] = byteArrayOf(1, 0, 0, 0) }) }
        for (tag in listOf(0, 3, 255)) assertFailsWith<E.Failure> { E.decodePolicy(mutate(original) { this[6][0] = tag.toByte() }) }
        for (index in 1..6) assertFailsWith<E.Failure> { E.decodeRequest(policy, mutate(hex(row["request_hex"])) { this[index] = ByteArray(this[index].size) }) }
        val foreign = E.decodePolicy(hex(rows().first { it["authority"] == "bank-non-regulated" }["policy_hex"]))
        assertFailsWith<E.Failure> { E.decodeRequest(foreign, hex(row["request_hex"])) }
    }
    @Test fun strictSignaturePointScalarAndFreshRequestBindingReject() {
        val row = rows().first(); val policy = E.decodePolicy(hex(row["policy_hex"])); val request = E.decodeRequest(policy, hex(row["request_hex"]))
        val original = hex(row["response_hex"])
        for (signature in listOf(ByteArray(64), hex(row["signature_hex"]).also { it.fill(0, 0, 32); it[0] = 1 },
            hex(row["signature_hex"]).also { it.fill(0xff.toByte(), 32, 64) }, hex(row["signature_hex"]).also { it[40] = (it[40].toInt() xor 1).toByte() }))
            assertFailsWith<E.Failure> { E.decodeResponse(policy, request, n(1100), mutate(original) { this[1] = signature }) }
        for (now in listOf(1099L, 2000L, 2001L)) assertFailsWith<E.Failure> { E.decodeResponse(policy, request, n(now), original) }
        val other = E.decodeRequest(policy, mutate(hex(row["request_hex"])) { this[5][0] = 42 })
        assertFailsWith<E.Failure> { E.decodeResponse(policy, other, n(1100), original) }
    }
    @Test fun unsignedMaximumsAndDefensiveCopiesPreserveOriginals() {
        val row = rows().first(); val input = hex(row["policy_hex"]); val policy = E.decodePolicy(input); val before = policy.originalBytes()
        input.fill(0); policy.publicKey().fill(0); policy.networkId().fill(0); policy.authorityDigest().fill(0); policy.originalBytes().fill(0)
        assertContentEquals(before, policy.originalBytes())
        val huge = E.decodePolicy(mutate(before) { this[4] = ByteArray(8) { 0xff.toByte() }; this[8] = ByteArray(8) { 0xff.toByte() } })
        assertEquals(BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE), huge.maximumResponseMs)
        val original = hex(row["request_hex"]); val request = E.decodeRequest(policy, original)
        original.fill(0); request.nonce().fill(0); request.actorDigest().fill(0); request.operationDigest().fill(0)
        assertContentEquals(hex(row["request_hex"]), request.originalBytes())
    }
    private fun mutate(original: ByteArray, change: MutableList<ByteArray>.() -> Unit): ByteArray {
        val decoded = NoritoHeader.decode(original, null); val reader = NoritoDecoder(decoded.payload, NoritoHeader.COMPACT_LEN)
        val fields = mutableListOf<ByteArray>()
        while (reader.remaining() > 0) fields += reader.readBytes(reader.readLength(true).toInt())
        fields.change()
        val payload = NoritoEncoder(NoritoHeader.COMPACT_LEN).apply { fields.forEach { writeLength(it.size.toLong(), true); writeBytes(it) } }.toByteArray()
        return NoritoHeader(decoded.header.schemaHash, payload.size, CRC64.compute(payload), NoritoHeader.COMPACT_LEN, 0).encode() + payload
    }
}
