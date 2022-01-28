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

pub fn win_ratio(state: &PokerViewState) -> f64 {
    let mut won: u64 = 0;
    let mut total: u64 = 0;

    let mut cards_left: CardSet = standard_deck().raw.iter().copied().collect();
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

    let mut my_visible: CardTuple = CardTuple::new();
    let mut my_hidden = 0;
    for cv in &state.players.get(&state.role).unwrap().hand {
        if let CardViewState::Visible(cs) = cv {
            my_visible.push(cs.card);
            cards_left.remove(cs.card);
        } else {
            my_hidden += 1;
        }
    }

    let mut players: Vec<(CardTuple, usize)> = Vec::new();
    let mut max_player_hidden = 0;
    for (role, player) in &state.players {
        if *role == state.role || player.folded {
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
        max_player_hidden = std::cmp::max(hidden, max_player_hidden);
    }

    for community_combo in combinations(cards_left.iter(), community_hidden) {
        let mut community = community_visible;
        let mut cards_left = cards_left;
        for card in community_combo {
            community.push(card);
            cards_left.remove(card);
        }

        let my_combos = combinations(cards_left.iter(), my_hidden);
        
        for my_combo in my_combos {
            let mut my_hand = my_visible;
            let mut cards_left = cards_left;
            for card in my_combo {
                my_hand.push(card);
                cards_left.remove(card);
            }

            let player_combos = combinations(cards_left.iter(), max_player_hidden);

            let my_best = best_hand_use_from_hand(state.variant.use_from_hand, my_hand, community, 5, &state.rules);
            for player_combo in player_combos {
                for (visible, hidden) in &players {
                    total += 1;
                    let mut player_hand = *visible;
                    for card in &player_combo[..*hidden] {
                        player_hand.push(*card);
                    }
                    let player_best = best_hand_use_from_hand(state.variant.use_from_hand, player_hand, community, 5, &state.rules);
                    if my_best > player_best {
                        won += 1;
                    }
                }
            }
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
    fn test_win_ratio() {
        let players = vec![(0, PlayerViewState {
                chips: 100,
                total_bet: 1,
                hand: make_cards(&vec![(2, 0), (3, 0), (0, NUM_RANKS-1), (1, NUM_RANKS-1)]),
                folded: false,
            }),
            (1, PlayerViewState {
                chips: 100,
                total_bet: 1,
                hand: std::iter::repeat(CardViewState::Invisible).take(4).collect(),
                folded: false,
            }),
        ].into_iter().collect();

        let vs = PokerViewState {
            role: 0,
            players,
            community_cards: make_cards(&vec![(0, 0), (1, 0), (0, 1), (0, 9), (2, 5)]),
            bet_this_round: HashMap::new(),
            rules: Vec::new(),
            variant: PokerVariantViewState {
                use_from_hand: 2
            },
        };

        let r = win_ratio(&vs);
        assert!(r >= 0.0);
        assert!(r <= 1.0);
    }

    #[test]
    fn test_win_ratio_empty_community() {
        let players = vec![(0, PlayerViewState {
                chips: 100,
                total_bet: 1,
                hand: make_cards(&vec![(2, 0), (3, 0), (0, NUM_RANKS-1), (1, NUM_RANKS-1)]),
                folded: false,
            }),
            (1, PlayerViewState {
                chips: 100,
                total_bet: 1,
                hand: std::iter::repeat(CardViewState::Invisible).take(4).collect(),
                folded: false,
            }),
        ].into_iter().collect();

        let vs = PokerViewState {
            role: 0,
            players,
            community_cards: Vec::new(),
            bet_this_round: HashMap::new(),
            rules: Vec::new(),
            variant: PokerVariantViewState {
                use_from_hand: 2
            },
        };

        let r = win_ratio(&vs);
        assert!(r >= 0.0);
        assert!(r <= 1.0);
    }

    #[test]
    fn test_win_ratio_start_seven_stud() {
        let mut other_cards = std::iter::repeat(CardViewState::Invisible).take(2).collect::<Vec<_>>();
        other_cards.extend_from_slice(&make_cards(&vec![(1, 5)]));
        let players = vec![(0, PlayerViewState {
                chips: 100,
                total_bet: 1,
                hand: make_cards(&vec![(2, 0), (3, 0), (0, NUM_RANKS-1),]),
                folded: false,
            }),
            (1, PlayerViewState {
                chips: 100,
                total_bet: 1,
                hand: other_cards,
                folded: false,
            }),
        ].into_iter().collect();

        let vs = PokerViewState {
            role: 0,
            players,
            community_cards: Vec::new(),
            bet_this_round: HashMap::new(),
            rules: Vec::new(),
            variant: PokerVariantViewState {
                use_from_hand: 5
            },
        };

        let r = win_ratio(&vs);
        assert!(r >= 0.0);
        assert!(r <= 1.0);
    }
}
