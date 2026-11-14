use crate::chan::Ring;

pub const VIEW_ID: u16 = 0xF0;
pub const SLOTS: u8 = 8;
pub const LOADER_SLOT: u8 = 0xFF;
pub const LOADER_ABI_MINOR: u16 = 3;
// the stub's own release, bumped with every stub change that goes out by OTA. A host reads it with INFO for
// STUB_SLOT (docs/abi-1.4.md, "Stub version"): the watch reports 1.0.0.73 whatever app.bin it runs, so this is the
// only way to tell one patched firmware from another. 1 = every stub before the field was readable. Not the stub
// header's `version`: that one is the header format, which `wp2apps firmware` checks is 1.
// 3 = the Apps widget in wgt_reg slot 0x40 (docs/recon.md, "Widget cards")
pub const STUB_VERSION: u32 = 3;
// INFO for this slot answers the stub's version instead of a package header; an older stub answers BadRequest
pub const STUB_SLOT: u8 = 0xFE;
pub const STUB_RAM: u32 = 0x3871_EEC0;
pub const STUB_RAM_SIZE: u32 = 0x1000;
pub const LOADER_SIZE: u32 = 0x1_0000;
pub const APP_SIZE: u32 = 0x2_0000;
pub const E_BAD: i32 = -2;
pub const E_IO: i32 = -5;
pub const E_NOMEM: i32 = -12;

// /NAND:A..K are read-only mounts; /NAND: is the writable FAT (docs/recon.md)
pub const DIR: &[u8] = b"/NAND:/wp2apps\0";

pub fn slot_path(slot: u8, tmp: bool) -> [u8; 24] {
    file_path(slot, if tmp { b"tmp" } else { b"pkg" })
}

// the app's data file (docs/abi-1.1.md, Storage): N.dat, and N.new while a save is in flight
pub fn data_path(slot: u8, new: bool) -> [u8; 24] {
    file_path(slot, if new { b"new" } else { b"dat" })
}

fn file_path(slot: u8, ext: &[u8; 3]) -> [u8; 24] {
    let mut path = [0u8; 24];
    let name = if slot == LOADER_SLOT { b'L' } else { b'0' + slot % 10 };
    let dir = &DIR[..DIR.len() - 1];
    path[..dir.len()].copy_from_slice(dir);
    path[dir.len()] = b'/';
    path[dir.len() + 1] = name;
    path[dir.len() + 2] = b'.';
    path[dir.len() + 3..dir.len() + 6].copy_from_slice(ext);
    path
}

// loader_state: 0 none, 1 loaded, 2 missing or corrupt, 3 out of memory
#[repr(C)]
pub struct StubRam {
    pub magic: u32,
    pub loader_state: u8,
    pub loader_active: u8,
    pub running_slot: u8,
    pub xfer_state: u8,
    pub xfer_slot: u8,
    // 1: this stub fills the ABI 1.1 fields below (an older stub leaves 0 and never touches them)
    pub api: u8,
    // written by the loader: APP_V11, APP_PAUSED
    pub app_flags: u8,
    pub _pad: u8,
    pub xfer_size: u32,
    pub xfer_crc: u32,
    pub xfer_written: u32,
    pub file: [u8; 0x2C],
    pub trace_head: u32,
    pub trace: [u32; 32],
    pub scratch: [u8; 0x200],
    pub reply: [u8; 0x220],
    pub loader_block: u32,
    pub loader_size: u32,
    pub app_block: u32,
    pub app_size: u32,
    pub alloc_limit: u32,
    pub loader_entry: u32,
    // the loader load in view 0xF0 has its own file and scratch: `file`/`scratch` belong to the BLE transfer
    pub load_file: [u8; 0x2C],
    pub load_scratch: [u8; 0x200],
    // ABI 1.1 channel: first word of the session key at the last authenticated wp2 frame, 0 = none
    pub host_key: u32,
    pub out_seq: u8,
    pub inbox: Ring,
    pub outbox: Ring,
    // own channel (docs/abi-1.4.md): 1 = the host on this link uses K
    pub chan_mode: u8,
    // 1 = chan_key holds K, read once from NVRAM APP_SECRET
    pub chan_ready: u8,
    pub chan_key: [u8; 16],
    // KICK_MAGIC when `kick` is set: the stub's `extern "C" fn()` that sends queued app messages to a K host now;
    // the loader calls it after the app's send (docs/abi-1.4.md). Not `api`: loader 1.3 needs api == 1, and an
    // older stub leaves these bytes unset, so the magic guards against garbage.
    pub kick_magic: u32,
    pub kick: u32,
    // 1 while a thread runs the stub's frame path (reply buffer, outbox consumer)
    pub busy: core::sync::atomic::AtomicU8,
}

pub const KICK_MAGIC: u32 = 0x4B43_494B; // "KICK"
pub const APP_V11: u8 = 1;
pub const APP_PAUSED: u8 = 2;

const _: () = assert!(core::mem::size_of::<StubRam>() <= 0xE00);

#[cfg(target_os = "none")]
pub fn stub_ram() -> &'static mut StubRam {
    unsafe { &mut *(STUB_RAM as *mut StubRam) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{offset_of, size_of};

    #[test]
    fn stub_ram_offsets() {
        assert_eq!(offset_of!(StubRam, trace_head), 0x44);
        assert_eq!(offset_of!(StubRam, loader_block), 0x4E8);
        assert_eq!(offset_of!(StubRam, app_block), 0x4F0);
        assert_eq!(offset_of!(StubRam, alloc_limit), 0x4F8);
        assert_eq!(offset_of!(StubRam, loader_entry), 0x4FC);
        assert_eq!(offset_of!(StubRam, load_file), 0x500);
        assert_eq!(offset_of!(StubRam, load_scratch), 0x52C);
        assert_eq!(offset_of!(StubRam, api), 9);
        assert_eq!(offset_of!(StubRam, host_key), 0x72C);
        assert!(size_of::<StubRam>() <= 0xE00);
    }

    #[test]
    fn slot_paths() {
        assert_eq!(&slot_path(LOADER_SLOT, false)[..21], b"/NAND:/wp2apps/L.pkg\0");
        assert_eq!(&slot_path(3, true)[..21], b"/NAND:/wp2apps/3.tmp\0");
        assert!(slot_path(3, true)[21..].iter().all(|&b| b == 0));
        assert_eq!(&DIR[..], b"/NAND:/wp2apps\0");
        assert_eq!(&data_path(2, false)[..21], b"/NAND:/wp2apps/2.dat\0");
        assert_eq!(&data_path(2, true)[..21], b"/NAND:/wp2apps/2.new\0");
    }
}
