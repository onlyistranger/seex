declare module "occt-import-js" {
  export interface OcctImportOptions {
    locateFile?: (fileName: string) => string;
  }

  export interface OcctMeshAttribute {
    array: ArrayLike<number>;
  }

  export interface OcctBrepFace {
    first: number;
    last: number;
    color?: [number, number, number];
  }

  export interface OcctMesh {
    name?: string;
    attributes: {
      position: OcctMeshAttribute;
      normal?: OcctMeshAttribute;
    };
    index: OcctMeshAttribute;
    color?: [number, number, number];
    brep_faces?: OcctBrepFace[];
  }

  export interface OcctImportApi {
    ReadStepFile: (content: Uint8Array, params: Record<string, unknown> | null) => {
      meshes: OcctMesh[];
    };
  }

  const createOcctImport: (options?: OcctImportOptions) => Promise<OcctImportApi>;
  export default createOcctImport;
}

declare module "occt-import-js/dist/occt-import-js.wasm?url" {
  const url: string;
  export default url;
}
