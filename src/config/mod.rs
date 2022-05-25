use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::Result;
#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub host: String,
    pub port: String,
    dirs_to_watch: Vec<String>,
    pub local_folder: String,
    pub user: String,
    pub pass: String,
    pub connections: usize,
    conn_healthcheck_rate_sec: u16,
    pub ready_flag_file_ext: String
}
impl Config {
    //read config file in root dir
    pub async fn new(mut dir: PathBuf, filename: &'static str) -> Result<Self> {
        dir.push(filename);
        let file = std::fs::File::open(dir)?;
        let rdr = std::io::BufReader::new(file);
        let config = serde_json::from_reader(rdr)?;
        Ok(config)
    }
    pub fn get_hostname(&self) -> String {
        let mut host = self.host.to_owned();
        host.push(':');
        host.push_str(&self.port.to_owned());
        host
    }
    pub fn get_download_target_path(&self, root: &str, ftp_path: &str) -> String {
        let path = ftp_path.replace(root, &self.local_folder);
        println!("{}", path);
        path
    }
    pub fn get_dirs_to_watch(&self) -> Vec<String> {
        self.dirs_to_watch.to_owned()
    }
    pub fn get_healthcheck_interval(&self) -> u16 {
        self.conn_healthcheck_rate_sec as u16
    }
}
