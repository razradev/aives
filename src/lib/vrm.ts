import * as THREE from "three/webgpu";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
import {
  MToonMaterialLoaderPlugin,
  VRMLoaderPlugin,
  VRMUtils,
} from "@pixiv/three-vrm";
import { MToonNodeMaterial } from "@pixiv/three-vrm/nodes";

import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";

export function init() {
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

  const renderer = new THREE.WebGPURenderer({
    alpha: true,
    antialias: false,
  });
  renderer.setSize(window.innerWidth, window.innerHeight);
  renderer.setPixelRatio(window.devicePixelRatio);
  const canvas = document
    .getElementById("renderer")
    ?.appendChild(renderer.domElement);
  canvas.style.filter = "saturate(1.6)";

  let currentVrm = undefined;
  const loader = new GLTFLoader();
  loader.crossOrigin = "anonymous";

  let cursorIgnored = false;

  loader.register((parser) => {
    const mtoonMaterialPlugin = new MToonMaterialLoaderPlugin(parser, {
      materialType: MToonNodeMaterial,
    });

    return new VRMLoaderPlugin(parser, {
      mtoonMaterialPlugin,
    });
  });

  loader.load(
    "/Hoshino.vrm",

    (gltf) => {
      const vrm = gltf.userData.vrm;

      VRMUtils.removeUnnecessaryVertices(gltf.scene);
      VRMUtils.combineSkeletons(gltf.scene);
      VRMUtils.combineMorphs(vrm);

      vrm.scene.traverse((obj) => {
        obj.frustumCulled = false;
      });

      currentVrm = vrm;

      scene.add(vrm.scene);

      console.log(vrm);
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

    if (currentVrm) {
      currentVrm.update(clock.getDelta());
    }

    renderer.renderAsync(scene, camera);
  }

  animate();

  setInterval(async () => {
    invoke("check_cursor_region");
  }, 100);
}
