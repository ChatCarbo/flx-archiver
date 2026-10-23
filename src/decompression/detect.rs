use std::fs::File;
use std::io::{Read, Result as IoResult};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    Zip,
    SevenZ,
    Zstd,
    Gzip,
    Rar,
    Unknown,
}

pub struct FormatDetector;

impl FormatDetector {
    pub fn detect_format<P: AsRef<Path>>(path: P) -> IoResult<ArchiveFormat> {
        let mut file = File::open(path)?;
        let mut buffer = [0u8; 8];

        let bytes_read = file.read(&mut buffer)?;
        if bytes_read < 2 {
            return Ok(ArchiveFormat::Unknown);
        }

        // Signature Magic Bytes
        match buffer {
            // ZIP : PK\x03\x04 ou PK\x05\x06 (archive vide/spanned)
            [0x50, 0x4B, 0x03, 0x04, ..] | [0x50, 0x4B, 0x05, 0x06, ..] => Ok(ArchiveFormat::Zip),
            
            // 7z : 7z¼¯'
            [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C, ..] => Ok(ArchiveFormat::SevenZ),
            
            // Zstandard : 0x28B52FFD
            [0x28, 0xB5, 0x2F, 0xFD, ..] => Ok(ArchiveFormat::Zstd),
            
            // Gzip : \x1f\x8b
            [0x1F, 0x8B, ..] => Ok(ArchiveFormat::Gzip),
            
            // RAR v4.x (\x52\x61\x72\x21\x1a\x07\x00) ou v5.0 (\x52\x61\x72\x21\x1a\x07\x01\x00)
            [0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, ..] => Ok(ArchiveFormat::Rar),
            
            _ => Ok(ArchiveFormat::Unknown),
        }
    }
}