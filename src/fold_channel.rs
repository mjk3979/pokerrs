use tokio::sync::{watch};

use std::sync::{Mutex, Arc};

#[derive(Clone)]
pub struct Sender<T, A> {
    private: Arc<Mutex<PrivateSender<T, A>>>
}

struct PrivateSender<T, A> {
    acc: Option<A>,
    f: fn (&mut A, T),
    tx: watch::Sender<A>
}

pub type Receiver<A> = watch::Receiver<A>;

impl<T: Clone, A> Sender<T, A> {
    pub fn send(&self, t: T) {
        let mut sender = self.private.lock().unwrap();
        (sender.f)(&mut sender.acc.as_mut().unwrap(), t.clone());
        let acc = sender.acc.take().unwrap();
        sender.acc = Some(sender.tx.send_replace(acc));
        // do it again because we swapped the buffer
        (sender.f)(&mut sender.acc.as_mut().unwrap(), t);
    }
}

pub fn channel<T: Clone, A: Clone>(start: A, f: fn (&mut A, T)) -> (Sender<T, A>, Receiver<A>) {
    let (tx, rx) = watch::channel(start.clone());
    let private = Arc::new(Mutex::new(PrivateSender {
        acc: Some(start),
        f,
        tx
    }));
    (Sender {private}, rx)
}
