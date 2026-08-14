use iroha_config_base::{ReadConfig, WithOrigin};
#[derive(ReadConfig)]
struct Test {
    required: u64,
    required_with_origin: WithOrigin<u64>,
    optional: Option<u64>,
    optional_with_origin: Option<WithOrigin<u64>>,
    #[config(default)]
    with_default: bool,
    #[config(default = "true")]
    with_default_expr: bool,
    #[config(env = "FROM_ENV")]
    from_env: String,
    #[config(nested)]
    nested: Nested,
    #[config(env = "TEST", default = "true")]
    with_default_expr_and_env: bool,
    #[config(key = "canonical_parameter")]
    renamed_parameter: u64,
    #[config(nested, key = "canonical_nested")]
    renamed_nested: Nested,
}
#[derive(ReadConfig)]
struct Nested {
    foo: Option<u32>,
}
pub fn main() {}
