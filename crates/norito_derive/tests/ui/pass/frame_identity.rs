//! Frame ownership is separate from field codecs and generic payload bounds.
use norito::{NoritoDeserialize, NoritoSchema, NoritoSerialize};

#[derive(Debug, PartialEq, Eq, norito::SerializePayload, norito::DeserializePayload)]
struct Field(u32);

#[derive(NoritoSerialize, NoritoDeserialize)]
struct Generic<T>(T);

#[derive(Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "example::Envelope", frame = "example.envelope")]
struct Envelope {
    field: Field,
}

#[derive(Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito_schema(name = "example::Message")]
enum Message {
    Unit,
    Value(Field),
}

fn require_payload<T: norito::SerializePayload + for<'a> norito::DeserializePayload<'a>>() {}
fn require_frame<T: norito::NoritoSerialize + for<'a> norito::NoritoDeserialize<'a>>() {}

fn main() {
    require_payload::<Field>();
    require_payload::<Generic<Field>>();
    require_frame::<Envelope>();
    require_frame::<Option<Envelope>>();
    require_frame::<Vec<Envelope>>();
    require_frame::<Message>();
    assert_eq!(Envelope::nominal_name(), "example::Envelope");
    assert_eq!(Envelope::frame_name(), "example.envelope");
    let value = Envelope { field: Field(7) };
    let bytes = norito::to_bytes(&value).expect("declared outer frame with a schema-free field");
    let decoded: Envelope =
        norito::decode_from_bytes(&bytes).expect("declared outer reconstruction");
    assert_eq!(decoded, value);
    for value in [Message::Unit, Message::Value(Field(9))] {
        let bytes = norito::to_bytes(&value).expect("declared enum frame");
        let decoded: Message =
            norito::decode_from_bytes(&bytes).expect("declared enum reconstruction");
        assert_eq!(decoded, value);
    }
}
