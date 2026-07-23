package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertNull;
import static org.junit.Assert.assertThrows;

import java.util.Arrays;
import java.util.concurrent.atomic.AtomicReference;
import org.hyperledger.iroha.sdk.offline.IrohaPeerIsoDepLimitsV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcApduResponseV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcCommandV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcCommitContextV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcDurableAcknowledgementV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcDurableCommitCompletionV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcLimitsV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcProfilePolicyV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReceiverApduBridgeV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReceiverSessionV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcStatusWordV1;
import org.junit.Test;

public final class IrohaPeerAndroidTransportV1Tests {
  @Test
  public void rawResponseCannotExceedProtocolChunkBound() {
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerNfcApduResponseV1.decode(
            new byte[org.hyperledger.iroha.sdk.offline.IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES + 3]));
  }

  @Test
  public void isoDepLimitsRespectShortAndExtendedApduBounds() {
    final IrohaPeerNfcLimitsV1 shortLimits = IrohaPeerIsoDepLimitsV1.derive(261, false);
    assertEquals(256, shortLimits.getMaximumReadChunkBytes());
    assertEquals(203, shortLimits.getMaximumWriteChunkBytes());

    final IrohaPeerNfcLimitsV1 extended = IrohaPeerIsoDepLimitsV1.derive(5_000, true);
    assertEquals(4_096, extended.getMaximumReadChunkBytes());
    assertEquals(4_096, extended.getMaximumWriteChunkBytes());
  }

  @Test
  public void commitResponseWaitsForExactDurableAcknowledgement() {
    final byte[] session = repeat(16, 0x21);
    final IrohaPeerWireMessageV1 request = message(IrohaPeerPayloadKind.RECEIVE_REQUEST, 0x31, 120);
    final IrohaPeerWireMessageV1 payment = message(IrohaPeerPayloadKind.PAYMENT, 0x32, 360);
    final IrohaPeerWireMessageV1 acknowledgement =
        message(IrohaPeerPayloadKind.ACKNOWLEDGEMENT, 0x33, 140);
    // Deliberately tighter caller policy; this is not the canonical 24,660-byte ceiling.
    final IrohaPeerNfcLimitsV1 limits = new IrohaPeerNfcLimitsV1(84 + 12_288, 240, 240);
    final IrohaPeerNfcProfilePolicyV1 policy =
        IrohaPeerNfcV1.profilePolicy(IrohaPeerPayloadProfile.OFFLINE_NOTE);
    final IrohaPeerNfcReceiverSessionV1 receiver =
        IrohaPeerNfcV1.receiver(session, request.encode(), null, policy, limits);
    admit(
        receiver,
        IrohaPeerNfcCommandV1.beginPayment(
            session,
            request.canonicalHash(),
            Arrays.copyOf(payment.encode(), IrohaPeerWireMessageV1.HEADER_LENGTH)));
    final byte[] encodedPayment = payment.encode();
    int offset = 0;
    while (offset < encodedPayment.length) {
      final int end = Math.min(offset + 240, encodedPayment.length);
      receiver.handle(
          IrohaPeerNfcCommandV1.write(
              session, payment.wireHash(), offset, Arrays.copyOfRange(encodedPayment, offset, end)));
      offset = end;
    }

    final AtomicReference<IrohaPeerNfcCommitContextV1> pendingContext = new AtomicReference<>();
    final AtomicReference<IrohaPeerNfcDurableCommitCompletionV1> pendingCompletion =
        new AtomicReference<>();
    final IrohaPeerNfcReceiverApduBridgeV1 bridge =
        IrohaPeerAndroidNfcV1.receiverBridge(
            receiver,
            (context, completion) -> {
              throw new AssertionError("BEGIN is already durable");
            },
            (context, completion) -> {
              pendingContext.set(context);
              pendingCompletion.set(completion);
            });
    final AtomicReference<IrohaPeerNfcApduResponseV1> response = new AtomicReference<>();
    bridge.handle(
        IrohaPeerNfcCommandV1.commit(
            session, request.canonicalHash(), payment.wireHash()),
        response::set);
    assertNull(response.get());
    assertNotNull(pendingContext.get());
    assertNotNull(pendingCompletion.get());

    final IrohaPeerNfcDurableAcknowledgementV1 durable =
        IrohaPeerNfcV1.durableAcknowledgement(
            pendingContext.get(), acknowledgement.encode(), limits);
    pendingCompletion.get().complete(durable, null);
    assertEquals(IrohaPeerNfcStatusWordV1.SUCCESS, response.get().getStatusWord());
  }

  @Test
  public void storageFailureNeverReturnsSuccessOrInstallsAcknowledgement() {
    final byte[] session = repeat(16, 0x41);
    final IrohaPeerWireMessageV1 request = message(IrohaPeerPayloadKind.RECEIVE_REQUEST, 0x51, 20);
    final IrohaPeerWireMessageV1 payment = message(IrohaPeerPayloadKind.PAYMENT, 0x52, 20);
    // Deliberately tighter caller policy; this is not the canonical 24,660-byte ceiling.
    final IrohaPeerNfcLimitsV1 limits = new IrohaPeerNfcLimitsV1(84 + 12_288, 240, 240);
    final IrohaPeerNfcProfilePolicyV1 policy = IrohaPeerNfcV1.profilePolicy(
        IrohaPeerPayloadProfile.OFFLINE_NOTE);
    final IrohaPeerNfcReceiverSessionV1 receiver =
        IrohaPeerNfcV1.receiver(session, request.encode(), null, policy, limits);
    admit(
        receiver,
        IrohaPeerNfcCommandV1.beginPayment(
            session,
            request.canonicalHash(),
            Arrays.copyOf(payment.encode(), IrohaPeerWireMessageV1.HEADER_LENGTH)));
    receiver.handle(
        IrohaPeerNfcCommandV1.write(session, payment.wireHash(), 0, payment.encode()));
    final IrohaPeerNfcReceiverApduBridgeV1 bridge =
        IrohaPeerAndroidNfcV1.receiverBridge(
            receiver,
            (context, completion) -> {
              throw new AssertionError("BEGIN is already durable");
            },
            (context, completion) -> completion.complete(null, new Exception("disk")));
    final AtomicReference<IrohaPeerNfcApduResponseV1> response = new AtomicReference<>();
    bridge.handle(
        IrohaPeerNfcCommandV1.commit(
            session, request.canonicalHash(), payment.wireHash()),
        response::set);
    assertEquals(IrohaPeerNfcStatusWordV1.STORAGE_FAILURE, response.get().getStatusWord());
  }

  private static IrohaPeerWireMessageV1 message(
      final IrohaPeerPayloadKind kind, final int repeated, final int count) {
    return new IrohaPeerWireMessageV1(
        new IrohaPeerCanonicalPayload(
            IrohaPeerPayloadProfile.OFFLINE_NOTE,
            kind,
            1,
            repeat(count, repeated)));
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

  private static byte[] repeat(final int count, final int value) {
    final byte[] bytes = new byte[count];
    Arrays.fill(bytes, (byte) value);
    return bytes;
  }
}
