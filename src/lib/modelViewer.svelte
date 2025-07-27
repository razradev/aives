<script lang="ts">
  import * as THREE from "three";
  import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
  import { VRMLoaderPlugin, VRMUtils } from "@pixiv/three-vrm";
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import {
    createVRMAnimationClip,
    VRMAnimationLoaderPlugin,
    VRMLookAtQuaternionProxy,
  } from "@pixiv/three-vrm-animation";

  let { hidden = $bindable(), responding, animationIndex = 0 } = $props();

  let currentVrm: any = undefined;
  let currentVrmAnimation: any;
  let currentVrmAnimationIndex: number = animationIndex;
  let currentMixer: any;
  let animations: any;
  let overCanvas: boolean = $state(false);

  onMount(() => {
    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(10, 1.54, 0.1, 1000);

    camera.position.z = 5.0;
    camera.position.y = 1.3;

    const light = new THREE.AmbientLight(0xffffff, 1);
    scene.add(light);
    const directionalLight = new THREE.DirectionalLight(0xffffff, 1.5);
    directionalLight.position.set(1, 3, 5);
    directionalLight.target.position.set(0, 1, 0);
    scene.add(directionalLight);
    scene.add(directionalLight.target);

    const lookAtTarget = new THREE.Object3D();
    camera.add(lookAtTarget);

    const renderer = new THREE.WebGLRenderer({
      alpha: true,
      antialias: true,
    });
    renderer.setSize(window.innerWidth, window.innerHeight * 0.65);
    renderer.setPixelRatio(window.devicePixelRatio);
    renderer.autoClear = false;
    const canvas = document
      .getElementById("renderer")
      ?.appendChild(renderer.domElement);
    if (canvas) {
      canvas.style.filter = "saturate(1.6)";
    }

    const loader = new GLTFLoader();
    loader.crossOrigin = "anonymous";

    loader.register((parser) => {
      return new VRMLoaderPlugin(parser);
    });

    loader.register((parser) => {
      return new VRMAnimationLoaderPlugin(parser);
    });

    function tryInitVRMA(gltf) {
      const vrmAnimations = gltf.animations;

      if (vrmAnimations == null) {
        return;
      }

      currentVrmAnimation = vrmAnimations[0] ?? null;

      console.log(vrmAnimations);
      animations = vrmAnimations;
    }

    function initAnimationClip() {
      animationIndex %= animations.length;
      console.log(animationIndex);
      currentVrmAnimationIndex = animationIndex;
      currentVrmAnimation = animations[currentVrmAnimationIndex];
      if (currentVrm && currentVrmAnimation) {
        currentMixer = new THREE.AnimationMixer(currentVrm.scene);

        const clip = currentVrmAnimation;
        currentMixer.clipAction(clip).play();
        //currentMixer.timeScale = params.timeScale;

        currentVrm.humanoid.resetNormalizedPose();
        currentVrm.lookAt.reset();
        currentVrm.lookAt.autoUpdate = currentVrmAnimation.lookAtTrack != null;
      }
    }

    loader.load(
      "/Hoshino0.3.vrm",
      (gltf) => {
        const vrm = gltf.userData.vrm;
        VRMUtils.removeUnnecessaryVertices(gltf.scene);
        VRMUtils.combineSkeletons(gltf.scene);
        VRMUtils.combineMorphs(vrm);
        scene.add(vrm.scene);
        currentVrm = vrm;
        vrm.lookAt.target = lookAtTarget;

        tryInitVRMA(gltf);
        // prepareAnimation(vrm); // Uncomment if you want to use the animation
      },
      (progress) =>
        console.log(
          "Loading model...",
          100.0 * (progress.loaded / progress.total),
          "%"
        ),
      (error) => console.error(error)
    );

    const clock = new THREE.Clock();
    clock.start();

    function animate() {
      requestAnimationFrame(animate);

      const deltaTime = clock.getDelta();

      invoke("check_cursor_region", { hidden: hidden }).then((data) => {
        if (data != null && data.length == 3) {
          if (!hidden) {
            lookAtTarget.position.x = 10.0 * data[0];
            lookAtTarget.position.y = -10.0 * data[1];
          }
          overCanvas = data[2];
        }
      });

      if (currentVrm) {
        // You might want to control expressions based on 'responding' prop here
        /*if (responding) {
          currentVrm.expressionManager.setValue("ih", 1); // Example expression
        } else {
          currentVrm.expressionManager.setValue("ih", 0); // Reset or default
        }*/
        currentVrm.update(deltaTime);
      }
      if (currentMixer) {
        currentMixer.update(deltaTime);
      }

      if (animationIndex != currentVrmAnimationIndex) {
        initAnimationClip();
      }

      if (!hidden) {
        renderer.clear();
      }
      scene.visible = !hidden;

      renderer.render(scene, camera);
    }

    animate();
  });
</script>

<div
  id="renderer"
  class="overflow-hidden origin-left {hidden ? 'w-0' : 'w-dvw'} {!hidden &&
  overCanvas
    ? 'opacity-10'
    : 'opacity-100'} origin-button h-[65dvh] z-0"
></div>

<style>
</style>
