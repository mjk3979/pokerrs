use crate::game::*;
use crate::viewstate::*;
use crate::server::{ServerPlayer, ServerActionRequest, ServerUpdate};

use async_trait::async_trait;

use tokio::sync::oneshot;

use std::convert::Infallible;
use std::net::SocketAddr;
use hyper::{Body, Request, Response, Client};
use hyper::body::HttpBody;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Method, StatusCode, Uri};
use hyper::client::connect::Connect;
use hyper::client::HttpConnector;
use url::Url;
use url::form_urlencoded::parse;
use serde::Serialize;

use std::collections::HashMap;
use std::sync::{Mutex, Condvar, RwLock, Arc};
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;

pub struct PokerHttpClient<P> {
    pub name: String,
    pub address: String,
    pub input: Mutex<P>
}

pub type PokerClientResult = Result<(), String>;

impl<P: PlayerInputSource + PokerViewClient> PokerHttpClient<P> {
    async fn bet(&mut self, min_bet: Chips, call_amount: Chips) -> BetResp {
        let mut input_lock = self.input.lock().unwrap();
        input_lock.bet(min_bet, call_amount).await
    }

    pub async fn start(&mut self) -> PokerClientResult {
        let client = Client::builder()
            .http2_only(true)
            .build_http::<hyper::Body>();
        let mut start_from = 0;
        loop {
            println!("sending request");
            let mut resp = client.get(Uri::builder().scheme("http")
                .authority(self.address.clone())
                .path_and_query(format!("/gamediff?player={}&start_from={}", self.name, start_from))
                .build()
                .unwrap()).await.map_err(|e| e.to_string())?;
            let mut body: Vec<u8> = Vec::new();
            println!("received response");
            while let Some(chunk) = resp.body_mut().data().await {
                body.extend_from_slice(&chunk.unwrap());
            }
            println!("received body");
            if resp.status() != StatusCode::OK {
                return Err(format!("Server Error {}: {}", resp.status(), std::str::from_utf8(&body).unwrap_or("")));
            }
            println!("Response: {}", std::str::from_utf8(&body).unwrap_or("body parse err"));
            let ServerUpdate{player: server_player, log: viewdiffs, table, ..} = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
            start_from += viewdiffs.iter().map(|l| l.log.len()).sum::<usize>();
            {
                let mut input_lock = self.input.lock().unwrap();
                input_lock.update_table(table);
            }
            if let Some(server_player) = server_player {
                if let Some(viewstate) = server_player.viewstate {
                        let mut input_lock = self.input.lock().unwrap();
                        input_lock.update(PokerViewUpdate{
                            viewstate,
                            diff: viewdiffs}
                        );
                }

                if let Some(action) = server_player.action_requested {
                    let (path, json) = match action {
                        ServerActionRequest::Bet { min_bet, call_amount } => {
                            println!("Requested bet: {} {}", min_bet, call_amount);
                            let resp = self.bet(call_amount, min_bet).await;
                            ("/bet", serde_json::to_vec(&resp).unwrap())
                        },
                        ServerActionRequest::Replace => {
                            panic!("unimp");
                        },
                        ServerActionRequest::DealersChoice{variants} => {
                            let mut input_lock = self.input.lock().unwrap();
                            let resp = input_lock.dealers_choice(variants).await;
                            ("/dealers_choice", serde_json::to_vec(&resp).unwrap())
                        }
                    };
                    let mut req = Request::builder()
                        .method("POST")
                        .uri(Uri::builder().scheme("http")
                            .authority(self.address.clone())
                            .path_and_query(format!("{}?player={}", path, self.name))
                            .build()
                            .unwrap())
                        .body(Body::from(json))
                        .unwrap();
                    let mut action_resp = client.request(req).await.map_err(|e| e.to_string())?;
                    if action_resp.status() != StatusCode::OK {
                        let mut body: Vec<u8> = Vec::new();
                        while let Some(chunk) = action_resp.body_mut().data().await {
                            body.extend_from_slice(&chunk.unwrap());
                        }
                        return Err(format!("Server Error on action {}: {}", action_resp.status(), std::str::from_utf8(&body).unwrap_or("")));
                    }
                }
            }
            // TODO end check
        }
        Ok(())
    }
}
