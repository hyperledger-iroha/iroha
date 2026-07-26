//! Parameter module split into system and custom parameters.

pub mod custom;
pub mod system;

pub(crate) use custom::CustomParameters;
pub use custom::{CustomParameter, CustomParameterId};
pub use system::{
    BlockParameter, BlockParameters, Parameter, Parameters, SmartContractParameter,
    SmartContractParameters, SumeragiParameter, SumeragiParameters, TransactionParameter,
    TransactionParameters,
};

pub mod prelude {
    //! Prelude: re-export of most commonly used traits, structs and macros in this crate.
    pub use super::{Parameter, Parameters, SmartContractParameters, TransactionParameters};
}
