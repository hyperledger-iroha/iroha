package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import org.hyperledger.iroha.norito.CRC64;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;
import org.junit.Test;

public final class IrohaPeerKagemushaAdapterV1Tests {
  @Test
  public void emitsOnlyFixedNativeArchiveSchemaAndPreservesExactBytes() {
    final byte[] archive = portableOfferFixture();
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
    assertEquals(14_005, archive.length);
    assertEquals(14_089, wrapped.encode().length);

    final IrohaPeerWireLimitsV1 tooSmall =
        new IrohaPeerWireLimitsV1(32 * 1024, archive.length - 1, archive.length - 1);
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

  @Test
  public void rejectsUnexpectedProfileBeforeTypedDecode() {
    final IrohaPeerWireMessageV1 offline =
        new IrohaPeerWireMessageV1(
            new IrohaPeerCanonicalPayload(
                IrohaPeerPayloadProfile.OFFLINE_NOTE,
                IrohaPeerPayloadKind.PAYMENT,
                1,
                new byte[] {1}));
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerKagemushaAdapterV1.decode(offline));
  }

  private static byte[] archive(final String schema) {
    final byte[] payload = {0x51};
    final NoritoHeader header =
        new NoritoHeader(
            SchemaHash.hash16(schema),
            payload.length,
            CRC64.compute(payload),
            NoritoHeader.COMPACT_LEN,
            NoritoHeader.COMPRESSION_NONE);
    final int padding = schema.endsWith("KagemushaReceiverAcknowledgementV2") ? 0 : 8;
    final byte[] archive = new byte[NoritoHeader.HEADER_LENGTH + padding + payload.length];
    System.arraycopy(header.encode(), 0, archive, 0, NoritoHeader.HEADER_LENGTH);
    System.arraycopy(payload, 0, archive, NoritoHeader.HEADER_LENGTH + padding, payload.length);
    return archive;
  }
}
