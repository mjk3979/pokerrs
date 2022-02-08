use pokerrs::bot::*;
use pokerrs::card::*;
use pokerrs::game::*;
use pokerrs::gamestate::*;
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
    let mut players = vec![(0, PlayerViewState {
            chips: 100,
            total_bet: 1,
            hand: make_cards(&vec![(2, 0), (3, 0), (0, NUM_RANKS-1), (1, 1), (2, 2)]),
            folded: false,
        }),
    ];
    let num_opponents = 1;
    for idx in 0..num_opponents {
        players.push((idx+1, PlayerViewState {
            chips: 100,
            total_bet: 1,
            hand: std::iter::repeat(CardViewState::Invisible).take(5).collect(),
            folded: false,
        }));
    }
    let players = players.into_iter().collect();
    let vs = PokerViewState {
        role: 0,
        players,
        community_cards: Vec::new(),
        bet_this_round: HashMap::new(),
        rules: Vec::new(),
        variant: PokerVariantViewState {
            use_from_hand: 5
        },
        current_turn: Some(0),
    };
    let resp = best_replace(&vs, 4);
    println!("{:?}", resp);
}
