//! Small helper binary that validates hard-coded BLS keypairs.

use std::error::Error;

use iroha_crypto::{KeyPair, PrivateKey, PublicKey};

const BLS_PAIRS: [(&str, &str); 4] = [
    (
        "ea01308683839424703437C5C8701F3A92D76E228337D2327602B8C0CED667A6ED7F8AD6360948B24FC21849E77411A0975B6D",
        "892620D4F6A246F90A95B7BA4AACF1948DA2F9C56A58E13E0590055BD0BA6951F5446B",
    ),
    (
        "ea0130839A8AE65879CCDF6FD59A099A72E0BA123244BDF5518D3093DCF741C1675006DE03DD71C2D8C20B4DF4656B9EA156DF",
        "892620E0550F7A83A83E8770D0A3D956EB4DC23739B546F30AFAD88DDB83AB3A53F36A",
    ),
    (
        "ea013096954BA5FE505AD52FB697D1728D99052CC088A582145C617A5D591044FB970ADBF6BE0A05E181DBF0D0D9133C6D3823",
        "892620A95B3A50F63724F370CA2345FEC0C8B32BE754013F2D6047E36DAED519B7032B",
    ),
    (
        "ea0130A710630B599B3289A1FFB8E7B14103F4927E9A2002885FBBCD43FA777EB441B2635308C83D6F0BD6775CB6D58F60BC05",
        "89262081623DF6E560E259917D2AE1740FE840190AABBAC16083672E82FD99E184455D",
    ),
];

fn validate_bls_pairs(pairs: &[(&str, &str)]) -> Result<(), Box<dyn Error>> {
    for (idx, (pub_hex, priv_hex)) in pairs.iter().enumerate() {
        let sk = priv_hex
            .parse::<PrivateKey>()
            .map_err(|err| format!("peer{idx} private parse: {err}"))?;
        let pk = pub_hex
            .parse::<PublicKey>()
            .map_err(|err| format!("peer{idx} public parse: {err}"))?;
        KeyPair::new(pk, sk).map_err(|err| format!("peer{idx}: mismatch: {err}"))?;
        println!("peer{idx}: OK");
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    validate_bls_pairs(&BLS_PAIRS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hard_coded_bls_pairs_validate() {
        validate_bls_pairs(&BLS_PAIRS).expect("hard-coded BLS keypairs must match");
    }

    #[test]
    fn mismatched_bls_pair_fails_closed() {
        let mismatched = [(BLS_PAIRS[0].0, BLS_PAIRS[1].1)];
        let err = validate_bls_pairs(&mismatched).expect_err("mismatched BLS pair must fail");

        assert!(
            err.to_string().contains("peer0: mismatch"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn malformed_bls_pair_fails_closed() {
        let malformed = [("not-a-public-key", BLS_PAIRS[0].1)];
        let err = validate_bls_pairs(&malformed).expect_err("malformed BLS pair must fail");

        assert!(
            err.to_string().contains("peer0 public parse"),
            "unexpected error: {err}"
        );
    }
}
