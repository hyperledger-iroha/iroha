package org.hyperledger.iroha.sdk.tx.norito

import org.hyperledger.iroha.sdk.norito.WireLayout

/**
 * The native bridge's typed instruction transcoder (`connect_norito_transcode_instruction_v1`).
 *
 * A current-layout instruction envelope carries the instruction's Norito frame; a dense envelope
 * (`norito.md`, "Dense layout") carries its bare dense payload with no nested header. The bridge
 * decodes the payload as the instruction registered under the wire ID, requires it to be exactly
 * that instruction's canonical payload in the source layout and re-encodes it in the target
 * layout. The layout is always the caller's explicit selection.
 */
internal class NativeInstructionLayoutBridge private constructor() {
    companion object {
        private const val LIBRARY_NAME = "connect_norito_bridge"
        private const val REQUIRED_ABI_VERSION = 33
        private const val LAYOUT_CURRENT = 0
        private const val LAYOUT_DENSE = 1

        private val nativeAvailable: Boolean = try {
            System.loadLibrary(LIBRARY_NAME)
            nativeBridgeAbiVersion() == REQUIRED_ABI_VERSION
        } catch (_: UnsatisfiedLinkError) {
            false
        } catch (_: SecurityException) {
            false
        }

        @JvmStatic
        fun isNativeAvailable(): Boolean = nativeAvailable

        /**
         * The bare dense payload of the instruction framed as [frame] under [wireName], or `null`
         * when the bridge registers no instruction under [wireName].
         *
         * @throws IllegalArgumentException when [frame] is not the instruction's canonical frame.
         * @throws IllegalStateException when the native bridge is unavailable.
         */
        @JvmStatic
        fun denseFromFrame(wireName: String, frame: ByteArray): ByteArray? =
            transcode(wireName, frame, WireLayout.COMPACT, WireLayout.DENSE)

        /**
         * The canonical Norito frame of the instruction whose bare dense payload is [payload], or
         * `null` when the bridge registers no instruction under [wireName].
         *
         * @throws IllegalArgumentException when [payload] is not the instruction's canonical bare
         *   dense payload.
         * @throws IllegalStateException when the native bridge is unavailable.
         */
        @JvmStatic
        fun frameFromDense(wireName: String, payload: ByteArray): ByteArray? =
            transcode(wireName, payload, WireLayout.DENSE, WireLayout.COMPACT)

        private fun transcode(
            wireName: String,
            payload: ByteArray,
            from: WireLayout,
            to: WireLayout,
        ): ByteArray? {
            check(nativeAvailable) {
                "$LIBRARY_NAME instruction layout transcoder is unavailable; " +
                    "dense instruction envelopes need the native bridge"
            }
            return nativeTranscodeInstruction(
                wireName.toByteArray(Charsets.UTF_8),
                payload.copyOf(),
                selector(from),
                selector(to),
            )
        }

        private fun selector(layout: WireLayout): Int = when (layout) {
            WireLayout.COMPACT -> LAYOUT_CURRENT
            WireLayout.DENSE -> LAYOUT_DENSE
            WireLayout.FIXED -> throw IllegalArgumentException(
                "Instruction envelopes are framed in the compact or dense layout",
            )
        }

        @JvmStatic private external fun nativeBridgeAbiVersion(): Int
        @JvmStatic private external fun nativeTranscodeInstruction(
            wireId: ByteArray,
            payload: ByteArray,
            fromLayout: Int,
            toLayout: Int,
        ): ByteArray?
    }
}
