# Installation Guide

This guide will walk you through setting up PDFTool and its required dependencies on your system.

## Windows Installation

### 1. Install Chocolatey (Package Manager)
Chocolatey is the easiest way to install the required tools on Windows.

1. Right-click the **Start** button and select **PowerShell (Admin)** or **Windows Terminal (Admin)**.
2. Run the following command to allow script execution:
   ```powershell
   Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
   ```
3. Copy and paste the following command to install Chocolatey:
   ```powershell
   [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072; iex ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))
   ```
4. Restart your terminal to apply changes.

### 2. Install qpdf and Ghostscript
With Chocolatey installed, you can now install the PDF processing engines.

1. Open **PowerShell (Admin)** again.
2. Run the following command:
   ```powershell
   choco install qpdf ghostscript -y
   ```

### 3. Verify Installation
Ensure the tools are correctly installed and in your PATH:
```powershell
where qpdf.exe
where gswin64c.exe
```
If both commands return a file path, you are ready to go.

### 4. Install PDFTool
1. Go to the [Releases](https://github.com/vijayvenkatj/PDFTool/releases) page.
2. Download the latest `PDFTool.msi` or `.exe` installer.
3. Run the installer and follow the prompts.
4. Launch **PDFTool** from your Start Menu.

---

## macOS Installation

### 1. Install Homebrew (Package Manager)
Homebrew is the standard package manager for macOS.

1. Open the **Terminal** app.
2. Run the following command:
   ```bash
   /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
   ```
3. Follow the on-screen instructions to add Homebrew to your PATH (if prompted).

### 2. Install qpdf and Ghostscript
1. In the **Terminal**, run:
   ```bash
   brew install qpdf ghostscript
   ```

### 3. Verify Installation
Run these commands to confirm:
```bash
which qpdf
which gs
```

### 4. Install PDFTool
1. Go to the [Releases](https://github.com/vijayvenkatj/PDFTool/releases) page.
2. Download the `PDFTool.app.tar.gz` file.
3. Extract the archive and drag **PDFTool.app** to your **Applications** folder.
4. **Note:** On first launch, you may need to right-click the app and select **Open** to bypass security warnings for unsigned apps.

---

## Troubleshooting

- **Degraded Status:** If the app shows a "Degraded" status, it means it cannot find `qpdf` or `gs`. Restart your computer to ensure the PATH changes are fully applied.
- **Tools not found:** Ensure you have completed the package manager installation steps above.
