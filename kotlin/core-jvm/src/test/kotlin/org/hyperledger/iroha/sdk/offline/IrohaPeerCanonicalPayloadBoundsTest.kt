package org.hyperledger.iroha.sdk.offline

import org.junit.jupiter.api.Test
import kotlin.test.assertFailsWith

class IrohaPeerCanonicalPayloadBoundsTest {
    @Test
    fun `oversized canonical payload fails before defensive copy`() {
        assertFailsWith<IllegalArgumentException> {
            IrohaPeerCanonicalPayload(
                IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
                IrohaPeerPayloadKind.PAYMENT,
                IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND.requiredSchemaVersion,
                ByteArray(IrohaPeerWireMessageV1.MAXIMUM_CANONICAL_BYTES + 1),
            )
        }
    }
}
