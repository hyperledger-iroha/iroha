package org.hyperledger.iroha.android.connect;

public final class ConnectRetryPolicyTests {
    private ConnectRetryPolicyTests() {}

    public static void main(String[] args) {
        testCapSaturates();
        testDeterministicSeriesZeroSeed();
        testDeterministicSeriesSequenceSeed();
        testDelayDoesNotExceedCap();
    }

    private static byte[] zeroSeed() {
        return new byte[32];
    }

    private static byte[] sequentialSeed() {
        byte[] bytes = new byte[32];
        for (int i = 0; i < bytes.length; i++) {
            bytes[i] = (byte) i;
        }
        return bytes;
    }

    private static void testCapSaturates() {
        ConnectRetryPolicy policy = new ConnectRetryPolicy();
        assert policy.capMillis(0) == 5_000L : "attempt 0";
        assert policy.capMillis(1) == 10_000L : "attempt 1";
        assert policy.capMillis(2) == 20_000L : "attempt 2";
        assert policy.capMillis(3) == 40_000L : "attempt 3";
        assert policy.capMillis(4) == 60_000L : "attempt 4";
        assert policy.capMillis(30) == 60_000L : "attempt 30";
    }

    private static void testDeterministicSeriesZeroSeed() {
        ConnectRetryPolicy policy = new ConnectRetryPolicy();
        long[] expected = {2_236L, 4_203L, 8_456L, 20_127L, 44_193L, 3_907L};
        byte[] seed = zeroSeed();
        for (int attempt = 0; attempt < expected.length; attempt++) {
            long actual = policy.delayMillis(attempt, seed);
            assert actual == expected[attempt] : "attempt " + attempt;
        }
    }

    private static void testDeterministicSeriesSequenceSeed() {
        ConnectRetryPolicy policy = new ConnectRetryPolicy();
        long[] expected = {922L, 1_579L, 16_071L, 34_738L, 7_203L, 20_824L};
        byte[] seed = sequentialSeed();
        for (int attempt = 0; attempt < expected.length; attempt++) {
            long actual = policy.delayMillis(attempt, seed);
            assert actual == expected[attempt] : "attempt " + attempt;
        }
    }

    private static void testDelayDoesNotExceedCap() {
        ConnectRetryPolicy policy = new ConnectRetryPolicy();
        byte[][] seeds = {zeroSeed(), sequentialSeed()};
        for (int attempt = 0; attempt < 12; attempt++) {
            long cap = policy.capMillis(attempt);
            for (byte[] seed : seeds) {
                long value = policy.delayMillis(attempt, seed);
                assert value <= cap : "attempt " + attempt + " exceeded cap";
            }
        }
    }
}
