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

fn test_win_ratio_perf_small() {
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
        variant: PokerVariantViewState {
            use_from_hand: 2
        },
    };

    let r = win_ratio(&vs);
    assert!(r >= 0.0);
    assert!(r <= 1.0);
}


fn test_win_ratio_perf() {
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
        variant: PokerVariantViewState {
            use_from_hand: 2
        },
    };

    let ratio = win_ratio(&vs);
}

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("win_ratio");
    group.sample_size(10);
    group.bench_function("win_ratio 2x4", |b| b.iter(|| test_win_ratio_perf_small()));
    group.bench_function("win_ratio 8x4", |b| b.iter(|| test_win_ratio_perf()));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
