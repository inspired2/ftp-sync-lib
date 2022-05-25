use ftp_cmd_list_parse::{FtpEntry, FtpEntryKind};
use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FtpDirEntry {
    File(String, usize),
    Folder(String),
}
impl Ord for FtpDirEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        let path = self.get_full_path();
        let other_path = other.get_full_path();
        path.cmp(&other_path)
    }
}

impl PartialOrd for FtpDirEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let path = self.get_full_path();
        let other_path = other.get_full_path();
        Some(path.cmp(&other_path))
    }
}

impl FtpDirEntry {
    pub fn get_full_path(&self) -> String {
        match &self {
            FtpDirEntry::File(path, ..) => path.to_owned(),
            FtpDirEntry::Folder(path) => path.to_owned(),
        }
    }
    pub fn is_dir(&self) -> bool {
        matches!(self, &FtpDirEntry::Folder(..))
    }
}

pub fn categorize_entries(entries: &mut Vec<FtpDirEntry>) -> (Vec<FtpDirEntry>, Vec<FtpDirEntry>) {
    let mut folders = Vec::new();
    let mut files = Vec::new();
    while let Some(ent) = entries.pop() {
        match ent {
            FtpDirEntry::File(..) => files.push(ent),
            FtpDirEntry::Folder(..) => folders.push(ent),
        }
    }
    (folders, files)
}

pub fn parse_ftp_entries(entries: Vec<String>, path: &str) -> Vec<FtpDirEntry> {
    let mut output: Vec<FtpDirEntry> = Vec::with_capacity(entries.len());
    for ent in entries {
        if let Some(ftp_entry) = FtpEntry::new(&ent) {
            let name = ftp_entry.name();
            match ftp_entry.kind() {
                FtpEntryKind::Directory => {
                    let dir = FtpDirEntry::Folder(get_absolute_path(path, name));
                    output.push(dir);
                }
                FtpEntryKind::File => {
                    let p = get_absolute_path(path, name);
                    let size = ftp_entry.size();
                    let file = FtpDirEntry::File(p, size);
                    output.push(file);
                }
                _ => {}
            }
        }
    }

    output
}

fn get_absolute_path(path: &str, name: &str) -> String {
    path.chars()
        .chain("/".chars())
        .chain(name.chars())
        .collect()
}
pub fn get_download_target_path(
    ftp_full_path: &str,
    target_folder: &str,
    ftp_root: &str,
) -> String {
    let order_path: String = ftp_full_path.to_owned().replace(ftp_root, "");
    target_folder
        .chars()
        .chain("/".chars())
        .chain(order_path.chars())
        .collect()
}
