//! MOCHI entry point.

#[cfg(feature = "gui")]
mod gui;

#[cfg(feature = "gui")]
fn main() -> eframe::Result<()> {
    gui::run()
}

#[cfg(not(feature = "gui"))]
fn main() {
    println!(
        "MOCHI GUI is not enabled in the default workspace build. Rebuild with `-p mochi-ui --features gui` to start the desktop supervisor."
    );
}
