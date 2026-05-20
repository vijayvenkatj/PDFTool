package job

import (
	"testing"
	"time"

	"pdftool/backend/go-service/internal/model"
)

func TestPerFileCompressTimeout(t *testing.T) {
	tests := []struct {
		name   string
		preset model.CompressionPreset
		want   time.Duration
	}{
		{"aggressive", model.PresetAggressive, 90 * time.Second},
		{"low", model.PresetLow, 2 * time.Minute},
		{"medium", model.PresetMedium, 3 * time.Minute},
		{"high", model.PresetHigh, 4 * time.Minute},
		{"default", model.CompressionPreset("unknown"), 3 * time.Minute},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			if got := perFileCompressTimeout(tc.preset); got != tc.want {
				t.Fatalf("perFileCompressTimeout(%q) = %v, want %v", tc.preset, got, tc.want)
			}
		})
	}
}
