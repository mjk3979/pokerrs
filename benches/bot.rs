use criterion::{black_box, criterion_group, criterion_main, Criterion};

use std::collections::HashMap;
use pokerrs::bot::*;
use pokerrs::card::*;
use pokerrs::game::*;
use pokerrs::gamestate::*;
use pokerrs::special_card::*;
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

fn test_win_ratio_perf_twos_wild() {
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
    assert!(r >= 0.0);
    assert!(r <= 1.0);
}


fn test_win_ratio_perf() {
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
        (2, PlayerViewState {
            chips: 100,
            total_bet: 1,
            hand: std::iter::repeat(CardViewState::Invisible).take(4).collect(),
            folded: false,
        }),
        (3, PlayerViewState {
            chips: 100,
            total_bet: 1,
            hand: std::iter::repeat(CardViewState::Invisible).take(4).collect(),
            folded: false,
        }),
        (4, PlayerViewState {
            chips: 100,
            total_bet: 1,
            hand: std::iter::repeat(CardViewState::Invisible).take(4).collect(),
            folded: false,
        }),
        (5, PlayerViewState {
            chips: 100,
            total_bet: 1,
            hand: std::iter::repeat(CardViewState::Invisible).take(4).collect(),
            folded: false,
        }),
        (6, PlayerViewState {
            chips: 100,
            total_bet: 1,
            hand: std::iter::repeat(CardViewState::Invisible).take(4).collect(),
            folded: false,
        }),
        (7, PlayerViewState {
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

    let ratio = win_ratio(&vs);
}

fn test_replace(num_opponents: usize) {
    let mut players = vec![(0, PlayerViewState {
            chips: 100,
            total_bet: 1,
            hand: make_cards(&vec![(2, 0), (3, 0), (0, NUM_RANKS-1), (1, 1), (2, 2)]),
            folded: false,
        }),
    ];
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
    };
    let resp = best_replace(&vs, 4);
}

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("bot");
    group.sample_size(10);
    //group.bench_function("win_ratio 2x4", |b| b.iter(|| test_win_ratio_perf_small()));
    //group.bench_function("win_ratio 8x4", |b| b.iter(|| test_win_ratio_perf()));
    group.bench_function("win_ratio 6x4 twos_wild", |b| b.iter(|| test_win_ratio_perf_twos_wild()));
    //group.bench_function("replace 2x5", |b| b.iter(|| test_replace(1)));
    //group.bench_function("replace 4x5", |b| b.iter(|| test_replace(3)));
    //group.bench_function("replace 6x5", |b| b.iter(|| test_replace(5)));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
