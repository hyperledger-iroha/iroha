// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.norito

/** Interface implemented by codec adapters. */
interface TypeAdapter<T> {
    fun encode(encoder: NoritoEncoder, value: T)

    fun decode(decoder: NoritoDecoder): T

    /** Returns the fixed size in bytes, or -1 if variable. */
    fun fixedSize(): Int = -1

    /** Returns true if the adapter carries its own length delimiters. */
    fun isSelfDelimiting(): Boolean = false
}
