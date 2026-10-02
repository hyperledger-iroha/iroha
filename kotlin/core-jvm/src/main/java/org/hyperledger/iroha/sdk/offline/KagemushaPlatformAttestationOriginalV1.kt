// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.MessageDigest
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash

/**
 * Data-only port of Rust's sole original platform-attestation archive.
 *
 * Ordered DERs and enrollment CBOR remain untouched. Successful construction or decoding grants
 * no PKIX, App Attest, Play Integrity, key possession, issuer, native or monetary authority.
 * Only the shared verifier under independently installed policy may admit these originals.
 */
class KagemushaPlatformAttestationOriginalV1 private constructor(
    chain: List<ByteArray>?,
    apple: ByteArray?,
) {
    private val chainValue = chain?.map { it.copyOf() }
    private val appleValue = apple?.copyOf()

    /** Original leaf-to-root order, defensively copied; null for Apple enrollment evidence. */
    fun androidCertificateChainDer(): List<ByteArray>? = chainValue?.map { it.copyOf() }

    /** Exact original enrollment object, defensively copied; null for Android evidence. */
    fun appleAttestationObjectCbor(): ByteArray? = appleValue?.copyOf()

    /** Complete canonical archive including its sole schema, version and platform role. */
    fun canonicalBytes(): ByteArray {
        // Recheck the retained snapshot, never a caller collection that may have changed.
        if (chainValue != null) {
            require(chainValue.size in 2..MAXIMUM_ANDROID_CERTIFICATES)
            require(chainValue.all { it.isNotEmpty() && it.size <= MAXIMUM_ORIGINAL_BYTES })
        } else require(appleValue != null && appleValue.isNotEmpty() && appleValue.size <= MAXIMUM_ORIGINAL_BYTES)
        requireArchiveLength(chainValue?.map { it.size }, appleValue?.size)
        val original = NoritoEncoder(NoritoHeader.COMPACT_LEN)
        if (chainValue != null) {
            original.writeUInt(chainValue.size.toLong(), 64)
            for (certificate in chainValue) {
                val vector = NoritoEncoder(NoritoHeader.COMPACT_LEN)
                vector.writeUInt(certificate.size.toLong(), 64)
                vector.writeBytes(certificate)
                field(original, vector.toByteArray())
            }
        } else {
            val bytes = checkNotNull(appleValue)
            original.writeUInt(bytes.size.toLong(), 64)
            original.writeBytes(bytes)
        }
        val evidence = NoritoEncoder(NoritoHeader.COMPACT_LEN)
        evidence.writeUInt(if (chainValue != null) 0 else 1, 32)
        field(evidence, original.toByteArray())
        val payload = NoritoEncoder(NoritoHeader.COMPACT_LEN)
        val version = NoritoEncoder(NoritoHeader.COMPACT_LEN)
        version.writeUInt(1, 16)
        field(payload, version.toByteArray())
        field(payload, evidence.toByteArray())
        val bytes = payload.toByteArray()
        val header = NoritoHeader(SchemaHash.hash16(SCHEMA), bytes.size, CRC64.compute(bytes),
            NoritoHeader.COMPACT_LEN, NoritoHeader.COMPRESSION_NONE)
        // The Rust archive alignment divides the canonical 40-byte header; no padding is present.
        return (header.encode() + bytes).also {
            require(it.size <= MAXIMUM_CANONICAL_BYTES) { "Platform original archive exceeds resource bound" }
        }
    }

    /** SHA-256 of the complete archive; a data selector, never a verification result. */
    fun canonicalDigest(): ByteArray = MessageDigest.getInstance("SHA-256").digest(canonicalBytes())

    companion object {
        const val MAXIMUM_CANONICAL_BYTES = 128 * 1024
        const val MAXIMUM_ANDROID_CERTIFICATES = 8
        const val MAXIMUM_ORIGINAL_BYTES = 16 * 1024
        private const val SCHEMA = "iroha_data_model::kagemusha::KagemushaPlatformAttestationOriginalV1"

        /** Preserve bounded Android originals; certificate semantics remain verifier duties. */
        @JvmStatic
        fun android(certificateChainDer: List<ByteArray>): KagemushaPlatformAttestationOriginalV1 {
            val count = certificateChainDer.size
            require(count in 2..MAXIMUM_ANDROID_CERTIFICATES)
            // Capture at most eight array references. Array lengths cannot change; the private
            // constructor then copies only this bounded local snapshot after exact preflight.
            val snapshot = ArrayList<ByteArray>(count)
            repeat(count) {
                val original = certificateChainDer[it]
                require(original.isNotEmpty() && original.size <= MAXIMUM_ORIGINAL_BYTES)
                snapshot.add(original)
            }
            require(certificateChainDer.size == count) { "Platform originals changed during capture" }
            requireArchiveLength(snapshot.map { it.size }, null)
            return KagemushaPlatformAttestationOriginalV1(snapshot, null).also { it.canonicalBytes() }
        }

        /** Preserve bounded Apple enrollment CBOR; approval assertions have a different role. */
        @JvmStatic
        fun apple(attestationObjectCbor: ByteArray): KagemushaPlatformAttestationOriginalV1 {
            require(attestationObjectCbor.isNotEmpty() && attestationObjectCbor.size <= MAXIMUM_ORIGINAL_BYTES)
            return KagemushaPlatformAttestationOriginalV1(null, attestationObjectCbor)
        }

        /** Decode only exact bounded canonical originals, with no legacy concatenation fallback. */
        @JvmStatic
        fun decodeCanonicalExact(original: ByteArray): KagemushaPlatformAttestationOriginalV1 {
            require(original.size in NoritoHeader.HEADER_LENGTH..MAXIMUM_CANONICAL_BYTES)
            val canonical = original.copyOf()
            require((canonical[22].toInt() and 255) == NoritoHeader.COMPRESSION_NONE)
            require((canonical[39].toInt() and 255) == NoritoHeader.COMPACT_LEN)
            val length = ByteBuffer.wrap(canonical).order(ByteOrder.LITTLE_ENDIAN).getLong(23)
            require(length == (canonical.size - NoritoHeader.HEADER_LENGTH).toLong())
            val frame = NoritoHeader.decode(canonical, SchemaHash.hash16(SCHEMA))
            frame.header.validateChecksum(frame.payload)
            val reader = NoritoDecoder(frame.payload, NoritoHeader.COMPACT_LEN)
            val version = readField(reader, 2)
            require(version.remaining() == 2 && version.readUInt(16) == 1L)
            val evidence = readField(reader, MAXIMUM_CANONICAL_BYTES)
            val role = evidence.readUInt(32)
            require(role == 0L || role == 1L)
            val material = readField(evidence, MAXIMUM_CANONICAL_BYTES)
            val value = if (role == 0L) {
                val count = material.readUInt(64)
                require(count in 2L..MAXIMUM_ANDROID_CERTIFICATES.toLong())
                val certificates = ArrayList<ByteArray>(count.toInt())
                repeat(count.toInt()) { certificates.add(readVector(readField(material, MAXIMUM_ORIGINAL_BYTES + 8))) }
                android(certificates)
            } else apple(readVector(material))
            require(reader.remaining() == 0 && evidence.remaining() == 0 && material.remaining() == 0)
            require(value.canonicalBytes().contentEquals(canonical)) { "Platform original archive is not canonical" }
            return value
        }

        private fun field(encoder: NoritoEncoder, bytes: ByteArray) {
            encoder.writeLength(bytes.size.toLong(), true)
            encoder.writeBytes(bytes)
        }

        // Shape bounds make all additions finite; count the exact frame before any byte copy.
        private fun requireArchiveLength(chainSizes: List<Int>?, appleSize: Int?) {
            val material = if (chainSizes != null) 8 + chainSizes.sumOf {
                val vector = 8 + it
                lengthPrefixBytes(vector) + vector
            } else 8 + checkNotNull(appleSize)
            val evidence = 4 + lengthPrefixBytes(material) + material
            val archive = NoritoHeader.HEADER_LENGTH + 3 + lengthPrefixBytes(evidence) + evidence
            require(archive <= MAXIMUM_CANONICAL_BYTES) { "Platform original archive exceeds resource bound" }
        }

        private fun lengthPrefixBytes(length: Int): Int {
            var n = length; var count = 1
            while (n >= 128) { count++; n = n ushr 7 }
            return count
        }

        private fun readField(reader: NoritoDecoder, maximum: Int): NoritoDecoder {
            val count = reader.readLength(true)
            require(count <= maximum.toLong() && count <= reader.remaining().toLong())
            return NoritoDecoder(reader.readBytes(count.toInt()), NoritoHeader.COMPACT_LEN)
        }

        private fun readVector(reader: NoritoDecoder): ByteArray {
            val count = reader.readUInt(64)
            require(count in 1L..MAXIMUM_ORIGINAL_BYTES.toLong() && count == reader.remaining().toLong())
            return reader.readBytes(count.toInt())
        }
    }
}
