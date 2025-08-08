configure current database reset ext::auth::AuthConfig::providers;
configure current database insert ext::auth::EmailPasswordProviderConfig {
    require_verification := <std::bool>false
};
