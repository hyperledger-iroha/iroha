#![cfg(feature = "json")]
//! Property test: `Parser::parse_string` roundtrips strings quoted with Norito helpers.

use norito::json::{Parser, write_json_string};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, .. ProptestConfig::default() })]
    #[test]
    fn parser_parse_string_matches_serde(ref s in "(?s).{0,64}") {
        let mut quoted = String::new();
        write_json_string(s.as_str(), &mut quoted);
        let mut p = Parser::new(&quoted);
        let got = p.parse_string().expect("parse string");
        prop_assert_eq!(got, s.as_str());
    }
}
