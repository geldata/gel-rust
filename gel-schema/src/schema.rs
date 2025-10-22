#![allow(dead_code)]

use im_rc as im;
use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::structure::{self, Field};
use crate::{Class, Name, Object, Value};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Schema {
    pub(crate) id_to_data: im::OrdMap<Uuid, Vec<Value>>,
    pub(crate) id_to_type: im::OrdMap<Uuid, Class>,
    pub(crate) name_to_id: im::OrdMap<Name, Uuid>,
    pub(crate) shortname_to_id: im::OrdMap<(Class, Name), im::OrdSet<Uuid>>,
    pub(crate) globalname_to_id: im::OrdMap<(Class, Name), Uuid>,
    pub(crate) refs_to: im::OrdMap<Uuid, im::OrdMap<(Class, String), im::OrdSet<Uuid>>>,

    generation: usize,
}

const SPECIAL_MODULES: &[&str] = &["__derived__", "__ext_casts__", "__ext_index_matches__"];

impl Schema {
    pub fn has_object(&self, id: &Uuid) -> bool {
        self.id_to_data.contains_key(id)
    }

    pub fn get_object_ids(&self) -> impl ExactSizeIterator<Item = &Uuid> {
        self.id_to_type.keys()
    }

    pub fn get_global_name_ids(&self) -> impl ExactSizeIterator<Item = (Class, &Uuid)> {
        self.globalname_to_id
            .iter()
            .map(|((cls, _name), id)| (*cls, id))
    }

    pub fn get_by_id(&self, id: Uuid) -> Option<Object> {
        let class = *self.id_to_type.get(&id)?;
        Some(Object::new(class, id))
    }

    pub fn get_by_name(&self, name: &Name) -> Option<Object> {
        let id = *self.name_to_id.get(name)?;
        let cls = self.id_to_type.get(&id)?;
        Some(Object::new(*cls, id))
    }

    pub fn get_by_global_name(&self, cls: Class, name: Name) -> Option<Object> {
        assert!(name.module.is_none());

        let id = *self.globalname_to_id.get(&(cls, name))?;
        Some(Object::new(cls, id))
    }

    pub fn get_by_short_name(&self, cls: Class, name: Name) -> impl Iterator<Item = Object> {
        let ids = self.shortname_to_id.get(&(cls, name));

        ids.into_iter()
            .flatten()
            .map(move |id| Object::new(cls, *id))
    }

    pub fn get_data(&self, obj: &Object) -> Option<&[Value]> {
        Some(self.id_to_data.get(&obj.id)?)
    }

    fn has_module(&self, name: &str) -> bool {
        self.get_by_global_name(Class::Module, Name::new_unqualified(name.to_string()))
            .is_some()
    }

    pub fn add(&mut self, obj: &Object, data: Vec<Value>) -> Result<(), String> {
        let structures = structure::get_structures();
        let cls_structure = structures.classes.get(obj.class.as_ref()).unwrap();

        let name_field = cls_structure.fields.get("name").unwrap();
        let Value::Name(name) = &data[name_field.index] else {
            panic!()
        };

        // name must not exist
        if let Some(id) = self.name_to_id.get(name) {
            let other_obj = self.get_by_id(*id);
            // TODO: verbose name
            // let vn = other_obj.get_verbosename(self, with_parent=True);
            return Err(format!("{name:?} already exists: {other_obj:?}"));
        }

        // id must not exist
        if let Some(id) = self.id_to_type.get(&obj.id) {
            let cls = obj.class;
            return Err(format!(
                "{cls:?} ({id:?}) is already present in the schema {self:?}"
            ));
        }

        // update refs
        let new_refs = collect_refs(cls_structure, &data);
        if !new_refs.is_empty() {
            self.update_refs_to(obj, &cls_structure.fields, Default::default(), new_refs);
        }

        self.update_name(obj, None, Some(name))?;

        if let Some(module) = &name.module
            && !self.has_module(module)
            && !SPECIAL_MODULES.contains(&module.as_str())
        {
            return Err(format!("module {} is not in this schema", &module));
        }

        self.id_to_type.insert(obj.id, obj.class);
        self.id_to_data.insert(obj.id, data);
        Ok(())
    }

    pub fn delist(&mut self, name: &Name) {
        self.name_to_id.remove(name);
    }

    pub fn delete(&mut self, obj: &Object) -> Result<(), String> {
        let Some(data) = self.id_to_data.get(&obj.id) else {
            return Err(format!("cannot delete {obj:?}: not in this schema"));
        };

        let structures = structure::get_structures();
        let cls_structure = structures.classes.get(obj.class.as_ref()).unwrap();

        let name_field = cls_structure.fields.get("name").unwrap();
        let Value::Name(name) = data[name_field.index].clone() else {
            panic!()
        };

        self.update_name(obj, Some(&name), None)?;

        let data = self.id_to_data.get(&obj.id).unwrap();

        let orig_refs = collect_refs(cls_structure, data);
        if !orig_refs.is_empty() {
            self.update_refs_to(obj, &cls_structure.fields, orig_refs, Default::default());
        }

        self.id_to_data.remove(&obj.id);
        self.id_to_type.remove(&obj.id);
        Ok(())
    }

    pub fn set_field(&mut self, obj: Object, fieldname: &str, value: Value) -> Result<(), String> {
        let Some(data) = self.id_to_data.get(&obj.id) else {
            let obj_id = obj.id;
            return Err(format!(
                "cannot set {fieldname} value: item {obj_id:?} is not present in the schema {self:?}"
            ));
        };

        let structures = structure::get_structures();
        let cls_structure = structures.classes.get(obj.class.as_ref()).unwrap();

        let field = cls_structure.fields.get(fieldname).unwrap();
        if fieldname == "name" {
            let Value::Name(old_name) = data[field.index].clone() else {
                panic!()
            };
            let Value::Name(new_name) = &value else {
                panic!()
            };

            self.update_name(&obj, Some(&old_name), Some(new_name))?;
        }

        let mut new_refs = HashMap::new();
        new_refs.insert(fieldname, value.ref_ids().collect());

        let data_ref = self.id_to_data.get_mut(&obj.id).unwrap();
        let mut data_new = data_ref.clone();
        let old_value = std::mem::replace(&mut data_new[field.index], value);
        *data_ref = data_new;

        let mut orig_refs = HashMap::new();
        orig_refs.insert(fieldname, old_value.ref_ids().collect());

        self.update_refs_to(&obj, &cls_structure.fields, orig_refs, new_refs);

        Ok(())
    }

    pub fn unset_field(&mut self, obj: Object, fieldname: &str) -> Result<(), String> {
        let Some(data) = self.id_to_data.get(&obj.id) else {
            return Ok(());
        };

        let structures = structure::get_structures();
        let cls_structure = structures.classes.get(obj.class.as_ref()).unwrap();

        let field = cls_structure.fields.get(fieldname).unwrap();

        let Some(orig_value) = data.get(field.index) else {
            return Ok(());
        };

        if matches!(orig_value, Value::None) {
            return Ok(());
        }

        let mut orig_refs = HashMap::new();
        orig_refs.insert(fieldname, orig_value.ref_ids().collect());

        if fieldname == "name" {
            let Value::Name(orig_value) = orig_value.clone() else {
                panic!()
            };
            self.update_name(&obj, Some(&orig_value), None)?;
        }

        let data = self.id_to_data.get_mut(&obj.id).unwrap();
        data[field.index] = Value::None;

        self.update_refs_to(&obj, &cls_structure.fields, orig_refs, Default::default());

        Ok(())
    }

    pub fn set_fields(
        &mut self,
        obj: Object,
        updates: HashMap<String, Value>,
    ) -> Result<(), String> {
        if updates.is_empty() {
            return Ok(());
        }

        let structures = structure::get_structures();
        let cls_structure = structures.classes.get(obj.class.as_ref()).unwrap();

        let mut data = self.id_to_data.get(&obj.id).cloned().unwrap_or_default();

        let mut orig_refs = HashMap::new();
        let mut new_refs = HashMap::new();
        for (fieldname, value) in updates {
            let (fieldname, field) = cls_structure.fields.get_key_value(&fieldname).unwrap();

            orig_refs.insert(fieldname.as_str(), data[field.index].ref_ids().collect());
            new_refs.insert(fieldname.as_str(), value.ref_ids().collect());

            if fieldname == "name" {
                let Value::Name(old_name) = data[field.index].clone() else {
                    panic!()
                };
                let Value::Name(new_name) = &value else {
                    panic!()
                };

                self.update_name(&obj, Some(&old_name), Some(new_name))?;
            }

            data[field.index] = value;
        }

        self.id_to_data.insert(obj.id, data);

        self.update_refs_to(&obj, &cls_structure.fields, orig_refs, new_refs);
        Ok(())
    }

    fn update_name(
        &mut self,
        obj: &Object,
        old_name: Option<&Name>,
        new_name: Option<&Name>,
    ) -> Result<(), String> {
        let is_global_name = !obj.class.is_qualified();

        let has_sn_cache = matches!(obj.class, Class::Function | Class::Operator);

        if let Some(old_name) = old_name {
            if is_global_name {
                self.globalname_to_id.remove(&(obj.class, old_name.clone()));
            } else {
                self.name_to_id.remove(old_name);
            }
            if has_sn_cache {
                let old_shortname = old_name.clone().fullname_into_shortname();
                let sn_key = (obj.class, old_shortname);

                if let Some(ids) = self.shortname_to_id.get_mut(&sn_key) {
                    ids.remove(&obj.id);
                }
            }
        }

        if let Some(new_name) = new_name {
            if is_global_name {
                let key = (obj.class, new_name.clone());
                if let Some(other_id) = self.globalname_to_id.get(&key) {
                    let other_obj = self.get_by_id(*other_id).unwrap();
                    // TODO: verbose name
                    // vn = other_obj.get_verbosename(self, with_parent=True)
                    let cls = obj.class;
                    return Err(format!(
                        "{cls:?} {new_name:?} already exists: {other_obj:?}"
                    ));
                }

                self.globalname_to_id.insert(key, obj.id);
            } else {
                if let Some(module) = &new_name.module
                    && !self.has_module(module)
                    && !SPECIAL_MODULES.contains(&module.as_str())
                {
                    return Err(format!("module {module} is not in this schema"));
                }

                if let Some(other_id) = self.name_to_id.get(new_name) {
                    let other_obj = self.get_by_id(*other_id).unwrap();
                    // TODO: verbose name
                    // vn = other_obj.get_verbosename(self, with_parent=True)
                    return Err(format!("{new_name:?} already exists: {other_obj:?}"));
                }
                self.name_to_id.insert(new_name.clone(), obj.id);
            }

            if has_sn_cache {
                let new_shortname = new_name.clone().fullname_into_shortname();

                let sn_key = (obj.class, new_shortname.clone());

                let entries = self.shortname_to_id.entry(sn_key).or_default();
                entries.insert(obj.id);
            }
        }
        Ok(())
    }

    fn update_refs_to(
        &mut self,
        obj: &Object,
        obj_fields: &HashMap<String, Field>,
        mut to_remove: HashMap<&str, HashSet<Uuid>>,
        mut to_add: HashMap<&str, HashSet<Uuid>>,
    ) {
        for (field_name, _) in obj_fields {
            let mut to_add = to_add.remove(field_name.as_str()).unwrap_or_default();

            let mut to_remove = to_remove.remove(field_name.as_str()).unwrap_or_default();

            let intersection: HashSet<_> = to_add.intersection(&to_remove).cloned().collect();
            to_add.retain(|i| !intersection.contains(i));
            to_remove.retain(|i| !intersection.contains(i));

            let key = (obj.class, field_name.clone());

            for ref_id in to_add {
                let refs = self.refs_to.entry(ref_id).or_default();

                let field_refs = refs.entry(key.clone()).or_default();
                field_refs.insert(obj.id);
            }

            for ref_id in to_remove {
                let refs = self.refs_to.get_mut(&ref_id).unwrap();
                if let Some(field_refs) = refs.get_mut(&key) {
                    field_refs.remove(&obj.id);
                }
            }
        }
    }

    pub fn get_referrers(
        &self,
        obj: Object,
        referrer_class: Option<Class>,
        referrer_field: Option<&str>,
    ) -> HashSet<Object> {
        let Some(refs) = self.refs_to.get(&obj.id) else {
            return HashSet::new();
        };

        let mut referrers = HashSet::new();
        match (referrer_class, referrer_field) {
            (Some(class), Some(field)) => {
                for ((cls, f), ids) in refs {
                    if cls.is_subclass(&class) && f == field {
                        referrers.extend(ids.iter().map(|id| self.get_by_id(*id).unwrap()));
                    }
                }
            }
            (Some(class), None) => {
                for ((cls, _), ids) in refs {
                    if cls.is_subclass(&class) {
                        referrers.extend(ids.iter().map(|id| self.get_by_id(*id).unwrap()));
                    }
                }
            }
            (None, Some(field_name)) => {
                for ((_, f), ids) in refs {
                    if f == field_name {
                        referrers.extend(ids.iter().map(|id| self.get_by_id(*id).unwrap()));
                    }
                }
            }
            (None, None) => {
                referrers.extend(
                    refs.values()
                        .flatten()
                        .map(|id| self.get_by_id(*id).unwrap()),
                );
            }
        }
        referrers
    }

    pub fn get_referrers_ex(
        &self,
        obj: Object,
        referrer_class: Option<Class>,
    ) -> HashMap<(Class, String), HashSet<Object>> {
        let Some(refs) = self.refs_to.get(&obj.id) else {
            return HashMap::new();
        };

        let mut result = HashMap::new();
        if let Some(scls_type) = referrer_class {
            for ((cls, f), ids) in refs {
                if cls.is_subclass(&scls_type) {
                    result.insert(
                        (*cls, f.clone()),
                        ids.iter().map(|i| self.get_by_id(*i).unwrap()).collect(),
                    );
                }
            }
        } else {
            for ((cls, f), ids) in refs {
                result.insert(
                    (*cls, f.clone()),
                    ids.iter().map(|i| self.get_by_id(*i).unwrap()).collect(),
                );
            }
        }
        result
    }
}

fn collect_refs<'s>(
    cls_structure: &'s structure::ClassStructure,
    data: &[Value],
) -> HashMap<&'s str, HashSet<Uuid>> {
    let mut refs = HashMap::new();
    for (field_name, field) in &cls_structure.fields {
        if let Some(val) = data.get(field.index) {
            let ids: HashSet<_> = val.ref_ids().collect();
            if !ids.is_empty() {
                refs.insert(field_name.as_str(), ids);
            }
        }
    }
    refs
}
