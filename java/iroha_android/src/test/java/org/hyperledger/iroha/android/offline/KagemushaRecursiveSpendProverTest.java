package org.hyperledger.iroha.android.offline;

import java.net.URI;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.atomic.AtomicReference;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.norito.CRC64;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;

/** Source-level contract checks for the typed ABI-19 Kagemusha lifecycle bridge. */
public final class KagemushaRecursiveSpendProverTest {
  public static void main(final String[] args) {
    exactAbiIsRequired();
    artifactContractIsFixed();
    canonicalPeerCodecsAreTypedAndDefensive();
    lifecycleArchivesAreTypedDefensiveAndFailClosed();
    scaledAmountsAreExactAndNeverRound();
    peerTransportGoldenVectorsAreExact();
    qrNfcAndNearbyGoldenVectorsAreExact();
    toriiLifecycleRoutesAndHeadersAreExact();
    publicSurfaceIsKagemushaOnly();
  }

  private static void exactAbiIsRequired() {
    assert KagemushaRecursiveSpendProver.isExactBridgeAbi(19);
    assert !KagemushaRecursiveSpendProver.isExactBridgeAbi(20);
    assert KagemushaRecursiveSpendProver.detectExactNativeAvailability(
        () -> {}, () -> 19, () -> true);
    assert !KagemushaRecursiveSpendProver.detectExactNativeAvailability(
        () -> {}, () -> 19, () -> false);
    assert !KagemushaRecursiveSpendProver.detectExactNativeAvailability(
        () -> { throw new UnsatisfiedLinkError("missing"); }, () -> 19, () -> true);
  }

  private static void artifactContractIsFixed() {
    assert KagemushaRecursiveSpendProver.REQUIRED_NATIVE_BRIDGE_ABI_VERSION == 19;
    assert KagemushaRecursiveSpendProver.ARTIFACT_COUNT == 6;
    assert KagemushaRecursiveSpendProver.MAXIMUM_INPUTS_PER_TRANSITION == 2;
    assert KagemushaRecursiveSpendProver.MAXIMUM_LOCAL_APPEND_BUILDER_INPUTS == 1;
    assert KagemushaRecursiveSpendProver.MAXIMUM_PEER_HOPS == 8;
    assert KagemushaRecursiveSpendProver.MAX_PEER_ARCHIVE_BYTES == 32 * 1024;
    assert KagemushaRecursiveSpendProver.MAX_PEER_TEXT_ARCHIVE_BYTES == 9_211;
    assert KagemushaRecursiveSpendProver.CONFIDENTIAL_TREE_DEPTH == 16;
    assert "kagemusha.offline.recursive_spend.artifact_manifest.v3"
        .equals(KagemushaRecursiveSpendProver.ARTIFACT_MANIFEST_SCHEMA);
    assert KagemushaRecursiveSpendProver.ARTIFACT_FILES.equals(
        List.of(
            "step-eq.parameters.krv3",
            "step-eq.proving-key.krv3",
            "step-eq.verifying-key.krv3",
            "step-ep.parameters.krv3",
            "step-ep.proving-key.krv3",
            "step-ep.verifying-key.krv3"));
  }

  private static void lifecycleArchivesAreTypedDefensiveAndFailClosed() {
    final byte[] initBytes = archive("KagemushaRecursiveSpendInitRequestV2");
    final KagemushaRecursiveSpendProver.InitRequest init =
        KagemushaRecursiveSpendProver.decodeInitRequest(initBytes);
    initBytes[initBytes.length - 1] = 0;
    assert init.noritoEncoded()[init.noritoEncoded().length - 1] == 0x51;

    final KagemushaRecursiveSpendProver.AppendRequest append =
        KagemushaRecursiveSpendProver.decodeAppendRequest(
            archive("KagemushaRecursiveSpendAppendLocalRequestV2"), null);
    assert append.noritoEncoded().length > NoritoHeader.HEADER_LENGTH;
    assert KagemushaRecursiveSpendProver.decodeVerifyRequest(
            archive("KagemushaRecursiveSpendVerifyRequestV2"))
        .noritoEncoded().length > NoritoHeader.HEADER_LENGTH;
    assert KagemushaRecursiveSpendProver.decodeRedeemRequest(
            archive("KagemushaRecursiveSpendRedeemLocalRequestV2"), null)
        .noritoEncoded().length > NoritoHeader.HEADER_LENGTH;
    assert KagemushaRecursiveSpendProver.decodeInitResult(
            archive("KagemushaRecursiveSpendInitResultV2"))
        .noritoEncoded().length > NoritoHeader.HEADER_LENGTH;

    boolean wrongSchemaRejected = false;
    try {
      KagemushaRecursiveSpendProver.decodeVerifyRequest(
          archive("KagemushaRecursiveSpendInitRequestV2"));
    } catch (final IllegalArgumentException expected) {
      wrongSchemaRejected = true;
    }
    assert wrongSchemaRejected;

    boolean invalidTimestampRejected = false;
    try {
      KagemushaRecursiveSpendProver.appendSpend(
          append,
          KagemushaRecursiveSpendProver.decodeRecipientPaymentRequest(
              archive("KagemushaRecipientPaymentRequestV2")),
          0);
    } catch (final IllegalArgumentException expected) {
      invalidTimestampRejected = true;
    }
    assert invalidTimestampRejected;

    if (!KagemushaRecursiveSpendProver.isProofBackendAvailable()) {
      boolean unavailableRejected = false;
      try {
        KagemushaRecursiveSpendProver.initSpend(init);
      } catch (final IllegalStateException expected) {
        unavailableRejected = true;
      }
      assert unavailableRejected;
    }
    append.close();
    assert append.isDestroyed();
    boolean destroyedRejected = false;
    try {
      append.noritoEncoded();
    } catch (final IllegalStateException expected) {
      destroyedRejected = true;
    }
    assert destroyedRejected;
  }

  private static void scaledAmountsAreExactAndNeverRound() {
    final KagemushaScaledAmount amount = KagemushaScaledAmount.fromDecimal("10.75", 9);
    assert amount.atomicUnits().equals("10750000000");
    assert amount.scaledNumericDecimal().equals("10.750000000");
    assert amount.displayDecimal().equals("10.75");
    assert KagemushaScaledAmount.sum(
            List.of(
                KagemushaScaledAmount.fromDecimal("4.50", 9),
                KagemushaScaledAmount.fromDecimal("6.25", 9)))
        .atomicUnits().equals("10750000000");
    assert KagemushaScaledAmount.fromAtomicUnits("1", 9)
        .scaledNumericDecimal().equals("0.000000001");
    assert KagemushaScaledAmount.fromAtomicUnits(
            KagemushaScaledAmount.MAXIMUM_ATOMIC_UNITS, 28)
        .atomicUnits().equals(KagemushaScaledAmount.MAXIMUM_ATOMIC_UNITS);

    boolean precisionRejected = false;
    try {
      KagemushaScaledAmount.fromDecimal("1.001", 2);
    } catch (final IllegalArgumentException expected) {
      precisionRejected = true;
    }
    assert precisionRejected;

    boolean overflowRejected = false;
    try {
      KagemushaScaledAmount.fromAtomicUnits(
          "340282366920938463463374607431768211456", 9);
    } catch (final IllegalArgumentException expected) {
      overflowRejected = true;
    }
    assert overflowRejected;
  }

  private static void canonicalPeerCodecsAreTypedAndDefensive() {
    final byte[] requestArchive = archive("KagemushaRecipientPaymentRequestV2");
    final KagemushaRecursiveSpendProver.RecipientPaymentRequest request =
        KagemushaRecursiveSpendProver.decodeRecipientPaymentRequest(requestArchive);
    requestArchive[requestArchive.length - 1] ^= 1;
    assert request.noritoEncoded()[request.noritoEncoded().length - 1] == 0x51;

    assert KagemushaRecursiveSpendProver.decodePeerPayment(
            archive("KagemushaRecursiveSpendPeerPaymentV2"))
        .noritoEncoded().length > NoritoHeader.HEADER_LENGTH;
    assert KagemushaRecursiveSpendProver.decodeReceiverAcknowledgement(
            archive("KagemushaReceiverAcknowledgementV2"))
        .noritoEncoded().length > NoritoHeader.HEADER_LENGTH;
    assert KagemushaRecursiveSpendProver.decodeNoteMembershipWitness(
            archive("KagemushaNoteMembershipWitnessV2"))
        .noritoEncoded().length > NoritoHeader.HEADER_LENGTH;

    boolean rejected = false;
    try {
      KagemushaRecursiveSpendProver.decodePeerPayment(
          archive("KagemushaRecipientPaymentRequestV2"));
    } catch (final IllegalArgumentException expected) {
      rejected = true;
    }
    assert rejected;
  }

  private static byte[] archive(final String schema) {
    final byte[] payload = new byte[] {0x51};
    final NoritoHeader header =
        new NoritoHeader(
            SchemaHash.hash16(schema),
            payload.length,
            CRC64.compute(payload),
            NoritoHeader.COMPACT_LEN,
            NoritoHeader.COMPRESSION_NONE);
    final byte[] archive = new byte[NoritoHeader.HEADER_LENGTH + payload.length];
    System.arraycopy(header.encode(), 0, archive, 0, NoritoHeader.HEADER_LENGTH);
    System.arraycopy(payload, 0, archive, NoritoHeader.HEADER_LENGTH, payload.length);
    return archive;
  }

  private static void peerTransportGoldenVectorsAreExact() {
    final KagemushaPeerTransport.Payload request =
        KagemushaPeerTransport.Payload.decode(
            archive("KagemushaRecipientPaymentRequestV2"),
            KagemushaPeerTransport.Kind.RECEIVE_REQUEST);
    final String text = KagemushaPeerTransport.encode(request);
    assert text.equals(
        "PKK2R.TlJUMAAA27ZYXi51qDW87RkAOqt6zQABAAAAAAAAAN6BMN0_Z661AlE");
    assert KagemushaPeerTransport.decode(text).kind()
        == KagemushaPeerTransport.Kind.RECEIVE_REQUEST;
    assert KagemushaPeerTransport.decodeUserPresented(" \n" + text + "\t",
        KagemushaPeerTransport.Kind.RECEIVE_REQUEST).kind()
        == KagemushaPeerTransport.Kind.RECEIVE_REQUEST;
    assert KagemushaPeerTransport.RECEIVE_REQUEST_TEXT_PREFIX.equals("PKK2R.");
    assert KagemushaPeerTransport.PAYMENT_TEXT_PREFIX.equals("PKK2P.");
    assert KagemushaPeerTransport.ACKNOWLEDGEMENT_TEXT_PREFIX.equals("PKK2A.");
    assert KagemushaPeerTransport.QR_STREAM_TEXT_PREFIX.equals("PKKQ1.");
  }

  private static void qrNfcAndNearbyGoldenVectorsAreExact() {
    final KagemushaPeerTransport.Payload request =
        KagemushaPeerTransport.Payload.decode(
            archive("KagemushaRecipientPaymentRequestV2"),
            KagemushaPeerTransport.Kind.RECEIVE_REQUEST);
    final List<String> frames = KagemushaQrStream.encode(
        request, KagemushaQrStream.Options.STANDARD);
    assert frames.equals(List.of(
        "PKKQ1.S1EBALu6J7gkvW_mKvRoE04Tc9IAAAABAC4BAQQAAQAAAQABAAAAKbu6J7gkvW_mKvRoE04Tc9L3Ile8Baahf0wb7ZGckATmMK4Faw",
        "PKKQ1.S1EBAbu6J7gkvW_mKvRoE04Tc9IAAAABAClOUlQwAADbtlheLnWoNbztGQA6q3rNAAEAAAAAAAAA3oEw3T9nrrUCUZiX9lk",
        "PKKQ1.S1EBAru6J7gkvW_mKvRoE04Tc9IAAAABAQBOUlQwAADbtlheLnWoNbztGQA6q3rNAAEAAAAAAAAA3oEw3T9nrrUCUQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA4vsCHg"));
    final KagemushaQrStream.Decoder decoder = new KagemushaQrStream.Decoder();
    assert !decoder.ingest(frames.get(0)).isComplete();
    final KagemushaQrStream.DecodeResult recovered = decoder.ingest(frames.get(2));
    assert recovered.isComplete();
    assert recovered.recoveredDataFrames() == 1;

    final byte[] text = KagemushaPeerTransport.encode(request).getBytes(StandardCharsets.UTF_8);
    final List<byte[]> apdus = KagemushaNfcProtocol.writePayloadApdus(
        KagemushaNfcProtocol.PayloadKind.RECEIVE_REQUEST, text, 220);
    assert hex(apdus.get(0)).equals(
        "8020000025010000003d67c2b6e61aef6d1f5e6b10692f58e4c864e988e98d164a43139a8e5b343e77bc");
    assert hex(apdus.get(1)).equals(
        "802100003d504b4b32522e546c4a554d41414132375a59586935317144573837526b414f7174367a5141424141414141414141414e36424d4e305f5a363631416c45");
    assert hex(apdus.get(2)).equals("8022000000");
    assert KagemushaNfcProtocol.AID_HEX.equals("F0504B45504B524E464301");
    assert KagemushaNfcProtocol.SAFE_CHUNK_BYTES == 220;
    assert KagemushaNfcProtocol.parseCommand(apdus.get(0)).type()
        == KagemushaNfcProtocol.Type.WRITE_META;

    final byte[] nearby = KagemushaNearby.encode(request, KagemushaNearby.PairingSymbol.STARS);
    assert new String(nearby, StandardCharsets.UTF_8).equals(
        "{\"contentType\":\"text/vnd.pk.kagemusha-v2.receive-request\",\"kind\":\"receive_request\",\"pairingChallenge\":\"nearby_pairing_stars\",\"payload\":\"UEtLMlIuVGxKVU1BQUEyN1pZWGk1MXFEVzg3UmtBT3F0NnpRQUJBQUFBQUFBQUFONkJNTjBfWjY2MUFsRQ\"}");
    assert KagemushaNearby.decode(nearby).payload().kind()
        == KagemushaPeerTransport.Kind.RECEIVE_REQUEST;
    assert !KagemushaNearby.IS_AVAILABLE;
    Arrays.fill(text, (byte) 0);
    Arrays.fill(nearby, (byte) 0);
  }

  private static String hex(final byte[] bytes) {
    final StringBuilder out = new StringBuilder(bytes.length * 2);
    for (final byte value : bytes) out.append(String.format("%02x", value & 0xff));
    return out.toString();
  }

  private static void toriiLifecycleRoutesAndHeadersAreExact() {
    final AtomicReference<TransportRequest> captured = new AtomicReference<>();
    final KagemushaRecursiveSpendProver.ToriiClient client =
        KagemushaRecursiveSpendProver.newToriiClient(
            URI.create("https://torii.example/api/"),
            request -> {
              captured.set(request);
              final boolean command = "POST".equals(request.method());
              return CompletableFuture.completedFuture(
                  TransportResponse.builder()
                      .setStatusCode(command ? 202 : 200)
                      .addHeader("Content-Type", "application/x-norito")
                      .setBody(
                          archive(
                              command
                                  ? "OfflineOperationReference"
                                  : request.uri().getPath().contains("/operations/")
                                      ? "OfflineOperationStatus"
                                      : "OfflineReadiness"))
                      .build());
            });

    client.getReadiness("pkr#sbp").join();
    assert captured.get().uri().toString()
        .equals("https://torii.example/api/v1/offline/readiness?asset_definition_id=pkr%23sbp");
    assert captured.get().headers().get("Accept").equals(List.of("application/x-norito"));

    final String operationId = "11".repeat(32);
    client
        .submitTopUp(
            new KagemushaRecursiveSpendProver.TopUpRequest(
                archive("iroha.torii.v1.offline.top_up.request")),
            operationId)
        .join();
    assert captured.get().method().equals("POST");
    assert captured.get().uri().getPath().equals("/api/v1/offline/top-up");
    assert captured.get().headers().get("Content-Type").equals(List.of("application/x-norito"));
    assert captured.get().headers().get("Idempotency-Key").equals(List.of(operationId));

    client
        .submitRedeem(
            new KagemushaRecursiveSpendProver.RedeemSubmissionRequest(
                archive("iroha.torii.v1.offline.redeem.request")),
            operationId)
        .join();
    assert captured.get().uri().getPath().equals("/api/v1/offline/redeem");

    client.getOperation(operationId).join();
    assert captured.get().uri().getPath().equals("/api/v1/offline/operations/" + operationId);
  }

  private static void publicSurfaceIsKagemushaOnly() {
    final Set<String> methods = new TreeSet<>();
    for (final Method method : KagemushaRecursiveSpendProver.class.getDeclaredMethods()) {
      if (Modifier.isPublic(method.getModifiers())) {
        methods.add(method.getName());
      }
    }
    assert methods.equals(
        Set.of(
            "beginArtifactIngest",
            "beginArtifactInstallSession",
            "appendSpend",
            "buildAppendRequest",
            "buildInitRequest",
            "buildRedeem",
            "buildRedeemRequest",
            "buildVerifyRequest",
            "decodeAppendRequest",
            "decodeInitRequest",
            "decodeInitResult",
            "decodeNoteMembershipWitness",
            "decodeNoteOpening",
            "decodePeerPayment",
            "decodeRedeemRequest",
            "decodeReceiverAcknowledgement",
            "decodeRecipientPaymentRequest",
            "decodeRedeemBuildResult",
            "decodeRedeemSubmissionRequest",
            "decodeSplitResult",
            "decodeTopUpRequest",
            "decodeVerifyRequest",
            "decodeVerifyResult",
            "decodeTopUpFinalityRosterArtifact",
            "finalizeRedeem",
            "finalizeTopUp",
            "initSpend",
            "isArtifactStreamingAvailable",
            "isProofBackendAvailable",
            "newToriiClient",
            "prepareAcknowledgement",
            "prepareNoteOpening",
            "prepareRecipientPaymentRequest",
            "prepareRequestAuthorization",
            "prepareTopUp",
            "projectInitResult",
            "projectOperationStatus",
            "projectPeerPayment",
            "projectRecipientPaymentRequest",
            "projectReadiness",
            "projectRedeemBuildResult",
            "projectSplitResult",
            "projectVerifyResult",
            "restoreSpendableBranch",
            "signAcknowledgement",
            "signRecipientPaymentRequest",
            "signRequestAuthorization",
            "verifyAcknowledgement",
            "verifyRecipientPaymentRequest",
            "verifySpend")) : methods;
    for (final String name : List.of(
        "decodeAppendRequest",
        "decodeSplitResult",
        "decodeRedeemRequest",
        "decodeRedeemBuildResult")) {
      final List<Method> candidates = Arrays.stream(
              KagemushaRecursiveSpendProver.class.getDeclaredMethods())
          .filter(method -> Modifier.isPublic(method.getModifiers()) && method.getName().equals(name))
          .toList();
      assert candidates.size() == 1 : candidates;
      assert Arrays.equals(
          candidates.get(0).getParameterTypes(),
          new Class<?>[] {byte[].class, KagemushaRecursiveSpendProver.NoteOpening.class}) : name;
    }
  }
}
