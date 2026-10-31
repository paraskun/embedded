use core::fmt;
use static_cell::StaticCell;
use bytemuck::NoUninit;

use embassy_net::{Stack, udp::{PacketMetadata, UdpSocket}};

static RX: StaticCell<[u8; 1024]> = StaticCell::new();
static RM: StaticCell<[PacketMetadata; 4]> = StaticCell::new();
static TX: StaticCell<[u8; 1024]> = StaticCell::new();
static TM: StaticCell<[PacketMetadata; 4]> = StaticCell::new();

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, NoUninit)]
pub struct RGB(u8, u8, u8);

impl RGB {
    fn parse(buf: &[u8]) -> RGB {
        RGB(buf[0], buf[1], buf[2])
    }
}

#[derive(Default, Debug)]
pub struct Header {
    flags: u8,
    seq: u8,
    dtype: u8,
    id: u8,
    offset: u32,
    pub size: u16,
    tcode: u32,
}

#[derive(Copy, Clone)]
pub union Body {
    pub rgb: [RGB; 512],
}

pub struct Packet {
    pub header: Header,
    pub body: Body,
}

impl Default for Packet {
    fn default() -> Self {
        Self { 
            header: Header::default(),
            body: Body { rgb: [RGB::default(); 512] },
        }
    }
}

impl fmt::Debug for Packet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Packet")
            .field("hdr", &self.header)
            .finish()
    }
}

impl Packet {
    fn parse(buf: &[u8], pkt: &mut Packet) {
        pkt.header.flags = buf[0];
        pkt.header.seq = buf[1];
        pkt.header.dtype = buf[2];
        pkt.header.id = buf[3];
        pkt.header.offset = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
        pkt.header.size = u16::from_be_bytes([buf[8], buf[9]]);

        let mut off = 10;

        if (pkt.header.flags & 0b00010000) != 0 {
            pkt.header.tcode = u32::from_be_bytes([buf[10], buf[11], buf[12], buf[13]]);
            off = 14;
        }

        let t = pkt.header.dtype >> 3 & 0b111;
        if t != 0 && t != 1 {
            pkt.header.size = 0;
            return
        }

        for (i, ch) in buf[off..].chunks_exact(3).enumerate() {
            unsafe {
                pkt.body.rgb[i] = RGB::parse(ch)
            }
        }
    }
}

pub struct Listener<'a> {
    us: UdpSocket<'a>,
    rx: [u8; 1024],
}

impl<'a> Listener<'a> {
    pub fn new(stack: Stack<'a>) -> Listener<'a> {
        let mut us = UdpSocket::new(stack, 
            RM.init([PacketMetadata::EMPTY; 4]), 
            RX.init([0; 1024]),
            TM.init([PacketMetadata::EMPTY; 4]), 
            TX.init([0; 1024]));

        us.bind(4048).unwrap();

        Listener { us, rx: [0; 1024] }
    }

    pub async fn recv(&mut self, pkt: &mut Packet) {
        match self.us.recv_from(&mut self.rx).await {
            Ok(n) => Packet::parse(&self.rx[..n.0], pkt),
            Err(_) => {},
        }
    }
}
