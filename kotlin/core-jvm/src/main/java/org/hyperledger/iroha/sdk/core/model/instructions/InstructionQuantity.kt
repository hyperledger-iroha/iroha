package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.numeric.KotodamaQuantity

/** Validates the canonical non-negative quantity spelling shared by asset and RWA instructions. */
internal fun requireCanonicalQuantity(value: String): String =
    KotodamaQuantity.parseCanonical(value).toString()
