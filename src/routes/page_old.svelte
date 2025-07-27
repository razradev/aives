<script lang="ts">
  import * as THREE from "three";
  import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
  import { VRMLoaderPlugin, VRMUtils } from "@pixiv/three-vrm";
  import { MToonNodeMaterial } from "@pixiv/three-vrm/nodes";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import Border from "$lib/border.svelte";
  import { moveWindow, Position } from "@tauri-apps/plugin-positioner";
  import Star from "$lib/star.svelte";
  import Send from "$lib/send.svelte";

  moveWindow(Position.BottomLeft);

  let hidden = $state(false);
  let place = $state(
    ".................................................................\n..................................................................\n......................................................\n......................................................................................................................................................................................................................................................................................................................................................................................................................................................"
  );
  let responding = $state(false);
  let audioGen = $state(false);
  let selectedVoice: string = "af_aoede.3+af_heart.7";
  let voices: string[] = [];
  let responseFormat: "mp3" | "wav" | "pcm" = "mp3";
  let speed: number = 1.1;
  let audioSrc: string = $state("");

  onMount(() => {
    let appWindow = getCurrentWindow();

    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(15, 1, 0.1, 1000);

    camera.position.z = 5.0;
    camera.position.y = 1.1;

    const light = new THREE.AmbientLight(0xffffff, 1);
    scene.add(light);
    const directionalLight = new THREE.DirectionalLight(0xffffff, 1.5);
    directionalLight.position.set(1, 3, 5);
    directionalLight.target.position.set(0, 1, 0);
    scene.add(directionalLight);
    scene.add(directionalLight.target);

    const renderer = new THREE.WebGLRenderer({
      alpha: true,
      antialias: false,
    });
    renderer.setSize(window.innerWidth, window.innerHeight);
    renderer.setPixelRatio(window.devicePixelRatio);
    renderer.autoClear = false;
    const canvas = document
      .getElementById("renderer")
      ?.appendChild(renderer.domElement);
    canvas.style.filter = "saturate(1.6)";

    let currentVrm = undefined;
    let currentMixer = undefined;
    const loader = new GLTFLoader();
    loader.crossOrigin = "anonymous";

    loader.register((parser) => {
      return new VRMLoaderPlugin(parser);
    });

    loader.load(
      "/Hoshino.vrm",

      (gltf) => {
        const vrm = gltf.userData.vrm;
        // calling these functions greatly improves the performance
        VRMUtils.removeUnnecessaryVertices(gltf.scene);
        VRMUtils.combineSkeletons(gltf.scene);
        VRMUtils.combineMorphs(vrm);
        scene.add(vrm.scene);
        currentVrm = vrm;
        //prepareAnimation(vrm);
      },

      (progress) =>
        console.log(
          "Loading model...",
          100.0 * (progress.loaded / progress.total),
          "%"
        ),

      (error) => console.error(error)
    );

    function prepareAnimation(vrm) {
      currentMixer = new THREE.AnimationMixer(vrm.scene);

      const quatA = new THREE.Quaternion(0.0, 0.0, 0.0, 1.0);
      const quatB = new THREE.Quaternion(0.0, 0.0, 0.0, 1.0);
      quatB.setFromEuler(new THREE.Euler(0.0, 0.0, 0.25 * Math.PI));

      const armTrack = new THREE.QuaternionKeyframeTrack(
        vrm.humanoid.getNormalizedBoneNode("leftUpperArm").name + ".quaternion", // name
        [0.0, 0.5, 1.0], // times
        [...quatA.toArray(), ...quatB.toArray(), ...quatA.toArray()] // values
      );

      const blinkTrack = new THREE.NumberKeyframeTrack(
        vrm.expressionManager.getExpressionTrackName("blink"), // name
        [0.0, 0.5, 1.0], // times
        [0.0, 1.0, 0.0] // values
      );

      const clip = new THREE.AnimationClip("Animation", 1.0, [
        armTrack,
        blinkTrack,
      ]);

      const action = currentMixer.clipAction(clip);
      action.play();
    }

    const clock = new THREE.Clock();
    clock.start();

    function animate() {
      requestAnimationFrame(animate);

      const deltaTime = clock.getDelta();

      if (currentVrm) {
        currentVrm.expressionManager.setValue("ih", 1);

        currentVrm.update(deltaTime);

        if (currentMixer) {
          currentMixer.update(deltaTime);
        }
      }

      if (!hidden) {
        renderer.clear();
      }
      scene.visible = !hidden;

      renderer.render(scene, camera);
    }

    animate();

    setInterval(async () => {
      invoke("check_cursor_region", { hidden: hidden });
    }, 250);
  });

  async function handleInput(event) {
    if (event.inputType == "insertParagraph") {
      const text = event.srcElement.innerText.trim();
      responding = true;
      event.srcElement.innerText = "";
      place = "";
      await invoke("gen_res", {
        prompt: text,
      }).then((result) => {
        const response = (result.split("</think>")[1] || result).trim();
        console.log(response);
        place = response;
        generateAudio(response).then(() => {
          responding = false;
          audioGen = false;
        });
      });
    } else if (event.srcElement.innerHTML.trim() === "<br>") {
      event.srcElement.innerHTML = "";
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
          model: "kokoro", // As per your Rust backend, this is a placeholder
          input: inputText,
          voice: selectedVoice,
          response_format: responseFormat,
          speed: speed,
          initial_silence: 0,
          stream: false,
        },
      });

      // Create a Blob from the received audio data
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

  import { onDestroy } from "svelte";
  onDestroy(() => {
    if (audioSrc) {
      URL.revokeObjectURL(audioSrc);
    }
  });
</script>

<div
  id="renderer"
  class="overflow-hidden {hidden
    ? 'scale-0'
    : 'scale-100 blur-none'} origin-button w-dvw h-dvh z-0"
>
  <div class="z-50 fixed bottom-0 left-0 w-full h-1/4">
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
            ? 'line-through text-white decoration-[3dvh] decoration-[#FDD3E7] cursor-none pointer-events-none'
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
