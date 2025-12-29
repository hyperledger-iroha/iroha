use iroha_data_model::{
    isi::error::{InstructionExecutionError as Error, InvalidParameterError},
    parameter::CustomParameterId,
    prelude::*,
};

use crate::state::StateTransaction;

/// Default maximum size in bytes for JSON payloads (1 MiB).
pub const DEFAULT_JSON_LIMIT: usize = 1_048_576;

/// Enforce a maximum JSON size for values, consulting a custom parameter if present.
///
/// # Errors
/// Returns an error when the value exceeds the configured or default maximum size.
pub fn enforce_json_size(
    state_transaction: &mut StateTransaction<'_, '_>,
    value: &Json,
    param_name: &str,
    default: usize,
) -> Result<(), Error> {
    let params = state_transaction.world.parameters.get();
    let limit = if let Ok(name) = core::str::FromStr::from_str(param_name)
        && let Some(custom) = params.custom().get(&CustomParameterId(name))
        && let Ok(num) = custom.payload().try_into_any_norito::<u64>()
    {
        usize::try_from(num).unwrap_or(usize::MAX)
    } else {
        default
    };
    if value.as_ref().len() > limit {
        return Err(Error::InvalidParameter(
            InvalidParameterError::SmartContract(format!(
                "Payload too large for {}: {} > {} bytes",
                param_name,
                value.as_ref().len(),
                limit
            )),
        ));
    }
    Ok(())
}
