use iroha_data_model_derive::model_single;
model_single! {
    #[cfg_attr(feature = "std", ffi_type)]
    pub struct WrongGate;
}
fn main() {}
