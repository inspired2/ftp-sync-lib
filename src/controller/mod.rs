use crate::Pool;
use std::sync::Arc;
use tokio::{
    spawn,
    //task::JoinHandle,
    time::{interval, Duration},
};

/// Controller spawns a task that periodically polls each connection to keep it alive
/// If connection is closed => replace it with newly created one
#[derive(Debug)]
pub struct Controller {
    connections: Arc<Pool>,
}
impl Controller {
    pub fn new(pool: Arc<Pool>) -> Self {
        Self { connections: pool }
    }
    pub fn start(&'static mut self, healthcheck_interval_sec: u16) -> impl std::future::Future {
        spawn(async move {
            let mut interval = interval(Duration::from_secs(healthcheck_interval_sec as u64));
            loop {
                interval.tick().await;

                // loop through connections
                // if can't get the lock => connection is busy
                // if lock acquired => check conn status:
                // if bad => try to restore
                // if failed to restore => continue
                for ftp_conn in self.connections.get_connections().iter() {
                    println!("checking connection..");
                    if let Ok(mut conn) = ftp_conn.try_lock() {
                        println!("acquired conn mutex. Checking ftp response..");
                        if conn.noop().await.is_err() {
                            println!("connection degraded, restoring");
                            conn.restore().await.ok();
                        } else {
                            println!("connection is Ok!");
                        }
                    } else {
                        println!("connection is busy");
                    }
                }
            }
        })
    }
}
