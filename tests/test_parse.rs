extern crate fxbox_thinkerbell;
extern crate fxbox_taxonomy;
extern crate serde_json;

use fxbox_thinkerbell::ast::*;
use fxbox_thinkerbell::parse::*;
use fxbox_thinkerbell::values::*;
use fxbox_thinkerbell::util::*;

use fxbox_taxonomy::requests::*;

#[test]
fn test_parse_bad_field() {
    let src = "{
      \"requirements\": [],
      \"allocations\": [],
      \"rules\": []
    }".to_owned();

    let result = Parser::parse(src);
    match result {
        Err(serde_json::error::Error::SyntaxError(serde_json::error::ErrorCode::UnknownField(field), _, _)) => {
            assert!(field == "requirements".to_owned() || field == "allocations".to_owned())
        },
        _ => assert!(false)
    };
}

#[test]
fn test_parse_empty() {
    let src = "{ \"rules\": []}".to_owned();
    let script = Parser::parse(src).unwrap();
    assert_eq!(script.rules.len(), 0);
}

#[test]
fn test_parse_simple_rule() {
/*
    let script : Script<UncheckedCtx, UncheckedEnv> = Script {
        rules: vec![
            Trigger {
                condition: Conjunction {
                    all: vec! [
                        Condition {
                            input: InputRequest::new(),
                            range: Range::Any,
                            phantom: Phantom::new(),
                        }],
                    state: (),
                    phantom: Phantom::new()
                },
                execute: vec![],
                phantom: Phantom::new()
            }],
        phantom: Phantom::new()
    };
    println!("Converting to value\n");
    let val = serde_json::to_value(&script);
    println!("Converting to string {}\n", val.as_string().unwrap());
*/
    
    let src =
"{
  \"rules\": [
    {
      \"execute\": [],
      \"condition\": {
        \"all\": [
          {
            \"input\": {},
            \"range\": {
              \"Any\": []
            }
          }
        ]
      }
    }
  ]
}".to_owned();
    Parser::parse(src).unwrap();
}

