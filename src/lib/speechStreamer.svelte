<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onMount, onDestroy } from "svelte";
  import { v4 as uuidv4 } from "uuid"; // You'll need to install 'uuid' (e.g., `npm install uuid @types/uuid`)

  // Component Props (example for customization)
  export let defaultText: string =
    "Hello, this is a streaming audio test from Tauri using AudioWorklet!";
  export let defaultVoice: string = "af_heart"; // Use a voice that works for smaller texts
  export let defaultSpeed: number = 1.0;
  export let defaultFormat: "mp3" | "pcm" = "pcm"; // 'pcm' for streaming

  // Svelte reactive variables for UI feedback
  let inputText: string = defaultText;
  let voice: string = defaultVoice;
  let speed: number = defaultSpeed;
  let format: "mp3" | "pcm" = defaultFormat;
  let isPlaying: boolean = false;
  let statusMessage: string = "Ready";
  let currentRequestId: string | null = null;
  let currentChunkIndex: number = 0;
  let totalChunksExpected: number = 0;

  // Web Audio API variables for AudioWorklet
  let audioContext: AudioContext | null = null;
  let audioWorkletNode: AudioWorkletNode | null = null;

  // Tauri Unlisten Functions
  let unlistenStart: UnlistenFn | undefined;
  let unlistenChunk: UnlistenFn | undefined;
  let unlistenEnd: UnlistenFn | undefined;

  onMount(async () => {
    // Initialize AudioContext
    audioContext = new (window.AudioContext ||
      (window as any).webkitAudioContext)();

    // Handle user interaction to resume AudioContext (browser policy)
    const resumeAudioContext = () => {
      if (audioContext && audioContext.state === "suspended") {
        audioContext.resume().then(() => {
          console.log("AudioContext resumed successfully");
        });
      }
      // Remove listeners after first interaction
      document.removeEventListener("click", resumeAudioContext);
      document.removeEventListener("keydown", resumeAudioContext);
    };
    document.addEventListener("click", resumeAudioContext);
    document.addEventListener("keydown", resumeAudioContext);
  });

  onDestroy(() => {
    // Cleanup listeners and audio resources
    if (unlistenStart) unlistenStart();
    if (unlistenChunk) unlistenChunk();
    if (unlistenEnd) unlistenEnd();

    if (audioWorkletNode) {
      audioWorkletNode.disconnect();
      audioWorkletNode = null;
    }
    if (audioContext) {
      audioContext.close();
      audioContext = null;
    }
  });

  // --- Stream Control Functions ---

  async function startStream() {
    statusMessage = "Starting...";
    isPlaying = true;
    currentRequestId = uuidv4();
    currentChunkIndex = 0;
    totalChunksExpected = 0;

    // Ensure AudioContext is running
    if (audioContext && audioContext.state === "suspended") {
      await audioContext.resume();
    }

    // Register the AudioWorklet processor if not already done
    try {
      await audioContext!.audioWorklet.addModule(
        "/worklets/pcm-player-processor.js"
      );
      console.log("AudioWorklet module added successfully");
    } catch (e) {
      console.error("Error adding AudioWorklet module:", e);
      statusMessage = `Error: Could not load audio module.`;
      stopStream();
      return;
    }

    // Create a new AudioWorkletNode
    audioWorkletNode = new AudioWorkletNode(
      audioContext!,
      "pcm-player-processor"
    );
    audioWorkletNode.connect(audioContext!.destination); // Connect to speakers

    // Set up Tauri event listeners BEFORE invoking the command
    unlistenStart = await listen<any>("audio_stream_start", (event) => {
      if (event.payload.requestId !== currentRequestId) return; // Ignore old requests
      console.log("Stream started:", event.payload);
      statusMessage = "Receiving audio...";
      totalChunksExpected = event.payload.totalChunks;

      // Initialize the AudioWorkletProcessor with sample rate and channels
      audioWorkletNode!.port.postMessage({
        type: "init",
        sampleRate: event.payload.sampleRate || audioContext!.sampleRate, // Use actual sample rate or AudioContext default
        channels: event.payload.channels || 1, // Use actual channels or default to 1
      });
    });

    unlistenChunk = await listen<any>("audio_stream_chunk", (event) => {
      if (event.payload.requestId !== currentRequestId) return; // Ignore old requests
      // console.log(`Chunk ${event.payload.index}/${totalChunksExpected} received`);

      const base64Chunk = event.payload.chunk;
      const binaryString = atob(base64Chunk); // Decode Base64 to binary string
      const len = binaryString.length;
      const bytes = new Uint8Array(len);
      for (let i = 0; i < len; i++) {
        bytes[i] = binaryString.charCodeAt(i);
      }
      // Assuming backend sends 16-bit signed PCM (i16), convert to Int16Array
      const pcmInt16 = new Int16Array(bytes.buffer);

      // Send the PCM chunk to the AudioWorkletProcessor
      audioWorkletNode!.port.postMessage({
        type: "audio_chunk",
        chunk: pcmInt16,
      });

      currentChunkIndex = event.payload.index; // Update current index
    });

    unlistenEnd = await listen<any>("audio_stream_end", (event) => {
      if (event.payload.requestId !== currentRequestId) return; // Ignore old requests
      console.log("Stream ended:", event.payload);
      statusMessage = "Stream ended. Playback complete.";
      isPlaying = false;
      // Signal stream end to worklet (optional, depends on behavior)
      audioWorkletNode!.port.postMessage({ type: "end_stream" });
      cleanupListeners();
    });

    try {
      // Invoke the Tauri command to start the backend stream
      await invoke("start_speech_stream", {
        request: {
          input: inputText,
          voice: voice,
          response_format: format, // This is the requested format, backend streams PCM
          speed: speed,
          initial_silence: 500, // Example initial silence
          request_id: currentRequestId,
        },
      });
      statusMessage = "Backend stream initiated.";
    } catch (error) {
      console.error("Error invoking start_speech_stream:", error);
      statusMessage = `Error: ${error}`;
      stopStream(`Error initiating stream: ${error}`);
    }
  }

  function stopStream(message?: string) {
    if (isPlaying) {
      if (audioWorkletNode) {
        audioWorkletNode.disconnect(); // Disconnect from audio graph
        audioWorkletNode.port.postMessage({ type: "clear_buffer" }); // Clear any pending audio
        audioWorkletNode = null;
      }
      isPlaying = false;
      statusMessage = message || "Stream stopped by user.";
      cleanupListeners();
    }
  }

  function cleanupListeners() {
    if (unlistenStart) {
      unlistenStart();
      unlistenStart = undefined;
    }
    if (unlistenChunk) {
      unlistenChunk();
      unlistenChunk = undefined;
    }
    if (unlistenEnd) {
      unlistenEnd();
      unlistenEnd = undefined;
    }
  }
</script>

<div class="speech-container">
  <h2>Tauri TTS Streaming Demo (AudioWorklet)</h2>

  <textarea bind:value={inputText} placeholder="Enter text to speak"></textarea>

  <div class="controls">
    <label for="voice">Voice:</label>
    <input type="text" id="voice" bind:value={voice} />

    <label for="speed">Speed:</label>
    <input
      type="number"
      id="speed"
      bind:value={speed}
      step="0.1"
      min="0.5"
      max="2.0"
    />

    <label for="format">Format:</label>
    <select id="format" bind:value={format}>
      <option value="pcm">PCM</option>
      <option value="mp3">MP3 (backend streams as PCM)</option>
    </select>
  </div>

  <div class="controls">
    {#if !isPlaying}
      <button class="start" on:click={startStream}>Start Streaming</button>
    {:else}
      <button class="stop" on:click={stopStream}>Stop Streaming</button>
    {/if}
  </div>

  <p class="status">Status: {statusMessage}</p>

  {#if isPlaying && totalChunksExpected > 0}
    <progress value={currentChunkIndex} max={totalChunksExpected}></progress>
    <p>Processing chunk {currentChunkIndex + 1} of {totalChunksExpected}</p>
  {/if}
</div>

<style>
  .speech-container {
    display: flex;
    flex-direction: column;
    gap: 15px;
    padding: 20px;
    border: 1px solid #ccc;
    border-radius: 8px;
    max-width: 600px;
    margin: 20px auto;
    background-color: #f9f9f9;
  }
  textarea {
    width: 100%;
    min-height: 100px;
    padding: 10px;
    border: 1px solid #ddd;
    border-radius: 4px;
    font-size: 1rem;
    resize: vertical;
  }
  .controls {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
    align-items: center;
  }
  input[type="text"],
  input[type="number"],
  select {
    padding: 8px;
    border: 1px solid #ddd;
    border-radius: 4px;
  }
  button {
    padding: 10px 15px;
    border: none;
    border-radius: 5px;
    cursor: pointer;
    font-size: 1rem;
    transition: background-color 0.2s;
  }
  button.start {
    background-color: #4caf50;
    color: white;
  }
  button.start:hover {
    background-color: #45a049;
  }
  button.stop {
    background-color: #f44336;
    color: white;
  }
  button.stop:hover {
    background-color: #da190b;
  }
  .status {
    margin-top: 10px;
    font-style: italic;
    color: #555;
  }
  progress {
    width: 100%;
    margin-top: 10px;
  }
</style>
