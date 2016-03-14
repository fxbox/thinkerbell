extern crate foxbox_thinkerbell;

use std::path::Path;

use foxbox_thinkerbell::database::*;

#[test]
fn test_database_add_remove_script() {
    let db = ScriptDatabase::new(Path::new("./test_script_database.sqlite")).unwrap();

    db.remove_all().unwrap();

    let mut script = StoredScript {
        id: None,
        name: "My Script".to_owned(),
        source: "some source".to_owned(),
        is_active: false
    };
    db.insert(&mut script).unwrap();
    assert!(script.id != None);

    let scripts = db.get_all();
    assert_eq!(scripts.len(), 1);

    db.remove(script.id.unwrap()).unwrap();

    let scripts = db.get_all();
    assert_eq!(scripts.len(), 0);
}
