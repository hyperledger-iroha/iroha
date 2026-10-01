package org.hyperledger.iroha.sdk.crypto.keystore

import java.util.Base64
import java.util.concurrent.CompletableFuture
import java.util.concurrent.ExecutionException
import org.junit.jupiter.api.Assertions.*
import org.junit.jupiter.api.Test

/** Controlled async transport only. Tokens are inert and no Google/device/verdict is executed. */
class KagemushaAndroidPlayIntegrityProviderV1Test {
    private class Backend : KagemushaPlayIntegrityBackendV1, KagemushaPlayIntegrityPreparedV1 {
        var preparations = 0
        val warm = CompletableFuture<KagemushaPlayIntegrityPreparedV1>()
        val warms = mutableListOf(warm)
        val requests = mutableListOf<Pair<String, CompletableFuture<String>>>()
        override fun prepare(cloudProjectNumber: Long): CompletableFuture<KagemushaPlayIntegrityPreparedV1> {
            if (preparations > 0) warms += CompletableFuture<KagemushaPlayIntegrityPreparedV1>()
            return warms[preparations++]
        }
        override fun invalidatesPreparedProvider(error: Throwable): Boolean = error is ExpiredProvider
        override fun request(originalHashText: String): CompletableFuture<String> = CompletableFuture<String>().also {
            requests += originalHashText to it
        }
    }
    private class ExpiredProvider : IllegalStateException("inert expired provider")

    @Test fun exactImmutableHashUsesSole43CharacterBase64urlAndWarmProviderNeverCachesTokens() {
        val backend = Backend(); val provider = KagemushaAndroidPlayIntegrityProviderV1(backend)
        val source = ByteArray(32) { 31 }; val expected = source.copyOf()
        val first = provider.requestOriginal(1234, source) {}; source.fill(0)
        assertEquals(1, backend.preparations); assertTrue(backend.requests.isEmpty())
        backend.warm.complete(backend)
        assertEquals(Base64.getUrlEncoder().withoutPadding().encodeToString(expected), backend.requests.single().first)
        assertEquals(43, backend.requests.single().first.length)
        backend.requests.single().second.complete("inert.google.token.one")
        val original = first.get(); assertEquals(1234L, original.cloudProjectNumber)
        assertArrayEquals(expected, original.requestHash()); original.requestHash().fill(0)
        assertArrayEquals(expected, original.requestHash()); assertEquals("inert.google.token.one", original.opaqueToken())
        val next = provider.requestOriginal(1234, ByteArray(32) { 32 }) {}
        assertEquals(1, backend.preparations); assertEquals(2, backend.requests.size)
        backend.requests.last().second.complete("inert.google.token.two")
        assertEquals("inert.google.token.two", next.get().opaqueToken())
    }

    @Test fun replacedOwnerAfterWarmupCannotRequestToken() {
        val backend = Backend(); var current = true
        val original = KagemushaAndroidPlayIntegrityProviderV1(backend).requestOriginal(1234, ByteArray(32) { 1 }) { check(current) }
        current = false; backend.warm.complete(backend)
        assertThrows(ExecutionException::class.java) { original.get() }
        assertTrue(backend.requests.isEmpty())
    }

    @Test fun replacedOwnerDuringTokenRequestCannotExposeEvidenceOrRetry() {
        val backend = Backend(); backend.warm.complete(backend); var current = true
        val original = KagemushaAndroidPlayIntegrityProviderV1(backend).requestOriginal(1234, ByteArray(32) { 1 }) { check(current) }
        current = false; backend.requests.single().second.complete("inert.google.token")
        assertThrows(ExecutionException::class.java) { original.get() }
        assertEquals(1, backend.requests.size); assertEquals(1, backend.preparations)
    }

    @Test fun failedWarmupOrTokenIsTypedAndDoesNotAutomaticallyRetry() {
        val cold = Backend()
        val preparation = KagemushaAndroidPlayIntegrityProviderV1(cold).requestOriginal(1234, ByteArray(32) { 1 }) {}
        cold.warm.completeExceptionally(IllegalStateException("inert preparation failure"))
        assertThrows(ExecutionException::class.java) { preparation.get() }; assertEquals(1, cold.preparations); assertTrue(cold.requests.isEmpty())
        val warm = Backend(); warm.warm.complete(warm)
        val request = KagemushaAndroidPlayIntegrityProviderV1(warm).requestOriginal(1234, ByteArray(32) { 1 }) {}
        warm.requests.single().second.completeExceptionally(IllegalStateException("inert request uncertainty"))
        assertThrows(ExecutionException::class.java) { request.get() }; assertEquals(1, warm.requests.size)
    }

    @Test fun invalidRequestOrUnboundedOpaqueTokenCannotBecomeOriginalEvidence() {
        val backend = Backend(); val provider = KagemushaAndroidPlayIntegrityProviderV1(backend)
        for (project in listOf(0L, -1L)) assertThrows(IllegalArgumentException::class.java) {
            provider.requestOriginal(project, ByteArray(32) { 1 }) {}
        }
        for (hash in listOf(ByteArray(31), ByteArray(32), ByteArray(33))) assertThrows(IllegalArgumentException::class.java) {
            provider.requestOriginal(1234, hash) {}
        }
        assertEquals(0, backend.preparations)
        backend.warm.complete(backend)
        for (token in listOf("", "has space", "nonasciié", "x".repeat(64 * 1024 + 1))) {
            val result = provider.requestOriginal(1234, ByteArray(32) { 1 }) {}
            backend.requests.last().second.complete(token)
            assertThrows(ExecutionException::class.java) { result.get() }
        }
        val bounded = provider.requestOriginal(1234, ByteArray(32) { 1 }) {}
        backend.requests.last().second.complete("x".repeat(64 * 1024))
        assertEquals(64 * 1024, bounded.get().opaqueToken().length)
    }

    @Test fun expiredProviderFailsCurrentCallAndOnlyLaterExplicitRequestWarmsAgain() {
        val backend = Backend(); val provider = KagemushaAndroidPlayIntegrityProviderV1(backend)
        backend.warm.complete(backend)
        val first = provider.requestOriginal(1234, ByteArray(32) { 1 }) {}
        backend.requests.single().second.completeExceptionally(ExpiredProvider())
        assertThrows(ExecutionException::class.java) { first.get() }
        assertEquals(1, backend.preparations); assertEquals(1, backend.requests.size)
        val next = provider.requestOriginal(1234, ByteArray(32) { 2 }) {}
        assertEquals(2, backend.preparations); assertEquals(1, backend.requests.size)
        backend.warms.last().complete(backend)
        backend.requests.last().second.complete("inert.fresh.google.token")
        assertEquals("inert.fresh.google.token", next.get().opaqueToken())
    }

    @Test fun lateExpiredResponseCannotInvalidateAReplacementProjectProvider() {
        val backend = Backend(); val provider = KagemushaAndroidPlayIntegrityProviderV1(backend)
        backend.warm.complete(backend)
        val old = provider.requestOriginal(1234, ByteArray(32) { 1 }) {}
        val replacement = provider.requestOriginal(5678, ByteArray(32) { 2 }) {}
        backend.warms.last().complete(backend)
        backend.requests.first().second.completeExceptionally(ExpiredProvider())
        assertThrows(ExecutionException::class.java) { old.get() }
        backend.requests[1].second.complete("inert.replacement.google.token"); replacement.get()
        val next = provider.requestOriginal(5678, ByteArray(32) { 3 }) {}
        assertEquals(2, backend.preparations)
        backend.requests.last().second.complete("inert.same.new.provider"); next.get()
    }

    @Test fun expiredProviderClearsEvenWhenTheOriginalOwnerWasReplaced() {
        val backend = Backend(); val provider = KagemushaAndroidPlayIntegrityProviderV1(backend)
        backend.warm.complete(backend); var current = true
        val old = provider.requestOriginal(1234, ByteArray(32) { 1 }) { check(current) }
        current = false; backend.requests.single().second.completeExceptionally(ExpiredProvider())
        assertThrows(ExecutionException::class.java) { old.get() }
        provider.requestOriginal(1234, ByteArray(32) { 2 }) {}
        assertEquals(2, backend.preparations); assertEquals(1, backend.requests.size)
    }
}
