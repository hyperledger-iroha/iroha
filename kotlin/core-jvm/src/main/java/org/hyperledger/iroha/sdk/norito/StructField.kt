// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.norito

class StructField<T>(
    @JvmField val name: String,
    @JvmField val adapter: TypeAdapter<T>,
    @JvmField val accessor: ((Any) -> Any?)? = null,
)
