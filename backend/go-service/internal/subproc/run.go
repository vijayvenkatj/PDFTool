package subproc

import (
	"bytes"
	"context"
	"fmt"
	"os/exec"
	"time"
)

func Run(ctx context.Context, timeout time.Duration, bin string, args ...string) error {
	runCtx, cancel := context.WithTimeout(ctx, timeout)
	defer cancel()

	cmd := exec.Command(bin, args...)
	setCmdSysProcAttr(cmd)
	var stderr bytes.Buffer
	cmd.Stderr = &stderr

	if err := cmd.Start(); err != nil {
		return fmt.Errorf("%s %v start failed: %w", bin, args, err)
	}

	done := make(chan error, 1)
	go func() { done <- cmd.Wait() }()

	select {
	case <-runCtx.Done():
		_ = terminateProcessTree(cmd)
		<-done
		return fmt.Errorf("%s %v timeout/cancelled: %w (%s)", bin, args, runCtx.Err(), stderr.String())
	case err := <-done:
		if err != nil {
			return fmt.Errorf("%s %v failed: %w (%s)", bin, args, err, stderr.String())
		}
		return nil
	}
}
