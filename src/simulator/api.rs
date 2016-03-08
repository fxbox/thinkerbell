#![cfg_attr(not(test), allow(dead_code))]

extern crate serde;
extern crate serde_json;

extern crate foxbox_taxonomy;

use foxbox_taxonomy::devices::*;
use foxbox_taxonomy::selector::*;
use foxbox_taxonomy::values::*;
use foxbox_taxonomy::api::{API, WatchEvent, WatchOptions, Error};
use foxbox_taxonomy::util::Id;

use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::sync::Arc;

use serde::ser::{Serialize, Serializer};
use serde::de::{Deserialize, Deserializer};

use simulator::instruction::Op;


#[derive(Debug)]
pub enum Update {
    Put { id: Id<Setter>, value: Value, result: Result<(), String> },
    Done,
}

#[derive(Debug)]
struct GetterWithState {
    getter: Channel<Getter>,
    state: Option<Value>,
}
impl GetterWithState {
    fn set_state(&mut self, val: Value) {
        self.state = Some(val);
    }
}

struct APIBackEnd {
    nodes: HashMap<Id<NodeId>, Node>,
    getters: HashMap<Id<Getter>, GetterWithState>,
    setters: HashMap<Id<Setter>, Channel<Setter>>,
    watchers: Vec<(WatchOptions, Arc<Box<Fn(WatchEvent)>>)>,
    post_updates: Arc<Fn(Update)>
}
impl APIBackEnd {
    fn new<F>(cb: F) -> Self
        where F: Fn(Update) + Send + 'static {
        APIBackEnd {
            nodes: HashMap::new(),
            getters: HashMap::new(),
            setters: HashMap::new(),
            watchers: Vec::new(),
            post_updates: Arc::new(cb)
        }
    }

    fn add_nodes(&mut self, nodes: Vec<Node>) {
        for node in nodes {
            let previous = self.nodes.insert(node.id.clone(), node);
            if previous.is_some() {
                assert!(previous.is_none());
            }
        }
        // In a real implementation, this should update all NodeSelector
    }
    fn add_getters(&mut self, getters: Vec<Channel<Getter>>) {
        for getter in getters {
            let previous = self.getters.insert(
                getter.id.clone(),
                GetterWithState {
                    getter:getter,
                    state: None
                });
            assert!(previous.is_none());
        }
        // In a real implementation, this should update all GetterSelectors
    }
    fn add_setters(&mut self, setters: Vec<Channel<Setter>>)  {
        for setter in setters {
            let previous = self.setters.insert(setter.id.clone(), setter);
            assert!(previous.is_none());
        }
        // In a real implementation, this should update all SetterSelectors
    }

    fn add_watch(&mut self, options: Vec<WatchOptions>, cb: Box<Fn(WatchEvent)>) {
        let cb = Arc::new(cb);
        for opt in options {
            self.watchers.push((opt, cb.clone()));
        }
    }

    fn inject_getter_value(&mut self, id: Id<Getter>, value: Value) {
        let mut getter = self.getters.get_mut(&id).unwrap();
        getter.set_state(value.clone());

        // The list of watchers watching for new values on this getter.
        let watchers = self.watchers.iter().filter(|&&(ref options, _)| {
            options.should_watch_values &&
                options.source.matches(&getter.getter)
        });
        for watcher in watchers {
            watcher.1(WatchEvent::Value {
                from: id.clone(),
                value: value.clone()
            });
        }
    }

    fn put_value(&mut self,
                 selectors: Vec<SetterSelector>,
                 value: Value,
                 cb: Box<Fn(Vec<(Id<Setter>, Result<(), Error>)>)>)
    {
        // Very suboptimal implementation.
        let setters = self.setters
            .values()
            .filter(|setter|
                    selectors.iter()
                    .find(|selector| selector.matches(setter))
                    .is_some());
        let results = setters.map(|setter| {
            let result;
            let internal_result;
            if value.get_type() == setter.mechanism.kind.get_type() {
                result = Ok(());
                internal_result = Ok(());
            } else {
                result = Err(foxbox_taxonomy::api::Error::TypeError);
                internal_result = Err(format!("Invalid type, expected {:?}, got {:?}", value.get_type(), setter.mechanism.kind.get_type()));
            }
            (*self.post_updates)(Update::Put {
                id: setter.id.clone(),
                value: value.clone(),
                result: internal_result
            });
            (setter.id.clone(), result)
        }).collect();
        cb(results)
    }
}

#[derive(Clone)]
pub struct APIFrontEnd {
    // By definition, the cell is never empty
    pub tx: Sender<Op>
}
impl Serialize for APIFrontEnd {
    fn serialize<S>(&self, _: &mut S) -> Result<(), S::Error> where S: Serializer {
        panic!("WTF are we doing serializing the front-end?");
    }
}
impl Deserialize for APIFrontEnd {
    fn deserialize<D>(_: &mut D) -> Result<Self, D::Error> where D: Deserializer {
        panic!("WTF are we doing deserializing the front-end?");
    }
}
impl Default for APIFrontEnd {
    fn default() -> Self {
        panic!("WTF are we doing calling default() for the front-end?");
    }
}

impl APIFrontEnd {
    pub fn new<F>(cb: F) -> Self
        where F: Fn(Update) + Send + 'static {
        let (tx, rx) = channel();
        thread::spawn(move || {
            let mut api = APIBackEnd::new(cb);
            for msg in rx.iter() {
                use simulator::instruction::Op::*;
                match msg {
                    AddNodes(vec) => api.add_nodes(vec),
                    AddGetters(vec) => api.add_getters(vec),
                    AddSetters(vec) => api.add_setters(vec),
                    AddWatch{options, cb} => api.add_watch(options, cb),
                    SendValue{selectors, value, cb} => api.put_value(selectors, value, cb),
                    InjectGetterValue{id, value} => api.inject_getter_value(id, value),
                }
                (*api.post_updates)(Update::Done)
            }
        });
        APIFrontEnd {
            tx: tx
        }
    }
}

impl API for APIFrontEnd {
    type WatchGuard = ();

    fn get_nodes(&self, _: &Vec<NodeSelector>) -> Vec<Node> {
        unimplemented!()
    }

    fn put_node_tag(&self, _: &Vec<NodeSelector>, _: &Vec<String>) -> usize {
        unimplemented!()
    }

    fn delete_node_tag(&self, _: &Vec<NodeSelector>, _: String) -> usize {
        unimplemented!()
    }

    fn get_getter_channels(&self, _: &Vec<GetterSelector>) -> Vec<Channel<Getter>> {
        unimplemented!()
    }
    fn get_setter_channels(&self, _: &Vec<SetterSelector>) -> Vec<Channel<Setter>> {
        unimplemented!()
    }
    fn put_getter_tag(&self, _: &Vec<GetterSelector>, _: &Vec<String>) -> usize {
        unimplemented!()
    }
    fn put_setter_tag(&self, _: &Vec<SetterSelector>, _: &Vec<String>) -> usize {
        unimplemented!()
    }
    fn delete_getter_tag(&self, _: &Vec<GetterSelector>, _: &Vec<String>) -> usize {
        unimplemented!()
    }
    fn delete_setter_tag(&self, _: &Vec<SetterSelector>, _: &Vec<String>) -> usize {
        unimplemented!()
    }
    fn get_channel_value(&self, _: &Vec<GetterSelector>) -> Vec<(Id<Getter>, Result<Value, Error>)> {
        unimplemented!()
    }
    fn put_channel_value(&self, selectors: &Vec<SetterSelector>, value: Value) -> Vec<(Id<Setter>, Result<(), Error>)> {
        let (tx, rx) = channel();
        self.tx.send(Op::SendValue {
            selectors: selectors.clone(),
            value: value,
            cb: Box::new(move |result| { tx.send(result).unwrap(); })
        }).unwrap();
        rx.recv().unwrap()
    }
    fn register_channel_watch(&self, options: Vec<WatchOptions>, cb: Box<Fn(WatchEvent) + Send + 'static>) -> Self::WatchGuard {
        self.tx.send(Op::AddWatch {
            options: options,
            cb: cb
        }).unwrap();
        ()
    }
}
