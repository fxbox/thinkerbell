#![feature(custom_derive, plugin)]
#![plugin(serde_macros)]

extern crate docopt;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate lazy_static;

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

const USAGE: &'static str = "
Usage: simulator [options]...
       simulator --help

-h, --help            Show this message.
-r, --ruleset <path>  Load rules from a file.
-o, --output <path>   Load definition of an output device.
";

#[derive(Default, Serialize, Deserialize)]
struct TestEnv;
impl ExecutableDevEnv for TestEnv {
    // Don't bother stopping watches.
    type WatchGuard = ();
    type API = APIImpl;
}

enum Op {
    AddNodes(Vec<Node>),
    AddInputs(Vec<Service<Input>>),
    AddOutputs(Vec<Service<Output>>),
    AddWatch{options: Vec<WatchOptions>, cb: Box<FnMut(WatchEvent) + Send + 'static>},
}

lazy_static! {
    static ref SEND: Mutex<Sender<Op>> = { // FIXME: Find a way to get rid of that Mutex.
        let (tx, rx) = channel();
        thread::spawn(move || {
            let mut api = APIImpl::new();
            for msg in rx.iter() {
                use Op::*;
                match msg {
                    AddNodes(vec) => api.add_nodes(vec).unwrap(),
                    AddInputs(vec) => api.add_inputs(vec).unwrap(),
                    AddOutputs(vec) => api.add_outputs(vec).unwrap(),
                    AddWatch{options, cb} => api.add_watch(options, cb).unwrap()
                }
            }
        });
        Mutex::new(tx)
    };
}

enum Update {
    Put { id: ServiceId, value: Value, result: Result<(), String> },
    Done(String),
}

struct APIImpl {
    nodes: HashMap<NodeId, Node>,
    inputs: HashMap<ServiceId, (Service<Input>, Option<Value>)>,
    outputs: HashMap<ServiceId, Service<Output>>,
    watchers: Vec<(WatchOptions, Arc<Box<FnMut(WatchEvent)>>)>,
    post_updates: Box<Fn(Update)>
}
impl APIImpl {
    fn new<F>(cb: F) -> Self
        where F: Fn(Update) + 'static {
        APIImpl {
            nodes: HashMap::new(),
            inputs: HashMap::new(),
            outputs: HashMap::new(),
            watchers: Vec::new(),
            post_updates: Box::new(cb)
        }
    }
    
    fn add_nodes(&mut self, nodes: Vec<Node>) -> Result<(), ()> {
        for node in nodes {
            let previous = self.nodes.insert(node.id.clone(), node);
            if previous.is_none() {
                return Err(());
            }
        }
        Ok(())
        // FIXME: In a real implementation, this should update all NodeRequest
    }
    fn add_inputs(&mut self, inputs: Vec<Service<Input>>) -> Result<(), ()> {
        for input in inputs {
            let previous = self.inputs.insert(input.id.clone(), (input, None));
            if previous.is_none() {
                return Err(());
            }
        }
        Ok(())
        // FIXME: In a real implementation, this should update all InputRequests
    }
    fn add_outputs(&mut self, outputs: Vec<Service<Output>>) -> Result<(), ()> {
        for output in outputs {
            let previous = self.outputs.insert(output.id.clone(), output);
            if previous.is_none() {
                return Err(());
            }
        }
        Ok(())
        // FIXME: In a real implementation, this should update all OutputRequests
    }

    fn add_watch(&mut self, options: Vec<WatchOptions>, cb: Box<FnMut(WatchEvent)>) -> Result<(), ()> {
        let cb = Arc::new(cb);
        for opt in options {
            self.watchers.push((opt, cb.clone()));
        }
        Ok(())
    }

    fn put_value(&mut self, filters: Vec<OutputRequest>, value: Value) {
        // Very suboptimal implementation.
        let outputs = self.outputs.values().filter(|output| {
            for request in &filters {
                if !request.id.matches(&output.id) {
                    return false;
                }
                if !request.parent.matches(&output.node) {
                    return false;
                }
                if !request.kind.matches(&output.mechanism.kind) {
                    return false;
                }
                if let Some(Period {ref min, ref max}) = request.push {
                    if let Some(ref current) = output.mechanism.push {
                        if let Some(ref min) = *min {
                            if min > current {
                                return false;
                            }
                        }
                        if let Some(ref max) = *max {
                            if max < current {
                                return false;
                            }
                        }
                    } else {
                        return false;
                    }
                }
                for tag in &request.tags {
                    if output.tags.iter().find(|x| *x == tag).is_none() {
                        return false;
                    }
                }
                return true;
            }
            // No filters? Then nothing matches.
            return false;
        });
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

impl API for APIImpl {
    type WatchGuard = ();

    fn get_nodes(_: &Vec<NodeRequest>) -> Vec<Node> {
        unimplemented!()
    }

    fn put_node_tag(_: &Vec<NodeRequest>, _: &Vec<String>) -> usize {
        unimplemented!()
    }

    fn delete_node_tag(_: &Vec<NodeRequest>, _: String) -> usize {
        unimplemented!()
    }

    fn get_input_services(_: &Vec<InputRequest>) -> Vec<Service<Input>> {
        unimplemented!()
    }
    fn get_output_services(_: &Vec<OutputRequest>) -> Vec<Service<Output>> {
        unimplemented!()
    }
    fn put_input_tag(_: &Vec<InputRequest>, _: &Vec<String>) -> usize {
        unimplemented!()
    }
    fn put_output_tag(_: &Vec<OutputRequest>, _: &Vec<String>) -> usize {
        unimplemented!()
    }
    fn delete_input_tag(_: &Vec<InputRequest>, _: &Vec<String>) -> usize {
        unimplemented!()
    }
    fn delete_output_tag(_: &Vec<InputRequest>, _: &Vec<String>) -> usize {
        unimplemented!()
    }
    fn get_service_value(_: &Vec<InputRequest>) -> Vec<(ServiceId, Result<Value, APIError>)> {
        unimplemented!()
    }
    fn put_service_value(_: &Vec<OutputRequest>, _: Value) -> Vec<(ServiceId, Result<(), APIError>)> {
        unimplemented!()
    }
    fn register_service_watch<F>(options: Vec<WatchOptions>, cb: F) -> Self::WatchGuard
        where F: FnMut(WatchEvent) + Send + 'static {
        SEND.lock().unwrap().send(Op::AddWatch {
            options: options,
            cb: Box::new(cb)
        }).unwrap();
        ()
    }

}

/*
/// An implementation of DevEnv for the purpose of unit testing.
struct State {
    states: HashMap</*device*/String, HashMap</*capability*/String, HashMap<String, Value>> >,
    inputs: Vec<(String, String)>,
    outputs: Vec<(String, String)>
}
impl State {
    fn new() -> State {
        State {
            states: HashMap::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }
    fn reset(&mut self) {
        self.states.clear();
    }
}

lazy_static! {
    static ref STATE: Mutex<State> = Mutex::new(State::new());
}

struct TestEnv;

impl TestEnv {
    fn reset() {
        STATE.lock().unwrap().reset();
    }

    fn get_state(device: &String, cap: &String) -> Option<HashMap<String, Value>> {
        println!("[IN] Fetching the state of input device {}, service {}", device, cap);
        let states = &STATE.lock().unwrap().states;
        states.get(device).and_then(|per_device| {
            per_device.get(cap).cloned()
        })
    }

    fn set_state(device: &String, cap: &String, state: HashMap<String, Value>) {
        println!("[OUT] Setting the state of output device {}, service {}", device, cap);
        let mut states = &mut STATE.lock().unwrap().states;
        if !states.contains_key(device) {
            states.insert(device.clone(), HashMap::new());
        }
        let per_device = states.get_mut(device).unwrap();
        per_device.insert(cap.clone(), state);
    }
}

impl DevEnv for TestEnv {
    type DeviceKind = String;
    type Device = String;
    type InputCapability = String;
    type OutputCapability = String;
}

impl ExecutableDevEnv for TestEnv {
        type Watcher = TestWatcher;

    fn get_watcher() -> Self::Watcher {
        Self::Watcher::new()
    }

    fn get_device_kind(key: &String) -> Option<String> {
        // A set of well-known device kinds
        for s in vec!["clock", "display device", "kind 3"] {
            if s == key {
                return Some(key.clone());
            }
        }
        None
    }

    fn get_device(key: &String) -> Option<String> {
        // A set of well-known devices
        println!("Getting device {}", key);
        for s in vec!["built-in clock", "built-in display 1", "built-in display 2"] {
            if s == key {
                return Some(key.clone());
            }
        }
        None
    }

    fn get_input_capability(key: &String) -> Option<String> {
        // A set of well-known inputs
        for s in vec!["ticks", "input 2:string", "input 3: bool"] {
            if s == key {
                println!("Getting input capability {}", key);
                return Some(key.clone());
            }
        }
        None
    }

    fn get_output_capability(key: &String) -> Option<String> {
        for s in vec!["show", "output 2", "output 3"] {
            if s == key {
                println!("Getting output capability {}", key);
                return Some(key.clone());
            }
        }
        None
    }

    fn send(device: &Self::Device, cap: &Self::OutputCapability, value: &HashMap<String, Value>) {
        TestEnv::set_state(device, cap, value.clone());
    }
}

/// A mock watcher that informs clients with new values regularly.

enum TestWatcherMsg {
    Stop,
    Insert((String, String), Box<Fn(Value) + Send>)
}

struct TestWatcher {
    tx: Sender<TestWatcherMsg>,
}

impl TestWatcher {
    fn new() -> Self {
        use TestWatcherMsg::*;
        let (tx, rx) = channel();

        // FIXME: This should be replaced by manual entry on the REPL
        
        thread::spawn(move || {
            let mut watchers = HashMap::new();
            let mut ticks = 0;

            let clock_key = ("built-in clock".to_owned(), "ticks".to_owned());
            loop {
                ticks += 1;
                if let Ok(msg) = rx.try_recv() {
                    match msg {
                        Stop => {
                            return;
                        },
                        Insert(k, cb) => {
                            watchers.insert(k, cb);
                        }
                    }
                } else {
                    thread::sleep(std::time::Duration::new(1, 0));

                    let clock_key = clock_key.clone();
                    let ticks = ticks.clone();
                    match watchers.get(&clock_key) {
                        None => {},
                        Some(ref watcher) => {
                            let val = Value::Duration(Duration::new(ticks, 0));
                            println!("[WATCH] State of device {} service {} has reached {:?}", &clock_key.0, &clock_key.1, &val);
                            watcher(val);
                        }
                    }
                }
            }
        });
        TestWatcher {
            tx: tx,
        }
    }
}

impl Watcher for TestWatcher {
    type Witness = ();
    type Device = String;
    type InputCapability = String;

    fn add<F>(&mut self,
              device: &Self::Device,
              input: &Self::InputCapability,
              _condition: &Range,
              cb: F) -> Self::Witness where F:Fn(Value) + Send + 'static
{
        self.tx.send(TestWatcherMsg::Insert((device.clone(), input.clone()), Box::new(cb))).unwrap();
        ()
    }
}

impl Drop for TestWatcher {
    fn drop(&mut self) {
        self.tx.send(TestWatcherMsg::Stop).unwrap();
    }
}
*/
fn main () {
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
        runner.start(script, move |res| {tx.send(res).unwrap();});
        match rx.recv().unwrap() {
            Starting { result: Ok(()) } => println!("ready."),
            err => panic!("Could not launch script {:?}", err)
        }
        runners.push(runner);
    }

    thread::sleep(std::time::Duration::new(100, 0));
}

