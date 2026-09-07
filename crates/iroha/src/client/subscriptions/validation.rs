//! Canonical subscription draft identity and instruction-scope validation.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    HasMetadata,
    account::AccountId,
    asset::{AssetBalancePolicy, AssetDefinition},
    isi::{ExecuteTrigger, Grant, InstructionBox, Register, RegisterBox, SetKeyValue, Unregister},
    metadata::Metadata,
    nft::{Nft, NftId},
    permission::Permission,
    subscription::{
        SUBSCRIPTION_METADATA_KEY, SUBSCRIPTION_PLAN_METADATA_KEY,
        SUBSCRIPTION_TRIGGER_REF_METADATA_KEY, SubscriptionStatus, SubscriptionTriggerRef,
        SubscriptionUsageDelta,
    },
    transaction::{
        Executable, FeePaymentIntent, TransactionAdmissionIntent, TransactionBuilder,
        TransactionPayload,
    },
    trigger::{Trigger, TriggerId},
};
use iroha_executor_data_model::permission::trigger::CanExecuteTrigger;
use iroha_primitives::json::Json;
use iroha_torii_shared::subscriptions::{
    SUBSCRIPTION_MUTATION_DRAFT_VERSION_V1, SubscriptionActionRequest, SubscriptionActionResponse,
    SubscriptionCancelMode, SubscriptionCreateRequest, SubscriptionCreateResponse,
    SubscriptionInstructionDraft, SubscriptionPlanCreateRequest, SubscriptionPlanCreateResponse,
    SubscriptionUsageRequest, SubscriptionUsageResponse,
};

use super::AccountClient;
use crate::{Error, Result};

pub(super) fn require(valid: bool, operation: &'static str, field: &'static str) -> Result<()> {
    if valid {
        Ok(())
    } else {
        Err(Error::ResponseBinding { operation, field })
    }
}

fn decode_error(operation: &'static str, error: impl std::fmt::Display) -> Error {
    Error::Decode {
        operation,
        details: error.to_string(),
    }
}

fn canonical_base64(encoded: &str, operation: &'static str) -> Result<Vec<u8>> {
    let decoded = STANDARD
        .decode(encoded)
        .map_err(|error| decode_error(operation, error))?;
    require(
        STANDARD.encode(&decoded) == encoded,
        operation,
        "canonical_base64",
    )?;
    Ok(decoded)
}

fn payload(
    account: &AccountClient,
    operation: &'static str,
    payload_b64: &str,
    signing_b64: &str,
) -> Result<TransactionPayload> {
    let bytes = canonical_base64(payload_b64, operation)?;
    let builder = TransactionBuilder::decode_payload(&bytes)
        .map_err(|error| decode_error(operation, error))?;
    require(
        builder.encode_payload() == bytes,
        operation,
        "canonical_transaction_payload",
    )?;
    require(
        builder.payload().network_id() == Some(account.network_id()),
        operation,
        "network_id",
    )?;
    require(
        builder.payload().authority() == account.authority(),
        operation,
        "authority",
    )?;
    require(builder.payload().metadata.is_empty(), operation, "metadata")?;
    require(
        builder.payload().attachments.is_none(),
        operation,
        "attachments",
    )?;
    require(
        builder.payload().admission_intent == TransactionAdmissionIntent::Ordinary,
        operation,
        "admission_intent",
    )?;
    // Torii quotes may change charge limits, which remain visible in the draft
    // for review. These routes always select authority payment without gas.
    require(
        builder
            .payload()
            .fee_payment
            .has_same_payer_and_gas_bound(&FeePaymentIntent::authority(Vec::new(), None)),
        operation,
        "fee_payer_and_gas_bound",
    )?;
    let message = canonical_base64(signing_b64, operation)?;
    require(
        message.as_slice() == HashOf::new(builder.payload()).as_ref(),
        operation,
        "signing_message_b64",
    )?;
    builder
        .into_payload()
        .map_err(|error| decode_error(operation, error))
}

pub(super) fn plan(
    account: &AccountClient,
    request: &SubscriptionPlanCreateRequest,
    response: &SubscriptionPlanCreateResponse,
) -> Result<TransactionPayload> {
    const OP: &str = "subscriptions.prepare_plan";
    require(!response.submitted, OP, "submitted")?;
    require(response.plan_id == request.plan_id, OP, "plan_id")?;
    let payload = payload(
        account,
        OP,
        &response.transaction_payload_b64,
        &response.signing_message_b64,
    )?;
    let expected = vec![
        InstructionBox::from(Register::asset_definition(AssetDefinition::numeric(
            request.plan_id.clone(),
            request.plan_id.to_string(),
            AssetBalancePolicy::Global,
            None,
        ))),
        InstructionBox::from(SetKeyValue::asset_definition(
            request.plan_id.clone(),
            SUBSCRIPTION_PLAN_METADATA_KEY
                .parse()
                .map_err(|error| decode_error(OP, error))?,
            Json::new(request.plan.clone()),
        )),
    ];
    require(
        payload.instructions() == &Executable::from(expected),
        OP,
        "plan_instructions",
    )?;
    Ok(payload)
}

fn resolved_trigger(
    prefix: &str,
    id: &NftId,
    explicit: Option<&TriggerId>,
    operation: &'static str,
) -> Result<TriggerId> {
    if let Some(explicit) = explicit {
        return Ok(explicit.clone());
    }
    format!(
        "{prefix}{}",
        hex::encode(Hash::new(id.to_string()).as_ref())
    )
    .parse()
    .map_err(|error| decode_error(operation, error))
}

pub(super) fn usage(
    account: &AccountClient,
    id: &NftId,
    request: &SubscriptionUsageRequest,
    response: &SubscriptionUsageResponse,
) -> Result<TransactionPayload> {
    const OP: &str = "subscriptions.prepare_usage";
    require(!response.submitted, OP, "submitted")?;
    require(response.subscription_id == *id, OP, "subscription_id")?;
    let payload = payload(
        account,
        OP,
        &response.transaction_payload_b64,
        &response.signing_message_b64,
    )?;
    let trigger = resolved_trigger("sub_usage_", id, request.usage_trigger_id.as_ref(), OP)?;
    let expected = InstructionBox::from(ExecuteTrigger::new(trigger).with_args(
        SubscriptionUsageDelta {
            subscription_nft_id: id.clone(),
            unit_key: request.unit_key.clone(),
            delta: request.delta.clone(),
        },
    ));
    require(
        payload.instructions() == &Executable::from(vec![expected]),
        OP,
        "usage_instructions",
    )?;
    Ok(payload)
}

fn instructions(
    operation: &'static str,
    drafts: &[SubscriptionInstructionDraft],
) -> Result<Vec<InstructionBox>> {
    drafts
        .iter()
        .map(|draft| {
            let bytes =
                hex::decode(&draft.payload_hex).map_err(|error| decode_error(operation, error))?;
            require(
                hex::encode(&bytes) == draft.payload_hex,
                operation,
                "canonical_instruction_hex",
            )?;
            let instruction =
                iroha_data_model::isi::decode_instruction_from_pair(&draft.wire_id, &bytes)
                    .map_err(|error| decode_error(operation, error))?;
            let canonical = iroha_data_model::isi::framed_instruction_payload(&instruction);
            require(
                canonical.as_ref().is_some_and(|(wire_id, payload)| {
                    *wire_id == draft.wire_id && *payload == bytes
                }),
                operation,
                "canonical_instruction_frame",
            )?;
            Ok(instruction)
        })
        .collect()
}

fn registered_trigger<'a>(
    instruction: &'a InstructionBox,
    operation: &'static str,
    id: &TriggerId,
    authority: &AccountId,
) -> Result<&'a Trigger> {
    let Some(RegisterBox::Trigger(register)) = instruction.as_any().downcast_ref::<RegisterBox>()
    else {
        return Err(Error::ResponseBinding {
            operation,
            field: "trigger_registration",
        });
    };
    let trigger = register.object();
    require(trigger.id() == id, operation, "trigger_id")?;
    require(
        trigger.action().authority() == authority,
        operation,
        "trigger_authority",
    )?;
    require(
        matches!(trigger.action().executable(), Executable::Ivm(_)),
        operation,
        "trigger_executable_kind",
    )?;
    require(
        trigger.action().retry_policy().is_none(),
        operation,
        "trigger_retry_policy",
    )?;
    Ok(trigger)
}

fn billing_trigger(
    instruction: &InstructionBox,
    operation: &'static str,
    id: &TriggerId,
    authority: &AccountId,
    subscription_id: &NftId,
    charge_ms: u64,
) -> Result<()> {
    use iroha_data_model::{
        events::{
            EventFilterBox,
            time::{ExecutionTime, Schedule, TimeEventFilter},
        },
        trigger::action::Repeats,
    };
    let trigger = registered_trigger(instruction, operation, id, authority)?;
    let action = trigger.action();
    let filter = EventFilterBox::Time(TimeEventFilter(ExecutionTime::Schedule(Schedule {
        start_ms: charge_ms,
        period_ms: None,
    })));
    require(
        action.filter() == &filter,
        operation,
        "billing_trigger_schedule",
    )?;
    require(
        action.repeats() == Repeats::Exactly(1),
        operation,
        "billing_trigger_repeats",
    )?;
    let mut metadata = Metadata::default();
    metadata.insert(
        SUBSCRIPTION_TRIGGER_REF_METADATA_KEY
            .parse()
            .map_err(|error| decode_error(operation, error))?,
        Json::new(SubscriptionTriggerRef {
            subscription_nft_id: subscription_id.clone(),
        }),
    );
    require(
        action.metadata() == &metadata,
        operation,
        "billing_trigger_subscription",
    )
}

pub(super) fn create(
    request: &SubscriptionCreateRequest,
    response: &SubscriptionCreateResponse,
) -> Result<Vec<InstructionBox>> {
    const OP: &str = "subscriptions.prepare";
    require(
        response.version == SUBSCRIPTION_MUTATION_DRAFT_VERSION_V1,
        OP,
        "version",
    )?;
    require(response.action == "create", OP, "action")?;
    require(response.authority == request.authority, OP, "authority")?;
    require(
        response.subscription_id == request.subscription_id,
        OP,
        "subscription_id",
    )?;
    require(response.plan_id == request.plan_id, OP, "plan_id")?;
    require(
        request
            .first_charge_ms
            .is_none_or(|charge| charge == response.first_charge_ms),
        OP,
        "first_charge_ms",
    )?;
    require(
        response.billing_trigger_id
            == resolved_trigger(
                "sub_bill_",
                &request.subscription_id,
                request.billing_trigger_id.as_ref(),
                OP,
            )?,
        OP,
        "billing_trigger_id",
    )?;
    if response.usage_trigger_id.is_some() || request.usage_trigger_id.is_some() {
        require(
            response.usage_trigger_id.as_ref()
                == Some(&resolved_trigger(
                    "sub_usage_",
                    &request.subscription_id,
                    request.usage_trigger_id.as_ref(),
                    OP,
                )?),
            OP,
            "usage_trigger_id",
        )?;
    }
    let state = &response.resulting_subscription;
    require(
        state.subscriber == request.authority && state.plan_id == request.plan_id,
        OP,
        "resulting_subscription_identity",
    )?;
    require(
        state.billing_trigger_id == response.billing_trigger_id
            && state.next_charge_ms == response.first_charge_ms,
        OP,
        "resulting_subscription_billing",
    )?;
    require(
        state.status == SubscriptionStatus::Active
            && !state.cancel_at_period_end
            && state.cancel_at_ms.is_none()
            && state.failure_count == 0
            && state.usage_accumulated.is_empty(),
        OP,
        "initial_subscription_state",
    )?;
    let grant = response.usage_trigger_id.is_some()
        && request.grant_usage_to_provider.unwrap_or(true)
        && state.provider != request.authority;
    require(
        response.provider_usage_grant_included == grant,
        OP,
        "provider_usage_grant_included",
    )?;
    let decoded = instructions(OP, &response.tx_instructions)?;
    require(
        decoded.len() == 2 + usize::from(response.usage_trigger_id.is_some()) + usize::from(grant),
        OP,
        "instruction_count",
    )?;
    let mut metadata = Metadata::default();
    metadata.insert(
        SUBSCRIPTION_METADATA_KEY
            .parse()
            .map_err(|error| decode_error(OP, error))?,
        Json::new(state.clone()),
    );
    let nft = InstructionBox::from(Register::nft(Nft::new(
        request.subscription_id.clone(),
        metadata,
    )));
    require(decoded[0] == nft, OP, "subscription_registration")?;
    billing_trigger(
        &decoded[1],
        OP,
        &response.billing_trigger_id,
        &request.authority,
        &request.subscription_id,
        response.first_charge_ms,
    )?;
    if let Some(id) = &response.usage_trigger_id {
        use iroha_data_model::{
            events::{EventFilterBox, execute_trigger::ExecuteTriggerEventFilter},
            trigger::action::Repeats,
        };
        let trigger = registered_trigger(&decoded[2], OP, id, &request.authority)?;
        let expected = EventFilterBox::ExecuteTrigger(
            ExecuteTriggerEventFilter::new()
                .for_trigger(id.clone())
                .under_authority(request.authority.clone()),
        );
        require(
            trigger.action().filter() == &expected
                && trigger.action().repeats() == Repeats::Indefinitely
                && trigger.action().metadata().is_empty(),
            OP,
            "usage_trigger_contract",
        )?;
        if grant {
            let permission: Permission = CanExecuteTrigger {
                trigger: id.clone(),
            }
            .into();
            let expected = InstructionBox::from(Grant::account_permission(
                permission,
                state.provider.clone(),
            ));
            require(decoded[3] == expected, OP, "usage_permission")?;
        }
    }
    Ok(decoded)
}

pub(super) fn action(
    operation: &'static str,
    action: &'static str,
    id: &NftId,
    request: &SubscriptionActionRequest,
    response: &SubscriptionActionResponse,
) -> Result<Vec<InstructionBox>> {
    require(
        response.version == SUBSCRIPTION_MUTATION_DRAFT_VERSION_V1,
        operation,
        "version",
    )?;
    require(
        response.authority == request.authority,
        operation,
        "authority",
    )?;
    require(
        response.subscription_id == *id,
        operation,
        "subscription_id",
    )?;
    require(
        response.action
            == if action == "charge-now" {
                "charge_now"
            } else {
                action
            },
        operation,
        "action",
    )?;
    let details = &response.details;
    let state = &details.resulting_subscription;
    require(
        details.cancel_mode == request.cancel_mode,
        operation,
        "cancel_mode",
    )?;
    require(
        state.subscriber == request.authority
            && state.billing_trigger_id == details.billing_trigger_id,
        operation,
        "resulting_subscription_identity",
    )?;
    if matches!(action, "resume" | "charge-now") {
        require(
            details.effective_charge_ms == Some(state.next_charge_ms)
                && request
                    .charge_at_ms
                    .is_none_or(|charge| details.effective_charge_ms == Some(charge)),
            operation,
            "effective_charge_ms",
        )?;
    } else {
        require(
            details.effective_charge_ms.is_none(),
            operation,
            "effective_charge_ms",
        )?;
    }
    let valid_state = match action {
        "pause" => state.status == SubscriptionStatus::Paused,
        "resume" => state.status == SubscriptionStatus::Active && state.failure_count == 0,
        "charge-now" => matches!(
            state.status,
            SubscriptionStatus::Active | SubscriptionStatus::PastDue
        ),
        "cancel" => match request.cancel_mode {
            Some(SubscriptionCancelMode::Immediate) => {
                state.status == SubscriptionStatus::Canceled
                    && !state.cancel_at_period_end
                    && state.cancel_at_ms.is_none()
            }
            Some(SubscriptionCancelMode::PeriodEnd) => {
                matches!(
                    state.status,
                    SubscriptionStatus::Active | SubscriptionStatus::PastDue
                ) && state.cancel_at_period_end
                    && state.cancel_at_ms == Some(state.current_period_end_ms)
            }
            None => false,
        },
        "keep" => {
            !state.cancel_at_period_end
                && state.cancel_at_ms.is_none()
                && !matches!(
                    state.status,
                    SubscriptionStatus::Canceled | SubscriptionStatus::Suspended
                )
        }
        _ => false,
    };
    require(valid_state, operation, "resulting_subscription_state")?;
    let (remove, register) = match details.billing_trigger_operation.as_str() {
        "none" => (false, false),
        "unregister" => (true, false),
        "register" => (false, true),
        "replace" => (true, true),
        _ => {
            return Err(Error::ResponseBinding {
                operation,
                field: "billing_trigger_operation",
            });
        }
    };
    require(
        register == matches!(action, "resume" | "charge-now"),
        operation,
        "billing_trigger_operation",
    )?;
    if action == "keep" || request.cancel_mode == Some(SubscriptionCancelMode::PeriodEnd) {
        require(!remove, operation, "billing_trigger_operation")?;
    }
    let decoded = instructions(operation, &response.tx_instructions)?;
    require(
        decoded.len() == 1 + usize::from(remove) + usize::from(register),
        operation,
        "instruction_count",
    )?;
    let update = InstructionBox::from(SetKeyValue::nft(
        id.clone(),
        SUBSCRIPTION_METADATA_KEY
            .parse()
            .map_err(|error| decode_error(operation, error))?,
        Json::new(state.clone()),
    ));
    require(
        decoded[0] == update,
        operation,
        "subscription_state_instruction",
    )?;
    if remove {
        require(
            decoded[1]
                == InstructionBox::from(Unregister::trigger(details.billing_trigger_id.clone())),
            operation,
            "billing_trigger_removal",
        )?;
    }
    if register {
        billing_trigger(
            &decoded[1 + usize::from(remove)],
            operation,
            &details.billing_trigger_id,
            &request.authority,
            id,
            state.next_charge_ms,
        )?;
    }
    Ok(decoded)
}
