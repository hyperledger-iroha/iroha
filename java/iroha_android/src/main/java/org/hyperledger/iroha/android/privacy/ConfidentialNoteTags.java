package org.hyperledger.iroha.android.privacy;

import java.nio.charset.StandardCharsets;
import java.util.Collections;

/** Asset and chain tags used by the confidential-v2 note derivation. */
public final class ConfidentialNoteTags {
  private ConfidentialNoteTags() {}

  public static byte[] deriveAssetTag(final String asset) {
    return ConfidentialNoteScalars.scalarToLittleEndian(
        ConfidentialNoteScalars.hashToScalar(
            "iroha.confidential.v2.asset_tag",
            Collections.singletonList(
                ConfidentialNoteScalars.canonicalText(asset, "asset")
                    .getBytes(StandardCharsets.UTF_8))));
  }

  public static byte[] deriveChainTag(final String chainId) {
    return ConfidentialNoteScalars.scalarToLittleEndian(
        ConfidentialNoteScalars.hashToScalar(
            "iroha.confidential.v2.chain_tag",
            Collections.singletonList(
                ConfidentialNoteScalars.canonicalText(chainId, "chainId")
                    .getBytes(StandardCharsets.UTF_8))));
  }
}
