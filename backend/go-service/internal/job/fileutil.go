package job

import (
	"fmt"
	"hash/fnv"
	"os"
	"path/filepath"
	"strings"
	"time"
)

func sanitize(name string, fullPath string) string {
	name = strings.ReplaceAll(name, " ", "_")
	name = strings.TrimSuffix(name, filepath.Ext(name))
	if name == "" {
		return fmt.Sprintf("file-%d", time.Now().UnixNano())
	}
	return fmt.Sprintf("%s-%s", name, shortHash(fullPath))
}

func shortHash(value string) string {
	h := fnv.New32a()
	_, _ = h.Write([]byte(value))
	return fmt.Sprintf("%08x", h.Sum32())
}

func copyFile(src, dst string) error {
	var lastErr error
	for attempt := 0; attempt < 3; attempt++ {
		if attempt > 0 {
			time.Sleep(120 * time.Millisecond)
		}
		in, err := os.Open(src)
		if err != nil {
			lastErr = err
			continue
		}

		out, err := os.Create(dst)
		if err != nil {
			_ = in.Close()
			lastErr = err
			continue
		}

		_, readErr := out.ReadFrom(in)
		closeInErr := in.Close()
		syncErr := out.Sync()
		closeOutErr := out.Close()
		if readErr == nil && closeInErr == nil && syncErr == nil && closeOutErr == nil {
			return nil
		}
		if readErr != nil {
			lastErr = readErr
		} else if closeInErr != nil {
			lastErr = closeInErr
		} else if syncErr != nil {
			lastErr = syncErr
		} else {
			lastErr = closeOutErr
		}
	}
	return lastErr
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}

func max(a, b int) int {
	if a > b {
		return a
	}
	return b
}
