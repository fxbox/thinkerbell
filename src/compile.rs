use dependencies::DevEnv;
use std::marker::PhantomData;
use std::sync::{Arc, RwLock};
use std::collections::HashMap;

use ast::{Script, Resource, Trigger, Statement, Conjunction, Condition, Context, UncheckedCtx, UncheckedEnv};
use util::map;

extern crate fxbox_taxonomy;
use self::fxbox_taxonomy::values::*;
use self::fxbox_taxonomy::devices::*;
use self::fxbox_taxonomy::requests::*;
use self::fxbox_taxonomy::util::Exactly;

extern crate chrono;
use self::chrono::{DateTime, UTC};

///
/// # Precompilation
///

/// Data, labelled with its latest update.
pub struct DatedData {
    pub updated: DateTime<UTC>,
    pub data: Value,
}


pub struct CompiledCtx<DevEnv> {
    phantom: PhantomData<DevEnv>,
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
pub struct CompiledConditionState {
    pub is_met: bool
}

impl<Env> Context for CompiledCtx<Env> where Env: DevEnv {
    type ConditionState = CompiledConditionState; // FIXME: We could share this
    type ServiceKind = ServiceKind;
    type Inputs = InputRequest; // FIXME: Monitor changes to the topology
    type Outputs = OutputRequest; // FIXME: Monitor changes to the topology
}


#[derive(Debug)]
pub enum SourceError {
    AllocationLengthError { allocations: usize, requirements: usize},
    NoCapability, // FIXME: Add details
    NoSuchInput, // FIXME: Add details
    NoSuchOutput, // FIXME: Add details
}

#[derive(Debug)]
pub enum ServiceError {
    NoSuchService(String),
}

#[derive(Debug)]
pub enum TypeError {
    RequestHasConflictingKind(String),
    RangeHasConflictingType,
    RangeHasNoType(String),
}

#[derive(Debug)]
pub enum Error {
    SourceError(SourceError),
    ServiceError(ServiceError),
    TypeError(TypeError),
}

pub struct Precompiler<Env> where Env: self::fxbox_taxonomy::api::API {
    phantom: PhantomData<Env>,
}

impl<Env> Precompiler<Env> where Env: self::fxbox_taxonomy::api::API {
    pub fn new(source: &Script<UncheckedCtx, UncheckedEnv>) -> Result<Self, Error> {
        Ok(Precompiler {
            phantom: PhantomData
        })
    }

    pub fn rebind_script(&self, env: &Env, script: Script<UncheckedCtx, UncheckedEnv>) -> Result<Script<CompiledCtx<Env>, Env>, Error>
    {
        unimplemented!()
            /*
        if self.rules.len() == 0 {
            return Err(SourceError::NoRules);
        }
        if self.inputs.len() == 0 {
            return Err(SourceError::NoInputs);
        }
        if self.outputs.len() == 0 {
            return Err(SourceError::NoOutputs);
        }

        let service_api = env.get_service_api();

        let rules = try!(map(script.rules, |rule| self.rebind_trigger(rule)));

        let inputs = try!(map(self.inputs, |resource| {
            // Check that all inputs have the same ServiceKind
            let mut kind = None;
            let services = try!(map(resource.services, |id| {
                let mut devices = env.get_service_api().get_input_services(
                    InputRequest::new()
                        .with_id(ServiceId::new(id)));
                if devices.len() == 0 {
                    return Err(DevAccessError::NoSuchInput); // FIXME: Annotate id?
                }
                assert!(devices.len() == 1);
                let device = devices.pop();
                match kind.take() {
                    None => { kind = Some(device.kind.clone()); },
                    Some(k) => {
                        if k != device.kind {
                            return Err(DevAccessError::IncompatibleKind);
                        }
                    }
                }
                Ok(device.id)
            }));
            // FIXME: Check that all have the same ServiceKind.
            // FIXME:
        }));

        Ok(Script {
            metadata: (),
            rules: rules
        })
*/
    }

    fn rebind_trigger(&self, trigger: Trigger<UncheckedCtx, UncheckedEnv>) -> Result<Trigger<CompiledCtx<Env>, Env>, Error>
    {
/*
        let execute = try!(map(trigger.execute, |statement| {
            self.rebind_statement(statement)
        }));
        Ok(Trigger {
            execute: execute,
            condition: try!(self.rebind_conjunction(trigger.condition))
        })
         */
        unimplemented!()
    }

    fn rebind_conjunction(&self, conjunction: Conjunction<UncheckedCtx, UncheckedEnv>) -> Result<Conjunction<CompiledCtx<Env>, Env>, Error>
    {
        /*
        let all = try!(map(conjunction.all, |condition| {
            self.rebind_condition(condition)
        }));
        Ok(Conjunction {
            all: all,
            state: try!(self.rebind_condition_state(conjunction.state))
        })
         */
        unimplemented!()
    }

    fn rebind_condition(&self, env: &Env, condition: Condition<UncheckedCtx, UncheckedEnv>) -> Result<Condition<CompiledCtx<Env>, Env>, Error>
    {
        // Normalize kind and check that the list of devices is
        // currently non-empty.
        // FIXME: We should also re-check when the topology changes.

        let kind = match env.get_service_api()
            .get_input_services(condition.request.clone())
            .iter().fold(Exactly::Empty, |(kind, dev)| {
                kind.and(dev.kind)
            }) {
                Exactly::Empty => return Err(Error::ServiceError(ServiceError::NoSuchService(format!("{:?}", &condition.request())))),
                Exactly::Conflict => return Err(Error::TypeError(TypeError::RequestHasConflictingKind(format!("{:?}", &condition.request())))),
                Exactly::Exactly(k) => k
        };

        // Make sure that future tagging does not cause kind errors.
        let input = condition.request.with_kind(kind);

        // Check that the range and the type of `kind` agree.
        match condition.range.get_type() {
            Ok(None) => {},
            Ok(Some(typ)) => {
                if typ != kind.get_type() {
                    return Err(Error::TypeError(TypeError::RangeHasConflictingType))
                }
            },
            Err(err) => return Err(Error::TypeError(TypeError::RangeHasNoType(err)))
        };

        Ok(Condition {
            input: input,
            range: condition.range,
            state: CompiledConditionState { is_met: false }
        })
    }

    fn rebind_statement(&self, statement: Statement<UncheckedCtx, UncheckedEnv>) -> Result<Statement<CompiledCtx<Env>, Env>, Error>
    {
        /*
        let mut arguments = HashMap::with_capacity(statement.arguments.len());
        for (key, expr) in statement.arguments {
            arguments.insert(key.clone(), try!(self.rebind_expression(expr)));
        }
        Ok(Statement {
            destination: try!(self.rebind_output(statement.destination)),
            action: try!(self.rebind_output_capability(statement.action)),
            arguments: arguments
        })
         */
        unimplemented!()
    }

/*
    fn rebind_device(&self, dev: <UncheckedEnv as DevEnv>::Device) -> Result<Env::Device, Error>
    {
        unimplemented!()
        /*
        match Env::get_device(&dev) {
            None => Err(Error::DevAccessError(DevAccessError::DeviceNotFound)),
            Some(found) => Ok(found.clone())
        }
*/
    }


    fn rebind_device_kind(&self, kind: <UncheckedEnv as DevEnv>::DeviceKind) ->
        Result<Env::DeviceKind, Error>
    {
        unimplemented!()
        /*
        match Env::get_device_kind(&kind) {
            None => Err(Error::DevAccessError(DevAccessError::DeviceKindNotFound)),
            Some(found) => Ok(found.clone())
        }
*/
    }
    
    fn rebind_input_capability(&self, cap: <UncheckedEnv as DevEnv>::InputCapability) ->
        Result<Env::InputCapability, Error>
    {
        unimplemented!()
            /*
        match Env::get_input_capability(&cap) {
            None => Err(Error::DevAccessError(DevAccessError::DeviceCapabilityNotFound)),
            Some(found) => Ok(found.clone())
        }
*/
    }

    fn rebind_output_capability(&self, cap: <UncheckedEnv as DevEnv>::OutputCapability) ->
        Result<Env::OutputCapability, Error>
    {
        unimplemented!()
            /*
        match Env::get_output_capability(&cap) {
            None => Err(Error::DevAccessError(DevAccessError::DeviceCapabilityNotFound)),
            Some(found) => Ok(found.clone())
        }
         */
    }
*/
}
