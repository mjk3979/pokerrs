use crate::bitcard::*;
use crate::card::*;
use crate::comb::*;

use serde::{Serialize, Deserialize};
use ts_rs::{TS, export};

#[derive(Copy, Hash, Clone, Ord, PartialOrd, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[derive(TS)]
pub enum SpecialCardType {
    Wild,
    WinsItAll,
}

#[derive(Clone, Hash, Ord, PartialOrd, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[derive(TS)]
pub struct SpecialCard {
    pub wtype: SpecialCardType,
    pub card: Card,
}

#[derive(Clone, Hash, Ord, PartialOrd, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[derive(TS)]
pub struct SpecialCardGroupDesc {
    pub name: String,
}

#[derive(Clone, Hash, Ord, PartialOrd, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[derive(TS)]
pub struct SpecialCardGroup {
    pub name: String,
    pub cards: Vec<SpecialCard>,
}

impl SpecialCardGroup {
    pub fn all() -> Vec<SpecialCardGroup> {
        use SpecialCardType::*;
        vec![SpecialCardGroup {
                name: "Twos Wild".to_string(),
                cards: (0..NUM_SUITS).map(|s| {
                    SpecialCard {
                        wtype: Wild,
                        card: Card{rank: 1, suit: Suit(s)}
                    }
                }).collect(),
            },
            SpecialCardGroup {
                name: "Man with the axe wins it all".to_string(),
                cards: vec![SpecialCard {
                    wtype: WinsItAll,
                    card: Card{rank: 12, suit: Suit(2)},
                }],
            },
        ]
    }
}

impl SpecialCardGroupDesc {
    pub fn all() -> Vec<SpecialCardGroupDesc> {
        SpecialCardGroup::all().into_iter().map(|g| SpecialCardGroupDesc{name: g.name}).collect()
    }
}

impl From<&SpecialCardGroupDesc> for SpecialCardGroup {
    fn from(desc: &SpecialCardGroupDesc) -> SpecialCardGroup {
        SpecialCardGroup::all().into_iter().find(|SpecialCardGroup{name, cards}| name == &desc.name).unwrap()
    }
}

pub fn wild_combinations(unwild: CardTuple, num_wild: usize) -> Vec<CardTuple> {
    if num_wild == 0 {
        return vec![unwild];
    }
    let mut possible_good_wild: u64 = 1;
    let mut suit_counts: [u8; NUM_SUITS] = [0; NUM_SUITS];
    let mut max_suit = 0;
    for Card{suit: Suit(suit), rank} in unwild.iter() {
        suit_counts[suit] += 1;
        if suit_counts[suit] > suit_counts[max_suit] {
            max_suit = suit;
        }
        possible_good_wild |= 1 << rank;
        for offset in 1..num_wild+1 {
            possible_good_wild |= 1 << ((rank+offset) % NUM_RANKS);
            possible_good_wild |= 1 << ((rank+NUM_RANKS-offset) % NUM_RANKS);
        }
    }
    let cards: CardSet = (0..NUM_RANKS).filter_map(|rank| {
        if possible_good_wild & (1 << rank) != 0 {
            Some(Card{rank, suit: Suit(max_suit)})
        } else {
            None
        }
    }).collect();
    let mut retval = Vec::new();
    for comb in combinations_with_replacement(cards.iter(), num_wild) {
        let mut result = unwild.clone();
        for card in comb {
            result.push(card);
        }
        retval.push(result);
    }
    retval
}
