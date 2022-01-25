#![allow(unused_imports)]

use pokerrs::card::*;
use pokerrs::game::*;
use pokerrs::gamestate::*;
use pokerrs::cmdline::*;
use pokerrs::table::*;
use pokerrs::cmdline::*;
use pokerrs::bot::*;
use pokerrs::bot_easy::*;
use pokerrs::bot_always_call::*;
use pokerrs::fold_channel;
use rand::prelude::*;

use tokio::sync::broadcast;
use std::sync::{Arc, Mutex};

use std::io::{self, StdinLock, Stdin};

#[tokio::main]
async fn main() -> Result<(), PokerRoundError> {
    let mut player = CmdlineInputSource::new();
    let mut bot = BotInputSource::new(Arc::new(BotEasy::new()));
    let mut players = vec![(0, LivePlayer {
            player_id: "player".to_string(),
            chips: 20,
            input: Arc::new(player)
        }),
        (1, LivePlayer {
            player_id: "bot".to_string(),
            chips: 20,
            input: Arc::new(bot)
        })
    ].into_iter().collect();
    let table_rules = TableRules {
        ante: AnteRule::Blinds(vec![Blind{amount: 1}, Blind{amount: 2}]),
        ante_name: "ante".to_string(),
        min_bet: 1
    };
    let variant = seven_card_stud();
    let mut deck = Box::new(standard_deck());
    let mut rng = rand::thread_rng();
    deck.shuffle(&mut rng);
    //let (tx, rx) = fold_channel::channel(1, (), |_, _1| {});
    let winners = play_poker(variant,
        Mutex::new(deck),
        players,
        None,
        table_rules,
        vec![],
        0
    ).await?;
    println!("!!! Winners !!!\n{:#?}", winners);

    Ok(())
}
