# Professional Offline PDF Tool

A high-performance, privacy-focused desktop application for merging and compressing PDF documents. Built with Tauri, Rust, and React, this tool offers a professional SaaS-like experience while keeping all your files locally on your machine.

![Version](https://img.shields.io/badge/version-1.0.0-blue)
![License](https://img.shields.io/badge/license-MIT-green)
![Platform](https://img.shields.io/badge/platform-macOS%20|%20Windows-lightgrey)

## Key Features

*   **Visual File Management**: Card-based UI with live thumbnails for every PDF.
*   **High Capacity**: Built to handle up to **1,000 files** in a single job.
*   **Parallel Processing**: Multithreaded Rust backend for lightning-fast inspection and compression.
*   **Intelligent Compression**: Five levels of optimization, from "Fast Merge" to "Aggressive Compression".
*   **Intuitive UX**: Drag-and-drop reordering and file adding.
*   **Privacy First**: 100% offline. No telemetry. Your documents never leave your computer.

## Requirements

The tool leverages industry-standard PDF engines for maximum reliability:

- **qpdf**: Used for structural manipulation and fast merging.
- **Ghostscript**: Used for high-quality compression and thumbnail generation.

### Installation

#### macOS
```bash
brew install qpdf ghostscript
```

#### Windows
```powershell
choco install qpdf ghostscript
```

## Getting Started

1.  **Add Files**: Drag and drop your PDFs into the application grid.
2.  **Organize**: Drag cards to reorder the final document sequence.
3.  **Configure**: Select your desired compression level from the sidebar.
4.  **Process**: Choose an output location and click **Start Process**.

## Tech Stack

- **Frontend**: React, TypeScript, Vite, Vanilla CSS.
- **Backend**: Rust (Tauri), Ghostscript, qpdf.
- **Parallelism**: Tokio-based async runtime for non-blocking UI and background workers.

## Contributing

Contributions are welcome. Please ensure your code follows the established architectural patterns and includes necessary tests.

---
Developed by [vijayvenkatj](https://github.com/vijayvenkatj)
