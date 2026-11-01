use esp_storage::FlashStorage;

use crate::cap::Credentials;

const OFFSET: u32 = 0x3F_F000;
const MAGIC: u32 = 0x676C_696D;
const RECORD_SIZE: usize = 108;

const SSID_LEN: usize = 4;
const PASS_LEN: usize = 5;
const SSID: usize = 8;
const PASS: usize = SSID + 32;
const CHECKSUM: usize = PASS + 64;

#[repr(align(4))]
struct Record([u8; RECORD_SIZE]);

fn checksum(buf: &[u8]) -> u32 {
    buf.iter().fold(0u32, |acc, &b| acc.wrapping_add(u32::from(b)))
}

pub fn load(fs: &mut FlashStorage) -> Option<Credentials> {
    let mut rec = Record([0; RECORD_SIZE]);
    fs.read_nor(OFFSET, &mut rec.0).ok()?;

    let buf = &rec.0;

    if u32::from_le_bytes(buf[0..4].try_into().ok()?) != MAGIC {
        return None;
    }

    let ssid_len = usize::from(buf[SSID_LEN]);
    let pass_len = usize::from(buf[PASS_LEN]);

    if ssid_len == 0 || ssid_len > 32 || pass_len > 64 {
        return None;
    }

    if u32::from_le_bytes(buf[CHECKSUM..].try_into().ok()?) != checksum(&buf[SSID_LEN..CHECKSUM]) {
        return None;
    }

    let ssid = str::from_utf8(&buf[SSID..SSID + ssid_len]).ok()?;
    let pass = str::from_utf8(&buf[PASS..PASS + pass_len]).ok()?;

    Some(Credentials::new(ssid, pass))
}

pub fn store(fs: &mut FlashStorage, cred: &Credentials) {
    let mut rec = Record([0xFF; RECORD_SIZE]);
    let buf = &mut rec.0;

    let ssid = cred.ssid().as_bytes();
    let pass = cred.pass().as_bytes();

    buf[0..4].copy_from_slice(&MAGIC.to_le_bytes());
    buf[SSID_LEN] = ssid.len() as u8;
    buf[PASS_LEN] = pass.len() as u8;
    buf[SSID..SSID + ssid.len()].copy_from_slice(ssid);
    buf[PASS..PASS + pass.len()].copy_from_slice(pass);

    let sum = checksum(&buf[SSID_LEN..CHECKSUM]);
    buf[CHECKSUM..].copy_from_slice(&sum.to_le_bytes());

    if let Err(e) = fs
        .erase(OFFSET, OFFSET + FlashStorage::SECTOR_SIZE)
        .and_then(|()| fs.write_nor(OFFSET, buf))
    {
        esp_println::println!("nvm store error: {:?}", e);
    }
}
