package main

import (
	"flag"
	"log"
	"net/http"
	"os"
	"strings"

	"pdftool/backend/go-service/internal/api"
	"pdftool/backend/go-service/internal/job"
	"pdftool/backend/go-service/internal/pipeline"
	"pdftool/backend/go-service/internal/progress"
)

func main() {
	port := flag.String("port", "47832", "listen port")
	qpdfPath := flag.String("qpdf", envOrDefault("PDFTOOL_QPDF_PATH", "qpdf"), "qpdf binary path")
	gsPath := flag.String("gs", envOrDefault("PDFTOOL_GS_PATH", defaultGSBinary()), "ghostscript binary path")
	flag.Parse()
	*qpdfPath = normalizePathArg(*qpdfPath)
	*gsPath = normalizePathArg(*gsPath)

	broker := progress.NewBroker()
	tools := pipeline.ToolPaths{
		QPDF:        *qpdfPath,
		Ghostscript: *gsPath,
	}
	manager := job.NewManager(tools, broker)
	server := api.NewServer(manager, broker, tools)

	addr := "127.0.0.1:" + *port
	log.Printf("pdf service listening on %s", addr)
	if err := http.ListenAndServe(addr, server.Routes()); err != nil {
		log.Fatalf("server exited: %v", err)
	}
}

func envOrDefault(key, fallback string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return fallback
}

func defaultGSBinary() string {
	if os.Getenv("OS") == "Windows_NT" {
		return "gswin64c"
	}
	return "gs"
}

func normalizePathArg(v string) string {
	trimmed := strings.TrimSpace(v)
	trimmed = strings.Trim(trimmed, "\"'")
	trimmed = strings.ReplaceAll(trimmed, "\r", "")
	trimmed = strings.ReplaceAll(trimmed, "\n", "")
	if len(trimmed) >= 4 &&
		(trimmed[0] == '\\' || trimmed[0] == '/') &&
		trimmed[2] == ':' &&
		(trimmed[3] == '\\' || trimmed[3] == '/') &&
		((trimmed[1] >= 'a' && trimmed[1] <= 'z') || (trimmed[1] >= 'A' && trimmed[1] <= 'Z')) {
		trimmed = trimmed[1:]
	}
	if trimmed == "" {
		return v
	}
	return trimmed
}
