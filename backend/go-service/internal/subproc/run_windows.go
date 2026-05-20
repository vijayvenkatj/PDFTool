//go:build windows

package subproc

import (
	"fmt"
	"os/exec"
)

func setCmdSysProcAttr(cmd *exec.Cmd) {}

func terminateProcessTree(cmd *exec.Cmd) error {
	if cmd.Process == nil {
		return nil
	}
	killCmd := exec.Command("taskkill", "/F", "/T", "/PID", fmt.Sprintf("%d", cmd.Process.Pid))
	_ = killCmd.Run()
	return nil
}
