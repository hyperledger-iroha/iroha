// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.offline

/** Refusing test fixture: never creates originals or native/device monetary admission. */
class TestOnlyRequiredIncomingEvidenceOwnerV1 : KagemushaIncomingFoldEvidenceProviderV1 {
    override fun recheckOriginals(preparation: KagemushaNativeIncomingFoldPreparationV1) {
        throw KagemushaIncomingFoldEvidenceUnavailableV1()
    }
    override fun obtainOrRecoverOriginal(preparation: KagemushaNativeIncomingFoldPreparationV1): KagemushaIncomingFoldEvidenceV1 {
        throw KagemushaIncomingFoldEvidenceUnavailableV1()
    }
}
