#![allow(unused_variables)]
#![allow(dead_code)]

/// APIs that we need to implement the code in module lang.

/// The environment in which the code is meant to be executed.
/// This can be instantiated either with actual bindings to
/// devices, or with a unit-testing framework.
pub trait Environment {
    type Input;
    type Output;
    type ConditionState;
    type Device: Clone;
    type InputCapability;
    type OutputCapability;
    type Watcher: Watcher;
}

/// An object that may be used to track state changes in devices.
pub trait Watcher {
    type Witness;
    fn new() -> Self;

    /// Watch a property of a device.
    fn add<F>(&mut self,
              device: &Device,
              input: &InputCapability,
              condition: &Range,
              cb: F) -> Self::Witness where F:FnOnce(Value);
}


use std::sync::Arc;

extern crate rustc_serialize;

use self::rustc_serialize::json::Json;

use std::cmp::{Eq, Ord, Ordering};


/// A description of a device, e.g. "a lightbulb".
#[derive(Clone)]
pub struct DeviceKind;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct InputCapability;

#[derive(Clone)]
pub struct OutputCapability;

#[derive(Clone)]
pub struct Device;

impl Device {
    fn fetch(&self, cap: &InputCapability) -> Result<Value, ()> {
        panic!("Not implemented yet");
    }
}

#[derive(Clone)]
pub enum Range {
    /// Operations on numbers.

    /// Leq(x) accepts any value v such that v <= x.
    Leq(Number),

    /// Geq(x) accepts any value v such that v >= x.
    Geq(Number),

    /// BetweenEq {min, max} accepts any value v such that `min <= v`
    /// and `v <= max`. If `max < min`, it never accepts anything.
    BetweenEq {min:Number, max:Number},

    /// OutOfStrict {min, max} accepts any value v such that `v < min`
    /// or `max < v`
    OutOfStrict {min:Number, max:Number},


    /// Operations on strings

    /// `EqString(s) accepts any value v such that v == s`.
    EqString(String),

    /// Operations on bools

    /// `EqBool(s) accepts any value v such that v == s`.
    EqBool(bool),

    /// Operations on anything

    /// `Any` accepts all values.
    Any,
}

#[derive(Clone)]
pub struct Number {
    value: f64,
    physical_unit: (), // FIXME: Implement
}

impl Ord for Number {
    fn cmp(&self, other: &Self) -> Ordering {
        panic!("Not implemented")
    }
}

impl PartialOrd<Number> for Number {
    fn partial_cmp(&self, other: &Number) -> Option<Ordering> {
        panic!("Not implemented")
    }
}

impl PartialEq<Number> for Number {
    fn eq(&self, other: &Number) -> bool {
        panic!("Not implemented")
    }
}

impl Eq for Number {
}

#[derive(Clone)]
pub enum Value {
    String(String),
    Bool(bool),
    Num(Number),
    Json(Json),
    Blob{data: Arc<Vec<u8>>, mime_type: String},
}


/// A structure used to stop watching a property once it is dropped.
pub struct Witness; // FIXME: Define
