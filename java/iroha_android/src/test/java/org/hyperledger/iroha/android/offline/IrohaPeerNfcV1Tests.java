package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcCommandTypeV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcCommandV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcCommitDispositionV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcDurableAcknowledgementV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcLimitsV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcPhaseV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcProfilePolicyV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReaderExchangeResultV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReaderPlanningV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReaderResponseV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReceiverSessionV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcSenderActionV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcSenderCheckpointV1;
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
  public void receiverCommitRemainsUnsuccessfulUntilExactDurableRecordIsInstalled() throws Exception {
    final byte[] session = ascending(16, 1);
    final Messages messages = currentMessages();
    final IrohaPeerNfcProfilePolicyV1 policy = kagemushaPolicy();
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
        org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
        receiver.status().getPaymentProfile());
    assertEquals(
        org.hyperledger.iroha.sdk.offline.IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
        receiver.status().getAcknowledgementProfile());
  }

  @Test
  public void readerAndTwoTapReducerIntersectAsymmetricChunkLimitsBothWays() {
    for (final int[] pair : List.of(new int[] {240, 4_096}, new int[] {4_096, 240})) {
      final int localChunk = pair[0];
      final int remoteChunk = pair[1];
      final int expected = Math.min(localChunk, remoteChunk);
      final byte[] session = ascending(16, 1);
      final IrohaPeerWireMessageV1 request =
          message(IrohaPeerPayloadKind.RECEIVE_REQUEST, 0x61, 900);
      final IrohaPeerWireMessageV1 payment =
          message(IrohaPeerPayloadKind.PAYMENT, 0x62, 1_100);
      final IrohaPeerWireMessageV1 acknowledgement =
          message(IrohaPeerPayloadKind.ACKNOWLEDGEMENT, 0x63, 700);
      final IrohaPeerNfcLimitsV1 local =
          new IrohaPeerNfcLimitsV1(MAXIMUM_MESSAGE_BYTES, localChunk, localChunk);
      final IrohaPeerNfcLimitsV1 remote =
          new IrohaPeerNfcLimitsV1(MAXIMUM_MESSAGE_BYTES, remoteChunk, remoteChunk);
      final IrohaPeerNfcProfilePolicyV1 policy = kagemushaPolicy();
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
    final byte[] session = ascending(16, 1);
    final Messages messages = currentMessages();
    final IrohaPeerNfcProfilePolicyV1 policy = kagemushaPolicy();
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

  private static IrohaPeerNfcProfilePolicyV1 kagemushaPolicy() {
    return IrohaPeerNfcV1.profilePolicy(IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND);
  }

  private static Messages currentMessages() {
    return new Messages(
        message(IrohaPeerPayloadKind.RECEIVE_REQUEST, 0x31, 900),
        message(IrohaPeerPayloadKind.PAYMENT, 0x32, 1_100),
        message(IrohaPeerPayloadKind.ACKNOWLEDGEMENT, 0x33, 700));
  }

  private static IrohaPeerWireMessageV1 message(
      final IrohaPeerPayloadKind kind, final int repeated, final int count) {
    return IrohaPeerKagemushaStructuralTestV1.message(kind, repeat(count, repeated));
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

  private record Messages(
      IrohaPeerWireMessageV1 request,
      IrohaPeerWireMessageV1 payment,
      IrohaPeerWireMessageV1 acknowledgement) {}
}
