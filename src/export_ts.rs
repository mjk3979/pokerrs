use crate::card::*;
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
    PokerViewState,
    TableConfig,
    TableViewState,
    BetDiffKind,
    PokerVariantDesc,
    PokerVariants,
    PokerViewDiff<PlayerId>,
    CardState,
    HandStrength,
    Subpot<PlayerId>,
    Winners<PlayerId>,
    Seat,
    Suit,
    Card,
    Kind,
    ServerActionRequest,
    ServerPlayer,
    ServerUpdate
    => "ts/pokerrs.ts",
}
