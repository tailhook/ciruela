pub enum Signature {
    SshEd25519([u8; 64]),
}

pub struct SigData<'a> {
    pub path: &'a str,
    pub image: &'a [u8],
    pub timestamp: u64,
}

pub fn sign_default(src: SigData) -> Signature {
    unimplemented!();
}
