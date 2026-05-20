import { invoke } from "@tauri-apps/api/core";
import { BackendHealth, CreateJobRequest, CreateJobResponse, InspectFilesResponse, JobEvent } from "./types";

const BACKEND_URL = "http://127.0.0.1:47832";

export async function startBackend(): Promise<void> {
  await invoke("start_backend");
}

export async function stopBackend(): Promise<void> {
  await invoke("stop_backend");
}

export async function createJob(req: CreateJobRequest): Promise<CreateJobResponse> {
  const response = await fetch(`${BACKEND_URL}/v1/jobs`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(req)
  });
  if (!response.ok) {
    const text = await response.text();
    throw new Error(`create job failed: ${response.status} ${text}`);
  }
  return response.json() as Promise<CreateJobResponse>;
}

export async function inspectFiles(paths: string[]): Promise<InspectFilesResponse> {
  const response = await fetch(`${BACKEND_URL}/v1/files/inspect`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ paths })
  });
  if (!response.ok) {
    const text = await response.text();
    throw new Error(`inspect files failed: ${response.status} ${text}`);
  }
  return response.json() as Promise<InspectFilesResponse>;
}

export async function getHealth(): Promise<BackendHealth> {
  const response = await fetch(`${BACKEND_URL}/v1/health`);
  if (!response.ok) {
    const text = await response.text();
    throw new Error(`health check failed: ${response.status} ${text}`);
  }
  return response.json() as Promise<BackendHealth>;
}

export async function cancelJob(jobId: string): Promise<void> {
  const response = await fetch(`${BACKEND_URL}/v1/jobs/${jobId}/cancel`, {
    method: "POST"
  });
  if (!response.ok) {
    throw new Error(`cancel failed: ${response.status}`);
  }
}

export function subscribeEvents(onEvent: (event: JobEvent) => void): () => void {
  const source = new EventSource(`${BACKEND_URL}/v1/events`);
  source.onmessage = (msg) => {
    const parsed = JSON.parse(msg.data) as JobEvent;
    onEvent(parsed);
  };
  return () => source.close();
}
