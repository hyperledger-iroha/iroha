// Enrollment is checked at preparation and again at exact prepared consumption.
fn ensure_uid_enrollment_movement(
    world: &WorldTransaction<'_, '_>,
    source: &AssetId,
    destination: &AssetId,
    purpose: NumericAssetTransferSourcePolicy,
) -> Result<(), Error> {
    use crate::state::asset_uid_enrollment as enrollment;
    if enrollment::enrollment_with_owner(world, source.definition(), Some(world))?.is_none()
        && enrollment::enrollment_with_owner(world, destination.definition(), Some(world))?
            .is_none()
    {
        return Ok(());
    }
    let reject =
        |message: &'static str| InstructionExecutionError::InvariantViolation(message.into());
    if source.definition() != destination.definition()
        || source.scope() != &iroha_data_model::asset::AssetBalanceScope::Global
        || destination.scope() != &iroha_data_model::asset::AssetBalanceScope::Global
    {
        return Err(reject("enrolled movement requires one exact Global asset"));
    }
    let native_source = || {
        let mut records = world.asset_escrows().iter().filter(|(_, record)| {
            record.asset_definition == *source.definition()
                && record.custody == *source.account()
                && !record.remaining_amount.is_zero()
        });
        let record = records.next().map(|(_, record)| record).ok_or_else(|| {
            reject("enrolled native custody requires its exact unresolved record")
        })?;
        if records.next().is_some()
            || enrollment::beneficial_with_owner(
                world,
                source.definition(),
                source.account(),
                Some(world),
            )?
            .is_some()
            || enrollment::beneficial_with_owner(
                world,
                source.definition(),
                &record.seller,
                Some(world),
            )?
            .is_none()
        {
            return Err(reject(
                "enrolled native custody has ambiguous beneficial ownership",
            ));
        }
        Ok::<_, Error>(record)
    };
    let new_custody = || {
        // Only sealed native capabilities select Funding/Partition after checking
        // the derived destination. A caller-supplied address never selects them.
        if enrollment::beneficial_with_owner(
            world,
            destination.definition(),
            destination.account(),
            Some(world),
        )?
        .is_some()
            || world
                .asset_escrows()
                .iter()
                .any(|(_, record)| record.custody == *destination.account())
            || world
                .assets()
                .get(destination)
                .is_some_and(|value| !value.as_ref().is_zero())
        {
            return Err(reject(
                "new native custody must be unclassified and unfunded",
            ));
        }
        Ok(())
    };
    match purpose {
        NumericAssetTransferSourcePolicy::NativeEscrowFunding => {
            enrollment::require_active_beneficial_with_owner(world, source, Some(world))?;
            new_custody()
        }
        NumericAssetTransferSourcePolicy::NativeEscrowPartition => {
            native_source()?;
            new_custody()
        }
        NumericAssetTransferSourcePolicy::NativeEscrowReturn => {
            let record = native_source()?;
            if destination.account() != &record.seller {
                return Err(reject("retained return must name the exact original payer"));
            }
            // Returning existing ownership remains possible after institutional
            // revocation or an ordinary incoming freeze. No new credit is granted.
            Ok(())
        }
        NumericAssetTransferSourcePolicy::NativeEscrowCustody => {
            native_source()?;
            enrollment::require_active_beneficial_with_owner(world, destination, Some(world))
        }
        _ => {
            enrollment::require_active_beneficial_with_owner(world, source, Some(world))?;
            enrollment::require_active_beneficial_with_owner(world, destination, Some(world))
        }
    }
}
