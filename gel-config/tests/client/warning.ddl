# Coercing value for instance.config.current_email_provider_name to std::str
# Coercing value for instance.config.http_max_connections to std::int64
# Unexpected key: unknown-key
configure instance set current_email_provider_name := <std::str>10;
configure instance set http_max_connections := <std::int64>'100';
configure current database reset ext::auth::AuthConfig::providers;
configure current database insert ext::auth::AppleOAuthProvider {
    additional_scope := <std::str>'email name',
    client_id := <std::str>'YOUR_APPLE_CLIENT_ID',
    secret := <std::str>'YOUR_APPLE_SECRET'
};
