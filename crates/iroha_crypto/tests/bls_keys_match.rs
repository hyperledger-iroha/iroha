//! Validate that stored BLS keypair fixtures line up with the expected public keys.
#![cfg(feature = "bls")]
use iroha_crypto::{BlsNormal, BlsSmall, KeyGenOption, KeyPair, PrivateKey, PublicKey};
#[test]
fn bls_keys_match_localnet_soranexus() {
    // Sanity-check the localnet BLS keypairs used by the Sora Nexus configs.
    let keypairs = [
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
    for (public_hex, private_hex) in keypairs {
        let private = private_hex
            .parse::<PrivateKey>()
            .expect("parse private key");
        let public = public_hex.parse::<PublicKey>().expect("parse public key");
        KeyPair::new(public, private)
            .unwrap_or_else(|e| panic!("keypair mismatch for {public_hex}: {e}"));
    }
}

#[test]
fn seeded_signatures_match_first_release_golden_bytes() {
    let seed = b"iroha-bls-signature-golden-v1";
    let message = b"iroha first release BLS signature";
    let (normal_public, normal_private) =
        BlsNormal::try_keypair(KeyGenOption::UseSeed(seed.to_vec())).expect("normal keypair");
    let normal = BlsNormal::try_sign(message, &normal_private).expect("normal signature");
    BlsNormal::verify(message, &normal, &normal_public).expect("normal signature verifies");

    let (small_public, small_private) =
        BlsSmall::try_keypair(KeyGenOption::UseSeed(seed.to_vec())).expect("small keypair");
    let small = BlsSmall::try_sign(message, &small_private).expect("small signature");
    BlsSmall::verify(message, &small, &small_public).expect("small signature verifies");

    panic!(
        "normal={}\nsmall={}",
        hex::encode_upper(normal),
        hex::encode_upper(small)
    );
}
