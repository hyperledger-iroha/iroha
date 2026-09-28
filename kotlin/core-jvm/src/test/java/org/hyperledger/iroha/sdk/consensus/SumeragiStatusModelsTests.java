package org.hyperledger.iroha.sdk.consensus;

import static org.junit.jupiter.api.Assertions.*;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import org.junit.jupiter.api.Test;

/** Java-source consumers use the sole Kotlin-owned native status implementation. */
public final class SumeragiStatusModelsTests {
  @Test public void nativeFixturePreservesUnsignedValuesAndValueSemantics() {
    String json = NativeStatusFixtures.json("validator");
    SumeragiStatus status = SumeragiStatus.parseJson(json);
    assertEquals(8, status.protocolVersion);
    assertEquals(new BigInteger("18446744073709551615"), status.view);
    assertEquals(new BigInteger("4294967295"), status.level);
    assertEquals(new BigInteger("18446744073709551615"), status.footprint.votes);
    assertEquals(BigInteger.ONE, status.applyLag());
    assertTrue(status.isSigning());
    assertFalse(status.isHalted());
    assertEquals(status, SumeragiStatus.parseJson(json.getBytes(StandardCharsets.UTF_8)));
    assertEquals(status.hashCode(), SumeragiStatus.parseJson(json).hashCode());
    assertFalse(SumeragiStatus.parseJson(NativeStatusFixtures.json("observer")).isSigning());
  }
  @Test public void aliasesLegacyFieldsAndMalformedTransportAreRejected() {
    String json = NativeStatusFixtures.json("observer");
    for (String field : new String[]{"height_context", "phase", "last_commit_qc", "liveness", "rbc_status"}) {
      assertThrows(IllegalArgumentException.class, () -> SumeragiStatus.parseJson("{\"" + field + "\":null," + json.substring(1)));
    }
    assertThrows(IllegalArgumentException.class, () -> SumeragiStatus.parseJson(new byte[0]));
    assertThrows(IllegalArgumentException.class, () -> SumeragiStatus.parseJson(new byte[1_048_577]));
    assertThrows(IllegalArgumentException.class, () -> SumeragiStatus.parseJson(new byte[]{(byte)0xff}));
    assertThrows(IllegalArgumentException.class, () -> SumeragiStatus.parseJson("{\"view\":0," + json.substring(1)));
  }
}
