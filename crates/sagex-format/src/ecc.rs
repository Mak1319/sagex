use crate::format::PrivateFileFormatInternal;
use binrw::BinWrite;

pub fn calculate_ecc(ke: &PrivateFileFormatInternal) -> Vec<u8> {
    let mut buf = Vec::new();
    ke.write(&mut std::io::Cursor::new(&mut buf)).unwrap();
    vec![]
}
