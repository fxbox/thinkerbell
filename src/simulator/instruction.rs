#![cfg_attr(not(test), allow(dead_code))]

extern crate foxbox_taxonomy;

use foxbox_taxonomy::util::Id;
use foxbox_taxonomy::values::*;
use foxbox_taxonomy::devices::*;
use foxbox_taxonomy::selector::*;
use foxbox_taxonomy::api::{WatchEvent, WatchOptions, Error};

#[derive(Serialize, Deserialize, Debug)]
/// Instructions given to the simulator.
pub enum Instruction {
    AddNodes(Vec<Node>),
    AddGetters(Vec<Channel<Getter>>),
    AddSetters(Vec<Channel<Setter>>),
    InjectGetterValue{id: Id<Getter>, value: Value},
}

/// Operations internal to the simulator.
pub enum Op {
    AddNodes(Vec<Node>),
    AddGetters(Vec<Channel<Getter>>),
    AddSetters(Vec<Channel<Setter>>),
    AddWatch{options: Vec<WatchOptions>, cb: Box<Fn(WatchEvent) + Send + 'static>},
    SendValue{selectors: Vec<SetterSelector>, value: Value, cb: Box<Fn(Vec<(Id<Setter>, Result<(), Error>)>) + Send>},
    InjectGetterValue{id: Id<Getter>, value: Value},
}

impl Instruction {
    pub fn as_op(self) -> Op {
        use self::Instruction::*;
        match self {
            AddNodes(vec) => Op::AddNodes(vec),
            AddGetters(vec) => Op::AddGetters(vec),
            AddSetters(vec) => Op::AddSetters(vec),
            InjectGetterValue{id, value} => Op::InjectGetterValue{id:id, value: value}
        }
    }
}
