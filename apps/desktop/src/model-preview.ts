import {
  AmbientLight,
  Box3,
  BufferAttribute,
  BufferGeometry,
  Color,
  DirectionalLight,
  Group,
  Mesh,
  MeshStandardMaterial,
  Object3D,
  PerspectiveCamera,
  Scene,
  SRGBColorSpace,
  Vector3,
  WebGLRenderer,
} from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { VRMLLoader } from "three/addons/loaders/VRMLLoader.js";
import type { OcctImportApi, OcctMesh } from "occt-import-js";

export type ModelFormat = "step" | "stp" | "wrl";

let occtPromise: Promise<OcctImportApi> | null = null;

async function loadOcct() {
  occtPromise ??= Promise.all([
    import("occt-import-js"),
    import("occt-import-js/dist/occt-import-js.wasm?url"),
  ]).then(([module, wasm]) =>
    module.default({
      locateFile: (fileName) => (fileName.endsWith(".wasm") ? wasm.default : fileName),
    }),
  );
  return occtPromise;
}

function disposeMaterial(material: Mesh["material"]) {
  if (Array.isArray(material)) {
    material.forEach((entry) => entry.dispose());
  } else {
    material.dispose();
  }
}

function disposeObject(object: Object3D) {
  object.traverse((child) => {
    if (!(child instanceof Mesh)) {
      return;
    }
    child.geometry.dispose();
    disposeMaterial(child.material);
  });
}

function buildStepMesh(meshData: OcctMesh): Mesh {
  const geometry = new BufferGeometry();
  const position = Float32Array.from(meshData.attributes.position.array);
  geometry.setAttribute("position", new BufferAttribute(position, 3));

  if (meshData.attributes.normal) {
    const normal = Float32Array.from(meshData.attributes.normal.array);
    geometry.setAttribute("normal", new BufferAttribute(normal, 3));
  } else {
    geometry.computeVertexNormals();
  }

  const index = Uint32Array.from(meshData.index.array);
  geometry.setIndex(new BufferAttribute(index, 1));
  const color = meshData.color ?? [0.72, 0.76, 0.82];
  const material = new MeshStandardMaterial({
    color: new Color(color[0], color[1], color[2]),
    metalness: 0.08,
    roughness: 0.68,
  });
  const mesh = new Mesh(geometry, material);
  mesh.name = meshData.name ?? "STEP model";
  return mesh;
}

export class ModelPreviewViewer {
  private readonly host: HTMLElement;
  private readonly renderer: WebGLRenderer;
  private readonly scene: Scene;
  private readonly camera: PerspectiveCamera;
  private readonly controls: OrbitControls;
  private readonly resizeObserver: ResizeObserver;
  private model: Object3D | null = null;
  private animationFrame = 0;
  private loadToken = 0;

  constructor(host: HTMLElement) {
    this.host = host;
    this.renderer = new WebGLRenderer({ antialias: true, alpha: false });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    this.renderer.setClearColor(0x20252d, 1);
    this.renderer.outputColorSpace = SRGBColorSpace;
    this.renderer.domElement.className = "model-preview-canvas";
    this.host.replaceChildren(this.renderer.domElement);

    this.scene = new Scene();
    this.camera = new PerspectiveCamera(42, 1, 0.01, 100000);
    this.camera.up.set(0, 0, 1);
    this.camera.position.set(120, 120, 90);

    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enableDamping = true;
    this.controls.screenSpacePanning = true;

    const ambient = new AmbientLight(0xffffff, 1.8);
    const keyLight = new DirectionalLight(0xffffff, 2.6);
    keyLight.position.set(160, 180, 220);
    const fillLight = new DirectionalLight(0x9fb7ff, 1.2);
    fillLight.position.set(-120, -80, 80);
    this.scene.add(ambient, keyLight, fillLight);

    this.resizeObserver = new ResizeObserver(() => this.resize());
    this.resizeObserver.observe(this.host);
    this.resize();
    this.animate();
  }

  async load(format: ModelFormat, bytes: Uint8Array) {
    const token = ++this.loadToken;
    const object = format === "wrl" ? this.buildWrl(bytes) : await this.buildStep(bytes);
    if (token !== this.loadToken) {
      disposeObject(object);
      return;
    }

    this.clearModel();
    this.model = object;
    this.scene.add(object);
    this.fitCamera(object);
  }

  resetView() {
    if (this.model) {
      this.fitCamera(this.model);
    }
  }

  dispose() {
    this.loadToken += 1;
    this.clearModel();
    this.resizeObserver.disconnect();
    this.controls.dispose();
    cancelAnimationFrame(this.animationFrame);
    this.renderer.dispose();
    this.renderer.domElement.remove();
  }

  private buildWrl(bytes: Uint8Array) {
    const loader = new VRMLLoader();
    return loader.parse(new TextDecoder().decode(bytes), "");
  }

  private async buildStep(bytes: Uint8Array) {
    const occt = await loadOcct();
    const result = occt.ReadStepFile(bytes, {
      linearUnit: "millimeter",
      linearDeflectionType: "bounding_box_ratio",
      linearDeflection: 0.001,
      angularDeflection: 0.5,
    });
    const group = new Group();
    result.meshes.forEach((meshData) => group.add(buildStepMesh(meshData)));
    if (group.children.length === 0) {
      throw new Error("STEP file contains no renderable geometry");
    }
    return group;
  }

  private clearModel() {
    if (!this.model) {
      return;
    }
    this.scene.remove(this.model);
    disposeObject(this.model);
    this.model = null;
  }

  private fitCamera(object: Object3D) {
    const bounds = new Box3().setFromObject(object);
    if (bounds.isEmpty()) {
      throw new Error("3D model has no visible bounds");
    }
    const center = bounds.getCenter(new Vector3());
    const size = bounds.getSize(new Vector3());
    const radius = Math.max(size.length() * 0.5, 0.1);
    this.camera.near = Math.max(radius / 1000, 0.001);
    this.camera.far = Math.max(radius * 100, 1000);
    this.camera.position.copy(center).add(new Vector3(radius * 1.7, radius * 1.7, radius * 1.25));
    this.controls.target.copy(center);
    this.controls.minDistance = Math.max(radius * 0.02, 0.001);
    this.controls.maxDistance = radius * 100;
    this.controls.update();
    this.camera.updateProjectionMatrix();
  }

  private resize() {
    const width = Math.max(this.host.clientWidth, 1);
    const height = Math.max(this.host.clientHeight, 1);
    this.camera.aspect = width / height;
    this.camera.updateProjectionMatrix();
    this.renderer.setSize(width, height, false);
  }

  private animate = () => {
    this.controls.update();
    this.renderer.render(this.scene, this.camera);
    this.animationFrame = requestAnimationFrame(this.animate);
  };
}
