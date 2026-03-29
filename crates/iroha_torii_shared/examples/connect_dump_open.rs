use iroha_torii_shared::connect::{
    AppMeta, ConnectControlV1, ConnectFrameV1, Constraints, Dir, FrameKind, PermissionsV1,
    encode_connect_frame_bare,
};

fn main() {
    let frame = ConnectFrameV1 {
        sid: [0xAB; 32],
        dir: Dir::AppToWallet,
        seq: 1,
        kind: FrameKind::Control(ConnectControlV1::Open {
            app_pk: [0x33; 32],
            app_meta: Some(AppMeta {
                name: "SoraSwap".into(),
                url: Some("https://example.test/".into()),
                icon_hash: None,
            }),
            constraints: Constraints {
                chain_id: "809574f5-fee7-5e69-bfcf-52451e42d50f".into(),
            },
            permissions: Some(PermissionsV1 {
                methods: vec!["SIGN_REQUEST_TX".into(), "SIGN_REQUEST_RAW".into()],
                events: vec!["DISPLAY_REQUEST".into()],
                resources: None,
            }),
        }),
    };

    let bytes = encode_connect_frame_bare(&frame).expect("encode open frame");
    println!("{}", hex::encode(bytes));
}
