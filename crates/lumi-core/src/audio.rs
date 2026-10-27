pub fn mouth_level(samples: &[f32]) -> u8 {
    if samples.is_empty() {
        return 0;
    }
    let energy: f32 = samples.iter().map(|sample| sample * sample).sum();
    let rms = (energy / samples.len() as f32).sqrt();
    if rms < 0.02 {
        0
    } else if rms < 0.06 {
        1
    } else if rms < 0.12 {
        2
    } else {
        3
    }
}

pub fn frame_mouth_levels(samples: &[f32], frame_len: usize) -> Vec<u8> {
    if frame_len == 0 {
        return Vec::new();
    }
    samples.chunks(frame_len).map(mouth_level).collect()
}

pub fn resample_linear(input: &[f32], from_hz: u32, to_hz: u32) -> Vec<f32> {
    if input.is_empty() || from_hz == 0 || to_hz == 0 {
        return Vec::new();
    }
    if from_hz == to_hz {
        return input.to_vec();
    }
    let out_len = ((input.len() as u64) * (to_hz as u64) / (from_hz as u64)) as usize;
    let mut output = Vec::with_capacity(out_len.max(1));
    for index in 0..out_len {
        let position = index as f32 * from_hz as f32 / to_hz as f32;
        let left = position.floor() as usize;
        let right = (left + 1).min(input.len() - 1);
        let mix = position - left as f32;
        output.push(input[left] * (1.0 - mix) + input[right] * mix);
    }
    output
}

pub fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let energy: f32 = samples.iter().map(|sample| sample * sample).sum();
    (energy / samples.len() as f32).sqrt()
}

/// Loud enough, at the microphone, to count as a person talking.
pub const SPEECH_RMS: f32 = 0.01;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeechPause {
    /// Keep the capture open.
    Keep,
    /// The person has spoken and then paused long enough to answer.
    Endpoint,
    /// Nothing has been said. The quiet audio can be dropped.
    ForgetQuiet,
}

/// Decides when a person has finished a turn: a short stretch of speech,
/// then more than a second of quiet. A pause shorter than that stays in the
/// same turn. Quiet with no speech is forgotten so the microphone can stay open.
pub struct TurnListener {
    rate: u32,
    speech_frames: usize,
    silence_frames: usize,
    quiet_frames: usize,
    started: bool,
}

impl TurnListener {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            rate: sample_rate.max(1),
            speech_frames: 0,
            silence_frames: 0,
            quiet_frames: 0,
            started: false,
        }
    }

    fn frames(&self, seconds: f32) -> usize {
        (self.rate as f32 * seconds).round() as usize
    }

    pub fn push(&mut self, rms: f32, frames: usize) -> SpeechPause {
        if frames == 0 {
            return SpeechPause::Keep;
        }
        if rms >= SPEECH_RMS {
            self.quiet_frames = 0;
            self.silence_frames = 0;
            self.speech_frames += frames;
            if self.speech_frames >= self.frames(0.2) {
                self.started = true;
            }
        } else if self.started {
            self.silence_frames += frames;
            if self.silence_frames >= self.frames(1.0) {
                return SpeechPause::Endpoint;
            }
        } else {
            self.speech_frames = 0;
            self.quiet_frames += frames;
            if self.quiet_frames >= self.frames(5.0) {
                self.quiet_frames = 0;
                return SpeechPause::ForgetQuiet;
            }
        }
        if self.started && self.speech_frames + self.silence_frames >= self.frames(30.0) {
            return SpeechPause::Endpoint;
        }
        SpeechPause::Keep
    }
}

pub fn take_max_duration(samples: &[f32], sample_rate: u32, max_secs: u32) -> &[f32] {
    let max_len = (sample_rate as usize).saturating_mul(max_secs as usize);
    if samples.len() <= max_len {
        samples
    } else {
        &samples[..max_len]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouth_level_uses_four_steps() {
        assert_eq!(mouth_level(&[0.0, 0.0, 0.0]), 0);
        assert_eq!(mouth_level(&[0.04, -0.04, 0.04]), 1);
        assert_eq!(mouth_level(&[0.09, -0.09]), 2);
        assert_eq!(mouth_level(&[0.4, -0.4, 0.4]), 3);
    }

    #[test]
    fn frames_map_one_level_each() {
        let samples = vec![0.0, 0.0, 0.4, -0.4];
        assert_eq!(frame_mouth_levels(&samples, 2), vec![0, 3]);
    }

    #[test]
    fn resample_halves_a_constant_signal() {
        let input = vec![0.5_f32; 8];
        let output = resample_linear(&input, 16000, 8000);
        assert_eq!(output.len(), 4);
        assert!(output.iter().all(|sample| (*sample - 0.5).abs() < 0.001));
    }

    #[test]
    fn duration_cap_keeps_thirty_seconds() {
        let samples = vec![0.1_f32; 48_000];
        let capped = take_max_duration(&samples, 16_000, 30);
        assert_eq!(capped.len(), 48_000);
        let long = vec![0.1_f32; 160_000];
        assert_eq!(
            take_max_duration(&long, 16_000, 30).len(),
            480_000.min(160_000)
        );
        assert_eq!(take_max_duration(&long, 16_000, 1).len(), 16_000);
    }

    #[test]
    fn a_pause_over_one_second_ends_the_turn() {
        let mut listener = TurnListener::new(16_000);
        assert_eq!(listener.push(0.05, 8_000), SpeechPause::Keep);
        assert_eq!(listener.push(0.0, 8_000), SpeechPause::Keep);
        assert_eq!(listener.push(0.05, 1_600), SpeechPause::Keep);
        assert_eq!(listener.push(0.0, 16_000), SpeechPause::Endpoint);
    }

    #[test]
    fn a_short_blip_does_not_end_the_turn() {
        let mut listener = TurnListener::new(16_000);
        assert_eq!(listener.push(0.05, 1_600), SpeechPause::Keep);
        assert_eq!(listener.push(0.0, 16_000), SpeechPause::Keep);
    }

    #[test]
    fn quiet_waiting_is_forgotten() {
        let mut listener = TurnListener::new(16_000);
        assert_eq!(listener.push(0.0, 80_000), SpeechPause::ForgetQuiet);
        assert_eq!(listener.push(0.0, 1_000), SpeechPause::Keep);
    }
}
