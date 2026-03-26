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
        "soraカタカナ..."
            .parse()
            .unwrap();
    println!("ID: {}", account_id.signatory);
}
