// src/worklets/pcm-player-processor.js

// This class will run in a separate AudioWorklet thread.
class PcmPlayerProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    // A buffer to hold incoming PCM data.
    // We'll use a Float32Array as the AudioContext graph typically works with floats.
    this.buffer = new Float32Array(0);
    this.sampleRate = 0; // Will be set via port message
    this.channels = 0; // Will be set via port message

    // Listen for messages from the main thread (your Svelte component)
    this.port.onmessage = (event) => {
      if (event.data.type === "init") {
        this.sampleRate = event.data.sampleRate;
        this.channels = event.data.channels;
        console.log(
          `[AudioWorklet] Initialized with sampleRate: ${this.sampleRate}, channels: ${this.channels}`
        );
      } else if (event.data.type === "audio_chunk") {
        // Convert the incoming Int16Array (from backend's i16 PCM) to Float32Array
        // and append it to our buffer.
        const int16Chunk = event.data.chunk; // This is an Int16Array
        const float32Chunk = new Float32Array(int16Chunk.length);
        for (let i = 0; i < int16Chunk.length; i++) {
          // Normalize Int16 to Float32 range [-1.0, 1.0]
          float32Chunk[i] = int16Chunk[i] / 32768.0;
        }

        // Efficiently append the new chunk
        const newBuffer = new Float32Array(
          this.buffer.length + float32Chunk.length
        );
        newBuffer.set(this.buffer);
        newBuffer.set(float32Chunk, this.buffer.length);
        this.buffer = newBuffer;
        // console.log(`[AudioWorklet] Chunk received. Buffer size: ${this.buffer.length}`);
      } else if (event.data.type === "end_stream") {
        console.log("[AudioWorklet] Stream end signal received.");
        // Optionally, clear buffer or signal end of playback if needed
      } else if (event.data.type === "clear_buffer") {
        this.buffer = new Float32Array(0);
        console.log("[AudioWorklet] Buffer cleared.");
      }
    };
  }

  // The process method is called by the Web Audio API system
  // It takes inputs and produces outputs for each audio rendering quantum (128 frames)
  process(_inputs, outputs, _parameters) {
    const outputChannel = outputs[0]; // Assuming a single output node
    const outputBuffer = outputChannel[0]; // First channel of the first output

    // Determine how many frames we can output in this quantum (usually 128)
    const framesToOutput = outputBuffer.length;

    // If we have enough data in our buffer
    if (this.buffer.length >= framesToOutput) {
      // Copy data from our internal buffer to the output buffer
      outputBuffer.set(this.buffer.subarray(0, framesToOutput));
      // Remove the copied data from our internal buffer
      this.buffer = this.buffer.subarray(framesToOutput);
      // console.log(`[AudioWorklet] Outputting ${framesToOutput} frames. Remaining buffer: ${this.buffer.length}`);
    } else {
      // If we don't have enough data, output silence and clear the output buffer
      // This prevents glitches if data isn't arriving fast enough.
      for (let i = 0; i < framesToOutput; i++) {
        outputBuffer[i] = 0;
      }
      // console.log(`[AudioWorklet] Not enough data, outputting silence. Buffer size: ${this.buffer.length}`);
    }

    // Return true to keep the AudioWorkletNode alive.
    // Return false if you want to stop processing (e.g., at stream end and buffer empty).
    // For continuous streaming, usually return true.
    return true;
  }
}

// Register the processor with a unique name
registerProcessor("pcm-player-processor", PcmPlayerProcessor);
