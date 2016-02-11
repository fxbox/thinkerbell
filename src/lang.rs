
#![allow(unused_variables)]
#![allow(dead_code)]

/// Basic structure of a Monitor (aka Server App, aka wtttttt)
///
/// Monitors are designed so that the FoxBox can offer a simple
/// IFTTT-style Web UX to let users write their own scripts. More
/// complex monitors can installed from the web from a master device
/// (i.e. the user's cellphone or smart tv).

use dependencies::{DeviceAccess, DeviceKind, InputCapability, OutputCapability, Device, Range, Value, Watcher};

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
pub struct Script<Ctx, Dev> where Dev: DeviceAccess, Ctx: Context {
    /// Authorization, author, description, update url, version, ...
    metadata: (), // FIXME: Implement

    /// Monitor applications have sets of requirements (e.g. "I need a
    /// camera"), which are allocated to actual resources through the
    /// UX. Re-allocating resources may be requested by the user, the
    /// foxbox, or an application, e.g. when replacing a device or
    /// upgrading the app.
    requirements: Vec<Arc<Requirement<Ctx, Dev>>>,

    /// Resources actually allocated for each requirement.
    allocations: Vec<Resource<Ctx, Dev>>,

    /// A set of rules, stating what must be done in which circumstance.
    rules: Vec<Trigger<Ctx, Dev>>,
}

struct Resource<Ctx, Dev> where Dev: DeviceAccess, Ctx: Context {
    devices: Vec<Dev::Device>,
    phantom: PhantomData<Ctx>,
}


/// A resource needed by this application. Typically, a definition of
/// device with some input our output capabilities.
struct Requirement<Ctx, Dev> where Dev: DeviceAccess, Ctx: Context {
    /// The kind of resource, e.g. "a flashbulb".
    kind: Dev::DeviceKind,

    /// Input capabilities we need from the device, e.g. "the time of
    /// day", "the current temperature".
    inputs: Vec<Dev::InputCapability>,

    /// Output capabilities we need from the device, e.g. "play a
    /// sound", "set luminosity".
    outputs: Vec<Dev::OutputCapability>,
    
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
struct Trigger<Ctx, Dev> where Dev: DeviceAccess, Ctx: Context {
    /// The condition in which to execute the trigger.
    condition: Conjunction<Ctx, Dev>,

    /// Stuff to do once `condition` is met.
    execute: Vec<Statement<Ctx, Dev>>,

    /// Minimal duration between two executions of the trigger.  If a
    /// duration was not picked by the developer, a reasonable default
    /// duration should be picked (e.g. 10 minutes).
    cooldown: Duration,
}

/// A conjunction (e.g. a "and") of conditions.
struct Conjunction<Ctx, Dev> where Dev: DeviceAccess, Ctx: Context {
    /// The conjunction is true iff all of the following expressions evaluate to true.
    all: Vec<Condition<Ctx, Dev>>,
    state: Ctx::ConditionState,
}

/// An individual condition.
///
/// Conditions always take the form: "data received from sensor is in
/// given range".
///
/// A condition is true if *any* of the sensors allocated to this
/// requirement has yielded a value that is in the given range.
struct Condition<Ctx, Dev> where Dev: DeviceAccess, Ctx: Context {
    input: Ctx::InputSet, // FIXME: Well, is this a single device or many?
    capability: Dev::InputCapability,
    range: Range,
    state: Ctx::ConditionState,
}


/// Stuff to actually do. In practice, this means placing calls to devices.
struct Statement<Ctx, Dev> where Dev: DeviceAccess, Ctx: Context {
    /// The resource to which this command applies.  e.g. "all
    /// heaters", "a single communication channel", etc.
    destination: Ctx::OutputSet,

    /// The action to execute on the resource.
    action: Dev::OutputCapability,

    /// Data to send to the resource.
    arguments: HashMap<String, Expression<Ctx, Dev>>
}

struct InputSet<Ctx, Dev> where Dev: DeviceAccess, Ctx: Context {
    /// The set of inputs from which to grab the value.
    condition: Condition<Ctx, Dev>,
    /// The value to grab.
    capability: Dev::InputCapability,
}

/// A value that may be sent to an output.
enum Expression<Ctx, Dev> where Dev: DeviceAccess, Ctx: Context {
    /// A dynamic value, which must be read from one or more inputs.
    // FIXME: Not ready yet
    Input(InputSet<Ctx, Dev>),

    /// A constant value.
    Value(Value),

    /// More than a single value.
    Vec(Vec<Expression<Ctx, Dev>>)
}

/// A manner of representing internal nodes.
pub trait Context {
    /// A representation of one or more input devices.
    type InputSet;

    /// A representation of one or more output devices.
    type OutputSet;

    /// A representation of the current state of a condition.
    type ConditionState;
}

///
/// # Launching and running the script
///

/// A script ready to be executed.
/// Each script is meant to be executed in an individual thread.
struct ExecutionTask<Ctx, Dev> where Dev: DeviceAccess, Ctx: Context {
    /// The current state of execution the script.
    state: Script<Ctx, Dev>,

    /// Communicating with the thread running script.
    tx: Sender<ExecutionOp>,
    rx: Receiver<ExecutionOp>,
}



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


impl<Ctx, Dev> ExecutionTask<Ctx, Dev> where Dev: DeviceAccess, Ctx: Context {
    /// Create a new execution task.
    ///
    /// The caller is responsible for spawning a new thread and
    /// calling `run()`.
    fn new(script: &Script<UncheckedCtx, UncheckedDev>) -> Self {
        panic!("Not implemented");
/*
        // Prepare the script for execution:
        // - replace instances of Input with InputDev, which map
        //   to a specific device and cache the latest known value
        //   on the input.
        // - replace instances of Output with OutputDev
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
        let mut watcher = Dev::Watcher::new();
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
impl<Ctx, Dev> Trigger<Ctx, Dev> where Dev: DeviceAccess, Ctx: Context {
    fn is_met(&mut self) -> IsMet {
        self.condition.is_met()
    }
}

impl<Ctx, Dev> Conjunction<Ctx, Dev> where Dev: DeviceAccess, Ctx: Context {
    /// For a conjunction to be true, all its components must be true.
    fn is_met(&mut self) -> IsMet {
        let &mut is_met = Dev::condition_is_met(&mut self.state);
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

impl<Ctx, Dev> Condition<Ctx, Dev> where Dev: DeviceAccess, Ctx: Context {
    /// Determine if one of the devices serving as input for this
    /// condition meets the condition.
    fn is_met(&mut self) -> IsMet {
        let &mut is_met = Dev::condition_is_met(&mut self.state);
        let old = is_met;
        let mut new = false;
        for single in Dev::get_inputs(&mut self.input) {
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
    type SourceDev: DeviceAccess;
    type DestDev: DeviceAccess;

    // Rebinding the environment
    fn rebind_device(&self, &<<Self as Rebinder>::SourceDev as DeviceAccess>::Device) ->
        <<Self as Rebinder>::DestDev as DeviceAccess>::Device;
    fn rebind_device_kind(&self, &<<Self as Rebinder>::SourceDev as DeviceAccess>::DeviceKind) ->
        <<Self as Rebinder>::DestDev as DeviceAccess>::DeviceKind;
    fn rebind_input_capability(&self, &<<Self as Rebinder>::SourceDev as DeviceAccess>::InputCapability) ->
        <<Self as Rebinder>::DestDev as DeviceAccess>::InputCapability;
    fn rebind_output_capability(&self, &<<Self as Rebinder>::SourceDev as DeviceAccess>::OutputCapability) ->
        <<Self as Rebinder>::DestDev as DeviceAccess>::OutputCapability;

    // Recinding the context
    fn rebind_input(&self, &<<Self as Rebinder>::SourceCtx as Context>::InputSet) ->
        <<Self as Rebinder>::DestCtx as Context>::InputSet;

    fn rebind_output(&self, &<<Self as Rebinder>::SourceCtx as Context>::OutputSet) ->
        <<Self as Rebinder>::DestCtx as Context>::OutputSet;

    fn rebind_condition(&self, &<<Self as Rebinder>::SourceCtx as Context>::ConditionState) ->
        <<Self as Rebinder>::DestCtx as Context>::ConditionState;
}

impl<Ctx, Dev> Script<Ctx, Dev> where Dev: DeviceAccess, Ctx: Context {
    fn rebind<R>(&self, rebinder: &R) -> Script<R::DestCtx, R::DestDev>
        where R: Rebinder<SourceDev = Dev, SourceCtx = Ctx>
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


impl<Ctx, Dev> Trigger<Ctx, Dev> where Dev: DeviceAccess, Ctx: Context {
    fn rebind<R>(&self, rebinder: &R) -> Trigger<R::DestCtx, R::DestDev>
        where R: Rebinder<SourceDev = Dev, SourceCtx = Ctx>
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

impl<Ctx, Dev> Conjunction<Ctx, Dev> where Dev: DeviceAccess, Ctx: Context {
    fn rebind<R>(&self, rebinder: &R) -> Conjunction<R::DestCtx, R::DestDev>
        where R: Rebinder<SourceDev = Dev, SourceCtx = Ctx>
    {
        Conjunction {
            all: self.all.iter().map(|c| c.rebind(rebinder)).collect(),
            state: rebinder.rebind_condition(&self.state),
        }
    }
}


impl<Ctx, Dev> Condition<Ctx, Dev> where Dev: DeviceAccess, Ctx: Context {
    fn rebind<R>(&self, rebinder: &R) -> Condition<R::DestCtx, R::DestDev>
        where R: Rebinder<SourceDev = Dev, SourceCtx = Ctx>
    {
        Condition {
            range: self.range.clone(),
            capability: rebinder.rebind_input_capability(&self.capability),
            input: rebinder.rebind_input(&self.input),
            state: rebinder.rebind_condition(&self.state),
        }
    }
}


impl<Ctx, Dev> Statement<Ctx, Dev> where Dev: DeviceAccess, Ctx: Context {
    fn rebind<R>(&self, rebinder: &R) -> Statement<R::DestCtx, R::DestDev>
        where R: Rebinder<SourceDev = Dev, SourceCtx = Ctx>
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


impl<Ctx, Dev> Expression<Ctx, Dev> where Dev: DeviceAccess, Ctx: Context {
    fn rebind<R>(&self, rebinder: &R) -> Expression<R::DestCtx, R::DestDev>
        where R: Rebinder<SourceDev = Dev, SourceCtx = Ctx>
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

/// A Context used to represent a script that hasn't been compiled
/// yet. Rather than pointing to specific device + capability, inputs
/// and outputs are numbers that are meaningful only in the AST.
struct UncheckedCtx;
impl Context for UncheckedCtx {
    /// In this implementation, each input is represented by its index
    /// in the array of allocations.
    type InputSet = usize;

    /// In this implementation, each output is represented by its
    /// index in the array of allocations.
    type OutputSet = usize;

    /// In this implementation, conditions have no state.
    type ConditionState = ();
}

/// A DeviceAccess used to represent a script that hasn't been
/// compiled yet. Rather than having typed devices, capabilities,
/// etc. everything is represented by a string.
struct UncheckedDev;
impl DeviceAccess for UncheckedDev {
    type Device = String;
    type DeviceKind = String;
    type InputCapability = String;
    type OutputCapability = String;
    type Watcher = FakeWatcher;
}

struct CompiledDev<Ctx, Dev> {
    phantom: PhantomData<(Ctx, Dev)>,
}

impl<Ctx, Dev> DeviceAccess for CompiledDev<Ctx, Dev> where Dev: DeviceAccess, Ctx: Context {
    type Device = Dev::Device;
    type DeviceKind = Dev::DeviceKind;
    type InputCapability = Dev::InputCapability;
    type OutputCapability = Dev::OutputCapability;
    type Watcher = Dev::Watcher;
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
impl DeviceAccess for CompiledDev {
    type Input = Vector<Arc<InputDev>>;
    type Output = Vector<Arc<OutputDev>>;
    type ConditionState = ConditionDev;
    type Device = Device;
    type InputCapability = InputCapability;
    type OutputCapability = OutputCapability;
}

impl ExecutionDeviceAccess for CompiledDev {
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
struct SingleInputDev {
    device: Device,
    state: RwLock<Option<DatedData>>
}
type InputDev = Vec<SingleInputDev>;

/// A single output device, ready to use.
struct SingleOutputDev {
    device: Device
}
type OutputDev = Vec<SingleOutputDev>;

struct ConditionDev {
    is_met: bool
}

struct Precompiler<'a, CompiledDev> {
    script: &'a Script<UncheckedCtx, UncheckedDev>,
    phantom: PhantomData<CompiledDev>,
}

impl<'a, DestDev> Precompiler<'a, DestDev> where DestDev: DeviceAccess {
    fn new(source: &'a Script<UncheckedCtx, UncheckedDev>) -> Self {
        Precompiler {
            script: source,
            phantom: PhantomData
        }
    }
}

/*
impl<'a, DestDev> Rebinder for Precompiler<'a, DestDev>
    where DestDev: DeviceAccess, Ctx: Context {
    type DestDev = DestDev;
    type SourceDev = UncheckedDev;

    fn rebind_input(&self, &input: &usize) ->
    // Must be DestCtx::InputDev
    // Must be parameterized by an actual implementation of Dev
    {
    }
    
    fn rebind_input(&self, &input: &usize) -> DestCtx::InputDev {
        
    }

    // FIXME: Er, what? That's never going to actually share anything!
    fn rebind_input(&self, &input: &usize) -> Arc<DestCtx::InputDev> {
        Arc::new(
            self.script.allocations[input].devices.iter().map(|device| {
                SingleInputDev {
                    device: device.clone(),
                    state: RwLock::new(None),
                }
            }).collect())
    }

    fn rebind_output(&self, &output: &usize) -> Arc<OutputDev> {
        Arc::new(
            self.script.allocations[output].devices.iter().map(|device| {
                SingleOutputDev {
                    device: device.clone()
                }
            }).collect())
    }

    fn rebind_condition(&self, _: &()) -> ConditionDev {
        ConditionDev {
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
