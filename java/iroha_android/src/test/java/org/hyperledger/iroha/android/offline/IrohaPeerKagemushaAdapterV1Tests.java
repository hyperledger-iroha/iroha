package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import org.junit.Test;

public final class IrohaPeerKagemushaAdapterV1Tests {
  @Test
  public void requiresAbi21AndPreservesExactBytesThroughNative() {
    final byte[] archive = portableOfferFixture();
    final boolean nativeAvailable =
        KagemushaRecursiveSpendProver.isArtifactStreamingAvailable();
    assertTrue(
        "A freshly built connect_norito_bridge ABI 22 artifact-streaming library is required",
        nativeAvailable);

    final KagemushaPeerTransport.Payload typed =
        KagemushaPeerTransport.Payload.decode(
            archive, KagemushaPeerTransport.Kind.RECEIVE_REQUEST);
    final IrohaPeerWireMessageV1 wrapped = IrohaPeerKagemushaAdapterV1.wrap(typed);
    assertEquals(
        IrohaPeerKagemushaAdapterV1.NATIVE_ARCHIVE_SCHEMA_VERSION,
        wrapped.canonicalPayload().schemaVersion());
    assertEquals(
        IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
        wrapped.canonicalPayload().profile());
    assertEquals(IrohaPeerPayloadKind.RECEIVE_REQUEST, wrapped.canonicalPayload().kind());
    assertArrayEquals(archive, wrapped.canonicalPayload().bytes());
    assertArrayEquals(archive, IrohaPeerKagemushaAdapterV1.decode(wrapped).archive());
    assertEquals(12_423, archive.length);
    assertEquals(12_507, wrapped.encode().length);

    final IrohaPeerWireLimitsV1 tooSmall =
        new IrohaPeerWireLimitsV1(32 * 1024, archive.length - 1);
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerKagemushaAdapterV1.wrap(
            typed, IrohaPeerWireCompressionPolicyV1.DISABLED, tooSmall));
  }

  private static byte[] portableOfferFixture() {
    Path current = Paths.get(System.getProperty("user.dir")).toAbsolutePath();
    while (current != null) {
      final Path candidate = current.resolve(
          "crates/connect_norito_bridge/tests/fixtures/offline_recipient_receive_offer_v2.hex");
      if (Files.isRegularFile(candidate)) {
        try {
          final String hex = new String(Files.readAllBytes(candidate), StandardCharsets.US_ASCII)
              .replaceAll("\\s+", "");
          final byte[] bytes = new byte[hex.length() / 2];
          for (int index = 0; index < bytes.length; index++) {
            bytes[index] = (byte) Integer.parseInt(hex.substring(index * 2, index * 2 + 2), 16);
          }
          return bytes;
        } catch (final IOException failure) {
          throw new AssertionError("unable to load portable offer fixture", failure);
        }
      }
      current = current.getParent();
    }
    throw new AssertionError("portable offer fixture is missing");
  }
}
