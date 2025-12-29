use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use iroha_crypto::{Algorithm, KeyPair};
use iroha_data_model::events::time::{ExecutionTime, Schedule as TimeSchedule, TimeEventFilter};
use iroha_data_model::{
    isi::{InstructionBox, Register},
    metadata::Metadata,
    prelude::*,
    trigger::{
        Trigger, TriggerId,
        action::{Action, Repeats},
    },
};
use iroha_primitives::json::Json;
use norito::json;
use std::time::Duration;

fn main() {
    let keypair = KeyPair::from_seed(b"trigger-fixture-authority".to_vec(), Algorithm::Ed25519);
    let authority = AccountId::new("wonderland".parse().unwrap(), keypair.public_key().clone());

    let trigger_id: TriggerId = "mint_rose".parse().unwrap();

    let mut metadata = Metadata::default();
    metadata.insert("label".parse().unwrap(), Json::new("periodic".to_string()));

    let register_domain = Register::<Domain>::domain(Domain::new("triggerland".parse().unwrap()));
    let instruction_box: InstructionBox = register_domain.into();

    let action = Action::new(
        Executable::from(vec![instruction_box]),
        Repeats::Exactly(1),
        authority.clone(),
        TimeEventFilter::new(ExecutionTime::Schedule(
            TimeSchedule::starting_at(Duration::from_millis(1_735_000_000_000))
                .with_period(Duration::from_millis(60_000)),
        )),
    )
    .with_metadata(metadata);

    let trigger = Trigger::new(trigger_id, action);
    let instruction: InstructionBox = Register::trigger(trigger).into();

    let value = json::to_json_pretty(&instruction).unwrap();
    println!("{value}");
    let encoded = instruction.encode();
    println!("{}", BASE64.encode(encoded));
}
