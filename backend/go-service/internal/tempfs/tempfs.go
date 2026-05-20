package tempfs

import (
	"os"
	"path/filepath"
)

type Workspace struct {
	Root string
}

func New(jobID string) (*Workspace, error) {
	root, err := os.MkdirTemp("", "pdftool-"+jobID+"-")
	if err != nil {
		return nil, err
	}
	return &Workspace{Root: root}, nil
}

func (w *Workspace) Path(parts ...string) string {
	all := []string{w.Root}
	all = append(all, parts...)
	return filepath.Join(all...)
}

func (w *Workspace) Cleanup() error {
	return os.RemoveAll(w.Root)
}
