//! Disposable per-peer broker using the stock credential decoder and server.
//!
//! Only explicit network qualification builds include this executable. Its
//! Inherited standard input carries one exact threshold credential bundle. A
//! configured Soracloud signer consumes the same owner-private FD198 record as
//! the shipping Taira launcher. Credentials never enter arguments or environment.

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn main() {
    eprintln!("the disposable runtime-provider broker requires Unix peer credentials");
    std::process::exit(2);
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn main() {
    use clap::Parser as _;
    use irohad::taira_runtime_signer::load_disposable_runtime_provider_broker_v1;
    use irohad::{
        RuntimeProviderBrokerExecutableArgsV1, RuntimeProviderBrokerExecutableV1,
        load_owner_private_runtime_provider_broker_catalog_file_v1,
        load_owner_private_runtime_provider_broker_policy_file_v1,
    };
    use std::io::Write as _;

    let args = RuntimeProviderBrokerExecutableArgsV1::parse();
    let policy =
        load_owner_private_runtime_provider_broker_policy_file_v1(args.broker_policy_path())
            .expect("load exact owner-private public policy");
    let catalog = load_owner_private_runtime_provider_broker_catalog_file_v1(args.catalog_path())
        .expect("load exact owner-private public catalog");
    let backends =
        load_disposable_runtime_provider_broker_v1(&catalog, &policy, &mut std::io::stdin().lock())
            .expect("load exact inherited runtime provider credentials");
    let executable =
        RuntimeProviderBrokerExecutableV1::try_from_catalog_v1(catalog, policy, backends.as_ref())
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
