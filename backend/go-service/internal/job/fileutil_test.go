package job

import (
	"strings"
	"testing"
)

func TestSanitizeIsStableAndUniqueByPath(t *testing.T) {
	a := sanitize("sample file.pdf", "/tmp/a/sample file.pdf")
	b := sanitize("sample file.pdf", "/tmp/b/sample file.pdf")
	if a == b {
		t.Fatalf("expected different sanitized names for different paths, got %q", a)
	}
	if !strings.HasPrefix(a, "sample_file-") {
		t.Fatalf("unexpected sanitize prefix: %q", a)
	}
}

func TestShortHashDeterministic(t *testing.T) {
	v := "abc/path.pdf"
	h1 := shortHash(v)
	h2 := shortHash(v)
	if h1 != h2 {
		t.Fatalf("shortHash not deterministic: %q vs %q", h1, h2)
	}
	if len(h1) != 8 {
		t.Fatalf("expected 8-char hash, got %q", h1)
	}
}
