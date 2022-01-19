use crate::game::*;
use crate::viewstate::*;
use crate::bot::*;

pub struct BotAlwaysCall{}

impl Bot for BotAlwaysCall {
    fn bet(&self, state: &PokerViewState, call_amount: Chips, min_bet: Chips) -> BetResp {
        check_or_call_any(state, call_amount)
    }
}

impl BotAlwaysCall {
    pub fn new() -> BotAlwaysCall {
        BotAlwaysCall{}
    }
}
