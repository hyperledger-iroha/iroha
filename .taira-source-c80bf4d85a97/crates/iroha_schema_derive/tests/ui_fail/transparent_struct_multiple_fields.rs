use iroha_schema::IntoSchema;

#[derive(IntoSchema)]
#[schema(transparent)]
struct Wrapper {
    first: u32,
    second: u32,
}

fn main() {}
