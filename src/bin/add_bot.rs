#![allow(unused_imports)]

use tokio::sync::oneshot;

use std::convert::Infallible;
use std::net::SocketAddr;
use hyper::{Body, Request, Response, Client};
use hyper::body::HttpBody;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Method, StatusCode, Uri};
use url::Url;
use url::form_urlencoded::parse;

use std::collections::HashMap;
use std::sync::{Mutex, Condvar, RwLock, Arc};
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;

#[tokio::main]
async fn main() -> Result<(), String> {
    let client = Client::builder()
        .http2_only(true)
        .build_http::<hyper::Body>();
    let req = Request::builder()
        .method("POST")
        .uri(Uri::builder().scheme("http")
            .authority("localhost:8080")
            .path_and_query(format!("/add_bot"))
            .build()
            .unwrap())
        .body(Body::empty())
        .unwrap();
    let resp = client.request(req).await.unwrap();
    assert!(resp.status() == StatusCode::OK);
    Ok(())
}
