use crate::Config;
use crate::Connection;
use crate::CustomError;
use crate::Result;
use tokio::sync::{Mutex, MutexGuard};

const CONN_RETRY_ATTEMPTS: i32 = 5;
#[derive(Debug)]
pub struct Pool {
    inner: Vec<Mutex<Connection>>,
}
impl Pool {
    pub async fn with_config(config: &Config) -> Result<Self> {
        let size = config.connections;
        let mut retry_count = 0;
        let mut inner = Vec::with_capacity(size);

        while inner.len() < size {
            if let Ok(conn) = Connection::with_config(&config).await {
                inner.push(Mutex::new(conn));
            } else {
                retry_count += 1;
            }
            if !inner.is_empty() && retry_count > CONN_RETRY_ATTEMPTS {
                break;
            }
        }
        //if inner.is_empty() { return Err(CustomError::Ftp("cannot establish connection".into()))}
        Ok(Self { inner })
    }

    pub async fn get_free_connection(&'static self) -> Result<MutexGuard<'static, Connection>> {
        //iterate Vec<Mutex<Connection>> in a loop trying to aquire lock on each mutex
        //if lock is aquired => connection is free and can be used => return this Connection
        //return Err if tried too many times
        let mut count = 0;
        loop {
            for mx in (*self.inner).iter() {
                if let Ok(lock) = mx.try_lock() {
                    return Ok(lock);
                }
            }
            count += 1;
            if count > CONN_RETRY_ATTEMPTS {
                return Err(CustomError::Ftp("all connections are busy".into()));
            }
        }
    }
    pub fn len(&self) -> usize {
        self.inner.len()
    }
    pub fn get_connections(&self) -> &Vec<Mutex<Connection>> {
        &self.inner
    }
}
