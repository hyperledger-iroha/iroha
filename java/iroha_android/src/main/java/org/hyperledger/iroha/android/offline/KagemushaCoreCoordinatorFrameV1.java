// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import java.util.List;
import org.hyperledger.iroha.sdk.offline.KagemushaCoreCoordinatorMethodV1;

/** Java mirror of the Kotlin SDK's strict native coordinator framing; no monetary authority. */
public final class KagemushaCoreCoordinatorFrameV1 {
  private KagemushaCoreCoordinatorFrameV1() {}

  /** Encode a complete request using the sole current native schema. */
  public static byte[] encodeRequest(
      final KagemushaCoreCoordinatorMethodV1 method, final List<byte[]> fields) {
    return org.hyperledger.iroha.sdk.offline.KagemushaCoreCoordinatorFrameV1.encodeRequest(method, fields);
  }

  /** Decode exact request fields, rejecting stale schemas and malformed inputs. */
  public static List<byte[]> decodeRequest(
      final KagemushaCoreCoordinatorMethodV1 method, final byte[] frame) {
    return org.hyperledger.iroha.sdk.offline.KagemushaCoreCoordinatorFrameV1.decodeRequest(method, frame);
  }

  /** Correlate the response against the exact request before returning fields. */
  public static List<byte[]> decodeResponse(
      final KagemushaCoreCoordinatorMethodV1 method,
      final byte[] requestFrame, final byte[] responseFrame) {
    return org.hyperledger.iroha.sdk.offline.KagemushaCoreCoordinatorFrameV1.decodeResponse(
        method, requestFrame, responseFrame);
  }

  /** Encode a field-array response with the same strict native request correlation. */
  public static byte[] encodeResponse(
      final KagemushaCoreCoordinatorMethodV1 method,
      final byte[] requestFrame, final List<byte[]> fields) {
    return org.hyperledger.iroha.sdk.offline.KagemushaCoreCoordinatorFrameV1.encodeResponse(
        method, requestFrame, fields);
  }
}
