use crate::bitcard::*;
use crate::card::*;
use crate::comb::*;
use crate::game::*;
use crate::gamestate::*;
use crate::viewstate::*;

use tokio::sync::watch;

use async_trait::async_trait;

use std::collections::HashMap;
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

#[derive(Clone, Copy, Debug, PartialOrd, PartialEq)]
pub enum RiskFactor {
    CheckFold,
    PotRatio(f64),
    CallAny,
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

pub fn hands(cards: &[CardViewState]) -> Vec<CardTuple> {
    if cards.len() == 0 {
        return vec![CardTuple::new()];
    }
    let visible: CardTuple = cards.iter().filter_map(|cv| {
        if let CardViewState::Visible(cs) = cv {
            Some(cs.card)
        } else {
            None
        }
    }).collect();
    let mut retval = Vec::new();
    let num_hidden = cards.len() - visible.len();
    if num_hidden == 0 {
        return vec![visible];
    }
    for hidden in combinations_with_replacement(&standard_deck().raw, num_hidden) {
        let mut combo = visible;
        for card in hidden {
            combo.push(*card);
        }
        retval.push(combo);
    }
    retval
}

pub fn bet_risk_factor(state: &PokerViewState, call_amount: Chips, min_bet: Chips, risk_factor: RiskFactor) -> BetResp {
    let bet_this_round = state.bet_this_round.get(&state.role).unwrap_or(&0);

    use RiskFactor::*;
    match risk_factor {
        CheckFold => {
            if call_amount - bet_this_round > 0 {
                BetResp::Fold
            } else {
                BetResp::Bet(std::cmp::min(call_amount, state.bettable_chips(state.role)))
            }
        },
        PotRatio(ratio) => {
            let max_bet = (state.pot() as f64 * ratio) as Chips;
            if call_amount - bet_this_round > max_bet {
                BetResp::Fold
            } else if max_bet < min_bet {
                BetResp::Bet(std::cmp::min(call_amount, state.bettable_chips(state.role)))
            } else {
                BetResp::Bet(std::cmp::min(max_bet + call_amount, state.bettable_chips(state.role)))
            }
        },
        CallAny => {
            let max_bet = (state.pot() as f64 * 0.25) as Chips;
            if max_bet < min_bet {
                BetResp::Bet(std::cmp::min(call_amount, state.bettable_chips(state.role)))
            } else {
                BetResp::Bet(std::cmp::min(max_bet + call_amount, state.bettable_chips(state.role)))
            }
        }
    }
}

pub fn all_hands(state: &PokerViewState) -> Vec<(CardTuple, Vec<CardTuple>)> {
    let mut cards_left: CardSet = standard_deck().raw.into_iter().collect();
    let mut community_hidden = 0;
    let mut community_visible: CardTuple = CardTuple::new();
    for cv in &state.community_cards {
        if let CardViewState::Visible(cs) = cv {
            community_visible.push(cs.card);
            cards_left.remove(cs.card);
        } else {
            community_hidden += 1;
        }
    }

    let mut players: Vec<(CardTuple, usize)> = Vec::new();
    let mut visible: CardTuple = CardTuple::new();
    let mut hidden = 0;
    let mut total_hidden = 0;
    for cv in &state.players.get(&state.role).unwrap().hand {
        if let CardViewState::Visible(cs) = cv {
            visible.push(cs.card);
            cards_left.remove(cs.card);
        } else {
            hidden += 1;
        }
    }
    players.push((visible, hidden));
    total_hidden += hidden;
    for (role, player) in &state.players {
        if *role == state.role {
            continue;
        }
        let mut visible: CardTuple = CardTuple::new();
        let mut hidden = 0;
        for cv in &player.hand {
            if let CardViewState::Visible(cs) = cv {
                visible.push(cs.card);
                cards_left.remove(cs.card);
            } else {
                hidden += 1;
            }
        }
        players.push((visible, hidden));
        total_hidden += hidden;
    }

    let mut num_each = vec![community_hidden];
    for (_, hidden) in &players {
        num_each.push(*hidden);
    }
    let mut retval = Vec::new();
    for hidden_set in repeated_combinations(cards_left, &num_each) {
        let community = community_visible.clone() + hidden_set[0];
        let hands = players.iter().enumerate().map(|(idx, (visible, _))| {
            *visible + hidden_set[idx+1]
        }).collect();
        retval.push((community, hands));
    }
    retval
}

fn repeated_combinations<'a, I, It: Iterator<Item=&'a usize> + Clone>(cards: CardSet, num_each: I) -> Vec<Vec<CardTuple>>
where I: IntoIterator<Item = &'a usize, IntoIter=It> {
    let mut num_each = num_each.into_iter();
    let mut retval = Vec::new();
    let mut stack = vec![(cards, num_each, Vec::new())];
    while let Some((cards_left, mut num_each, mut combos)) = stack.pop() {
        if let Some(num) = num_each.next() {
            if *num == 0 {
                combos.push(CardTuple::new());
                stack.push((cards_left, num_each, combos));
                continue;
            }
            for combo in combinations(cards_left.iter(), *num) {
                let mut new_cards_left = cards_left;
                for card in combo.iter() {
                    new_cards_left.remove(*card);
                }
                let mut new_combos = combos.clone();
                new_combos.push(combo.into_iter().collect());
                stack.push((new_cards_left, num_each.clone(), new_combos));
            }
        } else {
            retval.push(combos);
        }
    }
    retval
}

pub fn win_ratio(state: &PokerViewState, call_amount: Chips, min_bet: Chips) -> f64 {
    let mut won: u64 = 0;
    let mut total: u64 = 0;
    for (community, hands) in all_hands(state) {
        total += 1;
        let my_best = best_hand(hands[0], community, 5, &state.rules);
        let mut beaten = false;
        for &hand in &hands[1..] {
            if my_best < best_hand(hand, community, 5, &state.rules) {
                beaten = true;
                break;
            }
        }
        if !beaten {
            won += 1;
        }
    }
    (won as f64) / (total as f64)
}

mod test {
    use crate::bot::*;
    use crate::viewstate::*;

    fn make_cards(tups: &[(usize, Rank)]) -> Vec<CardViewState> {
        tups.iter().map(|(suit, rank)| CardViewState::Visible(CardState {
            card: Card {
                rank: *rank,
                suit: Suit(*suit),
            },
            facing: Facing::FaceUp,
        })).collect()
    }

    #[test]
    fn test_all_hands() {
        let players = vec![(0, PlayerViewState {
                chips: 100,
                total_bet: 1,
                hand: make_cards(&vec![(2, 0), (3, 0), (0, NUM_RANKS-1), (1, NUM_RANKS-1)]),
            }),
            (1, PlayerViewState {
                chips: 100,
                total_bet: 1,
                hand: std::iter::repeat(CardViewState::Invisible).take(4).collect(),
            }),
        ].into_iter().collect();

        let vs = PokerViewState {
            role: 0,
            players,
            community_cards: make_cards(&vec![(0, 0), (1, 0), (0, 1), (0, 9), (2, 5)]),
            bet_this_round: HashMap::new(),
            rules: Vec::new(),
        };

        let hands = all_hands(&vs);
        println!("{}", hands.len());
        assert!(!hands.is_empty());
    }
}
