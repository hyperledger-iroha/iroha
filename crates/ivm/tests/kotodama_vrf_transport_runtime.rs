//! Runtime coverage for Kotodama VRF request transport normalization.

use std::collections::BTreeMap;

use blstrs::{G1Projective, G2Projective, Scalar};
use group::{Curve, Group};
use iroha_crypto::Hash;
use iroha_data_model::prelude::Name;
use iroha_primitives::json::Json;
use ivm::{
    IVM, ProgramMetadata,
    host::DefaultHost,
    pointer_abi::PointerType,
    vrf::{VrfVerifyBatchRequest, VrfVerifyRequest},
};

fn tlv(pointer_type: PointerType, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(7 + payload.len() + Hash::LENGTH);
    out.extend_from_slice(&(pointer_type as u16).to_be_bytes());
    out.push(1);
    out.extend_from_slice(
        &u32::try_from(payload.len())
            .expect("test payload fits u32")
            .to_be_bytes(),
    );
    out.extend_from_slice(payload);
    out.extend_from_slice(Hash::new(payload).as_ref());
    out
}

fn argument_record_tlv(entrypoint: &ivm::EmbeddedEntrypointDescriptor, payload: &Json) -> Vec<u8> {
    let schema = entrypoint
        .argument_schema
        .as_ref()
        .expect("parameterized VRF entrypoint argument schema");
    let record =
        ivm::encode_argument_record_from_json(schema, payload).expect("encode argument record");
    tlv(PointerType::NoritoBytes, &record)
}

fn compile_and_run(source: &str, arguments: Option<&Json>) -> IVM {
    let code = ivm::kotodama::compiler::Compiler::new()
        .compile_source(source)
        .expect("compile VRF transport contract");
    let parsed = ProgramMetadata::parse(&code).expect("parse VRF transport metadata");
    let run = parsed
        .contract_interface
        .as_ref()
        .expect("embedded contract interface")
        .entrypoints
        .iter()
        .find(|entrypoint| entrypoint.name == "run")
        .expect("run entrypoint descriptor");
    let entry_pc = u64::try_from(parsed.prefix_len()).expect("prefix fits u64") + run.entry_pc;

    let host = if let Some(arguments) = arguments {
        let key: Name = "trigger_event_json".parse().expect("public input key");
        DefaultHost::new()
            .with_public_inputs(BTreeMap::from([(key, argument_record_tlv(run, arguments))]))
    } else {
        DefaultHost::new()
    };
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&code).expect("load VRF transport contract");
    vm.set_program_counter(entry_pc)
        .expect("select run entrypoint");
    vm.set_host(host);
    vm.run().expect("execute VRF transport contract");
    vm
}

fn valid_vrf_request() -> (Vec<u8>, [u8; 32]) {
    const DST: &[u8] = b"BLS12381G2_XMD:SHA-256_SSWU_RO_IROHA_VRF_V1";

    let mut seed = [0_u8; 32];
    for (index, byte) in seed.iter_mut().enumerate() {
        *byte = u8::try_from(index + 1).expect("fixture index fits u8");
    }
    let secret = Scalar::from_bytes_be(&seed).expect("canonical non-zero fixture scalar");
    let public_key = (G1Projective::generator() * secret)
        .to_affine()
        .to_compressed();
    let chain_id = b"kotodama-vrf-transport";
    let input = b"entrypoint-bytes";
    let mut preimage = Vec::new();
    preimage.extend_from_slice(b"iroha:vrf:v1:input|");
    preimage.extend_from_slice(chain_id);
    preimage.push(b'|');
    preimage.extend_from_slice(input);
    let message: [u8; 32] = Hash::new(&preimage).into();
    let proof = (G2Projective::hash_to_curve(&message, DST, &[]) * secret)
        .to_affine()
        .to_compressed();

    let mut output_preimage = Vec::new();
    output_preimage.extend_from_slice(b"iroha:vrf:v1:output");
    output_preimage.extend_from_slice(&proof);
    let expected: [u8; 32] = Hash::new(&output_preimage).into();
    let request = VrfVerifyRequest {
        variant: 1,
        pk: public_key.to_vec(),
        proof: proof.to_vec(),
        chain_id: chain_id.to_vec(),
        input: input.to_vec(),
    };
    (
        norito::to_bytes(&request).expect("encode canonical VRF request"),
        expected,
    )
}

#[test]
fn vrf_verify_accepts_entrypoint_bytes_and_returns_expected_blob() {
    let source = r#"
seiyaku VrfEntrypointBytes {
  view fn run(bytes request) -> bytes {
    return crypto::vrf::verify(request: request);
  }
}
"#;
    let (request, expected) = valid_vrf_request();
    let request_hex = format!("0x{}", hex::encode(request));
    let arguments = Json::from(norito::json!({
        "request": request_hex,
    }));
    let vm = compile_and_run(source, Some(&arguments));

    let output = vm
        .validate_tlv(vm.register(10))
        .expect("VRF output pointer");
    assert_eq!(output.type_id, PointerType::Blob);
    assert_eq!(output.payload, expected);
}

#[test]
fn vrf_verify_batch_rejects_empty_entrypoint_bytes() {
    let source = r#"
seiyaku VrfBatchEntrypointBytes {
  view fn run(bytes batch) -> bytes {
    return crypto::vrf::verify_batch(batch);
  }
}
"#;
    let request = norito::to_bytes(&VrfVerifyBatchRequest { items: Vec::new() })
        .expect("encode empty VRF batch request");
    let request_hex = format!("0x{}", hex::encode(request));
    let arguments = Json::from(norito::json!({
        "batch": request_hex,
    }));
    let vm = compile_and_run(source, Some(&arguments));

    assert_eq!(vm.register(10), 0);
    assert_eq!(vm.register(11), 9, "empty batch bound status");
    assert_eq!(vm.register(12), u64::MAX);
}

#[test]
fn malformed_vrf_bytes_literal_reaches_decode_error() {
    let source = r#"
seiyaku MalformedVrfLiteral {
  view fn run() -> bytes {
    return crypto::vrf::verify(request: b"\x01\x02\x03");
  }
}
"#;
    let vm = compile_and_run(source, None);

    assert_eq!(vm.register(10), 0);
    assert_eq!(vm.register(11), 2, "malformed Norito request status");
    assert_ne!(
        vm.register(11),
        1,
        "request must not fail as a Blob type error"
    );
}
