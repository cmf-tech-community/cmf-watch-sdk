use core::ffi::c_void;
use wp2_core::heap::{self, RES};
use wp2_core::layout::{stub_ram, StubRam, E_NOMEM, LOADER_SLOT};
use wp2_core::load::load;

const PRELOAD: u32 = 2;
const DELETE: u32 = 4;
const REFUSE: i32 = -2;

type Proc = extern "C" fn(u32, u32, *mut c_void) -> i32;

pub extern "C" fn wp2_view_proc(view_id: u32, msg: u32, data: *mut c_void) -> i32 {
    let ram = stub_ram();
    ram.trace[ram.trace_head as usize % ram.trace.len()] = (msg << 16) | view_id;
    ram.trace_head = ram.trace_head.wrapping_add(1);
    if msg == PRELOAD {
        if ram.loader_state == 1 {
            // a second PRELOAD without DELETE: let the old instance free its framebuffer first
            loader(ram)(view_id, DELETE, core::ptr::null_mut());
        }
        release(ram);
        match load(LOADER_SLOT, &mut ram.load_file, &mut ram.load_scratch) {
            Ok((block, h)) => {
                ram.loader_block = block as u32;
                ram.loader_size = h.image_size;
                ram.loader_entry = h.entry;
                ram.loader_state = 1;
            }
            Err(e) => ram.loader_state = if e == E_NOMEM { 3 } else { 2 },
        }
    }
    if ram.loader_state != 1 {
        ram.launch = 0; // a widget tap must not start its app on a later, unrelated menu open
        return REFUSE;
    }
    ram.loader_active = 1;
    let rc = loader(ram)(view_id, msg, data);
    if msg == PRELOAD && rc < 0 {
        release(ram);
        ram.loader_state = 3;
        return REFUSE;
    }
    if msg == DELETE {
        release(ram);
        ram.loader_state = 0;
    }
    rc
}

fn loader(ram: &StubRam) -> Proc {
    unsafe { core::mem::transmute(((ram.loader_block + ram.loader_entry) | 1) as usize) }
}

fn release(ram: &mut StubRam) {
    unsafe { heap::free(RES, ram.loader_block as *mut u8) };
    ram.loader_block = 0;
    ram.loader_active = 0;
}
