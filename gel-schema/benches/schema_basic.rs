use std::rc::Rc;

use criterion::{Criterion, criterion_group, criterion_main};
use gel_schema::{Name, Schema};

fn schema_add() -> Schema {
    let schema = gel_schema::Schema::default();

    let uuid = uuid::Uuid::new_v4();

    let obj = gel_schema::Object::new(gel_schema::Class::Module, uuid);

    let mut data = Vec::with_capacity(13);
    data.push(gel_schema::Value::Bool(false));
    data.push(gel_schema::Value::None);
    data.push(gel_schema::Value::Name(Rc::new(Name {
        module: None,
        object: "default".into(),
    })));
    for _ in 0..10 {
        data.push(gel_schema::Value::None);
    }

    for _ in 0..100 {
        let mut schema = schema.clone();
        schema.add(&obj, data.clone()).unwrap();
    }

    schema
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("schema_add", |b| b.iter(schema_add));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
