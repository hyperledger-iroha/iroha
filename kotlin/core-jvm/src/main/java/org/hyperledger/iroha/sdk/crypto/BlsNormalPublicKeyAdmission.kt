// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.crypto

import java.math.BigInteger
import java.util.Collections
import java.util.LinkedHashMap

/** Canonical non-infinity BLS-Normal public-key admission shared by active clients. */
object BlsNormalPublicKeyAdmission {
    private val BIG_TWO: BigInteger = BigInteger.valueOf(2)
    private val BLS12_381_BASE_FIELD = BigInteger(
        "1A0111EA397FE69A4B1BA7B6434BACD7" +
            "64774B84F38512BF6730D2A0F6B0F624" +
            "1EABFFFEB153FFFFB9FEFFFFFFFFAAAB",
        16,
    )
    private val BLS12_381_SCALAR_FIELD = BigInteger(
        "73EDA753299D7D483339D80809A1D805" +
            "53BDA402FFFE5BFEFFFFFFFF00000001",
        16,
    )
    private class BlsNormalPeerId(
        val literal: String,
        val compressedKey: ByteArray,
    ) {
        val orderingKey: ByteArray =
            byteArrayOf(2) + compressedKey
    }

    private data class JacobianPoint(
        val x: BigInteger,
        val y: BigInteger,
        val z: BigInteger,
    )

    private val BLS_NORMAL_PEER_CACHE: MutableMap<String, BlsNormalPeerId> =
        Collections.synchronizedMap(
            object : LinkedHashMap<String, BlsNormalPeerId>(512, 0.75f, true) {
                override fun removeEldestEntry(
                    eldest: MutableMap.MutableEntry<String, BlsNormalPeerId>?,
                ): Boolean = size > 512
            },
        )

    /** Return true only for a canonical, non-infinity BLS-Normal subgroup point. */
    @JvmStatic
    fun isCanonicalBlsNormalPeerId(value: String): Boolean =
        try {
            decodeBlsNormalPeerId(value, "BLS-Normal public key")
            true
        } catch (_: IllegalArgumentException) {
            false
        }

    private fun decodeBlsNormalPeerId(
        value: String,
        path: String,
    ): BlsNormalPeerId {
        require(value == value.trim()) { "$path must not contain surrounding whitespace" }
        val bare =
            if (value.startsWith("bls_normal:")) {
                value.removePrefix("bls_normal:")
            } else {
                value
            }
        require(BLS_NORMAL_PEER_ID.matches(bare)) {
            "$path must be a canonical BLS-Normal PeerId"
        }
        synchronized(BLS_NORMAL_PEER_CACHE) {
            BLS_NORMAL_PEER_CACHE[bare]?.let { return it }
        }

        val compressed = hexBytes(bare.substring(6))
        val first = compressed[0].toInt() and 0xff
        val compressedFlag = first and 0x80 != 0
        val infinityFlag = first and 0x40 != 0
        val signFlag = first and 0x20 != 0
        val xBytes = compressed.copyOf()
        xBytes[0] = (first and 0x1f).toByte()
        val x = BigInteger(1, xBytes)
        require(compressedFlag && !infinityFlag && x < BLS12_381_BASE_FIELD) {
            "$path contains an invalid BLS-Normal compressed point"
        }
        val rhs =
            x.modPow(BigInteger.valueOf(3), BLS12_381_BASE_FIELD)
                .add(BigInteger.valueOf(4))
                .mod(BLS12_381_BASE_FIELD)
        var y = rhs.modPow(
            BLS12_381_BASE_FIELD.add(BigInteger.ONE).shiftRight(2),
            BLS12_381_BASE_FIELD,
        )
        require(y.multiply(y).mod(BLS12_381_BASE_FIELD) == rhs) {
            "$path contains an invalid BLS-Normal compressed point"
        }
        if ((y.shiftLeft(1) > BLS12_381_BASE_FIELD) != signFlag) {
            y = BLS12_381_BASE_FIELD.subtract(y)
        }
        require(isInBlsNormalSubgroup(x, y)) {
            "$path contains a non-subgroup BLS-Normal public key"
        }
        val decoded = BlsNormalPeerId(bare, compressed)
        synchronized(BLS_NORMAL_PEER_CACHE) {
            return BLS_NORMAL_PEER_CACHE[bare] ?: decoded.also {
                BLS_NORMAL_PEER_CACHE[bare] = it
            }
        }
    }

    private fun isInBlsNormalSubgroup(x: BigInteger, y: BigInteger): Boolean {
        var result = JacobianPoint(BigInteger.ZERO, BigInteger.ONE, BigInteger.ZERO)
        for (bit in BLS12_381_SCALAR_FIELD.bitLength() - 1 downTo 0) {
            result = jacobianDouble(result)
            if (BLS12_381_SCALAR_FIELD.testBit(bit)) {
                result = jacobianAddAffine(result, x, y)
            }
        }
        return result.z == BigInteger.ZERO
    }

    private fun jacobianDouble(point: JacobianPoint): JacobianPoint {
        if (point.z == BigInteger.ZERO || point.y == BigInteger.ZERO) {
            return JacobianPoint(BigInteger.ZERO, BigInteger.ONE, BigInteger.ZERO)
        }
        val modulus = BLS12_381_BASE_FIELD
        val a = point.x.multiply(point.x).mod(modulus)
        val b = point.y.multiply(point.y).mod(modulus)
        val c = b.multiply(b).mod(modulus)
        val d = BIG_TWO.multiply(
            point.x.add(b).pow(2).subtract(a).subtract(c),
        ).mod(modulus)
        val e = BigInteger.valueOf(3).multiply(a).mod(modulus)
        val f = e.multiply(e).mod(modulus)
        val x3 = f.subtract(BIG_TWO.multiply(d)).mod(modulus)
        val y3 = e.multiply(d.subtract(x3))
            .subtract(BigInteger.valueOf(8).multiply(c))
            .mod(modulus)
        val z3 = BIG_TWO.multiply(point.y).multiply(point.z).mod(modulus)
        return JacobianPoint(x3, y3, z3)
    }

    private fun jacobianAddAffine(
        point: JacobianPoint,
        affineX: BigInteger,
        affineY: BigInteger,
    ): JacobianPoint {
        if (point.z == BigInteger.ZERO) {
            return JacobianPoint(affineX, affineY, BigInteger.ONE)
        }
        val modulus = BLS12_381_BASE_FIELD
        val z1Squared = point.z.multiply(point.z).mod(modulus)
        val u2 = affineX.multiply(z1Squared).mod(modulus)
        val s2 = affineY.multiply(z1Squared).multiply(point.z).mod(modulus)
        val h = u2.subtract(point.x).mod(modulus)
        if (h == BigInteger.ZERO) {
            return if (s2 == point.y) {
                jacobianDouble(point)
            } else {
                JacobianPoint(BigInteger.ZERO, BigInteger.ONE, BigInteger.ZERO)
            }
        }
        val hh = h.multiply(h).mod(modulus)
        val i = BigInteger.valueOf(4).multiply(hh).mod(modulus)
        val j = h.multiply(i).mod(modulus)
        val r = BIG_TWO.multiply(s2.subtract(point.y)).mod(modulus)
        val v = point.x.multiply(i).mod(modulus)
        val x3 = r.multiply(r)
            .subtract(j)
            .subtract(BIG_TWO.multiply(v))
            .mod(modulus)
        val y3 = r.multiply(v.subtract(x3))
            .subtract(BIG_TWO.multiply(point.y).multiply(j))
            .mod(modulus)
        val z3 = point.z.add(h).pow(2)
            .subtract(z1Squared)
            .subtract(hh)
            .mod(modulus)
        return JacobianPoint(x3, y3, z3)
    }

    private fun hexBytes(value: String): ByteArray {
        require(value.length % 2 == 0) { "hex value must have an even length" }
        return ByteArray(value.length / 2) { index ->
            value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
        }
    }

    private val BLS_NORMAL_PEER_ID = Regex("^ea0130[0-9A-F]{96}$")
}
