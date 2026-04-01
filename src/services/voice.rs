//! Voice service: audio recording for push-to-talk voice input.
//!
//! Ported from ref/services/voice.ts`. The TypeScript implementation uses
//! native audio capture (cpal) on macOS/Linux/Windows with SoX/arecord
//! fallbacks. This Rust port provides the same interface with platform-
//! specific recording via process spawning (SoX `rec` on macOS, `arecord`
//! on Linux). Native cpal integration can be added later behind a feature
//! flag.

use std::io::Read;
use std::process::{Child, Command, Stdio};

// ---------------------------------------------------------------------------
// VoiceState
// ---------------------------------------------------------------------------

/// Current state of the voice recording subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceState {
    /// No recording in progress.
    Idle,
    /// Actively recording audio from the microphone.
    Recording,
    /// Post-recording: processing/transcribing audio data.
    Processing,
}

// ---------------------------------------------------------------------------
// VoiceService
// ---------------------------------------------------------------------------

/// Voice recording service.
///
/// Manages microphone recording lifecycle. Currently spawns SoX (`rec`) or
/// `arecord` as a child process; native cpal support can be added behind a
/// cargo feature.
pub struct VoiceService {
    state: VoiceState,
    recorder: Option<Child>,
    audio_buffer: Vec<u8>,
}

impl VoiceService {
    /// Create a new idle `VoiceService`.
    pub fn new() -> Self {
        Self {
            state: VoiceState::Idle,
            recorder: None,
            audio_buffer: Vec::new(),
        }
    }

    /// Current state.
    pub fn state(&self) -> VoiceState {
        self.state
    }

    /// Check whether voice recording is available on this platform.
    ///
    /// Returns `true` if we can find a supported recording backend
    /// (`rec` on macOS, `arecord` on Linux).
    pub fn is_available() -> bool {
        if cfg!(target_os = "macos") {
            has_command("rec")
        } else if cfg!(target_os = "linux") {
            has_command("arecord") || has_command("rec")
        } else {
            // Windows: no fallback in the process-spawning path.
            false
        }
    }

    /// Start recording from the default microphone.
    ///
    /// Returns `Ok(())` if recording started successfully, or an error if the
    /// recording backend is unavailable or failed to spawn.
    pub async fn start_recording(&mut self) -> anyhow::Result<()> {
        if self.state != VoiceState::Idle {
            anyhow::bail!("recording already in progress (state: {:?})", self.state);
        }

        self.audio_buffer.clear();

        let child = spawn_recorder()?;
        self.recorder = Some(child);
        self.state = VoiceState::Recording;

        tracing::debug!("voice: recording started");
        Ok(())
    }

    /// Stop recording and return the raw PCM audio data.
    ///
    /// The returned bytes are 16-bit signed LE, 16 kHz, mono PCM. The caller
    /// is responsible for sending this to a speech-to-text service.
    pub async fn stop_recording(&mut self) -> anyhow::Result<Vec<u8>> {
        if self.state != VoiceState::Recording {
            anyhow::bail!(
                "not currently recording (state: {:?})",
                self.state
            );
        }

        self.state = VoiceState::Processing;

        if let Some(mut child) = self.recorder.take() {
            // Read any remaining stdout data before killing.
            if let Some(ref mut stdout) = child.stdout {
                let mut buf = Vec::new();
                let _ = stdout.read_to_end(&mut buf);
                self.audio_buffer.extend_from_slice(&buf);
            }
            let _ = child.kill();
            let _ = child.wait();
        }

        let data = std::mem::take(&mut self.audio_buffer);
        self.state = VoiceState::Idle;

        tracing::debug!(bytes = data.len(), "voice: recording stopped");
        Ok(data)
    }
}

impl Default for VoiceService {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for VoiceService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VoiceService")
            .field("state", &self.state)
            .field("buffer_bytes", &self.audio_buffer.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Recording parameters.
const SAMPLE_RATE: u32 = 16000;
const CHANNELS: u32 = 1;

/// Spawn the appropriate recording process for the current platform.
fn spawn_recorder() -> anyhow::Result<Child> {
    if cfg!(target_os = "macos") {
        spawn_sox_recorder()
    } else if cfg!(target_os = "linux") {
        if has_command("arecord") {
            spawn_arecord_recorder()
        } else {
            spawn_sox_recorder()
        }
    } else {
        anyhow::bail!("voice recording not supported on this platform");
    }
}

/// Spawn SoX `rec` to capture raw PCM to stdout.
fn spawn_sox_recorder() -> anyhow::Result<Child> {
    let child = Command::new("rec")
        .args([
            "-q",
            "--buffer",
            "1024",
            "-t",
            "raw",
            "-r",
            &SAMPLE_RATE.to_string(),
            "-e",
            "signed",
            "-b",
            "16",
            "-c",
            &CHANNELS.to_string(),
            "-", // stdout
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    Ok(child)
}

/// Spawn `arecord` (ALSA) to capture raw PCM to stdout.
fn spawn_arecord_recorder() -> anyhow::Result<Child> {
    let child = Command::new("arecord")
        .args([
            "-f",
            "S16_LE",
            "-r",
            &SAMPLE_RATE.to_string(),
            "-c",
            &CHANNELS.to_string(),
            "-t",
            "raw",
            "-q",
            "-", // stdout
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    Ok(child)
}

/// Check whether a command is available on PATH.
fn has_command(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}
