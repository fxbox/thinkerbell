#![feature(custom_derive, plugin)]
#![plugin(serde_macros)]

//! This create defines mechanisms for executing simple scripts on the
//! server.
//!
//! By design, these scripts have limited capabilities. Each script
//! takes the form of a set of rules: "when any of the input services
//! of foo matches some condition, send some value to all of the onput
//! services of bar".
//!
//! See module `ast` for more details on the grammar of scripts.

extern crate chrono;
extern crate serde;
extern crate serde_json;
extern crate transformable_channels;

extern crate foxbox_taxonomy;

/// Definition of the AST.
pub mod ast;

/// Parsing JSON into an AST.
pub mod parse;

/// Compiling an AST into something runnable.
pub mod compile;

/// Actually executing code.
pub mod run;

/// Miscellaneous internal utilities.
pub mod util;

/// Database for storing scripts.
pub mod database;
