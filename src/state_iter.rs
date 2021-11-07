use crate::game::*;
use crate::card::*;

use std::collections::{HashSet, HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::cmp::Ordering;

pub enum PokerStateChange {
    Input(PlayerResp),
    RoundChange(Round)
}

pub struct PokerTransaction {
    pub state_before: RoundState,
    pub change: PokerStateChange,
    pub state_after: RoundState
}

pub trait PokerNode {
    fn neighbors(&self) -> HashMap<PokerStateChange, Self>;
}

impl PokerNode for HandState {
    fn neighbors(&self) -> HashMap<PokerStateChange, Self> {
        let mut retval = HashMap::new();
        match self.cur_round {
            None => retval
        }
    }
}

pub trait InputIter: Iterator {
    type Item = PlayerResp;
}

pub struct PokerStateIter<T>
    where T: Iterator<Item=PlayerResp> {
    variant: PokerVariant,
    state: HandState,
    inputs: Option<T>,
    state_q: VecDeque<HandState>,
    seen: HashSet<HandState>
}

impl Iterator for PokerStateIter {
    type Item = PokerState;
    fn next(&mut self) -> Option<Self::Item> {

    }
}
