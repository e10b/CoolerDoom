//! Packs Cheello's KVX models into one blob the engine embeds.
//!
//! Each record is a little-endian name length, the file stem, a data length,
//! and the `.kvx` bytes. `voxel.rs` slices that blob at runtime.

use std::fs;
use std::path::Path;

fn main() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let voxels = manifest.join("assets/cheello/voxels");
    println!("cargo:rerun-if-changed=assets/cheello/voxels");
    println!("cargo:rerun-if-changed=assets/cheello/VOXELDEF.txt");

    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("voxels.bin");
    let mut blob = Vec::new();
    if voxels.is_dir() {
        let mut names: Vec<_> = fs::read_dir(&voxels)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| s.eq_ignore_ascii_case("kvx"))
            })
            .collect();
        names.sort();
        for path in names {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap()
                .to_ascii_lowercase();
            let bytes = fs::read(&path).unwrap();
            let name = stem.as_bytes();
            blob.extend_from_slice(&(name.len() as u16).to_le_bytes());
            blob.extend_from_slice(name);
            blob.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            blob.extend_from_slice(&bytes);
        }
    }
    fs::write(out, blob).unwrap();
}
