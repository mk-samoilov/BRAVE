use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const MAGIC: &[u8; 4] = b"BRAV";
const VERSION: u32 = 1;

pub const ASSET_TYPE_MODEL:   u32 = 0;
pub const ASSET_TYPE_TEXTURE: u32 = 1;
pub const ASSET_TYPE_SHADER:  u32 = 2;

const NAME_LEN: usize = 64;
const ENTRY_SIZE: usize = NAME_LEN + 4 + 8 + 8;
const HEADER_SIZE: usize = 4 + 4 + 4;

struct Entry {
    name:   String,
    offset: u64,
    size:   u64,
}

pub struct AstFile {
    path:    std::path::PathBuf,
    entries: Vec<Entry>,
}

impl AstFile {
    pub fn open(path: &Path) -> Self {
        let mut f = File::open(path)
            .unwrap_or_else(|e| panic!("Failed to open '{}': {}", path.display(), e));

        let mut magic = [0u8; 4];
        f.read_exact(&mut magic).unwrap();
        assert_eq!(&magic, MAGIC, "Bad magic in '{}'", path.display());

        let _version = read_u32(&mut f);
        let count    = read_u32(&mut f) as usize;

        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            let mut name_buf = [0u8; NAME_LEN];
            f.read_exact(&mut name_buf).unwrap();
            let end  = name_buf.iter().position(|&b| b == 0).unwrap_or(NAME_LEN);
            let name = String::from_utf8_lossy(&name_buf[..end]).into_owned();
            let _asset_type = read_u32(&mut f);
            let offset      = read_u64(&mut f);
            let size        = read_u64(&mut f);
            entries.push(Entry { name, offset, size });
        }

        AstFile { path: path.to_path_buf(), entries }
    }

    pub fn read_raw(&self, name: &str) -> Vec<u8> {
        let entry = self.entries.iter()
            .find(|e| e.name == name)
            .unwrap_or_else(|| panic!(
                "Asset '{}' not found in '{}'", name, self.path.display()
            ));

        let mut f = File::open(&self.path).unwrap();
        f.seek(SeekFrom::Start(entry.offset)).unwrap();
        let mut data = vec![0u8; entry.size as usize];
        f.read_exact(&mut data).unwrap();
        data
    }

    pub fn has(&self, name: &str) -> bool {
        self.entries.iter().any(|e| e.name == name)
    }
}

fn read_u32(f: &mut File) -> u32 {
    let mut b = [0u8; 4];
    f.read_exact(&mut b).unwrap();
    u32::from_le_bytes(b)
}

fn read_u64(f: &mut File) -> u64 {
    let mut b = [0u8; 8];
    f.read_exact(&mut b).unwrap();
    u64::from_le_bytes(b)
}

pub fn data_offset(entry_count: usize) -> u64 {
    (HEADER_SIZE + entry_count * ENTRY_SIZE) as u64
}

pub fn write_header(buf: &mut Vec<u8>, entry_count: u32) {
    buf.extend_from_slice(MAGIC);
    buf.extend_from_slice(&VERSION.to_le_bytes());
    buf.extend_from_slice(&entry_count.to_le_bytes());
}

pub fn write_entry(buf: &mut Vec<u8>, name: &str, asset_type: u32, offset: u64, size: u64) {
    let mut name_buf = [0u8; NAME_LEN];
    let bytes = name.as_bytes();
    let len = bytes.len().min(NAME_LEN - 1);
    name_buf[..len].copy_from_slice(&bytes[..len]);
    buf.extend_from_slice(&name_buf);
    buf.extend_from_slice(&asset_type.to_le_bytes());
    buf.extend_from_slice(&offset.to_le_bytes());
    buf.extend_from_slice(&size.to_le_bytes());
}
