//! Launching and running the script

use ast::{Script, Statement, UncheckedCtx};
use compile::{Compiler, CompiledCtx, ExecutableDevEnv};
use compile;
use values::Range;

use fxbox_taxonomy::api;
use fxbox_taxonomy::api::{API, WatchEvent};
use fxbox_taxonomy::devices::ServiceId;

use std::sync::mpsc::{channel, Receiver, Sender};
use std::marker::PhantomData;
use std::result::Result;
use std::result::Result::*;
use std::thread;
use std::collections::HashMap;

/// Running and controlling a single script.
pub struct Execution<Env> where Env: ExecutableDevEnv + 'static {
    command_sender: Option<Sender<ExecutionOp>>,
    phantom: PhantomData<Env>,
}

impl<Env> Execution<Env> where Env: ExecutableDevEnv + 'static {
    pub fn new() -> Self {
        Execution {
            command_sender: None,
            phantom: PhantomData,
        }
    }

    /// Start executing the script.
    ///
    /// # Errors
    ///
    /// Produces RunningError:AlreadyRunning if the script is already running.
    pub fn start<F>(&mut self, script: Script<UncheckedCtx>, on_result: F) where F: FnOnce(Result<(), Error>) + Send + 'static {
        if self.command_sender.is_some() {
            on_result(Err(Error::RunningError(RunningError::AlreadyRunning)));
            return;
        }
        let (tx, rx) = channel();
        let tx2 = tx.clone();
        self.command_sender = Some(tx);
        thread::spawn(move || {
            match ExecutionTask::<Env>::new(script, tx2, rx) {
                Err(er) => {
                    on_result(Err(er));
                },
                Ok(mut task) => {
                    on_result(Ok(()));
                    task.run();
                }
            }
        });
    }


    /// Stop executing the script, asynchronously.
    ///
    /// # Errors
    ///
    /// Produces RunningError:NotRunning if the script is not running yet.
    pub fn stop<F>(&mut self, on_result: F) where F: Fn(Result<(), Error>) + Send + 'static {
        match self.command_sender {
            None => {
                /* Nothing to stop */
                on_result(Err(Error::RunningError(RunningError::NotRunning)));
            },
            Some(ref tx) => {
                // Shutdown the application, asynchronously.
                let _ignored = tx.send(ExecutionOp::Stop(Box::new(on_result)));
            }
        };
        self.command_sender = None;
    }
}

impl<Env> Drop for Execution<Env> where Env: ExecutableDevEnv + 'static {
    fn drop(&mut self) {
        let _ignored = self.stop(|_ignored| { });
    }
}

/// A script ready to be executed. Each script is meant to be
/// executed in an individual thread.
pub struct ExecutionTask<Env> where Env: ExecutableDevEnv {
    /// The script, annotated with its state.
    state: Script<CompiledCtx<Env>>,

    /// Communicating with the thread running script.
    tx: Sender<ExecutionOp>,
    rx: Receiver<ExecutionOp>,
}





enum ExecutionOp {
    Update { event: WatchEvent, rule_index: usize, condition_index: usize },
    /// Time to stop executing the script.
    Stop(Box<Fn(Result<(), Error>) + Send>)
}


impl<Env> ExecutionTask<Env> where Env: ExecutableDevEnv {
    /// Create a new execution task.
    ///
    /// The caller is responsible for spawning a new thread and
    /// calling `run()`.
    fn new(script: Script<UncheckedCtx>, tx: Sender<ExecutionOp>, rx: Receiver<ExecutionOp>) -> Result<Self, Error> {
        let compiler = try!(Compiler::new().map_err(|err| Error::CompileError(err)));
        let state = try!(compiler.compile(script).map_err(|err| Error::CompileError(err)));
        
        Ok(ExecutionTask {
            state: state,
            rx: rx,
            tx: tx
        })
    }

    /// Execute the monitoring task.
    /// This currently expects to be executed in its own thread.
    fn run(&mut self) {

        let mut witnesses = Vec::new();

        struct ConditionState {
            match_is_met: bool,
            per_input: HashMap<ServiceId, bool>,
            range: Range,
        };
        struct RuleState {
            rule_is_met: bool,
            per_condition: Vec<ConditionState>,
        };

        // Generate the state of rules, conditions, inputs and start
        // listening to changes in the inputs.

        let mut per_rule : Vec<_> = self.state.rules.iter().zip(0 as usize..).map(|(rule, rule_index)| {
            let per_condition = rule.conditions.iter().zip(0 as usize..).map(|(condition, condition_index)| {
                let options: Vec<_> = condition.source.iter().map(|input| {
                    api::WatchOptions::new()
                        .with_inputs(input.clone())
                }).collect();
                // We will often end up watching several times the
                // same service. For the moment, we do not attempt to
                // optimize either I/O (which we expect will be
                // optimized by `watcher`) or condition checking
                // (which we should eventually optimize, if we find
                // out that we end up with large rulesets).

                let tx2 = self.tx.clone();
                witnesses.push(
                    Env::API::register_service_watch(
                        options,
                        move |event| {
                            let _ignored = tx2.send(ExecutionOp::Update {
                                event: event,
                                rule_index: rule_index,
                                condition_index: condition_index,
                            });
                            // We ignore the result. Errors simply
                            // mean that the thread is already down,
                            // in which case we don't care about
                            // messages.
                        }));
                let range = condition.range.clone();
                ConditionState {
                    match_is_met: false,
                    per_input: HashMap::new(),
                    range: range,
                }
            }).collect();

            RuleState {
                rule_is_met: false,
                per_condition: per_condition
            }
        }).collect();

        for msg in self.rx.iter() {
            match msg {
                ExecutionOp::Stop(cb) => {
                    // Leave the loop. Watching will stop once
                    // `witnesses` is dropped.
                    cb(Ok(()));
                    return;
                },
                ExecutionOp::Update {
                    event,
                    rule_index,
                    condition_index,
                } => match event {
                    WatchEvent::InputRemoved(id) => {
                        per_rule[rule_index]
                            .per_condition[condition_index]
                            .per_input
                            .remove(&id);
                    },
                    WatchEvent::InputAdded(id) => {
                        // An input was added. Note that there is
                        // a possibility that the input was not
                        // empty, in case we received messages in
                        // the wrong order.
                        per_rule[rule_index]
                            .per_condition[condition_index]
                            .per_input
                            .insert(id, false);
                    }
                    WatchEvent::Value{from: id, value} => {
                        use std::mem::replace;

                        // An input was updated. Note that there is
                        // a possibility that the input was
                        // empty, in case we received messages in
                        // the wrong order.

                        let input_is_met : bool =
                            per_rule[rule_index]
                            .per_condition[condition_index]
                            .range
                            .contains(&value);

                        per_rule[rule_index]
                            .per_condition[condition_index]
                            .per_input
                            .insert(id, input_is_met); // FIXME: Could be used to optimize

                        // 1. Is the match met?
                        //
                        // The match is met iff any of the inputs
                        // meets the condition.
                        let some_input_is_met = input_is_met ||
                            per_rule[rule_index]
                            .per_condition[condition_index]
                            .per_input
                            .values().find(|is_met| **is_met).is_some();

                        per_rule[rule_index]
                            .per_condition[condition_index]
                            .match_is_met = some_input_is_met;

                        // 2. Is the condition met?
                        //
                        // The condition is met iff all of the
                        // matches are met.
                        let condition_is_met =
                            per_rule[rule_index]
                            .per_condition
                            .iter()
                            .find(|condition_state| condition_state.match_is_met)
                            .is_some();

                        // 3. Are we in a case in which the
                        // condition was not met and is now met?
                        let condition_was_met =
                            replace(&mut per_rule[rule_index].rule_is_met, condition_is_met);

                        if !condition_was_met && condition_is_met {
                            // Ahah, we have just triggered the statements!
                            for statement in &self.state.rules[rule_index].execute {
                                let _ignore = statement.eval(); // FIXME: Report errors
                            }
                        }
                    }
                }
            };
        }
    }
}


impl<Env> Statement<CompiledCtx<Env>> where Env: ExecutableDevEnv {
    fn eval(&self) -> Result<(), Error> {
        let _ignore = Env::API::put_service_value(&self.destination, self.value.clone());
        // FIXME: Report error
        Ok(())
    }
}



#[derive(Debug)]
pub enum RunningError {
    AlreadyRunning,
    NotRunning,
}

#[derive(Debug)]
pub enum Error {
    CompileError(compile::Error),
    RunningError(RunningError),
}

