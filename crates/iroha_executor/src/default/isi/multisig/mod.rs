use iroha_executor_data_model::isi::multisig::*;
use iroha_smart_contract::data_model::{
    account::Account,
    query::{
        error::QueryExecutionFail,
        json::{EqualsCondition, PredicateJson},
    },
};
use norito::json::Value;

use super::*;
use crate::data_model::{
    prelude::Json,
    query::{builder::SingleQueryError, dsl::CompoundPredicate, error::FindError},
};
mod account;
mod transaction;

impl VisitExecute for MultisigInstructionBox {
    fn visit_execute<V: Execute + Visit + ?Sized>(self, executor: &mut V) {
        visit_instruction(self, executor);
    }
}

pub(super) fn visit_instruction<V: Execute + Visit + ?Sized>(
    instruction: MultisigInstructionBox,
    executor: &mut V,
) {
    match instruction {
        MultisigInstructionBox::Register(instruction) => instruction.visit_execute(executor),
        MultisigInstructionBox::Propose(instruction) => instruction.visit_execute(executor),
        MultisigInstructionBox::Approve(instruction) => instruction.visit_execute(executor),
    }
}

const DELIMITER: char = '/';
const MULTISIG: &str = "multisig";
const MULTISIG_SIGNATORY: &str = "MULTISIG_SIGNATORY";

fn spec_key() -> Name {
    format!("{MULTISIG}{DELIMITER}spec").parse().unwrap()
}

fn home_domain_key() -> Name {
    format!("{MULTISIG}{DELIMITER}home_domain").parse().unwrap()
}

fn proposal_key(hash: &HashOf<Vec<InstructionBox>>) -> Name {
    format!("{MULTISIG}{DELIMITER}proposals{DELIMITER}{hash}")
        .parse()
        .unwrap()
}

fn account_id_predicate(
    account_id: &AccountId,
) -> Result<CompoundPredicate<Account>, ValidationFail> {
    let mut predicate = PredicateJson::default();
    predicate.equals.push(EqualsCondition::new(
        "id",
        Value::String(account_id.to_string()),
    ));

    predicate.into_compound::<Account>().map_err(|err| {
        ValidationFail::InternalError(format!("failed to encode account predicate: {err}"))
    })
}

pub(super) fn fetch_account_by_id<V: Execute + Visit + ?Sized>(
    account_id: &AccountId,
    executor: &V,
) -> Result<Account, ValidationFail> {
    let predicate = account_id_predicate(account_id)?;

    executor
        .host()
        .query(FindAccounts)
        .filter(predicate)
        .execute_single()
        .map_err(|error| match error {
            SingleQueryError::QueryError(e) => e,
            SingleQueryError::ExpectedOneGotNone => ValidationFail::QueryFailed(
                QueryExecutionFail::Find(FindError::Account(account_id.clone())),
            ),
            SingleQueryError::ExpectedOneGotMany | SingleQueryError::ExpectedOneOrZeroGotMany => {
                ValidationFail::InternalError(format!(
                    "multiple accounts found with the same id `{account_id}`"
                ))
            }
        })
}

pub(super) fn load_account_metadata<V: Execute + Visit + ?Sized>(
    account_id: &AccountId,
    key: &Name,
    executor: &V,
) -> Result<Json, ValidationFail> {
    fetch_account_by_id(account_id, executor)?
        .metadata()
        .get(key)
        .cloned()
        .ok_or_else(|| {
            ValidationFail::QueryFailed(QueryExecutionFail::Find(FindError::MetadataKey(
                key.clone(),
            )))
        })
}

pub(super) fn multisig_home_domain<V: Execute + Visit + ?Sized>(
    multisig_account: &AccountId,
    executor: &V,
) -> Result<DomainId, ValidationFail> {
    load_account_metadata(multisig_account, &home_domain_key(), executor)?
        .try_into_any_norito()
        .map_err(metadata_conversion_error)
}

fn multisig_role_for(home_domain: &DomainId, account: &AccountId) -> RoleId {
    let suffix = account
        .canonical_i105()
        .unwrap_or_else(|_| HashOf::new(account).to_string());
    format!("{MULTISIG_SIGNATORY}{DELIMITER}{home_domain}{DELIMITER}{suffix}")
        .parse()
        .unwrap()
}

pub(super) fn is_multisig<V: Execute + Visit + ?Sized>(
    account: &AccountId,
    executor: &V,
) -> Result<bool, ValidationFail> {
    match load_account_metadata(account, &spec_key(), executor) {
        Ok(_) => Ok(true),
        Err(ValidationFail::QueryFailed(QueryExecutionFail::Find(
            FindError::Account(_) | FindError::MetadataKey(_),
        ))) => Ok(false),
        Err(err) => Err(err),
    }
}

pub(super) fn multisig_spec<V: Execute + Visit + ?Sized>(
    multisig_account: &AccountId,
    executor: &V,
) -> Result<MultisigSpec, ValidationFail> {
    let key = spec_key();
    load_account_metadata(multisig_account, &key, executor)?
        .try_into_any_norito()
        .map_err(metadata_conversion_error)
}

#[expect(clippy::needless_pass_by_value)]
pub(super) fn metadata_conversion_error(err: norito::Error) -> ValidationFail {
    ValidationFail::QueryFailed(QueryExecutionFail::Conversion(format!(
        "multisig account metadata malformed:\n{err}"
    )))
}
