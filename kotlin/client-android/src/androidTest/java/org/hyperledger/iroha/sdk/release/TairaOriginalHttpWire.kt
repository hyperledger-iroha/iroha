// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.release

import java.io.ByteArrayOutputStream
import java.net.Proxy
import java.util.concurrent.TimeUnit
import okhttp3.Authenticator
import okhttp3.ConnectionPool
import okhttp3.CookieJar
import okhttp3.OkHttpClient
import okhttp3.Request
import org.hyperledger.iroha.sdk.client.JsonParser

/** Credential-free, redirect-free, read-only transport retaining original response bytes. */
internal class TairaOriginalHttpWire(
    private val evidence: TairaQualificationEvidence,
    private val directory: String,
) {
    private val requests = mutableListOf<Map<String, Any?>>()
    private val responses = mutableListOf<Map<String, Any?>>()
    private val client = OkHttpClient.Builder()
        .cookieJar(CookieJar.NO_COOKIES)
        .authenticator(Authenticator.NONE)
        .proxyAuthenticator(Authenticator.NONE)
        .proxy(Proxy.NO_PROXY)
        .followRedirects(false)
        .followSslRedirects(false)
        .retryOnConnectionFailure(false)
        .cache(null)
        .connectionPool(ConnectionPool(0, 1, TimeUnit.SECONDS))
        .connectTimeout(10, TimeUnit.SECONDS)
        .readTimeout(10, TimeUnit.SECONDS)
        .callTimeout(20, TimeUnit.SECONDS)
        .build()

    fun get(url: String): Map<*, *> {
        check(url == ACCOUNT_LIST_URL || url.startsWith(ACCOUNT_DETAIL_PREFIX)) {
            "Only the exact public Taira account GET corridor is allowed"
        }
        check(requests.size < 2) { "Unexpected additional wire request" }
        val index = requests.size
        val request = Request.Builder().url(url).get()
            .header("Accept", "application/json")
            .header("Cache-Control", "no-cache")
            .build()
        // These are the explicit application headers, not invented transport-wire headers.
        requests += evidence.writeJson("$directory/http-$index.request.json", mapOf(
            "method" to "GET", "url" to url,
            "headers" to mapOf("Accept" to "application/json", "Cache-Control" to "no-cache"),
            "body" to null,
        ))
        client.newCall(request).execute().use { response ->
            val contentType = response.header("Content-Type") ?: ""
            val metadata = evidence.writeJson("$directory/http-$index.metadata.json", mapOf(
                "url" to url, "status" to response.code, "contentType" to contentType,
            ))
            val body = checkNotNull(response.body) { "Account response has no body" }
            val bytes = body.byteStream().use { input ->
                val output = ByteArrayOutputStream()
                val buffer = ByteArray(8192)
                while (true) {
                    val size = input.read(buffer)
                    if (size == -1) break
                    check(output.size().toLong() + size <= MAX_BODY_BYTES) {
                        "Account response exceeds the original evidence bound"
                    }
                    output.write(buffer, 0, size)
                }
                output.toByteArray()
            }
            val original = evidence.write("$directory/http-$index.body.json", bytes)
            responses += mapOf("body" to original, "metadata" to metadata)
            check(response.request.url.toString() == url) { "Response URL changed" }
            check(response.code == 200) { "Account GET did not succeed" }
            check(contentType.lowercase(java.util.Locale.ROOT).startsWith("application/json")) {
                "Account GET did not return JSON"
            }
            check(bytes.isNotEmpty()) { "Account response is empty" }
            return JsonParser.parse(TairaQualificationEvidence.utf8(bytes)) as? Map<*, *>
                ?: error("Account response is not a JSON object")
        }
    }

    fun requestWrapper(): Map<String, Any?> = evidence.writeJson(
        "$directory/request.json", mapOf("requests" to requests.toList()),
    )

    fun responseWrapper(origin: Map<String, Any?>): Map<String, Any?> = evidence.writeJson(
        "$directory/response.json", mapOf("origin" to origin, "responses" to responses.toList()),
    )

    companion object {
        const val ACCOUNT_LIST_URL = "https://taira.sora.org/v1/accounts?limit=1"
        const val ACCOUNT_DETAIL_PREFIX = "https://taira.sora.org/v1/accounts/"
        private const val MAX_BODY_BYTES = 1024L * 1024L

        fun firstAccount(list: Map<*, *>): String {
            val items = list["items"] as? List<*> ?: error("Account list has no items array")
            check(items.size == 1) { "Exact limit-one account response is required" }
            val account = items[0] as? Map<*, *> ?: error("Account list item is not an object")
            val literal = account["id"] as? String ?: error("Account list has no id string")
            check(literal.isNotEmpty() && TairaQualificationEvidence.utf8(literal).size <= 36 * 1024) {
                "Account literal exceeds the wire evidence bound"
            }
            return literal
        }

        /** RFC 3986 path-segment encoding, matching the maintained verifier's exact URL. */
        fun detailUrl(literal: String): String = ACCOUNT_DETAIL_PREFIX + buildString {
            val hex = "0123456789ABCDEF"
            for (byte in TairaQualificationEvidence.utf8(literal)) {
                val value = byte.toInt() and 0xff
                if (value in 65..90 || value in 97..122 || value in 48..57 ||
                    value == 45 || value == 46 || value == 95 || value == 126) {
                    append(value.toChar())
                } else {
                    append('%').append(hex[value ushr 4]).append(hex[value and 15])
                }
            }
        }
    }
}
