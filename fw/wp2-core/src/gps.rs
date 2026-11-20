//! GPS for apps (docs/abi-1.4.md, "GPS"): the loader turns the stock GPS on and off as the stock workouts do
//! (ux2sys 0x63, docs/recon.md "GPS") and hands the app the stock's parsed GGA. The hold is the heart rate's: one
//! start per hold, one stop at its end, and a GPS the stock already runs is only read.
use crate::events::{Event, GPS};
pub use crate::heart::Heart as Hold;

/// One GPS event per this many ms while the app holds the GPS (the stock parses a GGA about once a second).
pub const EMIT_MS: u32 = 1000;
/// SENSOR record value for the GPS (lvcmd::sensor).
pub const SENSOR_GPS: u8 = 2;
/// Mode the loader sets: the stock workouts' (0x1007B2AC, 0x1007EF3C call sport_view_set_gps_enable(1)).
pub const MODE_ON: u32 = 1;
/// Bytes of the stock info block the loader reads (GGA cells up to the time to first fix at +0x54).
pub const RAW_LEN: usize = 0x58;
/// GPS_FIX record payload: lat_e7 i32, lon_e7 i32, alt_dm i32, ms u32, fix u8, sats u8, ttff_s u16 (LE).
pub const FIX_LEN: usize = 20;

/// Stock info block (0x38016C40, written by the NMEA parser 0x10197D40 on gps_service) to the app's fix. `ms`: the
/// loader's clock when it was read. Degrees are f64 in the block; the app gets 1e-7 degrees.
pub fn fix(raw: &[u8; RAW_LEN], ms: u32) -> [u8; FIX_LEN] {
    let f64_at = |o: usize| f64::from_le_bytes(raw[o..o + 8].try_into().unwrap());
    let u32_at = |o: usize| u32::from_le_bytes(raw[o..o + 4].try_into().unwrap());
    // rounded (core has no f64::round); `as` saturates, NaN gives 0
    let e7 = |deg: f64| (deg * 1e7 + if deg < 0.0 { -0.5 } else { 0.5 }) as i32;
    let alt_m = f32::from_bits(u32_at(0x48));
    let mut out = [0u8; FIX_LEN];
    out[0..4].copy_from_slice(&e7(f64_at(0x38)).to_le_bytes());
    out[4..8].copy_from_slice(&e7(f64_at(0x40)).to_le_bytes());
    out[8..12].copy_from_slice(&((alt_m * 10.0 + if alt_m < 0.0 { -0.5 } else { 0.5 }) as i32).to_le_bytes());
    out[12..16].copy_from_slice(&ms.to_le_bytes());
    out[16] = u32_at(0x4C).min(255) as u8; // GGA fix quality, 0 = no fix
    out[17] = u32_at(0x50).min(255) as u8; // satellites in use
    out[18..20].copy_from_slice(&(u32_at(0x54).min(0xFFFF) as u16).to_le_bytes());
    out
}

/// GPS event: `x` = fix quality (0 = none), `y` = satellites in use. The values come with `Api::gps_fix`.
pub fn event(fix: &[u8; FIX_LEN]) -> Event {
    Event { kind: GPS, x: fix[16].into(), y: fix[17].into(), ..Event::default() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(lat: f64, lon: f64, alt: f32, quality: u32, sats: u32, ttff: u32) -> [u8; RAW_LEN] {
        let mut r = [0u8; RAW_LEN];
        r[0x38..0x40].copy_from_slice(&lat.to_le_bytes());
        r[0x40..0x48].copy_from_slice(&lon.to_le_bytes());
        r[0x48..0x4C].copy_from_slice(&alt.to_le_bytes());
        r[0x4C..0x50].copy_from_slice(&quality.to_le_bytes());
        r[0x50..0x54].copy_from_slice(&sats.to_le_bytes());
        r[0x54..0x58].copy_from_slice(&ttff.to_le_bytes());
        r
    }

    #[test]
    fn converts_the_stock_block() {
        let f = fix(&raw(12.345678, -98.765432, 123.4, 1, 8, 31), 5000);
        assert_eq!(i32::from_le_bytes(f[0..4].try_into().unwrap()), 123_456_780);
        assert_eq!(i32::from_le_bytes(f[4..8].try_into().unwrap()), -987_654_320);
        assert_eq!(i32::from_le_bytes(f[8..12].try_into().unwrap()), 1234);
        assert_eq!(u32::from_le_bytes(f[12..16].try_into().unwrap()), 5000);
        assert_eq!((f[16], f[17], u16::from_le_bytes([f[18], f[19]])), (1, 8, 31));
        assert_eq!(event(&f), Event { kind: GPS, x: 1, y: 8, ..Event::default() });
    }

    #[test]
    fn no_fix_is_zeros() {
        let f = fix(&[0; RAW_LEN], 7);
        assert_eq!(&f[..12], &[0; 12]);
        assert_eq!(&f[16..], &[0; 4]);
        assert_eq!(event(&f).x, 0);
    }

    #[test]
    fn layout_matches_sdk() {
        assert_eq!(FIX_LEN, cmf_wp2_app::GPS_FIX_LEN);
        assert_eq!(GPS, cmf_wp2_app::GPS);
        let f = fix(&raw(-0.5, 179.9999999, -12.0, 2, 300, 70_000), 9);
        let s = cmf_wp2_app::GpsFix::from_bytes(&f);
        assert_eq!((s.lat_e7, s.lon_e7, s.alt_dm, s.ms), (-5_000_000, 1_799_999_999, -120, 9));
        assert_eq!((s.fix, s.sats, s.ttff_s), (2, 255, 0xFFFF));
    }
}
