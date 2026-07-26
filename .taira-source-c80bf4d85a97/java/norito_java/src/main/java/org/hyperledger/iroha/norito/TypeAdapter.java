// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.norito;

/** Interface implemented by codec adapters. */
public interface TypeAdapter<T> {
  void encode(NoritoEncoder encoder, T value);

  T decode(NoritoDecoder decoder);

  /** Returns the fixed size in bytes, or -1 if variable. */
  default int fixedSize() {
    return -1;
  }

  /** Returns true if the adapter carries its own length delimiters. */
  default boolean isSelfDelimiting() {
    return false;
  }
}
