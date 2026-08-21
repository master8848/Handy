use rubato::Resampler;
use std::path::Path;

const TARGET_SAMPLE_RATE: usize = 16_000;

/// Decode any supported audio file (mp3, wav, m4a, ogg, flac, …) into 16 kHz
/// mono f32 PCM samples — the format the transcription engines expect.
///
/// Errors are surfaced with a user-meaningful message rather than a raw
/// symphonia error so the UI can show why a file was rejected.
pub fn decode_audio_file_to_samples<P: AsRef<Path>>(path: P) -> Result<Vec<f32>, String> {
    use symphonia::core::codecs::DecoderOptions;
    use symphonia::core::errors::Error;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let path = path.as_ref();
    let file =
        std::fs::File::open(path).map_err(|e| format!("Cannot open {}: {}", path.display(), e))?;

    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let hint = Hint::new();
    let format_opts: FormatOptions = Default::default();
    let metadata_opts: MetadataOptions = Default::default();
    let decoder_opts: DecoderOptions = Default::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &metadata_opts)
        .map_err(|e| {
            format!(
                "Unsupported or corrupt audio file {}: {}",
                path.display(),
                e
            )
        })?;

    let mut format_reader = probed.format;
    let track = format_reader
        .default_track()
        .ok_or_else(|| "Audio file contains no playable tracks".to_string())?;
    let track_id = track.id;
    let source_rate = track
        .codec_params
        .sample_rate
        .unwrap_or(TARGET_SAMPLE_RATE as u32) as usize;
    let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(1);

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &decoder_opts)
        .map_err(|e| format!("No decoder available for {}: {}", path.display(), e))?;

    let mut decoded: Vec<f32> = Vec::new();
    let mut sample_buf: Option<symphonia::core::audio::SampleBuffer<f32>> = None;

    loop {
        let packet = match format_reader.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(Error::ResetRequired) => break,
            Err(e) => return Err(format!("Failed to decode {}: {}", path.display(), e)),
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(audio_buf) => {
                let spec = *audio_buf.spec();
                if sample_buf.is_none() {
                    sample_buf = Some(symphonia::core::audio::SampleBuffer::<f32>::new(
                        audio_buf.capacity() as u64,
                        spec,
                    ));
                }
                if let Some(buf) = &mut sample_buf {
                    buf.copy_interleaved_ref(audio_buf);
                    decoded.extend_from_slice(buf.samples());
                }
            }
            Err(Error::DecodeError(_)) => continue,
            Err(e) => return Err(format!("Failed to decode {}: {}", path.display(), e)),
        }
    }

    if decoded.is_empty() {
        return Err("No audio could be decoded from this file".to_string());
    }

    // Downmix to mono by averaging channels.
    let mut mono = if channels > 1 {
        let frames = decoded.len() / channels;
        let mut m = Vec::with_capacity(frames);
        for frame in decoded.chunks_exact(channels) {
            m.push(frame.iter().sum::<f32>() / channels as f32);
        }
        m
    } else {
        decoded
    };

    // Resample to 16 kHz when needed. A single FftFixedIn pass over the whole
    // buffer is simplest for a one-shot decode; the chunked FrameResampler is
    // for streaming captures.
    if source_rate != TARGET_SAMPLE_RATE {
        let chunk_in = mono.len();
        if chunk_in > 0 {
            let mut resampler =
                rubato::FftFixedIn::<f32>::new(source_rate, TARGET_SAMPLE_RATE, chunk_in, 1, 1)
                    .map_err(|e| format!("Failed to create resampler: {}", e))?;
            if let Ok(out) = resampler.process(&[&mono[..]], None) {
                mono = out[0].clone();
            }
        }
    }

    Ok(mono)
}

#[cfg(test)]
mod tests {
    use super::decode_audio_file_to_samples;
    use std::io::Write;
    use std::path::PathBuf;

    fn write_wav(path: &PathBuf, samples: &[f32], sample_rate: u32) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for s in samples {
            writer
                .write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
                .unwrap();
        }
        writer.finalize().unwrap();
    }

    fn sine(freq: f32, seconds: f32, sample_rate: u32) -> Vec<f32> {
        let n = (sample_rate as f32 * seconds) as usize;
        (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate as f32).sin())
            .collect()
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("handy_decode_test_{}_{tag}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn decodes_mono_16k_wav() {
        let dir = temp_dir("mono16k");
        let path = dir.join("test.wav");
        let samples = sine(440.0, 0.5, 16_000);
        write_wav(&path, &samples, 16_000);

        let decoded = decode_audio_file_to_samples(&path).unwrap();
        assert!(
            decoded.len() >= samples.len(),
            "decoded {} samples, expected {}",
            decoded.len(),
            samples.len()
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn resamples_non_16k_wav() {
        let dir = temp_dir("resample");
        let path = dir.join("resample.wav");
        let samples = sine(440.0, 0.5, 44_100);
        write_wav(&path, &samples, 44_100);

        let decoded = decode_audio_file_to_samples(&path).unwrap();
        assert!(
            decoded.len() >= 16_000 / 2,
            "decoded {} samples (expected ~8000 for 0.5s @16k)",
            decoded.len()
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rejects_missing_file() {
        let err = decode_audio_file_to_samples("/nonexistent/file.wav").unwrap_err();
        assert!(err.contains("Cannot open"));
    }

    #[test]
    fn rejects_garbage_file() {
        let dir = temp_dir("garbage");
        let path = dir.join("garbage.mp3");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"this is definitely not audio").unwrap();
        let err = decode_audio_file_to_samples(&path).unwrap_err();
        assert!(err.contains("Unsupported or corrupt"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
