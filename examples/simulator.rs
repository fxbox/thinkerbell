#![feature(custom_derive, plugin)]
#![plugin(serde_macros)]

extern crate docopt;
extern crate serde;
extern crate serde_json;

extern crate fxbox_thinkerbell;
extern crate fxbox_taxonomy;

use fxbox_thinkerbell::compile::ExecutableDevEnv;
use fxbox_thinkerbell::run::{Execution, ExecutionEvent};
use fxbox_thinkerbell::parse::Parser;
use fxbox_thinkerbell::values::Range;
use fxbox_thinkerbell::util::Phantom;

use fxbox_taxonomy::devices::*;
use fxbox_taxonomy::requests::*;
use fxbox_taxonomy::values::*;
use fxbox_taxonomy::api::{API, WatchEvent, WatchOptions};

type APIError = fxbox_taxonomy::api::Error;

use std::io::prelude::*;
use std::fs::File;
use std::sync::Mutex;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::time::Duration;
use std::sync::Arc;

use serde::ser::{Serialize, Serializer};
use serde::de::{Deserialize, Deserializer};
const USAGE: &'static str = "
Usage: simulator [options]...
       simulator --help

-h, --help            Show this message.
-r, --ruleset <path>  Load decision rules from a file.
-d, --devices <path>  Load devices from a file.
-s, --slowdown <num>  Duration of each tick, in ms. Default: 1ms. 
";

#[derive(Default, Serialize, Deserialize)]
struct TestEnv {
    front: APIFrontEnd,
}
impl ExecutableDevEnv for TestEnv {
    // Don't bother stopping watches.
    type WatchGuard = ();
    type API = APIFrontEnd;

    fn api(&self) -> Self::API {
        self.front.clone()
    }
}
impl TestEnv {
    fn new<F>(cb: F) -> Self
        where F: Fn(Update) + Send + 'static {
        TestEnv {
            front: APIFrontEnd::new(cb)
        }
    }
}

enum Op {
    AddNodes(Vec<Node>),
    AddInputs(Vec<Service<Input>>),
    AddOutputs(Vec<Service<Output>>),
    AddWatch{options: Vec<WatchOptions>, cb: Box<Fn(WatchEvent) + Send + 'static>},
}

enum Update {
    Put { id: ServiceId, value: Value, result: Result<(), String> },
    Inject { id: ServiceId, value: Value, result: Result<(), String> },
    Done(String),
}

struct InputWithState {
    input: Service<Input>,
    state: Option<Value>,
}
impl InputWithState {
    fn set_state(&mut self, val: Value) {
        self.state = Some(val);
    }
}

struct APIBackEnd {
    nodes: HashMap<NodeId, Node>,
    inputs: HashMap<ServiceId, InputWithState>,
    outputs: HashMap<ServiceId, Service<Output>>,
    watchers: Vec<(WatchOptions, Arc<Box<Fn(WatchEvent)>>)>,
    post_updates: Arc<Fn(Update)>
}
impl APIBackEnd {
    fn new<F>(cb: F) -> Self
        where F: Fn(Update) + Send + 'static {
        APIBackEnd {
            nodes: HashMap::new(),
            inputs: HashMap::new(),
            outputs: HashMap::new(),
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
        // In a real implementation, this should update all NodeRequest
    }
    fn add_inputs(&mut self, inputs: Vec<Service<Input>>) {
        for input in inputs {
            let previous = self.inputs.insert(
                input.id.clone(),
                InputWithState {
                    input:input,
                    state: None
                });
            assert!(previous.is_none());
        }
        // In a real implementation, this should update all InputRequests
    }
    fn add_outputs(&mut self, outputs: Vec<Service<Output>>)  {
        for output in outputs {
            let previous = self.outputs.insert(output.id.clone(), output);
            assert!(previous.is_none());
        }
        // In a real implementation, this should update all OutputRequests
    }

    fn add_watch(&mut self, options: Vec<WatchOptions>, cb: Box<Fn(WatchEvent)>) {
        let cb = Arc::new(cb);
        for opt in options {
            self.watchers.push((opt, cb.clone()));
        }
    }

    fn inject_input_value(&mut self, id: ServiceId, value: Value) {
        let mut input = self.inputs.get_mut(&id).unwrap();
        input.set_state(value.clone());

        // The list of watchers watching for new values on this input.
        let watchers = self.watchers.iter().filter(|&&(ref options, _)| {
            options.should_watch_values &&
                options.source.matches(&input.input)
        });
        for mut watcher in watchers {
            watcher.1(WatchEvent::Value {
                from: id.clone(),
                value: value.clone()
            });
        }
    }

    fn put_value(&mut self, requests: Vec<OutputRequest>, value: Value) {
        // Very suboptimal implementation.
        let outputs = self.outputs.values().filter(|output|
            requests.iter().find(|request| request.matches(output)).is_some());
        for output in outputs {
            let result =
                if value.get_type() == output.mechanism.kind.get_type() {
                    Ok(())
                } else {
                    Err(format!("Invalid type, expected {:?}, got {:?}", value.get_type(), output.mechanism.kind.get_type()))
                };
            (*self.post_updates)(Update::Put {
                id: output.id.clone(),
                value: value.clone(),
                result: result
            });
        }
        (*self.post_updates)(Update::Done("put".to_owned()));
    }
}

#[derive(Clone)]
struct APIFrontEnd {
    // By definition, the cell is never empty
    tx: Sender<Op>
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
    fn new<F>(cb: F) -> Self
        where F: Fn(Update) + Send + 'static {
        let (tx, rx) = channel();
        thread::spawn(move || {
            let mut api = APIBackEnd::new(cb);
            for msg in rx.iter() {
                use Op::*;
                match msg {
                    AddNodes(vec) => api.add_nodes(vec),
                    AddInputs(vec) => api.add_inputs(vec),
                    AddOutputs(vec) => api.add_outputs(vec),
                    AddWatch{options, cb} => api.add_watch(options, cb),
                }
            }
        });
        APIFrontEnd {
            tx: tx
        }
    }
}

impl API for APIFrontEnd {
    type WatchGuard = ();

    fn get_nodes(&self, _: &Vec<NodeRequest>) -> Vec<Node> {
        unimplemented!()
    }

    fn put_node_tag(&self, _: &Vec<NodeRequest>, _: &Vec<String>) -> usize {
        unimplemented!()
    }

    fn delete_node_tag(&self, _: &Vec<NodeRequest>, _: String) -> usize {
        unimplemented!()
    }

    fn get_input_services(&self, _: &Vec<InputRequest>) -> Vec<Service<Input>> {
        unimplemented!()
    }
    fn get_output_services(&self, _: &Vec<OutputRequest>) -> Vec<Service<Output>> {
        unimplemented!()
    }
    fn put_input_tag(&self, _: &Vec<InputRequest>, _: &Vec<String>) -> usize {
        unimplemented!()
    }
    fn put_output_tag(&self, _: &Vec<OutputRequest>, _: &Vec<String>) -> usize {
        unimplemented!()
    }
    fn delete_input_tag(&self, _: &Vec<InputRequest>, _: &Vec<String>) -> usize {
        unimplemented!()
    }
    fn delete_output_tag(&self, _: &Vec<InputRequest>, _: &Vec<String>) -> usize {
        unimplemented!()
    }
    fn get_service_value(&self, _: &Vec<InputRequest>) -> Vec<(ServiceId, Result<Value, APIError>)> {
        unimplemented!()
    }
    fn put_service_value(&self, _: &Vec<OutputRequest>, _: Value) -> Vec<(ServiceId, Result<(), APIError>)> {
        unimplemented!()
    }
    fn register_service_watch(&self, options: Vec<WatchOptions>, cb: Box<Fn(WatchEvent) + Send + 'static>) -> Self::WatchGuard {
        self.tx.send(Op::AddWatch {
            options: options,
            cb: cb
        }).unwrap();
        ()
    }

}
fn main () {
    let env = TestEnv::new(|_|{});

    use fxbox_thinkerbell::run:: ExecutionEvent::*;
    let args = docopt::Docopt::new(USAGE)
        .and_then(|d| d.argv(std::env::args().into_iter()).parse())
        .unwrap_or_else(|e| e.exit());

    let mut runners = Vec::new();

    println!("Preparing simulator");

    for path in args.get_vec("--ruleset") {
        print!("Loading ruleset from {}... ", path);
        let mut file = File::open(path).unwrap();
        let mut source = String::new();
        file.read_to_string(&mut source).unwrap();
        let script = Parser::parse(source).unwrap();
        print!("launching... ");

        let mut runner = Execution::<TestEnv>::new();
        let (tx, rx) = channel();
        runner.start(env.api(), script, move |res| {tx.send(res).unwrap();});
        match rx.recv().unwrap() {
            Starting { result: Ok(()) } => println!("ready."),
            err => panic!("Could not launch script {:?}", err)
        }
        runners.push(runner);
    }

    thread::sleep(std::time::Duration::new(100, 0));
}

