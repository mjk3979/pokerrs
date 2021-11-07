use crate::card::*;

use std::cmp::Ordering;

#[derive(Eq)]
pub struct HandStrength {
    pub kind: Kind,
    pub cards: Vec<Card>
}

impl HandStrength {
    pub fn kickers(&self) -> Vec<Card> {
        return self.cards.iter().filter(|card| {
            return match self.kind {
                Kind::FullHouse(rank1, rank2) => card.rank != rank1 && card.rank != rank2,
                Kind::TwoPair(rank1, rank2) => card.rank != rank1 && card.rank != rank2,
                Kind::StraightFlush(rank) => card.rank != rank,
                Kind::FourKind(rank) => card.rank != rank,
                Kind::Flush(rank) => card.rank != rank,
                Kind::Straight(rank) => card.rank != rank,
                Kind::ThreeKind(rank) => card.rank != rank,
                Kind::Pair(rank) => card.rank != rank,
                Kind::HighCard(rank) => card.rank != rank,
            }
        }).map(|card| *card).collect();
    }
}

impl PartialEq for HandStrength {
    fn eq(&self, other: &Self) -> bool {
        return self.kind == other.kind;
    }
}

impl Ord for HandStrength {
    fn cmp(&self, other: &Self) -> Ordering {
        let rank_map = |card: &Card| card.rank;
        return match self.kind.cmp(&other.kind) {
            Ordering::Less => Ordering::Less,
            Ordering::Greater => Ordering::Greater,
            Ordering::Equal => self.kickers().iter().map(rank_map).cmp(other.kickers().iter().map(rank_map))
        };
    }
}

impl PartialOrd for HandStrength {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        return Some(self.cmp(other));
    }
}

pub fn get_best_kind_and_rank(cards: &[Card]) -> Kind {
    let mut sorted = cards.to_vec();
    sorted.sort_unstable_by_key(|c| c.rank);
    let mut is_straight = false;
    let mut is_flush = false;
    if cards.len() == 5 {
        for idx in 1..5 {
            if cards[idx].rank != cards[0].rank + idx {
                break
            } else if idx == 4 {
                is_straight = true;
            }
        }
        is_flush = cards.iter().all(|c| c.suit == cards[0].suit);
    }

    let mut by_rank: [usize; NUM_RANKS+1] = [0; NUM_RANKS+1];
    for card in cards {
        by_rank[card.rank] += 1;
    }
    let mut by_amount: [Vec<Rank>; NUM_SUITS] = Default::default();
    for (rank, num) in by_rank.iter().enumerate() {
        if *num != 0 {
            by_amount[*num-1].push(rank)
        }
    }

    if is_straight && is_flush {
        return Kind::StraightFlush(*by_amount[0].last().unwrap());
    }

    if let Some(rank) = by_amount[3].first() {
        return Kind::FourKind(*rank);
    } else if let Some(high) = by_amount[2].first() {
        // Can't chain, have to do this instead
        if let Some(low) = by_amount[1].first() {
            return Kind::FullHouse(*high, *low);
        }
    }
    return Kind::HighCard(*by_amount[0].last().unwrap());
}
