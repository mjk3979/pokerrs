use crate::game::*;
use crate::viewstate::*;

use tokio::sync::oneshot;

use async_trait::async_trait;

pub struct BotAlwaysCallInputSource {
    state: Option<PokerViewUpdate>
}

impl BotAlwaysCallInputSource {
    pub fn new() -> BotAlwaysCallInputSource {
        BotAlwaysCallInputSource {
            state: None
        }
    }
}

impl PlayerInputSource for BotAlwaysCallInputSource {
    fn bet(&mut self, call_amount: Chips, min_bet: Chips, tx: oneshot::Sender<BetResp>) {
        let PokerViewUpdate{viewstate: state, ..} = self.state.as_ref().unwrap();
        tx.send(BetResp::Bet(std::cmp::min(call_amount, state.bettable_chips(state.role))));
    }

    fn replace(&mut self) -> ReplaceResp {
        // Replace nothing
        Vec::new()
    }

    fn update(&mut self, update: PokerViewUpdate) {
        self.state = Some(update);
    }
}
