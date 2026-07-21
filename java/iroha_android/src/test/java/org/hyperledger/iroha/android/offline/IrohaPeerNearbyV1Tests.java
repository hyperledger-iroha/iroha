package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Arrays;
import java.util.Map;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyAuthenticationV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyDiscoveryContextV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyEncryptedRecordV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyHelloV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyP256V1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNearbySessionV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNearbySignatureVerifierV1;
import org.junit.Test;

public final class IrohaPeerNearbyV1Tests {
  @Test
  public void fullPeerMessageAndNearbyRecordCeilingsAreExact() {
    assertEquals(
        32 * 1_024 - 64,
        org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyV1.MAXIMUM_MESSAGE_BYTES);
    assertEquals(84 + 24_576, IrohaPeerNfcV1.MAXIMUM_MESSAGE_BYTES);
    assertTrue(
        IrohaPeerNfcV1.MAXIMUM_MESSAGE_BYTES
            <= org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyV1.MAXIMUM_MESSAGE_BYTES);

    final int maximum = org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyV1.MAXIMUM_MESSAGE_BYTES;
    final IrohaPeerNearbyEncryptedRecordV1 record =
        new IrohaPeerNearbyEncryptedRecordV1(
            org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.OFFLINE_NOTE,
            org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyRoleV1.SENDER,
            repeat(16, 0x5a),
            0,
            repeat(maximum + 16, 0x5b));
    assertEquals(maximum + 54, record.encode().length);
    assertTrue(record.encode().length <= 32 * 1_024);
    assertArrayEquals(
        record.encode(),
        IrohaPeerNearbyEncryptedRecordV1.decode(record.encode()).encode());
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new IrohaPeerNearbyEncryptedRecordV1(
                org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.OFFLINE_NOTE,
                org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyRoleV1.SENDER,
                repeat(16, 0x5a),
                0,
                repeat(maximum + 17, 0x5b)));
  }

  @Test
  public void authenticationSignatureFitsCommonRadioRecordCeiling() {
    final int maximum =
        org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyV1
            .MAXIMUM_AUTHENTICATION_SIGNATURE_BYTES;
    final IrohaPeerNearbyAuthenticationV1 authentication =
        new IrohaPeerNearbyAuthenticationV1(
            org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.OFFLINE_NOTE,
            org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyRoleV1.SENDER,
            repeat(16, 1),
            repeat(32, 2),
            repeat(maximum, 3));
    assertEquals(32 * 1_024, authentication.encode().length);
    assertArrayEquals(
        authentication.encode(),
        IrohaPeerNearbyAuthenticationV1.decode(authentication.encode()).encode());
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new IrohaPeerNearbyAuthenticationV1(
                org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.OFFLINE_NOTE,
                org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyRoleV1.SENDER,
                repeat(16, 1),
                repeat(32, 2),
                repeat(maximum + 1, 3)));
  }

  @Test
  @SuppressWarnings("unchecked")
  public void matchesDiscoveryHelloAuthenticationAndCodecFixtures() throws Exception {
    final Map<String, Object> fixture = fixture();
    final byte[] session = hex((String) fixture.get("session_hex"));
    final byte[] request = hex((String) fixture.get("request_hash_hex"));
    final IrohaPeerNearbyDiscoveryContextV1 discovery =
        new IrohaPeerNearbyDiscoveryContextV1(
            org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.OFFLINE_NOTE,
            org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyRoleV1.RECEIVER,
            session,
            request);
    assertEquals(fixture.get("service_id"), IrohaPeerNearbySecureChannelV1.SERVICE_ID);
    assertArrayEquals(hex((String) fixture.get("discovery_receiver_hex")), discovery.encode());
    assertEquals(discovery, IrohaPeerNearbyDiscoveryContextV1.decode(discovery.encode()));

    final byte[] senderPrivate = scalar(1);
    final byte[] receiverPrivate = scalar(2);
    final IrohaPeerNearbySecureChannelV1 sender =
        new IrohaPeerNearbySecureChannelV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerNearbyRoleV1.SENDER,
            session,
            request,
            new byte[] {(byte) 0xa1, (byte) 0xa2},
            repeat(32, 0x51),
            senderPrivate);
    final IrohaPeerNearbySecureChannelV1 receiver =
        new IrohaPeerNearbySecureChannelV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerNearbyRoleV1.RECEIVER,
            session,
            request,
            new byte[] {(byte) 0xb1, (byte) 0xb2, (byte) 0xb3},
            repeat(32, 0x52),
            receiverPrivate);
    assertArrayEquals(hex((String) fixture.get("sender_hello_hex")), sender.localHello());
    assertArrayEquals(hex((String) fixture.get("receiver_hello_hex")), receiver.localHello());
    sender.acceptPeerHello(receiver.localHello());
    receiver.acceptPeerHello(sender.localHello());
    assertThrows(IllegalArgumentException.class, () -> sender.acceptPeerHello(receiver.localHello()));
    final byte[] senderAuthentication = sender.makeAuthentication(new byte[] {(byte) 0x99, (byte) 0x98});
    assertArrayEquals(hex((String) fixture.get("sender_auth_hex")), senderAuthentication);
    final IrohaPeerNearbyAuthenticationV1 decoded =
        IrohaPeerNearbyAuthenticationV1.decode(senderAuthentication);
    assertEquals(fixture.get("transcript_hash_hex"), toHex(decoded.getTranscriptHash()));

    final IrohaPeerNearbyEncryptedRecordV1 codec =
        new IrohaPeerNearbyEncryptedRecordV1(
            org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.OFFLINE_NOTE,
            org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyRoleV1.SENDER,
            session,
            0,
            ascending(20));
    assertArrayEquals(hex((String) fixture.get("encrypted_record_codec_hex")), codec.encode());

    final IrohaPeerNearbyDiscoveryContextV1 bootstrap =
        IrohaPeerNearbyDiscoveryContextV1.senderBootstrap(
            org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.OFFLINE_NOTE);
    assertArrayEquals(new byte[16], bootstrap.getSessionId());
    assertArrayEquals(new byte[32], bootstrap.getRequestCanonicalHash());
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new IrohaPeerNearbyDiscoveryContextV1(
                org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.OFFLINE_NOTE,
                org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyRoleV1.SENDER,
                new byte[16],
                repeat(32, 1)));
  }

  @Test
  @SuppressWarnings("unchecked")
  public void matchesAesGcmFixturesInBothDirectionsAndRejectsReplay() throws Exception {
    final Map<String, Object> vector = (Map<String, Object>) fixture().get("aes_gcm");
    final byte[] session = repeat(16, ((Number) vector.get("session_repeat_byte")).intValue());
    final byte[] request = repeat(32, ((Number) vector.get("request_hash_repeat_byte")).intValue());
    final IrohaPeerNearbySessionV1 sender =
        sharedSession(
            org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyRoleV1.SENDER,
            session,
            request,
            new byte[] {1},
            repeat(32, ((Number) vector.get("sender_nonce_repeat_byte")).intValue()),
            ((Number) vector.get("sender_private_scalar")).intValue());
    final IrohaPeerNearbySessionV1 receiver =
        sharedSession(
            org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyRoleV1.RECEIVER,
            session,
            request,
            new byte[] {2},
            repeat(32, ((Number) vector.get("receiver_nonce_repeat_byte")).intValue()),
            ((Number) vector.get("receiver_private_scalar")).intValue());
    sender.acceptPeerHello(receiver.getLocalHello());
    receiver.acceptPeerHello(sender.getLocalHello());
    final IrohaPeerNearbyAuthenticationV1 senderAuth = sender.makeAuthentication(new byte[] {0x11});
    final IrohaPeerNearbyAuthenticationV1 receiverAuth = receiver.makeAuthentication(new byte[] {0x22});
    assertEquals(vector.get("transcript_hash_hex"), toHex(senderAuth.getTranscriptHash()));
    final IrohaPeerNearbySignatureVerifierV1 acceptAll = (role, cert, signed, signature) -> true;
    sender.acceptPeerAuthentication(receiverAuth, acceptAll);
    receiver.acceptPeerAuthentication(senderAuth, acceptAll);
    assertTrue(sender.isAuthenticated());
    assertTrue(receiver.isAuthenticated());

    final byte[] payment = ((String) vector.get("sender_plaintext_utf8")).getBytes(StandardCharsets.UTF_8);
    final IrohaPeerNearbyEncryptedRecordV1 paymentRecord = sender.seal(payment);
    assertArrayEquals(hex((String) vector.get("sender_record_hex")), paymentRecord.encode());
    assertArrayEquals(payment, receiver.open(IrohaPeerNearbyEncryptedRecordV1.decode(paymentRecord.encode())));
    assertThrows(IllegalArgumentException.class, () -> receiver.open(paymentRecord));

    final byte[] acknowledgement =
        ((String) vector.get("receiver_plaintext_utf8")).getBytes(StandardCharsets.UTF_8);
    final IrohaPeerNearbyEncryptedRecordV1 acknowledgementRecord = receiver.seal(acknowledgement);
    assertArrayEquals(hex((String) vector.get("receiver_record_hex")), acknowledgementRecord.encode());
    assertArrayEquals(acknowledgement, sender.open(acknowledgementRecord));
  }

  @Test
  public void repeatedHelloAndAuthenticationCannotResetSequence() {
    final byte[] session = repeat(16, 0x31);
    final byte[] request = repeat(32, 0x32);
    final IrohaPeerNearbySecureChannelV1 sender =
        new IrohaPeerNearbySecureChannelV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerNearbyRoleV1.SENDER,
            session,
            request,
            new byte[] {1},
            repeat(32, 0x33),
            scalar(5));
    final IrohaPeerNearbySecureChannelV1 receiver =
        new IrohaPeerNearbySecureChannelV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerNearbyRoleV1.RECEIVER,
            session,
            request,
            new byte[] {2},
            repeat(32, 0x34),
            scalar(6));
    sender.acceptPeerHello(receiver.localHello());
    receiver.acceptPeerHello(sender.localHello());
    final byte[] senderAuth = sender.makeAuthentication(new byte[] {0x41});
    final byte[] receiverAuth = receiver.makeAuthentication(new byte[] {0x42});
    sender.acceptPeerAuthentication(receiverAuth, (role, cert, signed, signature) -> true);
    receiver.acceptPeerAuthentication(senderAuth, (role, cert, signed, signature) -> true);
    assertTrue(sender.isAuthenticated());

    final byte[] firstMessage = message("first").encode();
    final byte[] first = sender.sealIpm1(firstMessage);
    assertArrayEquals(firstMessage, receiver.openIpm1(first));
    assertThrows(
        IllegalArgumentException.class,
        () -> sender.acceptPeerAuthentication(receiverAuth, (role, cert, signed, signature) -> true));
    final byte[] second = sender.sealIpm1(message("second").encode());
    assertEquals(1L, IrohaPeerNearbyEncryptedRecordV1.decode(second).getSequence());
  }

  @Test
  public void authenticationIsMandatoryAndVerifierFailureFailsClosed() {
    final IrohaPeerNearbySecureChannelV1 channel =
        new IrohaPeerNearbySecureChannelV1(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerNearbyRoleV1.SENDER,
            repeat(16, 1),
            repeat(32, 2),
            new byte[] {3});
    assertFalse(channel.isAuthenticated());
    assertThrows(IllegalStateException.class, () -> channel.sealIpm1(message("blocked").encode()));
  }

  @Test
  public void recordDecodersRejectTruncationTrailingBytesAndForgedLengths() {
    final byte[] session = repeat(16, 0x41);
    final byte[] request = repeat(32, 0x42);
    final byte[] hello =
        new IrohaPeerNearbyHelloV1(
                org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.OFFLINE_NOTE,
                org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyRoleV1.SENDER,
                session,
                repeat(32, 0x43),
                request,
                IrohaPeerNearbyP256V1.fromPrivateBytes(scalar(7)).getPublicKey(),
                repeat(32, 0x44))
            .encode();
    final byte[] authentication =
        new IrohaPeerNearbyAuthenticationV1(
                org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.OFFLINE_NOTE,
                org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyRoleV1.SENDER,
                session,
                repeat(32, 0x45),
                repeat(64, 0x46))
            .encode();
    final byte[] encrypted =
        new IrohaPeerNearbyEncryptedRecordV1(
                org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.OFFLINE_NOTE,
                org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyRoleV1.SENDER,
                session,
                -1L,
                repeat(48, 0x47))
            .encode();
    final byte[] discovery =
        new IrohaPeerNearbyDiscoveryContextV1(
                org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.OFFLINE_NOTE,
                org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyRoleV1.RECEIVER,
                session,
                request)
            .encode();

    for (int cut = 0; cut < hello.length; cut++) {
      final byte[] truncated = Arrays.copyOf(hello, cut);
      assertThrows(IllegalArgumentException.class, () -> IrohaPeerNearbyHelloV1.decode(truncated));
    }
    for (int cut = 0; cut < authentication.length; cut++) {
      final byte[] truncated = Arrays.copyOf(authentication, cut);
      assertThrows(
          IllegalArgumentException.class,
          () -> IrohaPeerNearbyAuthenticationV1.decode(truncated));
    }
    for (int cut = 0; cut < encrypted.length; cut++) {
      final byte[] truncated = Arrays.copyOf(encrypted, cut);
      assertThrows(
          IllegalArgumentException.class,
          () -> IrohaPeerNearbyEncryptedRecordV1.decode(truncated));
    }
    for (int cut = 0; cut < discovery.length; cut++) {
      final byte[] truncated = Arrays.copyOf(discovery, cut);
      assertThrows(
          IllegalArgumentException.class,
          () -> IrohaPeerNearbyDiscoveryContextV1.decode(truncated));
    }

    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerNearbyHelloV1.decode(append(hello, (byte) 0)));
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerNearbyAuthenticationV1.decode(append(authentication, (byte) 0)));
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerNearbyEncryptedRecordV1.decode(append(encrypted, (byte) 0)));
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerNearbyDiscoveryContextV1.decode(append(discovery, (byte) 0)));

    for (final int publicKeyLength : new int[] {0, 64, 66, 0xffff}) {
      final byte[] forged = hello.clone();
      writeU16(forged, 90, publicKeyLength);
      assertThrows(IllegalArgumentException.class, () -> IrohaPeerNearbyHelloV1.decode(forged));
    }
    final byte[] zeroCertificate = hello.clone();
    writeU32(zeroCertificate, 157, 0);
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerNearbyHelloV1.decode(zeroCertificate));
    final byte[] oversizedCertificate = hello.clone();
    writeU32(
        oversizedCertificate,
        157,
        org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyV1.MAXIMUM_CERTIFICATE_BYTES + 1);
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerNearbyHelloV1.decode(oversizedCertificate));
    final byte[] zeroSignature = authentication.clone();
    writeU16(zeroSignature, 58, 0);
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerNearbyAuthenticationV1.decode(zeroSignature));
    final byte[] oversizedCiphertext = encrypted.clone();
    writeU32(
        oversizedCiphertext,
        34,
        org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyV1.MAXIMUM_MESSAGE_BYTES + 17);
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerNearbyEncryptedRecordV1.decode(oversizedCiphertext));
  }

  @Test
  public void unsignedSequenceExtremesRoundTripAndReorderingDoesNotAdvanceState() {
    final byte[] session = repeat(16, 0x51);
    for (final long sequence :
        new long[] {0L, Long.MAX_VALUE, Long.MIN_VALUE, -2L, -1L}) {
      final IrohaPeerNearbyEncryptedRecordV1 record =
          new IrohaPeerNearbyEncryptedRecordV1(
              org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.OFFLINE_NOTE,
              org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyRoleV1.SENDER,
              session,
              sequence,
              repeat(16, 0x52));
      assertEquals(sequence, IrohaPeerNearbyEncryptedRecordV1.decode(record.encode()).getSequence());
    }

    final byte[] request = repeat(32, 0x53);
    final IrohaPeerNearbySessionV1 sender =
        sharedSession(
            org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyRoleV1.SENDER,
            session,
            request,
            new byte[] {1},
            repeat(32, 0x54),
            8);
    final IrohaPeerNearbySessionV1 receiver =
        sharedSession(
            org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyRoleV1.RECEIVER,
            session,
            request,
            new byte[] {2},
            repeat(32, 0x55),
            9);
    sender.acceptPeerHello(receiver.getLocalHello());
    receiver.acceptPeerHello(sender.getLocalHello());
    final IrohaPeerNearbyAuthenticationV1 senderAuth = sender.makeAuthentication(new byte[] {3});
    final IrohaPeerNearbyAuthenticationV1 receiverAuth = receiver.makeAuthentication(new byte[] {4});
    final IrohaPeerNearbySignatureVerifierV1 acceptAll = (role, cert, signed, signature) -> true;
    sender.acceptPeerAuthentication(receiverAuth, acceptAll);
    receiver.acceptPeerAuthentication(senderAuth, acceptAll);
    final IrohaPeerNearbyEncryptedRecordV1 first = sender.seal("first".getBytes(StandardCharsets.UTF_8));
    final IrohaPeerNearbyEncryptedRecordV1 second = sender.seal("second".getBytes(StandardCharsets.UTF_8));
    assertThrows(IllegalArgumentException.class, () -> receiver.open(second));
    assertArrayEquals("first".getBytes(StandardCharsets.UTF_8), receiver.open(first));
    assertArrayEquals("second".getBytes(StandardCharsets.UTF_8), receiver.open(second));
  }

  @Test
  public void p256PublicKeyAccessReturnsDefensiveCopies() {
    final IrohaPeerNearbyP256V1 key = IrohaPeerNearbyP256V1.fromPrivateBytes(scalar(12));
    final byte[] expected = key.getPublicKey();
    final byte[] mutated = key.getPublicKey();
    Arrays.fill(mutated, (byte) 0);
    assertArrayEquals(expected, key.getPublicKey());
  }

  private static IrohaPeerNearbySessionV1 sharedSession(
      final org.hyperledger.iroha.sdk.offline.IrohaPeerNearbyRoleV1 role,
      final byte[] session,
      final byte[] request,
      final byte[] certificate,
      final byte[] nonce,
      final int scalar) {
    return new IrohaPeerNearbySessionV1(
        org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.OFFLINE_NOTE,
        role,
        session,
        request,
        certificate,
        nonce,
        IrohaPeerNearbyP256V1.fromPrivateBytes(scalar(scalar)));
  }

  private static IrohaPeerWireMessageV1 message(final String value) {
    return new IrohaPeerWireMessageV1(
        new IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            IrohaPeerPayloadKind.PAYMENT,
            1,
            value.getBytes(StandardCharsets.UTF_8)));
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> fixture() throws Exception {
    return (Map<String, Object>)
        JsonParser.parse(new String(Files.readAllBytes(sharedFixture()), StandardCharsets.UTF_8));
  }

  private static Path sharedFixture() {
    Path current = Paths.get("").toAbsolutePath();
    while (current != null) {
      final Path candidate = current.resolve("fixtures/offline/peer_nearby_v1.json");
      if (Files.isRegularFile(candidate)) return candidate;
      current = current.getParent();
    }
    throw new AssertionError("peer_nearby_v1.json was not found");
  }

  private static byte[] scalar(final int value) {
    final byte[] bytes = new byte[32];
    bytes[31] = (byte) value;
    return bytes;
  }

  private static byte[] repeat(final int count, final int value) {
    final byte[] bytes = new byte[count];
    Arrays.fill(bytes, (byte) value);
    return bytes;
  }

  private static byte[] ascending(final int count) {
    final byte[] bytes = new byte[count];
    for (int index = 0; index < count; index++) bytes[index] = (byte) index;
    return bytes;
  }

  private static byte[] append(final byte[] value, final byte suffix) {
    final byte[] result = Arrays.copyOf(value, value.length + 1);
    result[value.length] = suffix;
    return result;
  }

  private static void writeU16(final byte[] value, final int offset, final int number) {
    value[offset] = (byte) (number >>> 8);
    value[offset + 1] = (byte) number;
  }

  private static void writeU32(final byte[] value, final int offset, final int number) {
    value[offset] = (byte) (number >>> 24);
    value[offset + 1] = (byte) (number >>> 16);
    value[offset + 2] = (byte) (number >>> 8);
    value[offset + 3] = (byte) number;
  }

  private static byte[] hex(final String value) {
    final byte[] bytes = new byte[value.length() / 2];
    for (int index = 0; index < bytes.length; index++) {
      bytes[index] = (byte) Integer.parseInt(value.substring(index * 2, index * 2 + 2), 16);
    }
    return bytes;
  }

  private static String toHex(final byte[] value) {
    final StringBuilder out = new StringBuilder(value.length * 2);
    for (final byte element : value) out.append(String.format("%02x", element & 0xff));
    return out.toString();
  }
}
