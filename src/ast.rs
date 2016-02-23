/// Basic structure of a Monitor (aka Server App)
///
/// Monitors are designed so that the FoxBox can offer a simple
/// IFTTT-style Web UX to let users write their own scripts. More
/// complex monitors can installed from the web from a master device
/// (i.e. the user's cellphone or smart tv).

use dependencies::DevEnv;
use values::Range;
use util::Phantom;

use fxbox_taxonomy::values::Value;
use fxbox_taxonomy::devices::*;
use fxbox_taxonomy::requests::*;

use serde::ser::{Serialize, Serializer};
use serde::de::{Deserialize, Deserializer, Error};

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
#[derive(Serialize, Deserialize)]
pub struct Script<Ctx, Env> where Env: DevEnv, Ctx: Context {
    /// A set of rules, stating what must be done in which circumstance.
    pub rules: Vec<Trigger<Ctx, Env>>,

    #[serde(default)]
    #[allow(dead_code)]
    pub phantom: Phantom<(Ctx, Env)>,
}

/// A set of similar input services used together to provide a single
/// piece of information. For instance, a set of fire detectors.
///
/// All input services grouped as a resource must provide the same
/// service.
#[derive(Serialize, Deserialize)]
pub struct Resource<IO, Ctx, Env> where Env: DevEnv, Ctx: Context, IO: Deserialize + Default {
    /// The kind of service provided by this resource. During
    /// compilation, we make sure that each resource provides this
    /// service. // FIXME: Implement
    #[serde(default)]
    pub kind: Ctx::ServiceKind, // FIXME: Empty initially

    /// The actual list of endpoints. Must be non-empty. During
    /// compilation, we make sure that each resource appears only
    /// once. // FIXME: Implement.
    pub services: Vec<ServiceId>,

    #[serde(default)]
    pub phantom: Phantom<(IO, Ctx, Env)>,
}

/// A single trigger, i.e. "when some condition becomes true, do
/// something".
#[derive(Serialize, Deserialize)]
pub struct Trigger<Ctx, Env> where Env: DevEnv, Ctx: Context {
    /// The condition in which to execute the trigger.
    pub condition: Conjunction<Ctx, Env>,

    /// Stuff to do once `condition` is met.
    pub execute: Vec<Statement<Ctx, Env>>,

    #[serde(default)]
    #[allow(dead_code)]
    pub phantom: Phantom<(Ctx, Env)>,
}

/// A conjunction (e.g. a "and") of conditions.
#[derive(Serialize, Deserialize)]
pub struct Conjunction<Ctx, Env> where Env: DevEnv, Ctx: Context {
    /// The conjunction is true iff all of the following expressions evaluate to true.
    pub all: Vec<Condition<Ctx, Env>>,

    #[serde(default)]
    pub state: Ctx::ConditionState,

    #[serde(default)]
    #[allow(dead_code)]
    pub phantom: Phantom<(Ctx, Env)>,
}

/// An individual condition.
///
/// Conditions always take the form: "data received from input service
/// is in given range".
///
/// A condition is true if *any* of the corresponding input services
/// yielded a value that is in the given range.
#[derive(Serialize, Deserialize)]
pub struct Condition<Ctx, Env> where Env: DevEnv, Ctx: Context {
    /// The set of inputs to watch. Note that the set of inputs may
    /// change (e.g. when devices are relabelled) without rebooting
    /// the script.
    pub input: Ctx::Inputs,

    /// The range of values for which the condition is considered met.
    /// During compilation, we check that the type of `range` is
    /// compatible with that of `input`. // FIXME: Implement
    pub range: Range,

    #[serde(default)]
    #[allow(dead_code)]
    pub phantom: Phantom<(Ctx, Env)>,
}


/// Stuff to actually do. In practice, this means placing calls to devices.
#[derive(Serialize, Deserialize)]
pub struct Statement<Ctx, Env> where Env: DevEnv, Ctx: Context {
    /// The resource to which this command applies.
    pub destination: Ctx::Outputs,

    /// Data to send to the resource. During compilation, we check
    /// that the type of `value` is compatible with that of
    /// `destination`. // FIXME: Implement
    pub value: Value,

    #[serde(default)]
    #[allow(dead_code)]
    pub phantom: Phantom<(Ctx, Env)>,
}


/// A manner of representing internal nodes.
pub trait Context: Serialize + Deserialize + Default {
    /// A representation of the current state of a condition.
    type ConditionState: Serialize + Deserialize + Default;
    type Inputs: Serialize + Deserialize + Default;
    type Outputs: Serialize + Deserialize + Default;
    type ServiceKind: Serialize + Deserialize + Default;
}

/// A Context used to represent a script that hasn't been compiled
/// yet.
#[derive(Default, Serialize, Deserialize)]
pub struct UncheckedCtx;
impl Context for UncheckedCtx {
    /// In this implementation, conditions have no state.
    type ConditionState = ();
    type Inputs = InputRequest;
    type Outputs = OutputRequest;
    type ServiceKind = ();
}

/// A DevEnv used to represent a script that hasn't been
/// compiled yet. Rather than having typed devices, capabilities,
/// etc. everything is represented by a string.
#[derive(Default, Serialize, Deserialize)]
pub struct UncheckedEnv;
impl DevEnv for UncheckedEnv {
}
