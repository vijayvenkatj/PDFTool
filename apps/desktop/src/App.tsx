import { DragEvent, useEffect, useMemo, useState } from "react";
import { cancelJob, createJob, getHealth, inspectFiles, startBackend, subscribeEvents } from "./lib/api";
import { BackendHealth, CompressionPreset, InputFile, JobEvent } from "./lib/types";
import { open, save } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";

const DEFAULT_MAX_FILES = 100;

function readDroppedFiles(event: DragEvent<HTMLDivElement>): InputFile[] {
  const files = Array.from(event.dataTransfer.files);
  return files
    .filter((f) => f.name.toLowerCase().endsWith(".pdf"))
    .map((f) => ({
      path: (f as File & { path?: string }).path ?? "",
      name: f.name,
      sizeBytes: f.size
    }))
    .filter((f) => f.path.length > 0);
}

function toInputFile(path: string): InputFile {
  const parts = path.split(/[\\/]/);
  return {
    path,
    name: parts[parts.length - 1] ?? path,
    sizeBytes: 0
  };
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let idx = 0;
  while (value >= 1024 && idx < units.length - 1) {
    value /= 1024;
    idx++;
  }
  const precision = idx === 0 ? 0 : 2;
  return `${value.toFixed(precision)} ${units[idx]}`;
}

export function App() {
  const [files, setFiles] = useState<InputFile[]>([]);
  const [preset, setPreset] = useState<CompressionPreset>("medium");
  const [outputPath, setOutputPath] = useState("");
  const [maxFiles, setMaxFiles] = useState(DEFAULT_MAX_FILES);
  const [maxWorkers, setMaxWorkers] = useState(4);
  const [activeJobId, setActiveJobId] = useState<string | null>(null);
  const [lastEvent, setLastEvent] = useState<JobEvent | null>(null);
  const [logs, setLogs] = useState<string[]>([]);
  const [draggingIndex, setDraggingIndex] = useState<number | null>(null);
  const [health, setHealth] = useState<BackendHealth | null>(null);
  const [isInspecting, setIsInspecting] = useState(false);
  const [startupError, setStartupError] = useState<string | null>(null);
  const [runtimeState, setRuntimeState] = useState<"checking" | "ok" | "degraded">("checking");
  const canStart = files.length > 0 && outputPath.length > 0 && !activeJobId && health?.status === "ok" && !isInspecting;

  const addFiles = async (incoming: InputFile[]) => {
    const candidatePaths = incoming
      .map((f) => f.path)
      .filter((p) => p.toLowerCase().endsWith(".pdf"));
    if (candidatePaths.length === 0) return;

    setIsInspecting(true);
    try {
      const inspected = await inspectFiles(candidatePaths);
      const good = inspected.files
        .filter((f) => f.exists && f.sizeBytes >= 0)
        .map((f) => ({
          path: f.path,
          name: f.name,
          sizeBytes: f.sizeBytes
        }));
      const bad = inspected.files.filter((f) => !f.exists || !!f.error);
      if (bad.length > 0) {
        setLogs((x) => [`skipped ${bad.length} invalid file(s)`, ...x].slice(0, 200));
      }
      setFiles((current) => {
        const seen = new Set(current.map((f) => f.path));
        const unique = good.filter((f) => !seen.has(f.path));
        return [...current, ...unique].slice(0, maxFiles);
      });
    } catch (err) {
      setLogs((x) => [`file inspect failed: ${String(err)}`, ...x].slice(0, 200));
    } finally {
      setIsInspecting(false);
    }
  };

  useEffect(() => {
    startBackend()
      .then(async () => {
        setStartupError(null);
        setRuntimeState("checking");
        let got = false;
        for (let i = 0; i < 8; i++) {
          try {
            const h = await getHealth();
            setHealth(h);
            setRuntimeState(h.status === "ok" ? "ok" : "degraded");
            if (h.status !== "ok") {
              setLogs((x) => ["runtime health degraded: qpdf/ghostscript not ready", ...x].slice(0, 200));
            }
            got = true;
            break;
          } catch {
            await new Promise((r) => setTimeout(r, 250));
          }
        }
        if (!got) {
          setRuntimeState("degraded");
          setLogs((x) => ["health check error: backend not ready", ...x].slice(0, 200));
        }
      })
      .catch((err) => {
        const msg = `backend start error: ${String(err)}`;
        setStartupError(msg);
        setLogs((x) => [msg, ...x]);
      });
    const unsub = subscribeEvents((evt) => {
      setLastEvent(evt);
      setLogs((x) => [`${evt.stage}: ${evt.message}`, ...x].slice(0, 200));
      if (evt.status === "completed" || evt.status === "failed" || evt.status === "cancelled") {
        setActiveJobId((id) => (id === evt.jobId ? null : id));
      }
    });
    return () => {
      unsub();
    };
  }, []);

  useEffect(() => {
    const timer = setInterval(async () => {
      try {
        const h = await getHealth();
        setHealth(h);
        setRuntimeState(h.status === "ok" ? "ok" : "degraded");
      } catch {
        if (health) {
          setRuntimeState("degraded");
        }
      }
    }, 3000);
    return () => clearInterval(timer);
  }, [health]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    getCurrentWindow()
      .onDragDropEvent((event) => {
        if (event.payload.type === "drop") {
          const dropped = event.payload.paths.map((p) => toInputFile(p));
          void addFiles(dropped);
        }
      })
      .then((fn) => {
        unlisten = fn;
      })
      .catch((err) => {
        setLogs((x) => [`drag-drop listener error: ${String(err)}`, ...x]);
      });
    return () => {
      if (unlisten) unlisten();
    };
  }, [maxFiles]);

  const onDrop = (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    const dropped = readDroppedFiles(event);
    void addFiles(dropped);
  };

  const selectFiles = async () => {
    const selected = await open({
      multiple: true,
      filters: [{ name: "PDF", extensions: ["pdf"] }]
    });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    await addFiles(paths.map((p) => toInputFile(String(p))));
  };

  const selectOutput = async () => {
    const selected = await save({
      filters: [{ name: "PDF", extensions: ["pdf"] }],
      defaultPath: "output.pdf"
    });
    if (selected) setOutputPath(String(selected));
  };

  const totalBytes = useMemo(() => files.reduce((sum, f) => sum + f.sizeBytes, 0), [files]);

  return (
    <div className="app">
      <header className="appHeader">
        <div>
          <h1>PDF Tool</h1>
          <p className="subtitle">Compress and merge PDFs offline.</p>
        </div>
        <div className="headerStatus">
          <div className="statusRow">
            <span className="label">Runtime</span>
            <span className={`status ${runtimeState}`}>{runtimeState}</span>
          </div>
          <div className="toolRow">
            qpdf: {health ? (health.qpdf.ok ? "ok" : "missing") : "-"} | ghostscript: {health ? (health.ghostscript.ok ? "ok" : "missing") : "-"}
          </div>
        </div>
      </header>

      <main className="grid">
        <section className="panel">
          <div className="panelHeader">
            <h2>Input</h2>
            <button className="btnGhost" onClick={selectFiles}>Choose PDF files</button>
          </div>
          <div
            className="dropzone"
            onDragOver={(e) => e.preventDefault()}
            onDrop={onDrop}
          >
            <strong>Drop PDFs here</strong>
            <span>or use the button above</span>
          </div>
          <div className="row">
            <label>Max files</label>
            <input type="number" min={1} max={500} value={maxFiles} onChange={(e) => setMaxFiles(Number(e.target.value || DEFAULT_MAX_FILES))} />
            <label>Workers</label>
            <input type="number" min={1} max={8} value={maxWorkers} onChange={(e) => setMaxWorkers(Number(e.target.value || 4))} />
          </div>
          <div className="row">
            <label>Preset</label>
            <select value={preset} onChange={(e) => setPreset(e.target.value as CompressionPreset)}>
              <option value="low">Low</option>
              <option value="medium">Medium</option>
              <option value="high">High</option>
              <option value="aggressive">Aggressive</option>
            </select>
          </div>
          <div className="row">
            <label>Output file path</label>
            <input value={outputPath} onChange={(e) => setOutputPath(e.target.value)} placeholder="/path/to/output.pdf" />
            <button className="btnGhost" onClick={selectOutput}>Browse</button>
          </div>
        </section>

        <section className="panel">
          <div className="panelHeader">
            <div>
              <h2>Files</h2>
              <p className="muted">{files.length} files · {formatBytes(totalBytes)}</p>
            </div>
            <button className="btnGhost" onClick={() => setFiles([])} disabled={files.length === 0 || !!activeJobId}>Clear</button>
          </div>
          <p className="muted">Drag to reorder</p>
          {files.length === 0 ? (
            <div className="emptyState">No files added yet.</div>
          ) : (
            <ol className="fileList">
              {files.map((f, i) => (
                <li
                  key={`${f.path}-${i}`}
                  draggable
                  onDragStart={() => setDraggingIndex(i)}
                  onDragOver={(e) => e.preventDefault()}
                  onDragEnd={() => setDraggingIndex(null)}
                  onDrop={() => {
                    if (draggingIndex === null || draggingIndex === i) return;
                    setFiles((arr) => {
                      const next = [...arr];
                      const [item] = next.splice(draggingIndex, 1);
                      next.splice(i, 0, item);
                      return next;
                    });
                    setDraggingIndex(null);
                  }}
                >
                  <div className="fileMeta">
                    <span className="fileIndex">{i + 1}.</span>
                    <span className="fileName">{f.name}</span>
                    <span className="fileSize">{formatBytes(f.sizeBytes)}</span>
                  </div>
                  <button className="btnGhost" onClick={() => setFiles((arr) => arr.filter((_, idx) => idx !== i))}>Remove</button>
                </li>
              ))}
            </ol>
          )}
        </section>

        <section className="panel">
          <div className="panelHeader">
            <h2>Run</h2>
          </div>
          <div className="row">
            <button
              className="btnPrimary"
              disabled={!canStart}
              onClick={async () => {
                const res = await createJob({ files, preset, outputPath, maxWorkers });
                setActiveJobId(res.jobId);
              }}
            >
              Start
            </button>
            <button
              className="btnGhost"
              disabled={!activeJobId}
              onClick={async () => {
                if (activeJobId) {
                  await cancelJob(activeJobId);
                }
              }}
            >
              Cancel
            </button>
          </div>
          {startupError ? <p className="warning">{startupError}</p> : null}
          {isInspecting ? <p className="muted">Inspecting selected files...</p> : null}
          <div className="statusGrid">
            <div><span className="label">Status</span> {lastEvent?.status ?? "idle"}</div>
            <div><span className="label">Stage</span> {lastEvent?.stage ?? "-"}</div>
          </div>
          <div className="progressRow">
            <progress value={lastEvent?.progress ?? 0} max={1} />
            <span>{Math.round((lastEvent?.progress ?? 0) * 100)}%</span>
          </div>
          <div className="logBox">
            {logs.map((line, idx) => <div key={idx}>{line}</div>)}
          </div>
        </section>
      </main>
      <div className="watermark">github.com/vijayvenkatj</div>
    </div>
  );
}
