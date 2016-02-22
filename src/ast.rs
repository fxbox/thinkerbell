/// Basic structure of a Monitor (aka Server App)
///
/// Monitors are designed so that the FoxBox can offer a simple
/// IFTTT-style Web UX to let users write their own scripts. More
/// complex monitors can installed from the web from a master device
/// (i.e. the user's cellphone or smart tv).

use dependencies::DevEnv;
use values::Range;

extern crate fxbox_taxonomy;
use self::fxbox_taxonomy::values::Value;
use self::fxbox_taxonomy::devices::*;
use self::fxbox_taxonomy::requests::*;

use std::collections::HashMap;
use std::marker::PhantomData;


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
pub struct Script<Ctx, Env> where Env: DevEnv, Ctx: Context {
    /// Authorization, author, description, update url, version, ...
    pub metadata: (), // FIXME: Implement

    /// The list of input services used by this script. During
    /// compilation, we make sure that each resource appears only
    /// once. // FIXME: Implement
    pub inputs: Ctx::Inputs,

    /// The list of output services used by this script. During
    /// compilation, we make sure that each resource appears only
    /// once. // FIXME: Implement
    pub outputs: Ctx::Outputs,

    /// A set of rules, stating what must be done in which circumstance.
    pub rules: Vec<Trigger<Ctx, Env>>,
}

/// A set of similar input services used together to provide a single
/// piece of information. For instance, a set of fire detectors.
///
/// All input services grouped as a resource must provide the same
/// service.
pub struct Resource<IO, Ctx, Env> where Env: DevEnv, Ctx: Context {
    /// The kind of service provided by this resource. During
    /// compilation, we make sure that each resource provides this
    /// service. // FIXME: Implement
    pub kind: Ctx::ServiceKind,

    /// The actual list of endpoints. Must be non-empty. During
    /// compilation, we make sure that each resource appears only
    /// once. // FIXME: Implement.
    pub services: Vec<ServiceId>,

    pub phantom: PhantomData<(IO, Ctx, Env)>,
}

/// A single trigger, i.e. "when some condition becomes true, do
/// something".
pub struct Trigger<Ctx, Env> where Env: DevEnv, Ctx: Context {
    /// The condition in which to execute the trigger.
    pub condition: Conjunction<Ctx, Env>,

    /// Stuff to do once `condition` is met.
    pub execute: Vec<Statement<Ctx, Env>>,
}

/// A conjunction (e.g. a "and") of conditions.
pub struct Conjunction<Ctx, Env> where Env: DevEnv, Ctx: Context {
    /// The conjunction is true iff all of the following expressions evaluate to true.
    pub all: Vec<Condition<Ctx, Env>>,
    pub state: Ctx::ConditionState,
}

/// An individual condition.
///
/// Conditions always take the form: "data received from input service
/// is in given range".
///
/// A condition is true if *any* of the corresponding input services
/// yielded a value that is in the given range.
pub struct Condition<Ctx, Env> where Env: DevEnv, Ctx: Context {
    /// The set of inputs to watch. Note that the set of inputs may
    /// change without rebooting the script.
    pub input: InputRequest,

    /// The range of values for which the condition is considered met.
    /// During compilation, we check that the type of `range` is
    /// compatible with that of `input`. // FIXME: Implement
    pub range: Range,
    pub state: Ctx::ConditionState,
}


/// Stuff to actually do. In practice, this means placing calls to devices.
pub struct Statement<Ctx, Env> where Env: DevEnv, Ctx: Context {
    /// The resource to which this command applies.
    pub destination: Ctx::Output,

    /// Data to send to the resource. During compilation, we check
    /// that the type of `value` is compatible with that of
    /// `destination`. // FIXME: Implement
    pub value: Expression<Ctx, Env>
}

/// A value that may be sent to an output.
pub enum Expression<Ctx, Env> where Env: DevEnv, Ctx: Context {
    /// A constant value.
    Value(Value),
}

/// A manner of representing internal nodes.
pub trait Context {
    /// A representation of the current state of a condition.
    type ConditionState;

    type Inputs;
    type Outputs;

    type ServiceKind;
}

/// A Context used to represent a script that hasn't been compiled
/// yet.
pub struct UncheckedCtx;
impl Context for UncheckedCtx {
    /// In this implementation, conditions have no state.
    type ConditionState = ();

    /// In this implementation, we have not yet checked and grouped
    /// inputs.
    type Inputs = ();

    /// In this implementation, we have not yet checked and grouped
    /// outputs.
    type Outputs = ();

    type ServiceKind = String;
}

/// A DevEnv used to represent a script that hasn't been
/// compiled yet. Rather than having typed devices, capabilities,
/// etc. everything is represented by a string.
pub struct UncheckedEnv;
impl DevEnv for UncheckedEnv {
}
