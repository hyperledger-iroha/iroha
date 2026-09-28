// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.crypto;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;
import org.junit.jupiter.api.Test;

/** Java consumers use the canonical Kotlin-owned primitive. */
public final class BlsNormalPublicKeyAdmissionJavaTest {
  @Test
  public void canonicalPointAndInfinity() {
    assertTrue(BlsNormalPublicKeyAdmission.isCanonicalBlsNormalPeerId("ea013094D37A1FCA72E8734CAAD4163678D82C36FE2CA70B80F5626E6591709E0D44831BE86CBA9BD0471C6D0D73FF9C4B54E0"));
    assertFalse(BlsNormalPublicKeyAdmission.isCanonicalBlsNormalPeerId("ea0130C00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000"));
  }
}
