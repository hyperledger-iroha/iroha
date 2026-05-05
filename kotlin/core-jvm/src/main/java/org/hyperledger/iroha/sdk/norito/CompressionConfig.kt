// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.norito

import java.util.Locale

private const val ZSTD_MIN_LEVEL = 1
private const val ZSTD_MAX_LEVEL = 22

class CompressionConfig private constructor(
    @JvmField val mode: Int,
    @JvmField val level: Int,
) {

    enum class CompressionProfile(private val buckets: Array<LevelBucket>) {
        FAST(
            arrayOf(
                LevelBucket(0, 64 * 1024, 1),
                LevelBucket(64 * 1024, 512 * 1024, 2),
                LevelBucket(512 * 1024, 4 * 1024 * 1024, 3),
                LevelBucket(4 * 1024 * 1024, Int.MAX_VALUE, 4),
            )
        ),
        BALANCED(
            arrayOf(
                LevelBucket(0, 64 * 1024, 3),
                LevelBucket(64 * 1024, 512 * 1024, 5),
                LevelBucket(512 * 1024, 4 * 1024 * 1024, 7),
                LevelBucket(4 * 1024 * 1024, Int.MAX_VALUE, 9),
            )
        ),
        COMPACT(
            arrayOf(
                LevelBucket(0, 64 * 1024, 7),
                LevelBucket(64 * 1024, 512 * 1024, 11),
                LevelBucket(512 * 1024, 4 * 1024 * 1024, 15),
                LevelBucket(4 * 1024 * 1024, Int.MAX_VALUE, 19),
            )
        );

        fun resolveLevel(payloadBytes: Int): Int {
            require(payloadBytes >= 0) { "payloadBytes must be non-negative" }
            for (bucket in buckets) {
                if (bucket.matches(payloadBytes)) {
                    return clampLevel(bucket.level)
                }
            }
            return clampLevel(buckets.last().level)
        }

        companion object {
            @JvmStatic
            fun fromString(value: String): CompressionProfile = try {
                valueOf(value.trim().uppercase(Locale.ROOT))
            } catch (ex: IllegalArgumentException) {
                throw IllegalArgumentException("Unknown compression profile: $value", ex)
            }
        }
    }

    private data class LevelBucket(
        val lowerInclusive: Int,
        val upperExclusive: Int,
        val level: Int,
    ) {
        fun matches(payloadBytes: Int): Boolean =
            payloadBytes in lowerInclusive until upperExclusive
    }

    companion object {
        @JvmStatic
        fun none(): CompressionConfig =
            CompressionConfig(NoritoHeader.COMPRESSION_NONE, 0)

        @JvmStatic
        fun zstd(level: Int): CompressionConfig =
            CompressionConfig(NoritoHeader.COMPRESSION_ZSTD, clampLevel(level))

        @JvmStatic
        fun zstdProfile(profile: String, payloadBytes: Int): CompressionConfig =
            zstdProfile(CompressionProfile.fromString(profile), payloadBytes)

        @JvmStatic
        fun zstdProfile(profile: CompressionProfile, payloadBytes: Int): CompressionConfig {
            val resolvedLevel = profile.resolveLevel(payloadBytes)
            return CompressionConfig(NoritoHeader.COMPRESSION_ZSTD, resolvedLevel)
        }

        private fun clampLevel(level: Int): Int {
            require(level in ZSTD_MIN_LEVEL..ZSTD_MAX_LEVEL) {
                "Zstandard level $level must be between $ZSTD_MIN_LEVEL and $ZSTD_MAX_LEVEL"
            }
            return level
        }
    }
}
