#![allow(unused_imports)]

use pokerrs::card::*;
use pokerrs::table::*;
use pokerrs::gamestate::*;
use pokerrs::bot_always_call::*;
use pokerrs::game::*;
use pokerrs::server::*;

use rand::prelude::*;

use std::convert::Infallible;
use std::net::SocketAddr;
use hyper::{Body, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() -> Result<(), PokerRoundError> {
    let table_rules = TableRules {
        ante: AnteRule::Ante(1),
        ante_name: "ante".to_string(),
        min_bet: 1
    };
    GameServer::create_and_serve(table_rules).await;

    Ok(())
}
