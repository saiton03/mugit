use std::collections::{BTreeSet};
use std::env::current_dir;
use std::fs;
use std::io::Write;
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use clap::{ArgMatches};
use crate::common::{get_path_from_project_root, get_project_root};
use crate::index::{Index, IndexEntry};
use crate::object::{Blob};

pub fn run(matches: &ArgMatches) -> Result<(), String>{
    let path = matches.value_of("path").ok_or("no path specified")?;

    let proj_root = get_project_root()?;
    let object_path = proj_root.join(".git/objects");
    let mut blob_list: Vec<Box<Blob>> = Vec::new();
    let index = Index::from_file(&proj_root);
    let index_box = match index {
        None => None,
        Some(s) => Some(Box::new(s)),
    };

    let search_root = get_path_from_project_root(&PathBuf::from(path))?;
    let mut parser = DiffParser::from(index_box.clone(), search_root)?;
    let results = parser.parse()?;

    let new_index = match &index_box {
        None => {
            let index= create_index(&proj_root, results.0, &mut blob_list)?;
            index
        },
        Some(_) => {
            let mut new_index = index_box.unwrap();
            update_index(&proj_root, &mut new_index,
                         results.0, results.1, results.2, &mut blob_list)?;
            new_index
        },
    };

    let index_bytes = new_index.to_bytes();
    let mut index_file = fs::File::create(proj_root.join(".git/index")).
                        map_err(|e| e.to_string())?;
    index_file.write_all(&index_bytes).map_err(|e| e.to_string())?;

    for blob in blob_list {
        let blob_path = object_path.join(blob.hash.generate_path());
        let parent = blob_path.parent().unwrap();
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;

        let data = blob.generate_depress()?;
        let mut file = fs::File::create(blob_path).map_err(|e| e.to_string())?;
        file.write_all(&data).map_err(|e| e.to_string())?;
    }


    Ok(())
}

fn update_index(proj_root: &PathBuf, index: &mut Box<Index>, new: &BTreeSet<PathBuf>,
                modify: &BTreeSet<PathBuf>, delete: &BTreeSet<PathBuf>,
                blob_list:&mut Vec<Box<Blob>>) -> Result<(),String> {
    for node in delete {
        index.delete_entry(node)?;
    }
    add_entries(proj_root, new, index, blob_list)?;
    add_entries(proj_root, modify, index, blob_list)?;

    Ok(())
}

fn create_index(proj_root: &PathBuf, new: &BTreeSet<PathBuf>, blob_list:&mut Vec<Box<Blob>>) -> Result<Box<Index>, String> {
    let mut index =  Box::new(Index::new());
    add_entries(proj_root, new, &mut index, blob_list)?;

    Ok(index)
}

fn add_entries(proj_root: &PathBuf, nodes: &BTreeSet<PathBuf>,
               index: &mut Box<Index>, blob_list:&mut Vec<Box<Blob>>) -> Result<(),String> {
    for node in nodes {
        let abs_path = proj_root.join(&node);
        let blob = Box::new(Blob::from_file(&abs_path).
            ok_or(format!("could not fetch file: {}", abs_path.to_str().unwrap()))?);
        let hash = blob.generate_digest_bytes();

        index.add_entry(node, hash)?;
        blob_list.push(blob);
    }
    Ok(())
}


#[derive(Debug, PartialEq)]
struct DiffParser {
    index: Option<Box<Index>>,
    new_nodes: BTreeSet<PathBuf>,
    mod_nodes: BTreeSet<PathBuf>,
    delete_nodes: BTreeSet<PathBuf>,
    search_root: PathBuf,
}

impl DiffParser {
    pub fn from(index: Option<Box<Index>>, search_root: PathBuf) -> Result<Self, String>{
        let index = index;
        let new_nodes = BTreeSet::new();
        let mod_nodes = BTreeSet::new();
        let delete_nodes =  match &index {
            Some(ie) => get_all_sub_nodes(&search_root,
                              ie.entries().keys().cloned().collect()),
            None => BTreeSet::new()
        };
        let search_root = if search_root.to_str().unwrap() == "" {
            current_dir().map_err(|e| e.to_string())?
        } else {
            search_root
        };
        Ok(DiffParser {
            index,
            new_nodes,
            mod_nodes,
            delete_nodes,
            search_root
        })
    }

    pub fn parse(&mut self) -> Result<(&BTreeSet<PathBuf>, &BTreeSet<PathBuf>, &BTreeSet<PathBuf>), String> {
        self.search_partial(&self.search_root.clone())?;
        Ok((
            &self.new_nodes,
            &self.mod_nodes,
            &self.delete_nodes,
        ))
    }

    fn search_partial(&mut self, path: &PathBuf) -> Result<(),String> {
        if path.is_dir() {
            for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                if &entry.file_name() == ".git"{
                    continue;
                }
                self.search_partial(&entry.path())?;
            }
        } else if path.is_file() {
            match &self.index {
                Some(_) => {
                    self.update_node(path)?;
                },
                None => {
                    let trimmed_path = get_path_from_project_root(path)?;
                    self.new_nodes.insert(trimmed_path);
                },
            }
        }
        Ok(())
    }

    fn update_node(&mut self, path: &PathBuf) -> Result<(), String>{
        let trimmed_path = get_path_from_project_root(path)?;
        self.delete_nodes.remove(&trimmed_path);
        match self.index.as_ref().unwrap().get_entry(path.clone()) {
            Some(ie) => if self.is_modified(path, ie)? {
                    self.mod_nodes.insert(trimmed_path);
                },
            None => {
                self.new_nodes.insert(trimmed_path);
            },
        }
        Ok(())
    }

    fn is_modified(&self, path: &PathBuf, index_entry: IndexEntry) -> Result<bool, String> {
        let ref_time = index_entry.mod_time();
        let meta_data = path.metadata().map_err(|e| e.to_string())?;
        let mod_time = ((meta_data.mtime() as u64) << 32) + (meta_data.mtime_nsec() as u64);
        Ok(mod_time>ref_time)
    }

}

#[test]
fn test_diff_parser_search() {
    let cur_dir = PathBuf::from("testspace/");
    //let index = Index::from_file(&get_project_root_from(&cur_dir).expect("get root error"));
    let mut parser = DiffParser::from(None, cur_dir).expect("create parser error");
    let result = parser.parse().expect("parse failed");
    println!("{:?}\n{:?}\n{:?}", result.0, result.1, result.2);
}


fn get_all_sub_nodes(root: &PathBuf, all_nodes: Vec<PathBuf>) -> BTreeSet<PathBuf>{
    let start = match all_nodes.binary_search(root){
        Ok(n) => n,
        Err(n) => n,
    };

    let mut sub_node : BTreeSet<PathBuf> = BTreeSet::new();
    for node in &all_nodes[start..] {
        if !node.starts_with(root) {
            break;
        }
        sub_node.insert(node.clone());
    }
    sub_node
}
