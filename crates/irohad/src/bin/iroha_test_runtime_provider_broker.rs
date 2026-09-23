//! Disposable per-peer broker using the stock credential decoder and server.
//!
//! Only explicit network qualification builds include this executable. Its
//! inherited standard input carries one exact threshold credential bundle;
//! neither credentials nor provider selectors are accepted in argv or env.

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn main() {
    eprintln!("the disposable runtime-provider broker requires Unix peer credentials");
    std::process::exit(2);
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn main() {
    use clap::Parser as _;
    use irohad::external_software_signer::RuntimeConsensusThresholdSignerBackendsV1;
    use irohad::{
        RuntimeProviderBrokerExecutableArgsV1, RuntimeProviderBrokerExecutableV1,
        load_owner_private_runtime_provider_broker_catalog_file_v1,
    };
    use std::io::Write as _;

    let args = RuntimeProviderBrokerExecutableArgsV1::parse();
    let catalog = load_owner_private_runtime_provider_broker_catalog_file_v1(args.catalog_path())
        .expect("load exact owner-private public catalog");
    let backends =
        RuntimeConsensusThresholdSignerBackendsV1::load_from_launchd_credential_bundle_v1(
            &catalog,
            &mut std::io::stdin().lock(),
        )
        .expect("load exact inherited threshold credential bundle");
    let executable = RuntimeProviderBrokerExecutableV1::try_from_owner_private_catalog_v1(
        catalog,
        args.broker_endpoint().clone(),
        &backends,
    )
    .expect("qualify disposable broker catalog");
    executable
        .serve_until_shutdown_signal(|| {
            let mut stdout = std::io::stdout().lock();
            stdout
                .write_all(b"READY\n")
                .expect("publish broker readiness");
            stdout.flush().expect("flush broker readiness");
        })
        .expect("serve authenticated disposable broker");
}
