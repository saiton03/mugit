extern crate flate2;
extern crate regex;

use regex::Regex;
use chrono::{DateTime, Local, FixedOffset, TimeZone};
use std::fs;
use std::io::{Read, Write};
use std::path::{PathBuf};
use flate2::Compression;
use flate2::write::ZlibEncoder;
use crate::hash::{Hash, calc_sha1_bytes, calc_sha1_string};
use crate::index::IndexEntry;
use crate::object::CommitterType::{Author, Committer};
use crate::object::FilePermission::{Executable, UnExecutable};
use crate::object::FileType::{Directory, File, Submodule, SymbolicLink};
use self::flate2::read::ZlibDecoder;
use super::common::{extract_until_null};


#[derive(Debug,PartialEq)]
pub enum ObjType {
    Blob,
    Tree,
    Commit,
}

impl Default for ObjType {
    fn default() -> Self {
        ObjType::Blob
    }
}


fn depress_zlib(byte: &[u8]) -> Result<Vec<u8>,String> {
    let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
    e.write_all(byte).map_err(|e| e.to_string())?;
    let out = e.finish().map_err(|e| e.to_string())?;
    Ok(out)
}

fn extract_zlib(byte: &[u8]) -> Result<Vec<u8>, String> {
    let mut d = ZlibDecoder::new(&byte[..]);
    let mut s: Vec<u8> = Vec::new();
    d.read_to_end(&mut s).map_err(|e| e.to_string())?;
    Ok(s)
}


#[test]
fn test_zlib() {
    let input = "test".as_bytes();
    let enc = depress_zlib(input).unwrap();
    println!("{}", enc.len());
    let out = extract_zlib(&enc).unwrap();
    println!("{}", out.len());
    assert_eq!(out, input);
}


#[derive(Default,Debug,PartialEq)]
pub struct Blob {
    obj_type: ObjType,
    len: usize,
    data: Vec<u8>,

    payload: Vec<u8>,
    pub hash: Hash,
}


impl Blob {
    pub fn new(data: &Vec<u8>) -> Blob{
        let len = data.len();
        let mut payload = format!("blob {}\0", len).into_bytes();
        payload.extend(data);
        let hash = calc_sha1_bytes(&payload);
        Blob{
            obj_type: ObjType::Blob,
            data: data.clone(),
            len,
            payload,
            hash
        }
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Option<Blob> {
        if !bytes.starts_with("blob".as_bytes()) {
            return None
        }

        let len_string = String::from_utf8(extract_until_null(&bytes[5..])).
            ok()?;
        let len: usize = len_string.parse().ok()?;
        let header_len: usize = "blob ".len()+len_string.len()+1;
        let body = &bytes[header_len..];
        let hash = calc_sha1_bytes(&bytes);
        Some(Blob{
            obj_type: ObjType::Blob,
            len,
            data: body.to_vec(),
            payload: bytes,
            hash
        })
    }

    pub fn from_file(path: &PathBuf) -> Option<Self> {
        let mut file = fs::File::open(path).ok()?;
        let mut buf = Vec::new();
        let _ = file.read_to_end(&mut buf).ok()?;
        Some(Self::new(&buf))
    }

    pub fn generate_digest_string(&self) -> String {
        calc_sha1_string(&*self.payload)
    }
    pub fn generate_digest_bytes(&self) -> Hash {
        calc_sha1_bytes(&*self.payload)
    }

    pub fn generate_depress(&self) -> Result<Vec<u8>, String> {
        depress_zlib(&*self.payload)
    }


}

#[test]
fn test_blob() {
    let input_byte = String::from("ohayo").into_bytes();
    let mut b = Blob::new(&input_byte);
    b.generate_depress().unwrap();

    let sha = b.generate_digest_string();

    assert_eq!(sha, "e7c23f4e29dc1ae1bc1e8807bb2838d0c9fb6ab5")
}

#[test]
fn test_blob_from_bytes() {
    let bytes = vec![98u8, 108, 111, 98, 32, 49, 50, 0,
                     104, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100, 10];
    let out =  Blob::from_bytes(bytes).unwrap();
    assert_eq!(out, Blob{
        obj_type: ObjType::Blob,
        len: 12,
        data: vec![104u8, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100, 10],
        payload: vec![98u8, 108, 111, 98, 32, 49, 50, 0,
                      104, 101, 108, 108, 111, 32, 119, 111, 114, 108, 100, 10],
        hash: Hash::from(&[59u8, 24, 229, 18, 219, 167, 158, 76, 131, 0, 221, 8, 174, 179, 127, 142, 114, 139, 141, 173]).unwrap(),
    });
}

#[derive(Default,Debug,PartialEq)]
pub struct Tree {
    obj_type: ObjType,

    nodes: Vec<TreeNode>,

    payload: Vec<u8>,
    hash: Option<Hash>,
}

impl Tree {
    pub fn new() -> Self {
        Self {
            obj_type: ObjType::Tree,
            nodes: vec![],
            payload: vec![],
            hash: None
        }
    }
    pub fn from_bytes(byte: &[u8]) -> Option<Tree> {
        // validate file_type
        if !byte.starts_with("tree".as_bytes()) {
            return None
        }

        let len_string = String::from_utf8(extract_until_null(&byte[5..])).
            ok()?;
        let len: usize = len_string.parse().ok()?;
        let header_len: usize = "tree ".len()+len_string.len()+1;
        let body = &byte[header_len..];
        let mut offset: usize = 0;

        let mut nodes:Vec<TreeNode> = Vec::new();
        while offset < len {
            let (node, n) = TreeNode::parse(&body[offset..].to_vec()).ok()?;
            nodes.push(node);
            if n==0 {
                return None;
            }
            offset+=n;
        }

        let hash = Hash::from(byte)?;

        Some(Tree {
            obj_type: ObjType::Tree,
            nodes,
            payload: byte.to_vec(),
            hash: Some(hash)
        })
    }
    pub fn from_depressed_bytes(byte: &[u8]) -> Option<Tree> {
        let extracted_byte = extract_zlib(byte).ok()?;
        Self::from_bytes(&extracted_byte)
    }

    pub fn add_tree_node(&mut self, node: TreeNode) {
        self.nodes.push(node);
    }

    pub fn as_bytes(&self) -> Vec<u8>{
        let body: Vec<u8> = self.nodes.iter().
            map(|x| x.to_bytes()).collect::<Vec<_>>().concat();
        let len = body.len();
        let header = format!("tree {}\0", len).into_bytes();
        header.into_iter().chain(body.into_iter()).collect::<Vec<_>>()
    }

    pub fn calc_bytes_and_hash(&mut self) -> (Vec<u8>, Hash) {
        let bytes = self.as_bytes();
        self.hash = Some(calc_sha1_bytes(&bytes));
        (bytes, self.hash.unwrap())
    }

    pub fn calc_hash(&mut self) -> Hash {
        match self.hash {
            Some(hash) => hash,
            None => self.calc_bytes_and_hash().1,
        }
    }

    pub fn hash(&self) -> Option<Hash> {
        self.hash
    }

    pub fn generate_depress(&self) -> Result<Vec<u8>, String> {
        depress_zlib(&*self.as_bytes())
    }

}

#[test]
fn test_tree_from_bytes() {
    let bytes = [116u8, 114, 101, 101, 32, 54, 55, 0,
        49, 48, 48, 54, 52, 52, 32, 104, 101, 108, 108, 111, 46, 116, 120, 116, 0,
        59, 24, 229, 18, 219, 167, 158, 76, 131, 0,
        221, 8, 174, 179, 127, 142, 114, 139, 141, 173, 52, 48, 48, 48, 48, 32, 115, 117, 98, 0,
        104, 255, 217, 241, 253, 68, 123, 131, 242, 105, 99, 203, 80, 21, 85, 50, 176, 1, 8, 241];
    let out = Tree::from_bytes(&bytes).unwrap();
    assert_eq!(out, Tree{
        obj_type: ObjType::Tree,
        nodes: vec![TreeNode{
            file_type: FileType::File,
            permission: FilePermission::UnExecutable,
            file_name: "hello.txt".to_string(),
            hash: Hash::from_string("3b18e512dba79e4c8300dd08aeb37f8e728b8dad").unwrap(),
        }, TreeNode{
            file_type: FileType::Directory,
            permission: FilePermission::Other,
            file_name: "sub".to_string(),
            hash: Hash::from_string("68ffd9f1fd447b83f26963cb50155532b00108f1").unwrap(),
        }],
        payload: bytes.to_vec(),
        hash: Some(Hash::from(&*bytes)),
    })
}

#[derive(PartialEq, Debug)]
enum FileType {
    Directory,
    File,
    SymbolicLink,
    Submodule
}

impl FileType {
    fn from_code_bytes(code: &Vec<u8>) -> Result<FileType, String> {
        if code.starts_with("40".as_bytes()) {
            return Ok(Directory);
        }
        if code.starts_with("100".as_bytes()) {
            return Ok(File);
        } else if code.starts_with("120".as_bytes()) {
            return Ok(SymbolicLink)
        } else if code.starts_with("160".as_bytes()) {
            return Ok(Submodule)
        }
        Err("invalid type".to_string())
    }

    fn to_code_string(&self) -> String {
        match self {
            Directory => "40".to_string(),
            File => "100".to_string(),
            SymbolicLink => "120".to_string(),
            Submodule => "160".to_string()
        }
    }
}

#[test]
fn test_filetype_from_code_bytes() {
    let tests = [
        ("40".as_bytes(), Ok(FileType::Directory)),
        ("100".as_bytes(), Ok(FileType::File)),
        ("130".as_bytes(), Err("invalid type".to_string())),
        ("4".as_bytes(), Err("invalid type".to_string())),
    ];
    for t in tests {
        let out = FileType::from_code_bytes(&t.0.to_vec());
        assert_eq!(out, t.1);
    }
}

#[test]
fn test_filetype_to_code_string() {
    let tests = [
        (FileType::Directory, "40".to_string()),
        (FileType::SymbolicLink, "120".to_string()),
    ];
    for t in tests {
        let out = t.0.to_code_string();
        assert_eq!(out, t.1);
    }
}

#[derive(PartialEq, Debug)]
enum FilePermission {
    Other,
    Executable,
    UnExecutable,
}

impl FilePermission {
    fn from_code_bytes(code: &Vec<u8>) -> Result<FilePermission, String> {

        if code.starts_with("755".as_bytes()) {
            return Ok(Executable);
        } else if code.starts_with("644".as_bytes()) {
            return Ok(UnExecutable);
        }else if code.starts_with("000".as_bytes()) {
            return Ok(FilePermission::Other);
        }
        Err("invalid type".to_string())
    }
    fn to_code_string(&self) -> String {
        match self {
            FilePermission::Other=> "000".to_string(),
            Executable => "755".to_string(),
            UnExecutable => "644".to_string(),
        }
    }
}

#[test]
fn test_permission_from_code_bytes() {
    let tests = [
        ("755".as_bytes(), Ok(FilePermission::Executable)),
        ("4".as_bytes(), Err("invalid type".to_string())),
    ];
    for t in tests {
        let out = FilePermission::from_code_bytes(&t.0.to_vec());
        assert_eq!(out, t.1);
    }
}

#[derive(PartialEq,Debug)]
pub struct TreeNode {
    file_type: FileType,
    permission: FilePermission,
    file_name: String,
    hash: Hash,
}

impl TreeNode {
    pub fn from_index_entry(entry: &IndexEntry) -> Option<Self> {
        let file_type = match entry.file_type() {
            0b1000=> FileType::File,
            0b1010=> FileType::SymbolicLink,
            0b1110=> FileType::Submodule,
            _ => return None,
        };

        let permission = match entry.permission() {
            0b111_101_101=> FilePermission::Executable,
            0b110_100_100=> FilePermission::UnExecutable,
            _ => return None,
        };

        let file_name = entry.file_name();
        let hash = entry.hash();

        Some(Self {
            file_type,
            permission,
            file_name,
            hash
        })
    }

    pub fn from_tree_node(hash: Hash, dir_name: String) -> Option<Self> {
        let file_type = FileType::Directory;
        let permission = FilePermission::Other;
        Some(Self {
            file_type,
            permission,
            file_name: dir_name,
            hash,
        })
    }

    fn parse(bytes: &Vec<u8>) -> Result<(Self, usize),String> {
        let file_type = FileType::from_code_bytes(bytes)?;
        let mut offset = file_type.to_code_string().len();
        let pos = &bytes[offset..];

        let permission = FilePermission::from_code_bytes(&pos.to_vec())?;
        offset += permission.to_code_string().len()+1;

        let pos = &bytes[offset..];
        let file_name = String::from_utf8(extract_until_null(pos.as_ref())).
            map_err(|e| e.to_string())?;
        offset += file_name.len()+1;
        let hash = Hash::from(&bytes[offset..offset+20]).
                ok_or("invalid hash value".to_string())?;

        offset += 20;

        Ok((TreeNode{
            file_type,
            permission,
            file_name,
            hash
        },offset))

    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut out = format!("{}{} {}\0",
                          self.file_type.to_code_string(),
                          self.permission.to_code_string(),
                          self.file_name).into_bytes();
        out.extend_from_slice(&self.hash.bytes());
        out
    }

}

#[test]
fn test_parse_tree_node() {
    let a = vec![49u8, 48, 48, 54, 52, 52, 32, 104, 97, 115, 32, 115, 112, 97, 99, 101, 46, 116,
        120, 116, 0, 6, 74, 146, 215, 131, 249, 152, 81, 209, 81, 123, 81, 186, 11, 42, 237, 74,
        29, 49, 40, 243, 128]; // has unnecessary tail bytes

    let (tree, len) = TreeNode::parse(&a).unwrap();
    assert_eq!(len, 41);
    assert_eq!(tree, TreeNode{
        file_type: FileType::File,
        permission: FilePermission::UnExecutable,
        file_name: "has space.txt".to_string(),
        hash: Hash::from_string("064a92d783f99851d1517b51ba0b2aed4a1d3128").unwrap()
    });
}

#[test]
fn test_create_bytes_tree_node() {
    let source = TreeNode {
        file_type: FileType::File,
        permission: FilePermission::UnExecutable,
        file_name: "has space.txt".to_string(),
        hash: Hash::from_string("064a92d783f99851d1517b51ba0b2aed4a1d3128").unwrap()
    };

    let refer = vec![49u8, 48, 48, 54, 52, 52, 32, 104, 97, 115, 32, 115, 112, 97, 99, 101, 46, 116,
        120, 116, 0, 6, 74, 146, 215, 131, 249, 152, 81, 209, 81, 123, 81, 186, 11, 42, 237, 74,
        29, 49, 40];
    match source.create_bytes() {
        Some(s) => assert_eq!(s, refer),
        None => assert!(false)
    }
}

#[derive(PartialEq,Debug,Default)]
pub struct Commit {
    obj_type: ObjType,
    tree: Hash,
    parents: Vec<Hash>,
    author: CommitUser, //originalのコミッター（amendではデフォルトでは変更されない）
    committer: CommitUser, //コミッター（amendで変更される）
    commit_message: String,
}

impl Commit {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if !bytes.starts_with("commit".as_bytes()){
            return None;
        }
        let offset = "commit ".len();
        let len_str = String::from_utf8(extract_until_null(&bytes[offset..])).ok()?;
        let offset_header = offset + len_str.len() + 1;
        let body = String::from_utf8(bytes[offset_header..].to_vec()).ok()?;
        let lines: Vec<&str> = body.split("\n").collect();
        let mut commit = Commit {
            obj_type: ObjType::Commit,
            tree: Default::default(),
            parents: vec![],
            author: Default::default(),
            committer: Default::default(),
            commit_message: "".to_string(),
        };
        let mut idx: usize = 0;
        for line in &lines {
            if line.starts_with("tree") {
                commit.tree = Hash::from_string(&line["tree ".len()..])?;
            } else if line.starts_with("parent") {
                commit.parents.push(Hash::from_string(&line["parent ".len()..])?);
            } else if line.starts_with("author") {
                commit.author = CommitUser::from_bytes(line.as_bytes())?;
            } else if line.starts_with("committer") {
                commit.committer = CommitUser::from_bytes(line.as_bytes())?;
            }
            idx+=1;
            if line.len() == 0 {
                break;
            }
        }
        commit.commit_message = lines[idx..].to_vec().join("\n");


        Some(commit)
    }

    pub fn from_depressed_bytes(bytes: &[u8]) -> Option<Self> {
        Self::from_bytes(&extract_zlib(bytes).ok()?)
    }

    pub fn from(tree_root: Hash, parents: Vec<Hash>, author: CommitUser, committer: CommitUser,
                message: String) -> Self {
        Self {
            obj_type: ObjType::Commit,
            tree: tree_root,
            parents,
            author,
            committer,
            commit_message: message
        }
    }


    pub fn to_bytes(&self) -> Vec<u8>{
        let body = if self.parents.len() == 0 {
            format!("tree {}\n{}\n{}\n\n{}", self.tree.string(),
                           self.author.to_string(), self.committer.to_string(), self.commit_message)
        } else {
            let parents_str: Vec<String> =self.parents.clone().into_iter().
                map(|x| format!("parent {}", x.string())).collect();
            let parents_concat: String = parents_str.join("\n");
            format!("tree {}\n{}\n{}\n{}\n\n{}\n", self.tree.string(), parents_concat,
                           self.author.to_string(), self.committer.to_string(), self.commit_message)
        };

        format!("commit {}\0{}", body.len(), body).into_bytes()
    }

    pub fn generate_hash_and_depress(&self) -> Result<(Hash, Vec<u8>),String> {
        let bytes = self.to_bytes();
        let hash = calc_sha1_bytes(&bytes);
        let body = depress_zlib(&bytes)?;
        Ok((hash, body))
    }

    pub fn parents(&self) -> &Vec<Hash> {
        &self.parents
    }

    pub fn log_entry(&self,hash: Hash, refs: &Vec<String>) -> String {
        let refs_string = if refs.len() == 0 {
            String::new()
        } else {
            format!("({})", refs.join(" "))
        };
        let message =
            format!("    {}", self.commit_message.replace("\n", "\n    "));

        format!("commit {} {}\nAuthor: {} <{}>\nDate:   {}\n\n{}\n",
                    hash.string(), refs_string, self.author.name, self.author.address,
                    self.author.time_stamp.format("%c %z").to_string(), self.commit_message)
    }

    pub fn timestamp(&self) -> DateTime<FixedOffset> {
        self.author.time_stamp
    }
}

#[test]
fn test_commit_from() {
    let input = vec!(99, 111, 109, 109, 105, 116, 32, 50, 50, 56, 0, 116, 114, 101, 101,
                     32, 52, 49, 49, 98, 48, 55, 52, 99, 57, 48, 101, 54, 49, 49, 101, 49, 50, 98,
                     57, 97, 102, 101, 101, 49, 57, 49, 49, 50, 52, 100, 98, 101, 52, 99, 55, 53,
                     53, 51, 55, 48, 10, 112, 97, 114, 101, 110, 116, 32, 48, 98, 51, 50, 54, 51,
                     52, 48, 100, 99, 101, 100, 98, 55, 97, 50, 55, 56, 50, 98, 101, 98, 56, 98,
                     101, 100, 52, 100, 49, 98, 53, 56, 49, 50, 97, 100, 52, 50, 52, 51, 10, 97,
                     117, 116, 104, 111, 114, 32, 115, 97, 105, 116, 111, 110, 48, 51, 32, 60, 115,
                     97, 105, 116, 111, 110, 49, 53, 54, 48, 51, 64, 103, 109, 97, 105, 108, 46, 99,
                     111, 109, 62, 32, 49, 54, 51, 51, 51, 50, 53, 56, 49, 51, 32, 43, 48, 57, 48,
                     48, 10, 99, 111, 109, 109, 105, 116, 116, 101, 114, 32, 115, 97, 105, 116, 111,
                     110, 32, 48, 51, 32, 60, 115, 97, 105, 116, 111, 110, 49, 53, 54, 48, 51, 64,
                     103, 109, 97, 105, 108, 46, 99, 111, 109, 62, 32, 49, 54, 51, 51, 51, 51, 50,
                     57, 54, 55, 32, 43, 48, 57, 48, 48, 10, 10, 109, 117, 108, 116, 105, 112, 108,
                     101, 10, 108, 105, 110, 101, 115, 10);
    let out = Commit::from_bytes(&input).expect("error");
    assert_eq!(out, Commit{
        obj_type: ObjType::Commit,
        tree: Hash::from_string("411b074c90e611e12b9afee191124dbe4c755370").unwrap(),
        parents: vec![Hash::from_string("0b326340dcedb7a2782beb8bed4d1b5812ad4243").unwrap()],
        author: CommitUser{
            committer_type: CommitterType::Author,
            name: "saiton03".to_string(),
            address: "saiton15603@gmail.com".to_string(),
            time_stamp: FixedOffset::east(9*3600).timestamp(1633325813, 0)
        },
        committer: CommitUser{
            committer_type: CommitterType::Committer,
            name: "saiton 03".to_string(),
            address: "saiton15603@gmail.com".to_string(),
            time_stamp: FixedOffset::east(9*3600).timestamp(1633332967, 0)
        },
        commit_message: "multiple\nlines\n".to_string(),
    });
}

#[test]
fn test_commit_to_bytes() {
    let input = vec!(99, 111, 109, 109, 105, 116, 32, 50, 50, 56, 0, 116, 114, 101, 101,
                     32, 52, 49, 49, 98, 48, 55, 52, 99, 57, 48, 101, 54, 49, 49, 101, 49, 50, 98,
                     57, 97, 102, 101, 101, 49, 57, 49, 49, 50, 52, 100, 98, 101, 52, 99, 55, 53,
                     53, 51, 55, 48, 10, 112, 97, 114, 101, 110, 116, 32, 48, 98, 51, 50, 54, 51,
                     52, 48, 100, 99, 101, 100, 98, 55, 97, 50, 55, 56, 50, 98, 101, 98, 56, 98,
                     101, 100, 52, 100, 49, 98, 53, 56, 49, 50, 97, 100, 52, 50, 52, 51, 10, 97,
                     117, 116, 104, 111, 114, 32, 115, 97, 105, 116, 111, 110, 48, 51, 32, 60, 115,
                     97, 105, 116, 111, 110, 49, 53, 54, 48, 51, 64, 103, 109, 97, 105, 108, 46, 99,
                     111, 109, 62, 32, 49, 54, 51, 51, 51, 50, 53, 56, 49, 51, 32, 43, 48, 57, 48,
                     48, 10, 99, 111, 109, 109, 105, 116, 116, 101, 114, 32, 115, 97, 105, 116, 111,
                     110, 32, 48, 51, 32, 60, 115, 97, 105, 116, 111, 110, 49, 53, 54, 48, 51, 64,
                     103, 109, 97, 105, 108, 46, 99, 111, 109, 62, 32, 49, 54, 51, 51, 51, 51, 50,
                     57, 54, 55, 32, 43, 48, 57, 48, 48, 10, 10, 109, 117, 108, 116, 105, 112, 108,
                     101, 10, 108, 105, 110, 101, 115, 10);
    let mut out = Commit::from_bytes(&input).expect("error");
    let back = out.to_bytes();
    assert_eq!(input, back);
}

#[derive(PartialEq,Debug, Clone)]
pub struct CommitUser{
    committer_type: CommitterType,
    name: String,
    address: String,
    time_stamp: DateTime<FixedOffset>,
}

impl CommitUser {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let string = String::from_utf8(bytes.to_vec()).ok()?;
        let re = Regex::new(
            r"^(\w*) (.*) <([a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9-]+(?:\.[a-zA-Z0-9-]+)*)> (\d+) ([+-]?\d{4})"
        ).ok()?;

        let result = re.captures(&string)?;
        if result.len() != 6 {
            return None;
        }

        let committer_type = CommitterType::from_code_bytes(result[1].as_bytes()).ok()?;
        let name = result[2].to_string();
        let address = result[3].to_string();
        let u_time_without_timezone: i64 = result[4].parse().ok()?;
        let time_offset = calc_time_offset(&result[5])?;
        let time_stamp = FixedOffset::east(time_offset).timestamp(u_time_without_timezone, 0);

        Some(CommitUser{
            committer_type,
            name,
            address,
            time_stamp,
        })
    }

    pub fn from(user_name: String, user_email: String, committer_type: CommitterType) -> Self {
        let now_local = Local::now();
        let time_stamp = now_local.with_timezone(now_local.offset());
        Self {
            committer_type,
            name: user_name,
            address: user_email,
            time_stamp,
        }
    }

    pub fn change_committer_type_as(& self, committer_type: CommitterType) -> Self {
        let mut ret = self.clone();
        ret.committer_type = committer_type;
        ret
    }

    pub fn to_string(&self) -> String {
        let timestamp = self.time_stamp.timestamp();
        let timezone = self.time_stamp.timezone().local_minus_utc();
        let timezone_hour = timezone/3600;
        let timezone_min = (timezone%3600)/60;
        let timezone_str = if timezone > 0 {
            format!("+{:<02}{:<02}", timezone_hour, timezone_min)
        } else if timezone < 0 {
            format!("-{:<02}{:<02}", timezone_hour, timezone_min)
        } else {
            "0000".to_string()
        };
        format!("{} {} <{}> {} {}", self.committer_type.to_code_string(),
                self.name, self.address, timestamp, timezone_str)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.to_string().into_bytes()
    }
}

impl Default for CommitUser{
    fn default() -> Self {
        Self{
            committer_type: CommitterType::Author,
            name: "".to_string(),
            address: "".to_string(),
            time_stamp: FixedOffset::east(0).timestamp(0, 0)
        }
    }
}

#[test]
fn test_commit_user_from() {
    let input = "committer hogeo hoge <hoge@example.com> 1633332967 +0900".as_bytes();
    let out = CommitUser::from_bytes(input).expect("parse failed");
    assert_eq!(out, CommitUser{
        committer_type: CommitterType::Committer,
        name: "hogeo hoge".to_string(),
        address: "hoge@example.com".to_string(),
        time_stamp: FixedOffset::east(9*3600).timestamp(1633332967, 0),
    })
}

#[test]
fn test_commit_user_to_bytes() {
    let input = "committer hogeo hoge <hoge@example.com> 1633332967 +0900".as_bytes();
    let out = CommitUser::from_bytes(input).expect("parse failed");
    let result = out.to_bytes();
    assert_eq!(&result, input)

}

fn calc_time_offset(time: &str)-> Option<i32> {
    if time.len()<4 || time.len()>5 {
        return None;
    }
    if !time.starts_with("+") && !time.starts_with("-") {
        return Some(0);
    }
    let hour: i32= time[1..3].parse().ok()?;
    let min: i32= time[3..5].parse().ok()?;
    let value = hour*3600+min*60;
    match time.chars().nth(0)? {
        '+' => Some(value),
        '-' => Some(-value),
        _ => None,
    }
}


#[derive(PartialEq, Debug, Clone)]
pub enum CommitterType {
    Author,
    Committer,
}

impl CommitterType {
    fn from_code_bytes(code: &[u8]) -> Result<CommitterType, String> {
        if code.starts_with("author".as_bytes()) {
            return Ok(Author);
        } else if code.starts_with("committer".as_bytes()) {
            return Ok(Committer);
        }
        Err("invalid type".to_string())
    }
    fn to_code_string(&self) -> String {
        match self {
            Author => "author".to_string(),
            Committer => "committer".to_string(),
        }
    }
}
