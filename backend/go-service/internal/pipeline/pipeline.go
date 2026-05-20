package pipeline

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"time"

	"pdftool/backend/go-service/internal/model"
	"pdftool/backend/go-service/internal/subproc"
)

type ToolPaths struct {
	QPDF        string
	Ghostscript string
}

func RepairPDF(ctx context.Context, tools ToolPaths, inPath, outPath string) error {
	return subproc.Run(ctx, 2*time.Minute, tools.QPDF, "--object-streams=generate", inPath, outPath)
}

func CompressPDF(ctx context.Context, tools ToolPaths, preset model.CompressionPreset, inPath, outPath string) error {
	quality := "/ebook"
	isAggressive := false
	switch preset {
	case model.PresetLow:
		quality = "/screen"
	case model.PresetMedium:
		quality = "/ebook"
	case model.PresetHigh:
		quality = "/printer"
	case model.PresetAggressive:
		quality = "/screen"
		isAggressive = true
	}

	args := []string{
		"-sDEVICE=pdfwrite",
		"-dCompatibilityLevel=1.6",
		"-dNOPAUSE",
		"-dQUIET",
		"-dBATCH",
		"-dDetectDuplicateImages=true",
		"-dCompressFonts=true",
		"-dPDFSETTINGS=" + quality,
	}
	if isAggressive {
		args = append(args,
			"-dAutoFilterColorImages=false",
			"-dAutoFilterGrayImages=false",
			"-dColorImageFilter=/DCTEncode",
			"-dGrayImageFilter=/DCTEncode",
			"-dDownsampleColorImages=true",
			"-dDownsampleGrayImages=true",
			"-dColorImageDownsampleType=/Bicubic",
			"-dGrayImageDownsampleType=/Bicubic",
			"-dColorImageResolution=96",
			"-dGrayImageResolution=96",
			"-dJPEGQ=35",
		)
	}
	args = append(args,
		"-sOutputFile="+outPath,
		inPath,
	)
	return subproc.Run(ctx, 5*time.Minute, tools.Ghostscript, args...)
}

func MergePDFs(ctx context.Context, tools ToolPaths, inputs []string, outPath string) error {
	if len(inputs) == 0 {
		return fmt.Errorf("no inputs to merge")
	}
	args := []string{"--empty", "--pages"}
	args = append(args, inputs...)
	args = append(args, "--", outPath)
	return subproc.Run(ctx, 3*time.Minute, tools.QPDF, args...)
}

func FinalOptimize(ctx context.Context, tools ToolPaths, inPath, outPath string) error {
	if err := os.MkdirAll(filepath.Dir(outPath), 0o755); err != nil {
		return err
	}
	return subproc.Run(ctx, 2*time.Minute, tools.QPDF, "--object-streams=generate", "--stream-data=compress", inPath, outPath)
}

func CheckPDF(ctx context.Context, tools ToolPaths, path string) error {
	return subproc.Run(ctx, 90*time.Second, tools.QPDF, "--check", path)
}
