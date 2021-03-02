#[allow(
    clippy::all,
    non_camel_case_types,
    non_upper_case_globals,
    non_snake_case,
    improper_ctypes
)]
mod bindings {
    use crate::c_types;
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}
pub use bindings::*;

extern "C" {
    pub fn init_waitqueue_head_helper(h: *mut wait_queue_head_t);
    pub fn wake_up_interruptible_helper(h: *mut wait_queue_head_t);
}

pub const GFP_KERNEL: gfp_t = BINDINGS_GFP_KERNEL;
