#[test]
fn ipns_mode_rejects_every_signed_head_authenticator_field() {
    let mut service = valid_governance_dag_service();
    service.head_mode = "ipns".to_owned();
    service.signed_head_url = None;
    service.ipns_name = Some("k51qzi5uqu5dl-governance".to_owned());
    service.ipns_key_name = Some("governance-publisher".to_owned());
    service.head_authenticator_handle = None;
    service.head_authenticator_revision = None;
    service.head_authenticator_policy_digest_hex = None;
    service.head_request_auth_public_key_hex = None;
    let mut valid_emitter = Emitter::new();
    let _ = service.clone().parse(false, &mut valid_emitter);
    valid_emitter
        .into_result()
        .expect("IPNS mode without a signed-head authenticator must parse");

    service.signed_head_url = Some("https://governance-head.example/v1/head".to_owned());
    let mut endpoint_emitter = Emitter::new();
    let _ = service.clone().parse(false, &mut endpoint_emitter);
    drop(
        endpoint_emitter
            .into_result()
            .expect_err("a signed-head endpoint must fail in IPNS mode"),
    );

    service.signed_head_url = None;
    service.head_request_auth_public_key_hex = Some(ALTERNATE_PUBLISHER_PUBLIC_KEY_HEX.to_owned());
    let mut invalid_emitter = Emitter::new();
    let _ = service.parse(false, &mut invalid_emitter);
    drop(
        invalid_emitter
            .into_result()
            .expect_err("even a lone signed-head verifier key must fail in IPNS mode"),
    );
}

#[test]
fn signed_http_mode_rejects_every_ipns_selector() {
    let selectors: [fn(&mut SorafsGovernanceDagService); 2] = [
        |service: &mut SorafsGovernanceDagService| {
            service.ipns_name = Some("k51qzi5uqu5dl-governance".to_owned());
        },
        |service: &mut SorafsGovernanceDagService| {
            service.ipns_key_name = Some("governance-publisher".to_owned());
        },
    ];
    for set_selector in selectors {
        let mut service = valid_governance_dag_service();
        set_selector(&mut service);
        let mut emitter = Emitter::new();
        let _ = service.parse(false, &mut emitter);
        drop(
            emitter
                .into_result()
                .expect_err("IPNS selectors must fail in signed_http mode"),
        );
    }
}
