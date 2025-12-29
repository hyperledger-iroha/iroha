use iroha_schema::IntoSchema;

#[derive(IntoSchema)]
#[schema(transparent)]
enum TransparentEnum {
    First(u64),
    Second(u64),
}

fn main() {}
