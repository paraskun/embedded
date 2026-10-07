use core::fmt;
use embassy_rp::{
    Peri,
    interrupt::typelevel::{Binding, USBCTRL_IRQ},
    peripherals,
    usb::{Driver, InterruptHandler},
};
use embassy_usb::{
    Builder, Config, UsbDevice,
    class::cdc_acm::{CdcAcmClass, State},
};
use static_cell::StaticCell;

type D = Driver<'static, peripherals::USB>;

const MAX_PACKET: usize = 64;

static STATE: StaticCell<State<'static>> = StaticCell::new();
static CONFIG_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static BOS_DESC: StaticCell<[u8; 256]> = StaticCell::new();
static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

pub enum Cmd {
    Size,
    Frame,
    Unknown,
}

pub struct Serial {
    class: CdcAcmClass<'static, D>,
    // Last received packet and how far we've consumed it. Kept between calls,
    // so bytes after a newline (pasted text, "\r\n") aren't thrown away.
    rx: [u8; MAX_PACKET],
    rx_pos: usize,
    rx_len: usize,
    // Line being typed. Also kept between calls.
    line: [u8; 64],
    len: usize,
    last_cr: bool,
}

pub fn new<IRQ>(usb: Peri<'static, peripherals::USB>, irq: IRQ) -> (UsbDevice<'static, D>, Serial)
where
    IRQ: Binding<USBCTRL_IRQ, InterruptHandler<peripherals::USB>>,
{
    let driver = Driver::new(usb, irq);

    let mut config = Config::new(0xc0de, 0xcafe);
    config.manufacturer = Some("emb");
    config.product = Some("p3");
    config.serial_number = Some("1");

    let mut builder = Builder::new(
        driver,
        config,
        CONFIG_DESC.init([0; 256]),
        BOS_DESC.init([0; 256]),
        &mut [], // MS OS descriptors are only needed for WinUSB; CDC doesn't use them
        CONTROL_BUF.init([0; 64]),
    );

    let class = CdcAcmClass::new(&mut builder, STATE.init(State::new()), MAX_PACKET as u16);
    let device = builder.build();

    let serial = Serial {
        class,
        rx: [0; MAX_PACKET],
        rx_pos: 0,
        rx_len: 0,
        line: [0; 64],
        len: 0,
        last_cr: false,
    };

    (device, serial)
}

impl Serial {
    pub async fn read(&mut self, frame: &mut [u8]) -> Cmd {
        loop {
            if self.rx_pos == self.rx_len {
                self.class.wait_connection().await;

                match self.class.read_packet(&mut self.rx).await {
                    Ok(n) => (self.rx_pos, self.rx_len) = (0, n),
                    Err(_) => {
                        (self.rx_pos, self.rx_len, self.len) = (0, 0, 0);
                        continue;
                    }
                }
            }

            let b = self.rx[self.rx_pos];
            self.rx_pos += 1;

            let after_cr = core::mem::replace(&mut self.last_cr, b == b'\r');
            if b == b'\n' && after_cr {
                continue;
            }

            match b {
                b'\r' | b'\n' => {
                    self.send(b"\r\n").await;

                    if self.len == 0 {
                        continue;
                    }

                    let cmd = match core::str::from_utf8(&self.line[..self.len]).unwrap_or("").trim() {
                        "size" => Cmd::Size,
                        "frame" => {
                            let mut cur = 0;

                            loop {
                                match self.class.read_packet(&mut frame[cur..]).await {
                                    Ok(n) => {
                                        cur += n;

                                        if cur == frame.len() {
                                            break;
                                        }
                                    },
                                    Err(_) => {
                                        continue;
                                    }
                                }
                            }

                            Cmd::Frame
                        },
                        _ => Cmd::Unknown,
                    };

                    self.len = 0;
                    return cmd;
                }
                0x08 | 0x7f => {
                    if self.len > 0 {
                        self.len -= 1;
                        self.send(b"\x08 \x08").await;
                    }
                }
                _ => {
                    if self.len < self.line.len() {
                        self.line[self.len] = b;
                        self.len += 1;
                        self.send(&[b]).await;
                    }
                }
            }
        }
    }

    pub async fn write(&mut self, args: fmt::Arguments<'_>) {
        let mut raw = [0u8; 256];
        let mut len = 0usize;

        struct Sink<'a> {
            buf: &'a mut [u8],
            len: &'a mut usize,
        }

        impl fmt::Write for Sink<'_> {
            fn write_str(&mut self, s: &str) -> fmt::Result {
                for &b in s.as_bytes() {
                    let needed = if b == b'\n' { 2 } else { 1 };
                    if *self.len + needed > self.buf.len() {
                        return Err(fmt::Error);
                    }

                    if b == b'\n' {
                        self.buf[*self.len] = b'\r';
                        *self.len += 1;
                    }

                    self.buf[*self.len] = b;
                    *self.len += 1;
                }

                Ok(())
            }
        }

        let _ = fmt::write(&mut Sink { buf: &mut raw, len: &mut len }, args);
        self.send(&raw[..len]).await;
    }

    async fn send(&mut self, data: &[u8]) {
        for chunk in data.chunks(MAX_PACKET) {
            if self.class.write_packet(chunk).await.is_err() {
                return;
            }
        }

        if !data.is_empty() && data.len() % MAX_PACKET == 0 {
            self.class.write_packet(&[]).await.ok();
        }
    }
}
