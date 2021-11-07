use crate::card::*;
use crate::game::*;
use crate::gamestate::*;
use crate::viewstate::*;
use crate::fold_channel;

use ts_rs::{TS, export};

use tokio::sync::watch;
use serde::{Serialize, Deserialize};

use std::collections::{HashMap, BTreeMap};

use std::sync::{Arc, Mutex};
use std::time::Duration;


#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Blind {
    pub player: PlayerRole,
    pub amount: Chips
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AnteRule {
    Ante(Chips),
    Blinds(Vec<Blind>)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TableRules {
    pub ante: AnteRule,
    pub ante_name: String,
    pub min_bet: Chips
}

#[derive(Eq, Copy, Clone, PartialEq, Hash, Debug, Ord, PartialOrd, Serialize, Deserialize)]
#[derive(TS)]
pub struct Seat(pub usize);

struct HandLog {
    round: usize,
    log: Vec<PokerGlobalViewDiff<PlayerId>>,
}

struct TableState {
    players: HashMap<PlayerId, LivePlayer>,
    seats: BTreeMap<Seat, PlayerId>,
    last_dealer: Option<Seat>,
    roles: Option<HashMap<PlayerId, PlayerRole>>,
    old_logs: Vec<HandLog>,
    current_log_start: usize,
}

#[derive(Eq, Copy, Clone, PartialEq, Hash, Debug, Ord, PartialOrd, Serialize, Deserialize)]
#[derive(TS)]
pub struct TableConfig {
    pub max_players: usize,
    pub starting_chips: Chips
}

pub struct Table {
    running_tx: watch::Sender<bool>,
    running_rx: watch::Receiver<bool>,
    config: TableConfig,
    rules: TableRules,
    variant: PokerVariant,
    state: Mutex<TableState>,
    spectator_tx: fold_channel::Sender<Vec<PokerGlobalViewDiff<PlayerId>>, Vec<PokerGlobalViewDiff<PlayerId>>>,
    pub spectator_rx: fold_channel::Receiver<Vec<PokerGlobalViewDiff<PlayerId>>>,
    table_view_tx: watch::Sender<TableViewState>,
    pub table_view_rx: watch::Receiver<TableViewState>,
}

pub enum JoinError {
    Full,
    AlreadyJoined
}

impl TableState {
    fn next_seat(&self, seat: Seat) -> Option<(Seat, PlayerId)> {
        let Seat(seat) = seat;
        // Get the first one after
        self.seats.range(Seat(seat+1)..).next()
        // or wrap around
        .or(self.seats.range(..Seat(seat+1)).next()).map(|(&s, p)| (s, p.clone()))
    }

    fn logs(&self, cur_log: &[PokerGlobalViewDiff<PlayerId>], player: Option<&PlayerId>, start_from: usize) -> Vec<PokerLogUpdate> {
        let mut retval = Vec::new();
        if start_from < self.current_log_start {
            let mut start_from = start_from;
            for HandLog{round, log} in &self.old_logs {
                if start_from < log.len() {
                    retval.push(PokerLogUpdate {
                        round: *round,
                        log: (&log[start_from..]).iter().map(|l| l.player_diff(player)).collect(),
                    });
                    start_from = 0;
                } else {
                    start_from -= log.len();
                }
            }
        }

        if start_from < cur_log.len() {
            let round = self.old_logs.len();
            let start_from = std::cmp::max(self.current_log_start, start_from);
            retval.push(PokerLogUpdate {
                round,
                log: (&cur_log[start_from..]).iter().map(|l| l.player_diff(player)).collect(),
            });
        }
        retval
    }

    fn next_round_roles(&self) -> HashMap<PlayerRole, (Seat, PlayerId)> {
        let start =
            if let Some(last_dealer_seat) = self.last_dealer {
                self.next_seat(last_dealer_seat)
            } else {
                // Just start with the first seat with a player
                self.seats.iter().next().map(|(&s, p)| (s, p.clone()))
            };
        let dealer_seat = start.as_ref().map(|t| t.0);
        let mut retval = HashMap::new();
        let mut cur = start;
        let mut role = 0;
        while let Some(sp) = cur {
            cur = self.next_seat(sp.0);
            if self.players.get(&sp.1).unwrap().chips > 0 {
                retval.insert(role, sp);
                role += 1;
            }
            if cur.as_ref().map(|t| t.0) == dealer_seat {
                break;
            }
        };
        retval
    }
}

impl Table {
    pub fn new(config: TableConfig, rules: TableRules) -> Table {
        let variant = texas_hold_em();
        let state = TableState {
            players: HashMap::new(),
            seats: BTreeMap::new(),
            last_dealer: None,
            roles: None,
            current_log_start: 0,
            old_logs: Vec::new(),
        };
        let (running_tx, running_rx) = watch::channel(false);
        let (spectator_tx, spectator_rx) = fold_channel::channel(Vec::new(), |v, t: Vec<PokerGlobalViewDiff<PlayerId>>| v.extend_from_slice(&t));
        let (table_view_tx, table_view_rx) = watch::channel(Table::make_viewstate(false, &state, &config));
        Table {
            running_tx,
            running_rx,
            config,
            rules,
            variant,
            state: Mutex::new(state),
            spectator_tx,
            spectator_rx,
            table_view_tx,
            table_view_rx
        }
    }
    pub async fn next_round(&self) -> bool {
        {
            loop {
                let mut running_rx = self.running_rx.clone();
                if *running_rx.borrow_and_update() {
                    break;
                }
                running_rx.changed().await;
            }
        }
        let (roles, players, round) = {
            let mut state = self.state.lock().unwrap();
            let roles = state.next_round_roles();
            state.last_dealer = roles.get(&0).map(|(s, p)| *s);
            let just_roles: HashMap<PlayerRole, PlayerId> = roles.into_iter().map(|(r, (s, p))| (r, p)).collect();
            state.roles = Some(just_roles.iter().map(|(&r, p)| (p.clone(), r)).collect());
            let players: HashMap<PlayerRole, LivePlayer> = just_roles.iter().map(|(&r, p)| (r, state.players.get(p).cloned().unwrap())).collect();
            self.table_view_tx.send(self.viewstate(&state));
            let round = state.old_logs.len();
            (just_roles, players, round)
        };
        let mut deck = Box::new(standard_deck());
        {
            let mut rng = rand::thread_rng();
            deck.shuffle(&mut rng);
        }
        match play_poker(self.variant.clone(),
            Mutex::new(deck),
            players,
            Some(self.spectator_tx.clone()),
            self.rules.clone(),
            round,
            ).await {
            Ok(winners) => {
                tokio::time::sleep(Duration::from_millis(2_000)).await;
                let mut state = self.state.lock().unwrap();
                {
                    let old_log = (&self.spectator_rx.borrow()[state.current_log_start..]).iter().cloned().collect();
                    state.old_logs.push(HandLog {
                        round,
                        log: old_log,
                    });
                }
                state.current_log_start += state.old_logs.last().unwrap().log.len();
                for (role, change) in winners.into_iter() {
                    state.players.get_mut(roles.get(&role).unwrap()).unwrap().chips += change;
                }
                self.table_view_tx.send(self.viewstate(&state));
            }
            Err(message) => {
                println!("Error in play_poker, rolling back game {}", message);
            }
        }
        {
            let state = self.state.lock().unwrap();
            state.players.values().filter(|p| p.chips > 0).count() > 1
        }
    }

    pub fn logs(&self, player_id: Option<&PlayerId>, start_from: usize) -> Vec<PokerLogUpdate> {
        let state = self.state.lock().unwrap();
        let cur_log = self.spectator_rx.borrow();
        state.logs(&cur_log, player_id, start_from)
    }

    pub fn join(&self, player_id: PlayerId, player: Arc<Mutex<PlayerInputSource>>) -> Result<(), JoinError> {
        let mut state = self.state.lock().unwrap();
        if state.players.len() >= self.config.max_players {
            return Err(JoinError::Full);
        }
        if state.players.contains_key(&player_id) {
            return Err(JoinError::AlreadyJoined);
        }
        state.players.insert(player_id.clone(), LivePlayer {
            player_id: player_id.clone(),
            chips: self.config.starting_chips,
            input: player
        });
        let seat_len = state.seats.len();
        state.seats.insert(Seat(seat_len), player_id);
        self.table_view_tx.send(self.viewstate(&state));
        Ok(())
    }

    pub fn start(&self) {
        self.running_tx.send(true);
    }

    pub fn stop(&self) {
        self.running_tx.send(false);
    }

    fn viewstate(&self, state: &TableState) -> TableViewState {
        Table::make_viewstate(*self.running_rx.borrow(), state, &self.config)
    }

    fn make_viewstate(running: bool, state: &TableState, config: &TableConfig) -> TableViewState {
        TableViewState {
            running,
            roles: state.roles.as_ref().map(|h| h.iter().map(|(p, &r)| (r, p.clone())).collect()),
            seats: state.seats.iter().map(|(s, p)| (p.clone(), *s)).collect(),
            config: config.clone(),
        }
    }
}
