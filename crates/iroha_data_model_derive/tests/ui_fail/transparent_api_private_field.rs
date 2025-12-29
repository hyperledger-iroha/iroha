use account::AccountId;

mod account {
    use core::str::FromStr;

    pub struct AccountId {
        signatory: String,
    }

    impl FromStr for AccountId {
        type Err = ();
        fn from_str(_: &str) -> Result<Self, Self::Err> {
            Ok(Self {
                signatory: String::new(),
            })
        }
    }
}

fn main() {
    let account_id: AccountId =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
            .parse()
            .unwrap();
    println!("ID: {}", account_id.signatory);
}
