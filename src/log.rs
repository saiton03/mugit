use std::collections::{BTreeSet};
use std::path::PathBuf;
use std::fs;
use std::io;
use std::io::Write;
use clap::ArgMatches;
use crate::common::get_project_root;
use crate::head::Head;
use crate::hash::Hash;
use crate::object::Commit;


pub fn run(_matches: &ArgMatches) -> Result<(), String>{
    let object_root = get_project_root()?.join(".git/objects/");

    // HEAD only
    let head = Head::new()?;

    let head_hash = head.hash().ok_or(" HEAD does not any commits yes".to_string())?;

    let mut parser = LogParser::from(object_root, head_hash);
    let result = parser.parse()?;

    io::stdout().write_all(result.as_bytes()).map_err(|e| e.to_string())?;

    Ok(())
}

struct LogParser {
    object_root: PathBuf,
    head_hash: Hash,
    commits: Vec<(Hash, Commit)>,
}

impl LogParser {
    pub fn from(object_root: PathBuf, head_hash: Hash) -> Self {
        Self{
            object_root,
            head_hash,
            commits: Vec::new(),
        }
    }

    pub fn parse(&mut self) -> Result<String,String> {
        let mut visit : BTreeSet<Hash> = BTreeSet::new();
        self.dfs(self.head_hash, &mut visit)?;

        self.commits.sort_by_key(|c| c.1.timestamp());
        self.commits.reverse();
        let ret = self.commits.iter().
            map(|c| c.1.log_entry(c.0, &Vec::new())).
            collect::<Vec<_>>().join("\n");
        Ok(ret)
    }

    fn dfs(&mut self, node: Hash, visit: &mut BTreeSet<Hash>) -> Result<(),String>{
        if visit.get(&node) != None {
            return Ok(());
        }
        visit.insert(node.clone());

        let path = self.get_object_path(node);
        let bytes = fs::read(path).map_err(|e| e.to_string())?;
        let commit = Commit::from_depressed_bytes(&bytes).
                ok_or("parse commit error".to_string())?;
        let parents = commit.parents();
        for parent in parents {
            self.dfs(*parent, visit)?;
        }

        self.commits.push((node, commit));

        Ok(())
    }

    fn get_object_path(&self, hash: Hash) -> PathBuf {
        self.object_root.join(hash.generate_path())
    }
}

/*
struct Refs(BTreeMap<Hash, Vec<String>>);

impl Refs {
    fn new() -> Result<Self,String> {
        let refs = get_project_root()?;

    }
}
 */
