<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onDestroy, onMount } from "svelte";
  import Star from "$lib/star.svelte";
  import ModelViewer from "$lib/modelViewer.svelte";
  import { cubicOut } from "svelte/easing";
  import { resolveRoute } from "$app/paths";

  let hidden = $state(false);
  let place = $state("start typing");
  let responding = $state(false);
  let audioGen = $state(false);
  let selectedVoice: string = "af_aoede.3+af_heart.7";
  let responseFormat: "mp3" | "wav" | "pcm" = "mp3";
  let speed: number = 1.2;
  let audioSrc: string = $state("");

  let userText: string = $state("");

  let animationIndex = $state(0);

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

  async function generateResponse(event: KeyboardEvent) {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
    } else {
      return;
    }
    if (userText == "") {
      return;
    }
    const userPrompt = userText;
    place = "";
    responding = true;
    userText = "";
    await invoke("gen_res", {
      prompt: userPrompt,
    }).then((result: any) => {
      if (responding) {
        const response = (result.split("</think>")[1] || result).trim();
        place = response;
        generateAudio(response).then(() => {
          responding = false;
          audioGen = false;
        });
      }
    });
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

  function handleAIButton(event: Event) {
    if (responding) {
      console.log("Canceled");
      event.preventDefault;
      responding = false;
    }
    event.preventDefault;
    animationIndex++;
  }

  onDestroy(() => {
    if (audioSrc) {
      URL.revokeObjectURL(audioSrc);
    }
  });
</script>

<ModelViewer bind:hidden {responding} {animationIndex} />

<div
  class="z-10 fixed bottom-0 h-[35dvh] bg-white overflow-clip py-2 origin-left {hidden
    ? 'w-0 left-0 px-0'
    : 'px-[3dvw] w-[96dvw] left-[2dvw] -skew-x-6'}"
>
  <textarea
    bind:value={userText}
    readonly={responding}
    onkeydown={(e) => generateResponse(e)}
    class="mt-[10dvh] font-body {hidden
      ? 'overflow-y-clip'
      : 'overflow-y-scroll'} overflow-x-clip w-full h-[21dvh] p-[1dvw] placeholder:text-g4 resize-none outline-[0.5dvh] {userText ==
    ''
      ? 'outline-g3/[0.1]'
      : 'outline-g2/[0.2]'}"
    placeholder={place}
  ></textarea>
</div>

<div
  class="z-20 fixed {!hidden
    ? '-skew-x-6'
    : ''} top-[67dvh] left-[6dvh] bg-white p-1 flex flex-row items-center gap-2 outline-[0.5dvh] {userText ==
  ''
    ? 'outline-g3/[0.1]'
    : 'outline-g2/[0.2]'}"
>
  <button
    type="button"
    onclick={() => (hidden = !hidden)}
    oncontextmenu={handleAIButton}
    class="size-[6dvw] cursor-pointer"
  >
    <Star animating={responding} userTyping={userText == ""} />
  </button>
  <h1
    class="font-header text-2xl/[0.5] transition-all overflow-x-clip {userText ==
    ''
      ? 'text-g4'
      : 'text-black'} {hidden
      ? 'max-w-0 -mr-2 skew-x-6'
      : 'max-w-[25dvh] mr-4'}"
  >
    {userText == "" ? "Aives" : "You"}
  </h1>
</div>

<!--<div
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
</button>-->

{#if audioSrc}
  <audio autoplay src={audioSrc}></audio>
{/if}
