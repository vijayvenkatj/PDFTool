# PDFTool Setup Guide

## Prerequisites

PDFTool requires qpdf and Ghostscript to be installed on your system.

### Windows

1. **Install Chocolatey** (if not already installed):
   - Open PowerShell as Administrator
   - Run: `Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser`
   - Run: `[System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072; iex ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))`

2. **Install qpdf and Ghostscript**:
   - Open PowerShell as Administrator
   - Run: `choco install qpdf ghostscript -y`

3. **Verify installation**:
   - Run: `where qpdf.exe`
   - Run: `where gswin64c.exe`
   - Both should return paths

4. **Download and run PDFTool**:
   - Download `PDFTool.msi` from Releases
   - Run installer
   - Launch PDFTool from Start Menu

### macOS

1. **Install Homebrew** (if not already installed):
   ```bash
   /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
   ```

2. **Install qpdf and Ghostscript**:
   ```bash
   brew install qpdf ghostscript
   ```

3. **Verify installation**:
   ```bash
   which qpdf
   which gs
   ```
   Both should return paths

4. **Download and run PDFTool**:
   - Download `PDFTool.app.tar.gz` from Releases
   - Extract: `tar xzf PDFTool.app.tar.gz`
   - Run: `open PDFTool.app`

## Troubleshooting

**"Degraded" status on startup:**
- qpdf or Ghostscript not found in PATH
- Run the installation commands above
- Close and reopen PDFTool

**"Command not found" when running tools:**
- Open a new terminal/PowerShell
- Tools may not be in current session's PATH

**Permission denied on macOS:**
- Right-click PDFTool.app → Open
- Grant permission when prompted

## Development

For local development builds, ensure tools are installed first, then:

```bash
cd apps/desktop
npm install
npm run tauri:dev
```

