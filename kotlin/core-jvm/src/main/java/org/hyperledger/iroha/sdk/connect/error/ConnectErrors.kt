package org.hyperledger.iroha.sdk.connect.error

import java.io.EOFException
import java.io.UncheckedIOException
import java.lang.reflect.UndeclaredThrowableException
import java.net.ConnectException
import java.net.NoRouteToHostException
import java.net.PortUnreachableException
import java.net.SocketException
import java.net.SocketTimeoutException
import java.net.UnknownHostException
import java.nio.channels.ClosedChannelException
import java.nio.channels.UnresolvedAddressException
import java.security.cert.CertificateException
import java.text.ParseException
import java.util.Locale
import java.util.concurrent.CompletionException
import java.util.concurrent.ExecutionException
import java.util.concurrent.TimeoutException
import javax.net.ssl.SSLException

/** Helper functions for translating [Throwable] instances into [ConnectError]. */
object ConnectErrors {

    @JvmStatic
    fun from(throwable: Throwable?): ConnectError = from(throwable, null)

    @JvmStatic
    fun from(throwable: Throwable?, options: ConnectErrorOptions?): ConnectError {
        val base = classifyOrFallback(throwable)
        if (options == null) return base
        val overrideFatal = options.fatal != null
        val overrideStatus = options.httpStatus != null
        if (!overrideFatal && !overrideStatus) return base
        return base.toBuilder()
            .fatal(if (overrideFatal) options.fatal!! else base.fatal)
            .httpStatus(if (overrideStatus) options.httpStatus else base.httpStatus().orElse(null))
            .build()
    }

    @JvmStatic
    fun fromHttpStatus(statusCode: Int, message: String?): ConnectError {
        val payload = if (!message.isNullOrEmpty()) message else "HTTP $statusCode"
        val builder = ConnectError.builder().message(payload).httpStatus(statusCode)
        when {
            statusCode >= 500 ->
                builder.category(ConnectErrorCategory.TRANSPORT).code("http.server_error")
            statusCode == 401 || statusCode == 403 || statusCode == 407 ->
                builder.category(ConnectErrorCategory.AUTHORIZATION).code("http.forbidden")
            statusCode >= 400 ->
                builder.category(ConnectErrorCategory.AUTHORIZATION).code("http.client_error")
            else ->
                builder.category(ConnectErrorCategory.TRANSPORT).code("http.other")
        }
        return builder.build()
    }

    private fun classifyOrFallback(input: Throwable?): ConnectError {
        if (input == null) return unknown(null)
        if (input is ConnectError) return input
        if (input is ConnectErrorConvertible) return input.toConnectError()
        val unwrapped = unwrap(input)
        val direct = classify(unwrapped)
        if (direct != null) return direct
        val cause = unwrapped?.cause
        if (cause != null) {
            val nested = classify(cause)
            if (nested != null) return nested
        }
        return unknown(unwrapped)
    }

    private fun unwrap(error: Throwable?): Throwable? {
        var current = error
        while (true) {
            current = when {
                current is CompletionException && current.cause != null -> current.cause
                current is ExecutionException && current.cause != null -> current.cause
                current is UncheckedIOException && current.cause != null -> current.cause
                current is UndeclaredThrowableException && current.cause != null -> current.cause
                else -> return current
            }
        }
    }

    private fun classify(error: Throwable?): ConnectError? {
        if (error == null) return null
        if (error is ConnectError) return error
        if (error is ConnectErrorConvertible) return error.toConnectError()
        if (error is SSLException || error is CertificateException) {
            return newError(
                ConnectErrorCategory.AUTHORIZATION,
                "network.tls_failure",
                error,
                "TLS negotiation failure",
            )
        }
        if (isTimeout(error)) {
            return newError(
                ConnectErrorCategory.TIMEOUT, "network.timeout", error, "Connect timeout",
            )
        }
        if (isTransport(error)) {
            val code = if (error is SocketException && messageContains(error, "reset")) {
                "client.closed"
            } else {
                "network.socket_failure"
            }
            return newError(ConnectErrorCategory.TRANSPORT, code, error, "Transport failure")
        }
        if (isCodec(error)) {
            return newError(
                ConnectErrorCategory.CODEC, "codec.invalid_payload", error, "Codec failure",
            )
        }
        return null
    }

    private fun isTimeout(error: Throwable): Boolean =
        isClass(error, "java.net.http.HttpTimeoutException") ||
            isClass(error, "java.net.http.HttpConnectTimeoutException") ||
            error is SocketTimeoutException ||
            error is TimeoutException

    private fun isTransport(error: Throwable): Boolean =
        error is ConnectException ||
            error is NoRouteToHostException ||
            error is PortUnreachableException ||
            error is UnknownHostException ||
            error is ClosedChannelException ||
            error is UnresolvedAddressException ||
            isClass(error, "java.net.http.WebSocketHandshakeException") ||
            error is EOFException ||
            error is SocketException ||
            messageContains(error, "websocket") ||
            messageContains(error, "connection refused") ||
            messageContains(error, "connection reset") ||
            messageContains(error, "broken pipe")

    private fun isCodec(error: Throwable): Boolean {
        if (error is ParseException || error is NumberFormatException) return true
        if (error is IllegalStateException || error is IllegalArgumentException) {
            return messageContains(error, "json") ||
                messageContains(error, "codec") ||
                messageContains(error, "norito") ||
                messageContains(error, "payload")
        }
        return false
    }

    private fun messageContains(error: Throwable?, needle: String): Boolean {
        val message = error?.message
        if (message.isNullOrEmpty()) return false
        return message.lowercase(Locale.ROOT).contains(needle.lowercase(Locale.ROOT))
    }

    private fun newError(
        category: ConnectErrorCategory,
        code: String,
        cause: Throwable?,
        fallback: String,
    ): ConnectError {
        val message = messageOrDefault(cause, fallback)
        val builder = ConnectError.builder()
            .category(category)
            .code(code)
            .message(message)
            .cause(cause)
        if (cause != null) {
            builder.underlying(cause.toString())
        }
        return builder.build()
    }

    private fun isClass(error: Throwable?, className: String): Boolean =
        error != null && error.javaClass.name == className

    private fun unknown(cause: Throwable?): ConnectError =
        newError(ConnectErrorCategory.INTERNAL, "unknown_error", cause, "Unknown Connect error")

    private fun messageOrDefault(cause: Throwable?, fallback: String): String {
        val message = cause?.message
        return if (message.isNullOrEmpty()) fallback else message
    }
}
