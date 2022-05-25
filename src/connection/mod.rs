use crate::{config::Config, utils, CustomError, FtpDirEntry};
use async_ftp::types::FileType;
use async_ftp::FtpStream;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

pub type Result<T> = std::result::Result<T, CustomError>;

pub struct Connection {
    stream: async_ftp::FtpStream,
    config: Config,
}
impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let stream = self.get_ref();
        let default = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
        f.debug_struct("Connection")
            .field(
                "stream local address",
                &stream.local_addr().unwrap_or_else(|_| default.to_owned()),
            )
            .field(
                "stream peer address",
                &stream.peer_addr().unwrap_or(default),
            )
            .finish()
    }
}
impl Connection {
    pub fn get_ready_flag(&self) -> &str {
        &self.config.ready_flag_file_ext
    }
    pub async fn with_config(conf: &Config) -> Result<Self> {
        //connect
        //loop trying to connec
        let host = conf.get_hostname();
        println!("connecting to {:?}", &host);
        let mut stream = FtpStream::connect(host)
            .await
            .map_err(|e| tokio::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        stream
            .login(&conf.user, &conf.pass)
            .await
            .map_err(|e| tokio::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        stream.transfer_type(FileType::Binary).await?;
        Ok(Self {
            config: conf.to_owned(),
            stream,
        })
    }
    pub async fn restore(&mut self) -> Result<()> {
        let host = self.config.get_hostname();
        println!("connecting to {:?}", &host);
        self.stream = FtpStream::connect(host).await?;
        self.stream
            .login(&self.config.user, &self.config.pass)
            .await?;
        self.stream.transfer_type(FileType::Binary).await?;
        Ok(())
    }
    pub async fn batch_delete_remote(&mut self, entries: &[FtpDirEntry]) -> Result<()> {
        for entry in entries.iter().rev() {
            match entry {
                FtpDirEntry::File(p, ..) => self.rm(p).await?,
                FtpDirEntry::Folder(p) => self.rmdir(p).await?,
            };
        }
        Ok(())
    }

    pub async fn batch_download(
        &mut self,
        files: Vec<FtpDirEntry>,
        dest: &str,
    ) -> Result<Vec<FtpDirEntry>> {
        let mut failed_files: Vec<FtpDirEntry> = Vec::with_capacity(files.len());
        for file in files.iter() {
            let (path, &size) = match &file {
                FtpDirEntry::File(p, s) => (p.to_owned(), s),
                FtpDirEntry::Folder(..) => continue,
            };
            let ftp_root: String = path
                .to_owned()
                .chars()
                .take_while(|ch| *ch != '/')
                .chain("/".chars())
                .collect();
            let target_file_path = utils::get_download_target_path(&path, dest, &ftp_root);

            if let Ok(file) = tokio::fs::File::open(&target_file_path).await {
                //file already exists
                if let Ok(meta) = file.metadata().await {
                    if meta.len() == size as u64 {
                        //and it was fully downloaded
                        continue;
                    }
                }
            } else {
                match self.download_file(&path, &target_file_path).await {
                    Ok(()) => {
                        println!("downloaded file from {} to {}", path, &target_file_path);
                        continue;
                    }
                    Err(e) => {
                        println!("failed to download file to {}", &target_file_path);
                        println!("error: {:?}", e);
                        failed_files.push(file.clone());
                    }
                }
                //file does not exist
                //try to download
                //if failed => write to failed_files and continue to next iteration
            }
        }
        if !failed_files.is_empty() {
            return Err(CustomError::Ftp(format!(
                "failed to download some files: {:?}",
                failed_files
            )));
        }
        Ok(files)
    }

    pub async fn get_dir_entries(&mut self, path: &str) -> Result<Vec<FtpDirEntry>> {
        let entries = self.list(Some(path)).await?;
        let result: Vec<FtpDirEntry> = utils::parse_ftp_entries(entries, path);
        Ok(result)
    }
    //get_size returns size of entry at provided path.
    //none if no such entry, -1 if entry is dir
    //assuming no symlinks on ftp server
    pub async fn get_size(&mut self, path: &str) -> Option<isize> {
        if let Ok(opt) = self.size(path).await {
            match opt {
                Some(size) => Some(size as isize),
                None => Some(-1),
            }
        } else {
            None
        }
    }
    pub async fn download_file(&mut self, path: &str, to: &str) -> Result<()> {
        //check if destination folder path exists
        let mut dest = PathBuf::from(&to);
        dest.pop();
        tokio::fs::create_dir_all(&dest).await?;
        let mut file = tokio::fs::File::create(&to).await?;

        let cursor = self.simple_retr(path).await?;
        file.write_all(&cursor.into_inner()).await?;
        Ok(())
    }
    pub async fn remove_dir(&mut self, path: &str) -> Result<()> {
        self.rmdir(path).await.map_err(|e| e.into())
    }
    pub async fn get_ftp_entries(&mut self, path: &str) -> Result<Vec<FtpDirEntry>> {
        self.get_dir_entries(path).await
    }
    pub fn get_local_folder_path(&self) -> String {
        self.config.local_folder.to_owned()
    }
    pub fn get_watch_list(&self) -> Vec<String> {
        self.config.get_dirs_to_watch()
    }
}

impl std::ops::Deref for Connection {
    type Target = FtpStream;
    fn deref(&self) -> &Self::Target {
        &self.stream
    }
}
impl std::ops::DerefMut for Connection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.stream
    }
}
