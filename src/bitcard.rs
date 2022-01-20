use crate::card::*;

use serde::{Serialize, Deserialize};
use ts_rs::{TS, export};
use std::ops::Index;
use std::iter::FromIterator;

#[derive(Copy, Clone, Eq, Hash, PartialEq, Ord, PartialOrd, Serialize, Deserialize, TS)]
pub struct RankTuple {
    rank_field: u64,
    length: usize,
}

#[derive(Copy, Clone, Eq, PartialEq, Serialize, Deserialize, TS)]
pub struct CardTuple {
    card_field: u64,
    length: usize,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct CardTupleIter {
    card_tuple: CardTuple,
    idx: usize
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct RankTupleIter {
    rank_tuple: RankTuple,
    idx: usize,
    ridx: usize,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Ord, PartialOrd, Hash)]
pub struct CardSet {
    card_field: u64
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct CardSetIter {
    card_set: CardSet,
    idx: usize,
}

impl RankTuple {
    pub fn new() -> RankTuple {
        RankTuple {
            rank_field: 0,
            length: 0
        }
    }

    pub fn get(&self, idx: usize) -> Rank {
        if idx >= self.length {
            panic!("Attempt to get idx {} in RankTuple of length {}", idx, self.length);
        } else {
            ((self.rank_field >> (idx * 4)) & 15) as usize
        }
    }

    pub fn push(&mut self, rank: Rank) {
        assert!(self.length < 16);
        self.rank_field |= ((rank & 15) as u64) << (self.length * 4);
        self.length += 1;
    }

    pub fn sort(&mut self) {
        let mut counts: [u8; NUM_RANKS+1] = [0; NUM_RANKS+1];
        for idx in 0..self.length {
            counts[self.get(idx)] += 1;
        }
        self.clear();
        for (rank, &count) in counts.into_iter().enumerate() {
            for _ in 0..count {
                self.push(rank);
            }
        }
    }

    pub fn clear(&mut self) {
        self.rank_field = 0;
        self.length = 0;
    }

    pub fn iter(&self) -> RankTupleIter {
        if self.length == 0 {
            RankTupleIter {
                rank_tuple: *self,
                idx: 1,
                ridx: 0,
            }
        } else {
            RankTupleIter {
                rank_tuple: *self,
                idx: 0,
                ridx: self.length-1,
            }
        }
    }

    pub fn first(&self) -> Option<usize> {
        if self.length == 0 {
            None
        } else {
            Some(self.get(0))
        }
    }

    pub fn last(&self) -> Option<usize> {
        if self.length == 0 {
            None
        } else {
            Some(self.get(self.length-1))
        }
    }

    pub fn len(&self) -> usize {
        self.length
    }
}

impl CardTuple {
    pub fn new() -> CardTuple {
        CardTuple {
            card_field: 0,
            length: 0
        }
    }

    pub fn push(&mut self, card: Card) {
        assert!(self.length <= 9);
        let Suit(suit) = card.suit;
        self.card_field |= (suit as u64) << (self.length*6 + 4);
        self.card_field |= (card.rank as u64) << (self.length*6);
        self.length += 1;
    }

    pub fn get(&self, idx: usize) -> Card {
        if idx >= self.length {
            panic!("Attempt to get idx {} in CardTuple of length {}", idx, self.length);
        } else {
            let card = (self.card_field >> (idx * 6)) & 0b111111;
            let suit = Suit((card >> 4) as usize);
            let rank = (card & 0b1111) as usize;
            Card{suit, rank}
        }
    }

    pub fn len(&self) -> usize {
        self.length
    }

    pub fn iter(&self) -> CardTupleIter {
        CardTupleIter {
            card_tuple: *self,
            idx: 0
        }
    }
}

impl CardSet {
    pub fn new() -> CardSet {
        CardSet {
            card_field: 0,
        }
    }

    pub fn insert(&mut self, card: Card) -> bool {
        let Suit(suit) = card.suit;
        let idx = (suit * NUM_RANKS) + card.rank;
        let old_bit = self.contains_index(idx);
        self.card_field |= (1 << idx);
        !old_bit
    }

    pub fn remove(&mut self, card: Card) -> bool {
        let Suit(suit) = card.suit;
        let idx = (suit * NUM_RANKS) + card.rank;
        let old_bit = self.contains_index(idx);
        self.card_field &= !(1 << idx);
        old_bit
    }

    pub fn iter(&self) -> CardSetIter {
        CardSetIter {
            card_set: *self,
            idx: 0
        }
    }

    fn contains_index(&self, idx: usize) -> bool {
        (self.card_field & (1 << idx)) != 0
    }

    fn card_from_index(idx: usize) -> Card {
        let suit = Suit(idx / NUM_RANKS);
        let rank = idx % NUM_RANKS;
        Card{suit, rank}
    }
}

impl Iterator for CardTupleIter {
    type Item = Card;

    fn next(&mut self) -> Option<Self::Item> {
        if self.card_tuple.length <= self.idx {
            None
        } else {
            let retval = self.card_tuple.get(self.idx);
            self.idx += 1;
            Some(retval)
        }
    }
}

impl Iterator for RankTupleIter {
    type Item = Rank;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ridx < self.idx {
            None
        } else {
            let retval = self.rank_tuple.get(self.idx);
            self.idx += 1;
            Some(retval)
        }
    }
}

impl DoubleEndedIterator for RankTupleIter {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.ridx < self.idx {
            None
        } else {
            let retval = self.rank_tuple.get(self.ridx);
            if self.ridx == 0 {
                self.idx = 1;
            } else {
                self.ridx -= 1;
            }
            Some(retval)
        }
    }
}

impl ExactSizeIterator for RankTupleIter {
    fn len(&self) -> usize {
        self.ridx - self.idx + 1
    }
}

impl FromIterator<usize> for RankTuple {
    fn from_iter<T>(iter: T) -> Self
        where T: IntoIterator<Item = usize> {
        let mut rt = RankTuple::new();
        for r in iter {
            rt.push(r);
        }
        rt
    }
}

impl FromIterator<Card> for CardTuple {
    fn from_iter<T>(iter: T) -> Self
        where T: IntoIterator<Item = Card> {
        let mut retval = CardTuple::new();
        for card in iter {
            retval.push(card);
        }
        retval
    }
}

impl FromIterator<Card> for CardSet {
    fn from_iter<T>(iter: T) -> Self
        where T: IntoIterator<Item = Card> {
        let mut retval = CardSet::new();
        for card in iter {
            retval.insert(card);
        }
        retval
    }
}

impl From<Vec<usize>> for RankTuple {
    fn from(v: Vec<usize>) -> RankTuple {
        v.into_iter().collect()
    }
}

impl Iterator for CardSetIter {
    type Item = Card;

    fn next(&mut self) -> Option<Card> {
        while self.idx < (NUM_RANKS * NUM_SUITS) {
            if self.card_set.contains_index(self.idx) {
                self.idx += 1;
                return Some(CardSet::card_from_index(self.idx-1));
            } else {
                self.idx += 1;
            }
        }
        None
    }
}

impl From<CardTuple> for RankTuple {
    fn from(card_tuple: CardTuple) -> RankTuple {
        card_tuple.iter().map(|c| c.rank).collect()
    }
}

impl std::fmt::Debug for RankTuple {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "[{}]", self.iter().map(|r| r.to_string()).collect::<Vec<String>>().join(" "))
    }
}

impl std::fmt::Debug for CardTuple {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "[{}]", self.iter().map(|r| r.to_string()).collect::<Vec<String>>().join(" "))
    }
}

impl std::ops::Add for CardTuple {
    type Output = CardTuple;
    fn add(mut self, rhs: CardTuple) -> CardTuple {
        for card in rhs.iter() {
            self.push(card);
        }
        self
    }
}

mod test {
    use crate::bitcard::*;
    use crate::card::*;

    #[test]
    fn test_rank_tuple() {
        let mut rt = RankTuple::new();
        assert!(rt.len() == 0);
        assert!(rt.last().is_none());
        assert!(rt.first().is_none());

        rt.push(3);
        assert!(rt.len() == 1);
        assert!(rt.get(0) == 3, "{} != {}", rt.get(0), 3);
        assert!(rt.first() == Some(3));
        assert!(rt.last() == Some(3));

        rt.push(7);
        assert!(rt.len() == 2);
        assert!(rt.get(0) == 3, "{} != {}", rt.get(0), 3);
        assert!(rt.get(1) == 7);
        assert!(rt.first() == Some(3));
        assert!(rt.last() == Some(7));

        assert!(rt.iter().collect::<Vec<_>>() == vec![3, 7]);

        rt.push(NUM_RANKS);
        assert!(rt.len() == 3);
        assert!(rt.get(0) == 3, "{} != {}", rt.get(0), 3);
        assert!(rt.get(1) == 7);
        assert!(rt.get(2) == NUM_RANKS);
        assert!(rt.first() == Some(3));
        assert!(rt.last() == Some(NUM_RANKS));
        assert!(rt.iter().collect::<Vec<_>>() == vec![3, 7, NUM_RANKS]);

        let expected = vec![3, 5, 9, 0, 2, NUM_RANKS];
        let result = expected.iter().copied().collect::<RankTuple>().iter().collect::<Vec<_>>();
        assert!(result == expected, "{:?}", result);
    }

    #[test]
    fn test_card_tuple() {
        let mut ct = CardTuple::new();

        assert!(ct.len() == 0);

        ct.push(Card {
            rank: 4,
            suit: Suit(2)
        });

        assert!(ct.len() == 1);
        assert!(ct.get(0) == Card{rank: 4, suit: Suit(2)});

        ct.push(Card {
            rank: NUM_RANKS,
            suit: Suit(1),
        });
        assert!(ct.len() == 2);
        assert!(ct.get(0) == Card{rank: 4, suit: Suit(2)});
        assert!(ct.get(1) == Card{rank: NUM_RANKS, suit: Suit(1)});

        let cards: CardTuple = vec![4, 2, 1, 4, 9].into_iter().enumerate().map(|(i, rank)| Card{rank, suit: Suit(i%4)}).collect();
        assert!(cards.len() == 5);
        assert!(cards.get(0) == Card {
            rank: 4,
            suit: Suit(0),
        });
        assert!(cards.get(1) == Card {
            rank: 2,
            suit: Suit(1),
        });
        assert!(cards.get(2) == Card {
            rank: 1,
            suit: Suit(2),
        });
        assert!(cards.get(3) == Card {
            rank: 4,
            suit: Suit(3),
        });
        assert!(cards.get(4) == Card {
            rank: 9,
            suit: Suit(0),
        });
        let result = cards.iter().collect::<Vec<_>>();
        assert!(result == vec![Card {
                rank: 4,
                suit: Suit(0),
            },
            Card {
                rank: 2,
                suit: Suit(1),
            },
            Card {
                rank: 1,
                suit: Suit(2),
            },
            Card {
                rank: 4,
                suit: Suit(3),
            },
            Card {
                rank: 9,
                suit: Suit(0),
            },
        ]);

        let ranks: RankTuple = cards.into();
        assert!(ranks.len() == 5);
        assert!(ranks.get(0) == 4, "{:?}", ranks);
        assert!(ranks.get(1) == 2);
        assert!(ranks.get(2) == 1);
        assert!(ranks.get(3) == 4);
        assert!(ranks.get(4) == 9);
    }

    #[test]
    fn test_card_set() {
        let mut cs = CardSet::new();
        
        assert!(cs.iter().count() == 0);

        assert!(cs.insert(Card {
            rank: 4,
            suit: Suit(2)
        }));
        let result = cs.iter().collect::<Vec<_>>();
        assert!(result == vec![Card{rank: 4, suit: Suit(2)}], "{:?}", result);
        assert!(!cs.insert(Card {
            rank: 4,
            suit: Suit(2)
        }));
        let result = cs.iter().collect::<Vec<_>>();
        assert!(result == vec![Card{rank: 4, suit: Suit(2)}], "{:?}", result);

        assert!(cs.insert(Card {
            rank: 9,
            suit: Suit(1)
        }));
        let result = cs.iter().collect::<Vec<_>>();
        assert!(result == vec![Card{rank: 9, suit: Suit(1)}, Card{rank: 4, suit: Suit(2)}], "{:?}", result);
        assert!(!cs.insert(Card {
            rank: 9,
            suit: Suit(1)
        }));
        let result = cs.iter().collect::<Vec<_>>();
        assert!(result == vec![Card{rank: 9, suit: Suit(1)}, Card{rank: 4, suit: Suit(2)}], "{:?}", result);
    }
}
