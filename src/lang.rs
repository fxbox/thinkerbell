
#![allow(unused_variables)]
#![allow(dead_code)]

/// Basic structure of a Monitor (aka Server App, aka wtttttt)
///
/// Monitors are designed so that the FoxBox can offer a simple
/// IFTTT-style Web UX to let users write their own scripts. More
/// complex monitors can installed from the web from a master device
/// (i.e. the user's cellphone or smart tv).

use dependencies::{Bindings, DeviceKind, InputCapability, OutputCapability, Device, Range, Value, Watcher};

use std::collections::HashMap;
use std::sync::{Arc, RwLock}; // FIXME: Investigate if we really need so many instances of Arc. I suspect that most can be replaced by &'a.
use std::sync::mpsc::{channel, Receiver, Sender};
use std::marker::PhantomData;

extern crate chrono;
use self::chrono::{Duration, DateTime, UTC};

extern crate rustc_serialize;
use self::rustc_serialize::json::Json;


///
/// # Definition of the AST
///


/// A Monitor Application, i.e. an application (or a component of an
/// application) executed on the server.
///
/// Monitor applications are typically used for triggering an action
/// in reaction to an event: changing temperature when night falls,
/// ringing an alarm when a door is opened, etc.
///
/// Monitor applications are installed from a paired device. They may
/// either be part of a broader application (which can install them
/// through a web/REST API) or live on their own.
pub struct Script<Ctx, Bind> where Bind: Bindings, Ctx: Context {
    /// Authorization, author, description, update url, version, ...
    metadata: (), // FIXME: Implement

    /// Monitor applications have sets of requirements (e.g. "I need a
    /// camera"), which are allocated to actual resources through the
    /// UX. Re-allocating resources may be requested by the user, the
    /// foxbox, or an application, e.g. when replacing a device or
    /// upgrading the app.
    requirements: Vec<Arc<Requirement<Ctx, Bind>>>,

    /// Resources actually allocated for each requirement.
    allocations: Vec<Resource<Ctx, Bind>>,

    /// A set of rules, stating what must be done in which circumstance.
    rules: Vec<Trigger<Ctx, Bind>>,
}

struct Resource<Ctx, Bind> where Bind: Bindings, Ctx: Context {
    devices: Vec<Bind::Device>,
    phantom: PhantomData<Ctx>,
}


/// A resource needed by this application. Typically, a definition of
/// device with some input our output capabilities.
struct Requirement<Ctx, Bind> where Bind: Bindings, Ctx: Context {
    /// The kind of resource, e.g. "a flashbulb".
    kind: Bind::DeviceKind,

    /// Input capabilities we need from the device, e.g. "the time of
    /// day", "the current temperature".
    inputs: Vec<Bind::InputCapability>,

    /// Output capabilities we need from the device, e.g. "play a
    /// sound", "set luminosity".
    outputs: Vec<Bind::OutputCapability>,
    
    /// Minimal number of resources required. If unspecified in the
    /// script, this is 1.
    min: u32,

    /// Maximal number of resources that may be handled. If
    /// unspecified in the script, this is the same as `min`.
    max: u32,

    phantom: PhantomData<Ctx>,
    // FIXME: We may need cooldown properties.
}

/// A single trigger, i.e. "when some condition becomes true, do
/// something".
struct Trigger<Ctx, Bind> where Bind: Bindings, Ctx: Context {
    /// The condition in which to execute the trigger.
    condition: Conjunction<Ctx, Bind>,

    /// Stuff to do once `condition` is met.
    execute: Vec<Statement<Ctx, Bind>>,

    /// Minimal duration between two executions of the trigger.  If a
    /// duration was not picked by the developer, a reasonable default
    /// duration should be picked (e.g. 10 minutes).
    cooldown: Duration,
}

/// A conjunction (e.g. a "and") of conditions.
struct Conjunction<Ctx, Bind> where Bind: Bindings, Ctx: Context {
    /// The conjunction is true iff all of the following expressions evaluate to true.
    all: Vec<Condition<Ctx, Bind>>,
    state: Ctx::ConditionState,
}

/// An individual condition.
///
/// Conditions always take the form: "data received from sensor is in
/// given range".
///
/// A condition is true if *any* of the sensors allocated to this
/// requirement has yielded a value that is in the given range.
struct Condition<Ctx, Bind> where Bind: Bindings, Ctx: Context {
    input: Ctx::Input, // FIXME: Well, is this a single device or many?
    capability: Bind::InputCapability,
    range: Range,
    state: Ctx::ConditionState,
}


/// Stuff to actually do. In practice, this means placing calls to devices.
struct Statement<Ctx, Bind> where Bind: Bindings, Ctx: Context {
    /// The resource to which this command applies.  e.g. "all
    /// heaters", "a single communication channel", etc.
    destination: Ctx::Output,

    /// The action to execute on the resource.
    action: Bind::OutputCapability,

    /// Data to send to the resource.
    arguments: HashMap<String, Expression<Ctx, Bind>>
}

struct InputSet<Ctx, Bind> where Bind: Bindings, Ctx: Context {
    /// The set of inputs from which to grab the value.
    condition: Condition<Ctx, Bind>,
    /// The value to grab.
    capability: Bind::InputCapability,
}

/// A value that may be sent to an output.
enum Expression<Ctx, Bind> where Bind: Bindings, Ctx: Context {
    /// A dynamic value, which must be read from one or more inputs.
    // FIXME: Not ready yet
    Input(InputSet<Ctx, Bind>),

    /// A constant value.
    Value(Value),

    /// More than a single value.
    Vec(Vec<Expression<Ctx, Bind>>)
}

///
/// # Launching and running the script
///

/// A script ready to be executed.
/// Each script is meant to be executed in an individual thread.
struct ExecutionTask<Ctx, Bind> where Bind: Bindings, Ctx: Context {
    /// The current state of execution the script.
    state: Script<Ctx, Bind>,

    /// Communicating with the thread running script.
    tx: Sender<ExecutionOp>,
    rx: Receiver<ExecutionOp>,
}



/*
trait ExecInput {
}

trait ExecutionBindings: Bindings {
    type ExecInput: ExecInput;
    fn condition_is_met<'a>(&'a mut Self::ConditionState) -> &'a mut bool;

    fn get_inputs<'a>(input: &'a mut <Self as Bindings>::Input) -> &'a mut Vec<<Self as ExecutionBindings>::ExecInput>;
}
*/
struct IsMet {
    old: bool,
    new: bool,
}


enum ExecutionOp {
    /// An input has been updated, time to check if we have triggers
    /// ready to be executed.
    Update,

    /// Time to stop executing the script.
    Stop
}


impl<Ctx, Bind> ExecutionTask<Ctx, Bind> where Bind: Bindings, Ctx: Context {
    /// Create a new execution task.
    ///
    /// The caller is responsible for spawning a new thread and
    /// calling `run()`.
    fn new(script: &Script<UncheckedBind, UncheckedCtx>) -> Self {
        panic!("Not implemented");
/*
        // Prepare the script for execution:
        // - replace instances of Input with InputBind, which map
        //   to a specific device and cache the latest known value
        //   on the input.
        // - replace instances of Output with OutputBind
        let precompiler = Precompiler::new(script);
        let bound = script.rebind(&precompiler);
        
        let (tx, rx) = channel();
        ExecutionTask {
            state: bound,
            rx: rx,
            tx: tx
        }
*/
    }

    /// Get a channel that may be used to send commands to the task.
    fn get_command_sender(&self) -> Sender<ExecutionOp> {
        self.tx.clone()
    }

    /// Execute the monitoring task.
    /// This currently expects to be executed in its own thread.
    fn run(&mut self) {
        panic!("Not implemented");
        /*
        let mut watcher = Bind::Watcher::new();
        let mut witnesses = Vec::new();

        // Start listening to all inputs that appear in conditions.
        // Some inputs may appear only in expressions, so we are
        // not interested in their value.
        for rule in &self.state.rules  {
            for condition in &rule.condition.all {
                for single in &*condition.input {
                    witnesses.push(
                        // We can end up watching several times the
                        // same device + capability + range.  For the
                        // moment, we do not attempt to optimize
                        // either I/O (which we expect will be
                        // optimized by `watcher`) or condition
                        // checking (which we should eventually
                        // optimize, if we find out that we end up
                        // with large rulesets).
                        watcher.add(
                            &single.device,
                            &condition.capability,
                            &condition.range,
                            |value| {
                                // One of the inputs has been updated.
                                *single.state.write().unwrap() = Some(DatedData {
                                    updated: UTC::now(),
                                    data: value
                                });
                                // Note that we can unwrap() safely,
                                // as it fails only if the thread is
                                // already in panic.

                                // Find out if we should execute one of the
                                // statements of the trigger.
                                let _ignored = self.tx.send(ExecutionOp::Update);
                                // If the thread is down, it is ok to ignore messages.
                            }));
                    }
            }
        }

        // FIXME: We are going to end up with stale data in some inputs.
        // We need to find out how to get rid of it.
        // FIXME(2): We now have dates.

        // Now, start handling events.
        for msg in &self.rx {
            use self::ExecutionOp::*;
            match msg {
                Stop => {
                    // Leave the loop.
                    // The watcher and the witnesses will be cleaned up on exit.
                    // Any further message will be ignored.
                    return;
                }

                Update => {
                    // Find out if we should execute triggers.
                    for mut rule in &mut self.state.rules {
                        let is_met = rule.is_met();
                        if !(is_met.new && !is_met.old) {
                            // We should execute the trigger only if
                            // it was false and is now true. Here,
                            // either it was already true or it isn't
                            // false yet.
                            continue;
                        }

                        // Conditions were not met, now they are, so
                        // it is time to start executing.

                        // FIXME: We do not want triggers to be
                        // triggered too often. Handle cooldown.
                        
                        for statement in &rule.execute {
                            // FIXME: Execute
                        }
                    }
                }
            }
        }
*/
    }
}

///
/// # Evaluating conditions
///
/*
impl<Ctx, Bind> Trigger<Ctx, Bind> where Bind: Bindings, Ctx: Context {
    fn is_met(&mut self) -> IsMet {
        self.condition.is_met()
    }
}

impl<Ctx, Bind> Conjunction<Ctx, Bind> where Bind: Bindings, Ctx: Context {
    /// For a conjunction to be true, all its components must be true.
    fn is_met(&mut self) -> IsMet {
        let &mut is_met = Bind::condition_is_met(&mut self.state);
        let old = is_met;
        let mut new = true;

        for mut single in &mut self.all {
            if !single.is_met().new {
                new = false;
                // Don't break. We want to make sure that we update
                // `is_met` of all individual conditions.
            }
        }
        is_met = new;
        IsMet {
            old: old,
            new: new,
        }
    }
}

impl<Ctx, Bind> Condition<Ctx, Bind> where Bind: Bindings, Ctx: Context {
    /// Determine if one of the devices serving as input for this
    /// condition meets the condition.
    fn is_met(&mut self) -> IsMet {
        let &mut is_met = Bind::condition_is_met(&mut self.state);
        let old = is_met;
        let mut new = false;
        for single in Bind::get_inputs(&mut self.input) {
            // This will fail only if the thread has already panicked.
            let state = single.state.read().unwrap();
            let is_met = match *state {
                None => { false /* We haven't received a measurement yet.*/ },
                Some(ref data) => {
                    use dependencies::Range::*;
                    use dependencies::Value::*;

                    match (&data.data, &self.range) {
                        // Any always matches
                        (_, &Any) => true,
                        // Operations on bools and strings
                        (&Bool(ref b), &EqBool(ref b2)) => b == b2,
                        (&String(ref s), &EqString(ref s2)) => s == s2,

                        // Numbers. FIXME: Implement physical units.
                        (&Num(ref x), &Leq(ref max)) => x <= max,
                        (&Num(ref x), &Geq(ref min)) => min <= x,
                        (&Num(ref x), &BetweenEq{ref min, ref max}) => min <= x && x <= max,
                        (&Num(ref x), &OutOfStrict{ref min, ref max}) => x < min || max < x,

                        // Type errors don't match.
                        (&Bool(_), _) => false,
                        (&String(_), _) => false,
                        (_, &EqBool(_)) => false,
                        (_, &EqString(_)) => false,

                        // There is no such thing as a range on json or blob.
                        (&Json(_), _) |
                        (&Blob{..}, _) => false,
                    }
                }
            };
            if is_met {
                new = true;
                break;
            }
        }

        self.state.is_met = new;
        IsMet {
            old: old,
            new: new,
        }
    }
}
*/

/// Rebind a script from an environment to another one.
///
/// This is typically used as a compilation step, to turn code in
/// which device kinds, device allocations, etc. are represented as
/// strings or numbers into code in which they are represented by
/// concrete data structures.
trait Rebinder {
    type SourceCtx: Context;
    type DestCtx: Context;
    type SourceBind: Bindings;
    type DestBind: Bindings;

    // Rebinding the environment
    fn rebind_device(&self, &<<Self as Rebinder>::SourceBind as Bindings>::Device) ->
        <<Self as Rebinder>::DestBind as Bindings>::Device;
    fn rebind_device_kind(&self, &<<Self as Rebinder>::SourceBind as Bindings>::DeviceKind) ->
        <<Self as Rebinder>::DestBind as Bindings>::DeviceKind;
    fn rebind_input_capability(&self, &<<Self as Rebinder>::SourceBind as Bindings>::InputCapability) ->
        <<Self as Rebinder>::DestBind as Bindings>::InputCapability;
    fn rebind_output_capability(&self, &<<Self as Rebinder>::SourceBind as Bindings>::OutputCapability) ->
        <<Self as Rebinder>::DestBind as Bindings>::OutputCapability;

    // Recinding the context
    fn rebind_input(&self, &<<Self as Rebinder>::SourceCtx as Context>::Input) ->
        <<Self as Rebinder>::DestCtx as Context>::Input;

    fn rebind_output(&self, &<<Self as Rebinder>::SourceCtx as Context>::Output) ->
        <<Self as Rebinder>::DestCtx as Context>::Output;

    fn rebind_condition(&self, &<<Self as Rebinder>::SourceCtx as Context>::ConditionState) ->
        <<Self as Rebinder>::DestCtx as Context>::ConditionState;
}

impl<Ctx, Bind> Script<Ctx, Bind> where Bind: Bindings, Ctx: Context {
    fn rebind<R>(&self, rebinder: &R) -> Script<R::DestCtx, R::DestBind>
        where R: Rebinder<SourceBind = Bind, SourceCtx = Ctx>
    {
        let rules = self.rules.iter().map(|ref rule| {
            rule.rebind(rebinder)
        }).collect();

        let allocations = self.allocations.iter().map(|ref res| {
            Resource {
                devices: res.devices.iter().map(|ref device| rebinder.rebind_device(&device)).collect(),
                phantom: PhantomData,
            }
        }).collect();

        let requirements = self.requirements.iter().map(|ref req| {
            Arc::new(Requirement {
                kind: rebinder.rebind_device_kind(&req.kind),
                inputs: req.inputs.iter().map(|ref cap| rebinder.rebind_input_capability(cap)).collect(),
                outputs: req.outputs.iter().map(|ref cap| rebinder.rebind_output_capability(cap)).collect(),
                min: req.min,
                max: req.max,
                phantom: PhantomData,
            })
        }).collect();

        Script {
            metadata: self.metadata.clone(),
            requirements: requirements,
            allocations: allocations,
            rules: rules,
        }
    }
}


impl<Ctx, Bind> Trigger<Ctx, Bind> where Bind: Bindings, Ctx: Context {
    fn rebind<R>(&self, rebinder: &R) -> Trigger<R::DestCtx, R::DestBind>
        where R: Rebinder<SourceBind = Bind, SourceCtx = Ctx>
    {
        let execute = self.execute.iter().map(|ref ex| {
            ex.rebind(rebinder)
        }).collect();
        Trigger {
            cooldown: self.cooldown.clone(),
            execute: execute,
            condition: self.condition.rebind(rebinder),
        }
    }
}

impl<Ctx, Bind> Conjunction<Ctx, Bind> where Bind: Bindings, Ctx: Context {
    fn rebind<R>(&self, rebinder: &R) -> Conjunction<R::DestCtx, R::DestBind>
        where R: Rebinder<SourceBind = Bind, SourceCtx = Ctx>
    {
        Conjunction {
            all: self.all.iter().map(|c| c.rebind(rebinder)).collect(),
            state: rebinder.rebind_condition(&self.state),
        }
    }
}


impl<Ctx, Bind> Condition<Ctx, Bind> where Bind: Bindings, Ctx: Context {
    fn rebind<R>(&self, rebinder: &R) -> Condition<R::DestCtx, R::DestBind>
        where R: Rebinder<SourceBind = Bind, SourceCtx = Ctx>
    {
        Condition {
            range: self.range.clone(),
            capability: rebinder.rebind_input_capability(&self.capability),
            input: rebinder.rebind_input(&self.input),
            state: rebinder.rebind_condition(&self.state),
        }
    }
}


impl<Ctx, Bind> Statement<Ctx, Bind> where Bind: Bindings, Ctx: Context {
    fn rebind<R>(&self, rebinder: &R) -> Statement<R::DestCtx, R::DestBind>
        where R: Rebinder<SourceBind = Bind, SourceCtx = Ctx>
    {
            let arguments = self.arguments.iter().map(|(key, value)| {
                (key.clone(), value.rebind(rebinder))
            }).collect();
            Statement {
                destination: rebinder.rebind_output(&self.destination),
                action: rebinder.rebind_output_capability(&self.action),
                arguments: arguments
            }
        }
}


impl<Ctx, Bind> Expression<Ctx, Bind> where Bind: Bindings, Ctx: Context {
    fn rebind<R>(&self, rebinder: &R) -> Expression<R::DestCtx, R::DestBind>
        where R: Rebinder<SourceBind = Bind, SourceCtx = Ctx>
    {
        use self::Expression::*;
        match *self {
            Value(ref v) => Value(v.clone()),
            Vec(ref v) => Vec(v.iter().map(|x| x.rebind(rebinder)).collect()),
            //            Input(ref input) => Input(rebinder.rebind_input(input).clone()),
            Input(_) => panic!("Not impl implemented yet")
        }
    }
}


///
/// # Precompilation
///
pub trait Context {
    type Input;
    type Output;
    type ConditionState;
}
struct UncheckedCtx;
impl Context for UncheckedCtx {
    type Input = usize;
    type Output = usize;
    type ConditionState = ();
}

struct UncheckedBind;
impl Bindings for UncheckedBind {
    type Device = String;
    type DeviceKind = String;
    type InputCapability = String;
    type OutputCapability = String;
    type Watcher = FakeWatcher;
}

struct CompiledBind<Ctx, Bind> {
    phantom: PhantomData<(Ctx, Bind)>,
}

impl<Ctx, Bind> Bindings for CompiledBind<Ctx, Bind> where Bind: Bindings, Ctx: Context {
    type Device = Bind::Device;
    type DeviceKind = Bind::DeviceKind;
    type InputCapability = Bind::InputCapability;
    type OutputCapability = Bind::OutputCapability;
    type Watcher = Bind::Watcher;
}

struct FakeWatcher;
impl Watcher for FakeWatcher {
    type Witness = ();
    fn new() -> FakeWatcher {
        panic!("Cannot instantiate a FakeWatcher");
    }

    fn add<F>(&mut self,
              device: &Device,
              input: &InputCapability,
              condition: &Range,
              cb: F) -> () where F:FnOnce(Value)
    {
        panic!("Cannot execute a FakeWatcher");
    }
}

/*
impl Bindings for CompiledBind {
    type Input = Vector<Arc<InputBind>>;
    type Output = Vector<Arc<OutputBind>>;
    type ConditionState = ConditionBind;
    type Device = Device;
    type InputCapability = InputCapability;
    type OutputCapability = OutputCapability;
}

impl ExecutionBindings for CompiledBind {
    type Watcher = Box<Watcher>;
    fn condition_is_met<'a>(is_met: &'a mut Self::ConditionState) -> &'a IsMet {
        is_met
    }
}
 */

/// Data, labelled with its latest update.
struct DatedData {
    updated: DateTime<UTC>,
    data: Value,
}

/// A single input device, ready to use, with its latest known state.
struct SingleInputBind {
    device: Device,
    state: RwLock<Option<DatedData>>
}
type InputBind = Vec<SingleInputBind>;

/// A single output device, ready to use.
struct SingleOutputBind {
    device: Device
}
type OutputBind = Vec<SingleOutputBind>;

struct ConditionBind {
    is_met: bool
}

/*
struct Precompiler<'a, CompiledBind> {
    script: &'a Script<UncheckedBind>,
    phantom: PhantomData<CompiledBind>,
}

impl<'a, DestBind> Precompiler<'a, DestBind> where DestBind: Bindings, Ctx: Context {
    fn new(source: &'a Script<UncheckedBind>) -> Self {
        Precompiler {
            script: source,
            phantom: PhantomData
        }
    }
}
*/
/*
impl<'a, DestBind> Rebinder for Precompiler<'a, DestBind>
    where DestBind: Bindings, Ctx: Context {
    type DestBind = DestBind;
    type SourceBind = UncheckedBind;

    fn rebind_input(&self, &input: &usize) ->
    // Must be DestCtx::InputBind
    // Must be parameterized by an actual implementation of Bind
    {
    }
    
    fn rebind_input(&self, &input: &usize) -> DestCtx::InputBind {
        
    }

    // FIXME: Er, what? That's never going to actually share anything!
    fn rebind_input(&self, &input: &usize) -> Arc<DestCtx::InputBind> {
        Arc::new(
            self.script.allocations[input].devices.iter().map(|device| {
                SingleInputBind {
                    device: device.clone(),
                    state: RwLock::new(None),
                }
            }).collect())
    }

    fn rebind_output(&self, &output: &usize) -> Arc<OutputBind> {
        Arc::new(
            self.script.allocations[output].devices.iter().map(|device| {
                SingleOutputBind {
                    device: device.clone()
                }
            }).collect())
    }

    fn rebind_condition(&self, _: &()) -> ConditionBind {
        ConditionBind {
            is_met: false
        }
    }
}
*/
/*
impl Script {
    ///
    /// Start executing the application.
    ///
    pub fn start(&mut self) {
        if self.command_sender.is_some() {
            return;
        }
        let mut task = MonitorTask::new(self.clone());
        self.command_sender = Some(task.get_command_sender());
        thread::spawn(move || {
            task.run();
        });
    }

    ///
    /// Stop the execution of the application.
    ///
    pub fn stop(&mut self) {
        match self.command_sender {
            None => {
                /* Nothing to stop */
                return;
            },
            Some(ref tx) => {
                // Shutdown the application, asynchronously.
                let _ignored = tx.send(MonitorOp::Stop);
                // Do not return.
            }
        }
        self.command_sender = None;
    }
}

*/
