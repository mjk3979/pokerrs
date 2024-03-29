use crate::bitcard::*;
use crate::card::*;
use crate::special_card::*;
use crate::game::*;
use crate::gamestate::*;
use crate::table::*;
use crate::server::*;
use crate::viewstate::*;

use ts_rs::{TS, export};

export! {
    BetResp,
    Facing,
    PokerLogUpdate,
    CardViewState,
    PlayerViewState,
    PokerButton,
    PokerVariantViewState,
    PokerViewState,
    TableConfig,
    TableViewState,
    BetDiffKind,
    PokerVariantDesc,
    PokerVariants,
    PokerVariantSelector,
    PokerViewDiff<PlayerId>,
    CardState,
    HandStrength,
    Subpot<PlayerId>,
    Winners<PlayerId>,
    Seat,
    Suit,
    Card,
    RankTuple,
    CardTuple,
    Kind,
    ServerActionRequest,
    ServerPlayer,
    ServerUpdate,
    SpecialCardType,
    SpecialCard,
    SpecialCardGroup,
    SpecialCardGroupDesc,
    DealersChoiceResp,
    AnteRuleChangeDesc,
    AnteRuleDesc,
    ServerTableParameters,
    TableRules,
    AnteRule,
    Blind,
    TableEvent,
    TableViewDiff<PokerViewDiff<PlayerId>>
    => "ts/pokerrs.ts",
}
