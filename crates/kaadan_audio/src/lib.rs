//! Audio playback and sound management powered by [`rodio`].
//!
//! Handles music, sound effects, and volume control for KaadanEngine.

mod engine;

pub use engine::AudioEngine;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_engine_constructs_or_skips_when_no_device() {
        // Headless/CI environments may lack an audio device; treat that as a
        // skip rather than a failure. Where a device exists, volume controls
        // must apply without panicking.
        match AudioEngine::new() {
            Ok(mut engine) => {
                engine.set_master_volume(0.5);
                engine.set_music_volume(0.3);
                engine.set_sfx_volume(0.8);
            }
            Err(_) => {
                eprintln!("no audio output device available; skipping");
            }
        }
    }
}
