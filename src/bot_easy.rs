use crate::card::*;
use crate::game::*;
use crate::gamestate::*;
use crate::viewstate::*;
use crate::bot::*;

pub struct BotEasy{}

impl Bot for BotEasy {
    fn bet(&self, state: &PokerViewState, call_amount: Chips, min_bet: Chips) -> BetResp {
        let hand = my_hand(state);
        use RiskFactor::*;

        // Pocket threes
        let risk_factor = if pocket_threes(state) {
            CallAny
        } else if hand.len() + state.community_cards.len() < 5 {
            PotRatio(0.5f64)
        } else {
            let best = best_hand(*hands(&hand).first().unwrap(), *hands(&state.community_cards).first().unwrap(), 5, &state.rules);
            if best.kind >= Kind::FourKind(0) {
                CallAny
            } else {
            PotRatio(
                if best.kind >= (Kind::FullHouse{high: 0, low: 0}) {
                    2f64
                } else if best.kind >= Kind::Flush(vec![0].into()) {
                    1f64
                } else if best.kind >= Kind::Straight(0) {
                    0.75f64
                } else if best.kind >= Kind::ThreeKind(0) {
                    0.5f64
                } else if best.kind >= (Kind::TwoPair{high: 0, low: 0}) {
                    0.25f64
                } else if best.kind >= Kind::Pair(0) {
                    0.125f64
                } else {
                    0f64
                }
            )}
        };
        bet_risk_factor(state, call_amount, min_bet, risk_factor)
    }
}

impl BotEasy {
    pub fn new() -> BotEasy {
        BotEasy{}
    }
}
