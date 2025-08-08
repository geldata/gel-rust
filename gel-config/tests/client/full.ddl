# Ignoring internal property branch.config.allow_dml_in_functions
configure instance set http_max_connections := <std::int64>100;
configure current database set allow_bare_ddl := <cfg::AllowBareDDL>'NeverAllow';
configure current database set allow_user_specified_id := <std::bool>false;
configure current database set auto_rebuild_query_cache := <std::bool>false;
configure current database set auto_rebuild_query_cache_timeout := <std::duration>'30 seconds';
configure current database set cors_allow_origins := {<std::str>'http://localhost:8000', <std::str>'http://127.0.0.1:8000'};
configure current database set current_email_provider_name := <std::str>'mailtrap_sandbox';
configure current database set ext::ai::Config::indexer_naptime := <std::duration>'5 minutes';
configure current database set ext::auth::AuthConfig::allowed_redirect_urls := {<std::str>'http://localhost:8000', <std::str>'http://testserver'};
configure current database set ext::auth::AuthConfig::app_name := <std::str>'My Project';
configure current database set ext::auth::AuthConfig::auth_signing_key := <std::str>'__GENERATED_UUID__';
configure current database set ext::auth::AuthConfig::brand_color := <std::str>'#0000FF';
configure current database set ext::auth::AuthConfig::dark_logo_url := <std::str>'https://localhost:8000/static/darklogo.png';
configure current database set ext::auth::AuthConfig::logo_url := <std::str>'https://localhost:8000/static/logo.png';
configure current database set ext::auth::AuthConfig::token_time_to_live := <std::duration>'1 hour';
configure current database set query_execution_timeout := <std::duration>'1 minute';
configure current database set session_idle_transaction_timeout := <std::duration>'30 seconds';
configure current database set warn_old_scoping := <std::bool>false;
configure current database reset email_providers;
configure current database insert cfg::SMTPProviderConfig {
    name := <std::str>'mailtrap_sandbox',
    sender := <std::str>'hello@example.com',
    host := <std::str>'sandbox.smtp.mailtrap.io',
    port := <std::int32>2525,
    username := <std::str>'YOUR_USERNAME',
    password := <std::str>'YOUR_PASSWORD',
    validate_certs := <std::bool>false,
    timeout_per_email := <std::duration>'5 minutes',
    timeout_per_attempt := <std::duration>'1 minute'
};
configure current database reset ext::ai::Config::providers;
configure current database insert ext::ai::AnthropicProviderConfig {
    secret := <std::str>'YOUR_API_KEY',
    client_id := <std::str>'optional_client_id',
    api_url := <std::str>'https://api.anthropic.com/v1'
};
configure current database insert ext::ai::CustomProviderConfig {
    secret := <std::str>'YOUR_GEMINI_API_KEY',
    client_id := <std::str>'YOUR_GEMINI_CLIENT_ID',
    api_url := <std::str>'https://generativelanguage.googleapis.com/v1beta/openai',
    name := <std::str>'google_gemini',
    display_name := <std::str>'Google Gemini',
    api_style := <ext::ai::ProviderAPIStyle>'OpenAI'
};
configure current database insert ext::ai::MistralProviderConfig {
    secret := <std::str>'YOUR_API_KEY',
    client_id := <std::str>'optional_client_id',
    api_url := <std::str>'https://api.mistral.ai/v1'
};
configure current database insert ext::ai::OllamaProviderConfig {
    client_id := <std::str>'optional_client_id',
    api_url := <std::str>'http://localhost:11434/api'
};
configure current database insert ext::ai::OpenAIProviderConfig {
    secret := <std::str>'YOUR_API_KEY',
    client_id := <std::str>'optional_client_id',
    api_url := <std::str>'https://api.openai.com/v1'
};
configure current database reset ext::auth::AuthConfig::providers;
configure current database insert ext::auth::AppleOAuthProvider {
    additional_scope := <std::str>'email name',
    client_id := <std::str>'YOUR_APPLE_CLIENT_ID',
    secret := <std::str>'YOUR_APPLE_SECRET'
};
configure current database insert ext::auth::AzureOAuthProvider {
    additional_scope := <std::str>'openid profile email',
    client_id := <std::str>'YOUR_AZURE_CLIENT_ID',
    secret := <std::str>'YOUR_AZURE_SECRET'
};
configure current database insert ext::auth::DiscordOAuthProvider {
    additional_scope := <std::str>'identify email',
    client_id := <std::str>'YOUR_DISCORD_CLIENT_ID',
    secret := <std::str>'YOUR_DISCORD_SECRET'
};
configure current database insert ext::auth::EmailPasswordProviderConfig {
    require_verification := <std::bool>false
};
configure current database insert ext::auth::GitHubOAuthProvider {
    additional_scope := <std::str>'read:user user:email',
    client_id := <std::str>'YOUR_GITHUB_CLIENT_ID',
    secret := <std::str>'YOUR_GITHUB_SECRET'
};
configure current database insert ext::auth::GoogleOAuthProvider {
    additional_scope := <std::str>'openid email profile',
    client_id := <std::str>'YOUR_GOOGLE_CLIENT_ID',
    secret := <std::str>'YOUR_GOOGLE_SECRET'
};
configure current database insert ext::auth::MagicLinkProviderConfig {
    token_time_to_live := <std::duration>'15 minutes'
};
configure current database insert ext::auth::SlackOAuthProvider {
    additional_scope := <std::str>'identity.basic identity.email',
    client_id := <std::str>'YOUR_SLACK_CLIENT_ID',
    secret := <std::str>'YOUR_SLACK_SECRET'
};
configure current database insert ext::auth::WebAuthnProviderConfig {
    relying_party_origin := <std::str>'https://example.com',
    require_verification := <std::bool>true
};
configure current database reset ext::auth::AuthConfig::ui;
configure current database insert ext::auth::UIConfig {
    redirect_to := <std::str>'http://localhost:8000/auth/callback',
    redirect_to_on_signup := <std::str>'http://localhost:8000/auth/callback?isSignUp=true'
};
configure current database reset ext::auth::AuthConfig::webhooks;
configure current database insert ext::auth::WebhookConfig {
    url := <std::str>'https://example.com/webhook',
    events := {<ext::auth::WebhookEvent>'IdentityCreated', <ext::auth::WebhookEvent>'EmailVerified'},
    signing_secret_key := <std::str>'YOUR_WEBHOOK_SECRET'
};
