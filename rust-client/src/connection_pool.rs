use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use std::collections::VecDeque;
use std::future::Future;
use tracing::{debug, warn};

pub struct ConnectionPool<T> {
    connections: Arc<Mutex<VecDeque<T>>>,
    semaphore: Arc<Semaphore>,
    factory: Arc<dyn Fn() -> Box<dyn Future<Output = Result<T, tonic::transport::Error>> + Send> + Send + Sync>,
}

impl<T: Clone + Send + 'static> ConnectionPool<T> {
    pub async fn new<F, Fut, C>(
        size: usize,
        config: C,
        factory: F,
    ) -> Result<Self, tonic::transport::Error>
    where
        F: Fn(C) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T, tonic::transport::Error>> + Send + 'static,
        C: Clone + Send + 'static,
    {
        let connections = Arc::new(Mutex::new(VecDeque::with_capacity(size)));
        let semaphore = Arc::new(Semaphore::new(size));
        
        // Pre-create connections
        let mut initial_connections = Vec::with_capacity(size);
        for i in 0..size {
            debug!("Creating connection {} of {}", i + 1, size);
            let conn = factory(config.clone()).await?;
            initial_connections.push(conn);
        }
        
        {
            let mut pool = connections.lock().await;
            for conn in initial_connections {
                pool.push_back(conn);
            }
        }
        
        let factory = Arc::new(move |_: ()| -> Box<dyn Future<Output = Result<T, tonic::transport::Error>> + Send> {
            let config = config.clone();
            Box::new(factory(config))
        });
        
        Ok(Self {
            connections,
            semaphore,
            factory,
        })
    }

    pub async fn get(&self) -> Result<PooledConnection<T>, tonic::transport::Error> {
        let permit = self.semaphore.acquire().await.unwrap();
        
        let connection = {
            let mut pool = self.connections.lock().await;
            pool.pop_front()
        };
        
        let connection = match connection {
            Some(conn) => conn,
            None => {
                warn!("Connection pool empty, creating new connection");
                (self.factory)(()).await?
            }
        };
        
        Ok(PooledConnection {
            connection: Some(connection),
            pool: self.connections.clone(),
            _permit: permit,
        })
    }
}

pub struct PooledConnection<T> {
    connection: Option<T>,
    pool: Arc<Mutex<VecDeque<T>>>,
    _permit: tokio::sync::SemaphorePermit<'static>,
}

impl<T> std::ops::Deref for PooledConnection<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.connection.as_ref().unwrap()
    }
}

impl<T> std::ops::DerefMut for PooledConnection<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.connection.as_mut().unwrap()
    }
}

impl<T> Drop for PooledConnection<T> {
    fn drop(&mut self) {
        if let Some(connection) = self.connection.take() {
            let pool = self.pool.clone();
            tokio::spawn(async move {
                let mut pool = pool.lock().await;
                pool.push_back(connection);
            });
        }
    }
}