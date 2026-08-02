//! Native Falcon-backed Taira Bootle/Lantern issuance broker executable.

#[tokio::main]
async fn main() {
    if let Err(error) =
        irohad::taira_bootle_lantern_broker::run_taira_bootle_lantern_broker_v1().await
    {
        eprintln!("taira Bootle/Lantern broker failed: {error}");
        std::process::exit(1);
    }
}
