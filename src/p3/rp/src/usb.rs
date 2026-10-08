use embassy_usb::{
    Builder, Config, UsbDevice, class::cdc_acm::{CdcAcmClass, State}, driver::Driver
};
use static_cell::StaticCell;

const PKT_SIZE: usize = 64;

static STATE: StaticCell<State<'static>> = StaticCell::new();
static CFG_BUF: StaticCell<[u8; 256]> = StaticCell::new();
static BOS_BUF: StaticCell<[u8; 256]> = StaticCell::new();
static CTL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

#[derive(PartialEq)]
pub enum Cmd {
    Size,
    Frame,
}

impl Cmd {
    fn parse(buf: &[u8]) -> Result<Cmd, ()> {
        match core::str::from_utf8(buf).unwrap_or("").trim() {
            "size" => Ok(Cmd::Size),
            "frame" => Ok(Cmd::Frame),
            _ => Err(())
        }
    }
}

pub struct Interface<D: Driver<'static>> {
    usb: CdcAcmClass<'static, D>,
    rxb: [u8; PKT_SIZE],
    rxp: usize,
}

impl<D: Driver<'static>> Interface<D> {
    pub fn new(driver: D) -> (Interface<D>, UsbDevice<'static, D>) {
        let mut config = Config::new(0xc0de, 0xcafe);
        config.manufacturer = Some("paraskun");
        config.product = Some("visualizer");
        config.serial_number = Some("0x00");

        let mut builder = Builder::new(
            driver,
            config,
            CFG_BUF.init([0; 256]),
            BOS_BUF.init([0; 256]), 
            &mut [],
            CTL_BUF.init([0; 64])
        );

        let usb = CdcAcmClass::new(
            &mut builder,
            STATE.init(State::new()),
            PKT_SIZE as u16
        );

        (Interface { usb, rxb: [0; PKT_SIZE], rxp: 0 }, builder.build())
    }

    pub async fn read(&mut self, frame: &mut [u8]) -> Cmd {
        let mut cmd: Cmd = Cmd::Size;
        let mut buf = [0u8; PKT_SIZE];
        let mut end = false;

        while !end {
            self.usb.wait_connection().await;

            match self.usb.read_packet(&mut buf).await {
                Ok(n) => {
                    for i in 0..n {
                        self.rxb[self.rxp] = buf[i];
                        self.rxp += 1;

                        if buf[i] == b'\n' || buf[i] == b'\r' {
                            match Cmd::parse(&self.rxb[..self.rxp]) {
                                Ok(c) => cmd = c,
                                Err(_) => continue,
                            }

                            self.rxp = 0;
                            end = true;
                        }
                    }
                },
                Err(_) => continue,
            }
        }

        if cmd == Cmd::Frame {
            let mut cur = self.rxp;

            for i in 0..cur {
                frame[i] = self.rxb[i];
            }

            self.rxp = 0;

            while cur != frame.len() {
                match self.usb.read_packet(&mut buf).await {
                    Ok(n) => {
                        for i in 0..n {
                            if cur == frame.len() {
                                self.rxb[self.rxp] = buf[i];
                                self.rxp += 1;
                            } else {
                                frame[cur] = buf[i];
                                cur += 1;
                            }
                        }
                    },
                    Err(_) => continue,
                }
            }
        }

        cmd
    }

    pub async fn write(&mut self, buf: &[u8]) {
        for ch in buf.chunks(PKT_SIZE as usize) {
            self.usb.write_packet(ch).await.ok();
        }
    }
}
