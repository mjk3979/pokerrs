use crate::card::*;

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
