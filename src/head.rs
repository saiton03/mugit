use std::fs;
use clap::ArgMatches;
use crate::hash::Hash;
use crate::common::{get_project_root};

pub fn run(_matches: &ArgMatches) -> Result<(), String>{
    let head = Head::new()?;
    println!("{:?}", head);
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Head {
    branch: Option<String>,
    hash: Option<Hash>,
    is_dangling: bool,
}

impl Head {
    pub fn new() -> Result<Self, String> {
        let proj_root = get_project_root()?;
        let head_file = proj_root.join(".git/HEAD");
        if !head_file.exists() {
            return Ok(Self {
                branch: None,
                hash: None,
                is_dangling: false,
            });
        }

        let ref_str = fs::read(head_file).map_err(|e| e.to_string())?;
        if is_head_dangling(&ref_str) {
            return Ok(Self {
                branch: None,
                hash: Hash::from(&ref_str),
                is_dangling: true,
            });
        }
        let branch_path = String::from_utf8(ref_str.
            strip_prefix("ref: ".as_bytes()).ok_or("parse failed".to_string())?.
            strip_suffix("\n".as_bytes()).ok_or("trim suffix failed".to_string())?.
            to_vec()).
            map_err(|e| e.to_string())?;

        let ref_path = proj_root.join(".git").join(&branch_path);
        let branch_name = branch_path.strip_prefix("refs/heads/").
                ok_or("parse failed".to_string())?;

        let hash = fs::read(ref_path).ok();
        let hash =  match hash {
            Some(h) => Hash::from_string(&String::from_utf8(h).unwrap()),
            None=> None,
        };

        Ok(Self {
            branch: Some(branch_name.to_string()),
            hash,
            is_dangling: false,
        })
    }

    pub fn branch(&self) -> Option<String> {
        self.branch.clone()
    }

    pub fn hash(&self) -> Option<Hash> {
        self.hash
    }

    pub fn is_dangling(&self) -> bool {
        self.is_dangling
    }

}

pub fn is_head_dangling(ref_str: &[u8]) -> bool {

    !ref_str.starts_with("ref: ".as_bytes())
}
