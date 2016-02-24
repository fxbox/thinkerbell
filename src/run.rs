//! Launching and running the script

use ast::{Script, Rule, Statement, Match, UncheckedCtx};
use compile::{Compiler, CompiledCtx, ExecutableDevEnv};
use compile;

use fxbox_taxonomy::values::*;
use fxbox_taxonomy::api;
use fxbox_taxonomy::api::{API, WatchEvent};

use std::sync::mpsc::{channel, Receiver, Sender};
use std::marker::PhantomData;
use std::result::Result;
use std::result::Result::*;
use std::thread;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use chrono::UTC;

fn test<F>(cb: F) where F: FnMut(WatchEvent) + Send + Sync {
    unimplemented!()
}


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
    /// An input has been updated, time to check if we have triggers
    /// ready to be executed.
    Update {index: usize, value: Value},

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

        // Start listening to all inputs that appear in conditions.
        for rule in &self.state.rules  {

            let mut rule_is_met = false; // FIXME: Why does this build? Shouldn't I protect `rule_is_met` by my mutex?

            // A Arc<Mutex<_>> is required here as we spawn several
            // Send + Sync closures, which could in theory be executed
            // each on its thread.
            //
            // FIXME: Find a way to specify that all the closures are
            // executed on the same thread, to be rid of the Mutex?
            let conditions_are_met = Arc::new(Mutex::new({
                let mut vec = Vec::with_capacity(rule.conditions.len());
                vec.resize(rule.conditions.len(), false);
                vec
            }));

            for (condition, index) in rule.conditions.iter().zip(0 as usize..) {
                let conditions_are_met = conditions_are_met.clone();
                // A mapping from `ServiceId` to `true` (the condition
                // is met for this service) or `false` (the condition
                // isn't met for this service).
                let mut inputs_are_met = HashMap::new();

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
                witnesses.push(
                    Env::API::register_service_watch(
                        options,
                        move |event| {
                            match event {
                                WatchEvent::InputRemoved(id) => {
                                    inputs_are_met.remove(&id);
                                },
                                WatchEvent::InputAdded(id) => {
                                    // An input was added. Note that there is
                                    // a possibility that the input was not
                                    // empty, in case we received messages in
                                    // the wrong order.
                                    inputs_are_met.insert(id, false);
                                }
                                WatchEvent::Value{from: id, value} => {
                                    use std::mem::replace;

                                    // An input was updated. Note that there is
                                    // a possibility that the input was
                                    // empty, in case we received messages in
                                    // the wrong order.
                                    let input_is_met = condition.range.contains(&value);
                                    inputs_are_met.insert(id, input_is_met); // FIXME: Could be used to optimize

                                    // 1. Is there at least one input_is_met = true in this condition?
                                    let some_input_is_met = input_is_met ||
                                        inputs_are_met.values().find(|is_met| **is_met).is_some();
                                    let mut conditions_are_met = conditions_are_met.lock().unwrap();
                                    // This can fail only if one of the threads has already panicked, that's ok with us.
                                    conditions_are_met[index] = some_input_is_met;

                                    // 2. Are all conditions in this rule met?
                                    let all_conditions_are_met = conditions_are_met.iter().find(|is_met| **is_met).is_some();

                                    // Release the lock as early as possible.
                                    drop(conditions_are_met);

                                    // 3. Is the rule already met?
                                    // FIXME: Why does this build?
                                    let rule_was_met = replace(&mut rule_is_met, all_conditions_are_met);

                                    if !rule_was_met && all_conditions_are_met {
                                        // Ahah, we have just triggered the statements!
                                        for statement in &rule.execute {
                                            let _ignore = statement.eval(); // FIXME: Report errors
                                        }
                                    }
                                }
                            }
                        }));
                }
        }

        loop {
            // FIXME: Receive
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

