# EXIF Date Corrector for Photos

Fast tool to correct EXIF dates of photos based on filename and Google Photos JSON files.

**Optimized for Google Takeout**: This tool is specifically designed to work with photos exported from Google Photos via Google Takeout. It automatically reads the supplemental JSON files (`.supplemental-metadata.json`, `.supplemental.json`, etc.) that Google Photos generates during export to recover the original photo dates.

## Rust Version (VERY FAST! ⚡)

The Rust version is **much faster** than the Python version because:
- Uses native Rust libraries to read EXIF (no external calls to exiftool)
- Automatically parallelizes with rayon (uses all CPU cores)
- Is compiled, so much faster than interpreted Python
- Includes modern GUI with egui

### Installation

1. Install Rust (if not already installed):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

Or add this line to your `~/.bashrc` to load it automatically:
```bash
echo 'source $HOME/.cargo/env' >> ~/.bashrc
source ~/.bashrc
```

2. Verify Rust is available:
```bash
cargo --version
```

You should see something like: `cargo 1.91.1 (ed61e7d7e 2025-11-07)`

3. Make the build script executable (first time only):
```bash
cd /home/alberto/takeout_photo/CorreggiEXIF
chmod +x ./build.sh
```

4. Build the project:

**Option A: Use build script (recommended)**
```bash
cd /home/alberto/takeout_photo/CorreggiEXIF
source $HOME/.cargo/env
./build.sh
```

**Option B: Build manually**
```bash
cd /home/alberto/takeout_photo/CorreggiEXIF
source $HOME/.cargo/env
cargo build --release
```

The first build may take a few minutes as it downloads and compiles all dependencies. Subsequent builds will be much faster.

### Usage

#### GUI (Graphical Interface)

To start the GUI, run without arguments:
```bash
./target/release/corrigi-exif
```

The GUI includes:
- Table with columns: File Name, Severity, Incongruities, DateTimeOriginal ⭐, → Proposal, CreateDate, → Proposal
- Side panel with 3 phases:
  - **Phase 1**: Folder selection
  - **Phase 2**: Proposal modifications (global strategy, calculate proposals)
  - **Phase 3**: Apply modifications
- Highlighting of rows with proposals (orange)
- Real-time statistics

#### CLI (Command Line)

To use the CLI version, pass the directory as argument:
```bash
./target/release/corrigi-exif <directory>
```

**Example:**
```bash
./target/release/corrigi-exif "/home/alberto/takeout_photo/Takeout/Google Foto/Miglior foto_ Natura"
```

### Available Strategies

- `json_photo_taken` (default): Use photoTakenTime from JSON
- `json_creation`: Use creationTime from JSON
- `nome_file_preferito`: Prefer year from filename, otherwise use JSON
- `nome_file`: Use only year from filename
- `json_preferito`: Prefer JSON, otherwise filename
- `exif_attuale`: Keep current EXIF

### Expected Output (CLI)

The program will show:
- How many photos it found
- Time taken for reading (should be very fast!)
- Statistics (total, with EXIF, with proposals)
- First 10 photos with modification proposals

### Troubleshooting

#### Error: "command not found: cargo"
```bash
source $HOME/.cargo/env
export PATH="$HOME/.cargo/bin:$PATH"
```

#### Build Errors

**Error: "failed to select a version for the requirement `exif = \"^0.8\"" or `exif = \"^0.7\""`
```bash
# This error means the `exif` library doesn't exist with those versions
# The project uses `kamadak-exif` instead of `exif`
# If you see this error, verify that Cargo.toml uses:
#   exif = { package = "kamadak-exif", version = "0.6" }
# instead of:
#   exif = "0.7" or "0.8"
cd /home/alberto/takeout_photo/CorreggiEXIF
cargo update
cargo build --release
```

**Other build errors:**
- Make sure all dependencies are installed. Rust will download them automatically during the first build.
- If you see version errors, try: `cargo update` to update dependencies

**Warning: "the following packages contain code that will be rejected by a future version of Rust: ashpd"**
- This is a future compatibility warning from a transitive dependency (`ashpd`, used by `rfd` for file dialog)
- It's not a critical error and the program works correctly
- The warning will be resolved when library authors update their dependencies
- You can safely ignore it

### Performance

The Rust version is **10-50x faster** than the Python version:
- **Rust**: ~0.5-2 seconds for 170 photos (parallel, native libraries)
- **Optimized Python**: ~10-30 seconds for 170 photos (multiprocessing, exiftool calls)
- **Original Python**: ~60-120 seconds for 170 photos (sequential)

## Python Version (deprecated)

The Python version has been removed in favor of the Rust version which also includes the GUI and is much faster.

## License

This project is released under the MIT license.
