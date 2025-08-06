use crate::schema::*;
use crate::PrimitiveType;

/// Constructs the schema of most used config options
pub fn default_schema() -> Schema {
    let mut rv = Schema(Vec::new());
    let schema = rv.ext("auth");

    // auth
    let provider_config = vec![(
        "name".to_string(),
        Pointer::new(primitive(PrimitiveType::String)).required(),
    )];

    let oauth_provider_config = ObjectType::new(provider_config.clone().into_iter().chain([
        (
            "secret".into(),
            Pointer::new(primitive(PrimitiveType::String)).required(),
        ),
        (
            "client_id".into(),
            Pointer::new(primitive(PrimitiveType::String)).required(),
        ),
        (
            "display_name".into(),
            Pointer::new(primitive(PrimitiveType::String)).required(),
        ),
        (
            "additional_scope".into(),
            Pointer::new(primitive(PrimitiveType::String)),
        ),
    ]));

    let openid_connect_provider_config = ObjectType::new(
        provider_config
            .clone()
            .into_iter()
            .chain(oauth_provider_config.pointers.clone())
            .chain([
                (
                    "issuer_url".into(),
                    Pointer::new(primitive(PrimitiveType::String)).required(),
                ),
                (
                    "logo_url".into(),
                    Pointer::new(primitive(PrimitiveType::String)),
                ),
            ]),
    );
    let vendor_oauth_provider_config = ObjectType::new(
        oauth_provider_config
            .pointers
            .clone()
            .into_iter()
            .optional("name")
            .optional("display_name"),
    );

    let email_password_provider_config = ObjectType::new(vec![
        (
            "name".to_string(),
            Pointer::new(primitive(PrimitiveType::String)),
        ),
        (
            "require_verification".into(),
            Pointer::new(primitive(PrimitiveType::Boolean)),
        ),
    ]);
    let web_authn_provider_config = ObjectType::new(vec![
        (
            "name".to_string(),
            Pointer::new(primitive(PrimitiveType::String)),
        ),
        (
            "relying_party_origin".into(),
            Pointer::new(primitive(PrimitiveType::String)).required(),
        ),
        (
            "require_verification".into(),
            Pointer::new(primitive(PrimitiveType::Boolean)),
        ),
    ]);
    let magic_link_provider_config = ObjectType::new(vec![
        (
            "name".to_string(),
            Pointer::new(primitive(PrimitiveType::String)),
        ),
        (
            "token_time_to_live".into(),
            Pointer::new(primitive(PrimitiveType::Duration)),
        ),
    ]);

    let auth_providers = Type::new_union(vec![
        schema.register("ext::auth::OAuthProviderConfig", oauth_provider_config),
        schema.register(
            "ext::auth::OpenIDConnectProvider",
            openid_connect_provider_config,
        ),
        schema.register(
            "ext::auth::AppleOAuthProvider",
            vendor_oauth_provider_config.clone(),
        ),
        schema.register(
            "ext::auth::AzureOAuthProvider",
            vendor_oauth_provider_config.clone(),
        ),
        schema.register(
            "ext::auth::DiscordOAuthProvider",
            vendor_oauth_provider_config.clone(),
        ),
        schema.register(
            "ext::auth::SlackOAuthProvider",
            vendor_oauth_provider_config.clone(),
        ),
        schema.register(
            "ext::auth::GitHubOAuthProvider",
            vendor_oauth_provider_config.clone(),
        ),
        schema.register(
            "ext::auth::GoogleOAuthProvider",
            vendor_oauth_provider_config.clone(),
        ),
        schema.register(
            "ext::auth::EmailPasswordProviderConfig",
            email_password_provider_config,
        ),
        schema.register(
            "ext::auth::WebAuthnProviderConfig",
            web_authn_provider_config,
        ),
        schema.register(
            "ext::auth::MagicLinkProviderConfig",
            magic_link_provider_config,
        ),
    ]);
    let ui_config = schema.register(
        "ext::auth::UIConfig",
        ObjectType::new([
            (
                "redirect_to",
                Pointer::new(primitive(PrimitiveType::String)).required(),
            ),
            (
                "redirect_to_on_signup",
                Pointer::new(primitive(PrimitiveType::String)),
            ),
            (
                "flow_type",
                Pointer::new(enumeration("ext::auth::FlowType", ["PKCE", "implicit"])),
            ),
            ("app_name", Pointer::new(primitive(PrimitiveType::String))),
            ("logo_url", Pointer::new(primitive(PrimitiveType::String))),
            (
                "dark_logo_url",
                Pointer::new(primitive(PrimitiveType::String)),
            ),
            (
                "brand_color",
                Pointer::new(primitive(PrimitiveType::String)),
            ),
        ]),
    );
    let webhooks_config = schema.register(
        "ext::auth::WebhookConfig",
        ObjectType::new([
            (
                "url",
                Pointer::new(primitive(PrimitiveType::String)).required(),
            ),
            (
                "events",
                Pointer::new(enumeration(
                    "ext::auth::WebhookEvent",
                    [
                        "IdentityCreated",
                        "IdentityAuthenticated",
                        "EmailFactorCreated",
                        "EmailVerified",
                        "EmailVerificationRequested",
                        "PasswordResetRequested",
                        "MagicLinkRequested",
                    ],
                ))
                .multi()
                .required(),
            ),
            (
                "signing_secret_key",
                Pointer::new(primitive(PrimitiveType::String)),
            ),
        ]),
    );
    schema.register(
        "ext::auth::AuthConfig",
        ObjectType::new([
            ("providers", Pointer::new(auth_providers).multi()),
            ("ui", Pointer::new(ui_config)),
            ("webhooks", Pointer::new(webhooks_config).multi()),
            ("app_name", Pointer::new(primitive(PrimitiveType::String))),
            ("logo_url", Pointer::new(primitive(PrimitiveType::String))),
            (
                "dark_logo_url",
                Pointer::new(primitive(PrimitiveType::String)),
            ),
            (
                "brand_color",
                Pointer::new(primitive(PrimitiveType::String)),
            ),
            (
                "auth_signing_key",
                Pointer::new(primitive(PrimitiveType::String)),
            ),
            (
                "token_time_to_live",
                Pointer::new(primitive(PrimitiveType::Duration)),
            ),
            (
                "allowed_redirect_urls",
                Pointer::new(primitive(PrimitiveType::String)).multi(),
            ),
        ]),
    );

    // AI
    let schema = rv.ext("ai");
    let provider_config = vec![
        (
            "name".to_string(),
            Pointer::new(primitive(PrimitiveType::String)).required(),
        ),
        (
            "display_name".to_string(),
            Pointer::new(primitive(PrimitiveType::String)).required(),
        ),
        (
            "api_url".to_string(),
            Pointer::new(primitive(PrimitiveType::String)).required(),
        ),
        (
            "client_id".to_string(),
            Pointer::new(primitive(PrimitiveType::String)),
        ),
        (
            "secret".to_string(),
            Pointer::new(primitive(PrimitiveType::String)).required(),
        ),
        (
            "api_style".to_string(),
            Pointer::new(enumeration(
                "ext::ai::ProviderAPIStyle",
                ["OpenAI", "Anthropic", "Ollama"],
            ))
            .required(),
        ),
    ];
    let vendor_provider_config = provider_config
        .clone()
        .into_iter()
        .optional("name")
        .optional("display_name")
        .optional("api_url")
        .optional("api_style");

    let ai_providers = Type::new_union(vec![
        schema.register(
            "ext::ai::CustomProviderConfig",
            ObjectType::new(
                provider_config
                    .clone()
                    .into_iter()
                    .optional("display_name")
                    .optional("api_style"),
            ),
        ),
        schema.register(
            "ext::ai::OpenAIProviderConfig",
            ObjectType::new(vendor_provider_config.clone()),
        ),
        schema.register(
            "ext::ai::MistralProviderConfig",
            ObjectType::new(vendor_provider_config.clone()),
        ),
        schema.register(
            "ext::ai::AnthropicProviderConfig",
            ObjectType::new(vendor_provider_config.clone()),
        ),
        schema.register(
            "ext::ai::OllamaProviderConfig",
            ObjectType::new(vendor_provider_config.optional("secret")),
        ),
    ]);

    schema.register(
        "ext::ai::Config",
        ObjectType::new([
            (
                "indexer_naptime",
                Pointer::new(primitive(PrimitiveType::Duration)),
            ),
            ("providers", Pointer::new(ai_providers).multi()),
        ]),
    );

    let schema = rv.std();

    // email provider
    let email_provider_config = vec![(
        "name",
        Pointer::new(primitive(PrimitiveType::String)).required(),
    )];

    let smtp_provider_config = schema.register(
        "cfg::SMTPProviderConfig",
        ObjectType::new(email_provider_config.clone().into_iter().chain([
            ("sender", Pointer::new(primitive(PrimitiveType::String))),
            ("host", Pointer::new(primitive(PrimitiveType::String))),
            ("port", Pointer::new(primitive(PrimitiveType::Int32))),
            ("username", Pointer::new(primitive(PrimitiveType::String))),
            ("password", Pointer::new(primitive(PrimitiveType::String))),
            (
                "security",
                Pointer::new(enumeration(
                    "cfg::SMTPSecurity",
                    ["PlainText", "TLS", "STARTTLS", "STARTTLSOrPlainText"],
                )),
            ),
            (
                "validate_certs",
                Pointer::new(primitive(PrimitiveType::Boolean)),
            ),
            (
                "timeout_per_email",
                Pointer::new(primitive(PrimitiveType::Duration)),
            ),
            (
                "timeout_per_attempt",
                Pointer::new(primitive(PrimitiveType::Duration)),
            ),
        ])),
    );

    // cfg::Auth
    let transport = enumeration("cfg::ConnectionTransport", ["TCP", "TCP_PG", "HTTP"]);
    let cfg_auth_method = Type::new_union(vec![
        schema.register(
            "cfg::Trust",
            ObjectType::new([("transports", Pointer::new(transport.clone()).multi())]),
        ),
        schema.register(
            "cfg::SCRAM",
            ObjectType::new([("transports", Pointer::new(transport.clone()).multi())]),
        ),
        schema.register(
            "cfg::JWT",
            ObjectType::new([("transports", Pointer::new(transport).multi())]),
        ),
    ]);

    let cfg_auth = schema.register(
        "cfg::Auth",
        ObjectType::new([
            (
                "priority",
                Pointer::new(primitive(PrimitiveType::Int64)).required(),
            ),
            (
                "user",
                Pointer::new(primitive(PrimitiveType::String)).multi(),
            ),
            ("method", Pointer::new(cfg_auth_method).required()),
            ("comment", Pointer::new(primitive(PrimitiveType::String))),
        ]),
    );

    schema.register(
        "cfg::Config",
        ObjectType::new([
            (
                "default_transaction_isolation",
                Pointer::new(enumeration(
                    "sys::TransactionIsolation",
                    ["Serializable", "RepeatableRead"],
                )),
            ),
            (
                "default_transaction_access_mode",
                Pointer::new(enumeration(
                    "sys::TransactionAccessMode",
                    ["ReadOnly", "ReadWrite"],
                )),
            ),
            (
                "default_transaction_deferrable",
                Pointer::new(enumeration(
                    "sys::TransactionDeferrability",
                    ["Deferrable", "NotDeferrable"],
                )),
            ),
            (
                "session_idle_transaction_timeout",
                Pointer::new(primitive(PrimitiveType::Duration)),
            ),
            (
                "query_execution_timeout",
                Pointer::new(primitive(PrimitiveType::Duration)),
            ),
            (
                "email_providers",
                Pointer::new(Type::new_union([smtp_provider_config])).multi(),
            ),
            (
                "current_email_provider_name",
                Pointer::new(primitive(PrimitiveType::String)),
            ),
            (
                "allow_dml_in_functions",
                Pointer::new(primitive(PrimitiveType::Boolean)),
            ),
            (
                "allow_bare_ddl",
                Pointer::new(enumeration(
                    "cfg::AllowBareDDL",
                    ["AlwaysAllow", "NeverAllow"],
                )),
            ),
            (
                "store_migration_sdl",
                Pointer::new(enumeration(
                    "cfg::StoreMigrationSDL",
                    ["AlwaysStore", "NeverStore"],
                )),
            ),
            (
                "apply_access_policies",
                Pointer::new(primitive(PrimitiveType::Boolean)),
            ),
            (
                "apply_access_policies_pg",
                Pointer::new(primitive(PrimitiveType::Boolean)),
            ),
            (
                "allow_user_specified_id",
                Pointer::new(primitive(PrimitiveType::Boolean)),
            ),
            (
                "simple_scoping",
                Pointer::new(primitive(PrimitiveType::Boolean)),
            ),
            (
                "warn_old_scoping",
                Pointer::new(primitive(PrimitiveType::Boolean)),
            ),
            (
                "cors_allow_origins",
                Pointer::new(primitive(PrimitiveType::String)).multi(),
            ),
            (
                "auto_rebuild_query_cache",
                Pointer::new(primitive(PrimitiveType::Boolean)),
            ),
            (
                "auto_rebuild_query_cache_timeout",
                Pointer::new(primitive(PrimitiveType::Duration)),
            ),
            (
                "query_cache_mode",
                Pointer::new(enumeration(
                    "cfg::QueryCacheMode",
                    ["InMemory", "RegInline", "PgFunc", "Default"],
                )),
            ),
            (
                "http_max_connections",
                Pointer::new(primitive(PrimitiveType::Int64)),
            ),
            (
                "track_query_stats",
                Pointer::new(enumeration("cfg::QueryStatsOption", ["None", "All"])),
            ),
            ("auth", Pointer::new(cfg_auth).multi()),
        ]),
    );

    rv
}
