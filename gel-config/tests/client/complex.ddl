configure instance set http_max_connections := <std::int64>100;
configure instance reset auth;
configure instance insert cfg::Auth {
    priority := <std::int64>100,
    user := {<std::str>'gel'},
    method := (insert cfg::JWT {
        transports := {<cfg::ConnectionTransport>'HTTP'}
    })
};
configure instance insert cfg::Auth {
    priority := <std::int64>200,
    user := {<std::str>'admin', <std::str>'gel'},
    method := (insert cfg::SCRAM {
        transports := {<cfg::ConnectionTransport>'TCP', <cfg::ConnectionTransport>'HTTP'}
    })
};
configure current database set allow_user_specified_id := <std::bool>true;
configure current database set query_execution_timeout := <std::duration>'1 minute';
configure current database set session_idle_transaction_timeout := <std::duration>'30 seconds';
configure current database reset email_providers;
configure current database insert cfg::SMTPProviderConfig {
    name := <std::str>'some-other-smtp-provider',
    port := <std::int32>2525,
    validate_certs := <std::bool>false,
    timeout_per_email := <std::duration>'5 minutes',
    timeout_per_attempt := <std::duration>'1 minute'
};
configure current database insert cfg::SMTPProviderConfig {
    name := <std::str>'mailtrap-sandbox',
    port := <std::int32>2525,
    validate_certs := <std::bool>false,
    timeout_per_email := <std::duration>'5 minutes',
    timeout_per_attempt := <std::duration>'1 minute'
};
