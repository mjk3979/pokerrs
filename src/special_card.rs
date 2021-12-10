use crate::card::*;

use serde::{Serialize, Deserialize};
use ts_rs::{TS, export};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[derive(TS)]
pub enum SpecialCardType {
    Wild,
    WinsItAll,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[derive(TS)]
pub struct SpecialCard {
    pub wtype: SpecialCardType,
    pub card: Card,
}
