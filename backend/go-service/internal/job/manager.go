package job

import (
	"context"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"sync/atomic"
	"time"

	"pdftool/backend/go-service/internal/model"
	"pdftool/backend/go-service/internal/pipeline"
	"pdftool/backend/go-service/internal/progress"
	"pdftool/backend/go-service/internal/tempfs"
)

type jobState struct {
	cancel context.CancelFunc
}

type Manager struct {
	tools   pipeline.ToolPaths
	broker  *progress.Broker
	jobs    map[string]jobState
	jobsMu  sync.Mutex
	counter atomic.Uint64
}

type result struct {
	in      model.InputFile
	outPath string
	err     error
	skipped bool
	reason  string
	note    string
}

func NewManager(tools pipeline.ToolPaths, broker *progress.Broker) *Manager {
	return &Manager{
		tools:  tools,
		broker: broker,
		jobs:   make(map[string]jobState),
	}
}

func (m *Manager) Start(req model.CreateJobRequest) (string, error) {
	if len(req.Files) == 0 {
		return "", errors.New("no files")
	}
	if req.MaxWorkers <= 0 {
		req.MaxWorkers = max(1, min(runtime.NumCPU()/2, 4))
	}

	id := fmt.Sprintf("job-%d", m.counter.Add(1))
	ctx, cancel := context.WithCancel(context.Background())
	m.jobsMu.Lock()
	m.jobs[id] = jobState{cancel: cancel}
	m.jobsMu.Unlock()

	go m.run(ctx, id, req)
	return id, nil
}

func (m *Manager) Cancel(jobID string) error {
	m.jobsMu.Lock()
	defer m.jobsMu.Unlock()
	st, ok := m.jobs[jobID]
	if !ok {
		return fmt.Errorf("job not found")
	}
	st.cancel()
	return nil
}

func (m *Manager) emit(evt model.JobEvent) {
	m.broker.Publish(evt)
}

func (m *Manager) run(ctx context.Context, jobID string, req model.CreateJobRequest) {
	defer func() {
		m.jobsMu.Lock()
		delete(m.jobs, jobID)
		m.jobsMu.Unlock()
	}()

	w, err := tempfs.New(jobID)
	if err != nil {
		m.emit(model.JobEvent{JobID: jobID, Stage: "init", Progress: 0, Message: err.Error(), Status: "failed"})
		return
	}
	defer w.Cleanup()

	m.emit(model.JobEvent{JobID: jobID, Stage: "queued", Progress: 0, Message: "job started", Status: "running"})

	inputCh := make(chan model.InputFile)
	resultCh := make(chan result, len(req.Files))
	var wg sync.WaitGroup
	var activeWorkers atomic.Int32
	var activeMu sync.Mutex
	activeFiles := make(map[string]struct{})

	for i := 0; i < req.MaxWorkers; i++ {
		wg.Add(1)
		go func(workerID int) {
			defer wg.Done()
			for in := range inputCh {
				select {
				case <-ctx.Done():
					return
				default:
				}
				m.emit(model.JobEvent{
					JobID:    jobID,
					Stage:    "compress",
					Progress: 0,
					Message:  "starting " + in.Name,
					Status:   "running",
				})
				activeMu.Lock()
				activeFiles[in.Name] = struct{}{}
				activeMu.Unlock()
				activeWorkers.Add(1)
				r := m.processOne(ctx, w.Root, req.Preset, in)
				activeWorkers.Add(-1)
				activeMu.Lock()
				delete(activeFiles, in.Name)
				activeMu.Unlock()
				resultCh <- r
			}
		}(i)
	}

	go func() {
		for _, f := range req.Files {
			inputCh <- f
		}
		close(inputCh)
		wg.Wait()
		close(resultCh)
	}()

	compressed := make([]string, 0, len(req.Files))
	skipped := make([]model.SkippedFile, 0)
	done := 0
	total := len(req.Files) + 2
	remaining := len(req.Files)
	ticker := time.NewTicker(2 * time.Second)
	defer ticker.Stop()

	for remaining > 0 {
		select {
		case r, ok := <-resultCh:
			if !ok {
				remaining = 0
				break
			}
			remaining--
			done++
			if r.err != nil || r.skipped {
				reason := r.reason
				if reason == "" && r.err != nil {
					reason = r.err.Error()
				}
				skipped = append(skipped, model.SkippedFile{Path: r.in.Path, Reason: reason})
				m.emit(model.JobEvent{
					JobID:    jobID,
					Stage:    "compress",
					Progress: float64(done) / float64(total),
					Message:  "skipped " + r.in.Name,
					Status:   "running",
				})
				continue
			}
			compressed = append(compressed, r.outPath)
			message := "processed " + r.in.Name
			if r.note != "" {
				message += " (" + r.note + ")"
			}
			m.emit(model.JobEvent{
				JobID:    jobID,
				Stage:    "compress",
				Progress: float64(done) / float64(total),
				Message:  message,
				Status:   "running",
			})
		case <-ticker.C:
			if remaining > 0 {
				activeMu.Lock()
				current := make([]string, 0, len(activeFiles))
				for name := range activeFiles {
					current = append(current, name)
				}
				activeMu.Unlock()
				activeHint := ""
				if len(current) > 0 {
					activeHint = " current: " + strings.Join(current, ", ")
				}
				m.emit(model.JobEvent{
					JobID:    jobID,
					Stage:    "compress",
					Progress: float64(done) / float64(total),
					Message:  fmt.Sprintf("compressing... %d/%d complete (%d active).%s", done, len(req.Files), activeWorkers.Load(), activeHint),
					Status:   "running",
				})
			}
		}
	}

	if ctx.Err() != nil {
		m.emit(model.JobEvent{JobID: jobID, Stage: "cancel", Progress: 1, Message: "cancelled", Status: "cancelled", Skipped: skipped})
		return
	}

	if len(compressed) == 0 {
		m.emit(model.JobEvent{JobID: jobID, Stage: "merge", Progress: 1, Message: "all files failed", Status: "failed", Skipped: skipped})
		return
	}

	merged := filepath.Join(w.Root, "merged.pdf")
	if err := pipeline.MergePDFs(ctx, m.tools, compressed, merged); err != nil {
		m.emit(model.JobEvent{JobID: jobID, Stage: "merge", Progress: 0.9, Message: err.Error(), Status: "failed", Skipped: skipped})
		return
	}

	m.emit(model.JobEvent{JobID: jobID, Stage: "merge", Progress: float64(total-1) / float64(total), Message: "merge complete", Status: "running"})

	finalTmp := filepath.Join(w.Root, "final.pdf")
	if err := pipeline.FinalOptimize(ctx, m.tools, merged, finalTmp); err != nil {
		m.emit(model.JobEvent{JobID: jobID, Stage: "optimize", Progress: 0.95, Message: err.Error(), Status: "failed", Skipped: skipped})
		return
	}

	if err := os.MkdirAll(filepath.Dir(req.OutputPath), 0o755); err != nil {
		m.emit(model.JobEvent{JobID: jobID, Stage: "finalize", Progress: 0.99, Message: err.Error(), Status: "failed", Skipped: skipped})
		return
	}

	// Atomic final write on same filesystem.
	staged := req.OutputPath + ".tmp"
	if err := copyFile(finalTmp, staged); err != nil {
		m.emit(model.JobEvent{JobID: jobID, Stage: "finalize", Progress: 0.99, Message: err.Error(), Status: "failed", Skipped: skipped})
		return
	}
	if err := os.Rename(staged, req.OutputPath); err != nil {
		m.emit(model.JobEvent{JobID: jobID, Stage: "finalize", Progress: 0.99, Message: err.Error(), Status: "failed", Skipped: skipped})
		return
	}

	m.emit(model.JobEvent{
		JobID:    jobID,
		Stage:    "done",
		Progress: 1,
		Message:  "completed",
		Status:   "completed",
		Skipped:  skipped,
		Output:   req.OutputPath,
	})
}

func (m *Manager) processOne(ctx context.Context, root string, preset model.CompressionPreset, in model.InputFile) result {
	if strings.TrimSpace(in.Path) == "" {
		return result{in: in, skipped: true, reason: "missing absolute file path"}
	}
	if _, err := os.Stat(in.Path); err != nil {
		return result{in: in, skipped: true, reason: err.Error()}
	}
	base := sanitize(filepath.Base(in.Path), in.Path)
	original := filepath.Join(root, base+".input.pdf")
	repaired := filepath.Join(root, base+".repaired.pdf")
	compressed := filepath.Join(root, base+".compressed.pdf")

	if err := copyFile(in.Path, original); err != nil {
		return result{in: in, err: err}
	}
	if err := pipeline.RepairPDF(ctx, m.tools, original, repaired); err != nil {
		return result{in: in, skipped: true, reason: "repair failed: " + err.Error()}
	}
	compressCtx := ctx
	compressTimeout := perFileCompressTimeout(preset)
	if compressTimeout > 0 {
		var cancel context.CancelFunc
		compressCtx, cancel = context.WithTimeout(ctx, compressTimeout)
		defer cancel()
	}
	if err := pipeline.CompressPDF(compressCtx, m.tools, preset, repaired, compressed); err != nil {
		if preset == model.PresetAggressive {
			// Aggressive can fail on some malformed/image-heavy PDFs; retry lower preset per-file.
			lowCtx, cancelLow := context.WithTimeout(ctx, 3*time.Minute)
			defer cancelLow()
			if retryErr := pipeline.CompressPDF(lowCtx, m.tools, model.PresetLow, repaired, compressed); retryErr == nil {
				if okErr := ensureUsableOutput(ctx, m.tools, repaired, compressed); okErr == nil {
					return result{in: in, outPath: compressed, note: "aggressive fallback to low"}
				}
			}
		}
		// Already-compressed or unusual PDFs can fail Ghostscript; fallback to safe qpdf optimization path.
		if fallbackErr := pipeline.FinalOptimize(ctx, m.tools, repaired, compressed); fallbackErr == nil {
			if okErr := ensureUsableOutput(ctx, m.tools, repaired, compressed); okErr == nil {
				return result{in: in, outPath: compressed, note: "compression fallback (qpdf optimize)"}
			}
		}
		return result{in: in, skipped: true, reason: "compress failed: " + err.Error()}
	}
	if err := ensureUsableOutput(ctx, m.tools, repaired, compressed); err != nil {
		if fallbackErr := pipeline.FinalOptimize(ctx, m.tools, repaired, compressed); fallbackErr == nil {
			if okErr := ensureUsableOutput(ctx, m.tools, repaired, compressed); okErr == nil {
				return result{in: in, outPath: compressed, note: "compression fallback (qpdf optimize)"}
			}
		}
		return result{in: in, skipped: true, reason: "compressed output invalid: " + err.Error()}
	}
	return result{in: in, outPath: compressed}
}
