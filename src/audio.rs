use std::io::Cursor;

use symphonia::core::{
    audio::{AudioBufferRef, SampleBuffer, SignalSpec},
    codecs::DecoderOptions,
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
};

const TARGET_SAMPLE_RATE: u32 = 16_000;

pub fn normalize_to_wav_mono_16k(input: &[u8]) -> Result<Vec<u8>, String> {
    let cursor = Cursor::new(input.to_vec());
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("wav");

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("probe failed: {e}"))?;

    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| "no default track".to_string())?;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("decoder create failed: {e}"))?;

    let mut mono_f32 = Vec::<f32>::new();
    let mut input_rate = track.codec_params.sample_rate.unwrap_or(TARGET_SAMPLE_RATE);

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(_) => break,
        };

        let decoded = decoder
            .decode(&packet)
            .map_err(|e| format!("decode failed: {e}"))?;

        input_rate = decoded.spec().rate;

        match decoded {
            AudioBufferRef::F32(buf) => collect_mono_f32_from_f32(buf, &mut mono_f32),
            _ => {
                let spec = *decoded.spec();
                let duration = decoded.capacity() as u64;
                let mut sample_buf = SampleBuffer::<f32>::new(duration, spec);
                sample_buf.copy_interleaved_ref(decoded);
                let channels = spec.channels.count().max(1);
                let samples = sample_buf.samples();
                if channels == 1 {
                    mono_f32.extend_from_slice(samples);
                } else {
                    for frame in samples.chunks(channels) {
                        let sum: f32 = frame.iter().copied().sum();
                        mono_f32.push(sum / channels as f32);
                    }
                }
            }
        }
    }

    let resampled = linear_resample(&mono_f32, input_rate, TARGET_SAMPLE_RATE);
    let pcm_i16: Vec<i16> = resampled
        .into_iter()
        .map(|x| (x.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
        .collect();

    Ok(build_wav_mono_i16(TARGET_SAMPLE_RATE, &pcm_i16))
}

fn collect_mono_f32_from_f32(buf: symphonia::core::audio::AudioBuffer<f32>, out: &mut Vec<f32>) {
    let channels = buf.spec().channels.count().max(1);
    if channels == 1 {
        out.extend_from_slice(buf.chan(0));
        return;
    }

    let frames = buf.frames();
    for i in 0..frames {
        let mut sum = 0.0f32;
        for c in 0..channels {
            sum += buf.chan(c)[i];
        }
        out.push(sum / channels as f32);
    }
}

fn linear_resample(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if input.is_empty() || from_rate == to_rate {
        return input.to_vec();
    }

    let ratio = to_rate as f64 / from_rate as f64;
    let out_len = ((input.len() as f64) * ratio).round() as usize;
    let mut out = Vec::with_capacity(out_len.max(1));

    for i in 0..out_len {
        let src_pos = (i as f64) / ratio;
        let idx = src_pos.floor() as usize;
        let frac = (src_pos - idx as f64) as f32;
        let a = *input.get(idx).unwrap_or(&0.0);
        let b = *input.get(idx + 1).unwrap_or(&a);
        out.push(a + (b - a) * frac);
    }

    out
}

fn build_wav_mono_i16(sample_rate: u32, samples: &[i16]) -> Vec<u8> {
    let data_size = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_size as usize);

    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_size).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_size.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_wav_generates_16k_mono_pcm16() {
        let src = build_wav_mono_i16(8_000, &[0, 1000, -1000, 500, -500, 0, 100, -100]);
        let out = normalize_to_wav_mono_16k(&src).expect("normalize ok");

        assert!(out.starts_with(b"RIFF"));
        assert_eq!(&out[8..12], b"WAVE");
        let channels = u16::from_le_bytes([out[22], out[23]]);
        let sample_rate = u32::from_le_bytes([out[24], out[25], out[26], out[27]]);
        let bits = u16::from_le_bytes([out[34], out[35]]);
        assert_eq!(channels, 1);
        assert_eq!(sample_rate, 16_000);
        assert_eq!(bits, 16);
        assert!(out.len() > src.len());
    }
}
