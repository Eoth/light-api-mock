//! Embeds the built UI (`frontend/dist`) in the binary, so that one file is a complete Mimicway.
//!
//! The only build step of the crate. It lists the files of `frontend/dist` and writes, into `OUT_DIR`, a table of
//! `include_bytes!` entries; nothing else is generated, downloaded or run. When the UI has not been built (Rust-only
//! work, the Rust CI job), the table is empty and the server reads the UI from a directory, as it always did.
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let dist = Path::new("frontend").join("dist");
    // A directory is scanned for changes as a whole: rebuilding the UI re-embeds it.
    println!("cargo:rerun-if-changed=frontend/dist");

    let mut files = Vec::new();
    if dist.join("index.html").is_file() {
        collect(&dist, &dist, &mut files);
    }
    // Sorted by path, so that the server can binary-search the table.
    files.sort();

    let mut table = String::from(
        "/// The built UI as (path, content) pairs, sorted by path; empty when the UI was not built.\n",
    );
    table.push_str("pub static EMBEDDED_UI: &[(&str, &[u8])] = &[\n");
    for (relative, absolute) in &files {
        table.push_str(&format!(
            "    ({relative:?}, include_bytes!({absolute:?})),\n"
        ));
    }
    table.push_str("];\n");

    let out_dir =
        PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR for build scripts"));
    fs::write(out_dir.join("embedded_ui.rs"), table).expect("OUT_DIR is writable");
}

/// Every file under `dir`, as (path relative to `root` with `/` separators, absolute path).
fn collect(root: &Path, dir: &Path, files: &mut Vec<(String, String)>) {
    let entries = fs::read_dir(dir).expect("frontend/dist is readable");
    for entry in entries {
        let path = entry.expect("frontend/dist is readable").path();
        if path.is_dir() {
            collect(root, &path, files);
        } else {
            let relative = path
                .strip_prefix(root)
                .expect("a file of frontend/dist is under it")
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            let absolute = fs::canonicalize(&path).expect("a listed file exists");
            files.push((relative, absolute.to_string_lossy().into_owned()));
        }
    }
}
