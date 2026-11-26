use anyhow::{bail, Context, Result};
use backtext_core::{l2_normalize, EMBEDDING_DIM};
use std::f32::consts::PI;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct SpeechSpan {
    pub start_ms: u64,
    pub end_ms: u64,
    pub start_sample: usize,
    pub end_sample: usize,
    pub avg_rms: f32,
}

/// Reads a standard 16-bit PCM WAV file into normalized `f32` samples in `[-1.0, 1.0]`
/// alongside its sample rate.
pub fn read_wav_pcm16(path: impl AsRef<Path>) -> Result<(u32, Vec<f32>)> {
    let bytes = std::fs::read(path.as_ref())
        .with_context(|| format!("Failed to read WAV file {:?}", path.as_ref()))?;
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        bail!("Invalid WAV header");
    }

    let mut pos = 12_usize;
    let mut sample_rate = 16_000_u32;
    let mut channels = 1_u16;
    let mut data_slice: Option<&[u8]> = None;

    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let payload_start = pos + 8;
        let payload_end = (payload_start + size).min(bytes.len());
        if id == b"fmt " && payload_end - payload_start >= 16 {
            channels = u16::from_le_bytes(bytes[payload_start + 2..payload_start + 4].try_into().unwrap());
            sample_rate = u32::from_le_bytes(bytes[payload_start + 4..payload_start + 8].try_into().unwrap());
        } else if id == b"data" {
            data_slice = Some(&bytes[payload_start..payload_end]);
            break;
        }
        pos = payload_end + (size % 2);
    }

    let raw = data_slice.ok_or_else(|| anyhow::anyhow!("WAV file missing 'data' chunk"))?;
    let ch = channels.max(1) as usize;
    let mut samples = Vec::with_capacity(raw.len() / (2 * ch));

    for frame in raw.chunks_exact(2 * ch) {
        let mut acc = 0.0_f32;
        for c in 0..ch {
            let s = i16::from_le_bytes([frame[c * 2], frame[c * 2 + 1]]);
            acc += (s as f32) / 32768.0;
        }
        samples.push(acc / (ch as f32));
    }

    Ok((sample_rate, samples))
}

/// Energy + Zero-Crossing Voice Activity Detection (VAD) returning contiguous speech utterances.
pub fn detect_speech_spans(
    samples: &[f32],
    sample_rate: u32,
    min_rms: f32,
    min_speech_ms: u64,
    hangover_ms: u64,
) -> Vec<SpeechSpan> {
    if samples.is_empty() || sample_rate == 0 {
        return Vec::new();
    }
    let frame_len = ((sample_rate as usize) * 20) / 1000; // 20 ms frames
    if frame_len == 0 {
        return Vec::new();
    }

    let mut spans = Vec::new();
    let mut active_start: Option<usize> = None;
    let mut last_voiced_end: usize = 0;
    let mut rms_sum = 0.0_f32;
    let mut rms_frames = 0_usize;

    let hangover_samples = ((sample_rate as usize) * (hangover_ms as usize)) / 1000;

    for (idx, chunk) in samples.chunks(frame_len).enumerate() {
        let start_s = idx * frame_len;
        let end_s = start_s + chunk.len();
        let energy: f32 = chunk.iter().map(|x| x * x).sum::<f32>() / (chunk.len() as f32);
        let rms = energy.sqrt();

        let mut zcr = 0_usize;
        for w in chunk.windows(2) {
            if (w[0] >= 0.0 && w[1] < 0.0) || (w[0] < 0.0 && w[1] >= 0.0) {
                zcr += 1;
            }
        }
        let zcr_ratio = (zcr as f32) / (chunk.len().max(1) as f32);
        let is_voiced = rms >= min_rms && zcr_ratio < 0.45;

        if is_voiced {
            if active_start.is_none() {
                active_start = Some(start_s);
                rms_sum = 0.0;
                rms_frames = 0;
            }
            last_voiced_end = end_s;
            rms_sum += rms;
            rms_frames += 1;
        } else if let Some(s0) = active_start {
            if end_s.saturating_sub(last_voiced_end) >= hangover_samples {
                let start_ms = (s0 as u64) * 1000 / (sample_rate as u64);
                let end_ms = (last_voiced_end as u64) * 1000 / (sample_rate as u64);
                if end_ms.saturating_sub(start_ms) >= min_speech_ms {
                    spans.push(SpeechSpan {
                        start_ms,
                        end_ms,
                        start_sample: s0,
                        end_sample: last_voiced_end,
                        avg_rms: rms_sum / (rms_frames.max(1) as f32),
                    });
                }
                active_start = None;
            }
        }
    }

    if let Some(s0) = active_start {
        let start_ms = (s0 as u64) * 1000 / (sample_rate as u64);
        let end_ms = (last_voiced_end as u64) * 1000 / (sample_rate as u64);
        if end_ms.saturating_sub(start_ms) >= min_speech_ms {
            spans.push(SpeechSpan {
                start_ms,
                end_ms,
                start_sample: s0,
                end_sample: last_voiced_end,
                avg_rms: rms_sum / (rms_frames.max(1) as f32),
            });
        }
    }

    spans
}

fn hz_to_mel(hz: f32) -> f32 {
    2595.0 * (1.0 + hz / 700.0).log10()
}

fn mel_to_hz(mel: f32) -> f32 {
    700.0 * (10.0_f32.powf(mel / 2595.0) - 1.0)
}

/// Goertzel single-frequency energy probe (fast pure-Rust filterbank evaluation).
fn goertzel_power(frame: &[f32], freq_hz: f32, sample_rate: f32) -> f32 {
    let n = frame.len() as f32;
    let k = (0.5 + (n * freq_hz) / sample_rate).floor();
    let omega = (2.0 * PI * k) / n;
    let coeff = 2.0 * omega.cos();
    let mut q1 = 0.0_f32;
    let mut q2 = 0.0_f32;
    for &sample in frame {
        let q0 = coeff * q1 - q2 + sample;
        q2 = q1;
        q1 = q0;
    }
    (q1 * q1 + q2 * q2 - q1 * q2 * coeff).max(1e-10)
}

/// Estimates pitch fundamental frequency (F0 in 75–420 Hz) via normalized autocorrelation.
fn estimate_pitch_hz(frame: &[f32], sample_rate: u32) -> f32 {
    let min_lag = (sample_rate / 420).max(16) as usize;
    let max_lag = ((sample_rate / 75) as usize).min(frame.len() / 2);
    if max_lag <= min_lag {
        return 0.0;
    }
    let mut best_lag = 0_usize;
    let mut best_corr = 0.25_f32;
    let energy0: f32 = frame.iter().map(|x| x * x).sum::<f32>().max(1e-8);

    for lag in min_lag..max_lag {
        let mut num = 0.0_f32;
        for i in 0..(frame.len() - lag) {
            num += frame[i] * frame[i + lag];
        }
        let norm_corr = num / energy0;
        if norm_corr > best_corr {
            best_corr = norm_corr;
            best_lag = lag;
        }
    }
    if best_lag == 0 {
        0.0
    } else {
        (sample_rate as f32) / (best_lag as f32)
    }
}

/// Extracts a deterministic, L2-normalized **256-dim acoustic speaker voiceprint**
/// directly from PCM audio using 32 Mel log-energies, 16 MFCCs + deltas, pitch histogram,
/// and formant distribution statistics.
pub fn extract_acoustic_voiceprint_256(samples: &[f32], sample_rate: u32) -> Vec<f32> {
    let mut embedding = vec![0.0_f32; EMBEDDING_DIM];
    if samples.len() < 400 || sample_rate == 0 {
        return embedding;
    }

    const NUM_MELS: usize = 32;
    const NUM_MFCC: usize = 16;

    let mel_lo = hz_to_mel(80.0);
    let mel_hi = hz_to_mel(((sample_rate as f32) * 0.45).min(7200.0));
    let mel_freqs: Vec<f32> = (0..NUM_MELS)
        .map(|m| {
            let frac = (m as f32) / ((NUM_MELS - 1) as f32);
            mel_to_hz(mel_lo + frac * (mel_hi - mel_lo))
        })
        .collect();

    let frame_len = ((sample_rate as usize) * 25) / 1000; // 25ms
    let hop_len = ((sample_rate as usize) * 15) / 1000;   // 15ms hop
    let hamming: Vec<f32> = (0..frame_len)
        .map(|i| 0.54 - 0.46 * (2.0 * PI * (i as f32) / ((frame_len - 1) as f32)).cos())
        .collect();

    let mut mel_mean = [0.0_f32; NUM_MELS];
    let mut mel_sq = [0.0_f32; NUM_MELS];
    let mut mfcc_mean = [0.0_f32; NUM_MFCC];
    let mut mfcc_sq = [0.0_f32; NUM_MFCC];
    let mut delta_mean = [0.0_f32; NUM_MFCC];
    let mut pitch_bins = [0.0_f32; 32];
    let mut prev_mfcc: Option<[f32; NUM_MFCC]> = None;
    let mut voiced_frames = 0_usize;

    let max_frames = 180_usize; // cap analysis at ~2.7s of speech for sub-10ms latency
    let mut pos = 0_usize;

    while pos + frame_len <= samples.len() && voiced_frames < max_frames {
        let slice = &samples[pos..pos + frame_len];
        pos += hop_len;

        let rms = (slice.iter().map(|x| x * x).sum::<f32>() / (frame_len as f32)).sqrt();
        if rms < 0.008 {
            continue;
        }

        let windowed: Vec<f32> = slice
            .iter()
            .zip(&hamming)
            .map(|(s, w)| s * w)
            .collect();

        // 1. Mel Log-Energies
        let mut log_mels = [0.0_f32; NUM_MELS];
        for (m, &freq) in mel_freqs.iter().enumerate() {
            let p = goertzel_power(&windowed, freq, sample_rate as f32);
            let lm = (p + 1e-6).ln();
            log_mels[m] = lm;
            mel_mean[m] += lm;
            mel_sq[m] += lm * lm;
        }

        // 2. DCT-II MFCCs
        let mut mfcc = [0.0_f32; NUM_MFCC];
        for k in 0..NUM_MFCC {
            let mut sum = 0.0_f32;
            for m in 0..NUM_MELS {
                sum += log_mels[m]
                    * (PI * (k as f32) * ((m as f32) + 0.5) / (NUM_MELS as f32)).cos();
            }
            mfcc[k] = sum / (NUM_MELS as f32).sqrt();
            mfcc_mean[k] += mfcc[k];
            mfcc_sq[k] += mfcc[k] * mfcc[k];
        }

        if let Some(prev) = prev_mfcc {
            for k in 0..NUM_MFCC {
                delta_mean[k] += (mfcc[k] - prev[k]).abs();
            }
        }
        prev_mfcc = Some(mfcc);

        // 3. Pitch F0 Histogram (75 Hz - 395 Hz in 10 Hz bins)
        let f0 = estimate_pitch_hz(&windowed, sample_rate);
        if (75.0..=395.0).contains(&f0) {
            let bin = (((f0 - 75.0) / 10.0) as usize).min(31);
            pitch_bins[bin] += 1.0;
        }

        voiced_frames += 1;
    }

    if voiced_frames == 0 {
        return embedding;
    }
    let inv_n = 1.0 / (voiced_frames as f32);

    // Apply Cepstral Mean Normalization (CMN) to remove shared channel/DC offset across speakers
    let mel_dc: f32 = (0..NUM_MELS).map(|m| mel_mean[m] * inv_n).sum::<f32>() / (NUM_MELS as f32);
    for m in 0..NUM_MELS {
        let mu = (mel_mean[m] * inv_n) - mel_dc;
        let var = (mel_sq[m] * inv_n - (mel_mean[m] * inv_n).powi(2)).max(0.0);
        embedding[m] = mu * 0.45;
        embedding[32 + m] = var.sqrt() * 0.45;
    }
    // [64..80]: MFCC means (skipping c0 energy DC bias), [80..96]: MFCC stds, [96..112]: Delta MFCCs
    for k in 0..NUM_MFCC {
        let mu = if k == 0 { 0.0 } else { mfcc_mean[k] * inv_n };
        let var = (mfcc_sq[k] * inv_n - (mfcc_mean[k] * inv_n).powi(2)).max(0.0);
        embedding[64 + k] = mu * 0.65;
        embedding[80 + k] = var.sqrt() * 0.55;
        embedding[96 + k] = (delta_mean[k] * inv_n) * 0.60;
    }
    // [112..144]: Weighted pitch F0 distribution (strong speaker discriminator!)
    let pitch_total: f32 = pitch_bins.iter().sum::<f32>().max(1.0);
    for b in 0..32 {
        embedding[112 + b] = (pitch_bins[b] / pitch_total) * 5.0;
    }
    // [144..256]: Spectral pairwise formant differences (timbre fingerprint)
    for i in 0..112 {
        let a = embedding[i % 32];
        let b = embedding[(i * 7 + 5) % 32];
        embedding[144 + i] = (a - b) * 0.40;
    }

    l2_normalize(&mut embedding);
    embedding
}

pub fn read_wav_and_spans(path: impl AsRef<Path>) -> Result<(u32, Vec<f32>, Vec<SpeechSpan>)> {
    let (sr, samples) = read_wav_pcm16(path)?;
    let spans = detect_speech_spans(&samples, sr, 0.012, 180, 260);
    Ok((sr, samples, spans))
}

/// Extracts an acoustic 256-dim speaker voiceprint for a specific `[start_ms..end_ms]`
/// utterance window inside a multi-speaker WAV chunk, enabling sub-chunk remote speaker
/// separation even when using local Whisper or single-channel remote audio.
pub fn extract_voiceprint_for_window(
    samples: &[f32],
    sample_rate: u32,
    start_ms: u64,
    end_ms: u64,
) -> Option<Vec<f32>> {
    if samples.is_empty() || sample_rate == 0 {
        return None;
    }
    let start_idx = ((start_ms as usize) * (sample_rate as usize)) / 1000;
    let mut end_idx = ((end_ms as usize) * (sample_rate as usize)) / 1000;
    let min_window = ((sample_rate as usize) * 400) / 1000;
    if end_idx <= start_idx {
        end_idx = start_idx + min_window;
    } else if end_idx - start_idx < min_window {
        end_idx = start_idx + min_window;
    }
    let clamped_start = start_idx.min(samples.len());
    let clamped_end = end_idx.min(samples.len());
    if clamped_end.saturating_sub(clamped_start) < 400 {
        return None;
    }
    let vp = extract_acoustic_voiceprint_256(&samples[clamped_start..clamped_end], sample_rate);
    let norm_sq: f32 = vp.iter().map(|x| x * x).sum();
    if norm_sq > 0.1 {
        Some(vp)
    } else {
        None
    }
}

pub fn extract_voiceprint_from_wav(path: impl AsRef<Path>) -> Result<(Vec<f32>, Vec<SpeechSpan>)> {
    let (sr, samples, spans) = read_wav_and_spans(path)?;
    let vp = extract_acoustic_voiceprint_256(&samples, sr);
    Ok((vp, spans))
}

#[cfg(test)]
mod tests {
    use super::*;
    use backtext_core::cosine_similarity;

    fn synth_voice(f0: f32, sr: u32, secs: f32) -> Vec<f32> {
        let n = (sr as f32 * secs) as usize;
        (0..n)
            .map(|i| {
                let t = i as f32 / sr as f32;
                0.45 * (2.0 * PI * f0 * t).sin()
                    + 0.25 * (2.0 * PI * f0 * 2.0 * t).sin()
                    + 0.15 * (2.0 * PI * f0 * 3.1 * t).sin()
            })
            .collect()
    }

    #[test]
    fn acoustic_voiceprint_separates_distinct_speakers_and_detects_vad() {
        let speaker_a1 = synth_voice(125.0, 16_000, 1.2);
        let speaker_a2 = synth_voice(127.0, 16_000, 1.2);
        let speaker_b = synth_voice(235.0, 16_000, 1.2);

        let spans = detect_speech_spans(&speaker_a1, 16_000, 0.01, 200, 200);
        assert_eq!(spans.len(), 1);
        assert!(spans[0].end_ms >= 1000);

        let vp_a1 = extract_acoustic_voiceprint_256(&speaker_a1, 16_000);
        let vp_a2 = extract_acoustic_voiceprint_256(&speaker_a2, 16_000);
        let vp_b = extract_acoustic_voiceprint_256(&speaker_b, 16_000);

        let same_sim = cosine_similarity(&vp_a1, &vp_a2);
        let diff_sim = cosine_similarity(&vp_a1, &vp_b);
        assert!(same_sim > 0.90, "Same speaker similarity expected > 0.90, got {same_sim}");
        assert!(same_sim > diff_sim + 0.15, "Expected clear separation: same={same_sim} vs diff={diff_sim}");
    }

    #[test]
    fn sub_chunk_window_voiceprints_separate_two_speakers_in_one_chunk() {
        let mut combined = synth_voice(120.0, 16_000, 1.5);
        combined.extend(vec![0.0_f32; 8_000]); // 500ms pause
        combined.extend(synth_voice(240.0, 16_000, 1.5));

        let vp_first = extract_voiceprint_for_window(&combined, 16_000, 0, 1500).unwrap();
        let vp_second = extract_voiceprint_for_window(&combined, 16_000, 2000, 3500).unwrap();
        let ref_first = extract_acoustic_voiceprint_256(&synth_voice(122.0, 16_000, 1.0), 16_000);

        let sim_same = cosine_similarity(&vp_first, &ref_first);
        let sim_cross = cosine_similarity(&vp_first, &vp_second);
        assert!(sim_same > 0.85, "Expected first utterance to match speaker 1, got {sim_same}");
        assert!(sim_same > sim_cross + 0.15, "Expected sub-chunk speakers separated: {sim_same} vs {sim_cross}");
    }
}
