//! Query cursors are host-managed and must not be constructed directly.

use iroha_smart_contract::{QueryCursor, data_model::query::parameters::ForwardCursor};

fn host_cursor() -> ForwardCursor {
    panic!("trybuild cases are compiled but not executed")
}

fn main() {
    let _cursor = QueryCursor {
        cursor: host_cursor(),
    };
}
