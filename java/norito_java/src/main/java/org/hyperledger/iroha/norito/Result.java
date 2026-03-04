// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.norito;

import java.util.Objects;

/** Lightweight Result type mirroring Rust's Ok/Err. */
public sealed interface Result<T, E> permits Result.Ok, Result.Err {
  record Ok<T, E>(T value) implements Result<T, E> {
    public Ok {
      Objects.requireNonNull(value);
    }
  }

  record Err<T, E>(E error) implements Result<T, E> {
    public Err {
      Objects.requireNonNull(error);
    }
  }
}
