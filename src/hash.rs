extern crate crypto;
extern crate hex;

use std::path::{PathBuf};
use crate::common::{bytes_to_hex, hex_to_bytes};
use self::crypto::digest::Digest;
use self::crypto::sha1::Sha1;

#[derive(Debug,PartialEq,Clone,Copy,Default,Ord,PartialOrd,Eq)]
pub struct Hash([u8;20]);

impl Hash {
    pub fn from(bytes: &[u8]) -> Option<Hash> {
        if bytes.len() != 20 {
            return None;
        }
        let mut hash: [u8; 20] = Default::default();
        hash.copy_from_slice(bytes);
        Some(Hash(hash))
    }
    pub fn from_string(string: &str) -> Option<Hash> {
        if string.len() != 40 {
            return None;
        }
        let bytes = hex_to_bytes(&string.to_string())?;
        Self::from(&bytes)
    }

    pub fn bytes(&self) -> [u8;20] {
        return self.0
    }

    pub fn string(&self) -> String {
        return bytes_to_hex(&self.0)
    }

    pub fn generate_path(&self) -> PathBuf {
        let s = self.string();
        let prefix = &s[0..2];
        let rest = &s[2..];
        PathBuf::from(format!("{}/{}", prefix, rest))
    }
}

pub fn calc_sha1_string(byte: &[u8]) -> String {
    let mut hasher = Sha1::new();

    hasher.input(byte);

    let result = hasher.result_str();
    println!("{}", result);
    result
}

#[test]
fn test_calc_sha1_string() {
    let input_byte = "hello world".as_bytes();
    let hex = calc_sha1_string(input_byte);
    assert_eq!(hex, "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed");
}

pub fn calc_sha1_bytes(byte: &[u8]) -> Hash {
    let mut hasher = Sha1::new();

    hasher.input(byte);

    let mut result:[u8;20] = [0;20];
    hasher.result(&mut result);
    Hash(result)
}


#[test]
fn test_calc_sha1_bytes() {
    let input_byte = "hello world".as_bytes();
    let hex = calc_sha1_bytes(input_byte);
    assert_eq!(hex.bytes(), [0x2au8, 0xae, 0x6c, 0x35, 0xc9, 0x4f, 0xcf, 0xb4, 0x15, 0xdb,
        0xe9, 0x5f, 0x40, 0x8b, 0x9c, 0xe9, 0x1e, 0xe8, 0x46, 0xed]);
}

fn is_valid_sha1(string: &str)-> bool {
    if string.len() != 40 {
        return false;
    }
    hex::decode(string).is_ok()
}

pub fn path_from_hash(hash: &str) -> Result<PathBuf, String> {
    if !is_valid_sha1(hash) {
        return Err("invalid string".to_string());
    }
    let begining = &hash[0..2];
    let rest = &hash[2..];
    let path = PathBuf::from(format!("{}/{}", begining, rest));

    Ok(path)
}

#[test]
fn test_path_from_hash() {
    let input = "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed";
    let out = path_from_hash(input).unwrap();
    assert_eq!(out.as_path(), Path::new("2a/ae6c35c94fcfb415dbe95f408b9ce91ee846ed"))
}


