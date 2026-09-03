package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.math.BigInteger;
import java.util.Arrays;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcCommandV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcCommandTypeV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcDurableIntentAdmissionV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcDurableAcceptanceTicketV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcIntentAdmissionDispositionV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcIntentCommitDispositionV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcLimitsV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcPhaseV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcProfilePolicyV1;
import org.hyperledger.iroha.sdk.offline.IrohaPeerNfcReceiverSessionV1;
import org.hyperledger.iroha.sdk.offline.KagemushaAcceptanceIntentV1;
import org.junit.Test;

public final class IrohaPeerNfcV1AdversarialTests {
  @Test
  public void javaFacadeRoundTripsTheIntentOnlyApduVocabulary() {
    final byte[] session = repeat(16, 0x11);
    final byte[] requestHash = repeat(32, 0x12);
    final KagemushaAcceptanceIntentV1 intent = new KagemushaAcceptanceIntentV1(
        1, repeat(32, 0x13), repeat(32, 0x14), BigInteger.TEN, repeat(32, 0x15));
    final IrohaPeerWireMessageV1 message = IrohaPeerKagemushaAdapterV1.wrap(
        IrohaPeerPayloadKind.INTENT, KagemushaNoritoV1.encodeAcceptanceIntentShape(intent));
    final IrohaPeerNfcCommandV1[] commands = {
      IrohaPeerNfcCommandV1.beginIntent(
          session, requestHash, Arrays.copyOf(message.encode(), IrohaPeerWireMessageV1.HEADER_LENGTH)),
      IrohaPeerNfcCommandV1.writeIntent(session, message.wireHash(), 0, message.encode()),
      IrohaPeerNfcCommandV1.commitIntent(session, requestHash, message.wireHash())
    };
    final IrohaPeerNfcCommandTypeV1[] kinds = {
      IrohaPeerNfcCommandTypeV1.BEGIN_INTENT,
      IrohaPeerNfcCommandTypeV1.WRITE_INTENT,
      IrohaPeerNfcCommandTypeV1.COMMIT_INTENT
    };
    for (int index = 0; index < commands.length; index++) {
      final byte[] encoded = IrohaPeerNfcV1.encodeCommand(commands[index]);
      assertEquals(0x12 + index, encoded[1] & 0xff);
      final IrohaPeerNfcCommandV1 decoded = IrohaPeerNfcV1.decodeCommand(encoded);
      assertEquals(kinds[index], decoded.getType());
      assertEquals(commands[index], decoded);
    }
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerNfcCommandTypeV1.valueOf("BEGIN_" + "AUTHORIZATION"));
  }

  @Test
  public void javaFacadePersistsIntentAdmissionBeforeIssuingTheTicket() throws Exception {
    final String fixture = KagemushaWireV1Tests.loadCanonicalFixture();
    final IrohaPeerWireMessageV1 request = IrohaPeerKagemushaAdapterV1.wrap(
        IrohaPeerPayloadKind.REQUEST, KagemushaWireV1Tests.fixtureHex(fixture, "payment_request"));
    final IrohaPeerWireMessageV1 intent = IrohaPeerKagemushaAdapterV1.wrap(
        IrohaPeerPayloadKind.INTENT, KagemushaWireV1Tests.fixtureHex(fixture, "acceptance_intent"));
    final IrohaPeerWireMessageV1 ticket = IrohaPeerKagemushaAdapterV1.wrap(
        IrohaPeerPayloadKind.TICKET, KagemushaWireV1Tests.fixtureHex(fixture, "acceptance_ticket"));
    final byte[] session = repeat(16, 0x61);
    final IrohaPeerNfcProfilePolicyV1 policy =
        IrohaPeerNfcV1.profilePolicy(IrohaPeerPayloadProfile.KAGEMUSHA_V1);
    final IrohaPeerNfcLimitsV1 limits = IrohaPeerNfcV1.limits(
        IrohaPeerNfcV1.MAXIMUM_MESSAGE_BYTES,
        IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES,
        IrohaPeerNfcV1.MAXIMUM_CHUNK_BYTES);
    final IrohaPeerNfcReceiverSessionV1 receiver =
        IrohaPeerNfcV1.receiver(session, request.encode(), policy, limits);
    final IrohaPeerNfcIntentAdmissionDispositionV1 admission = receiver.prepareIntentAdmission(
        IrohaPeerNfcCommandV1.beginIntent(
            session, request.canonicalHash(),
            Arrays.copyOf(intent.encode(), IrohaPeerWireMessageV1.HEADER_LENGTH)));
    assertTrue(admission instanceof IrohaPeerNfcIntentAdmissionDispositionV1.RequiresDurableAdmission);
    final IrohaPeerNfcDurableIntentAdmissionV1 durableAdmission = IrohaPeerNfcV1.durableIntentAdmission(
        ((IrohaPeerNfcIntentAdmissionDispositionV1.RequiresDurableAdmission) admission).getContext());
    receiver.installIntentAdmission(
        IrohaPeerNfcV1.decodeIntentAdmission(durableAdmission.encode(), policy, limits));
    receiver.handle(IrohaPeerNfcCommandV1.writeIntent(session, intent.wireHash(), 0, intent.encode()));
    final IrohaPeerNfcIntentCommitDispositionV1 commit = receiver.prepareIntentCommit(
        IrohaPeerNfcCommandV1.commitIntent(session, request.canonicalHash(), intent.wireHash()));
    assertTrue(commit instanceof IrohaPeerNfcIntentCommitDispositionV1.RequiresDurableTicket);
    final IrohaPeerNfcDurableAcceptanceTicketV1 durableTicket = IrohaPeerNfcV1.durableAcceptanceTicket(
        ((IrohaPeerNfcIntentCommitDispositionV1.RequiresDurableTicket) commit).getContext(),
        ticket.encode());
    receiver.installDurableTicket(durableTicket);
    assertEquals(IrohaPeerNfcPhaseV1.TICKET_READY, durableTicket.getSnapshot().getPhase());
    assertArrayEquals(
        intent.encode(),
        IrohaPeerNfcV1.senderCheckpoint(
            session, request.encode(), intent.encode(), ticket.encode(), null, null, policy, limits)
            .getIntent().encode());
  }

  @Test
  public void javaFacadeRejectsNonCanonicalExtendedApduAliases() {
    final byte[] aliasedGetInfo =
        new byte[] {(byte) 0x80, 0x10, 0, 0, 0, 0, 0x62};
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerNfcV1.decodeCommand(aliasedGetInfo));

    final byte[] session = repeat(16, 0x21);
    final byte[] hash = repeat(32, 0x31);
    final byte[] canonicalWrite =
        IrohaPeerNfcV1.encodeCommand(
            IrohaPeerNfcCommandV1.writePayment(session, hash, 0, new byte[] {0x55}));
    final int bodyLength = canonicalWrite.length - 5;
    final byte[] aliasedWrite = new byte[7 + bodyLength];
    aliasedWrite[0] = (byte) 0x80;
    aliasedWrite[1] = 0x21;
    aliasedWrite[5] = 0;
    aliasedWrite[6] = (byte) bodyLength;
    System.arraycopy(canonicalWrite, 5, aliasedWrite, 7, bodyLength);
    assertThrows(
        IllegalArgumentException.class,
        () -> IrohaPeerNfcV1.decodeCommand(aliasedWrite));
  }

  @Test
  public void javaFacadePreservesUnsignedU32OffsetWithoutWrap() {
    final byte[] session = repeat(16, 0x41);
    final byte[] hash = repeat(32, 0x51);
    final IrohaPeerNfcCommandV1 command =
        IrohaPeerNfcCommandV1.readRequest(session, hash, 0xffff_ffffL, 1);
    final IrohaPeerNfcCommandV1 decoded =
        IrohaPeerNfcV1.decodeCommand(IrohaPeerNfcV1.encodeCommand(command));

    assertEquals(IrohaPeerNfcCommandTypeV1.READ_REQUEST, decoded.getType());
    assertEquals(0xffff_ffffL, decoded.getOffset());
    assertArrayEquals(session, decoded.getSessionId());
    assertArrayEquals(hash, decoded.getFirstHash());
  }

  private static byte[] repeat(final int count, final int value) {
    final byte[] bytes = new byte[count];
    java.util.Arrays.fill(bytes, (byte) value);
    return bytes;
  }
}
