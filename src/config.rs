use std::sync::atomic::{AtomicBool, Ordering};

pub const FRAMES_MAX: usize = 64;
pub const STACK_MAX: usize = FRAMES_MAX * 256;

static STD_MODE: AtomicBool = AtomicBool::new(false);

pub fn is_std_mode() -> bool {
    STD_MODE.load(Ordering::Relaxed)
}

pub fn set_std_mode(val: bool) {
    STD_MODE.store(val, Ordering::Relaxed);
}
