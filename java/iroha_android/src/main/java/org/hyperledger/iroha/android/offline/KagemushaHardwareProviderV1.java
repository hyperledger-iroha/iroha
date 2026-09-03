// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

/**
 * Java surface for the mandatory KAGEMUSHA V1 non-forking hardware provider.
 *
 * <p>The behavioral contract and implementation live in the default Kotlin SDK. Implementations
 * must provide every inherited hardware operation and may not use a software fallback.
 */
public interface KagemushaHardwareProviderV1
    extends org.hyperledger.iroha.sdk.offline.KagemushaHardwareProviderV1 {}
