use iroha_schema::IntoSchema;

#[derive(IntoSchema)]
enum NamedVariant {
    Foo { first: u8, second: u8 },
}

fn main() {}
