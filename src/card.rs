use crate::game::PokerRoundError;

use rand::prelude::*;
use serde::{Serialize, Deserialize};
use ts_rs::{TS, export};

use std::cmp::Ordering;

#[derive(Debug, Eq, PartialEq, Clone, Copy, Hash, Serialize, Deserialize, TS)]
pub struct Suit(pub usize);

pub type Rank = usize;

pub type StrengthRank = usize;

#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Ord, TS)]
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", content="data")]
pub enum Kind {
    HighCard(StrengthRank),
    Pair(StrengthRank),
    TwoPair {
        high: StrengthRank,
        low: StrengthRank,
    },
    ThreeKind(StrengthRank),
    Straight(StrengthRank),
    Flush(Vec<StrengthRank>),
    FullHouse {
        high: StrengthRank,
        low: StrengthRank,
    },
    FourKind(StrengthRank),
    StraightFlush(StrengthRank),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, TS)]
#[derive(Serialize, Deserialize)]
pub struct Card {
    pub suit: Suit,
    pub rank: Rank,
}

fn rank_name(rank: &Rank) -> &'static str {
    const NAMES: [&str; NUM_RANKS+1] = [
        "Ace", "Two", "Three", "Four", "Five",
        "Six", "Seven", "Eight", "Nine", "Ten",
        "Jack", "Queen", "King", "Ace"
    ];
    NAMES[*rank]
}

impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use Kind::*;
        match self {
            HighCard(r) => write!(f, "{} high", rank_name(r))?,
            Pair(r) => write!(f, "Pair of {}s", rank_name(r))?,
            TwoPair{high, low} => write!(f, "Two pair {}s over {}s", rank_name(high), rank_name(low))?,
            ThreeKind(r) => write!(f, "Three {}s", rank_name(r))?,
            Straight(r) => write!(f, "Straight, {} high", rank_name(r))?,
            Flush(cards) => write!(f, "Flush, {} high", rank_name(&cards[0]))?,
            FullHouse{high, low} => write!(f, "Full House {}s over {}s", rank_name(high), rank_name(low))?,
            FourKind(r) => write!(f, "Four {}s", rank_name(r))?,
            StraightFlush(r) => write!(f, "Straight Flush, {} high", rank_name(r))?
            //_ => panic!("")
        }
        Ok(())
    }
}

impl std::fmt::Display for Card {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:1}{:-2}", char::from_u32(match self.suit {
                Suit(0) => 0x2660,
                Suit(1) => 0x2665,
                Suit(2) => 0x2666,
                Suit(3) => 0x2663,
                _ => panic!("Invalid suit {:?}", self.suit)
            }).unwrap(),
            match self.rank {
                0 => "A".to_string(),
                10 => "J".to_string(),
                11 => "Q".to_string(),
                12 => "K".to_string(),
                _ => (self.rank + 1).to_string()
            }
        )
    }
}

impl From<(usize, usize)> for Card {
    fn from(t: (usize, usize)) -> Card {
        let (rank, suit) = t;
        Card {
            rank,
            suit: Suit(suit)
        }
    }
}

pub trait Deck {
    fn draw(&mut self) -> Result<Card, PokerRoundError>;
}

pub trait Shuffleable {
    fn shuffle<R: Rng>(&mut self, rng: &mut R);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VecDeck {
    raw: Vec<Card>
}

impl Deck for VecDeck {
    fn draw(&mut self) -> Result<Card, PokerRoundError> {
        if let Some(card) = self.raw.pop() {
            Ok(card)
        } else {
            Err("Ran out of cards in deck".to_string())
        }
    }
}

impl Shuffleable for VecDeck {
    fn shuffle<R: Rng>(&mut self, rng: &mut R) {
        self.raw.shuffle(rng);
    }
}

pub const NUM_RANKS: usize = 13;
pub const NUM_SUITS: usize = 4;

pub fn standard_deck() -> VecDeck {
    let mut raw = Vec::new();
    for suit in 0..NUM_SUITS {
        for rank in 0..NUM_RANKS {
            raw.push(Card {
                suit: Suit(suit),
                rank
            });
        }
    }
    VecDeck {
        raw
    }
}

#[test]
fn test_standard_deck() {
    let deck = standard_deck();
    assert_eq!(deck.raw.len(), 52);
    for rank in 0..NUM_RANKS {
        assert_eq!(deck.raw.iter().filter(|x| x.rank == rank).count(), NUM_SUITS)
    }
    for suit in 0..NUM_SUITS {
        assert_eq!(deck.raw.iter().filter(|x| x.suit == Suit(suit)).count(), NUM_RANKS)
    }
}
