// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.address

/**
 * Decoded asset definition carrying the `aid:<hex>` identifier.
 *
 * Since `AssetDefinitionId` is now a one-way blake3 hash,
 * name and domain cannot be recovered from the binary representation.
 * Use the app's cached asset definitions for display info lookup.
 */
class AssetDefinition(@JvmField val aidHex: String)
