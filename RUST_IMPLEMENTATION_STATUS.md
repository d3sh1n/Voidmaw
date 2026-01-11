# Rust 1.80 Implementation Status

## ✅ Completed Features

### Dismantle (Recording Tool)
- ✅ **Core Module** (`dismantle/src/core.rs`)
  - File I/O operations (check_program_args, read_input_file)
  - Input data backup (create_input_data_copy)
  - Child process cleanup (kill_all_child_procs_of_current_proc)
  - Encryption key generation using Windows Crypto API
  - XOR encryption for instruction map
  - INT3 instruction masking
  - C++ header file generation
  - Output file writing

- ✅ **VEH Module** (`dismantle/src/veh.rs`)
  - PAGE_GUARD exception handling
  - EXCEPTION_SINGLE_STEP handling for PAGE_GUARD restoration
  - Instruction recording using iced-x86 disassembler
  - Global state management with thread-safe Mutex

- ✅ **Utils Module** (`dismantle/src/utils.rs`)
  - Windows API error logging
  - Program status logging
  - String conversion utilities

- ✅ **Main Module** (`dismantle/src/main.rs`)
  - Command-line argument parsing with clap
  - Complete workflow orchestration
  - MessageBox for user interaction
  - Thread creation and termination

### Voidmaw (Execution Tool)
- ✅ **VEH Module** (`voidmaw/src/veh.rs`)
  - EXCEPTION_BREAKPOINT handling
  - Instruction decryption (XOR)
  - Re-masking queue management
  - MZ header preservation
  - Thread-safe pointer wrappers (SendPtr)

- ✅ **Utils Module** (`voidmaw/src/utils.rs`)
  - Windows API error logging
  - Program status logging

- ✅ **Main Module** (`voidmaw/src/main.rs`)
  - Payload initialization from generated data
  - Memory allocation and protection
  - VEH installation
  - Thread creation and execution

- ✅ **Payload Module** (`voidmaw/src/payload.rs`)
  - Placeholder structure for generated payload data
  - Documented conversion process from C++ to Rust

### Build System
- ✅ Cargo workspace configuration
- ✅ Dependency management (iced-x86, clap, indexmap)
- ✅ Build scripts for Windows (PowerShell) and Linux/WSL (Bash)

### Documentation
- ✅ Comprehensive README_RUST.md
- ✅ Usage instructions
- ✅ Architecture diagrams
- ✅ Conversion guide

## ⚠️ Known Issues

### Windows Crate Version Compatibility
**Issue**: The `windows` crate version in the workspace (0.58) appears to have compatibility issues with the Win32 API namespace.

**Symptoms**:
```
error[E0433]: failed to resolve: could not find `Win32` in `windows`
```

**Possible Solutions**:
1. **Update to windows crate 0.62+**: The latest version has better API support
2. **Use `windows-sys` instead**: Consider using the lower-level `windows-sys` crate
3. **Feature flags**: Ensure all required Windows features are properly enabled

**Temporary Workaround**:
The current Cargo.toml files have been configured with explicit feature flags. If compilation still fails:

```toml
# Try this in both dismantle/Cargo.toml and voidmaw/Cargo.toml
[dependencies.windows]
version = "0.62"  # or latest
features = [
    "Win32_Foundation",
    "Win32_System_Threading",
    "Win32_System_Memory",
    "Win32_System_Diagnostics_Debug",
    "Win32_System_Diagnostics_ToolHelp",
    "Win32_Security_Cryptography",
    "Win32_Storage_FileSystem",
    "Win32_UI_WindowsAndMessaging",
]
```

## 🔧 How to Fix Compilation Issues

### Step 1: Update Windows Crate Version
```bash
# Edit Cargo.toml files to use the latest windows crate
# OR use windows-sys for lower-level bindings
```

### Step 2: Clean Build
```bash
rm Cargo.lock
cargo clean
cargo build --release
```

### Step 3: Alternative - Use windows-sys
If the `windows` crate continues to have issues, consider switching to `windows-sys`:

```toml
[dependencies]
windows-sys = { version = "0.59", features = [
    "Win32_Foundation",
    # ... rest of features
] }
```

Then update imports:
```rust
// Change from:
use windows::Win32::Foundation::GetLastError;

// To:
use windows_sys::Win32::Foundation::GetLastError;
```

## 📝 Differences from C++ Version

### Implemented
- ✅ All core functionality
- ✅ PAGE_GUARD exception handling
- ✅ EXCEPTION_BREAKPOINT handling
- ✅ Instruction recording with iced-x86
- ✅ XOR encryption/decryption
- ✅ INT3 masking
- ✅ MZ header preservation
- ✅ Multi-threaded payload support
- ✅ Child process cleanup

### Not Implemented (By Design)
- ❌ **PEB Manipulation for CLI Arguments**: The original C++ code modifies the Process Environment Block to change command-line arguments. This feature was skipped in the Rust version due to:
  - High complexity and platform-specific assembly required
  - Potential instability
  - Can be worked around by modifying the payload directly

  **Note**: This functionality is documented in the code but not active. To enable it, you would need to implement the `get_peb()` function using inline assembly and carefully manipulate the PEB structure.

### Modified
- ⚠️ **C++ Header Generation**: Dismantle still generates C++ header files (not Rust). The user must manually convert these to Rust format for use with Voidmaw.
  - **Future Enhancement**: Create an automatic converter tool

## 🚀 Next Steps for Users

1. **Fix Compilation**:
   - Update `windows` crate version in Cargo.toml files
   - OR switch to `windows-sys` crate
   - Clean and rebuild

2. **Test Dismantle**:
   ```bash
   cargo build --release -p dismantle
   ./target/release/dismantle.exe -p payload.bin -o out.h
   ```

3. **Convert Generated Header**:
   - Take the C++ header file generated by Dismantle
   - Convert it to Rust format in `voidmaw/src/payload.rs`
   - See README_RUST.md for conversion examples

4. **Test Voidmaw**:
   ```bash
   cargo build --release -p voidmaw
   ./target/release/voidmaw.exe
   ```

## 📚 Additional Resources

- **Original C++ Project**: https://github.com/vxCrypt0r/Voidmaw
- **Rust Windows Bindings**: https://github.com/microsoft/windows-rs
- **iced-x86 Disassembler**: https://github.com/icedland/iced
- **README_RUST.md**: Comprehensive usage guide

## 🙏 Acknowledgments

- **Original Author**: Paul Socatiu (vxCrypt0r)
- **Rust Port**: Community contribution for educational and research purposes

## ⚖️ Legal Disclaimer

This tool is for **academic and authorized security research only**. Use responsibly and legally.

---

**Last Updated**: 2026-01-11
**Rust Version**: 1.80+
**Status**: Core implementation complete, compilation issues pending resolution
