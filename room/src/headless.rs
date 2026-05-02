use std::cell::Cell;

thread_local! {
    static VIRTUAL_MS: Cell<u32> = const { Cell::new(0) };
    static FRAMES: Cell<u64> = const { Cell::new(0) };
    static TITLE: Cell<Option<&'static str>> = const { Cell::new(None) };
}

pub const TICK_MS: u32 = 1000 / 35;

pub fn bump_virtual_ms(delta: u32) {
    VIRTUAL_MS.with(|v| v.set(v.get() + delta));
}

pub fn virtual_ms() -> u32 {
    VIRTUAL_MS.with(|v| v.get())
}

pub fn note_frame() {
    FRAMES.with(|f| f.set(f.get() + 1));
}

pub fn frame_count() -> u64 {
    FRAMES.with(|f| f.get())
}

pub fn note_title(s: &'static str) {
    TITLE.with(|t| t.set(Some(s)));
}

pub fn title() -> Option<&'static str> {
    TITLE.with(|t| t.get())
}
