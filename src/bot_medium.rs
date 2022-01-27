use crate::card::*;
use crate::game::*;
use crate::gamestate::*;
use crate::viewstate::*;
use crate::bot::*;

pub struct BotMedium{}

impl Bot for BotMedium {
    fn bet(&self, state: &PokerViewState, call_amount: Chips, min_bet: Chips) -> BetResp {
        use RiskFactor::*;

        let risk_factor = if pocket_threes(state) {
            CallAny
        } else {
            let r = win_ratio(state);
            PotRatio(r)
        };
        bet_risk_factor(state, call_amount, min_bet, risk_factor)
    }
}

impl BotMedium {
    pub fn new() -> BotMedium {
        BotMedium{}
    }
}
