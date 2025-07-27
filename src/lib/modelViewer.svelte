<script lang="ts">
  import * as THREE from "three";
  import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
  import { VRMLoaderPlugin, VRMUtils } from "@pixiv/three-vrm";
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  let { hidden = $bindable(), responding } = $props();

  let currentVrm: any = undefined;
  let currentMixer: THREE.AnimationMixer | undefined = undefined;
  let overCanvas: boolean = $state(false);

  onMount(() => {
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

    const lookAtTarget = new THREE.Object3D();
    camera.add(lookAtTarget);

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
    if (canvas) {
      canvas.style.filter = "saturate(1.6)";
    }

    const loader = new GLTFLoader();
    loader.crossOrigin = "anonymous";

    loader.register((parser) => {
      return new VRMLoaderPlugin(parser);
    });

    loader.load(
      "/Hoshino.vrm",
      (gltf) => {
        const vrm = gltf.userData.vrm;
        VRMUtils.removeUnnecessaryVertices(gltf.scene);
        VRMUtils.combineSkeletons(gltf.scene);
        VRMUtils.combineMorphs(vrm);
        scene.add(vrm.scene);
        currentVrm = vrm;
        vrm.lookAt.target = lookAtTarget;
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

    // Function for animation (optional, can be moved or removed based on needs)
    function prepareAnimation(vrm: any) {
      currentMixer = new THREE.AnimationMixer(vrm.scene);

      const quatA = new THREE.Quaternion(0.0, 0.0, 0.0, 1.0);
      const quatB = new THREE.Quaternion(0.0, 0.0, 0.0, 1.0);
      quatB.setFromEuler(new THREE.Euler(0.0, 0.0, 0.25 * Math.PI));

      const armTrack = new THREE.QuaternionKeyframeTrack(
        vrm.humanoid.getNormalizedBoneNode("leftUpperArm").name + ".quaternion",
        [0.0, 0.5, 1.0],
        [...quatA.toArray(), ...quatB.toArray(), ...quatA.toArray()]
      );

      const blinkTrack = new THREE.NumberKeyframeTrack(
        vrm.expressionManager.getExpressionTrackName("blink"),
        [0.0, 0.5, 1.0],
        [0.0, 1.0, 0.0]
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

      if (!hidden) {
        invoke("check_cursor_region", { hidden: hidden }).then((data) => {
          if (data != null && data.length == 3) {
            lookAtTarget.position.x = 10.0 * data[0];
            lookAtTarget.position.y = -10.0 * data[1];
            overCanvas = data[2];
          }
        });
      }

      if (currentVrm) {
        // You might want to control expressions based on 'responding' prop here
        if (responding) {
          currentVrm.expressionManager.setValue("ih", 1); // Example expression
        } else {
          currentVrm.expressionManager.setValue("ih", 0); // Reset or default
        }
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
  });
</script>

<div
  id="renderer"
  class="overflow-hidden {hidden ? 'scale-0' : 'scale-100'} {!hidden &&
  overCanvas
    ? 'opacity-10'
    : 'opacity-100'} origin-button w-dvw h-dvh z-0"
></div>

<style>
</style>
