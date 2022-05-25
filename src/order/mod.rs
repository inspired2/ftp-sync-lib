use crate::{utils, Connection, CustomError, FtpDirEntry, Result};
use std::{cmp::Ordering, path::PathBuf};
use tokio::sync::MutexGuard;

#[derive(Debug, Clone)]
pub struct Order {
    root_path: PathBuf,
    download_target_path: PathBuf,
    files: Option<Vec<FtpDirEntry>>,
    folders: Option<Vec<FtpDirEntry>>,
}

impl Ord for Order {
    fn cmp(&self, other: &Order) -> Ordering {
        self.root_path
            .iter()
            .count()
            .cmp(&other.root_path.iter().count())
    }
}
impl PartialOrd for Order {
    fn partial_cmp(&self, other: &Order) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for Order {}
impl PartialEq for Order {
    fn eq(&self, other: &Order) -> bool {
        self.root_path == other.root_path
    }
}
impl Order {
    pub fn new(dir: &FtpDirEntry, local_dest: &str) -> Self {
        let root_path = std::path::Path::new(&dir.get_full_path()).to_path_buf();
        let download_target_path = std::path::Path::new(local_dest).to_path_buf();

        Self {
            root_path,
            download_target_path,
            files: None,
            folders: None,
        }
    }
    pub async fn download(
        &self,
        conn: &mut MutexGuard<'_, Connection>,
    ) -> Result<Vec<FtpDirEntry>> {
        if self.files.is_none() {
            return Err(CustomError::Io(
                "job was not initialized properly (file list is None)".into(),
            ));
        }
        let files = self.get_files_list().unwrap();
        let dest = self.download_target_path.to_str().unwrap();
        conn.batch_download(files, dest).await
    }

    pub fn get_root_path(&self) -> String {
        self.root_path
            .to_owned()
            .to_str()
            .expect("cannot parse PathBuf to string")
            .to_string()
    }

    pub async fn read_all_entries(&mut self, conn: &mut MutexGuard<'_, Connection>) -> Result<()> {
        let mut entries = conn.get_dir_entries(&self.get_root_path()).await?;
        let (mut folders, mut files) = utils::categorize_entries(&mut entries);
        let mut output_folders = folders.clone();
        output_folders.push(FtpDirEntry::Folder(self.get_root_path()));

        while !folders.is_empty() {
            let folder = folders.pop().unwrap(); //unwrap is safe as folders was not empty
            let path = match folder {
                FtpDirEntry::Folder(path) => path,
                _ => unreachable!(),
            };
            entries = conn.get_dir_entries(&path).await?;
            let (mut fol, mut fil) = utils::categorize_entries(&mut entries);
            let mut cloned_fol = fol.clone();
            folders.append(&mut fol);
            output_folders.append(&mut cloned_fol);
            files.append(&mut fil);
        }

        output_folders.sort();
        self.folders = Some(output_folders);
        self.files = Some(files);

        Ok(())
    }
    pub fn get_files_list(&self) -> Option<Vec<FtpDirEntry>> {
        self.files.to_owned()
    }
    pub fn get_folders_list(&self) -> Option<Vec<FtpDirEntry>> {
        self.folders.to_owned()
    }
    pub async fn is_ready_for_download(&self, conn: &mut MutexGuard<'_, Connection>) -> bool {
        let ready_flag_file_ext = &conn.get_ready_flag().to_owned();
        if let Ok(entries) = conn.get_dir_entries(&self.get_root_path()).await {
            entries.iter().any(|ent| {
                !ent.is_dir()
                    && ent
                        .get_full_path()
                        .chars()
                        .rev()
                        .take(3)
                        .collect::<String>()
                        .to_lowercase()
                        == *ready_flag_file_ext
            })
        } else {
            false
        }
    }
}
