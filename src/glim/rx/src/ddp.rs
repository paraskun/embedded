use embassy_net::{
    Stack,
    udp::{PacketMetadata, UdpSocket},
};
use static_cell::StaticCell;

pub const MAX_PIXELS: u16 = 1024;

const HEADER_SIZE: usize = 10;
const TIMECODE_FLAG: u8 = 0b0001_0000;
const TIMECODE_SIZE: usize = 4;
const MAX_FRAME_SIZE: usize = MAX_PIXELS as usize * 3;
const MAX_PACKET_SIZE: usize = HEADER_SIZE + TIMECODE_SIZE + MAX_FRAME_SIZE;

static RX: StaticCell<[u8; MAX_PACKET_SIZE]> = StaticCell::new();
static RM: StaticCell<[PacketMetadata; 4]> = StaticCell::new();
static TX: StaticCell<[u8; 1024]> = StaticCell::new();
static TM: StaticCell<[PacketMetadata; 4]> = StaticCell::new();

pub struct Listener<'a> {
    us: UdpSocket<'a>,
    rx: [u8; MAX_PACKET_SIZE],
}

impl<'a> Listener<'a> {
    pub fn new(stack: Stack<'a>) -> Listener<'a> {
        let mut us = UdpSocket::new(
            stack,
            RM.init([PacketMetadata::EMPTY; 4]),
            RX.init([0; MAX_PACKET_SIZE]),
            TM.init([PacketMetadata::EMPTY; 4]),
            TX.init([0; 1024]),
        );

        us.bind(4048).unwrap();

        Listener {
            us,
            rx: [0; MAX_PACKET_SIZE],
        }
    }

    pub async fn recv(&mut self, pixels: u16) -> Option<&[u8]> {
        let n = self.us.recv_from(&mut self.rx).await.ok()?.0;
        parse(&self.rx[..n], pixels)
    }
}

fn parse(buf: &[u8], pixels: u16) -> Option<&[u8]> {
    let flags = *buf.first()?;
    let header_size = if flags & TIMECODE_FLAG != 0 {
        HEADER_SIZE + TIMECODE_SIZE
    } else {
        HEADER_SIZE
    };

    if buf.len() < header_size {
        return None;
    }

    let data_type = (buf[2] >> 3) & 0b111;
    let offset = u32::from_be_bytes(buf[4..8].try_into().ok()?);
    let size = usize::from(u16::from_be_bytes(buf[8..10].try_into().ok()?));
    let expected = usize::from(pixels).checked_mul(3)?;

    if data_type > 1 || offset != 0 || size != expected || buf.len() != header_size + size {
        return None;
    }

    Some(&buf[header_size..])
}
