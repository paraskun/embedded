use embassy_net::{Stack, udp::{PacketMetadata, UdpSocket}};
use static_cell::StaticCell;

static RX: StaticCell<[u8; 1024]> = StaticCell::new();
static RM: StaticCell<[PacketMetadata; 4]> = StaticCell::new();
static TX: StaticCell<[u8; 1024]> = StaticCell::new();
static TM: StaticCell<[PacketMetadata; 4]> = StaticCell::new();

pub struct RGB(u8, u8, u8);

pub enum Type {
    RGB,
    HSL,
    RGBW,
    GS,
}

#[repr(packed)]
pub struct Hdr {
    flags: u8,
    seq: u8,
    dtype: u8,
    id: u8,
    offset: u32,
    size: u16,
}

pub struct Listener<'a> {
    sock: UdpSocket<'a>,
}

impl Listener {
    pub fn new(stack: Stack<'_>) -> Listener {
        let sock = UdpSocket::new(stack, 
            RM.init([PacketMetadata::EMPTY; 4]), 
            RX.init([0; 1024]),
            TM.init([PacketMetadata::EMPTY; 4]), 
            TX.init([0; 1024]));

        Listener { sock }
    }
}