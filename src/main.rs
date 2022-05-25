use ftp_sync::{Config, Controller, Pool, Result, Watcher};
use once_cell::sync::OnceCell;
use std::env::current_dir;
use tokio::join;

static CFG_FILENAME: &str = "config.json";
static mut WATCHER: OnceCell<Watcher> = OnceCell::new();
static mut CONTROLLER: OnceCell<Controller> = OnceCell::new();

#[tokio::main]
async fn main() -> Result<()> {
    //get path to config.json
    let config_dir = current_dir().expect("could not get CWD");
    let config = Config::new(config_dir, CFG_FILENAME).await?;
    let healthcheck_interval = config.get_healthcheck_interval();
    //create pool of connections to ftp server;
    let pool: Pool = Pool::with_config(&config).await?;
    //create watcher passing ownership to the pool;
    let watcher: Watcher = Watcher::with_pool(pool).await?;

    //using once_cell as we need watcher and controller to be static;
    unsafe {
        WATCHER.set(watcher).expect("error setting up watcher");
    }

    //controller manage connections to ftp through arc;
    //we must take ref to the pool from watcher as it owns the arc to the running pool;
    let pool_arc = unsafe { WATCHER.get_mut().unwrap().get_pool_ref() };
    let controller = Controller::new(pool_arc);
    unsafe {
        //set controller to once_cell;
        CONTROLLER
            .set(controller)
            .expect("error setting up connection controller");
    }

    //start the controller;
    //it will periodically iterate over connections in the pool trying to aquire the mutexlock
    //and check connection;
    //if connection is bad - reconnect;
    let controller;
    let watcher;

    unsafe {
        controller = CONTROLLER.get_mut().unwrap().start(healthcheck_interval);
        //watcher will never finish watching
        //it will check ftp-server for folders to download, download and remove them from ftp-server
        watcher = WATCHER.get().unwrap().watch();
    }

    join!(controller, watcher);
    Ok(())
}
