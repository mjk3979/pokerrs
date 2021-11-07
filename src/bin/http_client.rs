#![allow(unused_imports)]

use pokerrs::http_client::*;
use pokerrs::cmdline::*;
use pokerrs::game::*;

use async_trait::async_trait;

use std::collections::HashMap;
use std::sync::{Mutex, Condvar, RwLock, Arc};
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;

#[tokio::main]
async fn main() -> Result<(), String> {
    let stdin = std::io::stdin();
    let cmdline = Mutex::new(CmdlineInputSource::new());
    PokerHttpClient {
        name: "matt".to_string(),
        address: "127.0.0.1:8080".to_string(),
        input: cmdline
    }.start().await?;
    Ok(())
}
