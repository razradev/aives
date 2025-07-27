<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount, onDestroy } from "svelte";
  import { moveWindow, Position } from "@tauri-apps/plugin-positioner";
  import Border from "$lib/border.svelte";
  import Star from "$lib/star.svelte";
  import ModelViewer from "$lib/modelViewer.svelte"; // Import the new 3D component
  import SpeechStreamer from "$lib/speechStreamer.svelte";

  moveWindow(Position.BottomLeft);

  let hidden = $state(false);
  let place = $state("________");
  let responding = $state(false);
  let audioGen = $state(false);
  let selectedVoice: string = "af_aoede.3+af_heart.7";
  let responseFormat: "mp3" | "wav" | "pcm" = "mp3";
  let speed: number = 1.2;
  let audioSrc: string = $state("");

  async function handleInput(event: Event) {
    const target = event.target as HTMLElement;
    if (event.inputType === "insertParagraph") {
      const text = target.innerText.trim();
      responding = true;
      target.innerText = "";
      place = "";
      await invoke("gen_res", {
        prompt: text,
      }).then((result: any) => {
        const response = (result.split("</think>")[1] || result).trim();
        console.log(response);
        place = response;
        generateAudio(response).then(() => {
          responding = false;
          audioGen = false;
        });
      });
    } else if (target.innerHTML.trim() === "<br>") {
      target.innerHTML = "";
    }
  }

  async function generateAudio(inputText: string) {
    audioSrc = "";
    try {
      console.log("Calling Command");
      console.log(inputText);
      audioGen = true;
      const audioData: number[] = await invoke("generate_speech", {
        speechRequest: {
          model: "kokoro",
          input: inputText,
          voice: selectedVoice,
          response_format: responseFormat,
          speed: speed,
          initial_silence: 0,
          stream: false,
        },
      });
      const audioBlob = new Blob([new Uint8Array(audioData)], {
        type: `audio/${responseFormat === "mp3" ? "mpeg" : responseFormat}`,
      });
      audioSrc = "";
      audioSrc = URL.createObjectURL(audioBlob);
      console.log("Audio generated successfully!");
    } catch (e: any) {
      console.error("Error generating speech:", e);
    }
  }

  onDestroy(() => {
    if (audioSrc) {
      URL.revokeObjectURL(audioSrc);
    }
  });
</script>

<ModelViewer bind:hidden {responding} />

<div
  class="z-50 fixed bottom-0 left-0 w-full h-1/4 overflow-hidden {hidden
    ? 'scale-0'
    : 'scale-100'} origin-text"
>
  <div class="w-full h-full p-2 gap-x-2 bg-white flex flex-row">
    <div class="w-5 mt-10">
      <Border flip={false} />
    </div>
    <div
      class="w-full h-full resize-none z-50 overflow-y-auto overflow-x-hidden scrollbar"
    >
      <div
        class="w-[3.5dvh] h-[8dvh] bg-white top-0 relative float-left"
        contenteditable="false"
      ></div>
      <div
        class="w-full h-full focus:outline-0 {responding && audioGen
          ? 'line-through text-white decoration-[3dvh] hidden-placeholder animate-decoration-pulse decoration-solid cursor-none pointer-events-none'
          : 'cursor-text'}"
        contenteditable={!responding}
        oninput={async (e) => handleInput(e)}
        placeholder={place}
      ></div>
    </div>
    <div class="w-5">
      <Border flip={true} />
    </div>
  </div>
</div>

<button
  class="p-1 left-0 taskbar fixed z-50 cursor-pointer"
  type="button"
  onclick={() => (hidden = !hidden)}
>
  <div
    class="size-[8dvh] rounded-sm border-2 border-white/[0.2] {hidden
      ? 'bg-white/[0.1]'
      : 'bg-white'}"
  >
    <Star animating={responding} />
  </div>
</button>

{#if audioSrc}
  <audio autoplay src={audioSrc}></audio>
{/if}
