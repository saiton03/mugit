use std::path::PathBuf;
use std::fs;
use std::io::Write;
use clap::ArgMatches;
use crate::common::get_project_root;
use crate::head::Head;
use crate::index::{Index, IndexEntry};
use crate::object::{Tree, TreeNode, Commit, CommitUser, CommitterType};
use super::config as config;
use super::hash::Hash;

pub fn run(matches: &ArgMatches) -> Result<(), String>{
    let conf: config::Config = config::parse_config()?;
    let message = matches.value_of("message").map(|m| m.to_string());

    let proj_root = get_project_root()?;
    let head = Head::new()?;
    if head.is_dangling() {
        return Err("header is detached, please create branch".to_string());
    }

    let user_name = conf.user.name;
    let user_email = conf.user.email;
    let config= CommitConf{
        user_name,
        user_email,
        is_amend: false
    };

    let index = Index::from_file(&proj_root).ok_or("no index found".to_string())?;

    let mut generator = CommitGenerator::new(index,proj_root.clone(), config, message, head.clone())?;
    let commit_obj =  generator.exec()?;
    let (hash, body) = commit_obj.generate_hash_and_depress()?;
    let obj_path = proj_root.join(".git/objects").join(hash.generate_path());

    let path_parent = obj_path.parent().unwrap();
    fs::create_dir_all(path_parent).map_err(|e| e.to_string())?;
    if !obj_path.exists() {
        fs::write(obj_path, &body).map_err(|e| e.to_string())?;
    }

    let branch_path = proj_root.join(".git/refs/heads").
                                join(&head.branch().unwrap());
    fs::write(branch_path,hash.string()).map_err(|e| e.to_string())?;


    Ok(())
}

struct CommitGenerator {
    commit_tree: CommitTree,
    obj_root: PathBuf,
    config: CommitConf,
    message: Option<String>,
    head: Head,
}

impl CommitGenerator {
    pub fn new(index: Index, proj_root: PathBuf, config: CommitConf,
               message: Option<String>, head: Head) -> Result<Self, String> {
        let obj_root = proj_root.join(".git/objects");
        let commit_tree = CommitTree::from_index(index)?;
        Ok(CommitGenerator {
            commit_tree,
            obj_root,
            config,
            message,
            head
        })
    }

    pub fn exec(&mut self) -> Result<Commit, String>{
        let mut tree_list: Vec<(Hash, Tree)> = Vec::new();
        let root_hash = self.commit_tree.generate_tree_obj(&mut tree_list)?;

        self.generate_tree_file(&mut tree_list)?;

        if self.config.is_amend {
            todo!()
        } else {
            self.generate_new_commit(root_hash)
        }
    }

    fn generate_new_commit(&mut self, root_hash: Hash) -> Result<Commit, String> {
        let parents =  match self.head.hash() {
            None => { Vec::new() }
            Some(h) => { vec![h] }
        };

        let author = CommitUser::from(self.config.user_name.clone(),
                                      self.config.user_email.clone(),
                                      CommitterType::Author);
        let committer = author.change_committer_type_as(CommitterType::Committer);
        let message = self.message.clone().ok_or("no commit message")?;
        Ok(Commit::from(root_hash,parents,author, committer,message))
    }

    fn generate_tree_file(&self, tree_list: &Vec<(Hash, Tree)>) -> Result<(), String> {
        for (hash, tree) in  tree_list {
            let obj_path = self.obj_root.join(hash.generate_path());
            if obj_path.exists() {
                continue;
            }
            let parent = obj_path.parent().unwrap();
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;

            let data = tree.generate_depress()?;
            let mut file = fs::File::create(obj_path).map_err(|e| e.to_string())?;
            file.write_all(&data).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

}

struct CommitConf {
    user_name: String,
    user_email: String,
    is_amend: bool,
}

enum CommitTree {
    Leaf(IndexEntry),
    Node(Vec<(String, Box<CommitTree>)>),
}

impl CommitTree {
    pub fn new() -> Self {
        Self::Node(Vec::new())
    }

    pub fn from_index(index: Index) -> Result<Self, String> {
        let mut tree = Self::new();

        let entries = index.entries();
        for entry in entries {
            let node_array = entry.0.to_str().
                ok_or("cannot parse pathbuf".to_string())?.split("/").collect::<Vec<_>>();
            tree.update_node(&node_array, entry.1)?;
        }

        Ok(tree)
    }

    pub fn update_node(&mut self, node_array: &[&str], entry: IndexEntry) -> Result<(),String> {
        match self {
            Self::Leaf(_) => Err("ref leaf".to_string()),
            Self::Node(ref mut node) => {
                if node_array.len() == 1 {
                    node.push((node_array[0].to_string(), Box::new(Self::Leaf(entry))));
                    return Ok(());
                }
                let node_name = node_array[0].to_string();
                let node_array = &node_array[1..];
                for i in 0..node.len() {
                    if node_name == node[i].0 {
                        return node[i].1.update_node(node_array, entry);
                    }
                }
                let mut new_node = Box::new(CommitTree::new());
                new_node.update_node(node_array, entry)?;
                node.push((node_name, new_node));
                Ok(())
            },
        }
    }

    pub fn generate_tree_obj(&self, tree_list: &mut Vec<(Hash, Tree)>) -> Result<Hash, String> {
        match self {
            CommitTree::Leaf(_) => unreachable!(),
            CommitTree::Node(ref node) => {
                let mut tree = Tree::new();
                for i in 0..node.len() {
                    match &*node[i].1 {
                        CommitTree::Leaf(ie) => {
                           tree.add_tree_node(TreeNode::from_index_entry(ie)
                               .ok_or("create node failed".to_string())?)
                        },
                        CommitTree::Node(_) => {
                            let hash = node[i].1.generate_tree_obj(tree_list)?;
                            let node_name = PathBuf::from(&node[i].0).file_name().
                                ok_or("get node_name failed")?.
                                to_str().ok_or("convert file_name to string failed")?.to_string();
                            tree.add_tree_node(
                                TreeNode::from_tree_node(hash, node_name).
                                    ok_or("create node failed".to_string())?
                            );

                        }
                    }
                }

                let hash = tree.calc_hash();

                tree_list.push((hash, tree));

                Ok(hash)
            },
        }
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool{
        match self {
            CommitTree::Leaf(_) => false,
            CommitTree::Node(v) => v.is_empty(),
        }
    }

    #[allow(dead_code)]
    pub fn show_tree(&self, depth: u32, name: String) -> String {
        let mut ret = format!("{}{}\n","  ".repeat(depth as usize),name);
        match self {
            CommitTree::Leaf(_) => {}
            CommitTree::Node(nodes) => {
                for n in nodes {
                    ret =  format!("{}{}",ret,n.1.show_tree(depth+1, n.0.clone()));
                }
            }
        }

        ret
    }
}

