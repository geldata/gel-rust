# Rebuild the cached config schema. This query is used to generate the schema.json.gz file.

with

# This is a list of all the config objects in the schema.
cfg_introspection := (
    with cfgs := {
        (select schema::ObjectType { name, children := .<ancestors[is schema::ObjectType] }
            filter .name = 'cfg::AbstractConfig').children.id,
        (select schema::ObjectType { id }
            filter .name = 'cfg::AbstractConfig').id
    },

    cfg_objects := (select schema::ObjectType {
        children := .<ancestors[is schema::ObjectType]
    } filter (
        .name = 'cfg::AbstractConfig' OR .name = 'cfg::ExtensionConfig' OR .name = 'cfg::ConfigObject'))
    .children,

    cfg_links := (select cfg_objects.links.id),

    O := schema::ObjectType,

    select cfg_objects {
        id, name,
        properties: {target: {[is schema::ScalarType].enum_values} }
            filter .name != 'id',
        links
        filter .target.id not in cfgs and .name != '__type__',
    } filter .abstract = false
),

# This is a list of the config roots
cfg_roots := (select cfg_introspection filter 'cfg::AbstractConfig' in .ancestors.name order by .name),

# Construct the schema.
select {
    roots := cfg_roots { name },
    types := (
        select cfg_introspection {
            name,
            ancestors: { name },
            properties: {
                name,
                multi := true if .cardinality = schema::Cardinality.Many else false,
                default,
                readonly,
                required,
                protected,
                target: { name, enum_values },
                constraints: { name, params: { name, value := @value } filter .name != '__subject__' },
                annotations: { name, value := @value }
            },
            links: {
                name,
                multi := true if .cardinality = schema::Cardinality.Many else false,
                readonly,
                required,
                target: { name },
                constraints: { name, params: { name, value := @value } filter .name != '__subject__' },
                annotations: { name, value := @value }
            }
        } order by .name
    )
};
