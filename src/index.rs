use std::cmp::min;
use std::collections::BTreeMap;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::str::FromStr;
use crate::common::{bytes_to_u16, bytes_to_u32, extract_until_null, get_path_from_project_root, u16_to_bytes, u32_to_bytes};
use super::hash::Hash;


// Index format https://github.com/git/git/blob/v2.12.0/Documentation/technical/index-format.txt
#[derive(Debug, PartialEq, Default, Clone)]
pub struct Index {
    version: u32,
    entry_num: u32,
    entries: BTreeMap<PathBuf, IndexEntry>,
}


impl Index {
    pub fn new() -> Self {
        Self {
            version: 2,
            entry_num: 0,
            entries: Default::default()
        }
    }

    pub fn from(bytes: &[u8]) -> Option<Self> {
        if !bytes.starts_with("DIRC".as_bytes()) {
            return None;
        }
        let len = bytes.len();
        let mut offset: usize= 4;
        let version = bytes_to_u32(&bytes[offset..offset+4])?;
        offset += 4;
        let entry_num = bytes_to_u32(&bytes[offset..offset+4])?;
        offset += 4;
        let mut entries: BTreeMap<PathBuf, IndexEntry> = BTreeMap::new();
        while offset < len && entries.len() < entry_num as usize {
            let out = IndexEntry::from(&bytes[offset..])?;
            entries.insert(out.0.file_name.clone(), out.0);
            offset += out.1;
        }
        Some(Self {
            version,
            entry_num,
            entries
        })
    }

    pub fn from_file(proj_root: &PathBuf) -> Option<Self> {
        let index_path = proj_root.join(".git/index");
        let mut file = File::open(index_path).ok()?;
        let mut buf = Vec::new();
        let _ = file.read_to_end(&mut buf).ok()?;
        Self::from(&buf)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::from("DIRC".as_bytes());
        buf.append(&mut u32_to_bytes(self.version));
        buf.append(&mut u32_to_bytes(self.entry_num));

        for entry in &self.entries {
            buf.append(&mut entry.1.to_bytes());
        }

        buf
    }

    pub fn add_entry(&mut self, path: &PathBuf, hash: Hash) -> Result<(),String>{
        let ie = IndexEntry::from_file(path, hash)?;
        self.entries.insert(ie.file_name.clone(), ie);
        self.update_entry_num();
        Ok(())
    }

    pub fn get_entry(& self, path: PathBuf) -> Option<IndexEntry> {
        let key = get_path_from_project_root(&path).ok()?;
        let ret = self.entries.get(key.as_path())?;
        Some(ret.clone())
    }

    pub fn entries(&self) -> BTreeMap<PathBuf, IndexEntry> {
        self.entries.clone()
    }

    pub fn delete_entry(&mut self, path_from_root: &PathBuf) -> Result<(),String>{
        self.entries.remove(path_from_root.as_path());
        self.update_entry_num();
        Ok(())
    }

    fn update_entry_num(&mut self) {
        self.entry_num = self.entries.len() as u32;
    }


}


#[derive(Debug, PartialEq, Default, Clone)]
pub struct IndexEntry {
    ctime: u32,
    ctime_nano: u32,
    mtime: u32,
    mtime_nano: u32,
    dev: u32,
    inode: u32,
    mode: u32,
    uid: u32,
    gid: u32,
    size: u32,
    hash: Hash,
    flags: u16,
    file_name: PathBuf,
}

impl IndexEntry {
    fn from(bytes: &[u8]) -> Option<(Self, usize)> {
        let mut pos: usize = 0;
        let ctime = bytes_to_u32(&bytes[pos..pos + 4])?;
        pos+=4;
        let ctime_nano = bytes_to_u32(&bytes[pos..pos+4])?;
        pos+=4;
        let mtime = bytes_to_u32(&bytes[pos..pos+4])?;
        pos+=4;
        let mtime_nano = bytes_to_u32(&bytes[pos..pos+4])?;
        pos+=4;
        let dev = bytes_to_u32(&bytes[pos..pos+4])?;
        pos+=4;
        let inode = bytes_to_u32(&bytes[pos..pos+4])?;
        pos+=4;
        let mode = bytes_to_u32(&bytes[pos..pos+4])?;
        pos+=4;
        let uid = bytes_to_u32(&bytes[pos..pos+4])?;
        pos+=4;
        let gid = bytes_to_u32(&bytes[pos..pos+4])?;
        pos+=4;
        let size = bytes_to_u32(&bytes[pos..pos+4])?;
        pos+=4;
        let hash = Hash::from(&bytes[pos..pos+20])?;
        pos+=20;
        let flags = bytes_to_u16(&bytes[pos..pos+2])?;
        pos+=2;
        let file_name_str = String::from_utf8(extract_until_null(&bytes[pos..])).ok()?;
        let file_name= PathBuf::from_str(&file_name_str).ok()?;
        pos+=file_name_str.len();
        let len = (pos/8+1)*8;

        Some((IndexEntry{
            ctime,
            ctime_nano,
            mtime,
            mtime_nano,
            dev,
            inode,
            mode,
            uid,
            gid,
            size,
            hash,
            flags,
            file_name
        }, len))
    }

    pub fn from_file(path: &PathBuf, hash: Hash) -> Result<Self,String> {
        let metadata = fs::metadata(path).map_err(|e| e.to_string())?;

        let ctime = metadata.ctime() as u32;
        let ctime_nano = metadata.ctime_nsec() as u32;
        let mtime = metadata.mtime() as u32;
        let mtime_nano = metadata.mtime_nsec() as u32;
        let dev = metadata.dev() as u32;
        let inode = metadata.ino() as u32;
        let mode = metadata.mode();
        let uid = metadata.uid();
        let gid = metadata.gid();
        let size = metadata.size() as u32;
        let file_name = get_path_from_project_root(path)?;
        let flags = min(file_name.to_str().ok_or("convert path to string failed")?.len(),
                        0xfff) as u16;
        
        Ok(Self{
            ctime,
            ctime_nano,
            mtime,
            mtime_nano,
            dev,
            inode,
            mode,
            uid,
            gid,
            size,
            hash,
            flags,
            file_name
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut ret = Vec::new();
        ret.append(&mut u32_to_bytes(self.ctime));
        ret.append(&mut u32_to_bytes(self.ctime_nano));
        ret.append(&mut u32_to_bytes(self.mtime));
        ret.append(&mut u32_to_bytes(self.mtime_nano));
        ret.append(&mut u32_to_bytes(self.dev));
        ret.append(&mut u32_to_bytes(self.inode));
        ret.append(&mut u32_to_bytes(self.mode));
        ret.append(&mut u32_to_bytes(self.uid));
        ret.append(&mut u32_to_bytes(self.gid));
        ret.append(&mut u32_to_bytes(self.size));
        ret.append(&mut self.hash.bytes().to_vec());
        ret.append(&mut u16_to_bytes(self.flags));
        ret.append(&mut self.file_name.to_str().unwrap().as_bytes().to_vec());

        let zero_pad_len = if ret.len() % 8 == 0 {
            8
        }else {
            8-ret.len()%8
        };
        ret.append(&mut vec![0u8; zero_pad_len]);


        ret
    }

    pub fn mod_time(&self) -> u64 {
        let a = ((self.mtime as u64) << 32) + self.mtime_nano as u64;
        a
    }

    pub fn file_type(&self) -> u8 {
        ((self.mode >> 12) & 0b1111) as u8
    }

    pub fn permission(&self) -> u16 {
        (self.mode & 0b111_111_111) as u16
    }

    pub fn file_name(&self) -> String {
        self.file_name.file_name().unwrap().to_str().unwrap().to_string()
    }

    pub fn file_path(&self) -> String {
        self.file_name.to_str().unwrap().to_string()
    }

    pub fn hash(&self) -> Hash {
        self.hash
    }
}

#[test]
fn test_index_entry_from() {
    let input = vec!(0x61u8, 0x61, 0x26, 0x33, 0x0e, 0xfd, 0xac, 0x2d, 0x61, 0x61, 0x26, 0x33, 0x0e,
        0xfd, 0xac, 0x2d, 0x01, 0x00, 0x00, 0x04, 0x05, 0xb6, 0x93, 0x32, 0x00, 0x00, 0x81, 0xa4,
        0x00, 0x00, 0x01, 0xf5, 0x00, 0x00, 0x00, 0x14, 0x00, 0x00, 0x00, 0x03, 0x97, 0x66, 0x47,
        0x5a, 0x41, 0x85, 0xa1, 0x51, 0xdc, 0x9d, 0x56, 0xd6, 0x14, 0xff, 0xb9, 0xaa, 0xea, 0x3b,
        0xfd, 0x42, 0x00, 0x06, 0x6f, 0x6b, 0x2e, 0x74, 0x78, 0x74, 0x00, 0x00, 0x00, 0x00);
    let out = IndexEntry::from(&input).expect("error");
    assert_eq!(out.0, IndexEntry{
        ctime: 1633756723,
        ctime_nano: 251505709,
        mtime: 1633756723,
        mtime_nano: 251505709,
        dev: 16777220,
        inode: 95851314,
        mode: 33188,
        uid: 501,
        gid: 20,
        size: 3,
        hash: Hash::from_string("9766475a4185a151dc9d56d614ffb9aaea3bfd42").unwrap(),
        flags: 6,
        file_name: PathBuf::from("ok.txt"),
    });
    assert_eq!(out.1, 72 as usize);
}
