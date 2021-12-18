use crate::card::*;
use crate::special_card::*;
use crate::viewstate::*;
use crate::gamestate::PlayerState;

use ts_rs::{TS, export};

use serde::{Serialize, Deserialize};
use async_trait::async_trait;
use tokio::sync::oneshot;
use std::sync::{Arc, Mutex};

pub type Chips = i64;
pub type PlayerId = String;
pub type PlayerRole = usize; // 0 is dealer, 1 is to the left of the dealer, etc.
pub type PokerRoundError = String;

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
#[derive(TS)]
#[serde(tag = "kind", content="data")]
pub enum Facing {
    FaceUp,
    FaceDown
}

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
#[derive(TS)]
#[serde(tag = "kind", content="data")]
pub enum BetResp {
    Bet(Chips),
    Fold
}

pub type ReplaceResp = Vec<usize>;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum PlayerResp {
    Bet(BetResp),
    Replace(ReplaceResp)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[derive(TS)]
pub struct PokerLogUpdate {
    pub round: usize,
    pub log: Vec<PokerViewDiff<PlayerId>>
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PokerViewUpdate {
    pub viewstate: PokerViewState,
    pub diff: Vec<PokerLogUpdate>
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[derive(TS)]
pub struct PokerVariantDesc {
    pub name: String
}

#[derive(Clone, Serialize)]
#[derive(TS)]
pub struct PokerVariants {
    pub descs: Vec<PokerVariantDesc>,
    #[serde(skip)]
    pub variants: Vec<PokerVariant>,
}

#[derive(Clone, Serialize, Deserialize)]
#[derive(TS)]
pub struct DealersChoiceResp {
    pub variant_idx: usize,
    pub special_cards: Vec<SpecialCard>,
}

#[async_trait]
pub trait PlayerInputSource: Send + Sync {
    async fn bet(&self, call_amount: Chips, min_bet: Chips) -> BetResp;
    async fn replace(&self, max_can_replace: usize) -> ReplaceResp;
    async fn dealers_choice(&self, variants: Vec<PokerVariantDesc>) -> DealersChoiceResp;
    fn update(&self, viewstate: PokerViewUpdate);
}

#[derive(Clone)]
pub enum Round {
    Ante,
    DrawToHand {
        facing: Vec<Facing>
    },
    DrawToCommunity {
        quant: usize
    },
    Bet {
        starting_player: PlayerRole,
    },
    Replace {
        max_replace_fun: fn (&PlayerState) -> usize,
    }
}

#[derive(Clone)]
pub struct PokerVariant {
    pub rules: Vec<Round>,
    pub use_from_hand: usize,
}

#[derive(Clone)]
pub struct LivePlayer {
    pub player_id: PlayerId,
    pub chips: Chips,
    pub input: Arc<dyn PlayerInputSource>
}

pub fn texas_hold_em() -> PokerVariant {
    use Facing::*;
    use Round::*;
    PokerVariant {
        rules: vec![
            Ante,
            DrawToHand{
                facing: vec![FaceDown, FaceDown]
            },
            Bet {
                starting_player: 1
            },
            DrawToCommunity {
                quant: 3
            },
            Bet {
                starting_player: 1
            },
            DrawToCommunity {
                quant: 1
            },
            Bet {
                starting_player: 1
            },
            DrawToCommunity {
                quant: 1
            },
            Bet {
                starting_player: 1
            }
        ],
        use_from_hand: 2,
    }
}

pub fn omaha_hold_em() -> PokerVariant {
    use Facing::*;
    use Round::*;
    PokerVariant {
        rules: vec![
                Ante,
                DrawToHand{
                    facing: vec![FaceDown, FaceDown, FaceDown, FaceDown]
                },
                Bet {
                    starting_player: 1
                },
                DrawToCommunity {
                    quant: 3
                },
                Bet {
                    starting_player: 1
                },
                DrawToCommunity {
                    quant: 1
                },
                Bet {
                    starting_player: 1
                },
                DrawToCommunity {
                    quant: 1
                },
                Bet {
                    starting_player: 1
                }
            ],
        use_from_hand: 2,
    }
}

pub fn five_card_stud() -> PokerVariant {
    use Facing::*;
    use Round::*;
    PokerVariant {
        rules: vec![
            Ante,
            DrawToHand{
                facing: vec![FaceDown, FaceDown, FaceDown, FaceDown, FaceDown]
            },
            Bet {
                starting_player: 1
            }
        ],
        use_from_hand: 5,
    }
}

pub fn seven_card_stud() -> PokerVariant {
    use Facing::*;
    use Round::*;
    PokerVariant {
        rules: vec![
            Ante,
            DrawToHand{
                facing: vec![FaceDown, FaceDown, FaceUp]
            },
            Bet {
                starting_player: 1
            },
            DrawToHand{
                facing: vec![FaceUp]
            },
            Bet {
                starting_player: 1
            },
            DrawToHand {
                facing: vec![FaceUp]
            },
            Bet {
                starting_player: 1
            },
            DrawToHand {
                facing: vec![FaceUp]
            },
            Bet {
                starting_player: 1
            },
            DrawToHand {
                facing: vec![FaceDown]
            },
            Bet {
                starting_player: 1
            },
        ],
        use_from_hand: 5,
    }
}

fn three_or_four_with_ace(player: &PlayerState) -> usize {
    if player.hand.iter().any(|c| c.card.rank == 0) {
        4
    } else {
        3
    }
}

pub fn five_card_draw() -> PokerVariant {
    use Facing::*;
    use Round::*;
    PokerVariant {
        rules: vec![
            Ante,
            DrawToHand{
                facing: vec![FaceDown; 5]
            },
            Bet {
                starting_player: 1
            },
            Replace {
                max_replace_fun: three_or_four_with_ace
            },
            Bet {
                starting_player: 1
            }
        ],
        use_from_hand: 5,
    }
}

impl PokerVariants {
    pub fn all() -> PokerVariants {
        let all = vec![
            ("Texas Hold 'Em", texas_hold_em()),
            ("Omaha Hold 'Em", omaha_hold_em()),
            ("Seven Card Stud", seven_card_stud()),
            ("Five Card Stud", five_card_stud()),
            ("Five Card Draw", five_card_draw())
        ];
        let (names, variants): (Vec<_>, Vec<_>) = all.into_iter().unzip();
        PokerVariants { descs: names.into_iter().map(|name| PokerVariantDesc{name: name.to_string()}).collect(), variants }
    }
}

impl DealersChoiceResp {
    pub fn default() -> DealersChoiceResp {
        DealersChoiceResp {
            variant_idx: 0,
            special_cards: Vec::new(),
        }
    }
}
