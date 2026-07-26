#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::suboptimal_flops
)]

//! Theme intro with animated ASCII prologue and optional audio playback.
//! The builtin audio path renders a gagaku-inspired chamber arrangement of
//! Etenraku with softer winds and a slower shō bed.

use std::{fs, io::Write as _, path::PathBuf};

#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", feature = "linux-builtin-synth")
))]
use std::{
    io::BufWriter,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use eyre::{Context as _, Result};
use tokio::{
    process::Child,
    time::{Duration, sleep},
};

use crate::{ascii::AsciiAnimator, etenraku};

#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", feature = "linux-builtin-synth")
))]
use crate::synth;

#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", feature = "linux-builtin-synth")
))]
const THEME_WAV_SAMPLE_RATE: u32 = 44_100;
#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", feature = "linux-builtin-synth")
))]
const THEME_WAV_CHANNELS: usize = 2;
#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", feature = "linux-builtin-synth")
))]
const THEME_WAV_SECONDS: u32 = 82;
#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", feature = "linux-builtin-synth")
))]
const THEME_WAV_CHUNK_FRAMES: usize = 2048;

pub struct ThemeIntro;

#[derive(Default)]
pub struct ThemePlayback {
    midi_child: Option<Child>,
    audio_file: Option<PathBuf>,
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
                match start_builtin_synth_demo() {
                    Ok((child, audio_file)) => {
                        playback.midi_child = Some(child);
                        playback.audio_file = Some(audio_file);
                    }
                    Err(err) => {
                        eprintln!("iroha_monitor: theme audio disabled ({err:?})");
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
        if let Some(mut child) = self.midi_child.take() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
        if let Some(path) = self.audio_file.take() {
            let _ = fs::remove_file(path);
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
fn start_builtin_synth_demo() -> eyre::Result<(Child, PathBuf)> {
    #[cfg(test)]
    if crate::theme::test_support::should_force_failure() {
        return Err(eyre::eyre!("forced builtin audio failure (test)"));
    }

    let path = render_builtin_theme_wav()?;
    let child = spawn_default_audio_player(&path)
        .wrap_err_with(|| format!("start default audio player for {}", path.display()))?;
    Ok((child, path))
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", feature = "linux-builtin-synth")
)))]
fn start_builtin_synth_demo() -> eyre::Result<(Child, PathBuf)> {
    #[cfg(test)]
    let _ = crate::theme::test_support::should_force_failure();

    Err(eyre::eyre!(
        "built-in synth unavailable; rebuild with linux-builtin-synth or pass --midi-player"
    ))
}

#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", feature = "linux-builtin-synth")
))]
fn render_builtin_theme_wav() -> Result<PathBuf> {
    let path = std::env::temp_dir().join(format!(
        "iroha-monitor-etenraku-{}-{}.wav",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let file = fs::File::create(&path).wrap_err("create theme wav")?;
    let mut writer = BufWriter::new(file);
    let frames = THEME_WAV_SAMPLE_RATE * THEME_WAV_SECONDS;
    write_wav_header(&mut writer, frames)?;

    let mut state = synth::prepare(THEME_WAV_SAMPLE_RATE, THEME_WAV_CHANNELS);
    let mut chunk = vec![0.0f32; THEME_WAV_CHUNK_FRAMES * THEME_WAV_CHANNELS];
    let mut remaining = frames as usize;
    while remaining > 0 {
        let chunk_frames = remaining.min(THEME_WAV_CHUNK_FRAMES);
        let sample_len = chunk_frames * THEME_WAV_CHANNELS;
        state.render_chunk(&mut chunk[..sample_len]);
        for sample in &chunk[..sample_len] {
            let pcm = (sample * 32_767.0).clamp(-32_768.0, 32_767.0).round() as i16;
            writer.write_all(&pcm.to_le_bytes())?;
        }
        remaining -= chunk_frames;
    }
    writer.flush()?;
    Ok(path)
}

#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", feature = "linux-builtin-synth")
))]
fn write_wav_header(writer: &mut impl std::io::Write, frames: u32) -> Result<()> {
    let channels = THEME_WAV_CHANNELS as u16;
    let bits_per_sample = 16u16;
    let block_align = channels * bits_per_sample / 8;
    let byte_rate = THEME_WAV_SAMPLE_RATE * u32::from(block_align);
    let data_len = frames * u32::from(block_align);
    writer.write_all(b"RIFF")?;
    writer.write_all(&(36 + data_len).to_le_bytes())?;
    writer.write_all(b"WAVEfmt ")?;
    writer.write_all(&16u32.to_le_bytes())?;
    writer.write_all(&1u16.to_le_bytes())?;
    writer.write_all(&channels.to_le_bytes())?;
    writer.write_all(&THEME_WAV_SAMPLE_RATE.to_le_bytes())?;
    writer.write_all(&byte_rate.to_le_bytes())?;
    writer.write_all(&block_align.to_le_bytes())?;
    writer.write_all(&bits_per_sample.to_le_bytes())?;
    writer.write_all(b"data")?;
    writer.write_all(&data_len.to_le_bytes())?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn spawn_default_audio_player(path: &Path) -> Result<Child> {
    tokio::process::Command::new("afplay")
        .arg(path)
        .spawn()
        .wrap_err("spawn afplay")
}

#[cfg(target_os = "windows")]
fn spawn_default_audio_player(path: &Path) -> Result<Child> {
    let path = path.to_string_lossy().replace('\'', "''");
    tokio::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!("(New-Object Media.SoundPlayer '{path}').PlaySync()"),
        ])
        .spawn()
        .wrap_err("spawn powershell audio player")
}

#[cfg(all(unix, not(target_os = "macos"), feature = "linux-builtin-synth"))]
fn spawn_default_audio_player(path: &Path) -> Result<Child> {
    let candidates: &[(&str, &[&str])] = &[
        ("paplay", &[]),
        ("aplay", &[]),
        ("ffplay", &["-nodisp", "-autoexit", "-loglevel", "quiet"]),
    ];
    let mut last_error = None;
    for (program, args) in candidates {
        let mut command = tokio::process::Command::new(program);
        command.args(*args).arg(path);
        match command.spawn() {
            Ok(child) => return Ok(child),
            Err(err) => last_error = Some(err),
        }
    }
    Err(eyre::eyre!(
        "no default audio player found ({})",
        last_error
            .map(|err| err.to_string())
            .unwrap_or_else(|| "no candidates tried".to_string())
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
