#[test]
fn real_template_wrappers_are_parametric_and_single_pass() {
    let render_outputs = |name: &str, package: &str, bundle: &str, dns: &str| {
        [
            ("site_package_json", site_package_json(package)),
            (
                "webapp_root_package_json",
                webapp_root_package_json(package),
            ),
            (
                "webapp_frontend_package_json",
                webapp_frontend_package_json(package),
            ),
            (
                "pii_app_root_package_json",
                pii_app_root_package_json(package),
            ),
            (
                "pii_app_frontend_package_json",
                pii_app_frontend_package_json(package),
            ),
            (
                "hayahi_app_root_package_json",
                hayahi_app_root_package_json(package),
            ),
            ("site_app_vue", site_app_vue(name)),
            (
                "single_api_api_dev_server_mjs",
                single_api_api_dev_server_mjs(name),
            ),
            ("http_service_build_sh", http_service_build_sh(bundle)),
            (
                "http_service_build_and_sync_sh",
                http_service_build_and_sync_sh(bundle),
            ),
            ("http_service_server_mjs", http_service_server_mjs(name)),
            ("split_app_live_server_mjs", split_app_live_server_mjs(name)),
            ("http_service_readme", http_service_readme(name, package)),
            (
                "split_app_frontend_package_json",
                split_app_frontend_package_json(package),
            ),
            (
                "split_app_frontend_app_vue",
                split_app_frontend_app_vue(name),
            ),
            (
                "split_app_vault_dev_server_mjs",
                split_app_vault_dev_server_mjs(name),
            ),
            (
                "split_app_vault_contract_ko",
                split_app_vault_contract_ko(name),
            ),
            ("split_app_live_readme", split_app_live_readme(name)),
            ("split_app_vault_readme", split_app_vault_readme(name)),
            ("split_app_readme", split_app_readme(name, package)),
            (
                "split_app_existing_repo_readme",
                split_app_existing_repo_readme(name),
            ),
            ("site_readme", site_readme(name, dns)),
            ("single_api_api_readme", single_api_api_readme(name)),
            (
                "single_api_app_readme",
                single_api_app_readme(name, package),
            ),
            ("webapp_readme", webapp_readme(name)),
            ("pii_app_readme", pii_app_readme(name)),
            ("hayahi_app_readme", hayahi_app_readme(name)),
        ]
    };

    const NAME: &str = "travel_ops";
    const PACKAGE: &str = "travel-ops";
    const BUNDLE: &str = "http-service.tgz";
    const DNS: &str = "travel-ops.sora";
    let canonical = render_outputs(NAME, PACKAGE, BUNDLE, DNS);
    let name = "line one\n\"quoted\" {braced} __SORACLOUD_PACKAGE_NAME__ 雪";
    let package = "pkg-{braced}-__SORACLOUD_SERVICE_NAME__";
    let bundle = "bundle-__SORACLOUD_APP_NAME__.tgz";
    let dns = "__SORACLOUD_DNS_HOST__.example";
    let adversarial = render_outputs(name, package, bundle, dns);
    let name_debug = format!("{name:?}");
    let contract = format!("{}_vault_api", normalized_contract_identifier(name));

    assert_eq!(canonical.len(), 27, "canonical wrapper inventory");
    assert_eq!(adversarial.len(), canonical.len(), "adversarial inventory");
    for ((label, canonical), (adversarial_label, actual)) in canonical.iter().zip(&adversarial) {
        assert_eq!(label, adversarial_label, "adversarial function order");
        let substitutions: &[(&str, &str)] = match *label {
            "site_package_json"
            | "webapp_root_package_json"
            | "webapp_frontend_package_json"
            | "pii_app_root_package_json"
            | "pii_app_frontend_package_json"
            | "hayahi_app_root_package_json"
            | "split_app_frontend_package_json" => &[(PACKAGE, package)],
            "single_api_api_dev_server_mjs"
            | "http_service_server_mjs"
            | "split_app_live_server_mjs"
            | "split_app_vault_dev_server_mjs" => &[(r#""travel_ops""#, &name_debug)],
            "http_service_build_sh" | "http_service_build_and_sync_sh" => &[(BUNDLE, bundle)],
            "http_service_readme" | "split_app_readme" | "single_api_app_readme" => {
                &[(NAME, name), (PACKAGE, package)]
            }
            "site_readme" => &[(NAME, name), (DNS, dns)],
            "split_app_vault_contract_ko" => &[("travel_ops_vault_api", &contract)],
            "site_app_vue"
            | "split_app_frontend_app_vue"
            | "split_app_live_readme"
            | "split_app_vault_readme"
            | "split_app_existing_repo_readme"
            | "single_api_api_readme"
            | "webapp_readme"
            | "pii_app_readme"
            | "hayahi_app_readme" => &[(NAME, name)],
            unknown => panic!("missing adversarial substitutions for {unknown}"),
        };
        let expected = substitutions
            .iter()
            .fold(canonical.clone(), |rendered, pair| {
                rendered.replace(pair.0, pair.1)
            });
        assert_eq!(actual, &expected, "{label} adversarial rendering");
    }
}
