use pokerrs::card::*;
use pokerrs::gamestate::*;
use pokerrs::comb::*;
use pokerrs::frozen::*;
use pokerrs::bitcard::*;

use std::collections::{HashMap, HashSet};
use std::cmp::Ordering;

#[derive(Debug)]
struct ThreeCardResult {
    play_wins: f64,
    fold_wins: f64,
    hands: usize,
    pair_plus_total: f64,
}

fn three_card_strength_sort_key(strength: HandStrength) -> (usize, RankTuple, RankTuple) {
    use Kind::*;
    let kind = strength.kind;
    match kind {
        HighCard(r) => (0, vec![r].into(), strength.kickers),
        Pair(r) => (1, vec![r].into(), strength.kickers),
        Flush(ranks) => (2, ranks, strength.kickers),
        Straight(r) => (3, vec![r].into(), strength.kickers),
        ThreeKind(r) => (4, vec![r].into(), strength.kickers),
        StraightFlush(r) => (5, vec![r].into(), strength.kickers),
        _ => panic!("Not possible!"),
    }
}

async fn calc_single_hand(player_hand: Vec<Card>) -> HashMap<CardSet, ThreeCardResult> {
    let mut retval = HashMap::new();
    let deck = standard_deck().raw;
    let mut remaining_deck: CardSet = deck.iter().cloned().collect();
    for &card in &player_hand {
        remaining_deck.remove(card);
    }
    let player_strength = best_hand(player_hand.iter().cloned().collect(), 3);
    let mut memo = HashMap::new();
    let expected_suits = vec![1, 2, 3];
    let combos = combinations(remaining_deck.iter(), 3).filter(|combo| {
        combo.iter().map(|c| {let Suit(suit) = c.suit; suit}).collect::<Vec<_>>() == expected_suits
        || combo.iter().all(|c| c.suit == Suit(3))
    });

    let num_combos = combos.clone().count();
    let mut last_p = None;
    for (idx, other_player_hand) in combos.into_iter().enumerate() {
        let p = (idx * 100) / num_combos;
        if Some(p) != last_p {
            last_p = Some(p);
            if p % 5 == 0 {
                //println!("\t{} / {}", idx, num_combos);
            }
        }
        let mut result = ThreeCardResult {
            play_wins: 0f64,
            fold_wins: 0f64,
            hands: 0,
            pair_plus_total: 0f64,
        };
        let mut remaining_deck = remaining_deck.clone();
        for &card in other_player_hand.iter() {
            remaining_deck.remove(card);
        }
        for dealer_hand in combinations(remaining_deck.iter(), 3) {
            let dealer_hand: CardTuple = dealer_hand.into_iter().collect();
            result.hands += 1;
            result.play_wins -= 2f64;
            result.fold_wins -= 1f64;
            result.pair_plus_total -= 1f64;
            let dhand: CardTuple = dealer_hand;
            let dealer_strength = memo.entry(dhand.iter().collect::<CardSet>()).or_insert_with(|| best_hand(dhand, 3)).clone();

            // Dealer plays
            if dealer_strength.kind >= Kind::HighCard(11) {
                match three_card_strength_sort_key(player_strength.clone()).cmp(&three_card_strength_sort_key(dealer_strength)) {
                    Ordering::Greater => {
                        result.play_wins += 4f64;
                    },
                    Ordering::Less => {

                    },
                    Ordering::Equal => {
                        result.play_wins += 2f64;
                    }
                }
            } else {
                result.play_wins += 3f64;
            }

            use Kind::*;
            match player_strength.kind {
                Pair(_) => {
                    result.pair_plus_total += 2f64;
                },
                Flush(_) => {
                    result.pair_plus_total += 5f64;
                },
                Straight(_) => {
                    result.pair_plus_total += 7f64;
                    result.play_wins += 2f64;
                    result.fold_wins += 2f64;
                },
                ThreeKind(_) => {
                    result.pair_plus_total += 31f64;
                    result.play_wins += 5f64;
                    result.fold_wins += 5f64;
                },
                StraightFlush(_) => {
                    result.pair_plus_total += 41f64;
                    result.play_wins += 6f64;
                    result.fold_wins += 6f64;
                }
                _ => {}
            }
        }
        retval.insert(other_player_hand.into_iter().collect(), result);
    }
    retval
}

#[tokio::main]
async fn main() {
    let mut results: HashMap<(CardSet, CardSet), ThreeCardResult> = HashMap::new();
    let deck = standard_deck().raw;
    let expected_suits = vec![0, 1, 2];
    let combos = combinations(&deck[..], 3).filter(|combo| {
        combo.iter().map(|c| {let Suit(suit) = c.suit; suit}).collect::<Vec<_>>() == expected_suits
    });
    let num_combos = combos.clone().count();
    let num_player_hands = num_combos;
    let mut num_hands = 0;
    let mut futures = Vec::new();
    for (cidx, player_hand) in combos.into_iter().enumerate() {
        let player_hand: Vec<Card> = player_hand.into_iter().copied().collect();
        futures.push((player_hand.clone(), tokio::spawn(calc_single_hand(player_hand))));
    }

    let num_futures = futures.len();
    let mut last_p = None;
    for (fidx, (player_hand, f)) in futures.into_iter().enumerate() {
        let p = (fidx * 100) / num_futures;
        if Some(p) != last_p {
            last_p = Some(p);
            println!("{}/{}", fidx, num_futures);
        }
        let result = f.await.unwrap();
        let player_hand = player_hand.into_iter().collect();
        for (k, r) in result.into_iter() {
            results.insert((player_hand, k), r);
        }
    }

    let mut results: Vec<_> = results.into_iter().collect();
    results.sort_by_key(|(e, _)| e.clone());

    let mut total_p = 0f64;
    let num_results = results.len();
    for ((player_cards, other_player_cards), result) in results {
        let hands = result.hands as f64;
        println!("{} | {}: {} {}", player_cards.iter().map(|c| c.rank.to_string()).collect::<Vec<String>>().join(" "), other_player_cards.iter().map(|c| c.to_string()).collect::<Vec<String>>().join(" "), result.play_wins / hands, result.fold_wins / hands);
        if result.play_wins < result.fold_wins {
            total_p += result.fold_wins / hands;
        } else {
            total_p += result.play_wins / hands;
        }
    }
    println!("Total: {}", total_p / (num_results as f64));
}
