# Building PortableSource

This guide explains how to build PortableSource into a distributable Windows installer.

## Prerequisites

1. **Node.js** (v18 or later)
2. **Rust** (latest stable version)
3. **Visual Studio Build Tools** or **Visual Studio Community** with C++ development tools
4. **Windows SDK**

### Installing Rust
```bash
# Install Rust via rustup
winget install Rustlang.Rustup
# or download from https://rustup.rs/
```

### Installing Visual Studio Build Tools
```bash
# Install via winget
winget install Microsoft.VisualStudio.2022.BuildTools
```

Make sure to include:
- MSVC v143 compiler toolset
- Windows 10/11 SDK
- CMake tools for Visual Studio

## Building Steps

### 1. Install Dependencies
```bash
npm install
```

### 2. Build for Development (Optional)
To test the app before building the installer:
```bash
npm run tauri:dev
```

### 3. Build Production Installer
```bash
npm run tauri:build
```

This command will:
1. Build the Svelte frontend
2. Compile the Rust backend
3. Create Windows installer files

### 4. Find Your Installer

After successful build, you'll find the installer files in:
```
src-tauri/target/release/bundle/
├── msi/           # Windows MSI installer
└── nsis/          # NSIS installer (if configured)
```

## Installer Types

The configuration supports two types of Windows installers:

### MSI Installer (Recommended)
- **Location**: `src-tauri/target/release/bundle/msi/PortableSource_0.1.0_x64_en-US.msi`
- **Features**: Standard Windows installer with proper uninstall support
- **Best for**: Distribution to end users

### NSIS Installer
- **Location**: `src-tauri/target/release/bundle/nsis/PortableSource_0.1.0_x64-setup.exe`
- **Features**: Customizable installer with more options
- **Best for**: Advanced distribution scenarios

## Troubleshooting

### Build Fails with "linker not found"
- Install Visual Studio Build Tools with C++ development tools
- Restart your terminal after installation

### "Windows SDK not found"
- Install Windows 10/11 SDK via Visual Studio Installer
- Set environment variable: `set WINDOWS_SDK_DIR=C:\Program Files (x86)\Windows Kits\10`

### "npm run tauri:build" fails
- Make sure all dependencies are installed: `npm install`
- Try building in debug mode first: `npm run tauri:build:debug`
- Check that Rust is properly installed: `rustc --version`

### Large Installer Size
- The installer includes the Rust runtime and all dependencies
- Typical size: 10-20 MB for the base app
- This is normal for Tauri applications

## Distribution

Once built, you can distribute the `.msi` or `.exe` file to users. The installer will:

1. Install the application to `Program Files` (or user directory)
2. Create desktop and start menu shortcuts
3. Register the application for proper uninstall
4. Set up file associations (if configured)

## Code Signing (Optional)

For production distribution, consider code signing your installer:

1. Obtain a code signing certificate
2. Update `tauri.conf.json` with certificate details:
```json
"windows": {
  "certificateThumbprint": "YOUR_CERT_THUMBPRINT",
  "timestampUrl": "http://timestamp.digicert.com"
}
```

## Build Optimization

### Release Build (Smaller Size)
```bash
npm run tauri:build
```

### Debug Build (Faster Build Time)
```bash
npm run tauri:build:debug
```

### Custom Build Target
```bash
npx tauri build --target x86_64-pc-windows-msvc
```

## Next Steps

After building:
1. Test the installer on a clean Windows machine
2. Verify all features work correctly
3. Consider setting up automated builds with GitHub Actions
4. Plan your distribution strategy (website, GitHub releases, etc.)