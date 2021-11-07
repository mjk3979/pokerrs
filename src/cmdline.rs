use crate::game::*;
use crate::viewstate::*;
use crate::gamestate::*;

use async_trait::async_trait;
use tokio::sync::oneshot;

use std::io::{self, StdinLock, BufRead, Stdin};

pub struct CmdlineInputSource {
    state: Option<PokerViewUpdate>,
    table: Option<TableViewState>,
}

impl CmdlineInputSource {
    pub fn new() -> CmdlineInputSource {
        CmdlineInputSource {
            state: None,
            table: None,
        }
    }

    fn prompt(&mut self, prompt: &str) -> String {
        print!("{}: ", prompt);
        self.read_str()
    }

    fn menu<T>(&mut self, choices: &[(T, &str)]) -> T
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

    fn read_str(&mut self) -> String {
        let mut buf: String = "".to_string();
        match io::stdin().lock().read_line(&mut buf) {
            Ok(_) => {},
            Err(_) => panic!("At the disco")
        };
        buf = buf.trim().to_string();
        return buf;
    }

    fn bet_amount(&mut self, min_bet: Chips, call_amount: Chips) -> Option<Chips> {
        let prompt = if call_amount == 0 {
            "Bet amount?".to_string()
        } else {
            format!("Raise amount (>={} or all in)?", min_bet)
        };
        if let Ok(v) = self.prompt(&prompt).parse::<Chips>() {
            if let Some(PokerViewUpdate{viewstate: state, ..}) = self.state.as_ref() {
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
        if let Some(PokerViewUpdate{viewstate: state, ..}) = &self.state {
            println!("{}", divider);
            println!("Pot: {}", state.pot());
            println!("Chips: {}", state.bettable_chips(state.role));
            println!("Hand: {}", state.hand_str(state.role));
            println!("{}", divider);
        }
    }

}

#[derive(Copy, Clone)]
enum MenuChoice {
    CheckCall,
    Bet,
    Fold
}

impl PokerViewClient for CmdlineInputSource {
    fn update_table(&mut self, table: TableViewState) {
        self.table = Some(table);
    }
}

#[async_trait]
impl PlayerInputSource for CmdlineInputSource {
    fn bet(&mut self, call_amount: Chips, min_bet: Chips, tx: oneshot::Sender<BetResp>) {
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
                    tx.send(BetResp::Bet(call_amount));
                    return;
                },
                Fold => {
                    tx.send(BetResp::Fold);
                    return;
                },
                Bet => {
                    if let Some(value) = self.bet_amount(min_bet, call_amount) {
                        tx.send(BetResp::Bet(value));
                        return;
                    }
                }
            }
        }
    }

    fn replace(&mut self) -> ReplaceResp {
        panic!("Unimplemented!");
    }

    fn update(&mut self, update: PokerViewUpdate) {
        let divider = "----------------";
        for PokerLogUpdate{log, ..} in &update.diff {
            for d in log {
                println!("{}\n{}\n{}", divider, d, divider);
            }
        }
        self.state = Some(update);
    }
}
