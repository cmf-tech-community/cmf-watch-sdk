use crate::chan::MSG_MAX;
use crate::layout::{LOADER_SLOT, SLOTS, STUB_SLOT};

pub const MAX_DATA: usize = 128;
pub const INFO: u16 = 0x90A0;
pub const BEGIN: u16 = 0x90A1;
pub const DATA: u16 = 0x90A2;
pub const END: u16 = 0x90A3;
pub const DELETE: u16 = 0x90A4;
pub const MSG: u16 = 0x90A5;
// watch -> host, unsolicited: seq, slot, data (docs/abi-1.1.md)
pub const EVENT: u16 = 0xB0A6;
pub const ANY_SLOT: u8 = 0xFF;
// keep-alive: refreshes the host key, answers running_slot and app_flags (0x90A6 is skipped: its reply is EVENT)
pub const PING: u16 = 0x90A7;

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u8)]
pub enum Status {
    Ok = 0,
    BadRequest = 1,
    Io = 2,
    Sequence = 3,
    Crc = 4,
    Busy = 5,
    Invalid = 6,
    NoApp = 7,
    Full = 8,
    Paused = 9,
}

#[derive(Debug, PartialEq)]
pub enum Request<'a> {
    Info(u8),
    StubVersion,
    Begin { slot: u8, size: u32, crc: u32 },
    Data { offset: u32, bytes: &'a [u8] },
    End,
    Delete(u8),
    Msg { slot: u8, data: &'a [u8] },
    Ping,
}

fn slot(raw: u8) -> Result<u8, Status> {
    if raw < SLOTS || raw == LOADER_SLOT { Ok(raw) } else { Err(Status::BadRequest) }
}

fn word(raw: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(raw[off..off + 4].try_into().unwrap())
}

pub fn parse(c2: u16, payload: &[u8]) -> Result<Request<'_>, Status> {
    match (c2, payload.len()) {
        (INFO, 1) if payload[0] == STUB_SLOT => Ok(Request::StubVersion),
        (INFO, 1) => Ok(Request::Info(slot(payload[0])?)),
        (BEGIN, 9) => Ok(Request::Begin { slot: slot(payload[0])?, size: word(payload, 1), crc: word(payload, 5) }),
        (DATA, 5..=0x84) => Ok(Request::Data { offset: word(payload, 0), bytes: &payload[4..] }),
        // the stock decrypt helper returns NULL for an empty plaintext, so END carries one dummy byte
        (END, 1) => Ok(Request::End),
        (DELETE, 1) => Ok(Request::Delete(slot(payload[0])?)),
        (PING, 1) => Ok(Request::Ping),
        (MSG, n) if (2..=1 + MSG_MAX).contains(&n) && (payload[0] < SLOTS || payload[0] == ANY_SLOT) => Ok(Request::Msg { slot: payload[0], data: &payload[1..] }),
        _ => Err(Status::BadRequest),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_malformed() {
        assert_eq!(parse(0x90A0, &[]), Err(Status::BadRequest));
        assert_eq!(parse(0x90A0, &[0, 0]), Err(Status::BadRequest));
        assert_eq!(parse(0x90A2, &[0; 4]), Err(Status::BadRequest));
        assert_eq!(parse(0x90A3, &[]), Err(Status::BadRequest));
        assert_eq!(parse(0x90A3, &[0, 0]), Err(Status::BadRequest));
        assert_eq!(parse(0x90A3, &[0]), Ok(Request::End));
        assert_eq!(parse(0x90A6, &[]), Err(Status::BadRequest));
        assert_eq!(parse(0x90A4, &[16]), Err(Status::BadRequest));
    }

    #[test]
    fn msg_bounds() {
        assert_eq!(parse(MSG, &[0xFF, 7]), Ok(Request::Msg { slot: 0xFF, data: &[7] }));
        assert!(matches!(parse(MSG, &[3; 1 + MSG_MAX]), Ok(Request::Msg { slot: 3, .. })));
        assert_eq!(parse(MSG, &[3; 2 + MSG_MAX]), Err(Status::BadRequest));
        assert_eq!(parse(MSG, &[3]), Err(Status::BadRequest));
        assert_eq!(parse(MSG, &[16, 1]), Err(Status::BadRequest));
        assert!(matches!(parse(MSG, &[15, 1]), Ok(Request::Msg { slot: 15, .. })));
        assert_eq!(parse(PING, &[0]), Ok(Request::Ping));
        assert_eq!(parse(0x90A6, &[0]), Err(Status::BadRequest));
    }

    #[test]
    fn data_bounds() {
        assert_eq!(parse(0x90A2, &[4, 0, 0, 0, 7]), Ok(Request::Data { offset: 4, bytes: &[7] }));
        assert!(matches!(parse(0x90A2, &[0; 4 + MAX_DATA]), Ok(Request::Data { .. })));
        assert_eq!(parse(0x90A0, &[7]), Ok(Request::Info(7)));
        assert_eq!(parse(0x90A0, &[0xFE]), Ok(Request::StubVersion));
        assert_eq!(parse(0x90A0, &[0xFD]), Err(Status::BadRequest));
    }

    #[test]
    fn parse_requests() {
        assert_eq!(parse(0x90A1, &[3, 0x10, 0, 0, 0, 0xEF, 0xBE, 0xAD, 0xDE]), Ok(Request::Begin { slot: 3, size: 16, crc: 0xDEADBEEF }));
        assert_eq!(parse(0x90A1, &[16, 0, 0, 0, 0, 0, 0, 0, 0]), Err(Status::BadRequest));
        assert!(matches!(parse(0x90A1, &[15, 0, 0, 0, 0, 0, 0, 0, 0]), Ok(Request::Begin { slot: 15, .. })));
        assert_eq!(parse(0x90A2, &[0; 4 + 129]), Err(Status::BadRequest));
        assert_eq!(parse(0x90A4, &[0xFF]), Ok(Request::Delete(0xFF)));
    }
}
