import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { BackendHealth, CreateJobRequest, CreateJobResponse, InspectFilesResponse, JobEvent } from "./types";

export async function createJob(req: CreateJobRequest): Promise<CreateJobResponse> {
  const jobId = (await invoke("create_job", { req })) as string;
  return { jobId };
}

export async function inspectFiles(paths: string[]): Promise<InspectFilesResponse> {
  const files = (await invoke("inspect_files", { paths })) as any[];
  return { files };
}

export async function getHealth(): Promise<BackendHealth> {
  return (await invoke("get_health")) as BackendHealth;
}

export async function cancelJob(jobId: string): Promise<void> {
  await invoke("cancel_job", { jobId });
}

export function subscribeEvents(onEvent: (event: JobEvent) => void): () => void {
  let unlisten: (() => void) | undefined;
  
  listen("job-event", (event: any) => {
    onEvent(event.payload as JobEvent);
  }).then(fn => {
    unlisten = fn as any;
  });

  return () => {
    if (unlisten) unlisten();
  };
}
