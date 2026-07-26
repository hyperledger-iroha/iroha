package org.hyperledger.iroha.android.sorafs;

import java.util.Map;
import org.hyperledger.iroha.android.model.instructions.RecordCapacityTelemetryInstruction;

/** Regression tests covering SoraFS capacity telemetry instruction builder. */
public final class SorafsCapacityTelemetryInstructionTests {

  private SorafsCapacityTelemetryInstructionTests() {}

  private static final String PROVIDER_ID = "aa".repeat(32);

  public static void main(final String[] args) {
    testRecordCapacityTelemetryBuilder();
    System.out.println(
        "[IrohaAndroid] SorafsCapacityTelemetryInstructionTests passed (telemetry).");
  }

  private static void testRecordCapacityTelemetryBuilder() {
    final RecordCapacityTelemetryInstruction instruction =
        RecordCapacityTelemetryInstruction.builder()
            .setProviderIdHex(PROVIDER_ID)
            .setWindowStartEpoch(1_700_100L)
            .setWindowEndEpoch(1_700_112L)
            .setDeclaredGib(2_048L)
            .setEffectiveGib(2_000L)
            .setUtilisedGib(1_536L)
            .setOrdersIssued(12)
            .setOrdersCompleted(10)
            .setUptimeBps(9_800)
            .setPorSuccessBps(9_750)
            .setEgressBytes(1_234_567L)
            .setPdpChallenges(5)
            .setPdpFailures(1)
            .setPotrWindows(3)
            .setPotrBreaches(0)
            .setNonce(77)
            .build();

    final Map<String, String> args = instruction.toArguments();
    assert "RecordCapacityTelemetry".equals(args.get("action")) : "action mismatch";
    assert PROVIDER_ID.equals(args.get("provider_id_hex")) : "provider id mismatch";
    assert "1700112".equals(args.get("window_end_epoch")) : "window end mismatch";
    assert "9800".equals(args.get("uptime_bps")) : "uptime mismatch";
    assert "9750".equals(args.get("por_success_bps")) : "por success mismatch";
    assert "1234567".equals(args.get("egress_bytes")) : "egress mismatch";
    assert "77".equals(args.get("nonce")) : "nonce mismatch";

    assert instruction.windowStartEpoch() == 1_700_100L : "window start mismatch";
    assert instruction.ordersCompleted() == 10 : "orders completed mismatch";
    assert instruction.pdpFailures() == 1 : "pdp failures mismatch";
    assert instruction.nonce() == 77 : "nonce mismatch";
  }
}
