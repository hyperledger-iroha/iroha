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
    val DEFAULT_FLAGS: Int = NoritoHeader.MINOR_VERSION

    private val DECODE_FLAGS_STACK: ThreadLocal<Deque<Int>> =
        ThreadLocal.withInitial { ArrayDeque() }
    private val DECODE_FLAGS_HINT_STACK: ThreadLocal<Deque<Int>> =
        ThreadLocal.withInitial { ArrayDeque() }
    private val ACTIVE_DECODE_CONTEXTS: ThreadLocal<Deque<ContextFlags>> =
        ThreadLocal.withInitial { ArrayDeque() }

    private val LAST_ENCODE_FLAGS: ThreadLocal<Int?> = ThreadLocal()
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
    ): ByteArray {
        val encoder = NoritoEncoder(flags)
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
        val header = NoritoHeader(schemaHash, payload.size, checksum, flags, compressionTag)
        val headerBytes = header.encode()
        val out = ByteArray(headerBytes.size + payloadBytes.size)
        System.arraycopy(headerBytes, 0, out, 0, headerBytes.size)
        System.arraycopy(payloadBytes, 0, out, headerBytes.size, payloadBytes.size)
        LAST_ENCODE_FLAGS.set(flags and 0xFF)
        return out
    }

    @JvmStatic
    fun <T> encodeAdaptive(value: T, adapter: TypeAdapter<T>): AdaptiveEncoding =
        encodeAdaptive(value, adapter, DEFAULT_FLAGS)

    @JvmStatic
    fun <T> encodeAdaptive(value: T, adapter: TypeAdapter<T>, flags: Int): AdaptiveEncoding {
        var encoder = NoritoEncoder(flags)
        adapter.encode(encoder, value)
        var payload = encoder.toByteArray()
        val finalFlags = applyAdaptiveFlags(flags, payload.size)
        if (finalFlags != flags) {
            encoder = NoritoEncoder(finalFlags)
            adapter.encode(encoder, value)
            payload = encoder.toByteArray()
        }
        LAST_ENCODE_FLAGS.set(finalFlags and 0xFF)
        return AdaptiveEncoding(payload, finalFlags and 0xFF)
    }

    @JvmStatic
    fun <T> encodeWithHeaderFlags(value: T, adapter: TypeAdapter<T>): AdaptiveEncoding =
        encodeAdaptive(value, adapter, DEFAULT_FLAGS)

    @JvmStatic
    fun takeLastEncodeFlags(): Int? {
        val current = LAST_ENCODE_FLAGS.get()
        LAST_ENCODE_FLAGS.remove()
        return current
    }

    private fun combineFlags(flags: Int, hint: Int): Int = flags and 0xFF

    @JvmStatic
    fun <T> decodeAdaptive(payload: ByteArray, adapter: TypeAdapter<T>): T {
        val stack = DECODE_FLAGS_STACK.get()
        val flags = if (stack.isEmpty()) {
            applyAdaptiveFlags(DEFAULT_FLAGS, payload.size)
        } else {
            stack.peekLast()
        }
        RootGuard(payload, flags, NoritoHeader.MINOR_VERSION).use { guard ->
            guard.keepAlive()
            val decoder = NoritoDecoder(payload, flags, NoritoHeader.MINOR_VERSION)
            val value = adapter.decode(decoder)
            require(decoder.remaining() == 0) { "Trailing bytes after Norito decode" }
            return value
        }
    }

    @JvmStatic
    fun resetDecodeState() {
        DECODE_FLAGS_STACK.get().clear()
        DECODE_FLAGS_HINT_STACK.get().clear()
        ACTIVE_DECODE_CONTEXTS.get().clear()
        LAST_ENCODE_FLAGS.remove()
        DECODE_ROOT_PAYLOAD.remove()
    }

    @JvmStatic
    fun effectiveDecodeFlags(): Int? {
        val contexts = ACTIVE_DECODE_CONTEXTS.get()
        if (contexts.isNotEmpty()) {
            val ctx = contexts.peekLast()
            return combineFlags(ctx.flags, ctx.hint)
        }
        val stack = DECODE_FLAGS_STACK.get()
        if (stack.isNotEmpty()) {
            val hints = DECODE_FLAGS_HINT_STACK.get()
            val hint = if (hints.isEmpty()) NoritoHeader.MINOR_VERSION else hints.peekLast()
            return combineFlags(stack.peekLast(), hint)
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
        return ArchiveView(payload, header.flags, header.minor)
    }

    @JvmStatic
    fun <T> decode(data: ByteArray, adapter: TypeAdapter<T>, schemaPath: String?): T {
        val expectedHash = schemaPath?.let { SchemaHash.hash16(it) }
        val result = NoritoHeader.decode(data, expectedHash)
        val header = result.header
        val payload = result.payload
        val decodedPayload = when (header.compression) {
            NoritoHeader.COMPRESSION_ZSTD ->
                NoritoCompression.decompressZstd(payload, header.payloadLength)
            NoritoHeader.COMPRESSION_NONE -> payload
            else -> throw IllegalArgumentException(
                "Unsupported compression tag: ${header.compression}"
            )
        }
        header.validateChecksum(decodedPayload)
        RootGuard(decodedPayload, header.flags, header.minor).use { guard ->
            guard.keepAlive()
            val decoder = NoritoDecoder(decodedPayload, header.flags, header.minor)
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
            RootGuard(decodedPayload, header.flags, header.minor).use { guard ->
                guard.keepAlive()
                val decoder = NoritoDecoder(decodedPayload, header.flags, header.minor)
                val value = adapter.decode(decoder)
                require(decoder.remaining() == 0) { "Trailing bytes after Norito decode" }
                return value
            }
        }
        require(header.compression == NoritoHeader.COMPRESSION_NONE) {
            "Unsupported compression tag: ${header.compression}"
        }
        header.validateChecksum(payload)
        RootGuard(payload, header.flags, header.minor).use { guard ->
            guard.keepAlive()
            val decoder = NoritoDecoder(payload, header.flags, header.minor)
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

    class DecodeFlagsGuard private constructor(flags: Int, hint: Int) : AutoCloseable {
        private var active = true

        init {
            DECODE_FLAGS_STACK.get().addLast(flags and 0xFF)
            DECODE_FLAGS_HINT_STACK.get().addLast(hint and 0xFF)
        }

        override fun close() {
            if (active) {
                val stack = DECODE_FLAGS_STACK.get()
                if (stack.isNotEmpty()) stack.removeLast()
                val hints = DECODE_FLAGS_HINT_STACK.get()
                if (hints.isNotEmpty()) hints.removeLast()
                active = false
            }
        }

        companion object {
            @JvmStatic
            fun enter(flags: Int): DecodeFlagsGuard =
                DecodeFlagsGuard(flags, NoritoHeader.MINOR_VERSION)

            @JvmStatic
            fun enterWithHint(flags: Int, hint: Int): DecodeFlagsGuard =
                DecodeFlagsGuard(flags, hint)
        }
    }

    class AdaptiveEncoding(payload: ByteArray, flags: Int) {
        private val _payload: ByteArray = payload.copyOf()

        @JvmField
        val flags: Int = flags and 0xFF

        fun payload(): ByteArray = _payload.copyOf()
    }

    class ArchiveView internal constructor(payload: ByteArray, flags: Int, flagsHint: Int) {
        private val _payload: ByteArray = payload.copyOf()

        @JvmField
        val flags: Int = flags and 0xFF

        @JvmField
        val flagsHint: Int = flagsHint and 0xFF

        fun asBytes(): ByteArray = _payload.copyOf()

        fun <T> decode(adapter: TypeAdapter<T>): T {
            RootGuard(_payload, flags, flagsHint).use { guard ->
                guard.keepAlive()
                val decoder = NoritoDecoder(_payload, flags, flagsHint)
                val value = adapter.decode(decoder)
                require(decoder.remaining() == 0) { "Trailing bytes after Norito decode" }
                return value
            }
        }
    }

    private data class ContextFlags(val flags: Int, val hint: Int)

    private class RootGuard : java.io.Closeable {
        private val installed: Boolean
        private val contextPushed: Boolean

        constructor(payload: ByteArray, flags: Int? = null, hint: Int? = null) {
            if (DECODE_ROOT_PAYLOAD.get() == null) {
                DECODE_ROOT_PAYLOAD.set(payload.copyOf())
                installed = true
            } else {
                installed = false
            }
            if (flags != null) {
                val normalizedHint = hint?.let { it and 0xFF } ?: NoritoHeader.MINOR_VERSION
                ACTIVE_DECODE_CONTEXTS.get()
                    .addLast(ContextFlags(flags and 0xFF, normalizedHint))
                contextPushed = true
            } else {
                contextPushed = false
            }
        }

        constructor(payload: ByteBuffer, flags: Int? = null, hint: Int? = null) {
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
                val normalizedHint = hint?.let { it and 0xFF } ?: NoritoHeader.MINOR_VERSION
                ACTIVE_DECODE_CONTEXTS.get()
                    .addLast(ContextFlags(flags and 0xFF, normalizedHint))
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
