async fn handle_tts_streaming(
    tts_instances: Vec<TTSKoko>,
    input: String,
    voice: String,
    response_format: AudioFormat,
    speed: f32,
    initial_silence: Option<usize>,
    request_id: String,
    request_start: Instant,
) -> Result<Response, SpeechError> {
    // Streaming implementation: PCM format for optimal performance
    let content_type = match response_format {
        AudioFormat::Pcm => "audio/pcm",
        _ => "audio/pcm", // Force PCM for optimal streaming performance
    };

    // Create worker pool with vector of TTS instances for true parallelism
    let worker_pool = TTSWorkerPool::new(tts_instances);

    // Create speech chunks based on word count and punctuation
    let mut chunks = split_text_into_speech_chunks(&input, 10);

    // Add empty chunk at end as completion signal to client
    chunks.push(String::new());
    let total_chunks = chunks.len();

    let colored_request_id = get_colored_request_id_with_relative(&request_id, request_start);
    debug!(
        "{} Processing {} chunks for streaming with window size {}",
        colored_request_id,
        total_chunks,
        worker_pool.instance_count()
    );

    if chunks.is_empty() {
        return Err(SpeechError::Mp3Conversion(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "No text to process",
        )));
    }

    // Create channels for sequential chunk processing
    let (task_tx, mut task_rx) = mpsc::unbounded_channel::<TTSTask>();
    let (audio_tx, audio_rx) = mpsc::unbounded_channel::<(usize, Vec<u8>)>(); // Tag chunks with order ID

    // Track total bytes transferred
    let total_bytes = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    // Create session for tracking
    let session = StreamingSession {
        session_id: Uuid::new_v4(),
        start_time: Instant::now(),
    };

    let colored_request_id = get_colored_request_id_with_relative(&request_id, request_start);
    info!(
        "{} TTS session started - {} chunks streaming",
        colored_request_id, total_chunks
    );

    // Queue all tasks in order for sequential processing
    for (id, chunk) in chunks.into_iter().enumerate() {
        let task = TTSTask {
            id,
            chunk,
            voice: voice.clone(),
            speed,
            initial_silence: if id == 0 { initial_silence } else { None },
            result_tx: audio_tx.clone(),
        };

        task_tx.send(task).unwrap();
    }

    // Drop the task sender to signal completion
    drop(task_tx);

    // Windowed parallel processing: allow chunks to process concurrently up to available TTS instances
    let worker_pool_clone = worker_pool.clone();
    let total_bytes_clone = total_bytes.clone();
    let audio_tx_clone = audio_tx.clone();
    let total_chunks_expected = total_chunks;
    tokio::spawn(async move {
        use std::collections::BTreeMap;

        let mut chunk_counter = 0;
        let mut pending_chunks: BTreeMap<
            usize,
            tokio::task::JoinHandle<Result<(usize, Vec<u8>), String>>,
        > = BTreeMap::new();
        let mut next_to_send = 0;
        let mut chunks_processed = 0;
        let window_size = worker_pool_clone.instance_count(); // Allow chunks to process in parallel up to available TTS instances

        loop {
            // Receive new tasks while we have window space and tasks are available
            while pending_chunks.len() < window_size {
                // Use a non-blocking approach but with proper channel closure detection
                match task_rx.try_recv() {
                    Ok(task) => {
                        let task_id = task.id;
                        let worker_pool_clone = worker_pool_clone.clone();
                        let total_bytes_clone = total_bytes_clone.clone();
                        let request_id_clone = request_id.clone();

                        // Process chunk with dedicated TTS instance (alternates between instances)
                        let (tts_instance, actual_instance_id) =
                            worker_pool_clone.get_instance(chunk_counter);
                        let chunk_text = task.chunk.clone();
                        let voice = task.voice.clone();
                        let speed = task.speed;
                        let initial_silence = task.initial_silence;
                        let chunk_num = chunk_counter;

                        // Spawn parallel processing
                        let handle = tokio::spawn(async move {
                            // Handle empty chunks (completion signals) without TTS processing
                            if chunk_text.trim().is_empty() {
                                // Empty chunk - send as completion signal
                                return Ok((task_id, Vec::new()));
                            }

                            let result = tokio::task::spawn_blocking(move || {
                                let audio_result = tts_instance.tts_raw_audio(
                                    &chunk_text,
                                    "en-us",
                                    &voice,
                                    speed,
                                    initial_silence,
                                    Some(&request_id_clone),
                                    Some(&actual_instance_id),
                                    Some(chunk_num),
                                );

                                audio_result
                                    .map(|audio| audio)
                                    .map_err(|e| format!("TTS processing error: {:?}", e))
                            })
                            .await;

                            // Convert audio to PCM
                            match result {
                                Ok(Ok(audio_samples)) => {
                                    let mut pcm_data = Vec::with_capacity(audio_samples.len() * 2);
                                    for sample in audio_samples {
                                        let pcm_sample =
                                            (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
                                        pcm_data.extend_from_slice(&pcm_sample.to_le_bytes());
                                    }
                                    total_bytes_clone.fetch_add(
                                        pcm_data.len(),
                                        std::sync::atomic::Ordering::Relaxed,
                                    );
                                    Ok((task_id, pcm_data))
                                }
                                Ok(Err(e)) => Err(e),
                                Err(e) => Err(format!("Task execution error: {:?}", e)),
                            }
                        });

                        pending_chunks.insert(chunk_counter, handle);
                        chunk_counter += 1;
                    }
                    Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                        // No tasks available right now, break inner loop to check completed chunks
                        break;
                    }
                    Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                        // Channel is closed, no more tasks will come
                        break;
                    }
                }
            }

            // Check if we can send the next chunk in order
            if let Some(handle) = pending_chunks.remove(&next_to_send) {
                if handle.is_finished() {
                    match handle.await {
                        Ok(Ok((task_id, pcm_data))) => {
                            if let Err(_) = audio_tx_clone.send((task_id, pcm_data)) {
                                break;
                            }
                            next_to_send += 1;
                            chunks_processed += 1;
                        }
                        Ok(Err(_e)) => {
                            // TTS processing error - skip this chunk
                            next_to_send += 1;
                            chunks_processed += 1;
                        }
                        Err(_e) => {
                            // Task execution error - skip this chunk
                            next_to_send += 1;
                            chunks_processed += 1;
                        }
                    }
                } else {
                    // Not finished yet, put it back
                    pending_chunks.insert(next_to_send, handle);
                }
            }

            // Check if all chunks have been processed and sent
            // We're done when we've processed all expected chunks
            if chunks_processed >= total_chunks_expected {
                break;
            }

            // Also check if we have no more work to do (fallback safety check)
            if pending_chunks.is_empty()
                && task_rx.is_empty()
                && chunks_processed < total_chunks_expected
            {
                // This shouldn't happen, but log it for debugging
                eprintln!(
                    "Warning: Early termination detected - processed {} of {} chunks",
                    chunks_processed, total_chunks_expected
                );
                break;
            }

            // Small delay to prevent busy waiting
            tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
        }

        // Wait for any remaining chunks to complete and collect them
        // This fixes the previous issue where only chunks matching next_to_send exactly were processed
        let mut remaining_chunks = Vec::new();

        for (chunk_id, handle) in pending_chunks {
            match handle.await {
                Ok(Ok((task_id, pcm_data))) => {
                    // Collect all successful chunks regardless of order
                    remaining_chunks.push((chunk_id, task_id, pcm_data));
                }
                Ok(Err(_e)) => {
                    // TTS processing error - still count as processed
                    chunks_processed += 1;
                }
                Err(_e) => {
                    // Task execution error - still count as processed
                    chunks_processed += 1;
                }
            }
        }

        // Sort remaining chunks by chunk_id to maintain proper order
        // This ensures audio continuity even for out-of-order completions
        remaining_chunks.sort_by_key(|(chunk_id, _, _)| *chunk_id);

        // Send all remaining chunks in order, preventing data loss
        for (chunk_id, task_id, pcm_data) in remaining_chunks {
            // Only send chunks that are in the expected sequence (>= next_to_send)
            // This prevents duplicate sends while ensuring no valid chunks are skipped
            if chunk_id >= next_to_send {
                let _ = audio_tx_clone.send((task_id, pcm_data));
                chunks_processed += 1;
            }
        }

        let _session_time = session.start_time.elapsed();

        // Log completion
        let bytes_transferred = total_bytes.load(std::sync::atomic::Ordering::Relaxed);
        // Calculate audio duration: 16-bit PCM (2 bytes per sample) at 24000 Hz
        let total_samples = bytes_transferred / 2;
        let duration_seconds = total_samples as f64 / 24000.0;
        let colored_request_id = get_colored_request_id_with_relative(&request_id, request_start);
        info!(
            "{} TTS session completed - {} chunks, {} bytes, {:.1}s audio, PCM format",
            colored_request_id, total_chunks, bytes_transferred, duration_seconds
        );

        // Send termination signal
        let _ = audio_tx.send((total_chunks, vec![])); // Empty data as termination signal
    });

    // No ordering needed - sequential processing guarantees order

    // Create immediate streaming - chunks are already sent in order from TTS processing
    let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(audio_rx)
        .map(|(_chunk_id, data)| -> Result<Vec<u8>, std::io::Error> {
            // Check for termination signal (empty data)
            if data.is_empty() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "Stream complete",
                ));
            }
            Ok(data)
        })
        .take_while(|result| {
            // Continue until we hit an error (termination signal)
            std::future::ready(result.is_ok())
        });

    // Convert to HTTP body with explicit ordering
    let body = Body::from_stream(stream);

    Ok(Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONNECTION, "keep-alive")
        .header(header::CACHE_CONTROL, "no-cache")
        .header("X-Accel-Buffering", "no") // Disable nginx buffering
        .header("Transfer-Encoding", "chunked") // Enable HTTP chunked transfer encoding
        .header("Access-Control-Allow-Origin", "*") // CORS for browser clients
        .body(body)
        .map_err(|e| {
            SpeechError::Mp3Conversion(std::io::Error::new(std::io::ErrorKind::Other, e))
        })?)
}
