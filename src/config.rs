use std::env;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::io::Read;
use serde_derive::Deserialize;

//confはglobalのみ

#[derive(Default, Deserialize)]
pub struct Config {
    pub user: User,
}

#[derive(Default, Deserialize)]
pub struct User {
    pub name: String,
    pub email: String,
}

pub fn parse_config() -> Result<Config, String>{
    let ret: Config = Default::default();

    //global values
    let global_path = get_global_config_path()?;
    let ret =parse_from_file(global_path, ret)?;

    Ok(ret)
}

fn get_global_config_path() -> Result<PathBuf, String> {
    let home_dir = env::var("HOME").map_err(|e| e.to_string())?;
    Ok(Path::new(&home_dir).join(".gitconfig"))
}

fn parse_from_file(path: PathBuf, _conf: Config) -> Result<Config, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    let _ = file.read_to_end(&mut buf);
    let conf_after: Config = toml::from_slice(&buf[..]).map_err(|e| e.to_string())?;
    Ok(conf_after)
}