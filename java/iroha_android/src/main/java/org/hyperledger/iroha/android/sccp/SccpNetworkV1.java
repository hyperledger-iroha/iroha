package org.hyperledger.iroha.android.sccp;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;

/** Closed first-release SCCP network inventory with exact, case-sensitive profile keys. */
public enum SccpNetworkV1 {
  SORA_NEXUS("sora-nexus", 0, 0, true),
  SORA_TAIRA("sora-taira", 1, 0, false),
  ETHEREUM_MAINNET("ethereum-mainnet", 2, 1, true),
  ETHEREUM_SEPOLIA("ethereum-sepolia", 3, 1, false),
  BSC_MAINNET("bsc-mainnet", 4, 2, true),
  BSC_TESTNET("bsc-testnet", 5, 2, false),
  SOLANA_MAINNET_BETA("solana-mainnet-beta", 6, 3, true),
  SOLANA_TESTNET("solana-testnet", 7, 3, false),
  TON_MAINNET("ton-mainnet", 8, 4, true),
  TON_TESTNET("ton-testnet", 9, 4, false),
  TRON_MAINNET("tron-mainnet", 10, 5, true),
  TRON_NILE("tron-nile", 11, 5, false),
  TRON_SHASTA("tron-shasta", 12, 5, false);

  private static final Map<String, SccpNetworkV1> BY_PROFILE;
  private static final Map<Integer, SccpNetworkV1> BY_TAG;

  static {
    final Map<String, SccpNetworkV1> profiles = new LinkedHashMap<>();
    final Map<Integer, SccpNetworkV1> tags = new LinkedHashMap<>();
    for (final SccpNetworkV1 value : values()) {
      profiles.put(value.profileKey, value);
      tags.put(value.tag, value);
    }
    BY_PROFILE = Collections.unmodifiableMap(profiles);
    BY_TAG = Collections.unmodifiableMap(tags);
  }

  private final String profileKey;
  private final int tag;
  private final int domainId;
  private final boolean production;

  SccpNetworkV1(
      final String profileKey, final int tag, final int domainId, final boolean production) {
    this.profileKey = profileKey;
    this.tag = tag;
    this.domainId = domainId;
    this.production = production;
  }

  public String profileKey() {
    return profileKey;
  }

  public int tag() {
    return tag;
  }

  public int domainId() {
    return domainId;
  }

  public boolean isProduction() {
    return production;
  }

  public boolean isSora() {
    return this == SORA_NEXUS || this == SORA_TAIRA;
  }

  public boolean isExternal() {
    return !isSora();
  }

  /** Parse only a canonical profile key. Aliases, case changes, and whitespace return null. */
  public static SccpNetworkV1 fromProfileKey(final String profile) {
    return BY_PROFILE.get(profile);
  }

  /** Decode a stable first-release profile tag. */
  public static SccpNetworkV1 fromTag(final int tag) {
    return BY_TAG.get(tag);
  }
}
