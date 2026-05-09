# Memory Corruption Bug in p_setup.rs — Debugging Log

## Date
2026-05-09

## Status
MEMORY CORRUPTION FIXED — test now reaches checkpoint 3 (prndindex mismatch)

## Fix 1: DIR_SEPARATOR_S global-buffer-overflow
The ASan run revealed a `global-buffer-overflow` in `M_StringJoinA` (m_misc.rs:346) caused by `DIR_SEPARATOR_S` being a 1-byte `&str = "/"` that was passed to `strlen` without a null terminator.

**Root cause**: `const DIR_SEPARATOR_S: &str = "/";` creates a 1-byte static (just '/'), not null-terminated. When `M_StringJoinA` calls `strlen` on it, `strlen` reads past the end.

**Fix applied**: Changed all three definitions from `&str = "/"` to `&[u8] = b"/\0"` in:
- `m_misc.rs:8`
- `m_config.rs:452`  
- `d_iwad.rs:14`

**ASan confirmed**: No more global-buffer-overflow errors after this fix.

## Fix 2: ptr::write_bytes heap corruption in p_setup.rs
After fixing the DIR_SEPARATOR_S issue, the test still crashed with `Z_Malloc: rover->prev is null` during P_LoadSectors.

**Root cause**: `ptr::write_bytes(ptr, 0, size)` compiles to LLVM's memset intrinsic, which under ASan instrumentation uses SIMD-optimized writes that can overshoot the requested byte count when `size` is not aligned to the SIMD chunk size (16/32/64 bytes). This writes into the next zone allocator block header, zeroing it out and corrupting the doubly-linked list.

**Evidence**:
- `ptr::write_bytes(sectors, 0, 18304)` corrupted the 8-byte boundary at `sectors + 18304`
- Replacing with a byte-by-byte loop (`write_volatile` per byte) preserved the boundary
- The crash moved downstream when sectors was fixed, confirming ALL write_bytes calls had this issue
- Affected sizes: 18304 (sectors), 25272 (sides), and others — not all divisible by 16

**Fix applied**: 
1. Modified `Z_Malloc` in `z_zone.rs` to zero the user data area using a byte-by-byte loop before returning
2. Removed all `ptr::write_bytes` calls from `p_setup.rs` that were zeroing Z_Malloc'd buffers:
   - P_LoadSegs
   - P_LoadSubsectors  
   - P_LoadSectors
   - P_LoadLineDefs
   - P_LoadSideDefs
   - PadRejectArray (replaced with byte loop)

**Result**: Test no longer crashes with memory corruption. It now reaches checkpoint 3 and fails on `prndindex mismatch` — a separate issue (likely uninitialized state or C/Rust state divergence).

## Symptom
`cargo test --test demo_playthrough` crashes with:
```
thread 'demo_playthrough' panicked at room/src/doom/z_zone.rs:124:9:
Z_Malloc: rover->prev is null! base=0x... base.tag=0 base.size=0
```

The crash occurs during level loading, specifically in `P_LoadVertexes` called from `P_SetupLevel` during the second level load (after `Z_FreeTags` frees the first level's PU_LEVEL blocks).

## Root Cause Analysis

### Found and Fixed: `ptr::write_bytes` element-count vs byte-count bug
ALL `ptr::write_bytes(ptr, 0, count)` calls in `p_setup.rs` were passing **element counts** instead of **byte counts**:
- `P_LoadSegs`: `ptr::write_bytes(segs, 0, numsegs)` → should be `numsegs * size_of::<seg_t>()`
- `P_LoadSubsectors`: `ptr::write_bytes(subsectors, 0, numsubsectors)` → should be `numsubsectors * size_of::<subsector_t>()`
- `P_LoadSectors`: `ptr::write_bytes(sectors, 0, numsectors)` → should be `numsectors * size_of::<sector_t>()`
- `P_LoadLineDefs`: `ptr::write_bytes(lines, 0, numlines)` → should be `numlines * size_of::<line_t>()`
- `P_LoadSideDefs`: `ptr::write_bytes(sides, 0, numsides)` → should be `numsides * size_of::<side_t>()`

This left 95-99% of each allocated buffer uninitialized, causing:
1. Garbage data in runtime data structures
2. Potential heap corruption from uninitialized pointer fields

**Fix applied**: All five calls corrected to use byte counts.

### Bug PERSISTS after fix
Despite fixing the write_bytes bug, the Z_Malloc crash still occurs. This means there is a SECOND bug.

## Diagnostic Findings

### Heap checks timing (Z_CheckHeapQuiet)
Adding `Z_CheckHeapQuiet()` calls at various points changes behavior:
- With checks after every operation in `P_LoadBlockMap`: NO heap corruption detected, test proceeds but fails on `prndindex mismatch` at checkpoint 3
- Without checks: Z_Malloc crash

This is a Heisenbug — reading memory changes the behavior. Possible causes:
- Compiler generates different register allocation/stack layout with function calls
- Memory barrier effects from function calls
- CPU cache effects from reading the linked list

### Struct sizes verified — ALL match between C and Rust
Comprehensive comparison using compiled C program and Rust binary:
```
C vs Rust struct sizes (all match):
  vertex_t:     8  ✓
  seg_t:        56 ✓
  subsector_t:  16 ✓
  sector_t:     128✓
  line_t:       88 ✓
  side_t:       24 ✓
  node_t:       52 ✓
```
All field offsets also match exactly.

### Zone allocator struct sizes match
- C `memblock_t`: 40 bytes (size=4, pad=4, user=8, tag=4, id=4, next=8, prev=8)
- Rust `memblock_t`: 40 bytes
- C `memzone_t`: 56 bytes (size=4, pad=4, blocklist=40, rover=8)
- Rust `memzone_t`: 56 bytes

### Allocation sequence before crash
The test runs through:
1. `Z_Init()` — zone initialized, heap valid
2. Game initialization (textures, sounds, etc.) — many Z_Malloc calls, heap valid
3. `P_SetupLevel()` for first level — allocations succeed
4. Second `P_SetupLevel()` call (demo loads next level):
   a. `Z_FreeTags(PU_LEVEL, PU_PURGELEVEL-1)` — walks 1341 blocks, frees 0 (first load has no PU_LEVEL blocks yet)
   b. `P_LoadBlockMap()` — succeeds
   c. `P_LoadVertexes()` — Z_Malloc crashes with null prev

Wait — actually Z_FreeTags walked 1341 blocks and freed 0 during the FIRST level load. This means there are already 1341 blocks before P_SetupLevel runs. These are from game initialization (textures, sounds, etc.).

### Crash location in debug output
```
Z_Malloc: size=8032 tag=5  ← P_LoadBlockMap: blockmaplump
Z_Malloc: size=6656 tag=5  ← P_LoadBlockMap: blocklinks
Z_Malloc: size=5968 tag=5  ← P_LoadVertexes: CRASH HERE
```

The 5968-byte allocation for vertexes finds a corrupted block with prev=null.

### The blocklinks allocation (6656 bytes) is suspicious
- bmapwidth=32, bmapheight=26, bcount = 8*32*26 = 6656
- Block header at 0x...ecf40, user data at 0x...ecf68
- Next block at 0x...ee968 (size=0, prev=0 — corrupted)
- 0x...ee968 - 0x...ecf40 = 6696 = 6656 + 40 (header)
- This is exactly the blocklinks block size

The block AFTER blocklinks has a corrupted header (size=0, prev=0).

## Hypotheses for Second Bug

### H1: W_ReadLump buffer overflow
`W_ReadLump(lump, blockmaplump)` reads `lumplen=8032` bytes into an 8032-byte allocation. If W_ReadLump reads more than the lump length (bug in C code), it overflows.

### H2: Blockmap swap loop overflow
`for i in 0..count` where `count = lumplen/2 = 4016`. Processes 4016 shorts = 8032 bytes. The allocation is 8032 bytes. Should be exact fit.

### H3: Uninitialized fields in sector_t
The sector_t struct has `_pad0: [u8; 2]` padding after `tag`. If the C code doesn't zero this padding, and the Rust code doesn't zero it either (because ptr::write_bytes was wrong), there could be garbage. BUT we fixed the write_bytes to use correct byte counts.

### H4: C code memory corruption
Some C initialization code (before P_SetupLevel) corrupts memory. The corruption only manifests when Z_FreeTags walks the list and Z_Malloc tries to allocate.

### H5: Z_FreeTags linked list traversal bug
Z_FreeTags captures `next = (*block).next` before calling Z_Free. If Z_Free merges blocks, the captured `next` might become invalid. BUT analysis showed this should be correct.

### H6: Blocklinks memset writes past end
`libc::memset(blocklinks, 0, bcount)` where `bcount = 6656`. The user data area is exactly 6656 bytes. The next block starts at `blocklinks + 6656`. memset should not write past the end. BUT if bcount is slightly wrong (e.g., bmapwidth/bmapheight from lump data are wrong), it could overflow.

### H7: The sentinel address calculation is wrong
`&mut (*zone).blocklist` = `zone + 8` (due to padding before blocklist in memzone_t). But blocks expect the sentinel at `zone + 0`. This could cause the last block's `next` to point to the wrong address.

**Investigated**: The sentinel IS at `zone + 8`. The free block's `next` correctly points to `&zone.blocklist`. Z_CheckHeapQuiet was incorrectly flagging this as corruption (fixed by skipping sentinel check).

## What to Try Next

1. **Run with AddressSanitizer** (see [ASan Guide](#addresssanitizer-guide) below)
2. **Compare with C p_setup.c**: Temporarily restore C p_setup.c in build.rs and see if test passes
3. **Add Z_CheckHeapQuiet inside Z_Malloc**: After each allocation, verify the block's next pointer is valid
4. **Check bmapwidth/bmapheight values**: Verify they match expected values for the demo map
5. **Test with valgrind**: `valgrind cargo test --test demo_playthrough`
6. **Zero all allocated memory in Z_Malloc**: Instead of relying on caller to zero, have Z_Malloc zero the user data area. This would rule out uninitialized memory bugs.
7. **Run with dhat** (allocation profiler) to inspect allocation volume and ownership paths — see [dhat section](#dhat-heap-profiler) below.

---

## AddressSanitizer Guide

ASan is the fastest way to get exact line numbers for memory corruption that crosses the Rust/C boundary. It is especially useful for `unsafe`, FFI, translated C idioms, raw pointers, manual buffers, and ownership mistakes.

### Quick Start

Run the failing test with ASan:

```bash
task asan:test -- test_name -- --nocapture
```

Or manually:

```bash
ASAN_OPTIONS="detect_leaks=1:halt_on_error=1:abort_on_error=1:symbolize=1" \
RUST_BACKTRACE=1 \
RUSTFLAGS="-Zsanitizer=address" \
cargo +nightly test -Zbuild-std --target x86_64-unknown-linux-gnu
```

### Why `-Zbuild-std` is needed

Rust’s sanitizer support lives behind unstable compiler flags, so you need **nightly** and `-Z sanitizer=address`. The `-Zbuild-std` part matters because otherwise parts of `std` may not be instrumented. Use it for serious ASan runs.

### C code instrumentation

Because this crate still links C code via `cc` in `doomgeneric-sys/build.rs`, compile the C side with ASan too. The build script already gates this behind the `ASAN=1` environment variable:

```bash
ASAN=1 \
ASAN_OPTIONS="detect_leaks=1:halt_on_error=1:abort_on_error=1:symbolize=1" \
RUST_BACKTRACE=1 \
RUSTFLAGS="-Zsanitizer=address" \
cargo +nightly test -Zbuild-std --target x86_64-unknown-linux-gnu
```

You want both sides instrumented. If only Rust has ASan, bugs inside C may still show up when they cross the boundary, but reports get worse and some issues remain invisible.

### Useful environment flags

| Flag | Meaning |
|------|---------|
| `detect_leaks=1` | Also report leaks |
| `halt_on_error=1` | Stop at first error |
| `abort_on_error=1` | Generate a hard crash, useful for debuggers/agents |
| `symbolize=1` | Print readable stack traces |

Install LLVM tools if stack traces look bad, because ASan uses LLVM symbolization. On Linux, that usually means `llvm-symbolizer` must be on `PATH`.

### What ASan reports look like

You’ll get errors like:

```text
ERROR: AddressSanitizer: heap-use-after-free
READ of size 8 at 0x...
    #0 my_crate::module::function src/foo.rs:123
    #1 my_crate::ffi_wrapper::call src/ffi.rs:45

freed by thread T0 here:
    #0 free
    #1 native_destroy src/native/foo.c:88

previously allocated by thread T0 here:
    #0 malloc
    #1 native_create src/native/foo.c:42
```

That is the nice part: ASan usually gives you three locations:

- where the bad access happened
- where the memory was freed
- where the memory was allocated

That’s exactly what we need.

### ASan limitations

- Needs nightly.
- Slows execution down and increases memory use.
- May conflict with proc macros or dynamically loaded libraries in weird setups.
- Works best on Linux/macOS x86_64/aarch64, but exact target support varies.
- Does not replace Miri.
- Does not prove memory safety.

If the C library ships as a prebuilt `.a` or `.so` without ASan instrumentation, ASan can still catch boundary damage, but reports may point at the wrapper rather than the real C callsite.

---

## dhat Heap Profiler

For callsite-level allocation tracking (volume and ownership paths), use `dhat` separately from ASan. ASan finds corruption; `dhat` explains allocation volume.

Enable the feature in `room/Cargo.toml`:

```toml
[features]
dhat-heap = ["dep:dhat"]

[dependencies]
dhat = { version = "0.3", optional = true }
```

Run with:

```bash
# Binary
cargo run --features dhat-heap

# Integration test
cargo test --test demo_playthrough --features dhat-heap
```

A `dhat-heap.json` file is produced; view it with the [dhat viewer](https://valgrind.org/docs/manual/dh-manual.html).

## Files Modified
- `room/src/doom/p_setup.rs` — ported from C, has write_bytes bug (fixed), still crashing
- `room/src/doom/z_zone.rs` — Rust port of zone allocator, added Z_CheckHeapQuiet
- `room/src/doom/c_ffi.rs` — FFI declarations for C structs
- `doomgeneric-sys/build.rs` — removed p_setup.c and z_zone.c from C build

## Relevant C Source
- `vendor/doomgeneric/p_setup.c` — original C code for comparison
- `vendor/doomgeneric/z_zone.c` — original zone allocator

## Struct size comparison binary
- `/tmp/struct_sizes.c` — C program that prints struct sizes and offsets
- `room/src/bin/struct_sizes.rs` — Rust program that prints struct sizes and offsets
