//! Startup jingle for the optional `beep` feature.
//!
//! The tune is synthesized in Rust as 16-bit mono PCM inside a RIFF/WAVE
//! buffer, written to a private temporary file and handed to the platform's
//! built-in audio player: `afplay` on macOS, `paplay` or `aplay` on Linux, and
//! the .NET `SoundPlayer` through PowerShell on Windows. A missing player or
//! audio device is ignored, so node startup never depends on audio output.

use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

/// PCM sample rate in Hz.
const SAMPLE_RATE: u32 = 48_000;
/// Tempo of the score in beats per minute.
const BPM: u32 = 120;
/// Length of one beat in milliseconds.
const BEAT_MS: u32 = 60_000 / BPM;
/// Peak amplitude of every note relative to full scale.
const AMPLITUDE: f64 = 0.20;
/// Size of the canonical RIFF/WAVE header written by [`wav_bytes`].
const WAV_HEADER_LEN: usize = 44;

/// A note of the startup tune.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Note {
    A4,
    B4,
    G4,
    E4,
    Rest,
}

impl Note {
    /// Frequency in Hz, or `None` for a rest.
    const fn freq(self) -> Option<u32> {
        match self {
            Self::A4 => Some(440),
            Self::B4 => Some(494),
            Self::G4 => Some(392),
            Self::E4 => Some(330),
            Self::Rest => None,
        }
    }
}

/// Japanese song, Kagome-Kagome, as `(note, length in half beats)` pairs.
const KAGOME: [(Note, u32); 55] = {
    use Note::{A4, B4, E4, G4, Rest};
    [
        (A4, 4),
        (A4, 2),
        (B4, 2),
        (A4, 2),
        (A4, 2),
        (A4, 2),
        (Rest, 2),
        (A4, 2),
        (A4, 1),
        (A4, 1),
        (A4, 2),
        (G4, 1),
        (G4, 1),
        (A4, 2),
        (A4, 1),
        (G4, 1),
        (E4, 2),
        (Rest, 2),
        (A4, 2),
        (G4, 2),
        (A4, 2),
        (G4, 2),
        (A4, 2),
        (A4, 1),
        (G4, 1),
        (E4, 2),
        (Rest, 2),
        (A4, 2),
        (A4, 2),
        (A4, 2),
        (B4, 2),
        (A4, 2),
        (A4, 2),
        (A4, 2),
        (Rest, 2),
        (A4, 2),
        (G4, 1),
        (G4, 1),
        (A4, 2),
        (G4, 1),
        (G4, 1),
        (A4, 2),
        (A4, 2),
        (E4, 2),
        (Rest, 2),
        (A4, 1),
        (A4, 1),
        (A4, 1),
        (A4, 1),
        (A4, 2),
        (B4, 2),
        (A4, 3),
        (G4, 1),
        (A4, 2),
        (Rest, 2),
    ]
};

/// Play the startup tune on the default audio output, when one is available.
///
/// Blocks until playback finishes. A missing player, audio device or writable
/// temporary directory skips the tune without reporting an error.
pub(crate) fn play_startup_jingle() {
    if let Some(wav) = wav_bytes(&synthesize(&KAGOME)) {
        play_wav(&wav, platform_players);
    }
}

/// Number of PCM samples in a score element lasting `half_beats` half beats.
fn element_samples(half_beats: u32) -> usize {
    let millis = u64::from(half_beats) * u64::from(BEAT_MS) / 2;
    let samples = u64::from(SAMPLE_RATE) * millis / 1_000;
    usize::try_from(samples).unwrap_or(usize::MAX)
}

/// Render `score` as signed 16-bit mono PCM at [`SAMPLE_RATE`].
///
/// Each note is a sine wave of [`AMPLITUDE`] that starts at phase zero; a rest
/// is silence of the same length.
fn synthesize(score: &[(Note, u32)]) -> Vec<i16> {
    let total = score
        .iter()
        .map(|&(_, half_beats)| element_samples(half_beats))
        .sum();
    let mut pcm = Vec::with_capacity(total);
    for &(note, half_beats) in score {
        let samples = element_samples(half_beats);
        let Some(freq) = note.freq() else {
            pcm.resize(pcm.len() + samples, 0);
            continue;
        };
        let mut phase = 0u32;
        for _ in 0..samples {
            let turn = f64::from(phase) / f64::from(SAMPLE_RATE);
            let value = AMPLITUDE * f64::from(i16::MAX) * (std::f64::consts::TAU * turn).sin();
            pcm.push(value.round() as i16);
            // Integer phase keeps every sample exact: `phase / SAMPLE_RATE` is the
            // fraction of the current period, advanced by `freq / SAMPLE_RATE`.
            phase = (phase + freq) % SAMPLE_RATE;
        }
    }
    pcm
}

/// Encode mono 16-bit PCM `samples` as a canonical RIFF/WAVE file.
///
/// Returns `None` when the data does not fit a RIFF chunk.
fn wav_bytes(samples: &[i16]) -> Option<Vec<u8>> {
    const CHANNELS: u16 = 1;
    const BITS_PER_SAMPLE: u16 = 16;
    const PCM_FORMAT: u16 = 1;
    let block_align = CHANNELS * BITS_PER_SAMPLE / 8;
    let byte_rate = SAMPLE_RATE * u32::from(block_align);
    let data_len = u32::try_from(samples.len().checked_mul(usize::from(block_align))?).ok()?;
    let riff_len = data_len.checked_add(u32::try_from(WAV_HEADER_LEN - 8).ok()?)?;

    let mut out = Vec::with_capacity(WAV_HEADER_LEN + samples.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_len.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&PCM_FORMAT.to_le_bytes());
    out.extend_from_slice(&CHANNELS.to_le_bytes());
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&BITS_PER_SAMPLE.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    Some(out)
}

/// Write `wav` to a private temporary file and play it with the first player
/// from `players` that succeeds.
///
/// Returns whether a player finished successfully. The file is removed
/// afterwards either way.
fn play_wav(wav: &[u8], players: impl FnOnce(&Path) -> Vec<Command>) -> bool {
    let Ok(file) = PrivateTempFile::create(wav) else {
        return false;
    };
    run_first_available(players(file.path()))
}

/// Run `players` in order until one exits successfully; returns whether one did.
///
/// A player that cannot be started or exits with a failure (for example
/// because no audio device is present) moves on to the next one.
fn run_first_available(players: Vec<Command>) -> bool {
    players.into_iter().any(|mut player| {
        player
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    })
}

/// The platform's built-in audio players for the WAV file at `path`, in
/// preference order.
#[cfg(target_os = "macos")]
fn platform_players(path: &Path) -> Vec<Command> {
    let mut afplay = Command::new("afplay");
    afplay.arg(path);
    vec![afplay]
}

/// The platform's built-in audio players for the WAV file at `path`, in
/// preference order.
#[cfg(target_os = "linux")]
fn platform_players(path: &Path) -> Vec<Command> {
    let mut paplay = Command::new("paplay");
    paplay.arg(path);
    let mut aplay = Command::new("aplay");
    aplay.arg("-q").arg(path);
    vec![paplay, aplay]
}

/// The platform's built-in audio players for the WAV file at `path`, in
/// preference order.
#[cfg(windows)]
fn platform_players(path: &Path) -> Vec<Command> {
    // The path travels in the child's environment, so it needs no PowerShell quoting.
    let mut powershell = Command::new("powershell");
    powershell
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "(New-Object System.Media.SoundPlayer $env:IROHA_IVM_JINGLE_WAV).PlaySync()",
        ])
        .env("IROHA_IVM_JINGLE_WAV", path);
    vec![powershell]
}

/// The platform's built-in audio players for the WAV file at `path`, in
/// preference order.
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn platform_players(_path: &Path) -> Vec<Command> {
    Vec::new()
}

/// A temporary file only the current user can read, removed on drop.
#[derive(Debug)]
struct PrivateTempFile {
    path: PathBuf,
}

impl PrivateTempFile {
    /// Create a new file holding `contents` in the system temporary directory.
    ///
    /// The name is unique to this process and the file is created exclusively,
    /// owner-only on Unix, so an existing path is never reused.
    fn create(contents: &[u8]) -> io::Result<Self> {
        let dir = std::env::temp_dir();
        let pid = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.subsec_nanos());
        for attempt in 0..16u32 {
            let path = dir.join(format!("iroha-ivm-jingle-{pid}-{nanos}-{attempt}.wav"));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&path) {
                Ok(mut file) => {
                    let created = Self { path };
                    file.write_all(contents)?;
                    return Ok(created);
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "no unused temporary file name for the startup jingle",
        ))
    }

    /// Location of the file.
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PrivateTempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Samples in one beat at the score tempo.
    const BEAT_SAMPLES: usize = 24_000;
    /// Largest sample magnitude a note may reach.
    const PEAK: i16 = 6_553;

    #[test]
    fn notes_keep_their_pitches() {
        assert_eq!(Note::A4.freq(), Some(440));
        assert_eq!(Note::B4.freq(), Some(494));
        assert_eq!(Note::G4.freq(), Some(392));
        assert_eq!(Note::E4.freq(), Some(330));
        assert_eq!(Note::Rest.freq(), None);
    }

    #[test]
    fn element_samples_follow_the_tempo() {
        assert_eq!(BEAT_MS, 500);
        assert_eq!(element_samples(1), BEAT_SAMPLES / 2);
        assert_eq!(element_samples(2), BEAT_SAMPLES);
        assert_eq!(element_samples(3), BEAT_SAMPLES * 3 / 2);
        assert_eq!(element_samples(4), BEAT_SAMPLES * 2);
    }

    #[test]
    fn score_keeps_the_original_shape() {
        let half_beats: u32 = KAGOME.iter().map(|&(_, length)| length).sum();
        assert_eq!(half_beats, 96, "the tune lasts 48 beats");
        let rests: Vec<u32> = KAGOME
            .iter()
            .filter(|&&(note, _)| note == Note::Rest)
            .map(|&(_, length)| length)
            .collect();
        assert_eq!(rests, [2; 6], "six one-beat rests");
        assert_eq!(KAGOME.first(), Some(&(Note::A4, 4)));
        assert_eq!(KAGOME.last(), Some(&(Note::Rest, 2)));
        assert_eq!(synthesize(&KAGOME).len(), 48 * BEAT_SAMPLES);
    }

    #[test]
    fn rests_are_silent_and_notes_restart_at_zero_phase() {
        let pcm = synthesize(&[(Note::Rest, 1), (Note::A4, 1), (Note::A4, 1)]);
        let half = BEAT_SAMPLES / 2;
        assert_eq!(pcm.len(), 3 * half);
        assert!(pcm[..half].iter().all(|&sample| sample == 0));
        assert_eq!(pcm[half], 0, "a note starts at phase zero");
        assert_eq!(pcm[2 * half], 0, "the next note restarts at phase zero");
        assert_eq!(pcm[half..2 * half], pcm[2 * half..]);
        // 440 Hz at 48 kHz: sample 3 sits at 1320/48000 of a period.
        let expected = (AMPLITUDE * f64::from(i16::MAX) * (std::f64::consts::TAU * 0.0275).sin())
            .round() as i16;
        assert_eq!(pcm[half + 3], expected);
    }

    #[test]
    fn notes_stay_within_the_amplitude() {
        let pcm = synthesize(&KAGOME);
        let peak = pcm.iter().map(|sample| sample.unsigned_abs()).max();
        assert_eq!(peak, Some(PEAK.unsigned_abs()));
        let pcm = synthesize(&[(Note::E4, 2)]);
        assert!(
            pcm.iter()
                .all(|sample| sample.unsigned_abs() <= PEAK.unsigned_abs())
        );
        assert!(pcm.iter().any(|&sample| sample > 6_500));
        assert!(pcm.iter().any(|&sample| sample < -6_500));
    }

    #[test]
    fn wav_header_describes_mono_16_bit_pcm() {
        let samples = [0i16, 1, -1, i16::MAX, i16::MIN];
        let wav = wav_bytes(&samples).expect("small buffer fits");
        assert_eq!(wav.len(), WAV_HEADER_LEN + samples.len() * 2);
        let u16_at = |at: usize| u16::from_le_bytes([wav[at], wav[at + 1]]);
        let u32_at =
            |at: usize| u32::from_le_bytes([wav[at], wav[at + 1], wav[at + 2], wav[at + 3]]);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(u32_at(4), 36 + 10);
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(u32_at(16), 16, "PCM fmt chunk size");
        assert_eq!(u16_at(20), 1, "PCM format tag");
        assert_eq!(u16_at(22), 1, "mono");
        assert_eq!(u32_at(24), SAMPLE_RATE);
        assert_eq!(u32_at(28), SAMPLE_RATE * 2, "byte rate");
        assert_eq!(u16_at(32), 2, "block align");
        assert_eq!(u16_at(34), 16, "bits per sample");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(u32_at(40), 10);
        assert_eq!(
            &wav[WAV_HEADER_LEN..],
            [0, 0, 1, 0, 0xff, 0xff, 0xff, 0x7f, 0x00, 0x80]
        );
    }

    #[test]
    fn jingle_wav_holds_the_whole_tune() {
        let wav = wav_bytes(&synthesize(&KAGOME)).expect("jingle fits a RIFF chunk");
        assert_eq!(wav.len(), WAV_HEADER_LEN + 48 * BEAT_SAMPLES * 2);
    }

    #[test]
    fn missing_players_are_ignored() {
        assert!(!run_first_available(Vec::new()));
        assert!(!run_first_available(vec![Command::new(
            "iroha-ivm-jingle-test-missing-player"
        )]));
    }

    #[cfg(unix)]
    #[test]
    fn first_successful_player_ends_the_search() {
        assert!(run_first_available(vec![
            Command::new("false"),
            Command::new("true"),
            Command::new("iroha-ivm-jingle-test-missing-player"),
        ]));
        assert!(!run_first_available(vec![Command::new("false")]));
    }

    #[test]
    fn play_without_a_player_is_silent_and_removes_the_file() {
        let seen = RefCell::new(None);
        let played = play_wav(b"RIFF", |path| {
            assert_eq!(fs::read(path).expect("jingle file is readable"), b"RIFF");
            *seen.borrow_mut() = Some(path.to_path_buf());
            vec![Command::new("iroha-ivm-jingle-test-missing-player")]
        });
        assert!(!played);
        let path = seen.into_inner().expect("players were asked for the file");
        assert!(!path.exists(), "the jingle file is removed after playback");
    }

    #[test]
    fn private_temp_files_are_unique_and_removed() {
        let first = PrivateTempFile::create(b"one").expect("create first file");
        let second = PrivateTempFile::create(b"two").expect("create second file");
        assert_ne!(first.path(), second.path());
        assert_eq!(fs::read(first.path()).expect("read first file"), b"one");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(first.path())
                .expect("stat")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600, "owner-only file");
        }
        let path = first.path().to_path_buf();
        drop(first);
        assert!(!path.exists());
    }

    #[test]
    fn platform_players_receive_the_file() {
        let path = Path::new("jingle.wav");
        let players = platform_players(path);
        #[cfg(target_os = "macos")]
        {
            assert_eq!(players.len(), 1);
            assert_eq!(players[0].get_program(), "afplay");
            assert_eq!(
                players[0].get_args().collect::<Vec<_>>(),
                [path.as_os_str()]
            );
        }
        #[cfg(target_os = "linux")]
        {
            let programs: Vec<_> = players.iter().map(Command::get_program).collect();
            assert_eq!(programs, ["paplay", "aplay"]);
            assert!(
                players
                    .iter()
                    .all(|player| player.get_args().last() == Some(path.as_os_str()))
            );
        }
        #[cfg(windows)]
        {
            assert_eq!(players.len(), 1);
            assert!(players[0].get_envs().any(
                |(key, value)| key == "IROHA_IVM_JINGLE_WAV" && value == Some(path.as_os_str())
            ));
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
        assert!(players.is_empty());
    }
}
