use crate::game::*;
use crate::viewstate::*;

use tokio::sync::watch;

use async_trait::async_trait;

pub struct BotAlwaysCallInputSource {
    viewstate_tx: watch::Sender<Option<PokerViewState>>,
    viewstate_rx: watch::Receiver<Option<PokerViewState>>,
}

impl BotAlwaysCallInputSource {
    pub fn new() -> BotAlwaysCallInputSource {
        let (viewstate_tx, viewstate_rx) = watch::channel(None);
        BotAlwaysCallInputSource {
            viewstate_tx, viewstate_rx
        }
    }
}

#[async_trait]
impl PlayerInputSource for BotAlwaysCallInputSource {
    async fn bet(&self, call_amount: Chips, min_bet: Chips) -> BetResp {
        let mstate = self.viewstate_rx.borrow();
        let state = mstate.as_ref().unwrap();
        BetResp::Bet(std::cmp::min(call_amount, state.bettable_chips(state.role)))
    }

    async fn replace(&self, max_can_replace: usize) -> ReplaceResp {
        // Replace nothing
        Vec::new()
    }

    async fn dealers_choice(&self, variants: Vec<PokerVariantDesc>) -> usize {
        0
    }

    fn update(&self, update: PokerViewUpdate) {
        self.viewstate_tx.send(Some(update.viewstate));
    }
}
