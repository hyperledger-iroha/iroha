use iroha_config_base::ReadConfig;
#[derive(ReadConfig)]
struct Test {
    canonical: u64,
    #[config(key = "canonical")]
    renamed: u64,
}
fn main() {}
