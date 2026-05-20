package api

import (
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"os/exec"
	"path/filepath"
	"strings"

	"pdftool/backend/go-service/internal/job"
	"pdftool/backend/go-service/internal/model"
	"pdftool/backend/go-service/internal/pipeline"
	"pdftool/backend/go-service/internal/progress"
)

type Server struct {
	manager *job.Manager
	broker  *progress.Broker
	tools   pipeline.ToolPaths
}

func NewServer(manager *job.Manager, broker *progress.Broker, tools pipeline.ToolPaths) *Server {
	return &Server{manager: manager, broker: broker, tools: tools}
}

func (s *Server) Routes() http.Handler {
	mux := http.NewServeMux()
	mux.Handle("/v1/health", s.withCORS(http.HandlerFunc(s.health)))
	mux.Handle("/v1/events", s.withCORS(http.HandlerFunc(s.broker.ServeHTTP)))
	mux.Handle("/v1/files/inspect", s.withCORS(http.HandlerFunc(s.inspectFiles)))
	mux.Handle("/v1/jobs", s.withCORS(http.HandlerFunc(s.createJob)))
	mux.Handle("/v1/jobs/", s.withCORS(http.HandlerFunc(s.jobActions)))
	return mux
}

func (s *Server) withCORS(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Access-Control-Allow-Origin", "*")
		w.Header().Set("Access-Control-Allow-Methods", "GET,POST,OPTIONS")
		w.Header().Set("Access-Control-Allow-Headers", "Content-Type")
		if r.Method == http.MethodOptions {
			w.WriteHeader(http.StatusNoContent)
			return
		}
		next.ServeHTTP(w, r)
	})
}

func (s *Server) createJob(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var req model.CreateJobRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}
	id, err := s.manager.Start(req)
	if err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}
	_ = json.NewEncoder(w).Encode(model.CreateJobResponse{JobID: id})
}

func (s *Server) jobActions(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	path := strings.TrimPrefix(r.URL.Path, "/v1/jobs/")
	if !strings.HasSuffix(path, "/cancel") {
		http.Error(w, "unknown action", http.StatusNotFound)
		return
	}
	jobID := strings.TrimSuffix(path, "/cancel")
	jobID = strings.TrimSuffix(jobID, "/")
	if jobID == "" {
		http.Error(w, "missing job id", http.StatusBadRequest)
		return
	}
	if err := s.manager.Cancel(jobID); err != nil {
		http.Error(w, fmt.Sprintf("cancel failed: %v", err), http.StatusNotFound)
		return
	}
	w.WriteHeader(http.StatusNoContent)
}

func (s *Server) inspectFiles(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	var req model.InspectFilesRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}
	resp := model.InspectFilesResponse{Files: make([]model.InspectedFile, 0, len(req.Paths))}
	for _, p := range req.Paths {
		item := model.InspectedFile{
			Path: p,
			Name: filepath.Base(p),
		}
		st, err := os.Stat(p)
		if err != nil {
			item.Exists = false
			item.Error = err.Error()
		} else {
			item.Exists = true
			item.SizeBytes = st.Size()
		}
		resp.Files = append(resp.Files, item)
	}
	_ = json.NewEncoder(w).Encode(resp)
}

func (s *Server) health(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}
	resp := model.HealthResponse{
		QPDF:        checkTool(s.tools.QPDF, "--version"),
		Ghostscript: checkTool(s.tools.Ghostscript, "--version"),
	}
	if resp.QPDF.Ok && resp.Ghostscript.Ok {
		resp.Status = "ok"
	} else {
		resp.Status = "degraded"
	}
	_ = json.NewEncoder(w).Encode(resp)
}

func checkTool(path string, versionArg string) model.ToolHealth {
	cmd := exec.Command(path, versionArg)
	output, err := cmd.CombinedOutput()
	if err != nil {
		return model.ToolHealth{
			Path:  path,
			Ok:    false,
			Error: err.Error(),
		}
	}
	version := strings.TrimSpace(string(output))
	if idx := strings.Index(version, "\n"); idx >= 0 {
		version = version[:idx]
	}
	return model.ToolHealth{
		Path:    path,
		Ok:      true,
		Version: version,
	}
}
