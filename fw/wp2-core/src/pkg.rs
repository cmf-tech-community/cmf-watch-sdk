use crate::layout::{APP_SIZE, LOADER_ABI_MINOR, LOADER_SIZE, LOADER_SLOT, SLOTS};

pub const HEADER_SIZE: usize = 0x40;
const MAGIC: &[u8; 4] = b"WP2A";
const CRC_SPAN: usize = 0x34;
pub const KIND_APP: u8 = 0;
pub const KIND_LOADER: u8 = 1;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Header {
    pub kind: u8,
    pub abi: u16,
    pub abi_minor: u8,
    pub name: [u8; 24],
    pub version: u32,
    pub image_size: u32,
    pub entry: u32,
    pub reloc_count: u32,
    pub body_crc: u32,
}

#[derive(Debug, PartialEq)]
pub enum PkgError {
    Magic,
    HeaderCrc,
    Abi,
    Kind,
    Bounds,
    Name,
    Reloc,
}

fn word(raw: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(raw[off..off + 4].try_into().unwrap())
}

pub fn parse_header(raw: &[u8; HEADER_SIZE]) -> Result<Header, PkgError> {
    if &raw[..4] != MAGIC || raw[0x38..].iter().any(|&b| b != 0) {
        return Err(PkgError::Magic);
    }
    if crc32(0, &raw[..CRC_SPAN]) != word(raw, CRC_SPAN) {
        return Err(PkgError::HeaderCrc);
    }
    let kind = raw[6];
    if kind > KIND_LOADER {
        return Err(PkgError::Kind);
    }
    let name: [u8; 24] = raw[8..0x20].try_into().unwrap();
    let len = name.iter().position(|&b| b == 0).unwrap_or(24);
    if len == 0 || len > 23 || name[len..].iter().any(|&b| b != 0) || core::str::from_utf8(&name[..len]).is_err() {
        return Err(PkgError::Name);
    }
    Ok(Header {
        kind,
        abi: u16::from_le_bytes([raw[4], raw[5]]),
        abi_minor: raw[7],
        name,
        version: word(raw, 0x20),
        image_size: word(raw, 0x24),
        entry: word(raw, 0x28),
        reloc_count: word(raw, 0x2C),
        body_crc: word(raw, 0x30),
    })
}

// ponytail: a slot outside 0..SLOTS and not LOADER_SLOT is reported as Kind
pub fn check_slot(h: &Header, slot: u8) -> Result<(), PkgError> {
    let limit = match (slot, h.kind) {
        (LOADER_SLOT, KIND_LOADER) => LOADER_SIZE,
        (s, KIND_APP) if s < SLOTS => APP_SIZE,
        _ => return Err(PkgError::Kind),
    };
    if h.abi != 1 || u16::from(h.abi_minor) > LOADER_ABI_MINOR {
        return Err(PkgError::Abi);
    }
    if h.image_size > limit || h.entry >= h.image_size || h.reloc_count > h.image_size / 4 {
        return Err(PkgError::Bounds);
    }
    Ok(())
}

pub fn file_size(h: &Header) -> u32 {
    HEADER_SIZE as u32 + h.image_size + 4 * h.reloc_count
}

// `next` is the lowest allowed offset, 0 before the first chunk of a table: offsets must strictly increase
pub fn relocate(image: &mut [u8], base: u32, relocs: &[u8], next: &mut usize) -> Result<(), PkgError> {
    if relocs.len() % 4 != 0 {
        return Err(PkgError::Reloc);
    }
    for r in relocs.chunks_exact(4) {
        let off = u32::from_le_bytes(r.try_into().unwrap()) as usize;
        if off < *next {
            return Err(PkgError::Reloc);
        }
        let end = off.checked_add(4).filter(|&e| e <= image.len()).ok_or(PkgError::Reloc)?;
        *next = off + 1;
        let w = u32::from_le_bytes(image[off..end].try_into().unwrap()).wrapping_add(base);
        image[off..end].copy_from_slice(&w.to_le_bytes());
    }
    Ok(())
}

// App icon (src/wp2apps/icon.py): a descriptor at image offset 0, then LV_IMG_CF_TRUE_COLOR_ALPHA pixels.
pub const ICON_DESC: usize = 16;
pub const ICON_SIDE: u32 = 96;
pub const ICON_BYTES: u32 = ICON_SIDE * ICON_SIDE * 3;

/// The app's declarations (docs/app-guide.md, "Widget" and "Page"): `"WP2F", flags, 0, 0, 0` right after the icon,
/// or at image offset 0 without one (`cmf_wp2_app::app_features!`). None of it = 0.
pub const FEATURES: usize = 8;
pub const F_WIDGET: u8 = 1;
pub const F_PAGE: u8 = 2;

pub fn features(d: &[u8]) -> u8 {
    match d {
        [b'W', b'P', b'2', b'F', f, ..] => *f,
        _ => 0,
    }
}

pub fn icon_ok(desc: &[u8; ICON_DESC], image_size: u32) -> bool {
    let half = |off: usize| u16::from_le_bytes([desc[off], desc[off + 1]]) as u32;
    &desc[..4] == b"WP2I"
        && desc[4..6] == [1, 5]
        && half(6) == ICON_SIDE
        && half(8) == ICON_SIDE
        && half(10) == 0
        && word(desc, 12) == ICON_BYTES
        && ICON_DESC as u32 + ICON_BYTES <= image_size
}

// ponytail: bitwise, no table; add a 1 KiB table if verifying 128 KiB gets too slow
pub fn crc32(init: u32, data: &[u8]) -> u32 {
    let mut c = !init;
    for &b in data {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = (c >> 1) ^ (0xEDB8_8320 & (c & 1).wrapping_neg());
        }
    }
    !c
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOLDEN: [u8; 88] = [
        0x57, 0x50, 0x32, 0x41, 0x01, 0x00, 0x00, 0x00, 0x67, 0x6f, 0x6c, 0x64,
        0x65, 0x6e, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00,
        0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
        0x28, 0x37, 0xff, 0x25, 0xbb, 0xfa, 0xaa, 0x7b, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00,
        0x01, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00,
        0x0c, 0x00, 0x00, 0x00,
    ];

    #[test]
    fn golden_package_from_python() {
        let h = parse_header(GOLDEN[..0x40].try_into().unwrap()).unwrap();
        assert_eq!((h.kind, h.abi, h.abi_minor, h.version, h.image_size, h.reloc_count), (0, 1, 0, 7, 16, 2));
        assert_eq!(file_size(&h) as usize, GOLDEN.len());
        assert_eq!(h.body_crc, crc32(0, &GOLDEN[0x40..]));
        assert_eq!(crc32(crc32(0, &GOLDEN[0x40..0x48]), &GOLDEN[0x48..]), h.body_crc);
    }

    #[test]
    fn relocate_adds_base() {
        let mut image = GOLDEN[0x40..0x50].to_vec();
        relocate(&mut image, 0x3810_0000, &GOLDEN[0x50..], &mut 0).unwrap();
        assert_eq!(image, [0u32, 0x3810_0010, 1, 0x3810_0008].iter().flat_map(|w| w.to_le_bytes()).collect::<Vec<_>>());
        assert_eq!(relocate(&mut image, 0, &13u32.to_le_bytes(), &mut 0), Err(PkgError::Reloc));
    }

    #[test]
    fn relocs_strictly_increase_across_chunks() {
        let mut image = [0u8; 16];
        let mut next = 0;
        relocate(&mut image, 1, &4u32.to_le_bytes(), &mut next).unwrap();
        assert_eq!(relocate(&mut image, 1, &4u32.to_le_bytes(), &mut next), Err(PkgError::Reloc));
        let pair: Vec<u8> = [8u32, 0].iter().flat_map(|w| w.to_le_bytes()).collect();
        assert_eq!(relocate(&mut image, 1, &pair, &mut 0), Err(PkgError::Reloc));
    }

    #[test]
    fn header_crc_and_reserved_checked() {
        let mut raw: [u8; 0x40] = GOLDEN[..0x40].try_into().unwrap();
        raw[0x28] ^= 1;
        assert_eq!(parse_header(&raw), Err(PkgError::HeaderCrc));
        let mut raw: [u8; 0x40] = GOLDEN[..0x40].try_into().unwrap();
        raw[0x3A] = 1;
        assert!(parse_header(&raw).is_err());
    }

    #[test]
    fn feature_record() {
        assert_eq!(features(b"WP2F\x03\0\0\0"), F_WIDGET | F_PAGE);
        assert_eq!(features(b"WP2I\x01\x05\0\0"), 0);
        assert_eq!(features(b"WP2"), 0);
    }

    #[test]
    fn icon_descriptor() {
        let mut d = [0u8; ICON_DESC];
        d[..6].copy_from_slice(b"WP2I\x01\x05");
        d[6] = 96;
        d[8] = 96;
        d[12..].copy_from_slice(&ICON_BYTES.to_le_bytes());
        let fit = ICON_DESC as u32 + ICON_BYTES;
        assert!(icon_ok(&d, fit));
        assert!(!icon_ok(&d, fit - 1), "truncated");
        for (i, v) in [(0, b'X'), (4, 2), (5, 4), (6, 128), (8, 95), (10, 1), (14, 1)] {
            let mut bad = d;
            bad[i] = v;
            assert!(!icon_ok(&bad, APP_SIZE), "byte {i} = {v}");
        }
        let code = [0x80, 0xB5, 0x6F, 0x46, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        assert!(!icon_ok(&code, APP_SIZE));
    }

    #[test]
    fn slot_rules() {
        let h = parse_header(GOLDEN[..0x40].try_into().unwrap()).unwrap();
        assert!(check_slot(&h, 0).is_ok());
        assert_eq!(check_slot(&h, LOADER_SLOT), Err(PkgError::Kind));
        let mut newer = h;
        newer.abi_minor = LOADER_ABI_MINOR as u8 + 1;
        assert_eq!(check_slot(&newer, 0), Err(PkgError::Abi));
        let mut big = h;
        big.image_size = APP_SIZE + 1;
        assert_eq!(check_slot(&big, 0), Err(PkgError::Bounds));
        let mut many = h;
        many.reloc_count = 5;
        assert_eq!(check_slot(&many, 0), Err(PkgError::Bounds));
    }
}
