package org.hyperledger.iroha.android.sccp;

import java.math.BigInteger;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.client.JsonParser;

/** Shared-vector and adversarial tests for the exact first-release SCCP layout. */
public final class SccpV1Tests {
  private SccpV1Tests() {}

  public static void main(final String[] args) throws Exception {
    sharedExactBindingVectorsMatchRust();
    bindingRotationPreservesIdentity();
    malformedTopologyAndHashRolesFail();
    canonicalCommitmentRejectsTampering();
    externalAccountCodecsAreBinary();
    nativeSourceEventDigestMatchesSharedVectors();
    System.out.println("[IrohaAndroid] exact SCCP V1 tests passed.");
  }

  private static void sharedExactBindingVectorsMatchRust() throws Exception {
    final Map<String, Object> fixture = fixture();
    assert "sccp_exact_binding_v1".equals(string(fixture, "schema"));
    for (final Object raw : list(fixture, "networks")) {
      final Map<String, Object> vector = object(raw);
      final SccpNetworkV1 network = profile(string(vector, "profile"));
      assert intValue(vector, "tag") == network.tag();
      assert string(vector, "canonical_bytes")
          .equals(SccpV1.encodeLowerHex(SccpV1.canonicalNetworkBytes(network)));
    }
    final Map<String, Object> payloadObject = object(fixture.get("payload"));
    final SccpPayloadV1 payload =
        new SccpPayloadV1.TokenPause(
            intValue(payloadObject, "target_domain"),
            Long.parseLong(string(payloadObject, "nonce")),
            SccpV1.decodeLowerHex(string(payloadObject, "sora_asset_id")));
    assert string(payloadObject, "canonical_bytes").equals(SccpV1.encodeLowerHex(payload.canonicalBytes()));
    assert string(payloadObject, "payload_hash").equals(SccpV1.encodeLowerHex(SccpV1.payloadHash(payload)));
    final byte[] binding = SccpV1.decodeLowerHex(string(fixture, "destination_binding_hash"));
    for (final Object raw : list(fixture, "positive_vectors")) {
      final Map<String, Object> vector = object(raw);
      final SccpLaneIdV1 lane =
          new SccpLaneIdV1(profile(string(vector, "source_profile")), profile(string(vector, "target_profile")));
      assert string(vector, "canonical_lane").equals(SccpV1.encodeLowerHex(SccpV1.canonicalLaneBytes(lane)));
      assert string(vector, "lane_hash").equals(SccpV1.encodeLowerHex(SccpV1.laneHash(lane)));
      final SccpHubCommitmentV1 commitment =
          SccpV1.commitment(new SccpOutboundMessageContextV1(lane, binding), payload);
      assert string(vector, "message_id").equals(SccpV1.encodeLowerHex(commitment.messageId()));
      assert string(vector, "canonical_commitment").equals(SccpV1.encodeLowerHex(SccpV1.canonicalCommitmentBytes(commitment)));
      assert string(vector, "commitment_root").equals(SccpV1.encodeLowerHex(SccpV1.commitmentRoot(commitment)));
      assert Arrays.equals(
          commitment.messageId(),
          SccpV1.decodeCanonicalCommitment(SccpV1.canonicalCommitmentBytes(commitment)).messageId());
    }
  }

  private static void bindingRotationPreservesIdentity() throws Exception {
    final Map<String, Object> rotation = object(fixture().get("binding_rotation"));
    final SccpLaneIdV1 lane =
        new SccpLaneIdV1(profile(string(rotation, "source_profile")), profile(string(rotation, "target_profile")));
    final SccpPayloadV1 payload = samplePayload();
    final SccpHubCommitmentV1 old =
        SccpV1.commitment(
            new SccpOutboundMessageContextV1(lane, SccpV1.decodeLowerHex(string(rotation, "old_binding_hash"))), payload);
    final SccpHubCommitmentV1 updated =
        SccpV1.commitment(
            new SccpOutboundMessageContextV1(lane, SccpV1.decodeLowerHex(string(rotation, "new_binding_hash"))), payload);
    assert Arrays.equals(old.messageId(), updated.messageId());
    assert !Arrays.equals(SccpV1.commitmentRoot(old), SccpV1.commitmentRoot(updated));
    assert string(rotation, "new_commitment_root").equals(SccpV1.encodeLowerHex(SccpV1.commitmentRoot(updated)));
  }

  private static void malformedTopologyAndHashRolesFail() {
    assert SccpNetworkV1.fromProfileKey("sora-nexus") == SccpNetworkV1.SORA_NEXUS;
    for (final String alias : List.of("SORA-NEXUS", "sora_nexus", "sora", "sora-nexus ", " bsc-mainnet", "bsc")) {
      assert SccpNetworkV1.fromProfileKey(alias) == null : alias;
    }
    expectFailure(() -> new SccpLaneIdV1(SccpNetworkV1.SORA_NEXUS, SccpNetworkV1.SORA_TAIRA));
    expectFailure(() -> new SccpLaneIdV1(SccpNetworkV1.BSC_MAINNET, SccpNetworkV1.ETHEREUM_MAINNET));
    final SccpLaneIdV1 inbound = new SccpLaneIdV1(SccpNetworkV1.BSC_MAINNET, SccpNetworkV1.SORA_NEXUS);
    expectFailure(() -> new SccpOutboundMessageContextV1(inbound, repeated(1, 32)));
    final SccpLaneIdV1 lane = new SccpLaneIdV1(SccpNetworkV1.SORA_NEXUS, SccpNetworkV1.BSC_MAINNET);
    expectFailure(() -> new SccpOutboundMessageContextV1(lane, new byte[32]));
    final SccpPayloadV1 payload = samplePayload();
    for (final byte[] collision : List.of(SccpV1.laneHash(lane), SccpV1.messageId(lane, payload), SccpV1.payloadHash(payload))) {
      expectFailure(() -> SccpV1.commitment(new SccpOutboundMessageContextV1(lane, collision), payload));
    }
    final SccpLaneIdV1 wrong = new SccpLaneIdV1(SccpNetworkV1.SORA_NEXUS, SccpNetworkV1.ETHEREUM_MAINNET);
    expectFailure(() -> SccpV1.messageId(wrong, payload));
  }

  private static void canonicalCommitmentRejectsTampering() {
    final SccpLaneIdV1 lane = new SccpLaneIdV1(SccpNetworkV1.SORA_NEXUS, SccpNetworkV1.BSC_MAINNET);
    final byte[] encoded =
        SccpV1.canonicalCommitmentBytes(
            SccpV1.commitment(new SccpOutboundMessageContextV1(lane, repeated(0x55, 32)), samplePayload()));
    for (final int offset : new int[] {0, 1, 2, 3}) {
      final byte[] tampered = encoded.clone();
      tampered[offset] = 0x7f;
      expectFailure(() -> SccpV1.decodeCanonicalCommitment(tampered));
    }
    expectFailure(() -> SccpV1.decodeCanonicalCommitment(Arrays.copyOf(encoded, encoded.length + 1)));
    final byte[] collision = encoded.clone();
    System.arraycopy(encoded, 36, collision, 4, 32);
    expectFailure(() -> SccpV1.decodeCanonicalCommitment(collision));
  }

  private static void externalAccountCodecsAreBinary() {
    validTransfer(2, repeated(1, 20));
    validTransfer(3, repeated(2, 32));
    validTransfer(4, ByteBuffer.allocate(36).order(ByteOrder.LITTLE_ENDIAN).putInt(0).put(repeated(3, 32)).array());
    final byte[] tron = new byte[21]; tron[0] = 0x41; Arrays.fill(tron, 1, tron.length, (byte) 4); validTransfer(5, tron);
    expectFailure(() -> validTransfer(2, ("0x" + "11".repeat(20)).getBytes(StandardCharsets.US_ASCII)));
    expectFailure(() -> validTransfer(2, repeated(1, 19)));
    expectFailure(() -> validTransfer(3, repeated(1, 31)));
    expectFailure(() -> validTransfer(4, repeated(1, 36)));
    expectFailure(() -> validTransfer(4, ByteBuffer.allocate(36).order(ByteOrder.LITTLE_ENDIAN).putInt(2).put(repeated(1, 32)).array()));
    final byte[] badTron = tron.clone(); badTron[0] = 0x42; expectFailure(() -> validTransfer(5, badTron));
    validCanonicalText("!".getBytes(StandardCharsets.US_ASCII));
    validCanonicalText(repeated('a', 256));
    for (final byte[] malformed :
        List.of(
            new byte[0],
            "contains space".getBytes(StandardCharsets.US_ASCII),
            "line\nbreak".getBytes(StandardCharsets.US_ASCII),
            new byte[] {0x7f},
            "é".getBytes(StandardCharsets.UTF_8),
            repeated('a', 257))) {
      expectFailure(() -> validCanonicalText(malformed));
    }
  }

  private static void nativeSourceEventDigestMatchesSharedVectors() throws Exception {
    final Map<String, Object> fixture = fixture("native_transfer_event_v1.json");
    assert intValue(fixture, "version") == 1;
    for (final Object raw : list(fixture, "vectors")) {
      final Map<String, Object> vector = object(raw);
      final SccpLaneIdV1 lane =
          new SccpLaneIdV1(
              profile(string(vector, "source_profile")),
              profile(string(vector, "target_profile")));
      final byte[] messageId = SccpV1.decodeLowerHex(string(vector, "message_id_hex"));
      final byte[] payloadHash = SccpV1.decodeLowerHex(string(vector, "payload_hash_hex"));
      assert string(vector, "canonical_lane_hex")
          .equals(SccpV1.encodeLowerHex(SccpV1.canonicalLaneBytes(lane)));
      assert string(vector, "lane_hash_hex").equals(SccpV1.encodeLowerHex(SccpV1.laneHash(lane)));
      assert string(vector, "source_event_digest_hex")
          .equals(SccpV1.encodeLowerHex(SccpV1.sourceEventDigest(lane, messageId, payloadHash)));
    }
    final SccpLaneIdV1 lane =
        new SccpLaneIdV1(SccpNetworkV1.BSC_MAINNET, SccpNetworkV1.SORA_TAIRA);
    final byte[] laneHash = SccpV1.laneHash(lane);
    final byte[] message = repeated(1, 32);
    final byte[] payload = repeated(2, 32);
    expectFailure(() -> SccpV1.sourceEventDigest(lane, new byte[32], payload));
    expectFailure(() -> SccpV1.sourceEventDigest(lane, message, message));
    expectFailure(() -> SccpV1.sourceEventDigest(lane, laneHash, payload));
  }

  private static void validTransfer(final int codec, final byte[] recipient) {
    final int domain = switch (codec) { case 2 -> 1; case 3 -> 3; case 4 -> 4; case 5 -> 5; default -> throw new AssertionError(); };
    new SccpPayloadV1.Transfer(
        0, domain, BigInteger.ONE, 0, 6, repeated(9, 32), BigInteger.ONE,
        1, "alice".getBytes(StandardCharsets.UTF_8), codec, recipient,
        1, "route".getBytes(StandardCharsets.UTF_8));
  }

  private static void validCanonicalText(final byte[] value) {
    new SccpPayloadV1.AssetRegister(1, 0, BigInteger.ONE, 1, value, 18);
  }

  private static SccpPayloadV1 samplePayload() { return new SccpPayloadV1.TokenPause(2, 41, repeated(0x31, 32)); }
  private static byte[] repeated(final int value, final int length) { final byte[] out = new byte[length]; Arrays.fill(out, (byte) value); return out; }
  private static SccpNetworkV1 profile(final String value) { final SccpNetworkV1 result = SccpNetworkV1.fromProfileKey(value); if (result == null) throw new AssertionError(value); return result; }
  private static void expectFailure(final Runnable action) { try { action.run(); throw new AssertionError("expected rejection"); } catch (final IllegalArgumentException expected) { /* expected */ } }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> fixture() throws Exception {
    return fixture("exact_binding_v1.json");
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> fixture(final String name) throws Exception {
    Path path = null;
    for (final String candidate :
        List.of(
            "fixtures/sccp/" + name,
            "../fixtures/sccp/" + name,
            "../../fixtures/sccp/" + name,
            "../../../fixtures/sccp/" + name)) {
      final Path resolved = Paths.get(candidate);
      if (Files.isRegularFile(resolved)) {
        path = resolved;
        break;
      }
    }
    if (path == null) {
      throw new IllegalStateException("unable to locate fixtures/sccp/" + name);
    }
    return (Map<String, Object>) JsonParser.parse(new String(Files.readAllBytes(path), StandardCharsets.UTF_8));
  }
  @SuppressWarnings("unchecked") private static Map<String, Object> object(final Object value) { return (Map<String, Object>) value; }
  @SuppressWarnings("unchecked") private static List<Object> list(final Map<String, Object> value, final String key) { return (List<Object>) value.get(key); }
  private static String string(final Map<String, Object> value, final String key) { return (String) value.get(key); }
  private static int intValue(final Map<String, Object> value, final String key) { return ((Number) value.get(key)).intValue(); }
}
