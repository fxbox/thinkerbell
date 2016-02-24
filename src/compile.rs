//! A script compiler
//!
//! This compiler take untrusted code (`Script<UncheckedCtx,
//! UncheckedEnv>`) and performs the following transformations and
//! checks:
//! - Ensure that the `Script` has at least one `Trigger`.
//! - Ensure that each `Trigger `has at least one `Conjunction`.
//! - Ensure that each `Conjunction` has at least one `Condition`.
//! - Transform each `Condition` to make sure that the type of the
//!   `range` matches the type of the `input`.
//! - Ensure that in each `Statement`, the type of the `value` matches
//!   the type of the `destination`.
//! - Introduce markers to keep track of which conditions were already
//!   met last time they were evaluated.

use std::marker::PhantomData;

use ast::{Script, Trigger, Statement, Conjunction, Condition, Context, UncheckedCtx, UncheckedEnv};
use util::*;

use fxbox_taxonomy::requests::*;
use fxbox_taxonomy::api::API;

use serde::ser::{Serialize, Serializer};
use serde::de::{Deserialize, Deserializer};


/// The environment in which the code is meant to be executed.  This
/// can typically be instantiated either with actual bindings to
/// devices, or with a unit-testing framework. // FIXME: Move this to run.rs
pub trait ExecutableDevEnv: Serialize + Deserialize + Default {
    type WatchGuard;
    type API: API<WatchGuard = Self::WatchGuard>;
}


///
/// # Precompilation
///

#[derive(Serialize, Deserialize)]
pub struct CompiledCtx<Env> where Env: Serialize + Deserialize {
    phantom: Phantom<Env>,
}

/*
pub struct CompiledInput<Env> where Env: DevEnv {
    pub device: Env::Device,
    pub state: RwLock<Option<DatedData>>,
}

pub struct CompiledOutput<Env> where Env: DevEnv {
    pub device: Env::Device,
}

pub type CompiledInputSet<Env> = Vec<Arc<CompiledInput<Env>>>;
pub type CompiledOutputSet<Env> = Vec<Arc<CompiledOutput<Env>>>;
*/

#[derive(Serialize, Deserialize)]
pub struct CompiledConditionState {
    /// `true` if the condition was met last time we evaluated it,
    /// `false` otherwise.
    pub is_met: bool
}
impl Default for CompiledConditionState {
    fn default() -> Self {
        CompiledConditionState {
            is_met: true
        }
    }
}

/// We implement `Default` to keep derive happy, but this code should
/// be unreachable.
impl<Env> Default for CompiledCtx<Env> where Env: Serialize + Deserialize {
    fn default() -> Self {
        panic!("Called CompledCtx<_>::default()");
    }
}

impl<Env> Context for CompiledCtx<Env> where Env: Serialize + Deserialize {
    type ConditionState = CompiledConditionState;
    type Inputs = InputRequest;
    type Outputs = OutputRequest;
}

#[derive(Debug)]
pub enum SourceError {
    /// The source doesn't define any rule.
    NoRules,

    /// A rule doesn't have any statements.
    NoStatements,
}

#[derive(Debug)]
pub enum TypeError {
    /// The range cannot be typed.
    InvalidRange,

    /// The range has one type but this type is incompatible with the
    /// kind of the `Condition`.
    KindAndRangeDoNotAgree,
}

#[derive(Debug)]
pub enum Error {
    SourceError(SourceError),
    TypeError(TypeError),
}

pub struct Compiler<Env> where Env: ExecutableDevEnv {
    phantom: PhantomData<Env>,
}

impl<Env> Compiler<Env> where Env: ExecutableDevEnv {
    pub fn new() -> Result<Self, Error> {
        Ok(Compiler {
            phantom: PhantomData
        })
    }

    pub fn compile(&self, script: Script<UncheckedCtx, UncheckedEnv>)
                   -> Result<Script<CompiledCtx<Env>, Env>, Error> {
        self.compile_script(script)
    }

    pub fn compile_script(&self, script: Script<UncheckedCtx, UncheckedEnv>) -> Result<Script<CompiledCtx<Env>, Env>, Error>
    {
        if script.rules.len() == 0 {
            return Err(Error::SourceError(SourceError::NoRules));
        }
        let rules = try!(map(script.rules, |rule| {
            self.compile_trigger(rule)
        }));
        Ok(Script {
            rules: rules,
            phantom: Phantom::new()
        })
    }

    pub fn compile_trigger(&self, trigger: Trigger<UncheckedCtx, UncheckedEnv>) -> Result<Trigger<CompiledCtx<Env>, Env>, Error>
    {
        if trigger.execute.len() == 0 {
            return Err(Error::SourceError(SourceError::NoStatements));
        }
        let condition = try!(self.compile_conjunction(trigger.condition));
        let execute = try!(map(trigger.execute, |statement| {
            self.compile_statement(statement)
        }));
        Ok(Trigger {
            condition: condition,
            execute: execute,
            phantom: Phantom::new()
        })
    }

    fn compile_conjunction(&self, conjunction: Conjunction<UncheckedCtx, UncheckedEnv>) -> Result<Conjunction<CompiledCtx<Env>, Env>, Error>
    {
        let all = try!(map(conjunction.all, |condition| {
            self.compile_condition(condition)
        }));
        Ok(Conjunction {
            all: all,
            state: CompiledConditionState::default(),
            phantom: Phantom::new(),
        })
    }

    fn compile_condition(&self, condition: Condition<UncheckedCtx, UncheckedEnv>) -> Result<Condition<CompiledCtx<Env>, Env>, Error>
    {
        let typ = match condition.range.get_type() {
            Err(_) => return Err(Error::TypeError(TypeError::InvalidRange)),
            Ok(typ) => typ
        };
        if condition.kind.get_type() != typ {
            return Err(Error::TypeError(TypeError::KindAndRangeDoNotAgree));
        }
        let input = condition.input.with_kind(condition.kind.clone());
        Ok(Condition {
            input: input,
            kind: condition.kind,
            range: condition.range,
            phantom: Phantom::new()
        })
    }

    fn compile_statement(&self, statement: Statement<UncheckedCtx, UncheckedEnv>) -> Result<Statement<CompiledCtx<Env>, Env>, Error>
    {
        let destination = statement.destination.with_kind(statement.kind.clone());
        Ok(Statement {
            destination: destination,
            value: statement.value,
            kind: statement.kind,
            phantom: Phantom::new()
        })
    }
}
