use std::fs;
use std::path::{Path};
use std::io::{Write};

use clap::{ArgMatches};

use super::common as common;

/*
    git init
    .git -- HEAD
         |- objects/
         |    |- info/
         |    |- pack/
         |- refs/
              |- heads/
              |- tags/
 */
pub fn run(matches: &ArgMatches) -> Result<(), String> {
    let path = matches.value_of("path").unwrap_or(".");
    let path_base = Path::new(path).join(".git");

    if !path_base.exists() {
        println!("initialize git to {}", path_base.display());
        fs::create_dir_all(&path_base).map_err(|e| e.to_string())?;
    } else {
        println!("reinitialize git to {}", path_base.display());
    }

    let path_head = path_base.join("HEAD");
    if !path_head.exists() {
        let initial_head= format!("ref: refs/heads/{}\n", common::DEFAULT_BRANCH_NAME);

        let mut file = fs::File::create(&path_head).map_err(|e| e.to_string())?;
        file.write_all(initial_head.as_bytes()).map_err(|e| e.to_string())?;
    }

    let path_objects = path_base.join("objects");
    if !path_objects.exists() {
        fs::create_dir(&path_objects).map_err(|e| e.to_string())?;
    }

    let path_objects_info = path_objects.join("info");
    if !path_objects_info.exists() {
        fs::create_dir(&path_objects_info).map_err(|e| e.to_string())?;
    }
    let path_objects_pack = path_objects.join("pack");
    if !path_objects_pack.exists() {
        fs::create_dir(&path_objects_pack).map_err(|e| e.to_string())?;
    }

    let path_refs = path_base.join("refs");
    if !path_refs.exists() {
        fs::create_dir(&path_refs).map_err(|e| e.to_string())?;
    }
    let path_refs_heads = path_refs.join("heads");
    if !path_refs_heads.exists() {
        fs::create_dir(&path_refs_heads).map_err(|e| e.to_string())?;
    }
    let path_refs_tags = path_refs.join("tags");
    if !path_refs_tags.exists() {
        fs::create_dir(&path_refs_tags).map_err(|e| e.to_string())?;
    }


    Ok(())
}


