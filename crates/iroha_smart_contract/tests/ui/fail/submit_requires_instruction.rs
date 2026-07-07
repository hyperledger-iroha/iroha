//! Host submission must be limited to built-in Iroha instructions.

use iroha_smart_contract::Iroha;

fn main() {
    let host = Iroha;
    let value = "not an instruction".to_owned();
    let _result = host.submit(&value);
}
