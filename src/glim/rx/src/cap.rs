
pub struct Credentials {
    ssid: [u8; 256],
    ssid_len: usize,
    pass: [u8; 256],
    pass_len: usize,
}

impl<'a> Credentials {
    pub fn new(ssid: &str, pass: &str) -> Credentials {
        let mut cred = Credentials { ssid: [0; 256], ssid_len: 0, pass: [0; 256], pass_len: 0 };

        cred.ssid[..ssid.len()].copy_from_slice(ssid.as_bytes());
        cred.ssid_len = ssid.len();
        cred.pass[..pass.len()].copy_from_slice(pass.as_bytes());
        cred.pass_len = pass.len();

        cred
    }

    pub fn ssid(&'a self) -> &'a str {
        str::from_utf8(&self.ssid[..self.ssid_len]).unwrap()
    }

    pub fn pass(&'a self) -> &'a str {
        str::from_utf8(&self.pass[..self.pass_len]).unwrap()
    }
}

pub fn default() -> Credentials {
    let ssid = "KaHypa";
    let pass = "diy@or@die";

    let mut cred = Credentials { ssid: [0; 256], ssid_len: 0, pass: [0; 256], pass_len: 0 };
    cred.ssid[..ssid.len()].copy_from_slice(ssid.as_bytes());
    cred.ssid_len = ssid.len();
    cred.pass[..pass.len()].copy_from_slice(pass.as_bytes());
    cred.pass_len = pass.len();

    cred
}
