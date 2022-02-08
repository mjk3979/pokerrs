use crate::card::*;
use crate::game::*;
use crate::gamestate::*;
use crate::special_card::*;
use crate::table::*;
use crate::gamestate;

use ts_rs::{TS, export};

use serde::{Serialize, Deserialize};

use std::collections::{HashMap};
use std::hash::Hash;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[derive(TS)]
#[serde(tag = "kind", content="data")]
pub enum CardViewState {
    Visible(CardState),
    Invisible
}

#[derive(Clone, Copy, Hash, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[derive(TS)]
pub enum PokerButton {
    Dealer,
    BigBlind,
    SmallBlind,
}

impl CardViewState {
    pub fn from_card_state_and_same_player(state: &CardState, same_player: bool) -> CardViewState {
        use CardViewState::*;
        use Facing::*;
        if same_player || state.facing == FaceUp {
            Visible(*state)
        } else {
            Invisible
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[derive(TS)]
pub struct PlayerViewState {
    pub chips: Chips,
    pub total_bet: Chips,
    pub hand: Vec<CardViewState>,
    pub folded: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[derive(TS)]
pub struct PokerVariantViewState {
    pub use_from_hand: usize,
}

#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
#[derive(TS)]
pub struct PokerViewState {
    pub role: PlayerRole,
    pub players: HashMap<PlayerRole, PlayerViewState>,
    pub community_cards: Vec<CardViewState>,
    pub bet_this_round: HashMap<PlayerRole, Chips>,
    pub current_turn: Option<PlayerRole>,

    #[serde(skip_serializing)]
    pub rules: SpecialRules,

    pub variant: PokerVariantViewState,
}

#[derive(Clone, Serialize, Deserialize)]
#[derive(TS)]
pub struct TableViewState {
    pub running: bool,
    pub roles: Option<HashMap<PlayerRole, PlayerId>>,
    pub buttons: HashMap<PlayerId, PokerButton>,
    pub seats: HashMap<PlayerId, Seat>,
    pub config: TableConfig,
    pub running_variant: Option<PokerVariantDesc>,
}

#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
#[derive(TS)]
#[serde(tag = "kind", content="data")]
pub enum BetDiffKind {
    Blind(String),
    Check,
    Call,
    Raise {
        diff_from_last_raise: Chips,
        total: Chips
    }
}

#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
#[derive(TS)]
#[serde(tag = "kind", content="data")]
pub enum PokerViewDiff<P> {
    Draw {
        player: P,
        drawn: Vec<CardViewState>
    },
    CommunityDraw {
        drawn: Vec<CardViewState>
    },
    Fold {
        player: P,
    },
    TurnStart {
        player: P
    },
    Bet {
        bet_kind: BetDiffKind,
        player: P,
        chips: Chips
    },
    Replace {
        player: P,
        discard: Vec<CardViewState>,
        drawn: Vec<CardViewState>
    },
    ShowCards {
        player: P,
        shown: Vec<(usize, CardViewState)>,
        strength: HandStrength
    },
    Winners(Winners<P>),
    Unknown
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[derive(Serialize, Deserialize, TS)]
#[serde(tag = "kind", content="data")]
pub enum TableEvent {
    PlayerJoined {
        player_id: PlayerId
    },
    VariantChange {
        new_variant_desc: PokerVariantDesc
    },
    AnteChange {
        new_table_rules: TableRules
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[derive(Serialize, Deserialize, TS)]
#[serde(tag = "kind", content="data")]
pub enum TableViewDiff<G> 
    where G: Clone + std::fmt::Debug
{
    GameDiff(G),
    TableDiff(TableEvent),
}


impl std::fmt::Display for CardViewState {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            CardViewState::Invisible => {
                write!(f, "a card")?;
            },
            CardViewState::Visible(CardState{card, facing}) => {
                write!(f, "{} {}", card, if *facing == Facing::FaceDown {"face down"} else {"face up"})?;
            }
        }
        Ok(())
    }
}

pub trait PokerViewClient {
    fn update_table(&self, table: TableViewState);
}

impl<P: std::fmt::Display> std::fmt::Display for PokerViewDiff<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use PokerViewDiff::*;
        match self {
            Draw {player, drawn} => {
                for cvs in drawn {
                    write!(f, "{} drew {}", player, cvs)?;
                }
            },
            CommunityDraw {drawn} => {
                write!(f, "{} drawn to community", drawn.iter().map(|cvs| cvs.to_string()).collect::<Vec<String>>().join(", "))?;
            },
            Fold {player} => {
                write!(f, "{} folded :(", player)?;
            },
            TurnStart {player} => {
                write!(f, "{} is taking their turn", player)?;
            },
            Bet {bet_kind, player, chips} => {
                write!(f, "{} ", player)?;
                use BetDiffKind::*;
                match bet_kind {
                    Blind(name) => {write!(f, "{} {}", name, chips)?;},
                    Check => { write!(f, "checked")?; },
                    Call => { write!(f, "called {}", chips)?; },
                    Raise{diff_from_last_raise, ..} => { write!(f, "raised {}", diff_from_last_raise)?; }
                }
            }
            Replace{player, drawn, discard} => {
                assert!(drawn.len() == discard.len());
                if !drawn.is_empty() && drawn.iter().all(|cvs| if let CardViewState::Visible(_) = cvs {true} else {false}) {
                    write!(f, "{} replaced {} with {}", player, PokerViewState::format_cards(discard), PokerViewState::format_cards(drawn));
                } else {
                    write!(f, "{} replaced {} cards", player, drawn.len());
                }
            },
            ShowCards{player, shown, strength} => {
                write!(f, "{} shows {} to give them a {}", player, shown.iter().map(|(_, cvs)| cvs.to_string()).collect::<Vec<String>>().join(", "), strength)?;
            },
            PokerViewDiff::Winners(gamestate::Winners{winners_by_pot}) => {
                for (Subpot{chips, ..}, winners) in winners_by_pot {
                    if winners.len() == 1 {
                        write!(f, "{} wins {}", winners.first().unwrap(), chips)?;
                    } else {
                        write!(f, "{} split {}", winners.iter().map(|w| w.to_string()).collect::<Vec<String>>().join(", "), chips)?;
                    }
                }
            },
            Unknown => {}
        }
        Ok(())
    }
}

impl<P: Clone + Eq + Hash> PokerViewDiff<P> {
    pub fn from_blind_name(chips: Chips, better: P, name: String) -> PokerViewDiff<P> {
        PokerViewDiff::Bet {
            bet_kind: BetDiffKind::Blind(name),
            player: better,
            chips
        }
    }

    pub fn from_player_bet_min_bet(better: P, chips: Chips, bet_this_round: Chips, last_bet: Chips) -> PokerViewDiff<P> {
        use PokerViewDiff::*;
        let call_amount = chips - bet_this_round;
        if (last_bet == 0 && chips == 0) || (call_amount == 0) {
            Bet {
                bet_kind: BetDiffKind::Check,
                player: better,
                chips: call_amount
            }
        } else if chips <= last_bet {
            Bet {
                bet_kind: BetDiffKind::Call,
                player: better,
                chips: call_amount
            }
        } else {
            let raise = chips - last_bet;
            Bet {
                bet_kind: BetDiffKind::Raise {
                    diff_from_last_raise: raise,
                    total: chips
                },
                player: better,
                chips
            }
        }
    }

    pub fn from_drawn_viewer(player: P, drawn: &[CardState], viewer: Option<&P>) -> PokerViewDiff<P> {
        let same_player = Some(&player) == viewer;
        PokerViewDiff::Draw {
            player,
            drawn: drawn.iter().map(|cs| CardViewState::from_card_state_and_same_player(cs, same_player)).collect()
        }
    }

    pub fn from_replace_discard_drawn_viewer(player: P, discard: &[CardState], drawn: &[CardState], viewer: Option<&P>) -> PokerViewDiff<P> {
        let same_player = Some(&player) == viewer;
        PokerViewDiff::Replace {
            player,
            discard: discard.iter().map(|cs| CardViewState::from_card_state_and_same_player(cs, same_player)).collect(),
            drawn: drawn.iter().map(|cs| CardViewState::from_card_state_and_same_player(cs, same_player)).collect()
        }
    }

    pub fn convert<T: Clone>(&self, mapping: &HashMap<P, T>) -> PokerViewDiff<T> {
        use PokerViewDiff::*;
        match self {
            Draw{player, drawn} => Draw{player: mapping.get(player).cloned().unwrap(), drawn: drawn.clone()},
            CommunityDraw{drawn} => CommunityDraw{drawn: drawn.clone()},
            Fold{player} => Fold{player: mapping.get(player).cloned().unwrap()},
            TurnStart{player} => TurnStart{player: mapping.get(player).cloned().unwrap()},
            Bet{bet_kind, player, chips} => Bet{bet_kind: bet_kind.clone(), player: mapping.get(player).cloned().unwrap(), chips: *chips},
            Replace{player, discard, drawn} => Replace{player: mapping.get(player).cloned().unwrap(), discard: discard.clone(), drawn: drawn.clone()},
            ShowCards{player, shown, strength} => ShowCards{player: mapping.get(player).cloned().unwrap(), shown: shown.clone(), strength: strength.clone()},
            Winners(ws) => Winners(ws.convert(mapping)),
            Unknown => Unknown,
        }
    }
}

#[derive(Clone, Debug)]
pub enum PokerGlobalViewDiff<P: Clone> {
    Common(PokerViewDiff<P>),
    Draw {
        player: P,
        drawn: Vec<CardState>
    },
    Replace {
        player: P,
        drawn: Vec<CardState>,
        discard: Vec<CardState>
    }
}

impl<P: Clone + Eq + Hash> PokerGlobalViewDiff<P> {
    pub fn player_diff(&self, role: Option<&P>) -> PokerViewDiff<P> {
        use PokerGlobalViewDiff::*;
        match self {
            Common(diff) => diff.clone(),
            Draw{player, drawn} => {
                PokerViewDiff::from_drawn_viewer(player.clone(), drawn, role)
            },
            Replace{player, drawn, discard} => {
                PokerViewDiff::from_replace_discard_drawn_viewer(player.clone(), discard, drawn, role)
            }
        }
    }

    pub fn convert<T: Clone>(&self, mapping: &HashMap<P, T>) -> PokerGlobalViewDiff<T> {
        use PokerGlobalViewDiff::*;
        match self {
            Common(v) => Common(v.convert(mapping)),
            Draw{player, drawn} => Draw{player: mapping.get(player).cloned().unwrap(), drawn: drawn.clone()},
            Replace{player, drawn, discard} => Replace{player: mapping.get(player).cloned().unwrap(), drawn: drawn.clone(), discard: discard.clone()},
        }
    }
}

pub type BetInvalidError = String;

impl PokerViewState {
    pub fn from_handstate_and_player(state: &HandState, variant: &PokerVariant, rules: &SpecialRules, player: PlayerRole) -> PokerViewState {
        use Facing::*;
        use CardViewState::*;
        let players = state.players.iter().map(|(&role, player_state)| {
            (role, PlayerViewState {
                chips: player_state.chips,
                total_bet: player_state.total_bet,
                hand: player_state.hand.iter().map(|card_state|
                    CardViewState::from_card_state_and_same_player(card_state, role == player)).collect(),
                folded: player_state.folded,
            })
        }).collect();
        let community_cards = state.community_cards.iter().map(|c| Visible(CardState{
            card: c,
            facing: FaceUp
        })).collect();
        let (bet_this_round, current_turn) = if let Some(RoundState::Bet{all_bets, player, ..}) = &state.cur_round {
            (all_bets.clone(), Some(*player))
        } else if let Some(RoundState::Replace{player, ..}) = &state.cur_round {
            (HashMap::new(), Some(*player))
        } else {
            (HashMap::new(), None)
        };
        PokerViewState {
            role: player,
            players,
            community_cards,
            bet_this_round,
            current_turn,
            rules: rules.clone(),
            variant: PokerVariantViewState {
                use_from_hand: variant.use_from_hand
            },
        }
    }

    pub fn valid_bet(&self, min_bet: Chips, call_amount: Chips, bet: Chips, role: PlayerRole) -> Result<(), BetInvalidError> {
        // Check all-ins. All-ins are always valid (until max-bets are implemented)
        let bettable = self.bettable_chips(role);
        if bet == bettable {
            return Ok(());
        }
        // Check if under the min bet. That is invalid if not an all-in
        if bet != call_amount && bet < min_bet {
            return Err("Bet less than minimum".to_string());
        }
        // Check if the player has the chips to make the bet, otherwise invalid
        if bet > bettable {
            return Err("Player has too few chips".to_string());
        }
        // If all these pass, then the bet is valid
        return Ok(());
    }
    
    pub fn bettable_chips(&self, role: PlayerRole) -> Chips {
        let player = self.players.get(&role).unwrap();
        assert!(player.chips >= player.total_bet);
        return player.chips - player.total_bet;//- self.bet_this_round.get(&role).copied().unwrap_or(0);
    }

    pub fn format_card(card: &CardViewState) -> String {
        use CardViewState::*;
        match card {
            Invisible => "XXX".to_string(),
            Visible(card) => format!("{}", card.card)
        }
    }

    pub fn format_cards(cards: &[CardViewState]) -> String {
        cards.iter().map(|c| Self::format_card(c)).collect::<Vec<String>>().join(" ")
    }

    pub fn hand_str(&self, role: PlayerRole) -> String {
        Self::format_cards(&self.players.get(&role).unwrap().hand)
    }

    pub fn community_str(&self) -> String {
        Self::format_cards(&self.community_cards)
    }

    pub fn pot(&self) -> Chips {
        self.bet_this_round.values().copied().sum::<Chips>() + self.players.values().map(|p| p.total_bet).sum::<Chips>()
    }
}

impl std::fmt::Display for TableEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use TableEvent::*;
        match self {
            PlayerJoined {player_id} => {
                write!(f, "{} joined", player_id)?;
            },
            VariantChange {new_variant_desc} => {
                write!(f, "Game changed to {}", new_variant_desc.name)?;
                for SpecialCardGroupDesc{name, ..} in &new_variant_desc.special_cards {
                    write!(f, "\n{}", name)?;
                }
            },
            AnteChange {new_table_rules} => {
                use AnteRule::*;
                match &new_table_rules.ante {
                    Ante(ante) => write!(f, "Ante is now {}", ante)?,
                    Blinds(blinds) => write!(f, "Blinds are now {}", blinds.iter().map(|b| b.amount.to_string()).collect::<Vec<String>>().join(", "))?,
                }
            },
        }
        Ok(())
    }
}

impl<G: std::fmt::Display + Clone + std::fmt::Debug> std::fmt::Display for TableViewDiff<G> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use TableViewDiff::*;
        match self {
            GameDiff(g) => write!(f, "{}", g)?,
            TableDiff(t) => write!(f, "{}", t)?,
        }
        Ok(())
    }
}

/*
impl Serialize for TableViewDiff<G>
where G: Clone + Serialize
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer {
        let mut state = serializer.serialize_struct_variant("TableViewDiff", 
    }
}
*/
