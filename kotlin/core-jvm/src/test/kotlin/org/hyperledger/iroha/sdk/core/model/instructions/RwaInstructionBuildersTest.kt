package org.hyperledger.iroha.sdk.core.model.instructions

import kotlin.test.Test
import kotlin.test.assertEquals

class RwaInstructionBuildersTest {

    @Test
    fun `register rwa stores raw NewRwa json`() {
        val instruction = RegisterRwaInstruction("""{"domain":"commodities"}""")

        assertEquals(InstructionKind.CUSTOM, instruction.kind)
        assertEquals(
            mapOf(
                "action" to "RegisterRwa",
                "rwa" to """{"domain":"commodities"}""",
            ),
            instruction.arguments,
        )
        assertEquals(instruction, RegisterRwaInstruction.fromArguments(instruction.arguments))
    }

    @Test
    fun `transfer rwa stores canonical argument schema`() {
        val instruction = TransferRwaInstruction(
            sourceAccountId = "source-account",
            rwaId = "lot-001${'$'}commodities",
            quantity = "10.5",
            destinationAccountId = "destination-account",
        )

        assertEquals(InstructionKind.CUSTOM, instruction.kind)
        assertEquals("TransferRwa", instruction.arguments["action"])
        assertEquals("source-account", instruction.arguments["source"])
        assertEquals("lot-001${'$'}commodities", instruction.arguments["rwa"])
        assertEquals("10.5", instruction.arguments["quantity"])
        assertEquals("destination-account", instruction.arguments["destination"])
        assertEquals(instruction, TransferRwaInstruction.fromArguments(instruction.arguments))
    }

    @Test
    fun `set rwa controls stores raw policy json`() {
        val instruction = SetRwaControlsInstruction(
            rwaId = "lot-002${'$'}commodities",
            controlsJson = """{"freeze_enabled":true}""",
        )

        assertEquals(InstructionKind.CUSTOM, instruction.kind)
        assertEquals("SetRwaControls", instruction.arguments["action"])
        assertEquals("lot-002${'$'}commodities", instruction.arguments["rwa"])
        assertEquals("""{"freeze_enabled":true}""", instruction.arguments["controls"])
        assertEquals(instruction, SetRwaControlsInstruction.fromArguments(instruction.arguments))
    }

    @Test
    fun `merge rwas stores raw merge json`() {
        val instruction = MergeRwasInstruction("""{"parents":[]}""")

        assertEquals(InstructionKind.CUSTOM, instruction.kind)
        assertEquals("MergeRwas", instruction.arguments["action"])
        assertEquals("""{"parents":[]}""", instruction.arguments["merge"])
        assertEquals(instruction, MergeRwasInstruction.fromArguments(instruction.arguments))
    }

    @Test
    fun `remaining rwa custom operations roundtrip`() {
        val redeem = RedeemRwaInstruction("lot-004${'$'}commodities", "2")
        val freeze = FreezeRwaInstruction("lot-004${'$'}commodities")
        val unfreeze = UnfreezeRwaInstruction("lot-004${'$'}commodities")
        val hold = HoldRwaInstruction("lot-004${'$'}commodities", 3)
        val release = ReleaseRwaInstruction("lot-004${'$'}commodities", "1")
        val forceTransfer = ForceTransferRwaInstruction(
            rwaId = "lot-004${'$'}commodities",
            quantity = "1",
            destinationAccountId = "controller-destination",
        )

        assertEquals("RedeemRwa", redeem.arguments["action"])
        assertEquals(redeem, RedeemRwaInstruction.fromArguments(redeem.arguments))

        assertEquals("FreezeRwa", freeze.arguments["action"])
        assertEquals(freeze, FreezeRwaInstruction.fromArguments(freeze.arguments))

        assertEquals("UnfreezeRwa", unfreeze.arguments["action"])
        assertEquals(unfreeze, UnfreezeRwaInstruction.fromArguments(unfreeze.arguments))

        assertEquals("HoldRwa", hold.arguments["action"])
        assertEquals("3", hold.arguments["quantity"])
        assertEquals(hold, HoldRwaInstruction.fromArguments(hold.arguments))

        assertEquals("ReleaseRwa", release.arguments["action"])
        assertEquals(release, ReleaseRwaInstruction.fromArguments(release.arguments))

        assertEquals("ForceTransferRwa", forceTransfer.arguments["action"])
        assertEquals("controller-destination", forceTransfer.arguments["destination"])
        assertEquals(
            forceTransfer,
            ForceTransferRwaInstruction.fromArguments(forceTransfer.arguments),
        )
    }

    @Test
    fun `set and remove key value accept rwa targets`() {
        val setInstruction = SetKeyValueInstruction.builder()
            .setRwaId("lot-003${'$'}commodities")
            .setKey("serial")
            .setValue("ABC-123")
            .build()
        val removeInstruction = RemoveKeyValueInstruction.builder()
            .setRwaId("lot-003${'$'}commodities")
            .setKey("serial")
            .build()

        assertEquals(SetKeyValueInstruction.Target.RWA, setInstruction.target)
        assertEquals("SetRwaKeyValue", setInstruction.arguments["action"])
        assertEquals("lot-003${'$'}commodities", setInstruction.arguments["rwa"])

        assertEquals(RemoveKeyValueInstruction.Target.RWA, removeInstruction.target)
        assertEquals("RemoveRwaKeyValue", removeInstruction.arguments["action"])
        assertEquals("lot-003${'$'}commodities", removeInstruction.arguments["rwa"])
    }
}
