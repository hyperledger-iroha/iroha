#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::suboptimal_flops
)]

//! Theme intro with animated ASCII prologue and optional audio playback.
//! The builtin audio path renders a gagaku-inspired chamber arrangement of
//! Etenraku with softer winds and a slower shō bed.

use std::io::Write as _;

#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", feature = "linux-builtin-synth")
))]
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use eyre::{Context as _, Result, eyre};
use tokio::{
    process::Child,
    time::{Duration, sleep},
};

#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", feature = "linux-builtin-synth")
))]
use crate::synth;
use crate::{ascii::AsciiAnimator, etenraku};

pub struct ThemeIntro;

#[derive(Default)]
pub struct ThemePlayback {
    midi_child: Option<Child>,
    #[cfg(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", feature = "linux-builtin-synth")
    ))]
    synth: Option<SoftSynthHandle>,
}

#[derive(Default, Clone)]
pub struct ThemeOptions {
    pub audio: bool,
    pub midi_player: Option<String>,
    pub midi_file: Option<String>,
}

impl ThemeIntro {
    pub fn new() -> Self {
        Self
    }

    #[allow(clippy::future_not_send)]
    pub async fn play(&self, options: ThemeOptions) -> Result<ThemePlayback> {
        let mut playback = ThemePlayback::default();
        if options.audio {
            if let Some(player) = &options.midi_player {
                let midi_path = if let Some(path) = &options.midi_file {
                    path.clone()
                } else {
                    etenraku::write_demo_midi_file().wrap_err("write theme midi")?
                };
                let child = tokio::process::Command::new(player)
                    .arg(&midi_path)
                    .spawn()
                    .wrap_err("spawn midi player")?;
                playback.midi_child = Some(child);
            } else {
                #[cfg(any(
                    target_os = "macos",
                    target_os = "windows",
                    all(target_os = "linux", feature = "linux-builtin-synth")
                ))]
                {
                    match start_builtin_synth_demo() {
                        Ok(handle) => {
                            playback.synth = Some(handle);
                        }
                        Err(err) => {
                            eprintln!("iroha_monitor: theme audio disabled ({err:?})");
                        }
                    }
                }
            }
        }

        render_ascii_intro().await?;

        Ok(playback)
    }
}

impl ThemePlayback {
    #[allow(clippy::future_not_send)]
    pub async fn stop(&mut self) {
        #[cfg(any(
            target_os = "macos",
            target_os = "windows",
            all(target_os = "linux", feature = "linux-builtin-synth")
        ))]
        {
            // Drop the CPAL stream on the current thread before awaiting child shutdown.
            let _ = self.synth.take();
        }

        if let Some(mut child) = self.midi_child.take() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
    }
}

async fn render_ascii_intro() -> Result<()> {
    let mut anim = AsciiAnimator::new();
    let mut stdout = std::io::stdout();
    for _ in 0..6 {
        print!("\x1b[2J\x1b[H");
        let frame = anim.frame(72);
        for line in frame {
            println!("{line}");
        }
        println!("\n   ♪  Etenraku drifts in a slower court-music breath...  ♪   ");
        stdout.flush().wrap_err("flush intro")?;
        sleep(Duration::from_millis(140)).await;
        anim.advance();
    }
    sleep(Duration::from_millis(220)).await;
    Ok(())
}

#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", feature = "linux-builtin-synth")
))]
pub struct SoftSynthHandle {
    _stream: cpal::Stream,
    _state: Option<std::sync::Arc<std::sync::Mutex<SynthPlaybackState>>>,
}

#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", feature = "linux-builtin-synth")
))]
struct SynthPlaybackState {
    synth: synth::SynthState,
}

#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", feature = "linux-builtin-synth")
))]
fn render_samples(state: &mut SynthPlaybackState, dest: &mut [f32]) {
    state.synth.render_chunk(dest);
}

#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", feature = "linux-builtin-synth")
))]
#[allow(clippy::too_many_lines)]
fn start_builtin_synth_demo() -> eyre::Result<SoftSynthHandle> {
    #[cfg(test)]
    if crate::theme::test_support::should_force_failure() {
        return Err(eyre!("forced builtin audio failure (test)"));
    }

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| eyre!("no default audio output device available"))?;
    let config = device
        .default_output_config()
        .wrap_err("query audio output configuration")?;
    let stream_config: cpal::StreamConfig = config.clone().into();

    let synth_state = synth::prepare(stream_config.sample_rate.0, stream_config.channels as usize);

    let shared = std::sync::Arc::new(std::sync::Mutex::new(SynthPlaybackState {
        synth: synth_state,
    }));
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let shared = shared.clone();
            device.build_output_stream(
                &stream_config,
                move |data: &mut [f32], _| {
                    if let Ok(mut guard) = shared.lock() {
                        render_samples(&mut guard, data);
                    } else {
                        data.fill(0.0);
                    }
                },
                |err| eprintln!("iroha_monitor: audio output error: {err}"),
                None,
            )?
        }
        cpal::SampleFormat::I16 => {
            let shared = shared.clone();
            let mut frame_buf = Vec::<f32>::new();
            device.build_output_stream(
                &stream_config,
                move |data: &mut [i16], _| {
                    if let Ok(mut guard) = shared.lock() {
                        if frame_buf.len() != data.len() {
                            frame_buf.resize(data.len(), 0.0);
                        }
                        render_samples(&mut guard, &mut frame_buf);
                        for (dst, src) in data.iter_mut().zip(frame_buf.iter()) {
                            let scaled = (src * 32_767.0).clamp(-32_767.0, 32_767.0).round() as i16;
                            *dst = scaled;
                        }
                    } else {
                        data.fill(0);
                    }
                },
                |err| eprintln!("iroha_monitor: audio output error: {err}"),
                None,
            )?
        }
        cpal::SampleFormat::I32 => {
            let shared = shared.clone();
            let mut frame_buf = Vec::<f32>::new();
            device.build_output_stream(
                &stream_config,
                move |data: &mut [i32], _| {
                    if let Ok(mut guard) = shared.lock() {
                        if frame_buf.len() != data.len() {
                            frame_buf.resize(data.len(), 0.0);
                        }
                        render_samples(&mut guard, &mut frame_buf);
                        for (dst, src) in data.iter_mut().zip(frame_buf.iter()) {
                            let scaled = (src * 2_147_483_647.0)
                                .clamp(-2_147_483_647.0, 2_147_483_647.0)
                                .round() as i32;
                            *dst = scaled;
                        }
                    } else {
                        data.fill(0);
                    }
                },
                |err| eprintln!("iroha_monitor: audio output error: {err}"),
                None,
            )?
        }
        cpal::SampleFormat::U16 => {
            let shared = shared.clone();
            let mut frame_buf = Vec::<f32>::new();
            device.build_output_stream(
                &stream_config,
                move |data: &mut [u16], _| {
                    if let Ok(mut guard) = shared.lock() {
                        if frame_buf.len() != data.len() {
                            frame_buf.resize(data.len(), 0.0);
                        }
                        render_samples(&mut guard, &mut frame_buf);
                        for (dst, src) in data.iter_mut().zip(frame_buf.iter()) {
                            let scaled =
                                ((src * 0.5 + 0.5) * 65_535.0).clamp(0.0, 65_535.0).round() as u16;
                            *dst = scaled;
                        }
                    } else {
                        data.fill(65_535 / 2);
                    }
                },
                |err| eprintln!("iroha_monitor: audio output error: {err}"),
                None,
            )?
        }
        other => {
            return Err(eyre!(
                "unsupported audio sample format for etenraku synth playback: {other:?}"
            ));
        }
    };
    stream.play().wrap_err("start etenraku synth playback")?;
    Ok(SoftSynthHandle {
        _stream: stream,
        _state: Some(shared),
    })
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", feature = "linux-builtin-synth")
)))]
fn start_builtin_synth_demo() -> eyre::Result<()> {
    Err(eyre!(
        "builtin synth disabled (enable linux-builtin-synth or build on macOS/Windows)"
    ))
}

#[cfg(test)]
mod test_support {
    use std::sync::atomic::{AtomicBool, Ordering};

    static FORCE_FAILURE: AtomicBool = AtomicBool::new(false);

    pub(super) fn should_force_failure() -> bool {
        FORCE_FAILURE.load(Ordering::SeqCst)
    }

    pub(super) struct FailureGuard;

    impl FailureGuard {
        pub(super) fn enable() -> Self {
            FORCE_FAILURE.store(true, Ordering::SeqCst);
            Self
        }
    }

    impl Drop for FailureGuard {
        fn drop(&mut self) {
            FORCE_FAILURE.store(false, Ordering::SeqCst);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{test_support, *};

    #[tokio::test]
    async fn intro_renders_without_audio() {
        let intro = ThemeIntro::new();
        let playback = intro
            .play(ThemeOptions {
                audio: false,
                midi_player: None,
                midi_file: None,
            })
            .await
            .expect("play intro");
        assert!(playback.midi_child.is_none());
    }

    #[cfg(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", feature = "linux-builtin-synth")
    ))]
    #[tokio::test]
    async fn intro_survives_synth_start_failure() {
        let _guard = test_support::FailureGuard::enable();

        let intro = ThemeIntro::new();
        let playback = intro
            .play(ThemeOptions {
                audio: true,
                midi_player: None,
                midi_file: None,
            })
            .await
            .expect("intro should ignore soft-synth failure");

        assert!(playback.midi_child.is_none());
    }
}
