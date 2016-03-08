#![feature(custom_derive, plugin)]
#![plugin(serde_macros)]

extern crate docopt;
extern crate serde;
extern crate serde_json;

extern crate foxbox_thinkerbell;
extern crate foxbox_taxonomy;

use foxbox_thinkerbell::compile::ExecutableDevEnv;
use foxbox_thinkerbell::run::Execution;
use foxbox_thinkerbell::parse::Parser;
use foxbox_thinkerbell::simulator::api::{APIFrontEnd, Update};
use foxbox_thinkerbell::simulator::instruction::Instruction;

use std::io::prelude::*;
use std::fs::File;
use std::thread;
use std::time::Duration;
use std::str::FromStr;
use std::sync::mpsc::channel;

const USAGE: &'static str = "
Usage: simulator [options]...
       simulator --help

-h, --help            Show this message.
-r, --ruleset <path>  Load decision rules from a file.
-e, --events <path>   Load events from a file.
-s, --slowdown <num>  Duration of each tick, in floating point seconds. Default: no slowdown.
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
    pub fn new<F>(cb: F) -> Self
        where F: Fn(Update) + Send + 'static {
        TestEnv {
            front: APIFrontEnd::new(cb)
        }
    }

    pub fn execute(&self, instruction: Instruction) {
        self.front.tx.send(instruction.as_op()).unwrap();
    }
}


fn main () {
    use foxbox_thinkerbell::run::ExecutionEvent::*;

    println!("Preparing simulator.");
    let (tx, rx) = channel();
    let env = TestEnv::new(move |event| {
        let _ = tx.send(event);
    });
    let (tx_done, rx_done) = channel();
    thread::spawn(move || {
        for event in rx.iter() {
            match event {
                Update::Done => {
                    let _ = tx_done.send(()).unwrap();
                },
                event => println!("<<< {:?}", event)
            }
        }
    });

    let args = docopt::Docopt::new(USAGE)
        .and_then(|d| d.argv(std::env::args().into_iter()).parse())
        .unwrap_or_else(|e| e.exit());

    let slowdown = match args.find("--slowdown") {
        None => Duration::new(0, 0),
        Some(value) => {
            let vec = value.as_vec();
            if vec.is_empty() || vec[0].is_empty() {
                Duration::new(0, 0)
            } else {
                let s = f64::from_str(vec[0]).unwrap();
                Duration::new(s as u64, (s.fract() * 1_000_000.0) as u32)
            }
        }
    };

    let mut runners = Vec::new();

    println!("Loading rulesets.");
    for path in args.get_vec("--ruleset") {
        print!("Loading ruleset from {}\n", path);
        let mut file = File::open(path).unwrap();
        let mut source = String::new();
        file.read_to_string(&mut source).unwrap();
        let script = Parser::parse(source).unwrap();
        print!("Ruleset loaded, launching... ");

        let mut runner = Execution::<TestEnv>::new();
        let (tx, rx) = channel();
        runner.start(env.api(), script, move |res| {
            let _ = tx.send(res);
        });
        match rx.recv().unwrap() {
            Starting { result: Ok(()) } => println!("ready."),
            err => panic!("Could not launch script {:?}", err)
        }
        runners.push(runner);
    }

    println!("Loading sequences of events.");
    for path in args.get_vec("--events") {
        println!("Loading events from {}...", path);
        let mut file = File::open(path).unwrap();
        let mut source = String::new();
        file.read_to_string(&mut source).unwrap();
        let script : Vec<Instruction> = serde_json::from_str(&source).unwrap();
        println!("Sequence of events loaded, playing...");

        for event in script {
            thread::sleep(slowdown.clone());
            println!(">>> {:?}", event);
            env.execute(event);
            rx_done.recv().unwrap();
        }
    }

    println!("Simulation complete.");
    thread::sleep(Duration::new(100, 0));
}
