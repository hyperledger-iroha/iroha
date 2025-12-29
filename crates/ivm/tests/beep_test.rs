#[cfg(feature = "beep")]
use ivm::IVM;

#[cfg(feature = "beep")]
#[test]
fn beep_music_runs() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!("Skipping: requires audio output device. Set IROHA_RUN_IGNORED=1 to run.");
        return;
    }
    IVM::beep_music();
}
