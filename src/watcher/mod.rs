pub use crate::Config;
pub use crate::Controller;
pub use crate::CustomError;
pub use crate::FtpDirEntry;
pub use crate::Order;
pub use crate::Pool;
pub use crate::{Connection, Result};
use std::sync::Arc;
use tokio::{
    join, spawn,
    sync::{Mutex, MutexGuard},
};

#[derive(Debug)]
pub struct Watcher {
    connections_pool: Arc<Pool>,
    pending_downloads: Mutex<Vec<Order>>,
    running_downloads: Mutex<Vec<Order>>,
}

impl Watcher {
    pub async fn with_pool(pool: Pool) -> Result<Watcher> {
        Ok(Watcher {
            connections_pool: Arc::new(pool),
            pending_downloads: Mutex::new(Vec::new()),
            running_downloads: Mutex::new(Vec::new()),
        })
    }
    /// This function will never return;
    /// Watcher maintains pending queue and pool of connections;
    /// It loops infinitely throug pending queue and processes contained tasks;
    /// At the same time it scans ftp folders (paths provided through connection config);
    /// When new folder appear in ftp root - a new task(Order) is build and put in pending queue;
    pub async fn watch(&'static self) {
        let downloader_handle = self.spawn_downloader_worker();
        let watcher_handle = self.spawn_ftp_watcher_worker();
        let (_, _) = join!(watcher_handle, downloader_handle);
    }
    ///Get arc to connections pool
    pub fn get_pool_ref(&self) -> Arc<Pool> {
        self.connections_pool.clone()
    }
    ///Get free connection from pool as mutex guard
    pub async fn get_connection(&'static self) -> Result<MutexGuard<'static, Connection>> {
        self.connections_pool.get_free_connection().await
    }

    ///Returns list of all found subfolders at remote root folders (can be many, provided by config);
    /// These are considered to be job units (Orders);
    /// Only subfolders, without recursive walkthrough;
    async fn get_root_subfolders(conn: &mut MutexGuard<'_, Connection>) -> Vec<FtpDirEntry> {
        let mut folders_list = Vec::new();
        let root_folders = Watcher::get_watch_list(conn);
        for root in root_folders {
            if let Ok(folders) = conn.get_ftp_entries(&root).await {
                for folder in folders {
                    if folder.is_dir() {
                        folders_list.push(folder);
                    }
                }
            }
        }
        folders_list
    }
    ///Gets list of folders to watch for job units (Orders);
    /// Provided by config;
    fn get_watch_list(conn: &MutexGuard<Connection>) -> Vec<String> {
        conn.get_watch_list()
    }
    ///Spawns task that starts infinite loop checking ftp root for folders;
    /// For every found folder - check if it is finished being written (and thus can be downloaded safely);
    /// The folder considered ready for download when it contains *.txt file (down one step, not further in subfolders tree);
    /// If so => put it in pending downloads queue;
    fn spawn_ftp_watcher_worker(&'static self) -> tokio::task::JoinHandle<()> {
        spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
            interval.tick().await;

            //on every iteration:
            //scan root ftp folder for all present subfolders(orders)
            //check if folder is ready to be downloaded
            //if so => put it in pending queue
            loop {
                if let Ok(mut conn) = self.get_connection().await {
                    println!(
                        "Remote watcher got connection from pool! checking ftp root folders..."
                    );
                    let subfolders = Watcher::get_root_subfolders(&mut conn).await;
                    let download_target_folder = conn.get_local_folder_path();

                    //loop through all found subfolders
                    for folder in subfolders {
                        let mut job = Order::new(&folder, &download_target_folder);
                        let is_running = self.running_downloads.lock().await.contains(&job);
                        let is_pending = self.pending_downloads.lock().await.contains(&job);
                        //check if is ready to be downloaded
                        if !is_running && !is_pending && job.is_ready_for_download(&mut conn).await
                        {
                            //finalize job creation & push to pending queue
                            //if fails - job will be processed on next iteration
                            if let Ok(()) = job.read_all_entries(&mut conn).await {
                                self.push_pending(job).await;
                            }
                        }
                    }
                    drop(conn);
                } else {
                    //if didn't get connection => try again later
                    println!("ftp watcher couldn't get free connection from pool, repeating...");
                }
                interval.tick().await;
            }
        })
    }
    ///Spawns a task that runs infinite loop:
    /// On every loop iteration check queue for pending tasks;
    /// If queue is not empty - get next task and run download process;
    fn spawn_downloader_worker(&'static self) -> tokio::task::JoinHandle<()> {
        spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
            interval.tick().await;

            //on every iteration check pending queue
            //if any job present => extract, put in running queue
            //spawn task to move files from ftp to local folder
            loop {
                //try get free connection from pool of connections:
                if let Ok(conn) = self.get_connection().await {
                    println!("Downloader got connection from pool! Checking pending queue..");

                    if let Some(job) = self.get_pending().await {
                        self.insert_runnning(job.to_owned()).await;
                        self.spawn_move_task(job, conn);
                    } else {
                        drop(conn);
                    }
                } else {
                    //if can't get free connection => wait for 2 seconds:
                    println!("Downloader couldn't get free connection from pool, repeating...");
                }
                interval.tick().await;
            }
        })
    }
    ///Spawn download task that copies folder (specified by provided Order struct) from ftp to local folder
    /// then removes it from ftp (if downloaded successfully)
    fn spawn_move_task(
        &'static self,
        job: Order,
        mut conn: MutexGuard<'static, Connection>,
    ) -> tokio::task::JoinHandle<()> {
        spawn(async move {
            let job_path = job.get_root_path();
            //if downloaded successfully
            if let Ok(files) = job.download(&mut conn).await {
                //first remove all files from ftp
                if let Err(e) = conn.batch_delete_remote(&files).await {
                    println!(
                        "error while batch removing files: {:?}, error: {:?}",
                        files, e
                    );
                }
                //then remove all folders from ftp
                let folders = job.get_folders_list().unwrap_or_default();
                if let Err(e) = conn.batch_delete_remote(&folders).await {
                    println!("error while removing dir: {:?}, error: {:?}", &job_path, e);
                }
                self.remove_from_runnig(&job).await;
            }
        })
    }
    ///Extract next job from queue of pending jobs if any
    async fn get_pending(&self) -> Option<Order> {
        let mut pending = self.pending_downloads.lock().await;
        if pending.is_empty() {
            return None;
        }
        Some(pending.remove(0))
    }
    ///Push provided job to pending queue
    async fn push_pending(&self, job: Order) {
        let mut pending = self.pending_downloads.lock().await;
        if pending.contains(&job) {
            return;
        }
        pending.push(job);
    }
    ///Push provided job to list of running jobs
    async fn insert_runnning(&self, job: Order) {
        let mut running = self.running_downloads.lock().await;
        if running.contains(&job) {
            return;
        }
        running.push(job);
    }
    ///Remove provided job from list of running, discarding it
    async fn remove_from_runnig(&self, job: &Order) {
        let mut running = self.running_downloads.lock().await;
        *running = running.clone().into_iter().filter(|j| j != job).collect();
    }
}
