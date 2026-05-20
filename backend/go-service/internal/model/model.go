package model

type CompressionPreset string

const (
	PresetLow        CompressionPreset = "low"
	PresetMedium     CompressionPreset = "medium"
	PresetHigh       CompressionPreset = "high"
	PresetAggressive CompressionPreset = "aggressive"
)

type InputFile struct {
	Path      string `json:"path"`
	Name      string `json:"name"`
	SizeBytes int64  `json:"sizeBytes"`
}

type CreateJobRequest struct {
	Files      []InputFile       `json:"files"`
	Preset     CompressionPreset `json:"preset"`
	OutputPath string            `json:"outputPath"`
	MaxWorkers int               `json:"maxWorkers"`
}

type CreateJobResponse struct {
	JobID string `json:"jobId"`
}

type InspectFilesRequest struct {
	Paths []string `json:"paths"`
}

type InspectedFile struct {
	Path      string `json:"path"`
	Name      string `json:"name"`
	SizeBytes int64  `json:"sizeBytes"`
	Exists    bool   `json:"exists"`
	Error     string `json:"error,omitempty"`
}

type InspectFilesResponse struct {
	Files []InspectedFile `json:"files"`
}

type ToolHealth struct {
	Path    string `json:"path"`
	Ok      bool   `json:"ok"`
	Version string `json:"version,omitempty"`
	Error   string `json:"error,omitempty"`
}

type HealthResponse struct {
	Status      string     `json:"status"`
	QPDF        ToolHealth `json:"qpdf"`
	Ghostscript ToolHealth `json:"ghostscript"`
}

type SkippedFile struct {
	Path   string `json:"path"`
	Reason string `json:"reason"`
}

type JobEvent struct {
	JobID    string        `json:"jobId"`
	Stage    string        `json:"stage"`
	Progress float64       `json:"progress"`
	Message  string        `json:"message"`
	Status   string        `json:"status"`
	Skipped  []SkippedFile `json:"skipped,omitempty"`
	Output   string        `json:"outputPath,omitempty"`
}
