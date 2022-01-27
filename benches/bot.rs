use criterion::{black_box, criterion_group, criterion_main, Criterion};

use std::collections::HashMap;
use pokerrs::bot::*;
use pokerrs::card::*;
use pokerrs::game::*;
use pokerrs::gamestate::*;
use pokerrs::viewstate::*;

fn make_cards(tups: &[(usize, Rank)]) -> Vec<CardViewState> {
    tups.iter().map(|(suit, rank)| CardViewState::Visible(CardState {
        card: Card {
            rank: *rank,
            suit: Suit(*suit),
        },
        facing: Facing::FaceUp,
    })).collect()
}


fn test_all_hands_perf() {
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
        (2, PlayerViewState {
            chips: 100,
            total_bet: 1,
            hand: std::iter::repeat(CardViewState::Invisible).take(4).collect(),
        }),
        (3, PlayerViewState {
            chips: 100,
            total_bet: 1,
            hand: std::iter::repeat(CardViewState::Invisible).take(4).collect(),
        }),
        (4, PlayerViewState {
            chips: 100,
            total_bet: 1,
            hand: std::iter::repeat(CardViewState::Invisible).take(4).collect(),
        }),
        (5, PlayerViewState {
            chips: 100,
            total_bet: 1,
            hand: std::iter::repeat(CardViewState::Invisible).take(4).collect(),
        }),
        (6, PlayerViewState {
            chips: 100,
            total_bet: 1,
            hand: std::iter::repeat(CardViewState::Invisible).take(4).collect(),
        }),
        (7, PlayerViewState {
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
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("all_hands 8x4", |b| b.iter(|| test_all_hands_perf()));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
