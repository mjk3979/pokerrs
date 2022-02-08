use pokerrs::bot::*;
use pokerrs::card::*;
use pokerrs::game::*;
use pokerrs::gamestate::*;
use pokerrs::special_card::*;
use pokerrs::viewstate::*;

use std::collections::HashMap;

fn make_cards(tups: &[(usize, Rank)]) -> Vec<CardViewState> {
    tups.iter().map(|(suit, rank)| CardViewState::Visible(CardState {
        card: Card {
            rank: *rank,
            suit: Suit(*suit),
        },
        facing: Facing::FaceUp,
    })).collect()
}

fn main() {
    let players = vec![(0, PlayerViewState {
            chips: 100,
            total_bet: 1,
            hand: make_cards(&vec![(2, 0), (3, 0), (0, NUM_RANKS-1), (1, NUM_RANKS-2), (0, 8)]),
            folded: false,
        }),
        (1, PlayerViewState {
            chips: 100,
            total_bet: 1,
            hand: std::iter::repeat(CardViewState::Invisible).take(5).collect(),
            folded: false,
        }),
        (2, PlayerViewState {
            chips: 100,
            total_bet: 1,
            hand: std::iter::repeat(CardViewState::Invisible).take(5).collect(),
            folded: false,
        }),
        (3, PlayerViewState {
            chips: 100,
            total_bet: 1,
            hand: std::iter::repeat(CardViewState::Invisible).take(5).collect(),
            folded: false,
        }),
        (4, PlayerViewState {
            chips: 100,
            total_bet: 1,
            hand: std::iter::repeat(CardViewState::Invisible).take(5).collect(),
            folded: false,
        }),
        (5, PlayerViewState {
            chips: 100,
            total_bet: 1,
            hand: std::iter::repeat(CardViewState::Invisible).take(5).collect(),
            folded: false,
        }),
    ].into_iter().collect();

    let vs = PokerViewState {
        role: 0,
        players,
        community_cards: Vec::new(),
        bet_this_round: HashMap::new(),
        rules: SpecialCardGroup::all().into_iter().next().unwrap().cards,
        variant: PokerVariantViewState {
            use_from_hand: 5
        },
    };

    let r = win_ratio(&vs);
}
