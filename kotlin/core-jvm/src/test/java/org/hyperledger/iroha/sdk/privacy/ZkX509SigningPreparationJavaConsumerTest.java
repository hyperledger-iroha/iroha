package org.hyperledger.iroha.sdk.privacy;

import java.security.MessageDigest;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

/** Java consumes the canonical Kotlin factory and defensive byte accessors. */
public final class ZkX509SigningPreparationJavaConsumerTest {
  @Test public void canonicalKotlinApiPreparesTheSharedX509Statement() throws Exception {
    assertTrue(PrivacyNativeBridge.isNativeAvailable(), "rebuilt JNI privacy bridge is required");
    PrivacyExact12FixtureBundleV1 bundle = PrivacyExact12FixtureCodecV1.decodeCanonical(
        PrivacyNativeBridge.exact12FixtureBundleV1());
    PrivacyExact12TypedFixtureRowV1 row = bundle.rows.stream()
        .filter(value -> value.protocolId == PrivacyProtocolIdV1.IROHA_ZK_X509_STARK_P256_V1)
        .findFirst().orElseThrow(() -> new AssertionError("X509 fixture"));
    ZkX509SigningPreparationV1 prepared =
        ZkX509SigningPreparationV1.prepareOwnershipSigningV1(row.statementNoritoBytes());
    byte[] message = prepared.getMessage();
    byte[] digest = prepared.getDigest();
    assertArrayEquals(MessageDigest.getInstance("SHA-256").digest(message), digest);
    message[0] ^= 1;
    digest[0] ^= 1;
    assertNotEquals(message[0], prepared.getMessage()[0]);
    assertNotEquals(digest[0], prepared.getDigest()[0]);
    ZkX509SigningPreparationException empty = assertThrows(
        ZkX509SigningPreparationException.class,
        () -> ZkX509SigningPreparationV1.prepareOwnershipSigningV1(new byte[0]));
    assertEquals(-2, empty.code);
  }
}
