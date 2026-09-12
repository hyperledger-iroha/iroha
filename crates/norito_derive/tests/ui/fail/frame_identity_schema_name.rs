//! Payload derive spellings reject independent frame identities.
#[derive(norito::NoritoSerialize)]
#[norito(schema_name = "retired")]
struct Encode(u32);

#[derive(norito::NoritoDeserialize)]
#[norito(schema_name = "retired")]
struct Decode(u32);

#[derive(norito::SerializePayload)]
#[norito(schema_name = "retired")]
struct EncodePayload(u32);

#[derive(norito::DeserializePayload)]
#[norito(schema_name = "retired")]
struct DecodePayload(u32);

fn main() {}
