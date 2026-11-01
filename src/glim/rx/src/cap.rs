use alloc::format;
use core::fmt::{Debug, Display};
use core::sync::atomic::Ordering;

use edge_http::Method;
use edge_http::io::Error as HttpError;
use edge_http::io::server::{Connection, Handler};
use embassy_net::Stack;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Sender;
use embedded_io_async::{Read, Write};

use crate::PAGE;

pub struct Credentials {
    ssid: [u8; 256],
    ssid_len: usize,
    pass: [u8; 256],
    pass_len: usize,
}

impl Credentials {
    pub fn new(ssid: &str, pass: &str) -> Credentials {
        let mut cred = Credentials { ssid: [0; 256], ssid_len: 0, pass: [0; 256], pass_len: 0 };

        cred.ssid[..ssid.len()].copy_from_slice(ssid.as_bytes());
        cred.ssid_len = ssid.len();
        cred.pass[..pass.len()].copy_from_slice(pass.as_bytes());
        cred.pass_len = pass.len();

        cred
    }

    pub fn ssid(&self) -> &str {
        str::from_utf8(&self.ssid[..self.ssid_len]).unwrap()
    }

    pub fn pass(&self) -> &str {
        str::from_utf8(&self.pass[..self.pass_len]).unwrap()
    }

    pub fn default() -> Credentials {
        Credentials::new("default", "default")
    }

    fn parse(path: &str) -> Option<Credentials> {
        let params = path.strip_prefix("/set/")?;
        let (ssid, pass) = params.split_once('/')?;

        if ssid.is_empty() || pass.contains('/') {
            return None;
        }

        let mut ssid_buf = [0; 32];
        let mut pass_buf = [0; 64];
        let ssid = Self::decode(ssid, &mut ssid_buf)?;
        let pass = Self::decode(pass, &mut pass_buf)?;

        Some(Credentials::new(ssid, pass))
    }

    fn decode<'a>(value: &str, buf: &'a mut [u8]) -> Option<&'a str> {
        let bytes = value.as_bytes();
        let mut pos = 0;
        let mut len = 0;

        while pos < bytes.len() {
            if len == buf.len() {
                return None;
            }

            buf[len] = if bytes[pos] == b'%' {
                let high = Self::hex(*bytes.get(pos + 1)?)?;
                let low = Self::hex(*bytes.get(pos + 2)?)?;
                pos += 3;
                (high << 4) | low
            } else {
                let byte = bytes[pos];
                pos += 1;
                byte
            };

            len += 1;
        }

        core::str::from_utf8(&buf[..len]).ok()
    }

    fn hex(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }
}

pub struct CaptureHandler {
    pub snd: Sender<'static, CriticalSectionRawMutex, Credentials, 1>,
    pub sta: Stack<'static>,
}

impl Handler for CaptureHandler {
    type Error<E> = HttpError<E>
    where
        E: Debug;

    async fn handle<T, const N: usize>(
        &self,
        _task_id: impl Display + Copy,
        conn: &mut Connection<'_, T, N>,
    ) -> Result<(), Self::Error<T::Error>>
    where
        T: Read + Write,
    {
        let headers = conn.headers()?;

        if headers.method != Method::Get {
            conn.initiate_response(
                405,
                Some("Method Not Allowed"),
                &[("Connection", "close")],
            ).await?;

            return Ok(());
        }

        esp_println::println!("web: {}", headers.path);

        match headers.path {
            "/" => {
                conn.initiate_response(
                    200,
                    Some("OK"),
                    &[
                        ("Content-Type", "text/html"),
                        ("Content-Encoding", "gzip"),
                        ("Connection", "close"),
                    ],
                ).await?;

                conn.write_all(PAGE).await?;
            }
            "/ip" => {
                let ip = self.sta.config_v4().map(|config| {
                    format!("{}", config.address.address())
                });

                conn.initiate_response(
                    200,
                    Some("OK"),
                    &[
                        ("Content-Type", "text/plain"),
                        ("Cache-Control", "no-store"),
                        ("Connection", "close"),
                    ],
                ).await?;

                if let Some(ip) = ip {
                    conn.write_all(ip.as_bytes()).await?;
                } else {
                    conn.write_all(b"Not acquired").await?;
                }
            }
            "/pixels" => {
                let pixels = crate::PIXELS.load(Ordering::Relaxed);

                conn.initiate_response(
                    200,
                    Some("OK"),
                    &[
                        ("Content-Type", "text/plain"),
                        ("Cache-Control", "no-store"),
                        ("Connection", "close"),
                    ],
                ).await?;

                if pixels > 0 {
                    conn.write_all(format!("{}", pixels).as_bytes()).await?;
                } else {
                    conn.write_all(b"Unknown").await?;
                }
            }
            path => match Credentials::parse(path) {
                Some(cred) => {
                    conn.initiate_response(
                        204,
                        Some("No Content"),
                        &[("Connection", "close")],
                    ).await?;

                    self.snd.send(cred).await;
                }
                None if path.starts_with("/set/") => {
                    conn.initiate_response(
                        400,
                        Some("Bad Request"),
                        &[("Connection", "close")],
                    ).await?;
                }
                None => {
                    conn.initiate_response(
                        404,
                        Some("Not Found"),
                        &[("Connection", "close")],
                    ).await?;
                }
            },
        }

        Ok(())
    }
}
