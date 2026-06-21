package org.hyperledger.iroha.android.model.instructions;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertThrows;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Map;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.testing.TestAccountIds;
import org.hyperledger.iroha.norito.CRC64;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;
import org.junit.Test;

public class SetPrimaryAccountAliasWirePayloadEncoderTests {
  private static final TypeAdapter<String> STRING_ADAPTER = NoritoAdapters.stringAdapter();
  private static final TypeAdapter<Long> U64_ADAPTER = NoritoAdapters.uint(64);

  @Test
  public void encodeSupportsDomainScopedAliasesInExplicitDataspace() {
    final InstructionBox instruction =
        SetPrimaryAccountAliasWirePayloadEncoder.encode(
            TestAccountIds.ed25519Authority(0x31), "tidal-river-4161", "hbl.sbp", 10L);

    assertEquals(SetPrimaryAccountAliasWirePayloadEncoder.WIRE_NAME, instruction.name());
    final Map<String, String> arguments = instruction.arguments();
    assertEquals(SetPrimaryAccountAliasWirePayloadEncoder.WIRE_NAME, arguments.get("wire_name"));
    assertFalse(arguments.get("payload_base64").isBlank());
  }

  @Test
  public void encodeDomainAliasUsesTransparentDomainNamePayload() {
    final InstructionBox instruction =
        SetPrimaryAccountAliasWirePayloadEncoder.encode(
            TestAccountIds.ed25519Authority(0x31), "tidal-river-4161", "hbl.sbp", 10L);
    final DecodedNorito decoded = decodeWirePayload(instruction);
    final NoritoDecoder payload = decoded.decoder(decoded.payload);

    readSizedField(payload);
    final byte[] aliasOptionPayload = readSizedField(payload);
    final byte[] leaseExpiryOptionPayload = readSizedField(payload);
    assertEquals(0, payload.remaining());

    final byte[] aliasPayload =
        readSomeOptionPayload(decoded.decoder(aliasOptionPayload), "alias");
    final NoritoDecoder alias = decoded.decoder(aliasPayload);
    assertEquals(
        "tidal-river-4161", decodeSizedField(decoded, alias, STRING_ADAPTER, "alias.label"));
    final byte[] domainOptionPayload = readSizedField(alias);
    assertEquals(
        Long.valueOf(10L), decodeSizedField(decoded, alias, U64_ADAPTER, "alias.dataspace"));
    assertEquals(0, alias.remaining());

    final byte[] domainPayload =
        readSomeOptionPayload(decoded.decoder(domainOptionPayload), "alias.domain");
    final NoritoDecoder domain = decoded.decoder(domainPayload);
    assertEquals(7L, domain.readLength(domain.compactLenActive()));
    assertEquals("hbl.sbp", new String(domain.readBytes(7), StandardCharsets.UTF_8));
    assertEquals(0, domain.remaining());

    final NoritoDecoder leaseExpiry = decoded.decoder(leaseExpiryOptionPayload);
    assertEquals(0, leaseExpiry.readByte());
    assertEquals(0, leaseExpiry.remaining());
  }

  @Test
  public void decodeRoundTripsDomainAlias() {
    final String accountId = TestAccountIds.ed25519Authority(0x31);
    final InstructionBox instruction =
        SetPrimaryAccountAliasWirePayloadEncoder.encode(
            accountId, "tidal-river-4161", "hbl.sbp", 10L);

    final SetPrimaryAccountAliasWirePayloadEncoder.DecodedSetPrimaryAccountAliasPayload decoded =
        SetPrimaryAccountAliasWirePayloadEncoder.decodePayload(wirePayloadBytes(instruction));

    assertEquals(accountId, decoded.accountId());
    final SetPrimaryAccountAliasWirePayloadEncoder.DecodedAccountAlias alias =
        decoded.alias().orElseThrow();
    assertEquals("tidal-river-4161", alias.alias());
    assertEquals("hbl.sbp", alias.aliasDomain().orElseThrow());
    assertEquals(10L, alias.dataspace());
    assertFalse(decoded.leaseExpiryMs().isPresent());
  }

  @Test
  public void decodeRejectsTrailingPayloadBytes() {
    final InstructionBox instruction =
        SetPrimaryAccountAliasWirePayloadEncoder.encode(
            TestAccountIds.ed25519Authority(0x31), "tidal-river-4161", "hbl.sbp", 10L);
    final NoritoHeader.DecodeResult decoded =
        NoritoHeader.decode(wirePayloadBytes(instruction), null);
    decoded.header().validateChecksum(decoded.payload());
    final byte[] mutated = Arrays.copyOf(decoded.payload(), decoded.payload().length + 1);

    assertThrows(
        IllegalArgumentException.class,
        () -> SetPrimaryAccountAliasWirePayloadEncoder.decodePayload(reframe(decoded.header(), mutated)));
  }

  @Test
  public void encodeRejectsNegativeDataspace() {
    final IllegalArgumentException error =
        assertThrows(
            IllegalArgumentException.class,
            () ->
                SetPrimaryAccountAliasWirePayloadEncoder.encode(
                    TestAccountIds.ed25519Authority(0x32), "tidal-river-4161", "mibank", -1L));

    assertEquals("dataspace must be non-negative", error.getMessage());
  }

  @Test
  public void encodeRejectsMalformedAliasDomainSegments() {
    assertEquals(
        "aliasDomain contains an empty segment",
        assertThrows(
                IllegalArgumentException.class,
                () ->
                    SetPrimaryAccountAliasWirePayloadEncoder.encode(
                        TestAccountIds.ed25519Authority(0x32),
                        "tidal-river-4161",
                        "hbl..sbp",
                        10L))
            .getMessage());
    assertEquals(
        "aliasDomain contains unsupported characters",
        assertThrows(
                IllegalArgumentException.class,
                () ->
                    SetPrimaryAccountAliasWirePayloadEncoder.encode(
                        TestAccountIds.ed25519Authority(0x32),
                        "tidal-river-4161",
                        "HBL.sbp",
                        10L))
            .getMessage());
  }

  private static DecodedNorito decodeWirePayload(final InstructionBox instruction) {
    final NoritoHeader.DecodeResult decoded = NoritoHeader.decode(wirePayloadBytes(instruction), null);
    decoded.header().validateChecksum(decoded.payload());
    return new DecodedNorito(decoded.header(), decoded.payload());
  }

  private static byte[] wirePayloadBytes(final InstructionBox instruction) {
    final InstructionBox.WirePayload wirePayload =
        (InstructionBox.WirePayload) instruction.payload();
    return wirePayload.payloadBytes();
  }

  private static byte[] readSizedField(final NoritoDecoder decoder) {
    final long length = decoder.readLength(decoder.compactLenActive());
    return decoder.readBytes(Math.toIntExact(length));
  }

  private static byte[] readSomeOptionPayload(final NoritoDecoder decoder, final String field) {
    assertEquals(field + " must be present", 1, decoder.readByte());
    final byte[] payload = readSizedField(decoder);
    assertEquals("trailing bytes after " + field, 0, decoder.remaining());
    return payload;
  }

  private static <T> T decodeSizedField(
      final DecodedNorito decoded,
      final NoritoDecoder decoder,
      final TypeAdapter<T> adapter,
      final String field) {
    final byte[] payload = readSizedField(decoder);
    final NoritoDecoder child = decoded.decoder(payload);
    final T value = adapter.decode(child);
    assertEquals("trailing bytes after " + field, 0, child.remaining());
    return value;
  }

  private static byte[] reframe(final NoritoHeader header, final byte[] payload) {
    final NoritoHeader reframed =
        new NoritoHeader(
            header.schemaHash(),
            payload.length,
            CRC64.compute(payload),
            header.flags(),
            NoritoHeader.COMPRESSION_NONE,
            header.minor());
    final byte[] headerBytes = reframed.encode();
    final byte[] out = new byte[headerBytes.length + payload.length];
    System.arraycopy(headerBytes, 0, out, 0, headerBytes.length);
    System.arraycopy(payload, 0, out, headerBytes.length, payload.length);
    return out;
  }

  private static final class DecodedNorito {
    private final NoritoHeader header;
    private final byte[] payload;

    private DecodedNorito(final NoritoHeader header, final byte[] payload) {
      this.header = header;
      this.payload = payload.clone();
    }

    private NoritoDecoder decoder(final byte[] payload) {
      return new NoritoDecoder(payload, header.flags(), header.minor());
    }
  }
}
