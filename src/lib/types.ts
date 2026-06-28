export type OutputFormat = "jpg" | "png" | "webp" | "keep";

export interface BatchSettings {
  inputDir: string;
  outputDir: string;
  maxSide: number;
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
