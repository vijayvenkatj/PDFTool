import { DragEvent, useEffect, useMemo, useState } from "react";
import { cancelJob, createJob, getHealth, getThumbnail, inspectFiles, subscribeEvents } from "./lib/api";
import { BackendHealth, CompressionPreset, InputFile, JobEvent } from "./lib/types";
import { open, save } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";

const DEFAULT_MAX_FILES = 500;
const MAX_FILES_LIMIT = 1000;
const MAX_WORKERS_LIMIT = 16;

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

function toInputFile(path: string): InputFile {
  const parts = path.split(/[\\/]/);
  return {
    path,
    name: parts[parts.length - 1] ?? path,
    sizeBytes: 0
  };
}

export function App() {
  const [files, setFiles] = useState<InputFile[]>([]);
  const [preset, setPreset] = useState<CompressionPreset>("none");
  const [outputPath, setOutputPath] = useState("");
  const [maxFiles, setMaxFiles] = useState(DEFAULT_MAX_FILES);
  const [activeJobId, setActiveJobId] = useState<string | null>(null);
  const [lastEvent, setLastEvent] = useState<JobEvent | null>(null);
  const [logs, setLogs] = useState<string[]>([]);
  const [draggingIndex, setDraggingIndex] = useState<number | null>(null);
  const [health, setHealth] = useState<BackendHealth | null>(null);
  const [isInspecting, setIsInspecting] = useState(false);
  const [isDropActive, setIsDropActive] = useState(false);
  const [showHelp, setShowHelp] = useState(false);

  const canStart = files.length > 0 && outputPath.length > 0 && !activeJobId && health?.status === "ok" && !isInspecting;

  const addFiles = async (incoming: InputFile[]) => {
    const candidatePaths = incoming
      .map((f) => f.path)
      .filter((p) => p.toLowerCase().endsWith(".pdf"));
    if (candidatePaths.length === 0) return;

    setIsInspecting(true);
    try {
      const inspected = await inspectFiles(candidatePaths);
      const good = inspected.files.filter((f) => f.exists && f.sizeBytes >= 0);
      
      const filesWithThumbs = await Promise.all(
        good.map(async (f) => {
          try {
            const thumb = await getThumbnail(f.path);
            return {
              path: f.path,
              name: f.name,
              sizeBytes: f.sizeBytes,
              pageCount: f.pageCount,
              thumbnail: thumb
            };
          } catch (e) {
            return {
              path: f.path,
              name: f.name,
              sizeBytes: f.sizeBytes,
              pageCount: f.pageCount
            };
          }
        })
      );

      setFiles((current) => {
        const seen = new Set(current.map((f) => f.path));
        const unique = filesWithThumbs.filter((f) => !seen.has(f.path));
        return [...current, ...unique].slice(0, Math.min(maxFiles, MAX_FILES_LIMIT));
      });
    } catch (err) {
      setLogs((x) => [`Error: ${String(err)}`, ...x]);
    } finally {
      setIsInspecting(false);
    }
  };

  useEffect(() => {
    const checkHealth = async () => {
      try {
        const h = await getHealth();
        setHealth(h);
      } catch (err) {
        setLogs((x) => [`Health check failed: ${String(err)}`, ...x]);
      }
    };
    checkHealth();
    const timer = setInterval(checkHealth, 5000);

    const unsub = subscribeEvents((evt) => {
      setLastEvent(evt);
      if (evt.status === "completed" || evt.status === "failed" || evt.status === "cancelled") {
        setActiveJobId(null);
      }
    });

    return () => {
      clearInterval(timer);
      unsub();
    };
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    getCurrentWindow()
      .onDragDropEvent((event) => {
        if (event.payload.type === "drop") {
          const dropped = event.payload.paths.map((p) => toInputFile(p));
          void addFiles(dropped);
        }
      })
      .then((fn) => { unlisten = fn; });
    return () => { if (unlisten) unlisten(); };
  }, [maxFiles]);

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
      defaultPath: "merged_and_compressed.pdf"
    });
    if (selected) setOutputPath(String(selected));
  };

  const totalBytes = useMemo(() => files.reduce((sum, f) => sum + f.sizeBytes, 0), [files]);
  const progressValue = Math.max(0, Math.min(1, lastEvent?.progress ?? 0));

  return (
    <div className="app">
      {showHelp && (
        <div className="modalOverlay" onClick={() => setShowHelp(false)}>
          <div className="modal" onClick={e => e.stopPropagation()}>
            <h2>How to use PDF Tool</h2>
            <p>1. Drag and drop PDF files into the grid.</p>
            <p>2. Reorder files by dragging them around.</p>
            <p>3. Choose a compression preset in the sidebar.</p>
            <p>4. Select where to save your output.</p>
            <p>5. Click Start and watch the magic happen.</p>
            <button className="btnPrimary" onClick={() => setShowHelp(false)}>Got it</button>
          </div>
        </div>
      )}

      <aside className="sidebar">
        <header className="sidebarHeader">
          <h1>
            <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round"><path d="M14.5 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7.5L14.5 2z"/><polyline points="14 2 14 8 20 8"/></svg>
            PDF Tool
          </h1>
        </header>

        <div className="sidebarSection">
          <label>Compression</label>
          <select value={preset} onChange={(e) => setPreset(e.target.value as CompressionPreset)}>
            <option value="none">Merge Only (Fastest)</option>
            <option value="low">Low (Highest Quality)</option>
            <option value="medium">Medium (Recommended)</option>
            <option value="high">High (Small Size)</option>
            <option value="aggressive">Aggressive (Smallest)</option>
          </select>
        </div>

        <div className="sidebarSection">
          <label>Output Location</label>
          <button className="btnSecondary" onClick={selectOutput}>
            {outputPath ? "Change Output..." : "Choose Save Path..."}
          </button>
          {outputPath && <div className="cardMeta" style={{wordBreak:'break-all'}}>{outputPath}</div>}
        </div>

        <div className="sidebarSection" style={{marginTop: 'auto'}}>
          <button 
            className="btnPrimary" 
            disabled={!canStart}
            onClick={async () => {
              const res = await createJob({ files, preset, outputPath, maxWorkers: 1 });
              setActiveJobId(res.jobId);
              setLogs([]);
            }}
          >
            {activeJobId ? "Processing..." : "Start Process"}
          </button>
          {activeJobId && (
            <button className="btnSecondary" onClick={() => cancelJob(activeJobId)}>
              Cancel Job
            </button>
          )}
        </div>

        <div className="sidebarSection">
          <div className="runtimeStatus">
            <div className={`indicator ${health?.status ?? 'error'}`} />
            {health?.status === 'ok' ? 'System Ready' : 'System Degraded'}
          </div>
          <button className="btnSecondary" style={{fontSize: '11px'}} onClick={() => setShowHelp(true)}>Help & Instructions</button>
        </div>
      </aside>

      <main className="main">
        <header className="topBar">
          <div className="fileStats">
            <span><strong>{files.length}</strong> Files</span>
            <span><strong>{formatBytes(totalBytes)}</strong> Total</span>
          </div>
          <div className="row">
            <button className="btnSecondary" onClick={selectFiles}>Add Files</button>
            <button className="btnSecondary" style={{color: '#ef4444'}} onClick={() => setFiles([])} disabled={files.length === 0 || !!activeJobId}>Clear All</button>
          </div>
        </header>

        <div 
          className="dropzoneContainer"
          onDragOver={(e) => { e.preventDefault(); setIsDropActive(true); }}
          onDragLeave={() => setIsDropActive(false)}
          onDrop={(e) => {
            e.preventDefault();
            setIsDropActive(false);
            const dropped = Array.from(e.dataTransfer.files)
              .filter(f => f.name.toLowerCase().endsWith('.pdf'))
              .map(f => ({ path: (f as any).path, name: f.name, sizeBytes: f.size }));
            addFiles(dropped as any);
          }}
        >
          <div className={`dropzoneOverlay ${isDropActive ? 'active' : ''}`}>
            <h2>Drop PDFs here to add</h2>
          </div>

          {files.length === 0 ? (
            <div className="emptyState">
              <div className="icon">📄</div>
              <h2>No PDFs added yet</h2>
              <p>Drag and drop files here or use the button above</p>
            </div>
          ) : (
            <div className="fileGrid">
              {files.map((f, i) => (
                <div 
                  key={`${f.path}-${i}`}
                  className={`fileCard ${draggingIndex === i ? 'dragging' : ''}`}
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
                  <button className="btnRemove" onClick={() => setFiles(arr => arr.filter((_, idx) => idx !== i))}>×</button>
                  <div className="thumbContainer">
                    {f.thumbnail ? (
                      <img src={f.thumbnail} alt={f.name} />
                    ) : (
                      <div className="noThumb">{isInspecting ? "Loading..." : "No Preview"}</div>
                    )}
                  </div>
                  <div className="cardInfo">
                    <div className="cardName" title={f.name}>{f.name}</div>
                    <div className="cardMeta">
                      <span>{f.pageCount} pages</span>
                      <span>{formatBytes(f.sizeBytes)}</span>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        {(activeJobId || lastEvent) && (
          <footer className="statusPanel">
            <div className="progressBar">
              <div className="progressFill" style={{width: `${progressValue * 100}%`}} />
            </div>
            <div className="statusInfo">
              <span>{lastEvent?.message || "Preparing..."}</span>
              <span>{Math.round(progressValue * 100)}%</span>
            </div>
            {logs.length > 0 && (
              <div className="logs">
                {logs.map((l, i) => <div key={i}>{l}</div>)}
              </div>
            )}
          </footer>
        )}
      </main>
    </div>
  );
}
