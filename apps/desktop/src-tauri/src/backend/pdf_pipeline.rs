use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter};
use tokio::process::Command;
use tokio::sync::Semaphore;
use std::process::Stdio;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum CompressionPreset {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "aggressive")]
    Aggressive,
}

#[derive(Debug, Deserialize)]
pub struct InputFile {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateJobRequest {
    pub files: Vec<InputFile>,
    #[serde(rename = "outputPath")]
    pub output_path: String,
    pub preset: CompressionPreset,
}

#[derive(Debug, Serialize, Clone)]
pub struct JobEvent {
    #[serde(rename = "jobId")]
    pub job_id: String,
    pub stage: String,
    pub progress: f64,
    pub message: String,
    pub status: String,
    pub output: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InspectedFile {
    pub path: String,
    pub name: String,
    pub exists: bool,
    #[serde(rename = "sizeBytes")]
    pub size_bytes: u64,
    pub error: Option<String>,
}

pub struct PipelineState {
    active_jobs: Arc<Mutex<HashMap<String, bool>>>,
}

impl PipelineState {
    pub fn new() -> Self {
        Self {
            active_jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn cancel_job(&self, job_id: &str) {
        if let Ok(mut jobs) = self.active_jobs.lock() {
            if jobs.contains_key(job_id) {
                jobs.insert(job_id.to_string(), true);
            }
        }
    }

    fn is_cancelled(&self, job_id: &str) -> bool {
        if let Ok(jobs) = self.active_jobs.lock() {
            return *jobs.get(job_id).unwrap_or(&false);
        }
        false
    }

    fn start_job(&self, job_id: String) {
        if let Ok(mut jobs) = self.active_jobs.lock() {
            jobs.insert(job_id, false);
        }
    }

    fn finish_job(&self, job_id: &str) {
        if let Ok(mut jobs) = self.active_jobs.lock() {
            jobs.remove(job_id);
        }
    }
}

// Minimum pages per chunk — below this, chunking adds overhead without benefit.
// Tuned so we don't spawn 8 GS processes for an 8-page doc.
const MIN_PAGES_PER_CHUNK: usize = 10;

fn gs_cmd() -> String {
    let name = if cfg!(windows) { "gswin64c" } else { "gs" };
    get_tool_path(name)
}

fn qpdf_cmd() -> String {
    get_tool_path("qpdf")
}

pub fn get_tool_path(name: &str) -> String {
    // 1. Try common absolute paths first (more reliable in GUI apps)
    #[cfg(target_os = "macos")]
    {
        let paths = [
            "/opt/homebrew/bin",   // Apple Silicon Homebrew
            "/usr/local/bin",      // Intel Homebrew / Manual install
            "/usr/bin",            // System
            "/bin",
            "/opt/local/bin",      // MacPorts
        ];
        for p in paths {
            let full = format!("{}/{}", p, name);
            if std::path::Path::new(&full).exists() {
                return full;
            }
        }
    }

    // 2. Try default PATH
    let check_cmd = if cfg!(windows) { "where" } else { "which" };
    if let Ok(out) = std::process::Command::new(check_cmd).arg(name).output() {
        if out.status.success() {
            let path_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !path_str.is_empty() {
                return path_str;
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        // For GS, we might need to check multiple versions
        if name == "gswin64c" {
            let base = "C:\\Program Files\\gs";
            if let Ok(entries) = std::fs::read_dir(base) {
                for entry in entries.flatten() {
                    let bin = entry.path().join("bin").join("gswin64c.exe");
                    if bin.exists() {
                        return bin.to_string_lossy().to_string();
                    }
                }
            }
        }
        // Generic check for qpdf
        let win_paths = ["C:\\Program Files\\qpdf\\bin", "C:\\Program Files (x86)\\qpdf\\bin"];
        for p in win_paths {
            let full = format!("{}\\{}.exe", p, name);
            if std::path::Path::new(&full).exists() {
                return full;
            }
        }
    }

    name.to_string()
}

pub async fn run_job(
    app: AppHandle,
    state: Arc<PipelineState>,
    req: CreateJobRequest,
) -> String {
    let job_id = Uuid::new_v4().to_string();
    let job_id_clone = job_id.clone();
    let state_clone = state.clone();

    state.start_job(job_id.clone());

    tauri::async_runtime::spawn(async move {
        let emit = |stage: &str, progress: f64, message: &str, status: &str, output: Option<String>| {
            let _ = app.emit("job-event", JobEvent {
                job_id: job_id_clone.clone(),
                stage: stage.to_string(),
                progress,
                message: message.to_string(),
                status: status.to_string(),
                output,
            });
        };

        emit("init", 0.05, "Starting job", "running", None);

        // Guard: prevent overwriting input files
        for file in &req.files {
            if file.path == req.output_path {
                emit("error", 0.0,
                    "Output path matches an input file. Choose a different output name.",
                    "failed", None);
                return;
            }
        }

        let temp_dir = std::env::temp_dir().join(format!("pdftool-{}", job_id_clone));
        let _ = std::fs::create_dir_all(&temp_dir);
        let merged_path = temp_dir.join("merged.pdf");

        // ── Stage 1: Merge ───────────────────────────────────────────────────────
        emit("merge", 0.10, "Merging files…", "running", None);
        if state_clone.is_cancelled(&job_id_clone) { return cancel(&emit, &temp_dir); }

        if !merge_with_qpdf(&req.files, &merged_path).await {
            emit("merge", 0.12, "Fast merge failed — retrying with Ghostscript…", "running", None);
            if !merge_with_gs(&req.files, &merged_path).await {
                emit("error", 0.0, "Merge failed. The input PDF may be too corrupted.", "failed", None);
                let _ = std::fs::remove_dir_all(&temp_dir);
                return;
            }
        }
        emit("merge", 0.20, "Merge complete", "running", None);

        // ── None preset: copy straight through ──────────────────────────────────
        if matches!(req.preset, CompressionPreset::None) {
            if let Some(parent) = PathBuf::from(&req.output_path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match std::fs::copy(&merged_path, &req.output_path) {
                Ok(_) => emit("done", 1.0, "Done", "completed", Some(req.output_path.clone())),
                Err(e) => emit("error", 0.0, &format!("Save failed: {e}"), "failed", None),
            }
            state_clone.finish_job(&job_id_clone);
            let _ = std::fs::remove_dir_all(&temp_dir);
            return;
        }

        // ── Stage 2: Page count ──────────────────────────────────────────────────
        let total_pages = match get_page_count(&merged_path).await {
            Some(n) if n > 0 => n,
            _ => {
                emit("error", 0.0, "Could not read page count.", "failed", None);
                let _ = std::fs::remove_dir_all(&temp_dir);
                return;
            }
        };

        let num_workers = thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let gs_settings = preset_to_gs_settings(&req.preset);

        // ── Decide: chunk+parallel vs direct single-pass ─────────────────────────
        // Chunking only pays off when pages are plentiful relative to workers.
        // If the doc is small, compress it in one shot — no split/reassemble overhead.
        let pages_per_chunk = total_pages / num_workers;
        let use_chunks = pages_per_chunk >= MIN_PAGES_PER_CHUNK;

        if !use_chunks {
            emit("compress", 0.30,
                &format!("Document is small ({total_pages} pages) — compressing in one pass…"),
                "running", None);

            if state_clone.is_cancelled(&job_id_clone) { return cancel(&emit, &temp_dir); }

            if let Some(parent) = PathBuf::from(&req.output_path).parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            let ok = compress_chunk(
                merged_path.to_str().unwrap(),
                &req.output_path,
                &gs_settings,
                num_workers, // give GS all threads for single-pass
            ).await;

            if ok {
                emit("done", 1.0, "Done", "completed", Some(req.output_path.clone()));
            } else {
                emit("error", 0.0, "Compression failed.", "failed", None);
            }
            state_clone.finish_job(&job_id_clone);
            let _ = std::fs::remove_dir_all(&temp_dir);
            return;
        }

        // ── Stage 3: Parallel chunk split ────────────────────────────────────────
        let chunks_dir = temp_dir.join("chunks");
        let compressed_dir = temp_dir.join("comp");
        let _ = std::fs::create_dir_all(&chunks_dir);
        let _ = std::fs::create_dir_all(&compressed_dir);

        let chunk_ranges = build_chunk_ranges(total_pages, num_workers);
        let total_chunks = chunk_ranges.len();

        emit("split", 0.25,
            &format!("Splitting into {total_chunks} chunks ({pages_per_chunk} pages each)…"),
            "running", None);

        // Split ALL chunks in parallel (qpdf is fast and low-memory per call)
        let split_semaphore = Arc::new(Semaphore::new(num_workers));
        let mut split_handles = Vec::new();

        for (idx, (start, end)) in chunk_ranges.iter().enumerate() {
            let sem = Arc::clone(&split_semaphore);
            let src = merged_path.clone();
            let dst = chunks_dir.join(format!("chunk_{idx:03}.pdf"));
            let range = format!("{start}-{end}");

            split_handles.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let status = Command::new(qpdf_cmd())
                    .args(["--warning-exit-0", "--empty", "--pages"])
                    .arg(&src)
                    .arg(&range)
                    .arg("--")
                    .arg(&dst)
                    .status()
                    .await;
                matches!(status, Ok(s) if s.success() || s.code() == Some(2))
            }));
        }

        for (idx, handle) in split_handles.into_iter().enumerate() {
            match handle.await {
                Ok(true) => {}
                _ => {
                    emit("error", 0.0, &format!("Split failed for chunk {idx}."), "failed", None);
                    let _ = std::fs::remove_dir_all(&temp_dir);
                    return;
                }
            }
        }

        if state_clone.is_cancelled(&job_id_clone) { return cancel(&emit, &temp_dir); }

        // ── Stage 4: Parallel compression ───────────────────────────────────────
        emit("compress", 0.40, "Compressing chunks in parallel…", "running", None);

        let sem = Arc::new(Semaphore::new(num_workers));
        let completed = Arc::new(Mutex::new(0usize));
        let mut comp_handles = Vec::new();

        for idx in 0..total_chunks {
            let sem = Arc::clone(&sem);
            let app = app.clone();
            let job_id = job_id_clone.clone();
            let state = state_clone.clone();
            let completed = Arc::clone(&completed);
            let src = chunks_dir.join(format!("chunk_{idx:03}.pdf"));
            let dst = compressed_dir.join(format!("comp_{idx:03}.pdf"));
            let settings = gs_settings.clone();

            comp_handles.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                if state.is_cancelled(&job_id) {
                    return Err("cancelled".to_string());
                }

                // Each GS worker gets 1 thread — the parallelism comes from
                // running num_workers GS processes simultaneously, not from
                // asking each one to multithread internally (which causes
                // contention and actually slows things down).
                let ok = compress_chunk(
                    src.to_str().unwrap(),
                    dst.to_str().unwrap(),
                    &settings,
                    1,
                ).await;

                if ok {
                    let mut c = completed.lock().unwrap();
                    *c += 1;
                    let n = *c;
                    let progress = 0.40 + (n as f64 / total_chunks as f64) * 0.45;
                    let _ = app.emit("job-event", JobEvent {
                        job_id,
                        stage: "compress".to_string(),
                        progress,
                        message: format!("Compressed {n}/{total_chunks}"),
                        status: "running".to_string(),
                        output: None,
                    });
                    Ok(())
                } else {
                    Err(format!("Chunk {idx} compression failed"))
                }
            }));
        }

        for handle in comp_handles {
            if let Err(e) = handle.await.unwrap() {
                if e == "cancelled" {
                    return cancel(&emit, &temp_dir);
                }
                emit("error", 0.0, &format!("Compression error: {e}"), "failed", None);
                let _ = std::fs::remove_dir_all(&temp_dir);
                return;
            }
        }

        // ── Stage 5: Reassemble ──────────────────────────────────────────────────
        emit("finalize", 0.90, "Assembling final PDF…", "running", None);

        if let Some(parent) = PathBuf::from(&req.output_path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let mut chunk_paths: Vec<_> = std::fs::read_dir(&compressed_dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        chunk_paths.sort();

        let mut args = vec![
            "--warning-exit-0".to_string(),
            "--empty".to_string(),
            "--pages".to_string(),
        ];
        for p in &chunk_paths {
            args.push(p.to_string_lossy().to_string());
        }
        args.push("--".to_string());
        // Use linearize for fast web/reader loading
        args.push("--linearize".to_string());
        args.push(req.output_path.clone());

        let status = Command::new(qpdf_cmd())
            .args(&args)
            .status()
            .await;

        match status {
            Ok(s) if s.success() || s.code() == Some(2) => {
                emit("done", 1.0, "Done", "completed", Some(req.output_path.clone()));
            }
            _ => {
                emit("error", 0.0, "Final assembly failed.", "failed", None);
            }
        }

        state_clone.finish_job(&job_id_clone);
        let _ = std::fs::remove_dir_all(&temp_dir);
    });

    job_id
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn cancel(
    emit: &impl Fn(&str, f64, &str, &str, Option<String>),
    temp_dir: &std::path::Path,
) {
    emit("cancel", 1.0, "Job cancelled", "cancelled", None);
    let _ = std::fs::remove_dir_all(temp_dir);
}

/// Build chunk page ranges. Always produces at least MIN_PAGES_PER_CHUNK pages
/// per chunk so we never create tiny single-page chunks.
fn build_chunk_ranges(total_pages: usize, num_workers: usize) -> Vec<(usize, usize)> {
    let pages_per_chunk = (total_pages / num_workers).max(MIN_PAGES_PER_CHUNK);
    let mut ranges = Vec::new();
    let mut start = 1;
    while start <= total_pages {
        let end = (start + pages_per_chunk - 1).min(total_pages);
        ranges.push((start, end));
        start = end + 1;
    }
    ranges
}

async fn get_page_count(path: &std::path::Path) -> Option<usize> {
    let out = Command::new(qpdf_cmd())
        .arg("--show-npages")
        .arg(path)
        .output()
        .await
        .ok()?;
    if !out.status.success() { return None; }
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

async fn merge_with_qpdf(files: &[InputFile], out: &std::path::Path) -> bool {
    let mut args = vec![
        "--warning-exit-0".to_string(),
        "--empty".to_string(),
        "--pages".to_string(),
    ];
    for f in files { args.push(f.path.clone()); }
    args.push("--".to_string());
    args.push("--object-streams=generate".to_string());
    args.push("--compress-streams=y".to_string());
    args.push(out.to_string_lossy().to_string());

    let result = Command::new(qpdf_cmd())
        .args(&args)
        .stderr(Stdio::piped())
        .output()
        .await;

    matches!(result, Ok(o) if (o.status.success() || o.status.code() == Some(2)) && out.exists())
}

async fn merge_with_gs(files: &[InputFile], out: &std::path::Path) -> bool {
    let mut args = vec![
        "-sDEVICE=pdfwrite".to_string(),
        "-dNOPAUSE".to_string(),
        "-dBATCH".to_string(),
        "-dPDFSTOPONERROR=false".to_string(),
        format!("-sOutputFile={}", out.to_string_lossy()),
    ];
    for f in files { args.push(f.path.clone()); }

    let status = Command::new(gs_cmd()).args(&args).status().await;
    matches!(status, Ok(s) if s.success()) && out.exists()
}

#[derive(Clone)]
struct GsSettings {
    pdf_settings: &'static str,
    color_res: &'static str,
    gray_res: &'static str,
    mono_res: &'static str,
    downsample_type: &'static str,
}

fn preset_to_gs_settings(preset: &CompressionPreset) -> GsSettings {
    // /Average produces better quality than /Subsample at the same resolution
    // and is not meaningfully slower. /Bicubic would be best but is slow for
    // batch work. /Average is the right default for everything except Aggressive.
    match preset {
        CompressionPreset::None     => GsSettings { pdf_settings: "/printer",  color_res: "300", gray_res: "300", mono_res: "300", downsample_type: "/Average" },
        CompressionPreset::Low      => GsSettings { pdf_settings: "/prepress", color_res: "300", gray_res: "300", mono_res: "300", downsample_type: "/Average" },
        CompressionPreset::Medium   => GsSettings { pdf_settings: "/printer",  color_res: "150", gray_res: "150", mono_res: "300", downsample_type: "/Average" },
        CompressionPreset::High     => GsSettings { pdf_settings: "/ebook",    color_res: "150", gray_res: "150", mono_res: "200", downsample_type: "/Average" },
        CompressionPreset::Aggressive => GsSettings { pdf_settings: "/screen", color_res: "72",  gray_res: "72",  mono_res: "150", downsample_type: "/Subsample" },
    }
}

/// Run Ghostscript on a single file. `gs_threads` controls -dNumRenderingThreads.
/// Pass 1 for parallel chunk workers, and num_workers for single-pass jobs.
async fn compress_chunk(
    input: &str,
    output: &str,
    s: &GsSettings,
    gs_threads: usize,
) -> bool {
    let status = Command::new(gs_cmd())
        .arg("-sDEVICE=pdfwrite")
        .arg("-dCompatibilityLevel=1.5")   // 1.5 enables better object streams than 1.4
        .arg("-dNOPAUSE")
        .arg("-dBATCH")
        .arg("-dQUIET")
        .arg("-dPDFSTOPONERROR=false")
        .arg(format!("-dNumRenderingThreads={gs_threads}"))
        .arg(format!("-dPDFSETTINGS={}", s.pdf_settings))
        .arg(format!("-dColorImageResolution={}", s.color_res))
        .arg(format!("-dGrayImageResolution={}", s.gray_res))
        .arg(format!("-dMonoImageResolution={}", s.mono_res))
        .arg("-dDownsampleColorImages=true")
        .arg("-dDownsampleGrayImages=true")
        .arg("-dDownsampleMonoImages=true")
        .arg(format!("-dColorImageDownsampleType={}", s.downsample_type))
        .arg(format!("-dGrayImageDownsampleType={}", s.downsample_type))
        .arg(format!("-dMonoImageDownsampleType={}", s.downsample_type))
        .arg(format!("-sOutputFile={output}"))
        .arg(input)
        .status()
        .await;

    matches!(status, Ok(s) if s.success())
}

pub fn inspect_files(paths: Vec<String>) -> Vec<InspectedFile> {
    paths.into_iter().map(|p| {
        let path = PathBuf::from(&p);
        let name = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| p.clone());
        let (exists, size_bytes, error) = match std::fs::metadata(&path) {
            Ok(meta) => (true, meta.len(), None),
            Err(e)   => (false, 0, Some(e.to_string())),
        };
        InspectedFile { path: p, name, exists, size_bytes, error }
    }).collect()
}