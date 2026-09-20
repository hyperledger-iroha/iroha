package org.hyperledger.iroha.sdk.client

import java.time.Duration
import kotlin.test.Test
import kotlin.test.assertEquals

class RetryPolicyTest {

    @Test
    fun overflowedDelayRespectsConfiguredMaximum() {
        val maximum = Duration.ofSeconds(5)
        val policy = RetryPolicy(baseDelay = Duration.ofSeconds(Long.MAX_VALUE), maxDelay = maximum)

        assertEquals(maximum, policy.delayForAttempt(2))
        assertEquals(maximum, policy.delayForAttempt(Int.MAX_VALUE))
    }

    @Test
    fun overflowedDelayRespectsZeroMaximum() {
        val policy = RetryPolicy(baseDelay = Duration.ofSeconds(Long.MAX_VALUE), maxDelay = Duration.ZERO)

        assertEquals(Duration.ZERO, policy.delayForAttempt(2))
    }

    @Test
    fun delayScalesUntilConfiguredMaximum() {
        val policy = RetryPolicy(baseDelay = Duration.ofMillis(100), maxDelay = Duration.ofMillis(250))

        assertEquals(Duration.ZERO, policy.delayForAttempt(0))
        assertEquals(Duration.ofMillis(100), policy.delayForAttempt(1))
        assertEquals(Duration.ofMillis(200), policy.delayForAttempt(2))
        assertEquals(Duration.ofMillis(250), policy.delayForAttempt(3))
    }
}
