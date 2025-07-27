use std::error::Error;
use std::io;
use std::sync::Arc;

// Tauri specific imports
use tauri::Manager; // Add Manager for event emission if needed for streaming

use kokoros::{
    tts::koko::{InitConfig as TTSKokoInitConfig, TTSKoko},
    utils::mp3::pcm_to_mp3,
    utils::wav::{write_audio_chunk, WavHeader},
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::{error, info}; // Still good for Rust-side logging
use uuid::Uuid;

use crate::AppState;

/// Break words used for chunk splitting
const BREAK_WORDS: &[&str] = &[
    "and", "or", "but", "&", "because", "if", "since", "though", "although", "however", "which",
];

#[allow(unused)]
/// Split text into speech chunks for streaming (utility function, not directly a command)
/// Prioritizes sentence boundaries over word count for natural speech breaks
/// Then applies center-break word splitting for long chunks
fn split_text_into_speech_chunks(text: &str, words_per_chunk: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current_chunk = String::new();
    let mut word_count = 0;

    // First pass: split by punctuation
    for word in text.split_whitespace() {
        if !current_chunk.is_empty() {
            current_chunk.push(' ');
        }
        // Check for numbered list patterns: 1. 2) 3: (4), 5(\s)[.\)\:]
        let is_numbered_break = is_numbered_list_item(word);

        if is_numbered_break && !current_chunk.is_empty() {
            chunks.push(current_chunk.trim().to_string());
            current_chunk.clear();
            word_count = 0;
        }
        current_chunk.push_str(word);
        word_count += 1;

        // Check for unconditional breaks (always break regardless of word count)
        let ends_with_unconditional = word.ends_with('.')
            || word.ends_with('!')
            || word.ends_with('?')
            || word.ends_with(':')
            || word.ends_with(';');

        // Check for conditional breaks (commas - only break if enough words)
        let ends_with_conditional = word.ends_with(',');

        // Split conditions:
        // 1. Unconditional punctuation - always break
        // 2. Conditional punctuation + target word count reached
        if ends_with_unconditional
            || is_numbered_break
            || (ends_with_conditional && word_count >= words_per_chunk)
        {
            chunks.push(current_chunk.trim().to_string());
            current_chunk.clear();
            word_count = 0;
        }
    }

    if !current_chunk.trim().is_empty() {
        chunks.push(current_chunk.trim().to_string());
    }

    // Second pass: apply center-break splitting for long chunks
    // All chunks: ≥12 words
    // First 2 chunks: punctuation priority, Others: break words only
    let mut final_chunks = Vec::new();
    for (index, chunk) in chunks.iter().enumerate() {
        let threshold = 12;
        let use_punctuation = index < 2; // First 2 chunks can use punctuation
        let split_chunks = split_long_chunk_with_depth(chunk, threshold, use_punctuation, 0);
        final_chunks.extend(split_chunks);
    }

    // Final processing: Move break words from end of chunks to beginning of next chunk
    for i in 0..final_chunks.len() - 1 {
        let current_chunk = &final_chunks[i];
        let words: Vec<&str> = current_chunk.trim().split_whitespace().collect();

        if let Some(last_word) = words.last() {
            // Check if last word is a break word (case insensitive)
            if BREAK_WORDS.contains(&last_word.to_lowercase().as_str()) && words.len() > 1 {
                // Only move if it won't create an empty chunk (need more than 1 word)
                let new_current = words[..words.len() - 1].join(" ");

                // Add break word to beginning of next chunk
                let next_chunk = &final_chunks[i + 1];
                let new_next = format!("{} {}", last_word, next_chunk);

                // Update the chunks
                final_chunks[i] = new_current;
                final_chunks[i + 1] = new_next;
            }
        }
    }

    final_chunks
}

#[allow(unused)]
/// Check if a word is a numbered list item: 1. 2) 3: (4), 5(\s)[.\)\:]
fn is_numbered_list_item(word: &str) -> bool {
    let numbered_regex = Regex::new(r"^\(?[0-9]+[.\)\:],?$").unwrap();
    numbered_regex.is_match(word)
}

#[allow(unused)]
fn split_long_chunk_with_depth(
    chunk: &str,
    threshold: usize,
    use_punctuation: bool,
    depth: usize,
) -> Vec<String> {
    // Prevent infinite recursion
    if depth >= 3 {
        return vec![chunk.to_string()];
    }
    let words: Vec<&str> = chunk.split_whitespace().collect();
    let word_count = words.len();

    // Only split if chunk meets the threshold
    if word_count < threshold {
        return vec![chunk.to_string()];
    }

    let center = word_count / 2;

    if use_punctuation {
        // Priority 1: Search for commas closest to center
        if let Some(pos) = find_closest_punctuation(&words, center, &[","]) {
            if pos >= 3 && pos < words.len() {
                let first_chunk = words[..pos].join(" ");
                let second_chunk = words[pos..].join(" ");

                // Recursively split both chunks if they're still too long
                let mut result = Vec::new();
                result.extend(split_long_chunk_with_depth(
                    &first_chunk,
                    threshold,
                    use_punctuation,
                    depth + 1,
                ));
                result.extend(split_long_chunk_with_depth(
                    &second_chunk,
                    threshold,
                    use_punctuation,
                    depth + 1,
                ));
                return result;
            }
        }
    }

    // Priority 2: Search for break words closest to center
    if let Some(pos) = find_closest_break_word(&words, center, BREAK_WORDS) {
        if pos >= 3 && pos < words.len() {
            let first_chunk = words[..pos].join(" ");
            let second_chunk = words[pos..].join(" ");

            // Recursively split both chunks if they're still too long
            let mut result = Vec::new();
            result.extend(split_long_chunk_with_depth(
                &first_chunk,
                threshold,
                use_punctuation,
                depth + 1,
            ));
            result.extend(split_long_chunk_with_depth(
                &second_chunk,
                threshold,
                use_punctuation,
                depth + 1,
            ));
            return result;
        }
    }

    // No suitable break point found, return original chunk
    vec![chunk.to_string()]
}

#[allow(unused)]
/// Find closest punctuation to center
fn find_closest_punctuation(words: &[&str], center: usize, punctuation: &[&str]) -> Option<usize> {
    let mut closest_pos = None;
    let mut min_distance = usize::MAX;

    for (i, word) in words.iter().enumerate() {
        if punctuation.iter().any(|p| word.ends_with(p)) {
            let distance = if i < center { center - i } else { i - center };
            if distance < min_distance {
                min_distance = distance;
                closest_pos = Some(i + 1); // Split after the punctuation
            }
        }
    }

    closest_pos
}

#[allow(unused)]
/// Find closest break word to center
fn find_closest_break_word(words: &[&str], center: usize, break_words: &[&str]) -> Option<usize> {
    let mut closest_pos = None;
    let mut min_distance = usize::MAX;

    for (i, word) in words.iter().enumerate() {
        if break_words.contains(&word.to_lowercase().as_str()) {
            let distance = if i < center { center - i } else { i - center };
            if distance < min_distance {
                min_distance = distance;
                closest_pos = Some(i); // Break word becomes first word of second chunk
            }
        }
    }

    closest_pos
}

#[derive(Deserialize, Default, Debug)]
#[serde(rename_all = "lowercase")]
enum AudioFormat {
    #[default]
    Mp3,
    Wav,
    Opus,
    Aac,
    Flac,
    Pcm,
}

#[derive(Deserialize)]
pub struct Voice(String);

impl Default for Voice {
    fn default() -> Self {
        Self("af_sky".into())
    }
}

#[derive(Deserialize)]
pub struct Speed(f32);

impl Default for Speed {
    fn default() -> Self {
        Self(1.)
    }
}

#[derive(Deserialize)]
// This struct will be directly deserialized from the JavaScript payload
pub struct SpeechRequest {
    // Only one Kokoro model exists
    #[allow(dead_code)]
    model: String,

    input: String,

    #[serde(default)]
    voice: Voice,

    #[serde(default)]
    response_format: AudioFormat,

    #[serde(default)]
    speed: Speed,

    #[serde(default)]
    initial_silence: Option<usize>,

    /// Enable streaming audio generation (not directly supported by simple commands, handled differently)
    #[serde(default)]
    #[allow(dead_code)]
    stream: Option<bool>,

    // OpenAI API compatibility parameters - accepted but not implemented
    // These fields ensure request parsing compatibility with OpenAI clients
    /// Return download link after generation (not implemented)
    #[serde(default)]
    #[allow(dead_code)]
    return_download_link: Option<bool>,

    /// Language code for text processing (not implemented)
    #[serde(default)]
    #[allow(dead_code)]
    lang_code: Option<String>,

    /// Volume multiplier for output audio (not implemented)
    #[serde(default)]
    #[allow(dead_code)]
    volume_multiplier: Option<f32>,

    /// Format for download when different from response_format (not implemented)
    #[serde(default)]
    #[allow(dead_code)]
    download_format: Option<String>,

    /// Text normalization options (not implemented)
    #[serde(default)]
    #[allow(dead_code)]
    normalization_options: Option<serde_json::Value>,
}

// Custom error struct for Tauri commands, must be Serialize
#[derive(Debug, Serialize)]
pub enum TauriSpeechError {
    KokoError(String),
    IoError(String),
    Mp3ConversionError(String),
}

impl From<Box<dyn Error>> for TauriSpeechError {
    fn from(err: Box<dyn Error>) -> Self {
        TauriSpeechError::KokoError(err.to_string())
    }
}

impl From<io::Error> for TauriSpeechError {
    fn from(err: io::Error) -> Self {
        TauriSpeechError::IoError(err.to_string())
    }
}

// We will store our TTSKoko instance in Tauri's managed state.
// This allows it to be accessible to commands without recreating it.

// This function will be called during Tauri's setup
pub async fn setup_tauri_commands(
    model_path: &str,
    voices_path: &str,
    tts_instance_arc_mutex: Arc<Mutex<Option<TTSKoko>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let tts = TTSKoko::new(model_path, voices_path).await;
    let mut tts_guard = tts_instance_arc_mutex.lock().await;
    *tts_guard = Some(tts);
    Ok(())
}

/// Tauri command to handle TTS requests.
///
/// This command will process the text-to-speech request and return the audio data
/// as a `Vec<u8>`. Streaming is not directly supported via a single command return,
/// so this will behave like the non-streaming `handle_tts` from the original.
#[tauri::command]
pub async fn generate_speech(
    app_handle: tauri::AppHandle, // Access managed state
    speech_request: SpeechRequest,
) -> Result<Vec<u8>, TauriSpeechError> {
    let app_state = app_handle.state::<AppState>();
    let tts_single = &app_state.tts_instance;

    let tts_guard = tts_single.lock().await;

    let SpeechRequest {
        input,
        voice: Voice(voice),
        response_format,
        speed: Speed(speed),
        initial_silence,
        stream: _, // This will be ignored for a direct command return
        ..
    } = speech_request;

    // For a Tauri command, we'll always behave like non-streaming,
    // as direct streaming is not a return type for commands.
    // If stream was true, we'd typically emit events.

    let request_id = Uuid::new_v4().to_string()[..8].to_string(); // Simple ID for logging
    info!(
        "TTS command received: request_id={}, input_len={}, voice={}, format={:?}",
        request_id,
        input.len(),
        voice,
        response_format
    );

    if let Some(tts) = tts_guard.as_ref() {
        let raw_audio = tts
            .tts_raw_audio(
                &input,
                "en-us", // Assuming English for simplicity, could be derived from voice or added to request
                &voice,
                speed,
                initial_silence,
                Some(&request_id),
                Some("00"), // Instance ID for a single instance
                None,
            )
            .map_err(|e| {
                error!("Koko TTS error: {:?}", e);
                TauriSpeechError::KokoError(format!("TTS generation failed: {}", e))
            })?;
        let sample_rate = TTSKokoInitConfig::default().sample_rate;

        let (audio_data, format_name) = match response_format {
            AudioFormat::Wav => {
                let mut wav_data = Vec::default();
                let header = WavHeader::new(1, sample_rate, 32);
                header.write_header(&mut wav_data).map_err(|e| {
                    error!("WAV header error: {:?}", e);
                    TauriSpeechError::IoError(format!("Failed to write WAV header: {}", e))
                })?;
                write_audio_chunk(&mut wav_data, &raw_audio).map_err(|e| {
                    error!("WAV chunk error: {:?}", e);
                    TauriSpeechError::IoError(format!("Failed to write WAV chunk: {}", e))
                })?;

                (wav_data, "WAV")
            }
            AudioFormat::Mp3 => {
                let mp3_data = pcm_to_mp3(&raw_audio, sample_rate).map_err(|e| {
                    error!("MP3 conversion error: {:?}", e);
                    TauriSpeechError::Mp3ConversionError(format!("Failed to convert to MP3: {}", e))
                })?;
                (mp3_data, "MP3")
            }
            AudioFormat::Pcm => {
                let mut pcm_data = Vec::with_capacity(raw_audio.len() * 2);
                for sample in raw_audio {
                    let pcm_sample = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
                    pcm_data.extend_from_slice(&pcm_sample.to_le_bytes());
                }
                (pcm_data, "PCM")
            }
            _ => {
                let mp3_data = pcm_to_mp3(&raw_audio, sample_rate).map_err(|e| {
                    error!("MP3 conversion error for fallback: {:?}", e);
                    TauriSpeechError::Mp3ConversionError(format!("Failed to convert to MP3: {}", e))
                })?;
                (mp3_data, "MP3")
            }
        };

        info!(
            "TTS command completed - {} bytes, {} format for request_id={}",
            audio_data.len(),
            format_name,
            request_id
        );

        Ok(audio_data)
    } else {
        Err(TauriSpeechError::KokoError(format!(
            "TTS instance not initialized yet"
        )))
    }
}

#[derive(Serialize)]
pub struct ModelObject {
    id: String,
    object: String,
    created: u64,
    owned_by: String,
}

// Removed: handle_home, handle_model, request_id_middleware
// These are specific to HTTP server routing and middleware.
// The `handle_model` logic could be integrated into `list_models` if individual model lookup is needed.
