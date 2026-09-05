// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

/**
 * Service-provider boundary for a qualified native-Core coordinator.
 *
 * Implementations are supplied by the audited device/runtime package and must have a public
 * zero-argument constructor so the Android wallet can discover them through [java.util.ServiceLoader].
 * The SDK intentionally ships no implementation and permits no process-memory or filesystem
 * substitute for native monetary authority.
 */
fun interface KagemushaNativeCoreCoordinatorFactoryV1 {
    /** Create the native coordinator owned by the qualified runtime package. */
    fun create(): KagemushaNativeCoreCoordinatorV1
}
