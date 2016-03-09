extern crate foxbox_thinkerbell;

use foxbox_thinkerbell::database::*;

#[test]
fn test_database_add_remove_script() {
    let db = ScriptDatabase::new("./test_script_database.sqlite".to_owned());

    db.remove_all().unwrap();

    let mut script = StoredScript {
        id: None,
        name: "My Script".to_owned(),
        source: "some source".to_owned(),
        is_active: false
    };
    db.save(&mut script).unwrap();
    assert!(script.id != None);

    let scripts = db.get_all();
    assert_eq!(scripts.len(), 1);

    db.remove(script.id.unwrap()).unwrap();

    let scripts = db.get_all();
    assert_eq!(scripts.len(), 0);
}
