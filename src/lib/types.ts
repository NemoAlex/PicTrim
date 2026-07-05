export type OutputFormat = "jpg" | "png" | "webp" | "keep";
export type ResizeMode = "fitLongestSide" | "fitBox" | "fitWidth" | "fitHeight" | "fixedCrop";
export type Rotation = "auto" | "rotate0" | "rotate90" | "rotate180" | "rotate270";
export type SourceKind = "file" | "directory" | "missing";
export type CropHorizontal = "left" | "center" | "right";
export type CropVertical = "top" | "center" | "bottom";

export interface SourceEntry {
  path: string;
  kind: SourceKind;
}

export interface BatchSettings {
  inputSources: string[];
  outputDir: string;
  resizeMode: ResizeMode;
  maxSide: number;
  width: number;
  height: number;
  allowUpscale: boolean;
  cropHorizontal: CropHorizontal;
  cropVertical: CropVertical;
  rotation: Rotation;
  thumbnail: boolean;
  quality: number;
  concurrency: number;
  outputFormat: OutputFormat;
  copyNonImages: boolean;
  skipExisting: boolean;
}

export interface BatchProgress {
  phase: string;
  discovered: number;
  processed: number;
  images: number;
  copied: number;
  skipped: number;
  failed: number;
  totalSrcBytes: number;
  totalDstBytes: number;
  current?: string | null;
  message?: string | null;
  done: boolean;
  cancelled: boolean;
}

export interface FailureEntry {
  rel: string;
  message: string;
}

export interface PreviewTree {
  items: PreviewItem[];
}

export interface PreviewItem {
  path: string;
  rel: string;
  name: string;
  segments: string[];
}

export interface PreviewPair {
  rel: string;
  before: PreviewImage;
  after: PreviewImage;
}

export interface PreviewImage {
  data: string;
  mime: string;
  width: number;
  height: number;
  bytes: number;
}
