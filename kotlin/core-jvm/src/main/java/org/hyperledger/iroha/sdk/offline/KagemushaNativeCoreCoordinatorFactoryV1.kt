// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

/**
 * Service-provider boundary for the shared Iroha native-Core runtime.
 *
 * A shared Iroha platform adapter may implement this interface using app-owned Android Keystore
 * P-256 keys under the accepted TEE or StrongBox policy, with Play Integrity verified separately.
 * Custom applets, OMAPI access and OEM provisioning are not prerequisites. Hardware-backed identity
 * enrollment does not by itself qualify offline nonforking, counters or recovery guarantees.
 *
 * Implementations must have a public zero-argument constructor for [java.util.ServiceLoader].
 * This interface supplies no implementation and permits no process-memory or filesystem
 * substitute for the actual native original owner and monetary authority.
 */
interface KagemushaNativeCoreCoordinatorFactoryV1 {
    /** Create the native coordinator owned by the shared qualified runtime. */
    fun create(): KagemushaNativeCoreCoordinatorV1

    /** Return the original evidence owner paired with this exact native coordinator; native checks remain mandatory. */
    fun incomingFoldEvidenceProvider(coordinator: KagemushaNativeCoreCoordinatorV1): KagemushaIncomingFoldEvidenceProviderV1
}
