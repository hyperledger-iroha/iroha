// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline;

import java.io.File;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.List;
import java.util.Map;
import java.util.concurrent.atomic.AtomicInteger;
import org.hyperledger.iroha.sdk.client.JsonParser;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

/** Java source calls the canonical Kotlin middleware owner; no duplicate implementation. */
public class KagemushaEligibilityJavaTest {
    private static byte[] hex(Object value) {
        String text = (String) value;
        byte[] bytes = new byte[text.length() / 2];
        for (int i = 0; i < bytes.length; i++) bytes[i] = (byte) Integer.parseInt(text.substring(2*i, 2*i+2), 16);
        return bytes;
    }
    @Test public void bankCanInjectCurrentLookupAndExistingSignerFromJava() throws Exception {
        File root = new File(".").getCanonicalFile();
        File fixture;
        do {
            fixture = new File(root, "fixtures/kagemusha/enrollment_eligibility_v1_vectors.json");
            if (fixture.isFile()) break;
            root = root.getParentFile();
            assertNotNull(root, "Rust eligibility fixture required");
        } while (true);
        Map<?, ?> vectors = (Map<?, ?>) JsonParser.parse(new String(Files.readAllBytes(fixture.toPath()), StandardCharsets.UTF_8));
        Map<?, ?> row = (Map<?, ?>) ((List<?>) vectors.get("cases")).get(0);
        KagemushaEnrollmentEligibilityV1.Policy policy = KagemushaEnrollmentEligibilityV1.decodePolicy(hex(row.get("policy_hex")));
        AtomicInteger reads = new AtomicInteger();
        byte[] response = KagemushaEnrollmentEligibilityV1.answer(policy, hex(row.get("request_hex")),
            () -> BigInteger.valueOf(1100),
            (selected, request) -> {
                reads.incrementAndGet();
                assertEquals(KagemushaEnrollmentEligibilityV1.Authority.BANK, selected.getAuthority());
                assertEquals(32, request.accountDigest().length);
                return new KagemushaEnrollmentEligibilityV1.Current(true, false, BigInteger.valueOf(6), BigInteger.valueOf(1100));
            }, message -> {
                assertArrayEquals(hex(row.get("signing_message_hex")), message);
                return hex(row.get("signature_hex"));
            });
        assertArrayEquals(hex(row.get("response_hex")), response);
        assertEquals(1, reads.get());
    }
}
