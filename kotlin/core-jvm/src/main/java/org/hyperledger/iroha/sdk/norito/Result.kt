// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.norito

/** Lightweight Result type mirroring Rust's Ok/Err. */
sealed interface Result<out T, out E> {

    data class Ok<out T, out E>(
        @JvmField val value: T,
    ) : Result<T, E> {
        fun value(): T = value
    }

    data class Err<out T, out E>(
        @JvmField val error: E,
    ) : Result<T, E> {
        fun error(): E = error
    }
}
