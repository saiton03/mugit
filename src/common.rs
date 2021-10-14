pub const DEFAULT_BRANCH_NAME: &str="master";

use std::env;
use std::fs::canonicalize;
use std::path::{PathBuf};

pub fn get_project_root()-> Result<PathBuf, String> {
    let curr_path = env::current_dir().unwrap();
    get_project_root_from(&curr_path)
}

pub fn get_project_root_from(path: &PathBuf) -> Result<PathBuf, String> {
    search_project_root(path.clone())
}

fn search_project_root(current_dir: PathBuf)-> Result<PathBuf, String> {
    if current_dir.join(".git").exists() {
        return canonicalize(current_dir).map_err(|e| e.to_string());
    }

    match current_dir.parent(){
        Some(s)=> search_project_root(s.to_path_buf()),
        None=> Err(String::from("no .git/ found")) // reach root dir
    }
}

pub fn get_path_from_project_root(path: &PathBuf) -> Result<PathBuf, String> {
    let project_root = get_project_root()?;
    get_path_from(path, &project_root)
}

pub fn get_path_from(path: &PathBuf, base_path: &PathBuf) -> Result<PathBuf, String> {
    let p = canonicalize(path.clone()).map_err(|e| e.to_string())?;
    let trimmed = p.strip_prefix(base_path).map_err(|e| e.to_string())?;

    Ok(trimmed.to_path_buf())
}

#[test]
fn test_get_path_from_project_root() {
    let path = PathBuf::from("./src/main.rs");
    let out = get_path_from_project_root(&path).expect("error");
    assert_eq!(out.as_path().to_str().expect("convert str error"), "src/main.rs")
}

// 文字列系
fn byte_to_hex(byte: &u8) -> String {
    let mut ret = Vec::new();
    let big = byte/16;
    if big<10 {
        ret.push('0' as u8 + big);
    }else{
        ret.push('a' as u8 + big-10);
    }
    let small = byte%16;
    if small<10 {
        ret.push('0' as u8 + small);
    }else{
        ret.push('a' as u8 + small-10);
    }
    String::from_utf8(ret).unwrap()
}

#[test]
fn test_byte_to_hex() {
    let tests = [
        (0u8, "00"),
        (74u8,"4a")
    ];
    for t in tests {
        let out = byte_to_hex(&t.0);
        assert_eq!(out, t.1.to_string());
    }
}

pub fn bytes_to_hex(byte: &[u8]) -> String {
    let mut ret = String::new();
    for i in byte {
        ret = ret.clone()+&byte_to_hex(i);
    }
    ret
}

#[test]
fn test_bytes_to_hex() {
    let tests = [6u8, 74, 146, 215, 131, 249, 152, 81, 209, 81, 123, 81, 186, 11, 42, 237, 74,
        29, 49, 40];
    let out = bytes_to_hex(&tests);
    assert_eq!(out, "064a92d783f99851d1517b51ba0b2aed4a1d3128".to_string());
}

// big endian
fn hex_to_u8(hex: char) -> u8 {
    if hex>='a' && hex<='f' {
        return hex as u8 - 'a' as u8 + 10;
    }
    hex as u8 -'0' as u8
}
pub fn hex_to_bytes(hex_str: &String) -> Option<Vec<u8>> {
    let len = hex_str.len();
    if len%2==1 {
        return None;
    }
    let mut ret: Vec<u8> = Vec::new();
    for i in (0..len).step_by(2) {
        ret.push(hex_to_u8(hex_str.chars().nth(i).unwrap())*16+
            hex_to_u8(hex_str.chars().nth(i+1).unwrap()));
    }
    Some(ret)
}

#[test]
fn test_hex_to_bytes() {
    let a = "f012".to_string();
    let out = hex_to_bytes(&a).unwrap();
    assert_eq!(out, vec![240, 18])
}

pub fn extract_until_null(byte: &[u8]) -> Vec<u8> {
    for (i, v) in byte.iter().enumerate() {
        if *v == 0 {
            return Vec::from(&byte[..i]);
        }
    }
    Vec::from(byte)
}

#[test]
fn test_extract_until_null() {
    let a = ['a' as u8, 'b' as u8, 0, 'c' as u8];
    let out = extract_until_null(&a);
    let s =String::from_utf8(out).unwrap();
    assert_eq!(s, "ab");
}

pub fn bytes_to_u32(bytes: &[u8]) -> Option<u32> {
    if bytes.len() != 4 {
        return None;
    }
    let mut ret: u32 = 0;
    for  i in 0..4 {
       ret += (bytes[i] as u32) << (8*(3-i));
    }
    Some(ret)
}

#[test]
fn test_bytes_to_u32() {
    let input = [0x1a, 0x35, 0x2b, 0x80];
    let out = bytes_to_u32(&input).unwrap();
    assert_eq!(out, 439692160)
}

pub fn bytes_to_u16(bytes: &[u8]) -> Option<u16> {
    if bytes.len() != 2 {
        return None;
    }
    let mut ret: u16 = 0;
    for  i in 0..2 {
        ret += (bytes[i] as u16) << (8*(1-i));
    }
    Some(ret)
}

pub fn u16_to_bytes(val: u16) -> Vec<u8> {
    let t_size: usize = 2;
    let mut ret = Vec::new();

    for i in 0..t_size {
        let byte = &val >> (8*(t_size-1-i));
        ret.push(byte as u8);
    }
    ret
}

pub fn u32_to_bytes(val: u32) -> Vec<u8> {
    let t_size: usize = 4;
    let mut ret = Vec::new();

    for i in 0..t_size {
        let byte = &val >> (8*(t_size-1-i));
        ret.push(byte as u8);
    }
    ret
}
