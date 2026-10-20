fn main() {
    tauri_build::build()
}

// This build script is used to configure the Tauri application build process.
// It calls the `tauri_build::build()` function, which handles the necessary steps
// to prepare the application for building, such as generating configuration files
// and setting up the environment.

// The `main` function is the entry point of the build script, and it is executed when the build process starts.
// `main` is located in the `build.rs` file, which is a special file recognized by Cargo (the Rust package manager)
// to perform custom build steps before compiling the main application code.
