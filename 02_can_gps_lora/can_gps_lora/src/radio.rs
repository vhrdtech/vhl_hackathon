use serde::{Serialize, Deserialize};
// use vhrdcan::RawFrame;

const MAGIC: [u8; 4] = [42, 32, 176, 21];

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct RadioFrame {
    magic: [u8; 4],
    source_node_id: u32,
    destination_node_id: u32,
    pub(crate) frame_type: FrameType,
}
impl RadioFrame {
    pub fn new(from: u32, to: u32, frame_type: FrameType) -> Self {
        let frame = Self {
            magic: MAGIC,
            source_node_id: from,
            destination_node_id: to,
            frame_type
        };
        frame
    }

    pub fn serialize<'a>(&self, buf: &'a mut [u8]) -> Result<&'a [u8], Error> {
        let used_size = {
            postcard::to_slice(self, &mut buf[4..])?.len()
        };
        let mut crc32 = crc_any::CRCu32::crc32();
        crc32.digest(&buf[4..used_size + 4]);
        let crc32: [u8; 4] = crc32.get_crc().to_le_bytes();
        buf[0..=3].copy_from_slice(&crc32);
        Ok(&buf[0..used_size+4])
    }

    pub fn deserialize(buf: &[u8]) -> Result<Self, Error> {
        let frame: RadioFrame = postcard::from_bytes(&buf[4..])?;

        let mut received_crc: [u8; 4] = [0u8; 4];
        received_crc.copy_from_slice(&buf[0..=3]);
        let received_crc = u32::from_le_bytes(received_crc);
        let mut crc32 = crc_any::CRCu32::crc32();
        crc32.digest(&buf[4..]);
        if crc32.get_crc() != received_crc {
            return Err(Error::CRCMismatch);
        }

        if frame.magic != MAGIC {
            return Err(Error::BadMagic);
        }

        Ok(frame)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum FrameType {
    // CANBusForward(RawFrame)
    HeartBeat(HeartBeat)
}

#[derive(Debug)]
pub enum Error {
    SerdesError(postcard::Error),
    BadMagic,
    CRCMismatch,
}
impl From<postcard::Error> for Error {
    fn from(e: postcard::Error) -> Self {
        Error::SerdesError(e)
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq)]
pub struct HeartBeat {
    pub uptime: u32,
    pub remote_rssi: i32
}