export type CompressionPreset = "low" | "medium" | "high" | "aggressive";

export interface InputFile {
  path: string;
  name: string;
  sizeBytes: number;
}

export interface CreateJobRequest {
  files: InputFile[];
  preset: CompressionPreset;
  outputPath: string;
  maxWorkers: number;
}

export interface CreateJobResponse {
  jobId: string;
}

export interface InspectedFile {
  path: string;
  name: string;
  sizeBytes: number;
  exists: boolean;
  error?: string;
}

export interface InspectFilesResponse {
  files: InspectedFile[];
}

export interface ToolHealth {
  path: string;
  ok: boolean;
  version?: string;
  error?: string;
}

export interface BackendHealth {
  status: "ok" | "degraded";
  qpdf: ToolHealth;
  ghostscript: ToolHealth;
}

export interface JobEvent {
  jobId: string;
  stage: string;
  progress: number;
  message: string;
  skipped?: Array<{ path: string; reason: string }>;
  outputPath?: string;
  status: "queued" | "running" | "failed" | "cancelled" | "completed";
}
