use rib::wit_type::builders as wt;
use rib::wit_type::{NameTypePair, TypeRecord, WitType};
use rib::{Value, ValueAndType};
use rib_repl::{try_runtime_to_value_and_type, try_value_and_type_to_runtime, RuntimeValue};

#[test]
fn roundtrip_u32() {
    let v = ValueAndType::new(Value::U32(42), wt::u32());
    let r = try_value_and_type_to_runtime(&v).unwrap();
    assert_eq!(format!("{r:?}"), "U32(42)");
    let back = try_runtime_to_value_and_type(&r, &wt::u32()).unwrap();
    assert_eq!(back, v);
}

#[test]
fn roundtrip_record() {
    let typ = WitType::Record(TypeRecord {
        name: None,
        owner: None,
        fields: vec![
            NameTypePair {
                name: "a".into(),
                typ: wt::u32(),
            },
            NameTypePair {
                name: "b".into(),
                typ: wt::str(),
            },
        ],
    });
    let v = ValueAndType::new(
        Value::Record(vec![Value::U32(1), Value::String("x".into())]),
        typ.clone(),
    );
    let r = try_value_and_type_to_runtime(&v).unwrap();
    let RuntimeValue::Record(p) = &r else {
        panic!("expected record");
    };
    assert_eq!(p.len(), 2);
    assert_eq!(p[0].0, "a");
    assert_eq!(p[1].0, "b");
    let back = try_runtime_to_value_and_type(&r, &typ).unwrap();
    assert_eq!(back, v);
}
