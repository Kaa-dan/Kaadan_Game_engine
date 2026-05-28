use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use std::io::Cursor;

pub struct AudioEngine {
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
    music_sink: Option<Sink>,
    sound_sinks: Vec<Sink>,
    master_volume: f32,
    music_volume: f32,
    sfx_volume: f32,
}

impl AudioEngine {
    pub fn new() -> Result<Self, kaadan_core::KaadanError> {
        let (stream, handle) = OutputStream::try_default()
            .map_err(|e| kaadan_core::KaadanError::Other(format!("Audio init failed: {e}")))?;
        Ok(Self {
            _stream: stream,
            stream_handle: handle,
            music_sink: None,
            sound_sinks: Vec::new(),
            master_volume: 1.0,
            music_volume: 0.7,
            sfx_volume: 1.0,
        })
    }

    /// Play a one-shot sound effect.
    pub fn play_sound(&mut self, data: Vec<u8>) -> Result<(), kaadan_core::KaadanError> {
        let cursor = Cursor::new(data);
        let source = Decoder::new(cursor)
            .map_err(|e| kaadan_core::KaadanError::Other(format!("Audio decode: {e}")))?;
        let sink = Sink::try_new(&self.stream_handle)
            .map_err(|e| kaadan_core::KaadanError::Other(format!("Audio sink: {e}")))?;
        sink.set_volume(self.master_volume * self.sfx_volume);
        sink.append(source);
        self.sound_sinks.retain(|s| !s.empty());
        self.sound_sinks.push(sink);
        Ok(())
    }

    /// Play background music (replaces current music).
    pub fn play_music(
        &mut self,
        data: Vec<u8>,
        looping: bool,
    ) -> Result<(), kaadan_core::KaadanError> {
        if let Some(sink) = self.music_sink.take() {
            sink.stop();
        }
        let cursor = Cursor::new(data);
        let source = Decoder::new(cursor)
            .map_err(|e| kaadan_core::KaadanError::Other(format!("Audio decode: {e}")))?;
        let sink = Sink::try_new(&self.stream_handle)
            .map_err(|e| kaadan_core::KaadanError::Other(format!("Audio sink: {e}")))?;
        sink.set_volume(self.master_volume * self.music_volume);
        if looping {
            sink.append(source.repeat_infinite());
        } else {
            sink.append(source);
        }
        self.music_sink = Some(sink);
        Ok(())
    }

    pub fn stop_music(&mut self) {
        if let Some(sink) = self.music_sink.take() {
            sink.stop();
        }
    }

    pub fn pause_music(&self) {
        if let Some(sink) = &self.music_sink {
            sink.pause();
        }
    }

    pub fn resume_music(&self) {
        if let Some(sink) = &self.music_sink {
            sink.play();
        }
    }

    pub fn set_master_volume(&mut self, volume: f32) {
        self.master_volume = volume.clamp(0.0, 1.0);
        self.update_volumes();
    }

    pub fn set_music_volume(&mut self, volume: f32) {
        self.music_volume = volume.clamp(0.0, 1.0);
        self.update_volumes();
    }

    pub fn set_sfx_volume(&mut self, volume: f32) {
        self.sfx_volume = volume.clamp(0.0, 1.0);
    }

    fn update_volumes(&self) {
        if let Some(sink) = &self.music_sink {
            sink.set_volume(self.master_volume * self.music_volume);
        }
    }
}
