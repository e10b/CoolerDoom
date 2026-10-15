//! Time the E1M1 octree on the native engine. The browser build was stalling
//! on the first hardware frame, before it could print the octree log.
//!
//!   cargo test --release -p room --test svo_e1m1 -- --nocapture

use std::ffi::{c_char, c_int, CString};
use std::path::PathBuf;

fn frame_nodes(frame: &room::hw::SvoFrame) -> usize {
    if let Some(nodes) = &frame.nodes {
        return nodes.len();
    }
    frame.tree.as_ref().map(|tree| tree.nodes().len()).unwrap_or(0)
}

fn find_wad() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .join("../doom1.wad")
        .canonicalize()
        .unwrap_or_else(|_| manifest.join("../doom1.wad"))
}

#[test]
fn e1m1_octree_builds() {
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .try_init();
    let wad = find_wad();
    assert!(wad.exists(), "doom1.wad missing at {}", wad.display());
    let wad_str = wad.to_str().expect("wad path");
    let argv = [
        CString::new("room").unwrap(),
        CString::new("-iwad").unwrap(),
        CString::new(wad_str).unwrap(),
        CString::new("-warp").unwrap(),
        CString::new("1").unwrap(),
        CString::new("1").unwrap(),
        CString::new("-nosound").unwrap(),
        CString::new("-nomusic").unwrap(),
        CString::new("-nograbmouse").unwrap(),
    ];
    let mut c_argv: Vec<*mut c_char> = argv.iter().map(|s| s.as_ptr() as *mut c_char).collect();
    room::platform::init_start_time();
    unsafe {
        room::doom::d_loop::singletics = 1;
        doomgeneric_sys::doomgeneric_Create(argv.len() as c_int, c_argv.as_mut_ptr());
        for _ in 0..120 {
            doomgeneric_sys::doomgeneric_Tick();
            if room::doom::g_game::gamestate == 0 && room::doom::p_setup::numsubsectors > 0 {
                break;
            }
        }
        assert_eq!(room::doom::g_game::gamestate, 0, "never reached the level");
        assert!(room::doom::p_setup::numsubsectors > 0);
        let started = std::time::Instant::now();
        let mut scene = room::hw::build_scene();
        let ms = started.elapsed().as_millis();
        let nodes = scene.svo.as_ref().map(frame_nodes).unwrap_or(0);
        eprintln!("e1m1 octree: {ms} ms, nodes={nodes}");
        assert!(scene.svo.is_some(), "octree was not built");
        assert!(nodes < 8_000_000, "octree has {nodes} nodes");
        // Texel cells are one map unit: E1M1 is about 58 million of them,
        // which takes a few seconds once per map, not once per frame.
        assert!(ms < 8_000, "octree build took {ms} ms");
        let frame = scene.svo.take().expect("octree frame");
        let mut tree = frame.tree.expect("the test keeps the octree");
        (*room::doom::p_setup::sectors).ceilingheight += 64 * 65536;
        let mut scene2 = room::hw::build_scene();
        let patch = scene2
            .svo
            .as_mut()
            .and_then(|frame| frame.patch.take())
            .expect("a moved ceiling should splice texels");
        let door = std::time::Instant::now();
        tree.edit(&patch.remove, &patch.add);
        tree.prime(patch.eye);
        let door_ms = door.elapsed().as_millis();
        eprintln!(
            "door splice: {door_ms} ms, remove {}, add {}",
            patch.remove.len(),
            patch.add.len()
        );
        assert!(
            !patch.remove.is_empty() || !patch.add.is_empty(),
            "sector 0 produced no texels"
        );
        assert!(door_ms < 150, "door splice took {door_ms} ms");
        room::hw::recycle(scene2);
        room::hw::recycle(scene);
    }
}
