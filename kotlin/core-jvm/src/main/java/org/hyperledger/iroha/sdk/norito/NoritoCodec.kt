// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.norito

import java.nio.ByteBuffer
import java.util.ArrayDeque
import java.util.Deque

private const val DYNAMIC_FLAGS_MASK = 0

/** High-level helpers for Norito encoding/decoding. */
object NoritoCodec {
    @JvmField
    val DEFAULT_FLAGS: Int = NoritoHeader.COMPACT_LEN

    private val DECODE_FLAGS_STACK: ThreadLocal<Deque<Int>> =
        ThreadLocal.withInitial { ArrayDeque() }
    private val ACTIVE_DECODE_CONTEXTS: ThreadLocal<Deque<Int>> =
        ThreadLocal.withInitial { ArrayDeque() }

    private val DECODE_ROOT_PAYLOAD: ThreadLocal<ByteArray?> = ThreadLocal()

    private fun applyAdaptiveFlags(flags: Int, payloadLen: Int): Int = flags

    @JvmStatic
    fun <T> encode(value: T, schemaPath: String, adapter: TypeAdapter<T>): ByteArray =
        encode(value, schemaPath, adapter, DEFAULT_FLAGS, CompressionConfig.none())

    @JvmStatic
    fun <T> encode(
        value: T,
        schemaPath: String,
        adapter: TypeAdapter<T>,
        flags: Int,
    ): ByteArray = encode(value, schemaPath, adapter, flags, CompressionConfig.none())

    @JvmStatic
    fun <T> encode(
        value: T,
        schemaPath: String,
        adapter: TypeAdapter<T>,
        flags: Int,
        compression: CompressionConfig,
    ): ByteArray =
        // The header constructor validates `flags`, so only a minor-0 flag byte frames a payload.
        frame(value, schemaPath, adapter, flags, flags, NoritoHeader.MINOR_VERSION, compression)

    /**
     * Encodes a frame in an explicit layout (`norito.md`, "Dense layout (header minor 1)").
     *
     * [WireLayout.COMPACT] produces exactly the bytes of the default [encode].
     * [WireLayout.DENSE] produces a minor-1 frame with flags `0x00`; the adapter must write
     * counts, enum tags and byte arrays through the layout-aware helpers ([NoritoAdapters],
     * [NoritoEncoder.writeCount], [NoritoEncoder.writeEnumTag]).
     */
    @JvmStatic
    fun <T> encode(
        value: T,
        schemaPath: String,
        adapter: TypeAdapter<T>,
        layout: WireLayout,
    ): ByteArray = encode(value, schemaPath, adapter, layout, CompressionConfig.none())

    /** [encode] in an explicit layout with a compressed payload. */
    @JvmStatic
    fun <T> encode(
        value: T,
        schemaPath: String,
        adapter: TypeAdapter<T>,
        layout: WireLayout,
        compression: CompressionConfig,
    ): ByteArray =
        frame(value, schemaPath, adapter, layout.code, layout.headerFlags, layout.minor, compression)

    /**
     * Encodes the bare (headerless) payload of `value` in an explicit layout.
     *
     * The owner of a headerless payload names its layout; [WireLayout.COMPACT] produces
     * exactly the payload of the default [encodeAdaptive].
     */
    @JvmStatic
    fun <T> encodeBare(value: T, adapter: TypeAdapter<T>, layout: WireLayout): ByteArray {
        val encoder = NoritoEncoder(layout)
        adapter.encode(encoder, value)
        return encoder.toByteArray()
    }

    /**
     * Decodes a bare (headerless) payload in the layout its owner names. The whole payload
     * must be consumed; bytes of another layout are refused rather than reinterpreted.
     */
    @JvmStatic
    fun <T> decodeBare(payload: ByteArray, adapter: TypeAdapter<T>, layout: WireLayout): T =
        decodePayload(payload, adapter, layout.code)

    private fun <T> frame(
        value: T,
        schemaPath: String,
        adapter: TypeAdapter<T>,
        layoutCode: Int,
        headerFlags: Int,
        minor: Int,
        compression: CompressionConfig,
    ): ByteArray {
        val encoder = NoritoEncoder(layoutCode)
        adapter.encode(encoder, value)
        val payload = encoder.toByteArray()
        val payloadBytes: ByteArray
        val compressionTag: Int
        if (compression.mode == NoritoHeader.COMPRESSION_ZSTD) {
            payloadBytes = NoritoCompression.compressZstd(payload, compression.level)
            compressionTag = NoritoHeader.COMPRESSION_ZSTD
        } else if (compression.mode != NoritoHeader.COMPRESSION_NONE) {
            throw IllegalArgumentException("Unsupported compression tag: ${compression.mode}")
        } else {
            payloadBytes = payload
            compressionTag = NoritoHeader.COMPRESSION_NONE
        }
        val checksum = CRC64.compute(payload)
        val schemaHash = SchemaHash.hash16(schemaPath)
        val header = NoritoHeader(schemaHash, payload.size, checksum, headerFlags, compressionTag, minor)
        val headerBytes = header.encode()
        val out = ByteArray(headerBytes.size + payloadBytes.size)
        System.arraycopy(headerBytes, 0, out, 0, headerBytes.size)
        System.arraycopy(payloadBytes, 0, out, headerBytes.size, payloadBytes.size)
        return out
    }

    @JvmStatic
    fun <T> encodeAdaptive(value: T, adapter: TypeAdapter<T>): AdaptiveEncoding =
        encodeAdaptive(value, adapter, DEFAULT_FLAGS)

    @JvmStatic
    fun <T> encodeAdaptive(value: T, adapter: TypeAdapter<T>, flags: Int): AdaptiveEncoding {
        NoritoHeader.validateFlags(flags)
        var encoder = NoritoEncoder(flags)
        adapter.encode(encoder, value)
        var payload = encoder.toByteArray()
        val finalFlags = applyAdaptiveFlags(flags, payload.size)
        if (finalFlags != flags) {
            encoder = NoritoEncoder(finalFlags)
            adapter.encode(encoder, value)
            payload = encoder.toByteArray()
        }
        return AdaptiveEncoding(payload, finalFlags and 0xFF)
    }

    @JvmStatic
    fun <T> encodeWithHeaderFlags(value: T, adapter: TypeAdapter<T>): AdaptiveEncoding =
        encodeAdaptive(value, adapter, DEFAULT_FLAGS)

    @JvmStatic
    fun <T> decodeAdaptive(payload: ByteArray, adapter: TypeAdapter<T>): T {
        val stack = DECODE_FLAGS_STACK.get()
        val flags = if (stack.isEmpty()) {
            applyAdaptiveFlags(DEFAULT_FLAGS, payload.size)
        } else {
            stack.peekLast()
        }
        RootGuard(payload, flags).use { guard ->
            guard.keepAlive()
            val decoder = NoritoDecoder(payload, flags)
            val value = adapter.decode(decoder)
            require(decoder.remaining() == 0) { "Trailing bytes after Norito decode" }
            return value
        }
    }

    @JvmStatic
    fun resetDecodeState() {
        DECODE_FLAGS_STACK.get().clear()
        ACTIVE_DECODE_CONTEXTS.get().clear()
        DECODE_ROOT_PAYLOAD.remove()
    }

    @JvmStatic
    fun effectiveDecodeFlags(): Int? {
        val contexts = ACTIVE_DECODE_CONTEXTS.get()
        if (contexts.isNotEmpty()) {
            return contexts.peekLast()
        }
        val stack = DECODE_FLAGS_STACK.get()
        if (stack.isNotEmpty()) {
            return stack.peekLast()
        }
        return null
    }

    @JvmStatic
    fun fromBytesView(data: ByteArray, schemaPath: String?): ArchiveView {
        resetDecodeState()
        val expectedHash = schemaPath?.let { SchemaHash.hash16(it) }
        val result = NoritoHeader.decode(data, expectedHash)
        val header = result.header
        require(header.compression == NoritoHeader.COMPRESSION_NONE) {
            "Archive views do not support compressed Norito payloads (found tag ${header.compression})"
        }
        val payload = result.payload
        header.validateChecksum(payload)
        return ArchiveView(payload, header.flags)
    }

    /**
     * Decodes a frame in the current layout (minor `0`, with or without `COMPACT_LEN`).
     *
     * A dense frame (minor `1`) is refused here: a reader accepts it only by naming
     * [WireLayout.DENSE] in the overload below.
     */
    @JvmStatic
    fun <T> decode(data: ByteArray, adapter: TypeAdapter<T>, schemaPath: String?): T {
        val expectedHash = schemaPath?.let { SchemaHash.hash16(it) }
        val result = NoritoHeader.decode(data, expectedHash)
        val header = result.header
        return decodePayload(checkedPayload(header, result.payload), adapter, header.flags)
    }

    /**
     * Decodes a frame that must declare exactly `layout` in its header.
     *
     * The header's minor version and flag byte select the layout; nothing is inferred from the
     * payload. A frame of another layout is refused, so a dense frame is never read as the
     * current layout or the reverse. The adapter must read counts, enum tags and byte arrays
     * through the layout-aware helpers.
     */
    @JvmStatic
    fun <T> decode(
        data: ByteArray,
        adapter: TypeAdapter<T>,
        schemaPath: String?,
        layout: WireLayout,
    ): T {
        val expectedHash = schemaPath?.let { SchemaHash.hash16(it) }
        val result = NoritoHeader.decodeDeclared(data, expectedHash)
        val header = result.header
        require(header.layout == layout) {
            "Norito frame declares the ${header.layout} layout; expected $layout"
        }
        return decodePayload(checkedPayload(header, result.payload), adapter, layout.code)
    }

    /** The frame's checksummed logical payload, decompressed when the header says so. */
    private fun checkedPayload(header: NoritoHeader, payload: ByteArray): ByteArray {
        val decodedPayload = when (header.compression) {
            NoritoHeader.COMPRESSION_ZSTD ->
                NoritoCompression.decompressZstd(payload, header.payloadLength)
            NoritoHeader.COMPRESSION_NONE -> payload
            else -> throw IllegalArgumentException(
                "Unsupported compression tag: ${header.compression}"
            )
        }
        header.validateChecksum(decodedPayload)
        return decodedPayload
    }

    /** Decodes exactly `payload` under one layout code; trailing bytes are refused. */
    private fun <T> decodePayload(payload: ByteArray, adapter: TypeAdapter<T>, layoutCode: Int): T {
        RootGuard(payload, layoutCode).use { guard ->
            guard.keepAlive()
            val decoder = NoritoDecoder(payload, layoutCode)
            val value = adapter.decode(decoder)
            require(decoder.remaining() == 0) { "Trailing bytes after Norito decode" }
            return value
        }
    }

    @JvmStatic
    fun <T> decode(data: ByteBuffer, adapter: TypeAdapter<T>, schemaPath: String?): T {
        val expectedHash = schemaPath?.let { SchemaHash.hash16(it) }
        val result = NoritoHeader.decodeView(data, expectedHash)
        val header = result.header
        val payload = result.payload
        if (header.compression == NoritoHeader.COMPRESSION_ZSTD) {
            val compressed = ByteArray(payload.remaining())
            payload.get(compressed)
            val decodedPayload =
                NoritoCompression.decompressZstd(compressed, header.payloadLength)
            header.validateChecksum(decodedPayload)
            RootGuard(decodedPayload, header.flags).use { guard ->
                guard.keepAlive()
                val decoder = NoritoDecoder(decodedPayload, header.flags)
                val value = adapter.decode(decoder)
                require(decoder.remaining() == 0) { "Trailing bytes after Norito decode" }
                return value
            }
        }
        require(header.compression == NoritoHeader.COMPRESSION_NONE) {
            "Unsupported compression tag: ${header.compression}"
        }
        header.validateChecksum(payload)
        RootGuard(payload, header.flags).use { guard ->
            guard.keepAlive()
            val decoder = NoritoDecoder(payload, header.flags)
            val value = adapter.decode(decoder)
            require(decoder.remaining() == 0) { "Trailing bytes after Norito decode" }
            return value
        }
    }

    /**
     * Fallible decode helper mirroring Rust's `NoritoDeserialize::try_deserialize`.
     *
     * Returns `Result.Ok` when decoding succeeds or `Result.Err` carrying the runtime
     * exception describing the failure. This avoids having to rely on exception control-flow at the
     * call site.
     */
    @JvmStatic
    fun <T> tryDecode(
        data: ByteArray,
        adapter: TypeAdapter<T>,
        schemaPath: String?,
    ): Result<T, RuntimeException> = try {
        Result.Ok(decode(data, adapter, schemaPath))
    } catch (ex: RuntimeException) {
        Result.Err(ex)
    }

    @JvmStatic
    fun <T> tryDecode(
        data: ByteBuffer,
        adapter: TypeAdapter<T>,
        schemaPath: String?,
    ): Result<T, RuntimeException> = try {
        Result.Ok(decode(data, adapter, schemaPath))
    } catch (ex: RuntimeException) {
        Result.Err(ex)
    }

    @JvmStatic
    fun payloadRootBytes(): ByteArray? =
        DECODE_ROOT_PAYLOAD.get()?.copyOf()

    class DecodeFlagsGuard private constructor(flags: Int) : AutoCloseable {
        private var active = true

        init {
            NoritoHeader.validateFlags(flags)
            DECODE_FLAGS_STACK.get().addLast(flags and 0xFF)
        }

        override fun close() {
            if (active) {
                val stack = DECODE_FLAGS_STACK.get()
                if (stack.isNotEmpty()) stack.removeLast()
                active = false
            }
        }

        companion object {
            @JvmStatic
            fun enter(flags: Int): DecodeFlagsGuard =
                DecodeFlagsGuard(flags)
        }
    }

    class AdaptiveEncoding(payload: ByteArray, flags: Int) {
        private val _payload: ByteArray = payload.copyOf()

        @JvmField
        val flags: Int = flags and 0xFF

        fun payload(): ByteArray = _payload.copyOf()
    }

    class ArchiveView internal constructor(payload: ByteArray, flags: Int) {
        private val _payload: ByteArray = payload.copyOf()

        @JvmField
        val flags: Int = flags and 0xFF

        fun asBytes(): ByteArray = _payload.copyOf()

        fun <T> decode(adapter: TypeAdapter<T>): T {
            RootGuard(_payload, flags).use { guard ->
                guard.keepAlive()
                val decoder = NoritoDecoder(_payload, flags)
                val value = adapter.decode(decoder)
                require(decoder.remaining() == 0) { "Trailing bytes after Norito decode" }
                return value
            }
        }
    }

    private class RootGuard : java.io.Closeable {
        private val installed: Boolean
        private val contextPushed: Boolean

        constructor(payload: ByteArray, flags: Int? = null) {
            if (DECODE_ROOT_PAYLOAD.get() == null) {
                DECODE_ROOT_PAYLOAD.set(payload.copyOf())
                installed = true
            } else {
                installed = false
            }
            if (flags != null) {
                ACTIVE_DECODE_CONTEXTS.get()
                    .addLast(flags and 0xFF)
                contextPushed = true
            } else {
                contextPushed = false
            }
        }

        constructor(payload: ByteBuffer, flags: Int? = null) {
            if (DECODE_ROOT_PAYLOAD.get() == null) {
                val view = payload.slice()
                val bytes = ByteArray(view.remaining())
                view.get(bytes)
                DECODE_ROOT_PAYLOAD.set(bytes)
                installed = true
            } else {
                installed = false
            }
            if (flags != null) {
                ACTIVE_DECODE_CONTEXTS.get()
                    .addLast(flags and 0xFF)
                contextPushed = true
            } else {
                contextPushed = false
            }
        }

        fun keepAlive() {
            // Intentionally empty; referenced by callers to silence -Xlint:try.
        }

        override fun close() {
            if (contextPushed) {
                val contexts = ACTIVE_DECODE_CONTEXTS.get()
                if (contexts.isNotEmpty()) contexts.removeLast()
            }
            if (installed) {
                DECODE_ROOT_PAYLOAD.remove()
            }
        }
    }
}
