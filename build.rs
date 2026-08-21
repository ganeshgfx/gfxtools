// build.rs
// Sets the Windows subsystem to "windows" in release builds so no console
// window flashes when launched from Explorer. The application allocates its
// own console at runtime when progress output is needed.
//
// Emits the correct linker flag for both MSVC and GNU (MinGW) toolchains.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        // Only in release — keep console visible in debug/test builds.
        if std::env::var("PROFILE").as_deref() == Ok("release") {
            let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
            if target_env == "msvc" {
                // MSVC linker flags
                println!("cargo:rustc-link-arg=/SUBSYSTEM:WINDOWS");
                println!("cargo:rustc-link-arg=/ENTRY:mainCRTStartup");
            } else {
                // GNU (MinGW) linker flags
                println!("cargo:rustc-link-arg=-mwindows");
            }
        }
    }
}
