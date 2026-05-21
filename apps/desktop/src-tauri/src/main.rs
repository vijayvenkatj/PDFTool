#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod backend;

use backend::pdf_pipeline::{self, PipelineState, CreateJobRequest, InspectedFile};
use std::sync::Arc;
use tauri::State;

struct AppState {
    pipeline: Arc<PipelineState>,
}

#[tauri::command]
async fn create_job(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    req: CreateJobRequest,
) -> Result<String, String> {
    Ok(pdf_pipeline::run_job(app, state.pipeline.clone(), req).await)
}

#[tauri::command]
fn cancel_job(state: State<'_, AppState>, job_id: String) -> Result< (), String> {
    state.pipeline.cancel_job(&job_id);
    Ok(())
}

#[tauri::command]
fn inspect_files(paths: Vec<String>) -> Result<Vec<InspectedFile>, String> {
    Ok(pdf_pipeline::inspect_files(paths))
}

#[tauri::command]
async fn get_thumbnail(path: String) -> Result<String, String> {
    pdf_pipeline::get_thumbnail(path).await
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolStatus {
    ok: bool,
    path: String,
    version: String,
    error: Option<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthStatus {
    status: String,
    qpdf: ToolStatus,
    ghostscript: ToolStatus,
}

#[tauri::command]
async fn get_health() -> Result<HealthStatus, String> {
    let qpdf_bin = pdf_pipeline::get_tool_path("qpdf");
    let mut qpdf_cmd = std::process::Command::new(&qpdf_bin);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        qpdf_cmd.creation_flags(0x08000000);
    }
    let (qpdf_ok, qpdf_err) = match qpdf_cmd.arg("--version").output() {
        Ok(out) => (out.status.success(), if out.status.success() { None } else { Some(String::from_utf8_lossy(&out.stderr).to_string()) }),
        Err(e) => (false, Some(e.to_string())),
    };
        
    let gs_name = if cfg!(windows) { "gswin64c" } else { "gs" };
    let gs_bin = pdf_pipeline::get_tool_path(gs_name);
    let mut gs_cmd = std::process::Command::new(&gs_bin);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        gs_cmd.creation_flags(0x08000000);
    }
    let (gs_ok, gs_err) = match gs_cmd.arg("--version").output() {
        Ok(out) => (out.status.success(), if out.status.success() { None } else { Some(String::from_utf8_lossy(&out.stderr).to_string()) }),
        Err(e) => (false, Some(e.to_string())),
    };
    
    Ok(HealthStatus {
        status: if qpdf_ok && gs_ok { "ok".to_string() } else { "degraded".to_string() },
        qpdf: ToolStatus {
            ok: qpdf_ok,
            path: qpdf_bin,
            version: "system".to_string(),
            error: qpdf_err,
        },
        ghostscript: ToolStatus {
            ok: gs_ok,
            path: gs_bin,
            version: "system".to_string(),
            error: gs_err,
        },
    })
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            pipeline: Arc::new(PipelineState::new()),
        })
        .invoke_handler(tauri::generate_handler![
            create_job,
            cancel_job,
            inspect_files,
            get_thumbnail,
            get_health
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
