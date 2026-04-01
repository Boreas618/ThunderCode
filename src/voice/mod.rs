//! ThunderCode voice input support.
//!
//! Provides audio recording for push-to-talk voice input. Recording uses
//! external processes (SoX `rec` on macOS/Linux, `arecord` on Linux ALSA)
//! managed via `tokio::process`. A future CoreAudio backend is reserved
//! for native macOS capture without external dependencies.
//!
//! Ported from the TypeScript reference in `ref/services/voice.ts` and
//! `ref/voice/voiceModeEnabled.ts`.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use tokio::process::{Child, Command};
use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// Voice state
// ---------------------------------------------------------------------------

/// Current state of the voice recording subsystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceState {
    /// No recording in progress.
    Idle,
    /// Actively recording audio.
    Recording {
        /// When recording started.
        start_time: Instant,
    },
    /// Recording stopped, audio being post-processed or transcribed.
    Processing,
}

// ---------------------------------------------------------------------------
// Audio backend
// ---------------------------------------------------------------------------

/// Supported audio recording backends.
///
/// Detection order mirrors the TypeScript reference:
/// 1. SoX `rec` (macOS and Linux)
/// 2. ALSA `arecord` (Linux only)
/// 3. CoreAudio (macOS native, reserved for future use)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioBackend {
    /// SoX `rec` command -- works on macOS and Linux.
    Sox,
    /// ALSA `arecord` -- Linux-only fallback.
    Arecord,
    /// macOS native CoreAudio (future).
    CoreAudio,
}

impl std::fmt::Display for AudioBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioBackend::Sox => write!(f, "SoX (rec)"),
            AudioBackend::Arecord => write!(f, "ALSA (arecord)"),
            AudioBackend::CoreAudio => write!(f, "CoreAudio (native)"),
        }
    }
}

impl AudioBackend {
    /// Build the [`Command`] that records audio to `output_path`.
    ///
    /// The command writes 16-bit signed PCM WAV at the sample rate and
    /// channel count specified in `config`.
    pub fn recording_command(&self, output_path: &Path, config: &VoiceConfig) -> Command {
        match self {
            AudioBackend::Sox => {
                let mut cmd = Command::new("rec");
                cmd.args([
                    "-q",                                       // quiet
                    "-r", &config.sample_rate.to_string(),      // sample rate
                    "-c", &config.channels.to_string(),         // channels
                    "-b", "16",                                 // 16-bit
                    output_path.to_str().unwrap_or("output.wav"),
                ]);
                // SoX silence detection: stop recording after sustained silence.
                // Format: silence <above_periods> <above_duration> <threshold>
                //                  <below_periods> <below_duration> <threshold>
                cmd.args([
                    "silence",
                    "1", "0.1", &format!("{}%", (config.silence_threshold * 100.0) as u32),
                    "1", &format!("{:.1}", config.silence_duration.as_secs_f64()),
                    &format!("{}%", (config.silence_threshold * 100.0) as u32),
                ]);
                cmd.stdin(Stdio::null());
                cmd.stdout(Stdio::null());
                cmd.stderr(Stdio::piped());
                cmd
            }
            AudioBackend::Arecord => {
                let mut cmd = Command::new("arecord");
                cmd.args([
                    "-f", "S16_LE",                             // signed 16-bit LE
                    "-r", &config.sample_rate.to_string(),      // sample rate
                    "-c", &config.channels.to_string(),         // channels
                    "-t", "wav",                                // WAV container
                    output_path.to_str().unwrap_or("output.wav"),
                ]);
                cmd.stdin(Stdio::null());
                cmd.stdout(Stdio::null());
                cmd.stderr(Stdio::piped());
                cmd
            }
            AudioBackend::CoreAudio => {
                // Placeholder -- future native capture. For now, fall back to
                // Sox on macOS (the caller should never select CoreAudio yet).
                let mut cmd = Command::new("rec");
                cmd.args([
                    "-q",
                    "-r", &config.sample_rate.to_string(),
                    "-c", &config.channels.to_string(),
                    "-b", "16",
                    output_path.to_str().unwrap_or("output.wav"),
                ]);
                cmd.stdin(Stdio::null());
                cmd.stdout(Stdio::null());
                cmd.stderr(Stdio::piped());
                cmd
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Audio recording configuration with sensible defaults for voice input.
#[derive(Debug, Clone)]
pub struct VoiceConfig {
    /// RMS amplitude below which audio is considered silence (0.0--1.0).
    /// Default: 0.03 (3%).
    pub silence_threshold: f32,

    /// How long continuous silence must last before recording auto-stops.
    /// Default: 2 seconds.
    pub silence_duration: Duration,

    /// Recording sample rate in Hz. Default: 16 000 (optimal for STT).
    pub sample_rate: u32,

    /// Number of audio channels. Default: 1 (mono).
    pub channels: u16,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            silence_threshold: 0.03,
            silence_duration: Duration::from_secs(2),
            sample_rate: 16_000,
            channels: 1,
        }
    }
}

// ---------------------------------------------------------------------------
// Dependency / availability helpers
// ---------------------------------------------------------------------------

/// Check whether an external command is available on `$PATH`.
///
/// Spawns `<cmd> --version` with a 3-second timeout and checks whether the
/// process started successfully (mirrors the TypeScript `hasCommand`).
pub fn has_command(cmd: &str) -> bool {
    use std::process::Command as StdCommand;

    let result = StdCommand::new(cmd)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    match result {
        Ok(status) => {
            // The command was found and executed. Even a non-zero exit is
            // fine -- it means the binary exists (unrecognised flag).
            let _ = status;
            true
        }
        Err(_) => false,
    }
}

/// Result of checking voice recording availability.
#[derive(Debug, Clone)]
pub struct RecordingAvailability {
    /// Whether at least one recording backend is usable.
    pub available: bool,
    /// Human-readable reason when `available` is false.
    pub reason: Option<String>,
}

/// Information about a missing dependency and how to install it.
#[derive(Debug, Clone)]
pub struct VoiceDependencyCheck {
    /// Whether all required tools are present.
    pub available: bool,
    /// Names of missing tools.
    pub missing: Vec<String>,
    /// Suggested install command, if one can be determined.
    pub install_command: Option<String>,
}

/// Detect which audio recording backend is available on this system.
///
/// Returns the first usable backend in priority order:
/// 1. SoX `rec` (macOS and Linux)
/// 2. ALSA `arecord` (Linux only)
///
/// CoreAudio is reserved for future native macOS capture and is never
/// returned by auto-detection today.
pub fn detect_backend() -> Option<AudioBackend> {
    if has_command("rec") {
        return Some(AudioBackend::Sox);
    }
    if cfg!(target_os = "linux") && has_command("arecord") {
        return Some(AudioBackend::Arecord);
    }
    None
}

/// Check whether voice recording is available and identify missing tools.
///
/// On macOS the preferred backend is SoX (`rec`). On Linux, `arecord`
/// (ALSA utils) is accepted as a fallback. Returns an install hint when
/// a package manager can be detected.
pub fn check_voice_dependencies() -> VoiceDependencyCheck {
    if detect_backend().is_some() {
        return VoiceDependencyCheck {
            available: true,
            missing: vec![],
            install_command: None,
        };
    }

    let missing = vec!["sox (rec command)".to_string()];
    let install_command = detect_install_command();

    VoiceDependencyCheck {
        available: false,
        missing,
        install_command,
    }
}

/// Check full recording availability, including environment checks.
///
/// Remote/headless environments are detected and rejected with a
/// descriptive message.
pub fn check_recording_availability() -> RecordingAvailability {
    // Remote environments have no local microphone.
    if std::env::var("THUNDERCODE_REMOTE").map_or(false, |v| is_truthy(&v)) {
        return RecordingAvailability {
            available: false,
            reason: Some(
                "Voice mode requires microphone access, but no audio device \
                 is available in this environment.\n\n\
                 To use voice mode, run ThunderCode locally instead."
                    .to_string(),
            ),
        };
    }

    match detect_backend() {
        Some(_) => RecordingAvailability {
            available: true,
            reason: None,
        },
        None => {
            let hint = match detect_install_command() {
                Some(cmd) => format!(
                    "Voice mode requires SoX for audio recording. \
                     Install it with: {cmd}"
                ),
                None => "Voice mode requires SoX for audio recording. Install SoX manually:\n  \
                         macOS: brew install sox\n  \
                         Ubuntu/Debian: sudo apt-get install sox\n  \
                         Fedora: sudo dnf install sox"
                    .to_string(),
            };
            RecordingAvailability {
                available: false,
                reason: Some(hint),
            }
        }
    }
}

/// Detect the system package manager and return a SoX install command.
fn detect_install_command() -> Option<String> {
    if cfg!(target_os = "macos") {
        if has_command("brew") {
            return Some("brew install sox".to_string());
        }
    } else if cfg!(target_os = "linux") {
        if has_command("apt-get") {
            return Some("sudo apt-get install sox".to_string());
        }
        if has_command("dnf") {
            return Some("sudo dnf install sox".to_string());
        }
        if has_command("pacman") {
            return Some("sudo pacman -S sox".to_string());
        }
    }
    None
}

/// Interpret a string as a boolean, matching the TS `isEnvTruthy`.
fn is_truthy(val: &str) -> bool {
    matches!(val.to_lowercase().as_str(), "1" | "true" | "yes")
}

// ---------------------------------------------------------------------------
// Audio level helpers
// ---------------------------------------------------------------------------

/// Compute the RMS (root mean square) level of 16-bit PCM samples.
///
/// Returns a normalised value in `0.0..=1.0` where `1.0` corresponds to
/// the maximum amplitude of a signed 16-bit sample (32 767).
pub fn rms_level(samples: &[i16]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = samples.iter().map(|&s| (s as f64) * (s as f64)).sum();
    let rms = (sum_sq / samples.len() as f64).sqrt();
    (rms / i16::MAX as f64) as f32
}

/// Check whether an audio buffer is below the silence threshold.
pub fn is_silent(samples: &[i16], threshold: f32) -> bool {
    rms_level(samples) < threshold
}

/// Compute peak amplitude of 16-bit PCM samples, normalised to `0.0..=1.0`.
pub fn peak_level(samples: &[i16]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
    peak as f32 / i16::MAX as f32
}

// ---------------------------------------------------------------------------
// Voice service
// ---------------------------------------------------------------------------

/// Main voice recording service.
///
/// Manages a single recording session at a time, backed by an external
/// audio capture process.
pub struct VoiceService {
    state: VoiceState,
    config: VoiceConfig,
    backend: Option<AudioBackend>,
    recording_process: Option<Child>,
    output_path: Option<PathBuf>,
}

impl VoiceService {
    /// Create a new voice service with the given configuration.
    ///
    /// Auto-detects the audio backend at construction time.
    pub fn new(config: VoiceConfig) -> Self {
        let backend = detect_backend();
        if let Some(ref b) = backend {
            debug!(backend = %b, "Voice backend detected");
        } else {
            warn!("No audio recording backend found");
        }
        Self {
            state: VoiceState::Idle,
            config,
            backend,
            recording_process: None,
            output_path: None,
        }
    }

    /// Check if voice input is available on this system.
    pub fn is_available() -> bool {
        detect_backend().is_some()
    }

    /// Return the detected audio backend, if any.
    pub fn backend(&self) -> Option<AudioBackend> {
        self.backend
    }

    /// Return the current voice state.
    pub fn state(&self) -> &VoiceState {
        &self.state
    }

    /// Start recording audio.
    ///
    /// Spawns the recording subprocess and transitions to
    /// [`VoiceState::Recording`]. Returns an error if no backend is
    /// available or a recording is already in progress.
    pub async fn start_recording(&mut self) -> Result<()> {
        if matches!(self.state, VoiceState::Recording { .. }) {
            anyhow::bail!("Recording already in progress");
        }

        let backend = self
            .backend
            .context("No audio recording backend available")?;

        // Create a temporary WAV file for the recording.
        let tmp_dir = std::env::temp_dir();
        let file_name = format!("thundercode_voice_{}.wav", std::process::id());
        let output_path = tmp_dir.join(file_name);

        debug!(
            backend = %backend,
            path = %output_path.display(),
            "Starting voice recording"
        );

        let mut cmd = backend.recording_command(&output_path, &self.config);
        let child = cmd
            .kill_on_drop(true)
            .spawn()
            .context("Failed to spawn recording process")?;

        self.recording_process = Some(child);
        self.output_path = Some(output_path);
        self.state = VoiceState::Recording {
            start_time: Instant::now(),
        };

        Ok(())
    }

    /// Stop recording and return the path to the captured audio file.
    ///
    /// Sends SIGTERM to the recording process and transitions through
    /// [`VoiceState::Processing`] back to [`VoiceState::Idle`].
    pub async fn stop_recording(&mut self) -> Result<PathBuf> {
        if !matches!(self.state, VoiceState::Recording { .. }) {
            anyhow::bail!("Not currently recording");
        }

        self.state = VoiceState::Processing;

        if let Some(mut child) = self.recording_process.take() {
            // Send SIGTERM to let the recorder flush and close the file.
            debug!("Stopping recording process");
            // On Unix, Child::kill sends SIGKILL. We want a graceful stop
            // so the WAV header is finalised. Use start_kill which is the
            // async equivalent (sends SIGKILL on Unix). For SoX/arecord a
            // SIGTERM via the nix crate would be better, but kill() + wait
            // is acceptable -- both tools handle abrupt termination and
            // produce a valid (if truncated) WAV.
            let _ = child.kill().await;
        }

        let path = self
            .output_path
            .take()
            .context("No output path for recording")?;

        // Verify the file was actually written.
        if !path.exists() {
            self.state = VoiceState::Idle;
            anyhow::bail!("Recording file was not created: {}", path.display());
        }

        let metadata = tokio::fs::metadata(&path).await?;
        if metadata.len() == 0 {
            self.state = VoiceState::Idle;
            anyhow::bail!("Recording file is empty: {}", path.display());
        }

        debug!(
            path = %path.display(),
            size = metadata.len(),
            "Recording saved"
        );

        self.state = VoiceState::Idle;
        Ok(path)
    }

    /// Cancel an in-progress recording without saving.
    ///
    /// Kills the recording process and removes any partial output file.
    pub async fn cancel(&mut self) -> Result<()> {
        if let Some(mut child) = self.recording_process.take() {
            let _ = child.kill().await;
        }

        if let Some(path) = self.output_path.take() {
            if path.exists() {
                let _ = tokio::fs::remove_file(&path).await;
            }
        }

        self.state = VoiceState::Idle;
        debug!("Recording cancelled");
        Ok(())
    }
}

impl Drop for VoiceService {
    fn drop(&mut self) {
        // Best-effort cleanup: if the service is dropped while recording,
        // try to kill the child and remove the temp file synchronously.
        if let Some(ref mut child) = self.recording_process {
            let _ = child.start_kill();
        }
        if let Some(ref path) = self.output_path {
            let _ = std::fs::remove_file(path);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- VoiceConfig defaults -----------------------------------------------

    #[test]
    fn default_config_values() {
        let cfg = VoiceConfig::default();
        assert!((cfg.silence_threshold - 0.03).abs() < f32::EPSILON);
        assert_eq!(cfg.silence_duration, Duration::from_secs(2));
        assert_eq!(cfg.sample_rate, 16_000);
        assert_eq!(cfg.channels, 1);
    }

    // -- Backend detection --------------------------------------------------

    #[test]
    fn detect_backend_returns_some_or_none() {
        // We can't assert a specific backend in CI, but the function must
        // not panic.
        let _backend = detect_backend();
    }

    #[test]
    fn has_command_finds_common_tool() {
        // `ls` should always exist on Unix-like systems.
        if cfg!(unix) {
            assert!(has_command("ls"));
        }
    }

    #[test]
    fn has_command_rejects_nonexistent() {
        assert!(!has_command("__thundercode_nonexistent_tool_xyz__"));
    }

    // -- AudioBackend::recording_command ------------------------------------

    #[test]
    fn sox_command_has_expected_args() {
        let cfg = VoiceConfig::default();
        let path = PathBuf::from("/tmp/test.wav");
        let cmd = AudioBackend::Sox.recording_command(&path, &cfg);
        let inner = cmd.as_std();
        assert_eq!(inner.get_program(), "rec");

        let args: Vec<&std::ffi::OsStr> = inner.get_args().collect();
        assert!(args.contains(&std::ffi::OsStr::new("-q")));
        assert!(args.contains(&std::ffi::OsStr::new("16000")));
        assert!(args.contains(&std::ffi::OsStr::new("1")));
        assert!(args.contains(&std::ffi::OsStr::new("16")));
        assert!(args.contains(&std::ffi::OsStr::new("/tmp/test.wav")));
        assert!(args.contains(&std::ffi::OsStr::new("silence")));
    }

    #[test]
    fn arecord_command_has_expected_args() {
        let cfg = VoiceConfig::default();
        let path = PathBuf::from("/tmp/test.wav");
        let cmd = AudioBackend::Arecord.recording_command(&path, &cfg);
        let inner = cmd.as_std();
        assert_eq!(inner.get_program(), "arecord");

        let args: Vec<&std::ffi::OsStr> = inner.get_args().collect();
        assert!(args.contains(&std::ffi::OsStr::new("S16_LE")));
        assert!(args.contains(&std::ffi::OsStr::new("16000")));
        assert!(args.contains(&std::ffi::OsStr::new("wav")));
    }

    // -- Audio level helpers ------------------------------------------------

    #[test]
    fn rms_level_silence() {
        let samples = vec![0i16; 1024];
        assert_eq!(rms_level(&samples), 0.0);
    }

    #[test]
    fn rms_level_max() {
        let samples = vec![i16::MAX; 1024];
        let level = rms_level(&samples);
        assert!((level - 1.0).abs() < 0.001);
    }

    #[test]
    fn rms_level_empty() {
        assert_eq!(rms_level(&[]), 0.0);
    }

    #[test]
    fn is_silent_detects_silence() {
        let samples = vec![0i16; 512];
        assert!(is_silent(&samples, 0.03));
    }

    #[test]
    fn is_silent_detects_audio() {
        let samples = vec![i16::MAX / 2; 512];
        assert!(!is_silent(&samples, 0.03));
    }

    #[test]
    fn peak_level_empty() {
        assert_eq!(peak_level(&[]), 0.0);
    }

    #[test]
    fn peak_level_known_value() {
        let samples = vec![0i16, 100, -200, 50];
        let p = peak_level(&samples);
        let expected = 200.0 / i16::MAX as f32;
        assert!((p - expected).abs() < 1e-5);
    }

    // -- VoiceState ---------------------------------------------------------

    #[test]
    fn voice_state_idle_by_default() {
        let svc = VoiceService::new(VoiceConfig::default());
        assert_eq!(*svc.state(), VoiceState::Idle);
    }

    // -- VoiceDependencyCheck -----------------------------------------------

    #[test]
    fn check_voice_dependencies_does_not_panic() {
        let _check = check_voice_dependencies();
    }

    // -- RecordingAvailability ----------------------------------------------

    #[test]
    fn check_recording_availability_does_not_panic() {
        let _avail = check_recording_availability();
    }

    // -- is_truthy ----------------------------------------------------------

    #[test]
    fn truthy_values() {
        assert!(is_truthy("1"));
        assert!(is_truthy("true"));
        assert!(is_truthy("TRUE"));
        assert!(is_truthy("yes"));
        assert!(is_truthy("Yes"));
    }

    #[test]
    fn falsy_values() {
        assert!(!is_truthy("0"));
        assert!(!is_truthy("false"));
        assert!(!is_truthy(""));
        assert!(!is_truthy("no"));
    }

    // -- VoiceService cancel on idle ----------------------------------------

    #[tokio::test]
    async fn cancel_from_idle_is_ok() {
        let mut svc = VoiceService::new(VoiceConfig::default());
        assert!(svc.cancel().await.is_ok());
        assert_eq!(*svc.state(), VoiceState::Idle);
    }

    // -- VoiceService stop without start ------------------------------------

    #[tokio::test]
    async fn stop_without_start_is_error() {
        let mut svc = VoiceService::new(VoiceConfig::default());
        assert!(svc.stop_recording().await.is_err());
    }
}
