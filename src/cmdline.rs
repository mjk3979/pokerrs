use crate::game::*;
use crate::viewstate::*;
use crate::gamestate::*;

use async_trait::async_trait;
use tokio::sync::{oneshot, watch};

use std::io::{self, StdinLock, BufRead, Stdin};

pub struct CmdlineInputSource {
    viewstate_tx: watch::Sender<Option<PokerViewState>>,
    viewstate_rx: watch::Receiver<Option<PokerViewState>>,
}

impl CmdlineInputSource {
    pub fn new() -> CmdlineInputSource {
        let (viewstate_tx, viewstate_rx) = watch::channel(None);
        CmdlineInputSource { viewstate_tx, viewstate_rx }
    }

    fn prompt(&self, prompt: &str) -> String {
        print!("{}: ", prompt);
        self.read_str()
    }

    fn menu<T>(&self, choices: &[(T, &str)]) -> T
        where T: Copy
    {
        let num_choices = choices.len();
        loop {
            for (idx, (_, choice)) in choices.iter().enumerate() {
                println!("{}: {}", idx, choice);
            }
            if let Ok(c) = self.prompt("?").parse::<usize>() {
                if c <= num_choices {
                    return choices[c].0;
                }
            }
        }
    }

    fn read_str(&self) -> String {
        let mut buf: String = "".to_string();
        match io::stdin().lock().read_line(&mut buf) {
            Ok(_) => {},
            Err(_) => panic!("At the disco")
        };
        buf = buf.trim().to_string();
        return buf;
    }

    fn bet_amount(&self, min_bet: Chips, call_amount: Chips) -> Option<Chips> {
        let prompt = if call_amount == 0 {
            "Bet amount?".to_string()
        } else {
            format!("Raise amount (>={} or all in)?", min_bet)
        };
        if let Ok(v) = self.prompt(&prompt).parse::<Chips>() {
            if let Some(state) = self.viewstate() {
                if let Err(reason) = state.valid_bet(min_bet, call_amount, v, state.role) {
                    println!("{}", reason);
                } else {
                    return Some(v);
                }
            }
        }
        None
    }

    fn draw(&self) {
        let divider = "====================";
        if let Some(state) = self.viewstate() {
            println!("{}", divider);
            println!("Pot: {}", state.pot());
            println!("Chips: {}", state.bettable_chips(state.role));
            println!("Hand: {}", state.hand_str(state.role));
            println!("{}", divider);
        }
    }

    fn viewstate(&self) -> Option<PokerViewState> {
        self.viewstate_rx.borrow().clone()
    }
}

#[derive(Copy, Clone)]
enum MenuChoice {
    CheckCall,
    Bet,
    Fold
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum ReplaceChoice {
    Toggle(usize),
    Submit,
}

#[async_trait]
impl PlayerInputSource for CmdlineInputSource {
    async fn bet(&self, call_amount: Chips, min_bet: Chips) -> BetResp {
        use MenuChoice::*;
        let choices = vec![
            (CheckCall, if call_amount == 0 {
                "Check"
            } else {
                "Call"
            }),
            (Bet, if call_amount == 0 {
                "Bet"
            } else {
                "Raise"
            }),
            (Fold, "Fold")
        ];
        loop {
            self.draw();
            match self.menu(&choices) {
                CheckCall => {
                    return BetResp::Bet(call_amount);
                },
                Fold => {
                    return BetResp::Fold;
                },
                Bet => {
                    if let Some(value) = self.bet_amount(min_bet, call_amount) {
                        return BetResp::Bet(value);
                    }
                }
            }
        }
    }

    async fn replace(&self, max_can_replace: usize) -> ReplaceResp {
        use ReplaceChoice::*;
        let viewstate = self.viewstate().unwrap();
        let hand = &viewstate.players.get(&viewstate.role).unwrap().hand;
        let mut selected: Vec<(CardViewState, bool)> = hand.iter().map(|c| (c.clone(), false)).collect();
        loop {
            let mut choices: Vec<(ReplaceChoice, String)> = selected.iter().enumerate().map(|(idx, (c, s))| (Toggle(idx), format!("{}{}", c, if *s {"*"} else {""}))).collect();
            let num_selected = selected.iter().filter(|(_, s)| *s).count();
            choices.push((Submit, format!("Replace {} cards", num_selected)));
            match self.menu(&choices.iter().map(|(rc, s)| (rc, s as &str)).collect::<Vec<_>>()) {
                Toggle(idx) => {
                    let idx: usize = *idx;
                    if selected[idx].1 || num_selected < max_can_replace {
                        selected[idx].1 = !selected[idx].1;
                    }
                },
                Submit => {
                    return selected.into_iter().enumerate().filter_map(|(idx, (_, s))| if s {Some(idx)} else {None}).collect();
                }
            }
        }
    }

    async fn dealers_choice(&self, variants: Vec<PokerVariantDesc>) -> DealersChoiceResp {
        let variant_idx = self.menu(&variants.iter().map(|v| &v.name as &str).enumerate().collect::<Vec<_>>()[..]);
        DealersChoiceResp {
            variant_idx,
            special_cards: Vec::new(),
        }
    }

    fn update(&self, update: PokerViewUpdate) {
        self.viewstate_tx.send(Some(update.viewstate));
        let divider = "----------------";
        for PokerLogUpdate{log, ..} in &update.diff {
            for d in log {
                println!("{}\n{}\n{}", divider, d, divider);
            }
        }
    }
}

impl PokerViewClient for CmdlineInputSource {
    fn update_table(&self, table: TableViewState) {

    }
}
