# Native single-transfer proposal projection

The multisig proposal query and resolve routes project an exact single native
`TransferBox::Asset` instruction into `intent` with `kind: "TRANSFER"`,
`asset_id`, `amount`, `from_account_id` and `to_account_id`. `asset_id` is the
native asset definition identifier; `amount` uses the native Quantity display
representation. Both account identifiers come from the typed instruction.

Empty and multi-instruction proposals have no ordinary transfer intent. A
projection of one instruction would omit the remaining effects. The projection
provides fields for display and transport; clients must bind the complete
original instruction batch and its hash when admitting, signing or approving a
transfer. A JSON intent alone does not prove execution or finality.
