package main

import (
	"flag"
	"log"
	"net/http"
	"os"

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
