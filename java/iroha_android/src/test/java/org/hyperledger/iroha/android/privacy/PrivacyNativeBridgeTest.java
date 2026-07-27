package org.hyperledger.iroha.android.privacy;

import java.util.Arrays;
import java.util.List;

public final class PrivacyNativeBridgeTest {
  private static final List<String> EXPECTED =
      Arrays.asList(
          "zk-ace-pq-authorization-v0",
          "anonymous-pgc-k-out-of-n-v1",
          "verange-transparent-range-v1",
          "iroha-zk-ams-v1",
          "vega-existing-credential-zk-v0",
          "iroha-zk-x509-stark-p256-v0",
          "iroha-jindo-polynomial-commitment-v0",
          "iroha-bootle-lantern-anoncred-v1",
          "orchard-halo2-actions-v1",
          "monero-fcmp-plus-plus-v1",
          "iroha-ivm-private-note-stark-v1",
          "pq-masp-stark-v0");

  private PrivacyNativeBridgeTest() {}

  public static void main(final String[] args) {
    exactClosedRegistryIsStable();
    aliasesAndNonCanonicalSpellingsAreRejected();
    capabilityArchiveValidationFailsClosed();
    capabilityArchiveReturnsDefensiveCopy();
    retiredGenericProofSurfaceIsAbsent();
    System.out.println("[IrohaAndroid] PrivacyNativeBridgeTest passed.");
  }

  private static void exactClosedRegistryIsStable() {
    assert PrivacyNativeBridge.REQUIRED_BRIDGE_ABI_VERSION == 21;
    assert PrivacyNativeBridge.protocolsV1().size() == 12;
    for (int index = 0; index < EXPECTED.size(); index++) {
      final String label = EXPECTED.get(index);
      final PrivacyNativeBridge.ProtocolIdV1 protocol = PrivacyNativeBridge.protocolsV1().get(index);
      assert protocol.canonicalLabel().equals(label);
      assert PrivacyNativeBridge.ProtocolIdV1.fromCanonicalLabel(label) == protocol;
    }
    assertThrows(() -> PrivacyNativeBridge.protocolsV1().clear());
  }

  private static void aliasesAndNonCanonicalSpellingsAreRejected() {
    for (final String rejected :
        Arrays.asList(
            "jindo-lattice-pcs-zk-v0",
            "sis-hints-anoncred-pq-v0",
            "silent-threshold-anoncred-v0",
            "zk-ams-recursive-admission-v0",
            "iroha-zk-ams-v1 ",
            "Iroha-Zk-Ams-V1",
            "",
            "unknown-privacy-protocol-v1")) {
      assertThrows(() -> PrivacyNativeBridge.ProtocolIdV1.fromCanonicalLabel(rejected));
    }
    assertThrows(() -> PrivacyNativeBridge.ProtocolIdV1.fromCanonicalLabel(null));
  }

  private static void capabilityArchiveValidationFailsClosed() {
    assertThrows(() -> PrivacyNativeBridge.requireCapabilityArchive(null));
    assertThrows(() -> PrivacyNativeBridge.requireCapabilityArchive(new byte[39]));

    final byte[] badMagic = capabilityArchive();
    badMagic[0] = 'X';
    assertThrows(() -> PrivacyNativeBridge.requireCapabilityArchive(badMagic));

    final byte[] badSchema = capabilityArchive();
    badSchema[13] = 0x51;
    assertThrows(() -> PrivacyNativeBridge.requireCapabilityArchive(badSchema));
  }

  private static void capabilityArchiveReturnsDefensiveCopy() {
    final byte[] archive = capabilityArchive();
    final byte[] accepted = PrivacyNativeBridge.requireCapabilityArchive(archive);
    assert accepted != archive;
    archive[0] = 'X';
    assert accepted[0] == 'N';
  }

  private static void retiredGenericProofSurfaceIsAbsent() {
    for (final java.lang.reflect.Method method : PrivacyNativeBridge.class.getDeclaredMethods()) {
      final String name = method.getName();
      assert !name.contains("ProofRequest") : name;
      assert !name.contains("BuildProof") : name;
      assert !name.contains("VerifyProof") : name;
      assert !name.equals("buildProof") : name;
      assert !name.equals("verifyProof") : name;
    }
  }

  private static byte[] capabilityArchive() {
    final byte[] archive = new byte[40];
    archive[0] = 'N';
    archive[1] = 'R';
    archive[2] = 'T';
    archive[3] = '0';
    Arrays.fill(archive, 6, 22, (byte) 0x50);
    return archive;
  }

  private static void assertThrows(final Runnable runnable) {
    try {
      runnable.run();
      throw new AssertionError("expected failure");
    } catch (final RuntimeException expected) {
      // Expected.
    }
  }
}
