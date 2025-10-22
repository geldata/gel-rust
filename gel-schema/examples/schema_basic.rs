use gel_schema::{Class, Name, Object, Schema, Value};

fn main() {
    let uuid = uuid::Uuid::new_v4();

    let obj = Object::new(Class::Module, uuid);

    let mut data = Vec::with_capacity(13);
    data.push(Value::Bool(false));
    data.push(Value::None);
    data.push(Value::Name(Name {
        module: None,
        object: "default".into(),
    }));
    for _ in 0..10 {
        data.push(Value::None);
    }

    let schema = Schema::default();
    for _ in 0..10000000 {
        let mut schema = schema.clone();
        schema.add(&obj, data.clone()).unwrap();
    }
}
