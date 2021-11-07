use tokio::sync::{broadcast, mpsc};

use std::sync::{Arc, Mutex};

pub struct Sender<T> {
    sender: mpsc::Sender<T>
}

#[derive(Clone)]
pub struct Receiver<T> {
    receiver: Arc<Mutex<Option<mpsc::Receiver<T>>>>,
    bsender: broadcast::Sender<()>
}

impl<T> Sender<T> {
    pub fn send(&self, val: T) {
        self.sender.try_send(val).map_err(|err| {
            use mpsc::error::TrySendError;
            match err {
                TrySendError::Full(_) => "try_send full",
                TrySendError::Closed(_) => "try_send closed"
            }.to_string()
        }).unwrap()
    }
}

pub struct OwningBroadcastGuard<T> {
    data: Option<T>,
    sender: broadcast::Sender<()>
}

impl<T> OwningBroadcastGuard<T> {
    pub fn take(mut self) -> T {
        self.data.take().unwrap()
    }
}

impl<T> Drop for OwningBroadcastGuard<T> {
    fn drop(&mut self) {
        self.sender.send(());
    }
}

impl<T> Receiver<T> {
    pub async fn recv(&self) -> Option<OwningBroadcastGuard<T>> {
        loop {
            let receiver = {
                let mut receiver = self.receiver.lock().unwrap();
                receiver.take()
            };
            if let Some(mut real_recv) = receiver {
                let mut retval = real_recv.recv().await;
                while let Ok(val) = real_recv.try_recv() {
                    retval = Some(val);
                }
                if let Some(data) = retval {
                    *self.receiver.lock().unwrap() = Some(real_recv);
                    return Some(OwningBroadcastGuard{data: Some(data), sender: self.bsender.clone()});
                }
                self.bsender.send(());
                *self.receiver.lock().unwrap() = Some(real_recv);
                return None;
            } else {
                use broadcast::error::RecvError;
                match self.bsender.subscribe().recv().await {
                    Ok(_) => return None,
                    Err(RecvError::Lagged(_)) => (),
                    Err(RecvError::Closed) => return None
                }
            }
        }
    }
    pub fn try_recv(&self) -> Option<OwningBroadcastGuard<T>> {
        loop {
            if let Some(mut real_recv) = {
                let mut receiver = self.receiver.lock().unwrap();
                receiver.take()
            } {
                use mpsc::error::TryRecvError;
                match real_recv.try_recv() {
                    Ok(val) => {
                        let mut data = val;
                        while let Ok(val) = real_recv.try_recv() {
                            data = val;
                        }
                        *self.receiver.lock().unwrap() = Some(real_recv);
                        return Some(OwningBroadcastGuard{data: Some(data), sender: self.bsender.clone()});
                    },
                    Err(TryRecvError::Disconnected) => {
                        self.bsender.send(());
                        *self.receiver.lock().unwrap() = Some(real_recv);
                        return None;
                    },
                    Err(TryRecvError::Empty) => {
                    }
                }
                *self.receiver.lock().unwrap() = Some(real_recv);
            } else {
                use broadcast::error::TryRecvError;
                match self.bsender.subscribe().try_recv() {
                    Ok(_) => return None,
                    Err(TryRecvError::Lagged(_)) => (),
                    Err(TryRecvError::Closed) => return None,
                    Err(TryRecvError::Empty) => return None
                }
            }
        }
    }
}

pub fn channel<T>(capacity: usize) -> (Sender<T>, Receiver<T>) {
    let (sender, receiver) = mpsc::channel(capacity);
    let (bsender, _) = broadcast::channel(1);
    (Sender {sender}, Receiver{receiver: Arc::new(Mutex::new(Some(receiver))), bsender})
}
