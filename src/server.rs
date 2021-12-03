use crate::card::*;
use crate::game::*;
use crate::table::*;
use crate::viewstate::*;
use crate::fold_channel;
use crate::gamestate::{play_poker};
use crate::bot_always_call::BotAlwaysCallInputSource;
use crate::static_files::*;

use ts_rs::{TS, export};

use async_trait::async_trait;
use serde::{Serialize, Deserialize};

use std::convert::Infallible;
use std::net::SocketAddr;
use hyper::{Body, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Method, StatusCode};
use hyper::header::HeaderValue;
use url::Url;
use url::form_urlencoded::parse;

use std::collections::HashMap;
use std::sync::{Mutex, Condvar, RwLock, Arc};
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;
use tokio::sync::watch;
use tokio::sync::oneshot;
use tokio::sync::broadcast;

pub struct PlayerConnection {
    pub input_source: Arc<GameServerPlayerInputSource>
}

#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
#[derive(TS)]
#[serde(tag = "kind", content="data")]
pub enum ServerActionRequest {
    Bet {
        call_amount: Chips,
        min_bet: Chips
    },
    Replace {
        max_can_replace: usize,
    },
    DealersChoice {
        variants: Vec<PokerVariantDesc>,
    },
}


#[derive(Clone, Debug, Serialize, Deserialize)]
#[derive(TS)]
pub struct ServerPlayer {
    pub viewstate: Option<PokerViewState>,
    pub action_requested: Option<ServerActionRequest>
}

#[derive(Clone, Serialize, Deserialize)]
#[derive(TS)]
pub struct ServerUpdate {
    pub player: Option<ServerPlayer>,
    pub log: Vec<PokerLogUpdate>,
    pub slog: Option<Vec<Vec<String>>>,
    pub table: TableViewState
}

pub struct GameServer {
    table: Table,
    players: Mutex<HashMap<PlayerId, Arc<GameServerPlayerInputSource>>>,
    log_update_channel_r: fold_channel::Receiver<Vec<PokerGlobalViewDiff<PlayerId>>>,
    bots: Mutex<Vec<Arc<BotAlwaysCallInputSource>>>,
    static_files: StaticFiles,
    //log_update_channel_t: fold_channel::Sender<Vec<PokerGlobalViewDiff>>
}

pub struct GameServerPlayerInputSource {
    update_tx: watch::Sender<Option<PokerViewUpdate>>,
    update_rx: watch::Receiver<Option<PokerViewUpdate>>,
    action_tx: watch::Sender<Option<ServerActionRequest>>,
    action_rx: watch::Receiver<Option<ServerActionRequest>>,
    bet_tx : watch::Sender<Option<BetResp>>,
    bet_rx: watch::Receiver<Option<BetResp>>,
    replace_tx: watch::Sender<Option<ReplaceResp>>,
    replace_rx: watch::Receiver<Option<ReplaceResp>>,
    dealers_choice_tx: watch::Sender<usize>,
    dealers_choice_rx: watch::Receiver<usize>,
}

impl GameServerPlayerInputSource {
    fn new() -> GameServerPlayerInputSource {
        let (update_tx, update_rx) = watch::channel(None);
        let (action_tx, action_rx) = watch::channel(None);
        let (bet_tx, bet_rx) = watch::channel(None);
        let (replace_tx, replace_rx) = watch::channel(None);
        let (dealers_choice_tx, dealers_choice_rx) = watch::channel(0);
        GameServerPlayerInputSource {
            update_tx,
            update_rx,
            action_tx,
            action_rx,
            bet_tx,
            bet_rx,
            replace_tx,
            replace_rx,
            dealers_choice_tx,
            dealers_choice_rx,
        }
    }
    fn server_player(&self) -> ServerPlayer {
        //println!("Update checked");
        let update = self.update_rx.borrow().clone();
        let viewstate = update.map(|u| u.viewstate);
        let action_requested = self.action_rx.borrow().clone();
        ServerPlayer { viewstate, action_requested }
    }
}

#[async_trait]
impl PlayerInputSource for GameServerPlayerInputSource {
    async fn bet(&self, call_amount: Chips, min_bet: Chips) -> BetResp {
        let mut rx = self.bet_rx.clone();
        rx.borrow_and_update();
        self.action_tx.send(Some(ServerActionRequest::Bet {
                call_amount,
                min_bet
        }));
        loop {
            rx.changed().await;
            if let Some(resp) = *rx.borrow() {
                self.action_tx.send(None);
                return resp;
            }
        }
    }

    async fn replace(&self, max_can_replace: usize) -> ReplaceResp {
        let mut rx = self.replace_rx.clone();
        rx.borrow_and_update();
        self.action_tx.send(Some(ServerActionRequest::Replace {
            max_can_replace
        }));
        loop {
            rx.changed().await;
            if let Some(retval) = rx.borrow().clone() {
                self.action_tx.send(None);
                return retval;
            }
        }
    }

    async fn dealers_choice(&self, variants: Vec<PokerVariantDesc>) -> usize {
        let mut rx = self.dealers_choice_rx.clone();
        rx.borrow_and_update();
        self.action_tx.send(Some(ServerActionRequest::DealersChoice {
            variants
        }));
        rx.changed().await;
        let retval = rx.borrow().clone();
        self.action_tx.send(None);
        retval
    }

    fn update(&self, update: PokerViewUpdate) {
        //println!("Update sent");
        self.update_tx.send(Some(update));
    }
}


fn player_from_params(params: &HashMap<String, String>) -> Option<PlayerId> {
    params.get("player").cloned()
}

async fn shutdown_signal() {
    // Wait for the CTRL+C signal
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C signal handler");
}

impl GameServer {
    pub async fn create_and_serve<'a>(table_rules: TableRules) {
        let table_config = TableConfig {
            max_players: 4,
            starting_chips: 1000,
            //variant_selector: PokerVariantSelector::Rotation(PokerVariants::all()),
            variant_selector: PokerVariantSelector::DealersChoice(PokerVariants::all()),
            //variant_selector: PokerVariantSelector::Rotation(PokerVariants {
                //descs: vec![PokerVariantDesc {
                    //name: "Five Card Draw".to_string()
                //}],
                //variants: vec![five_card_draw()]
            //}),
        };
        let static_files = StaticFiles::from_dir_path("ts/static");
        let table = Table::new(table_config, table_rules, |round| AnteRule::Ante((1 << ((round+1)/2)) as i64));
        let table_spectator_rx = table.spectator_rx.clone();
        let server = Arc::new({
            //let (log_update_channel_t, log_update_channel_r) = watch::channel(());
            GameServer {
                table,
                players: Mutex::new(HashMap::new()),
                log_update_channel_r: table_spectator_rx,
                bots: Mutex::new(Vec::new()),
                static_files
                //log_update_channel_t: spectator_tx.clone()
            }
        });

        let addr = ([0, 0, 0, 0], 8080).into();
        
        let http_server = Server::bind(&addr)
            .serve(make_service_fn(|conn| {
                println!("Got conn {:?}!", conn);
                let cserver = server.clone();
                async move {
                    Ok::<_, Infallible>(service_fn(move |req| {
                        println!("Got request!");
                        let server = cserver.clone();
                        async move {
                            server.serve(req).await
                        }
                    }))
                }
            }))
            .with_graceful_shutdown(shutdown_signal());

        println!("Serving on port 8080...");

        let tserver = server.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_signal() => break,
                    next_round = tserver.table.next_round() => {
                        if !next_round {
                            break;
                        }
                    }
                }
            }
        });

        http_server.await;
    }
        
    pub async fn notify_player<'a>(&'a self, player_id: PlayerId, start_from: usize, known_action_requested: Option<ServerActionRequest>) -> (Vec<PokerLogUpdate>, ServerPlayer) {
        let player = self.get_player(player_id.clone()).unwrap();
        let mut action_rx = player.action_tx.subscribe();
        let mut log_update_channel = self.log_update_channel_r.clone();
        let mut table_view_rx = self.table.table_view_rx.clone();
        table_view_rx.borrow_and_update();
        let mut table_changed = false;
        loop {
            {
                let source = &player;
                action_rx.borrow_and_update();
                let server_player = source.server_player();
                //println!("Checking log");
                let retlog = self.table.logs(Some(&player_id), start_from);
                if !retlog.is_empty() || server_player.action_requested != known_action_requested || table_changed {
                    println!("Notifying {} {} {}", !retlog.is_empty(), server_player.action_requested != known_action_requested, table_changed);
                    return (retlog, server_player);
                }
                println!("Not notifying");
            }
            
            tokio::select! {
                l = log_update_channel.changed() => { },
                a = action_rx.changed() => {},
                _ = table_view_rx.changed() => {
                    table_changed = true;
                },
            }
        }
    }

    pub async fn notify_logs<'a>(&'a self, start_from: usize) -> Vec<PokerLogUpdate> {
        let mut log_update_channel = self.log_update_channel_r.clone();
        let mut table_view_rx = self.table.table_view_rx.clone();
        table_view_rx.borrow_and_update();
        let mut table_changed = false;
        loop {
            let retlog = self.table.logs(None, start_from);
            if !retlog.is_empty() || table_changed {
                return retlog;
            }
            tokio::select! {
                _ = log_update_channel.changed() => {},
                _ = table_view_rx.changed() => {
                    table_changed = true;
                },
            }
        }
    }

    pub fn get_player(&self, player_id: PlayerId) -> Option<Arc<GameServerPlayerInputSource>> {
        use std::collections::hash_map::Entry::*;
        let mut players = self.players.lock().unwrap();
        match players.entry(player_id.clone()) {
            Occupied(e) => Some(e.get().clone()),
            Vacant(v) => {
                let new_player = Arc::new(GameServerPlayerInputSource::new());
                match self.table.join(player_id, new_player.clone()) {
                    Ok(()) => {
                        v.insert(new_player.clone());
                        Some(new_player)
                    },
                    Err(JoinError::Full) => None,
                    Err(JoinError::AlreadyJoined) => panic!("Impossible!")
                }
            }
        }
    }

    pub async fn serve(&self, req: Request<Body>) -> Result<Response<Body>, Infallible> {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::BAD_REQUEST;
        let params = req.uri().query().map(|q| url::form_urlencoded::parse(q.as_bytes()).into_owned().collect()).unwrap_or(HashMap::new());
        println!("{}: {}", req.method(), req.uri().path());
        match (req.method(), req.uri().path()) {
            (&Method::GET, "/game") => {
                if let Some(player_id) = player_from_params(&params) {
                    if let Some(input_source) = self.get_player(player_id) {
                        let player = input_source.server_player();
                        match serde_json::to_vec(&ServerUpdate {
                            player: Some(player),
                            log: Vec::new(),
                            slog: None,
                            table: self.table.table_view_rx.borrow().clone()
                        }) {
                            Ok(v) => {
                                *response.body_mut() = Body::from(v);
                                *response.status_mut() = StatusCode::OK;
                            },
                            Err(error) => {
                                *response.body_mut() = Body::from(error.to_string());
                                *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                            }
                        };
                    }
                }
            },
            (&Method::GET, "/gamediff") => {
                if let Some(start_from) = params.get("start_from").map(|p| p.parse::<usize>().ok()).unwrap_or(None) {
                    if let Some(player_id) = player_from_params(&params) {
                        let known_action_requested = params.get("known_action_requested").map(|t| serde_json::from_str::<Option<ServerActionRequest>>(&t).ok().flatten()).flatten();
                        if let Some(_) = self.get_player(player_id.clone()) {
                            let (log, player) = self.notify_player(player_id, start_from, known_action_requested).await;
                            let slog = if Some(true) == params.get("send_string_log").map(|p| p.parse::<i64>().ok().map(|i| i != 0)).flatten() {
                                Some(log.iter().map(|u| u.log.iter().map(|l| l.to_string()).collect()).collect())
                            } else {
                                None
                            };
                            let update = ServerUpdate {
                                player: Some(player),
                                log,
                                slog,
                                table: self.table.table_view_rx.borrow().clone()
                            };
                            //println!("responding: {:#?}", update);
                            *response.body_mut() = Body::from(serde_json::to_vec(&update).unwrap());
                            *response.status_mut() = StatusCode::OK;
                        }
                    } else {
                        let log = self.notify_logs(start_from).await;
                        let slog = if Some(true) == params.get("send_string_log").map(|p| p.parse::<i64>().ok().map(|i| i != 0)).flatten() {
                            Some(log.iter().map(|u| u.log.iter().map(|l| l.to_string()).collect()).collect())
                        } else {
                            None
                        };
                        let update = ServerUpdate {
                            player: None,
                            log,
                            slog,
                            table: self.table.table_view_rx.borrow().clone()
                        };
                        *response.body_mut() = Body::from(serde_json::to_vec(&update).unwrap());
                        *response.status_mut() = StatusCode::OK;
                    }
                }
            },
            (&Method::POST, "/start") => {
                self.table.start();
                *response.status_mut() = StatusCode::OK;
            },
            (&Method::POST, "/stop") => {
                self.table.stop();
                *response.status_mut() = StatusCode::OK;
            },
            (&Method::POST, "/add_bot") => {
                let (player_id, new_bot) = {
                    let mut bots = self.bots.lock().unwrap();
                    let bot = Arc::new(BotAlwaysCallInputSource::new());
                    bots.push(bot.clone());
                    (format!("bot{}", bots.len()), bot)
                };
                self.table.join(player_id, new_bot);
                *response.status_mut() = StatusCode::OK;
            },
            (&Method::POST, "/bet") => {
                if let Some(player_id) = player_from_params(&params) {
                    if let Some(conn) = self.get_player(player_id) {
                        if let Ok(resp) = serde_json::from_slice::<BetResp>(&hyper::body::to_bytes(req.into_body()).await.unwrap()) {
                            let input_source = conn;
                            let player = input_source.server_player();
                            let channel = &input_source.bet_tx;
                            if let Some(ServerActionRequest::Bet{call_amount, min_bet}) = player.action_requested.clone() {
                                match resp {
                                    BetResp::Bet(bet) => {
                                        if player.viewstate.as_ref().map(|s| s.valid_bet(min_bet, call_amount, bet, s.role).is_ok()).unwrap_or(false) {
                                            channel.send(Some(resp));
                                            *response.status_mut() = StatusCode::OK;
                                        }
                                    },
                                    BetResp::Fold => {
                                        channel.send(Some(resp));
                                        *response.status_mut() = StatusCode::OK;
                                    }
                                }
                            }
                        }
                    }
                }
            },
            (&Method::POST, "/dealers_choice") => {
                if let Some(player_id) = player_from_params(&params) {
                    if let Some(conn) = self.get_player(player_id) {
                        if let Ok(resp) = serde_json::from_slice::<usize>(&hyper::body::to_bytes(req.into_body()).await.unwrap()) {
                            conn.dealers_choice_tx.send(resp);
                        } else {
                            println!("Failed to parse request");
                        }
                    } else {
                        println!("Failed to find player");
                    }
                } else {
                    println!("No player id in request");
                }
            },
            (&Method::POST, "/replace") => {
                if let Some(player_id) = player_from_params(&params) {
                    if let Some(player) = self.get_player(player_id) {
                        if let Ok(resp) = serde_json::from_slice::<ReplaceResp>(&hyper::body::to_bytes(req.into_body()).await.unwrap()) {
                            if let Some(ServerActionRequest::Replace{max_can_replace}) = player.server_player().action_requested.clone() {
                                if resp.len() <= max_can_replace {
                                    player.replace_tx.send(Some(resp));
                                    *response.status_mut() = StatusCode::OK;
                                } else {
                                    println!("Attempting to replace too many cards {}", resp.len());
                                }
                            } else {
                                println!("No action requested");
                            }
                        } else {
                            println!("Failed to parse request");
                        }
                    } else {
                        println!("Failed to find player");
                    }
                } else {
                    println!("No player id in request");
                }
            },
            (&Method::GET, path) if path.len() > 1 => {
                if let Some(f) = self.static_files.load_file(&path[1..]) {
                    *response.status_mut() = StatusCode::OK;
                    *response.body_mut() = Body::from(f);
                    response.headers_mut().insert("Content-Type", HeaderValue::from_str(content_type(path)).unwrap());
                } else {
                    *response.status_mut() = StatusCode::NOT_FOUND;
                }
            },
            _ => {
                *response.status_mut() = StatusCode::NOT_FOUND;
            }
        };
        Ok(response)
    }
}
