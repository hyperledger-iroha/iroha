// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import org.hyperledger.iroha.sdk.offline.KagemushaDeviceOperationCodecV1;
import org.hyperledger.iroha.sdk.offline.KagemushaDeviceSenderPublicInputsV1;
import org.junit.Test;

/** The Java mirror consumes the same native reservation archives as the Kotlin SDK. */
public final class KagemushaOperationReservationV1Tests {
  @Test
  public void senderBindingsMatchTheSharedNativeCoreFixture() throws Exception {
    final String fixture = loadFixture();
    assertArrayEquals(
        bytes(fixture, "send_binding_hex"),
        KagemushaDeviceOperationCodecV1.encodeSenderPublicInputs(
            new KagemushaDeviceSenderPublicInputsV1.SendSplit(bytes(fixture, "send_request_hex"))));
    assertArrayEquals(
        bytes(fixture, "redeem_binding_hex"),
        KagemushaDeviceOperationCodecV1.encodeSenderPublicInputs(
            new KagemushaDeviceSenderPublicInputsV1.RedeemSplit(
                new BigInteger(value(fixture, "redeem_amount_decimal")),
                bytes(fixture, "redeem_beneficiary_payload_hex"))));
  }

  private static String loadFixture() throws Exception {
    Path current = Paths.get("").toAbsolutePath().normalize();
    while (current != null) {
      final Path candidate = current.resolve("fixtures/offline/kagemusha_sender_reservation_v1.json");
      if (Files.isRegularFile(candidate)) {
        return new String(Files.readAllBytes(candidate), StandardCharsets.UTF_8);
      }
      current = current.getParent();
    }
    throw new AssertionError("missing sender reservation fixture");
  }

  private static String value(final String fixture, final String field) {
    final Matcher matcher = Pattern.compile(
        "\\\"" + Pattern.quote(field) + "\\\"\\s*:\\s*\\\"([^\\\"]+)\\\"").matcher(fixture);
    if (!matcher.find()) {
      throw new AssertionError("missing fixture field " + field);
    }
    return matcher.group(1);
  }

  private static byte[] bytes(final String fixture, final String field) {
    final String hex = value(fixture, field);
    final byte[] result = new byte[hex.length() / 2];
    for (int index = 0; index < result.length; index++) {
      result[index] = (byte) Integer.parseInt(hex.substring(index * 2, index * 2 + 2), 16);
    }
    return result;
  }
}
