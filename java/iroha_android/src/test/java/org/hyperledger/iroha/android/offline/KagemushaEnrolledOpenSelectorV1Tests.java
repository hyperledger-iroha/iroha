// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertThrows;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Arrays;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import org.hyperledger.iroha.sdk.offline.KagemushaAssetDefinitionIdV1;
import org.hyperledger.iroha.sdk.offline.KagemushaEnrolledOpenSelectorV1;
import org.hyperledger.iroha.sdk.offline.KagemushaRetailEnrollmentOwnerV1;
import org.hyperledger.iroha.sdk.offline.KagemushaRetailEnrollmentRuntimeV1;
import org.junit.Test;

/** Java consumes exactly the same untrusted Rust owner projection as the Kotlin SDK. */
public final class KagemushaEnrolledOpenSelectorV1Tests {
  @Test public void rustSelectorAndOwnerIdentityMatchExactly() throws Exception {
    final byte[] bytes = fixture("selector_canonical_hex");
    final KagemushaEnrolledOpenSelectorV1 value =
        KagemushaNoritoV1.decodeEnrolledOpenSelectorShapeExact(bytes);
    assertArrayEquals(bytes, KagemushaNoritoV1.encodeEnrolledOpenSelectorShape(value));
    assertArrayEquals(fixture("enrollment_id_hex"), value.enrollmentId());
    assertArrayEquals(value.enrollmentId(), KagemushaNoritoV1.retailEnrollmentIdentityShape(value.owner));
    assertEquals("mibank", value.owner.runtime.fiId);
    assertEquals("mibank.bpng", value.owner.runtime.authenticationNamespace);
    assertEquals(10L, value.owner.runtime.ledgerDataspaceId);
    assertEquals(2, value.owner.runtime.scale);
    assertArrayEquals(fixture("lane_id_hex"), value.owner.laneId());
  }

  @Test public void oldPathsJsonEveryTruncationAndOversizeAreRejected() throws Exception {
    final byte[] bytes = fixture("selector_canonical_hex");
    for (int length = 0; length < bytes.length; length++) {
      final byte[] truncated = Arrays.copyOf(bytes, length);
      assertThrows(IllegalArgumentException.class,
          () -> KagemushaNoritoV1.decodeEnrolledOpenSelectorShapeExact(truncated));
    }
    final byte[][] invalid = {
        "/durable/wallet.db".getBytes(StandardCharsets.UTF_8),
        "{}".getBytes(StandardCharsets.UTF_8),
        Arrays.copyOf(bytes, bytes.length + 1),
        new byte[KagemushaNoritoV1.MAXIMUM_ENROLLED_OPEN_SELECTOR_BYTES + 1],
        fixture("owner_canonical_hex")
    };
    for (final byte[] value : invalid) {
      assertThrows(IllegalArgumentException.class,
          () -> KagemushaNoritoV1.decodeEnrolledOpenSelectorShapeExact(value));
    }
  }

  @Test public void changedBankScopeCannotReuseThePreviousOwnerIdentity() throws Exception {
    final KagemushaEnrolledOpenSelectorV1 original =
        KagemushaNoritoV1.decodeEnrolledOpenSelectorShapeExact(fixture("selector_canonical_hex"));
    final KagemushaRetailEnrollmentRuntimeV1 runtime = original.owner.runtime;
    final KagemushaRetailEnrollmentOwnerV1 owner = new KagemushaRetailEnrollmentOwnerV1(
        original.owner.accountId, new KagemushaRetailEnrollmentRuntimeV1("other-bank",
            runtime.ledgerDataspaceId, runtime.authenticationNamespace, runtime.networkId,
            runtime.asset, runtime.assetIncarnation, runtime.scale), original.owner.laneId());
    final KagemushaEnrolledOpenSelectorV1 substituted =
        new KagemushaEnrolledOpenSelectorV1(1, owner, original.enrollmentId());
    assertThrows(IllegalArgumentException.class,
        () -> KagemushaNoritoV1.encodeEnrolledOpenSelectorShape(substituted));
    assertFalse(Arrays.equals(original.enrollmentId(),
        KagemushaEnrolledOpenSelectorV1.fromOwner(owner).enrollmentId()));
  }

  @Test public void selectorAndOwnerArraysRemainDefensive() throws Exception {
    final byte[] bytes = fixture("selector_canonical_hex");
    final KagemushaEnrolledOpenSelectorV1 original = KagemushaNoritoV1.decodeEnrolledOpenSelectorShapeExact(bytes);
    final byte[] lane = original.owner.laneId();
    final byte[] id = original.enrollmentId();
    final KagemushaEnrolledOpenSelectorV1 copy = new KagemushaEnrolledOpenSelectorV1(1,
        new KagemushaRetailEnrollmentOwnerV1(original.owner.accountId, original.owner.runtime, lane), id);
    Arrays.fill(lane, (byte) 0);
    Arrays.fill(id, (byte) 0);
    Arrays.fill(copy.owner.laneId(), (byte) 0);
    Arrays.fill(copy.enrollmentId(), (byte) 0);
    Arrays.fill(bytes, (byte) 0);
    assertArrayEquals(fixture("selector_canonical_hex"), KagemushaNoritoV1.encodeEnrolledOpenSelectorShape(copy));
  }

  @Test public void invalidAssetUuidCannotEnterTheSharedProjection() throws Exception {
    final KagemushaEnrolledOpenSelectorV1 original =
        KagemushaNoritoV1.decodeEnrolledOpenSelectorShapeExact(fixture("selector_canonical_hex"));
    final byte[] payload = original.owner.runtime.asset.canonicalPayload();
    payload[13] = 0x30;
    assertThrows(IllegalArgumentException.class, () -> KagemushaAssetDefinitionIdV1.fromCanonicalPayload(payload));
  }

  private static byte[] fixture(final String field) throws Exception {
    Path fixture = null;
    for (final String prefix : Arrays.asList("../../../", "../../", "../", "")) {
      final Path candidate = Paths.get(prefix + "fixtures/offline/kagemusha_enrolled_open_selector_v1.json");
      if (Files.isRegularFile(candidate)) { fixture = candidate; break; }
    }
    if (fixture == null) throw new AssertionError("shared Rust selector fixture missing");
    final String json = new String(Files.readAllBytes(fixture), StandardCharsets.UTF_8);
    final Matcher matcher = Pattern.compile("\"" + field + "\"\\s*:\\s*\"([^\"]+)\"").matcher(json);
    if (!matcher.find()) throw new AssertionError("fixture field missing: " + field);
    final String hex = matcher.group(1);
    final byte[] bytes = new byte[hex.length() / 2];
    for (int index = 0; index < bytes.length; index++) {
      bytes[index] = (byte) Integer.parseInt(hex.substring(index * 2, index * 2 + 2), 16);
    }
    return bytes;
  }
}
