use std::collections::BTreeMap;
use std::error::Error;
use std::io;
use std::sync::Arc;
use std::time::Instant;

// Tauri specific imports
use tauri::{ipc::Response, AppHandle, Emitter, Manager, State}; // Add Manager for event emission if needed for streaming

use kokoros::{
    tts::koko::{InitConfig as TTSKokoInitConfig, TTSKoko},
    utils::mp3::pcm_to_mp3,
    utils::wav::{write_audio_chunk, WavHeader},
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{error, info}; // Still good for Rust-side logging
use uuid::Uuid;

use base64::{engine::general_purpose, Engine as _};

/// Break words used for chunk splitting
const BREAK_WORDS: &[&str] = &[
    "and", "or", "but", "&", "because", "if", "since", "though", "although", "however", "which",
];

#[derive(Debug)]
enum SpeechError {
    // Deciding to modify this example in order to see errors
    // (e.g. with tracing) is up to the developer
    #[allow(dead_code)]
    Koko(Box<dyn Error>),

    #[allow(dead_code)]
    Header(io::Error),

    #[allow(dead_code)]
    Chunk(io::Error),

    #[allow(dead_code)]
    Mp3Conversion(std::io::Error),
}

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

/// Check if a word is a numbered list item: 1. 2) 3: (4), 5(\s)[.\)\:]
fn is_numbered_list_item(word: &str) -> bool {
    let numbered_regex = Regex::new(r"^\(?[0-9]+[.\)\:],?$").unwrap();
    numbered_regex.is_match(word)
}

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

#[derive(Deserialize, Default, Debug, Clone, Copy)]
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

#[derive(Deserialize, Debug, Clone)]
pub struct Voice(String);

impl Default for Voice {
    fn default() -> Self {
        Self("af_heart".into())
    }
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub struct Speed(f32);

impl Default for Speed {
    fn default() -> Self {
        Self(1.)
    }
}

#[derive(Deserialize, Debug)]
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
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum TauriSpeechError {
    TTSGenerationError(String),
    EventEmissionError(String),
    InvalidInput(String),
    InternalError(String),
}

#[derive(Debug)]
struct TTSTask {
    id: usize,
    chunk: String,
    voice: String,
    speed: f32,
    initial_silence: Option<usize>,
    result_tx: mpsc::UnboundedSender<(usize, Vec<u8>)>,
}

#[derive(Debug)]
struct StreamingSession {
    session_id: Uuid,
    start_time: Instant,
}

#[derive(Clone)]
struct TTSWorkerPool {
    tts_instances: Vec<Arc<TTSKoko>>,
}

impl TTSWorkerPool {
    fn new(tts_instances: Vec<TTSKoko>) -> Self {
        Self {
            tts_instances: tts_instances.into_iter().map(Arc::new).collect(),
        }
    }

    fn get_instance(&self, worker_id: usize) -> (Arc<TTSKoko>, String) {
        let index = worker_id % self.tts_instances.len();
        let instance_id = format!("{:02x}", index);
        (Arc::clone(&self.tts_instances[index]), instance_id)
    }

    fn instance_count(&self) -> usize {
        self.tts_instances.len()
    }

    // process_chunk method removed - now handled inline in sequential queue processing
}

impl From<std::io::Error> for TauriSpeechError {
    fn from(err: std::io::Error) -> Self {
        TauriSpeechError::InternalError(format!("IO Error: {}", err))
    }
}

impl From<tokio::sync::mpsc::error::SendError<TTSTask>> for TauriSpeechError {
    fn from(err: tokio::sync::mpsc::error::SendError<TTSTask>) -> Self {
        TauriSpeechError::InternalError(format!("Task channel send error: {}", err))
    }
}

impl From<serde_json::Error> for TauriSpeechError {
    fn from(err: serde_json::Error) -> Self {
        TauriSpeechError::InternalError(format!("JSON serialization error: {}", err))
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for TauriSpeechError {
    fn from(err: Box<dyn std::error::Error + Send + Sync>) -> Self {
        TauriSpeechError::InternalError(format!("Generic error: {}", err))
    }
}

// We will store our TTSKoko instance in Tauri's managed state.
// This allows it to be accessible to commands without recreating it.
pub struct AppState {
    pub worker_pool: Arc<TTSWorkerPool>,
    // For multiple instances, you could have:
    // pub tts_instances: Arc<Vec<TTSKoko>>,
}

#[derive(Debug, serde::Deserialize)]
pub struct SpeechRequestStream {
    pub input: String,
    pub voice: String,
    pub response_format: AudioFormat,
    pub speed: f32,
    pub initial_silence: Option<usize>,
    pub request_id: String, // Keep request_id as part of the request
}

fn get_colored_request_id_with_relative(request_id: &str, start_time: Instant) -> String {
    format!("{}[{}ms]", request_id, start_time.elapsed().as_millis())
}

// This function will be called during Tauri's setup
pub async fn setup_tauri_commands(
    app: &mut tauri::App,
    model_path: &str,
    voices_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let tts = TTSKoko::new(model_path, voices_path).await;
    app.manage(AppState {
        worker_pool: Arc::new(TTSWorkerPool::new(vec![tts])),
    });
    Ok(())
}

/// Tauri command to handle TTS requests.
///
/// This command will process the text-to-speech request and return the audio data
/// as a `Vec<u8>`. Streaming is not directly supported via a single command return,
/// so this will behave like the non-streaming `handle_tts` from the original.
#[tauri::command]
pub async fn start_speech_stream(
    app_state: State<'_, AppState>,
    app_handle: AppHandle, // Tauri's AppHandle to emit events
    request: SpeechRequestStream,
) -> Result<(), TauriSpeechError> {
    let tts_worker_pool = app_state.worker_pool.clone();

    let input = request.input;
    let voice = request.voice;
    let response_format = request.response_format;
    let speed = request.speed;
    let initial_silence = request.initial_silence;
    let request_id = request.request_id;
    let request_start = Instant::now(); // Record start time for this specific command invocation

    // For Tauri streaming, we primarily send PCM data. Mime type for frontend.
    let (mime_type, sample_rate) = ("audio/pcm;codecs='pcm_s16le'", 24000); // Assuming 24kHz 16-bit PCM for streaming

    let mut chunks = split_text_into_speech_chunks(&input, 10);
    let total_chunks = chunks.len();

    let colored_request_id = get_colored_request_id_with_relative(&request_id, request_start);
    dbg!(
        "{} Processing {} chunks for streaming with window size {}",
        colored_request_id,
        total_chunks,
        tts_worker_pool.instance_count()
    );

    if chunks.is_empty() {
        // If the original input was empty, still send an end signal immediately.
        if let Err(e) = app_handle.emit(
            "audio_stream_end",
            serde_json::json!({ "requestId": request_id }),
        ) {
            error!("Failed to emit audio_stream_end for empty input: {:?}", e);
            // Don't return an error here, as the stream conceptually ended successfully
        }
        return Ok(());
    }

    // Emit initial stream start event to frontend
    if let Err(e) = app_handle.emit(
        "audio_stream_start",
        serde_json::json!({
            "requestId": request_id,
            "format": format!("{:?}", response_format).to_lowercase(),
            "mimeType": mime_type,
            "sampleRate": sample_rate,
            "totalChunks": total_chunks,
        }),
    ) {
        error!("Failed to emit audio_stream_start event: {:?}", e);
        return Err(TauriSpeechError::EventEmissionError(format!(
            "Failed to emit stream start event: {}",
            e
        )));
    }

    let (task_tx, mut task_rx) = mpsc::unbounded_channel::<TTSTask>();
    let total_bytes_transferred = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let colored_request_id_session =
        get_colored_request_id_with_relative(&request_id, request_start);
    info!(
        "{} TTS session started - {} chunks streaming",
        colored_request_id_session, total_chunks
    );

    // Queue all tasks for the worker pool
    for (id, chunk) in chunks.into_iter().enumerate() {
        let task = TTSTask {
            id,
            chunk,
            voice: voice.clone(),
            speed,
            initial_silence: if id == 0 { initial_silence } else { None },
            result_tx: mpsc::unbounded_channel().0, // Dummy sender for TTSTask, not used for Tauri events
        };
        // It's crucial to handle send errors for the channel here if it gets disconnected
        if let Err(e) = task_tx.send(task) {
            error!("Failed to send TTS task: {:?}", e);
            return Err(TauriSpeechError::InternalError(format!(
                "Failed to queue task: {}",
                e
            )));
        }
    }
    drop(task_tx); // Signal that no more tasks will be sent

    // Spawn the asynchronous processing and event emitting task
    let tts_worker_pool_clone = tts_worker_pool.clone();
    let total_chunks_expected_worker = total_chunks;
    let request_id_clone_worker = request_id.clone();
    let app_handle_clone_worker = app_handle.clone(); // Clone for the inner task to emit events
    let total_bytes_transferred_clone = total_bytes_transferred.clone(); // Clone for calculation

    tokio::spawn(async move {
        let mut chunk_counter = 0;
        let mut pending_chunks: BTreeMap<
            usize,
            tokio::task::JoinHandle<Result<(usize, Vec<u8>), String>>,
        > = BTreeMap::new();
        let mut next_to_send = 0;
        let mut chunks_processed = 0;
        let window_size = tts_worker_pool_clone.instance_count();

        loop {
            // Receive new tasks from `task_rx` while there's window space
            while pending_chunks.len() < window_size {
                match task_rx.try_recv() {
                    Ok(task) => {
                        let task_id = task.id;
                        let worker_pool_inner_clone = tts_worker_pool_clone.clone();
                        let total_bytes_inner_clone = total_bytes_transferred_clone.clone();
                        let request_id_inner_clone = request_id_clone_worker.clone();

                        let (tts_instance, actual_instance_id) =
                            worker_pool_inner_clone.get_instance(chunk_counter);
                        let chunk_text = task.chunk.clone();
                        let voice = task.voice.clone();
                        let speed = task.speed;
                        let initial_silence = task.initial_silence;
                        let chunk_num = chunk_counter;

                        let handle = tokio::spawn(async move {
                            if chunk_text.trim().is_empty() {
                                return Ok((task_id, Vec::new()));
                            }

                            let result = tokio::task::spawn_blocking(move || {
                                tts_instance
                                    .tts_raw_audio(
                                        &chunk_text,
                                        "en-us", // Hardcoded for now
                                        &voice,
                                        speed,
                                        initial_silence,
                                        Some(&request_id_inner_clone),
                                        Some(&actual_instance_id),
                                        Some(chunk_num),
                                    )
                                    .map(|audio| audio)
                                    .map_err(|e| format!("TTS processing error: {:?}", e))
                            })
                            .await;

                            match result {
                                Ok(Ok(audio_samples)) => {
                                    // Convert f32 samples to i16 PCM bytes
                                    let mut pcm_data = Vec::with_capacity(audio_samples.len() * 2);
                                    for sample in audio_samples {
                                        let pcm_sample =
                                            (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
                                        pcm_data.extend_from_slice(&pcm_sample.to_le_bytes());
                                    }
                                    total_bytes_inner_clone.fetch_add(
                                        pcm_data.len(),
                                        std::sync::atomic::Ordering::Relaxed,
                                    );
                                    Ok((task_id, pcm_data))
                                }
                                Ok(Err(e)) => Err(e),
                                Err(e) => Err(format!("Task execution join error: {:?}", e)),
                            }
                        });

                        pending_chunks.insert(chunk_counter, handle);
                        chunk_counter += 1;
                    }
                    Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                        // No new tasks available right now, check for completed tasks
                        break;
                    }
                    Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                        // Task channel closed, no more new tasks will arrive
                        break;
                    }
                }
            }

            // Process completed chunks in sequential order for sending to frontend
            while let Some(handle) = pending_chunks.remove(&next_to_send) {
                if handle.is_finished() {
                    match handle.await {
                        Ok(Ok((task_id, pcm_data))) => {
                            let base64_chunk = general_purpose::STANDARD.encode(&pcm_data); // Base64 encode
                            if let Err(e) = app_handle_clone_worker.emit(
                                // Emit to frontend
                                "audio_stream_chunk",
                                serde_json::json!({
                                    "requestId": request_id_clone_worker,
                                    "index": task_id,
                                    "chunk": base64_chunk, // Send base64 encoded chunk
                                }),
                            ) {
                                error!(
                                    "Failed to emit audio_stream_chunk event for chunk {}: {:?}",
                                    task_id, e
                                );
                                // Optionally handle this error, e.g., by logging but continuing
                            }
                            next_to_send += 1;
                            chunks_processed += 1;
                        }
                        Ok(Err(e)) => {
                            error!("Error processing TTS chunk {}: {}", next_to_send, e);
                            next_to_send += 1;
                            chunks_processed += 1;
                        }
                        Err(e) => {
                            error!("Join error for chunk {}: {:?}", next_to_send, e);
                            next_to_send += 1;
                            chunks_processed += 1;
                        }
                    }
                } else {
                    // Current `next_to_send` chunk is not finished yet, put it back
                    pending_chunks.insert(next_to_send, handle);
                    break; // Break inner loop to re-evaluate after a delay
                }
            }

            // Exit condition: all chunks processed and no more tasks or pending work
            if pending_chunks.is_empty()
                && task_rx.is_empty()
                && chunks_processed >= total_chunks_expected_worker
            {
                break;
            }

            // Sleep briefly to prevent busy-waiting if nothing is immediately available
            tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
        }

        // Final logging
        let bytes_transferred =
            total_bytes_transferred_clone.load(std::sync::atomic::Ordering::Relaxed);
        let total_samples = bytes_transferred / 2; // Assuming 16-bit PCM (2 bytes per sample)
        let duration_seconds = total_samples as f64 / 24000.0; // Assuming 24kHz sample rate
        let colored_request_id_final =
            get_colored_request_id_with_relative(&request_id_clone_worker, request_start);
        info!(
            "{} TTS session completed - {} chunks, {} bytes, {:.1}s audio, PCM format",
            colored_request_id_final,
            total_chunks_expected_worker,
            bytes_transferred,
            duration_seconds
        );

        // Emit stream end event
        if let Err(e) = app_handle_clone_worker.emit(
            "audio_stream_end",
            serde_json::json!({ "requestId": request_id_clone_worker }),
        ) {
            error!("Failed to emit audio_stream_end event: {:?}", e);
        }
    });

    Ok(())
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
