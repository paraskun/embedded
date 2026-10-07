use embassy_usb::{
    Builder, Config, class::{cdc_acm::{CdcAcmClass, State}}, driver::Driver
};
use static_cell::StaticCell;

const PKT_SIZE: usize = 64;

static STATE: StaticCell<State<'static>> = StaticCell::new();
static CFG_BUF: StaticCell<[u8; 256]> = StaticCell::new();
static BOS_BUF: StaticCell<[u8; 256]> = StaticCell::new();
static CTL_BUF: StaticCell<[u8; 64]> = StaticCell::new();

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
    pub fn new(driver: D) -> Interface<D> {
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

        let _ = builder.build();

        Interface { usb, rxb: [0; PKT_SIZE], rxp: 0 }
    }

    pub async fn read(&mut self, _frame: &mut [u8]) -> Cmd {
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

                        if buf[i] == b'\n' {
                            self.rxp = 0;

                            match Cmd::parse(&self.rxb[..self.rxp]) {
                                Ok(c) => cmd = c,
                                Err(_) => continue,
                            }

                            end = true;
                        }
                    }
                },
                Err(_) => continue,
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