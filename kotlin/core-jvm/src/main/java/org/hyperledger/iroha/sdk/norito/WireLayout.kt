// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.norito

/**
 * A Norito payload layout (`norito.md`, "Dense layout (header minor 1)").
 *
 * A frame declares its layout in the header: the minor version and the flag byte. The owner of
 * a headerless payload names the layout explicitly. Nothing infers a layout from payload bytes.
 */
enum class WireLayout(
    /** Header minor version that declares this layout. */
    @JvmField val minor: Int,
    /** Header flag byte of a frame in this layout. */
    @JvmField val headerFlags: Int,
    /**
     * Layout code carried by [NoritoEncoder.flags] and [NoritoDecoder.flags].
     *
     * For the minor-0 layouts it equals [headerFlags]. The dense code also sets
     * [NoritoHeader.COMPACT_LEN], because dense per-value lengths are varints as well, and it
     * never appears in a header: [NoritoHeader.validateFlags] rejects it.
     */
    @JvmField val code: Int,
) {
    /** Minor `0` with fixed-width `u64` per-value lengths (header flags `0x00`). */
    FIXED(NoritoHeader.MINOR_VERSION, 0x00, 0x00),

    /** Minor `0` with varint per-value lengths (header flags `0x02`); the default layout. */
    COMPACT(NoritoHeader.MINOR_VERSION, NoritoHeader.COMPACT_LEN, NoritoHeader.COMPACT_LEN),

    /**
     * Minor `1`: varint lengths and counts, raw byte sequences and arrays, varint enum tags and
     * no header alignment padding.
     */
    DENSE(NoritoHeader.MINOR_VERSION_DENSE, 0x00, DENSE_LAYOUT_CODE),
    ;

    companion object {
        /**
         * The layout a header's minor version and flag byte declare.
         *
         * @throws IllegalArgumentException for an unknown minor version, for reserved flag bits,
         *   and for any flag with minor `1`
         */
        @JvmStatic
        fun fromHeader(minor: Int, flags: Int): WireLayout {
            val normalizedFlags = flags and 0xFF
            return when (minor and 0xFF) {
                NoritoHeader.MINOR_VERSION -> {
                    NoritoHeader.validateFlags(normalizedFlags)
                    if ((normalizedFlags and NoritoHeader.COMPACT_LEN) != 0) COMPACT else FIXED
                }
                NoritoHeader.MINOR_VERSION_DENSE -> {
                    require(normalizedFlags == 0) {
                        "The dense Norito layout (minor 1) has no layout flags: " +
                            "0x${"%02x".format(normalizedFlags)}"
                    }
                    DENSE
                }
                else -> throw IllegalArgumentException(
                    "Unsupported Norito minor version: 0x${"%02x".format(minor and 0xFF)}",
                )
            }
        }

        /**
         * The layout an encoder or decoder layout code selects.
         *
         * Only the exact dense code selects [DENSE]. Any other value is read as minor-0 flags,
         * so a header flag byte can never select the dense layout.
         */
        @JvmStatic
        fun fromCode(code: Int): WireLayout = when {
            (code and 0xFF) == DENSE_LAYOUT_CODE -> DENSE
            (code and NoritoHeader.COMPACT_LEN) != 0 -> COMPACT
            else -> FIXED
        }
    }
}

/** Context bit that selects the dense layout together with [NoritoHeader.COMPACT_LEN]. */
private const val DENSE_LAYOUT_CODE = 0x80 or 0x02
