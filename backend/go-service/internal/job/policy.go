package job

import (
	"time"

	"pdftool/backend/go-service/internal/model"
)

func perFileCompressTimeout(preset model.CompressionPreset) time.Duration {
	switch preset {
	case model.PresetAggressive:
		return 90 * time.Second
	case model.PresetLow:
		return 2 * time.Minute
	case model.PresetMedium:
		return 3 * time.Minute
	case model.PresetHigh:
		return 4 * time.Minute
	default:
		return 3 * time.Minute
	}
}
