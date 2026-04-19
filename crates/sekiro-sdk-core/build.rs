//! Build script: locate `MinHook.x64.lib` on the link path.
//!
//! Only runs when the `minhook` feature is enabled.  Honours these
//! environment variables (first hit wins):
//!
//!   `MINHOOK_LIB_DIR`    — absolute path to a directory containing
//!                          `MinHook.x64.lib` (Windows)
//!   `VENDOR_MINHOOK_DIR` — path to a MinHook source checkout; build
//!                          script invokes `cl` / `cmake` (unimplemented)
//!
//! If neither is set and the feature is enabled, we emit a friendly
//! diagnostic pointing at the README rather than a silent
//! "library not found" error at link time.

fn main() {
    #[cfg(feature = "minhook")]
    locate_minhook();
}

#[cfg(feature = "minhook")]
fn locate_minhook() {
    use std::env;
    use std::path::PathBuf;

    println!("cargo:rerun-if-env-changed=MINHOOK_LIB_DIR");
    println!("cargo:rerun-if-env-changed=VENDOR_MINHOOK_DIR");

    if let Ok(dir) = env::var("MINHOOK_LIB_DIR") {
        let p = PathBuf::from(&dir);
        if !p.join("MinHook.x64.lib").is_file() {
            eprintln!(
                "warning: MINHOOK_LIB_DIR is set to {} but MinHook.x64.lib is not present there",
                dir
            );
        }
        println!("cargo:rustc-link-search=native={}", p.display());
        return;
    }

    if env::var_os("VENDOR_MINHOOK_DIR").is_some() {
        eprintln!(
            "note: VENDOR_MINHOOK_DIR is set but source-build is not yet implemented; \
             falling back to system search"
        );
    }

    // Best-effort: check the workspace `vendor/lib` directory first,
    // then a couple of common Windows install locations.
    let workspace_vendor = env::var("CARGO_MANIFEST_DIR")
        .ok()
        .and_then(|dir| {
            let p = PathBuf::from(dir)
                .parent()
                .and_then(|p| p.parent())
                .map(|p| p.join("vendor").join("lib"));
            p
        });
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(p) = workspace_vendor {
        candidates.push(p);
    }
    #[cfg(target_os = "windows")]
    {
        candidates.push(PathBuf::from("C:\\Program Files\\MinHook\\lib"));
        candidates.push(PathBuf::from("C:\\MinHook\\lib"));
    }
    for p in candidates {
        if p.join("MinHook.x64.lib").is_file() {
            println!("cargo:rustc-link-search=native={}", p.display());
            return;
        }
    }

    println!(
        "cargo:warning=MinHook.x64.lib was not located. \
         Set MINHOOK_LIB_DIR to the directory containing it, or see \
         docs/INSTALL.md for download instructions."
    );
}
