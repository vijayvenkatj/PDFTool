package job

import (
	"context"
	"fmt"
	"os"
	"time"

	"pdftool/backend/go-service/internal/pipeline"
)

func ensureUsableOutput(ctx context.Context, tools pipeline.ToolPaths, repairedPath, outputPath string) error {
	info, err := os.Stat(outputPath)
	if err != nil {
		return err
	}
	if info.Size() <= 0 {
		return fmt.Errorf("empty output")
	}
	checkCtx, cancel := context.WithTimeout(ctx, 90*time.Second)
	defer cancel()
	if err := pipeline.CheckPDF(checkCtx, tools, outputPath); err != nil {
		return err
	}
	// If output is substantially larger than repaired input, treat as non-beneficial.
	inInfo, err := os.Stat(repairedPath)
	if err == nil && inInfo.Size() > 0 && info.Size() > inInfo.Size()*12/10 {
		return fmt.Errorf("output grew too much")
	}
	return nil
}
