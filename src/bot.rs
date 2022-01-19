use crate::bitcard::*;
use crate::game::*;
use crate::viewstate::*;

use tokio::sync::watch;

use async_trait::async_trait;

use std::sync::Arc;

pub trait Bot: Send + Sync {
    fn bet(&self, state: &PokerViewState, call_amount: Chips, min_bet: Chips) -> BetResp;
    fn replace(&self, state: &PokerViewState, max_can_replace: usize) -> ReplaceResp {
        Vec::new()
    }
}

pub struct BotInputSource {
    bot: Arc<dyn Bot>,
    viewstate_tx: watch::Sender<Option<PokerViewState>>,
    viewstate_rx: watch::Receiver<Option<PokerViewState>>,
}

impl BotInputSource {
    pub fn new(bot: Arc<dyn Bot>) -> BotInputSource {
        let (viewstate_tx, viewstate_rx) = watch::channel(None);
        BotInputSource {
            bot, viewstate_tx, viewstate_rx
        }
    }
}

#[async_trait]
impl PlayerInputSource for BotInputSource {
    async fn bet(&self, call_amount: Chips, min_bet: Chips) -> BetResp {
        let mstate = self.viewstate_rx.borrow();
        let state = mstate.as_ref().unwrap();
        self.bot.bet(state, call_amount, min_bet)
    }

    async fn replace(&self, max_can_replace: usize) -> ReplaceResp {
        // Replace nothing
        Vec::new()
    }

    async fn dealers_choice(&self, variants: Vec<PokerVariantDesc>) -> DealersChoiceResp {
        let variant_idx = 0;
        DealersChoiceResp {
            variant_idx,
            special_cards: Vec::new(),
        }
    }

    fn update(&self, update: PokerViewUpdate) {
        self.viewstate_tx.send(Some(update.viewstate));
    }
}

pub fn my_hand(state: &PokerViewState) -> Vec<CardViewState> {
    state.players.get(&state.role).unwrap().hand.iter().cloned().collect()
}

pub fn check_or_call_any(state: &PokerViewState, call_amount: Chips) -> BetResp {
    BetResp::Bet(std::cmp::min(call_amount, state.bettable_chips(state.role)))
}

pub fn pocket_threes(state: &PokerViewState) -> bool {
    let hand = my_hand(state);
    hand.iter().filter(|cv| {
        if let CardViewState::Visible(card) = cv {
            card.card.rank == 2
        } else {
            false
        }
    }).count() >= 2
}

pub fn hands(cards: &[CardViewState]) -> CardTuple {
    cards.iter().filter_map(|cv| {
        if let CardViewState::Visible(cs) = cv {
            Some(cs.card)
        } else {
            None
        }
    }).collect()
}

pub fn bet_risk_factor(state: &PokerViewState, call_amount: Chips, min_bet: Chips, risk_factor: f64) -> BetResp {
    let bet_this_round = state.bet_this_round.get(&state.role).unwrap_or(&0);
    let max_bet = (state.pot() as f64 * risk_factor) as Chips;
    if call_amount - bet_this_round > max_bet {
        BetResp::Fold
    } else {
        BetResp::Bet(std::cmp::min(max_bet, state.bettable_chips(state.role)))
    }
}
