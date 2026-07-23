package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.crypto.Blake2b;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcAPDUCodecV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcCommandTypeV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcCommandV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcCommitDispositionV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcDurableAcknowledgementV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcDurablePaymentAdmissionV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcFlagsV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcInfoV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcLimitsV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcPhaseV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcPaymentAdmissionContextV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcProfilePolicyV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReaderExchangeResultV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReaderPlanningV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReaderResponseV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReceiverSessionV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcRequestIdentityV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcSenderActionV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcSenderCheckpointV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcStatusV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcTwoTapReducerV1;
import org.junit.Test;

public final class IrohaPeerNfcV1Tests {
  @Test
  public void messageLimitCannotExceedPortableV1Maximum() {
    IrohaPeerNfcV1.limits(
        IrohaPeerNfcV1.MAXIMUM_MESSAGE_BYTES,
        IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES,
        IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES);
    assertThrows(IllegalArgumentException.class, () -> IrohaPeerNfcV1.limits(
        IrohaPeerNfcV1.MAXIMUM_MESSAGE_BYTES + 1,
        IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES,
        IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES));
  }

  private static final int MAXIMUM_MESSAGE_BYTES = 84 + 24_576;

  @Test
  @SuppressWarnings("unchecked")
  public void matchesSharedInf1Nst1AndShortExtendedApduVectors() throws Exception {
    final Map<String, Object> fixture = fixture();
    final byte[] session = hexValue(fixture.get("session_hex"));
    assertEquals(fixture.get("aid_hex"), IrohaPeerNfcV1.APPLICATION_IDENTIFIER_HEX);
    assertArrayEquals(hexValue(fixture.get("aid_hex")), IrohaPeerNfcV1.applicationIdentifier());

    final IrohaPeerNfcRequestIdentityV1 identity =
        new IrohaPeerNfcRequestIdentityV1(
            org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.OFFLINE_NOTE,
            session,
            repeat(32, 0x11),
            repeat(32, 0x22));
    final IrohaPeerNfcInfoV1 info =
        new IrohaPeerNfcInfoV1(
            IrohaPeerNfcPhaseV1.REQUEST_READY,
            IrohaPeerNfcFlagsV1.REQUEST,
            identity,
            300,
            240,
            240);
    assertArrayEquals(hexValue(fixture.get("info_hex")), info.encode());
    assertEquals(info, IrohaPeerNfcV1.decodeInfo(info.encode()));

    final IrohaPeerNfcStatusV1 status =
        new IrohaPeerNfcStatusV1(
            IrohaPeerNfcPhaseV1.ACKNOWLEDGEMENT_READY,
            IrohaPeerNfcFlagsV1.DURABLE,
            identity,
            org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.OFFLINE_NOTE,
            700,
            700,
            repeat(32, 0x33),
            org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.OFFLINE_NOTE,
            270,
            repeat(32, 0x44),
            240,
            240);
    assertArrayEquals(hexValue(fixture.get("ack_ready_status_hex")), status.encode());
    assertEquals(status, IrohaPeerNfcV1.decodeStatus(status.encode()));

    final Messages messages = messages(fixture);
    final Map<String, Object> apdu = (Map<String, Object>) fixture.get("apdu_hex");
    final Map<String, IrohaPeerNfcCommandV1> commands = new LinkedHashMap<>();
    commands.put("select", IrohaPeerNfcCommandV1.SELECT_APPLICATION);
    commands.put("get_info", IrohaPeerNfcCommandV1.GET_INFO);
    commands.put(
        "read_request",
        IrohaPeerNfcCommandV1.readRequest(
            session, messages.request.canonicalHash(), 0x0102_0304L, 240));
    commands.put(
        "begin_payment",
        IrohaPeerNfcCommandV1.beginPayment(
            session,
            messages.request.canonicalHash(),
            Arrays.copyOf(messages.payment.encode(), IrohaPeerWireMessageV1.HEADER_LENGTH)));
    commands.put(
        "write_300",
        IrohaPeerNfcCommandV1.write(
            session, messages.payment.wireHash(), 0x0102_0304L, repeat(300, 0x55)));
    commands.put(
        "commit",
        IrohaPeerNfcCommandV1.commit(
            session, messages.request.canonicalHash(), messages.payment.wireHash()));
    commands.put(
        "read_ack_1024",
        IrohaPeerNfcCommandV1.readAcknowledgement(
            session, messages.payment.wireHash(), 0x0102_0304L, 1_024));
    commands.put(
        "confirm_ack",
        IrohaPeerNfcCommandV1.confirmAcknowledgement(
            session, messages.payment.wireHash(), messages.acknowledgement.wireHash()));
    commands.put(
        "get_status",
        IrohaPeerNfcCommandV1.getStatus(session, messages.request.canonicalHash()));

    for (final Map.Entry<String, IrohaPeerNfcCommandV1> entry : commands.entrySet()) {
      final byte[] encoded = IrohaPeerNfcV1.encodeCommand(entry.getValue());
      assertArrayEquals(entry.getKey(), hexValue(apdu.get(entry.getKey())), encoded);
      assertEquals(entry.getKey(), entry.getValue(), IrohaPeerNfcV1.decodeCommand(encoded));
    }
    assertTrue(IrohaPeerNfcV1.encodeCommand(commands.get("write_300")).length > 255);
  }

  @Test
  @SuppressWarnings("unchecked")
  public void matchesAllRetailIda1AndIsc1Fixtures() throws Exception {
    final Map<String, Object> fixture = fixture();
    final byte[] session = hexValue(fixture.get("session_hex"));
    final Messages messages = messages(fixture);
    final IrohaPeerNfcProfilePolicyV1 policy = retailPolicy();
    final IrohaPeerNfcLimitsV1 limits = defaultLimits();
    final IrohaPeerNfcSenderCheckpointV1 checkpoint =
        IrohaPeerNfcV1.senderCheckpoint(
            session,
            messages.request.encode(),
            messages.payment.encode(),
            null,
            policy,
            limits);
    final Map<String, Object> checkpointFixture = (Map<String, Object>) fixture.get("checkpoint");
    assertEquals(((Number) checkpointFixture.get("without_ack_length")).intValue(), checkpoint.encode().length);
    assertEquals(checkpointFixture.get("without_ack_blake2b_256_hex"), toHex(Blake2b.digest256(checkpoint.encode())));
    assertEquals(checkpoint, IrohaPeerNfcSenderCheckpointV1.decode(checkpoint.encode(), policy, limits));
    assertEquals(checkpoint, IrohaPeerNfcSenderCheckpointV1.decode(checkpoint.encode()));

    final IrohaPeerNfcReceiverSessionV1 receiver =
        IrohaPeerNfcV1.receiver(session, messages.request.encode(), null, policy, limits);
    final byte[] paymentHeader =
        Arrays.copyOf(messages.payment.encode(), IrohaPeerWireMessageV1.HEADER_LENGTH);
    final IrohaPeerNfcPaymentAdmissionContextV1 admissionContext =
        IrohaPeerNfcV1.paymentAdmissionContext(receiver, paymentHeader);
    final IrohaPeerNfcDurablePaymentAdmissionV1 admission =
        IrohaPeerNfcV1.durablePaymentAdmission(admissionContext, limits);
    final Map<String, Object> admissionFixture =
        (Map<String, Object>) fixture.get("payment_admission");
    assertEquals(((Number) admissionFixture.get("length")).intValue(), admission.encode().length);
    assertArrayEquals(hexValue(admissionFixture.get("encoded_hex")), admission.encode());
    assertEquals(
        admissionFixture.get("blake2b_256_hex"),
        toHex(Blake2b.digest256(admission.encode())));
    assertEquals(
        admission,
        IrohaPeerNfcV1.decodePaymentAdmission(admission.encode(), policy, limits));
    receiver.installPaymentAdmission(admission);
    writeAll(receiver, session, messages.payment, 113);
    final IrohaPeerNfcCommitDispositionV1.RequiresDurableCommit required =
        (IrohaPeerNfcCommitDispositionV1.RequiresDurableCommit)
            receiver.prepareCommit(
                IrohaPeerNfcCommandV1.commit(
                    session, messages.request.canonicalHash(), messages.payment.wireHash()));
    final IrohaPeerNfcDurableAcknowledgementV1 durable =
        IrohaPeerNfcV1.durableAcknowledgement(
            required.getContext(), messages.acknowledgement.encode(), limits);
    final Map<String, Object> durableFixture = (Map<String, Object>) fixture.get("durable_ack");
    assertEquals(((Number) durableFixture.get("length")).intValue(), durable.encode().length);
    assertEquals(durableFixture.get("blake2b_256_hex"), toHex(Blake2b.digest256(durable.encode())));
    assertEquals(
        durable,
        IrohaPeerNfcDurableAcknowledgementV1.decode(durable.encode(), policy, limits));
    assertEquals(durable, IrohaPeerNfcDurableAcknowledgementV1.decode(durable.encode()));

    final IrohaPeerNfcSenderCheckpointV1 checkpointWithAck =
        IrohaPeerNfcV1.senderCheckpoint(
            session,
            messages.request.encode(),
            messages.payment.encode(),
            messages.acknowledgement.encode(),
            policy,
            limits);
    assertEquals(((Number) checkpointFixture.get("with_ack_length")).intValue(), checkpointWithAck.encode().length);
    assertEquals(checkpointFixture.get("with_ack_blake2b_256_hex"), toHex(Blake2b.digest256(checkpointWithAck.encode())));
  }

  @Test
  public void receiverCommitRemainsUnsuccessfulUntilExactDurableRecordIsInstalled() throws Exception {
    final Map<String, Object> fixture = fixture();
    final byte[] session = hexValue(fixture.get("session_hex"));
    final Messages messages = messages(fixture);
    final IrohaPeerNfcProfilePolicyV1 policy = retailPolicy();
    final IrohaPeerNfcLimitsV1 limits = new IrohaPeerNfcLimitsV1(MAXIMUM_MESSAGE_BYTES, 97, 113);
    final IrohaPeerNfcReceiverSessionV1 receiver =
        IrohaPeerNfcV1.receiver(session, messages.request.encode(), null, policy, limits);
    admit(
        receiver,
        IrohaPeerNfcCommandV1.beginPayment(
            session,
            messages.request.canonicalHash(),
            Arrays.copyOf(messages.payment.encode(), IrohaPeerWireMessageV1.HEADER_LENGTH)));
    final byte[] payment = messages.payment.encode();
    receiver.handle(
        IrohaPeerNfcCommandV1.write(
            session, messages.payment.wireHash(), 0, Arrays.copyOfRange(payment, 0, 100)));
    receiver.handle(
        IrohaPeerNfcCommandV1.write(
            session, messages.payment.wireHash(), 50, Arrays.copyOfRange(payment, 50, 150)));
    assertEquals(150, receiver.status().getReceivedPaymentBytes());
    final byte[] conflicting = Arrays.copyOfRange(payment, 50, 100);
    conflicting[0] ^= 1;
    assertThrows(
        IllegalArgumentException.class,
        () ->
            receiver.handle(
                IrohaPeerNfcCommandV1.write(
                    session, messages.payment.wireHash(), 50, conflicting)));
    int offset = 150;
    while (offset < payment.length) {
      final int end = Math.min(offset + 113, payment.length);
      receiver.handle(
          IrohaPeerNfcCommandV1.write(
              session,
              messages.payment.wireHash(),
              offset,
              Arrays.copyOfRange(payment, offset, end)));
      offset = end;
    }
    final IrohaPeerNfcCommandV1 commit =
        IrohaPeerNfcCommandV1.commit(
            session, messages.request.canonicalHash(), messages.payment.wireHash());
    final IrohaPeerNfcCommitDispositionV1.RequiresDurableCommit required =
        (IrohaPeerNfcCommitDispositionV1.RequiresDurableCommit) receiver.prepareCommit(commit);
    assertEquals(IrohaPeerNfcPhaseV1.PAYMENT_RECEIVING, receiver.getPhase());
    assertThrows(
        IllegalStateException.class,
        () ->
            receiver.handle(
                IrohaPeerNfcCommandV1.readAcknowledgement(
                    session, messages.payment.wireHash(), 0, 32)));
    final IrohaPeerNfcDurableAcknowledgementV1 durable =
        IrohaPeerNfcV1.durableAcknowledgement(
            required.getContext(), messages.acknowledgement.encode(), limits);
    receiver.installDurableAcknowledgement(durable);
    receiver.installDurableAcknowledgement(durable);
    assertTrue(receiver.prepareCommit(commit) instanceof IrohaPeerNfcCommitDispositionV1.AlreadyCommitted);
    assertEquals(IrohaPeerNfcPhaseV1.ACKNOWLEDGEMENT_READY, receiver.status().getPhase());
    assertEquals(
        org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.OFFLINE_NOTE,
        receiver.status().getPaymentProfile());
    assertEquals(
        org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.OFFLINE_NOTE,
        receiver.status().getAcknowledgementProfile());
  }

  @Test
  public void readerAndTwoTapReducerIntersectAsymmetricChunkLimitsBothWays() {
    for (final int[] pair : List.of(new int[] {240, 4_096}, new int[] {4_096, 240})) {
      final int localChunk = pair[0];
      final int remoteChunk = pair[1];
      final int expected = Math.min(localChunk, remoteChunk);
      final byte[] session = ascending(16, 1);
      final IrohaPeerWireMessageV1 request = message(1, 1, 1, 0x61, 900);
      final IrohaPeerWireMessageV1 payment = message(1, 2, 1, 0x62, 1_100);
      final IrohaPeerWireMessageV1 acknowledgement = message(1, 3, 1, 0x63, 700);
      final IrohaPeerNfcLimitsV1 local =
          new IrohaPeerNfcLimitsV1(MAXIMUM_MESSAGE_BYTES, localChunk, localChunk);
      final IrohaPeerNfcLimitsV1 remote =
          new IrohaPeerNfcLimitsV1(MAXIMUM_MESSAGE_BYTES, remoteChunk, remoteChunk);
      final IrohaPeerNfcProfilePolicyV1 policy = retailPolicy();
      final IrohaPeerNfcReceiverSessionV1 receiver =
          IrohaPeerNfcV1.receiver(session, request.encode(), null, policy, remote);
      assertEquals(
          expected,
          IrohaPeerNfcReaderPlanningV1.readRequestCommand(receiver.info(), 0, local).getLength());
      final IrohaPeerNfcSenderCheckpointV1 checkpoint =
          IrohaPeerNfcV1.senderCheckpoint(
              session, request.encode(), payment.encode(), null, policy, remote);
      final IrohaPeerNfcTwoTapReducerV1 reducer = IrohaPeerNfcV1.twoTapReducer(checkpoint, local);
      final IrohaPeerNfcSenderActionV1.Send begin =
          (IrohaPeerNfcSenderActionV1.Send) reducer.nextAction(receiver.status());
      admit(receiver, begin.getCommand());
      final IrohaPeerNfcSenderActionV1.Send write =
          (IrohaPeerNfcSenderActionV1.Send) reducer.nextAction(receiver.status());
      assertEquals(IrohaPeerNfcCommandTypeV1.WRITE, write.getCommand().getType());
      assertEquals(expected, write.getCommand().getBytes().length);
      receiver.handle(write.getCommand());

      while (receiver.status().getReceivedPaymentBytes() < payment.encode().length) {
        final IrohaPeerNfcSenderActionV1.Send next =
            (IrohaPeerNfcSenderActionV1.Send) reducer.nextAction(receiver.status());
        receiver.handle(next.getCommand());
      }
      final IrohaPeerNfcSenderActionV1.Send commit =
          (IrohaPeerNfcSenderActionV1.Send) reducer.nextAction(receiver.status());
      final IrohaPeerNfcCommitDispositionV1.RequiresDurableCommit required =
          (IrohaPeerNfcCommitDispositionV1.RequiresDurableCommit)
              receiver.prepareCommit(commit.getCommand());
      receiver.installDurableAcknowledgement(
          IrohaPeerNfcV1.durableAcknowledgement(
              required.getContext(), acknowledgement.encode(), remote));
      final IrohaPeerNfcSenderActionV1.Send readAck =
          (IrohaPeerNfcSenderActionV1.Send) reducer.nextAction(receiver.status());
      assertEquals(IrohaPeerNfcCommandTypeV1.READ_ACKNOWLEDGEMENT, readAck.getCommand().getType());
      assertEquals(expected, readAck.getCommand().getLength());
    }
  }

  @Test
  public void javaFacadeRunsSharedDurableReaderWithoutDuplicatingStateMachine() throws Exception {
    final Map<String, Object> fixture = fixture();
    final byte[] session = hexValue(fixture.get("session_hex"));
    final Messages messages = messages(fixture);
    final IrohaPeerNfcProfilePolicyV1 policy = retailPolicy();
    final IrohaPeerNfcLimitsV1 limits =
        new IrohaPeerNfcLimitsV1(MAXIMUM_MESSAGE_BYTES, 240, 203);
    final IrohaPeerNfcReceiverSessionV1 receiver =
        IrohaPeerNfcV1.receiver(session, messages.request.encode(), null, policy, limits);
    final List<IrohaPeerNfcCommandV1> commands = new ArrayList<>();
    final List<byte[]> persisted = new ArrayList<>();

    final IrohaPeerNfcReaderExchangeResultV1 result =
        IrohaPeerNfcV1.runReaderExchange(
            policy,
            limits,
            command -> {
              commands.add(command);
              final byte[] data;
              if (command.getType() == IrohaPeerNfcCommandTypeV1.BEGIN_PAYMENT) {
                final org.hyperledger.iroha.sdk.offline.IrohaPeerNfcPaymentAdmissionDispositionV1
                    disposition = receiver.preparePaymentAdmission(command);
                if (disposition instanceof
                    org.hyperledger.iroha.sdk.offline.IrohaPeerNfcPaymentAdmissionDispositionV1
                        .RequiresDurableAdmission required) {
                  receiver.installPaymentAdmission(
                      IrohaPeerNfcV1.durablePaymentAdmission(required.getContext(), limits));
                }
                data = new byte[0];
              } else if (command.getType() == IrohaPeerNfcCommandTypeV1.COMMIT) {
                final IrohaPeerNfcCommitDispositionV1 disposition = receiver.prepareCommit(command);
                if (disposition instanceof IrohaPeerNfcCommitDispositionV1.RequiresDurableCommit required) {
                  receiver.installDurableAcknowledgement(
                      IrohaPeerNfcV1.durableAcknowledgement(
                          required.getContext(), messages.acknowledgement.encode(), limits));
                }
                data = new byte[0];
              } else {
                data = receiver.handle(command);
              }
              return IrohaPeerNfcReaderResponseV1.success(data);
            },
            (info, request) -> {
              final IrohaPeerNfcSenderCheckpointV1 checkpoint =
                  IrohaPeerNfcV1.senderCheckpoint(
                      info.getIdentity().getSessionId(),
                      request.encode(),
                      messages.payment.encode(),
                      null,
                      policy,
                      limits);
              // loadOrCreateDurableCheckpoint returns only after this exact
              // payment-bearing ISC1 has crossed the durable boundary.
              persisted.add(checkpoint.encode());
              return checkpoint;
            },
            checkpoint -> persisted.add(checkpoint.clone()));

    assertArrayEquals(messages.acknowledgement.encode(), result.getAcknowledgement().encode());
    assertEquals(2, persisted.size());
    assertEquals(
        IrohaPeerNfcCommandTypeV1.CONFIRM_ACKNOWLEDGEMENT,
        commands.get(commands.size() - 1).getType());
    for (final IrohaPeerNfcCommandV1 command : commands) {
      if (command.getType() != IrohaPeerNfcCommandTypeV1.WRITE) continue;
      assertTrue(command.getBytes().length <= 203);
      assertTrue((IrohaPeerNfcV1.encodeCommand(command)[4] & 0xff) != 0);
    }
  }

  private static void writeAll(
      final IrohaPeerNfcReceiverSessionV1 receiver,
      final byte[] session,
      final IrohaPeerWireMessageV1 payment,
      final int chunk) {
    final byte[] bytes = payment.encode();
    int offset = 0;
    while (offset < bytes.length) {
      final int end = Math.min(offset + chunk, bytes.length);
      receiver.handle(
          IrohaPeerNfcCommandV1.write(
              session, payment.wireHash(), offset, Arrays.copyOfRange(bytes, offset, end)));
      offset = end;
    }
  }

  private static void admit(
      final IrohaPeerNfcReceiverSessionV1 receiver,
      final IrohaPeerNfcCommandV1 begin) {
    final org.hyperledger.iroha.sdk.offline.IrohaPeerNfcPaymentAdmissionDispositionV1
            .RequiresDurableAdmission required =
        (org.hyperledger.iroha.sdk.offline.IrohaPeerNfcPaymentAdmissionDispositionV1
            .RequiresDurableAdmission) receiver.preparePaymentAdmission(begin);
    receiver.installPaymentAdmission(
        IrohaPeerNfcV1.durablePaymentAdmission(required.getContext(), receiver.getLimits()));
  }

  private static IrohaPeerNfcProfilePolicyV1 retailPolicy() {
    return IrohaPeerNfcV1.profilePolicy(IrohaPeerPayloadProfile.OFFLINE_NOTE);
  }

  private static IrohaPeerNfcLimitsV1 defaultLimits() {
    return new IrohaPeerNfcLimitsV1(MAXIMUM_MESSAGE_BYTES, 4_096, 4_096);
  }

  @SuppressWarnings("unchecked")
  private static Messages messages(final Map<String, Object> fixture) {
    final Map<String, Object> values = (Map<String, Object>) fixture.get("messages");
    return new Messages(
        vector((Map<String, Object>) values.get("request")),
        vector((Map<String, Object>) values.get("payment")),
        vector((Map<String, Object>) values.get("acknowledgement")));
  }

  private static IrohaPeerWireMessageV1 vector(final Map<String, Object> vector) {
    final IrohaPeerWireMessageV1 message =
        message(
            ((Number) vector.get("profile")).intValue(),
            ((Number) vector.get("kind")).intValue(),
            ((Number) vector.get("schema_version")).intValue(),
            ((Number) vector.get("repeat_byte")).intValue(),
            ((Number) vector.get("count")).intValue());
    assertEquals(vector.get("wire_hash_hex"), toHex(message.wireHash()));
    return message;
  }

  private static IrohaPeerWireMessageV1 message(
      final int profile,
      final int kind,
      final int schemaVersion,
      final int repeated,
      final int count) {
    return new IrohaPeerWireMessageV1(
        new IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.fromCode(profile),
            IrohaPeerPayloadKind.fromCode(kind),
            schemaVersion,
            repeat(count, repeated)));
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> fixture() throws Exception {
    return (Map<String, Object>)
        JsonParser.parse(new String(Files.readAllBytes(sharedFixture()), StandardCharsets.UTF_8));
  }

  private static Path sharedFixture() {
    Path current = Paths.get("").toAbsolutePath();
    while (current != null) {
      final Path candidate = current.resolve("fixtures/offline/peer_nfc_v1.json");
      if (Files.isRegularFile(candidate)) return candidate;
      current = current.getParent();
    }
    throw new AssertionError("peer_nfc_v1.json was not found");
  }

  private static byte[] hexValue(final Object value) {
    final String encoded;
    if (value instanceof List<?> pieces) {
      final StringBuilder joined = new StringBuilder();
      for (final Object piece : pieces) joined.append((String) piece);
      encoded = joined.toString();
    } else {
      encoded = (String) value;
    }
    final byte[] bytes = new byte[encoded.length() / 2];
    for (int index = 0; index < bytes.length; index++) {
      bytes[index] = (byte) Integer.parseInt(encoded.substring(index * 2, index * 2 + 2), 16);
    }
    return bytes;
  }

  private static byte[] repeat(final int count, final int value) {
    final byte[] bytes = new byte[count];
    Arrays.fill(bytes, (byte) value);
    return bytes;
  }

  private static byte[] ascending(final int count, final int first) {
    final byte[] bytes = new byte[count];
    for (int index = 0; index < count; index++) bytes[index] = (byte) (first + index);
    return bytes;
  }

  private static String toHex(final byte[] value) {
    final StringBuilder out = new StringBuilder(value.length * 2);
    for (final byte element : value) out.append(String.format("%02x", element & 0xff));
    return out.toString();
  }

  private record Messages(
      IrohaPeerWireMessageV1 request,
      IrohaPeerWireMessageV1 payment,
      IrohaPeerWireMessageV1 acknowledgement) {}
}
