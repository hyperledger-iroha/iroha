// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.model.instructions;

import java.util.Objects;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.offline.KagemushaNoritoV1;
import org.hyperledger.iroha.sdk.offline.KagemushaTopUpRequestV1;

/** Native first-release instruction for one payer-authorized KAGEMUSHA reserve top-up. */
public final class TopUpKagemushaV1Instruction {
  /** Sole first-release dynamic instruction registry identifier. */
  public static final String WIRE_ID = "iroha.kagemusha.v1.top_up";

  /** Exact concrete Rust type whose schema hash binds the instruction payload. */
  public static final String SCHEMA_NAME =
      "iroha_data_model::isi::kagemusha_v1::TopUpKagemushaV1";

  private final KagemushaTopUpRequestV1 request;

  public TopUpKagemushaV1Instruction(final KagemushaTopUpRequestV1 request) {
    this.request = Objects.requireNonNull(request, "request");
    KagemushaNoritoV1.encodeTopUpRequestShape(request);
  }

  /** Returns the complete proof-bearing deterministic top-up intent. */
  public KagemushaTopUpRequestV1 request() {
    return request;
  }

  /** Returns the registered, schema-bound instruction ready for a transaction executable. */
  public InstructionBox toInstructionBox() {
    return InstructionBox.fromWirePayload(
        WIRE_ID, KagemushaNoritoV1.encodeTopUpInstructionPayloadShape(request));
  }
}
