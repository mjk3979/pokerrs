use crate::card::*;
use crate::special_card::*;
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
#[derive(TS, Serialize, Deserialize)]
pub struct Blind {
    pub amount: Chips
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[derive(TS, Serialize, Deserialize)]
pub enum AnteRule {
    Ante(Chips),
    Blinds(Vec<Blind>)
}

pub type AnteRuleFn = dyn Send + (Fn(usize, Duration) -> AnteRule);

#[derive(Clone, Debug, Eq, PartialEq)]
#[derive(TS, Serialize, Deserialize)]
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
    start: usize,
    end: usize,
}

#[derive(Clone)]
pub enum PokerVariantState {
    Rotation {
        variants: PokerVariants,
        idx: usize,
    },
    DealersChoice {
        variants: PokerVariants,
    }
}

struct TableState {
    players: HashMap<PlayerId, LivePlayer>,
    seats: BTreeMap<Seat, PlayerId>,
    last_dealer: Option<Seat>,
    roles: Option<HashMap<PlayerId, PlayerRole>>,
    cur_log: Vec<TableViewDiff<PokerGlobalViewDiff<PlayerId>>>,
    old_logs: Vec<HandLog>,
    last_log_read: usize,
    variant_state: PokerVariantState,
    running_variant: Option<PokerVariantDesc>,
    start_time: std::time::Instant,
    past_time: Duration,
    ante_rule: Box<AnteRuleFn>,
}

#[derive(Clone, Serialize, Deserialize)]
#[derive(TS)]
#[serde(tag = "kind", content="data")]
pub enum PokerVariantSelector {
    Rotation(PokerVariants),
    DealersChoice(PokerVariants),
}

#[derive(Clone, Serialize, Deserialize)]
#[derive(TS)]
pub struct TableConfig {
    pub max_players: usize,
    pub starting_chips: Chips,
    pub variant_selector: PokerVariantSelector,
}

pub struct Table {
    running_tx: watch::Sender<bool>,
    running_rx: watch::Receiver<bool>,
    config: TableConfig,
    rules: TableRules,
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

    fn add_table_event(&mut self, event: TableEvent) {
        self.cur_log.push(TableViewDiff::TableDiff(event));
    }

    fn add_hand_logs(&mut self, full_hand_log: &[PokerGlobalViewDiff<PlayerId>]) {
        for log in &full_hand_log[self.last_log_read..] {
            self.cur_log.push(TableViewDiff::GameDiff(log.clone()));
        }
        self.last_log_read = full_hand_log.len();
    }

    fn new_round(&mut self) {
        let start = self.old_logs.last().map(|hl| hl.end).unwrap_or(0);
        self.old_logs.push(HandLog {
            round: self.old_logs.len()+1,
            start,
            end: self.cur_log.len(),
        });
    }

    fn logs(&self, player: Option<&PlayerId>, start_from: usize) -> Vec<PokerLogUpdate> {
        use TableViewDiff::*;
        let mut retval = Vec::new();
        for &HandLog{round, start, end} in &self.old_logs {
            if start >= start_from {
                retval.push(PokerLogUpdate {
                    round,
                    log: (&self.cur_log[start..end]).iter().map(|tv| match tv {
                        GameDiff(gvd) => GameDiff(gvd.player_diff(player)),
                        TableDiff(te) => TableDiff(te.clone()),
                    }).collect()
                });
            }
        }
        let round = self.old_logs.len();
        if self.cur_log.len() > start_from {
            let start = std::cmp::max(start_from, self.old_logs.last().map(|hl| hl.end).unwrap_or(0));
            retval.push(PokerLogUpdate {
                round,
                log: (&self.cur_log[start..]).iter().map(|tv| match tv {
                    GameDiff(gvd) => GameDiff(gvd.player_diff(player)),
                    TableDiff(te) => TableDiff(te.clone()),
                }).collect()
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

    fn server_uptime(&self) -> Duration {
        let now = std::time::Instant::now();
        (now - self.start_time) + self.past_time
    }
}

pub type SpecialRules = Vec<SpecialCard>;

impl Table {
    pub fn new(config: TableConfig, rules: TableRules, ante_rule: Box<AnteRuleFn>) -> Table {
        let start_time = std::time::Instant::now();
        let state = TableState {
            players: HashMap::new(),
            seats: BTreeMap::new(),
            last_dealer: None,
            roles: None,
            old_logs: Vec::new(),
            variant_state: config.variant_selector.clone().into(),
            running_variant: None,
            start_time,
            past_time: std::time::Duration::from_secs(0),
            ante_rule,
            cur_log: Vec::new(),
            last_log_read: 0,
        };
        let (running_tx, running_rx) = watch::channel(false);
        let (spectator_tx, spectator_rx) = fold_channel::channel(Vec::new(), |v, t: Vec<PokerGlobalViewDiff<PlayerId>>| v.extend_from_slice(&t));
        let (table_view_tx, table_view_rx) = watch::channel(Table::make_viewstate(false, &state, &config));
        Table {
            running_tx,
            running_rx,
            config,
            rules,
            state: Mutex::new(state),
            spectator_tx,
            spectator_rx,
            table_view_tx,
            table_view_rx,
        }
    }

    async fn get_next_variant(&self) -> (PokerVariant, PokerVariantDesc, SpecialRules) {
        let (variant_state, dealer) = {
            let state = self.state.lock().unwrap();
            let variant_state = state.variant_state.clone();
            (variant_state, state.players.get(&state.next_round_roles().get(&0).unwrap().1).unwrap().clone())
        };
        match variant_state {
            PokerVariantState::Rotation{variants, mut idx} => {
                if idx >= variants.descs.len() {
                    idx = 0;
                }
                let desc = variants.descs.get(idx).unwrap().clone();
                let retval = desc.variant();
                {
                    idx += 1;
                    let mut state = self.state.lock().unwrap();
                    state.variant_state = PokerVariantState::Rotation{variants, idx};
                }
                (retval, desc, Vec::new())
            },
            PokerVariantState::DealersChoice{variants} => {
                let DealersChoiceResp{variant_idx: idx, special_cards} = dealer.input.dealers_choice(variants.descs.clone()).await;
                let desc = variants.descs.get(idx).unwrap().clone();
                (desc.variant(), desc, special_cards)
            },
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
        println!("Next round starting...");
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
        let mut rules = self.rules.clone();
        println!("Getting variant");
        let (variant, variant_desc, special_cards) = self.get_next_variant().await;
        {
            println!("Got variant");
            let mut state = self.state.lock().unwrap();
            println!("Locked state");
            state.running_variant = Some(variant_desc.clone());
            state.add_table_event(TableEvent::VariantChange{new_variant_desc: variant_desc});
            self.table_view_tx.send(self.viewstate(&state));
            println!("Sent viewstate");

            rules.ante = (state.ante_rule)(round, state.server_uptime());
            println!("Got ante rule");
            let new_min_bet = match &rules.ante {
                AnteRule::Ante(ante) => *ante,
                AnteRule::Blinds(blinds) => blinds.iter().map(|b| b.amount).max().unwrap(),
            };
            if new_min_bet != rules.min_bet {
                rules.min_bet = new_min_bet;
                state.add_table_event(TableEvent::AnteChange{new_table_rules: rules.clone()});
            }
        }
        println!("Playing poker...");
        match play_poker(variant,
            Mutex::new(deck),
            players,
            Some(self.spectator_tx.clone()),
            rules,
            special_cards,
            round,
            ).await {
            Ok(winners) => {
                tokio::time::sleep(Duration::from_millis(2_000)).await;
                let mut state = self.state.lock().unwrap();
                {
                    let old_log = (&self.spectator_rx.borrow()[..]);
                    state.add_hand_logs(old_log);
                    state.new_round();
                }
                for (role, change) in winners.into_iter() {
                    state.players.get_mut(roles.get(&role).unwrap()).unwrap().chips += change;
                }
                state.running_variant = None;
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
        let mut state = self.state.lock().unwrap();
        let cur_log = self.spectator_rx.borrow();
        state.add_hand_logs(&cur_log);
        state.logs(player_id, start_from)
    }

    pub fn join(&self, player_id: PlayerId, player: Arc<PlayerInputSource>) -> Result<(), JoinError> {
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
        state.seats.insert(Seat(seat_len), player_id.clone());
        state.add_table_event(TableEvent::PlayerJoined{player_id});
        self.table_view_tx.send(self.viewstate(&state));
        Ok(())
    }

    pub fn start(&self) {
        self.running_tx.send(true);
        {
            let mut state = self.state.lock().unwrap();
            state.start_time = std::time::Instant::now();
        }
    }

    pub fn stop(&self) {
        let was_running = self.running_rx.borrow();
        self.running_tx.send(false);
        if *was_running {
            let mut state = self.state.lock().unwrap();
            let now = std::time::Instant::now();
            let start_time = state.start_time;
            state.past_time += now - start_time;
        }
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
            running_variant: state.running_variant.clone(),
        }
    }
}

impl From<PokerVariantSelector> for PokerVariantState {
    fn from(selector: PokerVariantSelector) -> PokerVariantState {
        match selector {
            PokerVariantSelector::Rotation(variants) => PokerVariantState::Rotation {
                variants,
                idx: 0
            },
            PokerVariantSelector::DealersChoice(variants) => PokerVariantState::DealersChoice {
                variants
            },
        }
    }
}
