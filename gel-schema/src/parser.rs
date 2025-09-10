use std::{collections::HashMap, str::FromStr};

use indexmap::IndexMap;
use tinyjson::JsonValue;
use uuid::Uuid;

use crate::{
    Class, ContainerTy, Expression, Name, Object, ObjectListTy, Schema, Value, Version,
    VersionStage,
};
use crate::{name, structure};

pub fn parse_reflection(
    base_schema: &Schema,
    reflected_json: &str,
    layouts: &structure::Structures,
) -> Schema {
    let reflected: JsonValue = reflected_json.parse().unwrap();
    let JsonValue::Array(reflected) = reflected else {
        panic!();
    };

    let mut schema = Schema::default();

    // iterate JSON and construct objects map
    let mut objects: HashMap<Uuid, (Object, HashMap<String, JsonValue>)> = Default::default();
    for object in reflected {
        let JsonValue::Object(entry) = object else {
            panic!()
        };

        let JsonValue::String(id) = entry.get("id").unwrap() else {
            panic!()
        };
        let JsonValue::String(t_name) = entry.get("_tname").unwrap() else {
            panic!()
        };

        let id = Uuid::from_str(id).unwrap();

        let (_, cls_name) = t_name.rsplit_once("::").unwrap_or_else(|| ("", t_name));

        let cls = Class::from_str(cls_name).unwrap_or_else(|_| todo!("schema class: {cls_name}"));
        let obj = Object::new(cls, id);
        objects.insert(id, (obj, entry));
    }

    let mut refdict_updates: HashMap<Uuid, HashMap<String, Value>> = HashMap::new();
    let mut refs_to: RefsTo = Default::default();
    for (obj, entry) in objects.values() {
        let span = tracing::span!(tracing::Level::DEBUG, "obj", o = format!("{obj:?}"));
        let _enter = span.enter();

        let obj_id = obj.id;
        let cls = obj.class;

        let JsonValue::String(name_internal) = entry.get("name__internal").unwrap() else {
            panic!();
        };
        let name = Name::new_from_string(name_internal);
        let cls_structure = layouts.classes.get(cls.as_ref()).unwrap();

        if base_schema.has_object(&obj_id) && cls != Class::SchemaVersion {
            continue;
        }

        if cls.is_qualified() {
            schema.name_to_id.insert(name.clone(), obj_id);
        } else {
            schema
                .globalname_to_id
                .insert((cls, name.clone().into_unqualified()), obj_id);
        }

        if matches!(cls, Class::Function | Class::Operator) {
            let shortname = name.as_shortname(cls);
            let ids = schema.shortname_to_id.entry((cls, shortname)).or_default();
            ids.insert(obj_id);
        }

        schema.id_to_type.insert(obj_id, cls);

        let fields_len = cls_structure
            .fields
            .values()
            .map(|f| f.index)
            .max()
            .map(|x| x + 1)
            .unwrap_or_default();
        let mut obj_data = vec![None; fields_len];

        for (k, v) in entry {
            let Some(layout) = cls_structure.layouts.get(k) else {
                continue;
            };

            // fn = field.fieldname
            let Some(field) = cls_structure.fields.get(&layout.fieldname) else {
                continue;
            };
            let f_index = field.index;
            let f_type = &field.r#type;

            tracing::trace!("field {k}, f_index = {f_index}, f_type = {f_type}");
            tracing::trace!("  value = {v:?}");
            tracing::trace!("  layout = {layout:#?}");
            tracing::trace!("  field = {field:#?}");

            if let Some(storage) = &layout.storage {
                if v.is_null() {
                } else if storage.ptrkind == "link" {
                    let JsonValue::Object(v) = v else { panic!() };
                    let Some(JsonValue::String(id)) = v.get("id") else {
                        panic!()
                    };
                    let ref_id = Uuid::from_str(id).unwrap();
                    let val = if let Some(newobj) = objects.get(&ref_id) {
                        Value::Object(newobj.0.clone())
                    } else {
                        base_schema
                            .get_by_id(ref_id)
                            .map(Value::Object)
                            .unwrap_or(Value::None)
                    };

                    record_refs(&mut refs_to, obj, &layout.fieldname, &val);
                    obj_data[f_index] = Some(val);
                } else if storage.ptrkind == "multi link" {
                    let (f_type, f_ty_arg) = unpack_generic_py_type(f_type);

                    let val = if f_type == "ObjectDict" {
                        let ty_args = f_ty_arg.unwrap().to_string();
                        let ty_args = ty_args.split_once(", ").unwrap();
                        assert_eq!(ty_args.0, "str");
                        let value_ty = ty_args.1.to_string();

                        let JsonValue::Array(v) = v else { panic!() };

                        let mut refids = Vec::with_capacity(v.len());
                        let mut refkeys = Vec::with_capacity(v.len());
                        for e in v {
                            let JsonValue::Object(e) = e else { panic!() };

                            let Some(JsonValue::String(name)) = e.get("name") else {
                                panic!()
                            };
                            refkeys.push(name.clone());

                            let Some(JsonValue::String(id)) = e.get("value") else {
                                panic!()
                            };
                            refids.push(Uuid::from_str(id).unwrap());
                        }
                        Value::ObjectDict {
                            keys: refkeys,
                            values: refids,
                            value_ty,
                        }
                    } else {
                        let values = _parse_array_of_ids(v);
                        if f_type == "FuncParameterList" {
                            Value::ObjectList {
                                ty: ObjectListTy::FuncParameterList,
                                values,
                                value_ty: None,
                            }
                        } else if f_type == "ObjectList" {
                            Value::ObjectList {
                                ty: ObjectListTy::ObjectList,
                                values,
                                value_ty: Some(f_ty_arg.unwrap().to_string()),
                            }
                        } else if f_type == "ObjectSet" {
                            let value_ty = Some(f_ty_arg.unwrap().to_string());
                            Value::ObjectSet { values, value_ty }
                        } else {
                            panic!()
                        }
                    };
                    record_refs(&mut refs_to, obj, &layout.fieldname, &val);
                    obj_data[f_index] = Some(val);
                } else if storage.shadow_ptrkind.is_some() {
                    let v = entry.get(&format!("{k}__internal"));
                    let val = if let Some(v) = v {
                        if f_type == "Expression" {
                            Value::Expression(_parse_expression(v, obj_id, k))
                        } else if f_type.starts_with("ExpressionList") {
                            let JsonValue::Array(v) = v else { panic!() };

                            let mut exprs = Vec::new();
                            for e_dict in v {
                                exprs.push(_parse_expression(e_dict, obj_id, k))
                            }
                            Value::ExpressionList(exprs)
                        } else if f_type.starts_with("ExpressionDict") {
                            let JsonValue::Array(v) = v else { panic!() };

                            let mut expr_dict = IndexMap::default();
                            for e_dict in v {
                                let JsonValue::Object(e_dict) = e_dict else {
                                    panic!()
                                };

                                let Some(JsonValue::String(name)) = e_dict.get("name") else {
                                    panic!()
                                };
                                let e = _parse_expression(e_dict.get("expr").unwrap(), obj_id, k);
                                expr_dict.insert(name.clone(), e);
                            }
                            Value::ExpressionDict(expr_dict)
                        } else if f_type.starts_with("Object") {
                            todo!()
                            // val = val.id
                        } else if f_type.starts_with("Name") {
                            let JsonValue::String(v) = v else { panic!() };

                            Value::Name(if cls.is_qualified() {
                                Name::new_from_string(v)
                            } else {
                                Name::new_unqualified(v.clone())
                            })
                        } else {
                            _parse_value(v, &layout.r#type, f_type)
                        }
                    } else {
                        todo!()
                    };
                    obj_data[f_index] = Some(val);
                } else {
                    let val = if f_type == "Version" {
                        todo!()
                        // objdata[findex] = Value::Version(_parse_version(v))
                    } else if f_type == "Name" {
                        let JsonValue::String(v) = v else { panic!() };
                        Value::Name(Name::new_from_string(v))
                    } else if f_type == "ParametricContainer"
                    // TODO:
                    // && ftype.types
                    // && len(ftype.types) == 1
                    {
                        todo!()
                        // Coerce the elements in a parametric container
                        // type.
                        // XXX: Or should we do it in the container?
                        // subtyp = ftype.types[0]
                        // obj_data[f_index] = ftype(subtyp(x) for x in v)
                    } else if f_type.starts_with("MultiPropSet") {
                        let value_ty = unpack_generic_py_type(f_type).1.unwrap().to_string();

                        let JsonValue::Array(v) = v else { panic!() };
                        let values = v
                            .iter()
                            .map(|v| _parse_value(v, &layout.r#type, &value_ty))
                            .collect();
                        Value::Container {
                            ty: ContainerTy::MultiPropSet,
                            value_ty,
                            values,
                        }
                    } else {
                        _parse_value(v, &layout.r#type, f_type)
                    };
                    obj_data[f_index] = Some(val);
                }
            } else if layout.is_refdict {
                let ref_ids = _parse_array_of_ids(v);

                let (obj_index_base_ty, ty_arg) = unpack_generic_py_type(f_type);
                let ty = Value::get_object_index_ty_name(obj_index_base_ty);

                let val = Value::ObjectIndex {
                    ty,
                    value_ty: ty_arg.unwrap().to_string(),
                    keys: None,
                    values: ref_ids.clone(),
                };

                record_refs(&mut refs_to, obj, &layout.fieldname, &val);
                obj_data[f_index] = Some(val);

                // properties
                let JsonValue::Array(v) = v else { panic!() };
                for (id, e_dict) in std::iter::zip(ref_ids, v) {
                    let JsonValue::Object(e_dict) = e_dict else {
                        panic!()
                    };

                    let mut property_updates: HashMap<String, Value> = Default::default();
                    for p in layout.properties.keys() {
                        let Some(pv) = e_dict.get(&format!("@{p}")) else {
                            continue;
                        };

                        let pv = match pv {
                            JsonValue::Number(n) => Value::Int(*n as i64),
                            JsonValue::Boolean(b) => Value::Bool(*b),
                            JsonValue::String(s) => Value::Str(s.clone()),
                            JsonValue::Null => Value::None,
                            JsonValue::Array(_) => todo!(),
                            JsonValue::Object(_) => todo!(),
                        };
                        property_updates.insert(p.clone(), pv);
                    }
                    refdict_updates.insert(id, property_updates);
                }
            }
        }
        let obj_data = obj_data
            .into_iter()
            .map(|d| d.unwrap_or(Value::None))
            .collect();
        schema.id_to_data.insert(obj_id, obj_data);
    }

    for (obj_id, updates) in refdict_updates {
        let Some(cls) = schema.id_to_type.get(&obj_id) else {
            tracing::warn!("cannot find object with id {obj_id}");
            continue;
        };
        let cls_structure = layouts.classes.get(cls.as_ref()).unwrap();

        let id_to_data = schema.id_to_data.get_mut(&obj_id).unwrap();
        for (f_name, v) in updates {
            let field = cls_structure
                .fields
                .get(&f_name)
                .unwrap_or_else(|| panic!("cannot get field {}.{f_name}", cls.as_ref()));

            let data = id_to_data.get_mut(field.index).unwrap();
            *data = v;
        }
    }

    for (referred_id, ref_data) in refs_to {
        let entry = schema.refs_to.entry(referred_id);

        match entry {
            im::ordmap::Entry::Occupied(mut entry) => {
                for (k, referrers) in ref_data {
                    let e = entry.get_mut().entry(k);
                    match e {
                        im::ordmap::Entry::Occupied(mut e) => {
                            e.get_mut().extend(referrers);
                        }
                        im::ordmap::Entry::Vacant(e) => {
                            e.insert(referrers);
                        }
                    }
                }
            }
            im::ordmap::Entry::Vacant(entry) => {
                entry.insert(ref_data);
            }
        }
    }

    schema
}

fn unpack_generic_py_type(f_type: &str) -> (&str, Option<&str>) {
    let Some((ty, ty_args)) = f_type.split_once('[') else {
        return (f_type, None);
    };
    let ty_args = ty_args.strip_suffix(']').unwrap();
    (ty, Some(ty_args))
}

fn _parse_array_of_ids(v: &JsonValue) -> Vec<Uuid> {
    let JsonValue::Array(v) = v else { panic!() };
    let mut ids = Vec::with_capacity(v.len());
    for e in v {
        let JsonValue::Object(e) = e else { panic!() };

        let Some(JsonValue::String(id)) = e.get("id") else {
            panic!("e: {e:?}")
        };
        ids.push(Uuid::from_str(id).unwrap());
    }
    ids
}

type RefsTo = im::OrdMap<Uuid, im::OrdMap<(Class, String), im::OrdSet<Uuid>>>;

/// Given an object and its field values, record refs of this value in the schema.
fn record_refs(refs_to: &mut RefsTo, object: &Object, field_name: &str, val: &Value) {
    for ref_id in val.ref_ids() {
        let target = refs_to.entry(ref_id).or_default();
        let refs = target
            .entry((object.class, field_name.to_string()))
            .or_default();
        refs.insert(object.id);
    }
}

fn _parse_expression(val: &JsonValue, id: Uuid, field: &str) -> Expression {
    let JsonValue::Object(val) = val else {
        panic!("expression expected an object, got: {val:?}")
    };

    let Some(JsonValue::Array(refs)) = val.get("refs") else {
        panic!()
    };
    let refs = refs
        .iter()
        .map(|r| {
            let JsonValue::String(r) = r else { panic!() };
            Uuid::from_str(r).unwrap()
        })
        .collect();

    let Some(JsonValue::String(text)) = val.get("text").cloned() else {
        panic!()
    };

    Expression {
        text,
        refs,
        origin: Some(format!("{id} {field}")),
    }
}

fn _parse_version(val: &JsonValue) -> Version {
    let JsonValue::Object(val) = val else {
        panic!()
    };

    let Some(JsonValue::Number(major)) = val.get("major").cloned() else {
        panic!()
    };
    let Some(JsonValue::Number(minor)) = val.get("minor").cloned() else {
        panic!()
    };
    let Some(JsonValue::String(stage)) = val.get("stage").cloned() else {
        panic!()
    };
    let stage = VersionStage::from_str(&stage).unwrap();
    let Some(JsonValue::Number(stage_no)) = val.get("stage_no").cloned() else {
        panic!()
    };
    let Some(JsonValue::Array(local)) = val.get("local").cloned() else {
        panic!()
    };
    let local = local
        .into_iter()
        .map(|v| match v {
            JsonValue::String(s) => s,
            _ => panic!(),
        })
        .collect();

    Version {
        major: major as u8,
        minor: minor as u8,
        stage,
        stage_no: stage_no as u16,
        local: local,
    }
}

fn _parse_value(val: &JsonValue, eql_ty: &str, py_ty: &str) -> Value {
    if ["Name", "QualName", "UnqualName"].contains(&py_ty) {
        let JsonValue::String(s) = val else { panic!() };
        return Value::Name(Name::new_from_string(s));
    }
    if py_ty.starts_with("FrozenChecked") {
        let (ty, ty_arg) = unpack_generic_py_type(py_ty);

        let ty = Value::get_container_ty_name(ty);
        let value_ty = ty_arg.unwrap().to_string();

        let JsonValue::Array(items) = val else {
            panic!()
        };
        let eql_item = eql_ty.strip_prefix("array<").unwrap();
        let eql_item = eql_item.strip_suffix(">").unwrap();
        let eql_item = name::unmangle_name(eql_item);
        let values = items
            .iter()
            .map(|i| _parse_value(i, &eql_item, &value_ty))
            .collect();

        return Value::Container {
            ty,
            values,
            value_ty,
        };
    }

    if let Some(schema_object) = eql_ty.strip_prefix("schema::") {
        if let JsonValue::String(val) = val {
            if let Some(val) = Value::parse_enum(schema_object, val) {
                return val;
            }
        }
    }

    match eql_ty {
        "std::uuid" => {
            let JsonValue::String(s) = val else { panic!() };
            Value::Uuid(Uuid::from_str(s).unwrap())
        }
        "std::bool" => {
            let JsonValue::Boolean(b) = val else { panic!() };
            Value::Bool(*b)
        }
        "std::str" => {
            let JsonValue::String(s) = val else { panic!() };
            Value::Str(s.clone())
        }
        "std::int16" | "std::int32" | "std::int64" => {
            let JsonValue::Number(n) = val else { panic!() };
            Value::Int(*n as i64)
        }
        _ => todo!("ty: {eql_ty}, val: {val:?}"),
    }
}
