// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.norito;

import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;

/** Computes domain-separated SHA-256 schema hashes matching the Rust implementation. */
public final class SchemaHash {
  private static final byte[] TYPE_NAME_DOMAIN =
      "norito:v1:type-name\0".getBytes(StandardCharsets.UTF_8);

  private SchemaHash() {}

  public static byte[] hash16(String canonicalPath) {
    return hash16(TYPE_NAME_DOMAIN, canonicalPath.getBytes(StandardCharsets.UTF_8));
  }

  private static byte[] hash16(byte[] domain, byte[] input) {
    MessageDigest digest = sha256();
    digest.update(domain);
    digest.update(input);
    return Arrays.copyOf(digest.digest(), 16);
  }

  private static MessageDigest sha256() {
    try {
      return MessageDigest.getInstance("SHA-256");
    } catch (NoSuchAlgorithmException ex) {
      throw new IllegalStateException("SHA-256 digest is unavailable", ex);
    }
  }

}
