use iroha_config_base::ReadConfig;

#[derive(ReadConfig)]
struct Test {
    #[config(env_only)]
    foo: u64,
}

pub fn main() {}
