use crate::schema2::raw::{
    ConfigSchema, ConfigSchemaObject, ConfigSchemaPropertyBuilder, ConfigSchemaType,
    ConfigSchemaTypeReference,
};
use crate::schema2::ConfigSchemaPrimitiveType;

/// Constructs the schema of most used config options in schema2 format
pub fn default_schema2() -> ConfigSchema {
    let mut types = Vec::new();
    let mut roots = Vec::new();

    // Example: cfg::SMTPProviderConfig
    let smtp_provider_config = ConfigSchemaObject {
        name: "cfg::SMTPProviderConfig".to_string(),
        ancestors: vec![],
        properties: vec![
            ConfigSchemaPropertyBuilder::new()
                .with_name("name".to_string())
                .with_target(ConfigSchemaPrimitiveType::Str.to_schema_type())
                .with_required(true)
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("sender".to_string())
                .with_target(ConfigSchemaPrimitiveType::Str.to_schema_type())
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("host".to_string())
                .with_target(ConfigSchemaPrimitiveType::Str.to_schema_type())
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("port".to_string())
                .with_target(ConfigSchemaPrimitiveType::Int32.to_schema_type())
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("username".to_string())
                .with_target(ConfigSchemaPrimitiveType::Str.to_schema_type())
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("password".to_string())
                .with_target(ConfigSchemaPrimitiveType::Str.to_schema_type())
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("security".to_string())
                .with_target(ConfigSchemaType {
                    name: "cfg::SMTPSecurity".to_string(),
                    enum_values: Some(vec![
                        "PlainText".to_string(),
                        "TLS".to_string(),
                        "STARTTLS".to_string(),
                        "STARTTLSOrPlainText".to_string(),
                    ]),
                })
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("validate_certs".to_string())
                .with_target(ConfigSchemaPrimitiveType::Bool.to_schema_type())
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("timeout_per_email".to_string())
                .with_target(ConfigSchemaPrimitiveType::Duration.to_schema_type())
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("timeout_per_attempt".to_string())
                .with_target(ConfigSchemaPrimitiveType::Duration.to_schema_type())
                .build(),
        ],
        links: vec![],
    };
    types.push(smtp_provider_config);

    // Example: cfg::Config (partial, just email_providers for demo)
    let cfg_config = ConfigSchemaObject {
        name: "cfg::Config".to_string(),
        ancestors: vec![],
        properties: vec![
            ConfigSchemaPropertyBuilder::new()
                .with_name("default_transaction_isolation".to_string())
                .with_target(ConfigSchemaType {
                    name: "sys::TransactionIsolation".to_string(),
                    enum_values: Some(vec![
                        "Serializable".to_string(),
                        "RepeatableRead".to_string(),
                    ]),
                })
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("default_transaction_access_mode".to_string())
                .with_target(ConfigSchemaType {
                    name: "sys::TransactionAccessMode".to_string(),
                    enum_values: Some(vec!["ReadOnly".to_string(), "ReadWrite".to_string()]),
                })
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("default_transaction_deferrable".to_string())
                .with_target(ConfigSchemaType {
                    name: "sys::TransactionDeferrability".to_string(),
                    enum_values: Some(vec!["Deferrable".to_string(), "NotDeferrable".to_string()]),
                })
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("session_idle_transaction_timeout".to_string())
                .with_target(ConfigSchemaPrimitiveType::Duration.to_schema_type())
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("query_execution_timeout".to_string())
                .with_target(ConfigSchemaPrimitiveType::Duration.to_schema_type())
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("email_providers".to_string())
                .with_target(ConfigSchemaType {
                    name: "cfg::SMTPProviderConfig".to_string(),
                    enum_values: None,
                })
                .with_multi(true)
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("current_email_provider_name".to_string())
                .with_target(ConfigSchemaPrimitiveType::Str.to_schema_type())
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("allow_dml_in_functions".to_string())
                .with_target(ConfigSchemaPrimitiveType::Bool.to_schema_type())
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("allow_bare_ddl".to_string())
                .with_target(ConfigSchemaType {
                    name: "cfg::AllowBareDDL".to_string(),
                    enum_values: Some(vec!["AlwaysAllow".to_string(), "NeverAllow".to_string()]),
                })
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("store_migration_sdl".to_string())
                .with_target(ConfigSchemaType {
                    name: "cfg::StoreMigrationSDL".to_string(),
                    enum_values: Some(vec!["AlwaysStore".to_string(), "NeverStore".to_string()]),
                })
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("apply_access_policies".to_string())
                .with_target(ConfigSchemaPrimitiveType::Bool.to_schema_type())
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("apply_access_policies_pg".to_string())
                .with_target(ConfigSchemaPrimitiveType::Bool.to_schema_type())
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("allow_user_specified_id".to_string())
                .with_target(ConfigSchemaPrimitiveType::Bool.to_schema_type())
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("simple_scoping".to_string())
                .with_target(ConfigSchemaPrimitiveType::Bool.to_schema_type())
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("warn_old_scoping".to_string())
                .with_target(ConfigSchemaPrimitiveType::Bool.to_schema_type())
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("cors_allow_origins".to_string())
                .with_target(ConfigSchemaPrimitiveType::Str.to_schema_type())
                .with_multi(true)
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("auto_rebuild_query_cache".to_string())
                .with_target(ConfigSchemaPrimitiveType::Bool.to_schema_type())
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("auto_rebuild_query_cache_timeout".to_string())
                .with_target(ConfigSchemaPrimitiveType::Duration.to_schema_type())
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("query_cache_mode".to_string())
                .with_target(ConfigSchemaType {
                    name: "cfg::QueryCacheMode".to_string(),
                    enum_values: Some(vec![
                        "InMemory".to_string(),
                        "RegInline".to_string(),
                        "PgFunc".to_string(),
                        "Default".to_string(),
                    ]),
                })
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("http_max_connections".to_string())
                .with_target(ConfigSchemaPrimitiveType::Int64.to_schema_type())
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("track_query_stats".to_string())
                .with_target(ConfigSchemaType {
                    name: "cfg::QueryStatsOption".to_string(),
                    enum_values: Some(vec!["None".to_string(), "All".to_string()]),
                })
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("auth".to_string())
                .with_target(ConfigSchemaType {
                    name: "cfg::Auth".to_string(),
                    enum_values: None,
                })
                .with_multi(true)
                .build(),
        ],
        links: vec![],
    };
    types.push(cfg_config);

    // --- Auth UIConfig ---
    let ui_config = ConfigSchemaObject {
        name: "ext::auth::UIConfig".to_string(),
        ancestors: vec![],
        properties: vec![
            ConfigSchemaPropertyBuilder::new()
                .with_name("redirect_to".to_string())
                .with_target(ConfigSchemaPrimitiveType::Str.to_schema_type())
                .with_required(true)
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("redirect_to_on_signup".to_string())
                .with_target(ConfigSchemaPrimitiveType::Str.to_schema_type())
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("flow_type".to_string())
                .with_target(ConfigSchemaType {
                    name: "ext::auth::FlowType".to_string(),
                    enum_values: Some(vec!["PKCE".to_string(), "implicit".to_string()]),
                })
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("app_name".to_string())
                .with_target(ConfigSchemaPrimitiveType::Str.to_schema_type())
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("logo_url".to_string())
                .with_target(ConfigSchemaPrimitiveType::Str.to_schema_type())
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("dark_logo_url".to_string())
                .with_target(ConfigSchemaPrimitiveType::Str.to_schema_type())
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("brand_color".to_string())
                .with_target(ConfigSchemaPrimitiveType::Str.to_schema_type())
                .build(),
        ],
        links: vec![],
    };
    types.push(ui_config);

    // --- Auth WebhookConfig ---
    let webhook_config = ConfigSchemaObject {
        name: "ext::auth::WebhookConfig".to_string(),
        ancestors: vec![],
        properties: vec![
            ConfigSchemaPropertyBuilder::new()
                .with_name("url".to_string())
                .with_target(ConfigSchemaPrimitiveType::Str.to_schema_type())
                .with_required(true)
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("events".to_string())
                .with_target(ConfigSchemaType {
                    name: "ext::auth::WebhookEvent".to_string(),
                    enum_values: Some(vec![
                        "IdentityCreated".to_string(),
                        "IdentityAuthenticated".to_string(),
                        "EmailFactorCreated".to_string(),
                        "EmailVerified".to_string(),
                        "EmailVerificationRequested".to_string(),
                        "PasswordResetRequested".to_string(),
                        "MagicLinkRequested".to_string(),
                    ]),
                })
                .with_required(true)
                .with_multi(true)
                .build(),
            ConfigSchemaPropertyBuilder::new()
                .with_name("signing_secret_key".to_string())
                .with_target(ConfigSchemaPrimitiveType::Str.to_schema_type())
                .build(),
        ],
        links: vec![],
    };
    types.push(webhook_config);

    // --- AI enums ---
    let _ai_api_style_enum = ConfigSchemaType {
        name: "ext::ai::ProviderAPIStyle".to_string(),
        enum_values: Some(vec![
            "OpenAI".to_string(),
            "Anthropic".to_string(),
            "Ollama".to_string(),
        ]),
    };
    types.push(ConfigSchemaObject {
        name: "ext::ai::ProviderAPIStyle".to_string(),
        ancestors: vec![],
        properties: vec![],
        links: vec![],
    });

    roots.push(ConfigSchemaTypeReference {
        name: "cfg::Config".to_string(),
    });

    ConfigSchema { roots, types }
}
