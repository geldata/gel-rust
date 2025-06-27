Gel Rust Binding
===================

This workspace is a collection of Rust crates for the Gel database. Individual
docs can currently be found on docs.rs:

| Crate | Source | Description |
|-------|--------|-------------|
| [gel-auth](https://docs.rs/gel-auth) | [Source](./gel-auth) | Authentication and authorization for the Gel database. |
| [gel-babelfish](https://docs.rs/gel-babelfish) | [Source](./gel-babelfish) | Babelfish is a Gel socket frontend that speaks Gel, Postgres, HTTP and more. |
| [gel-captive](https://docs.rs/gel-captive) | [Source](./gel-captive) | Run a captive Gel server for testing purposes. |
| [gel-derive](https://docs.rs/gel-derive) | [Source](./gel-derive) | Derive macros for Gel database client. |
| [gel-db-protocol](https://docs.rs/gel-db-protocol) | [Source](./gel-db-protocol) | Low-level protocol implementation of the EdgeDB/Gel wire protocol. |
| [gel-dsn](https://docs.rs/gel-dsn) | [Source](./gel-dsn) | Data-source name (DSN) parser for Gel and PostgreSQL databases. |
| [gel-errors](https://docs.rs/gel-errors) | [Source](./gel-errors) | Error types for Gel database client. |
| [gel-jwt](https://docs.rs/gel-jwt) | [Source](./gel-jwt) | JWT implementation for the Gel database. |
| [gel-pg-captive](https://docs.rs/gel-pg-captive) | [Source](./gel-pg-captive) | Run a captive PostgreSQL server for testing purposes. |
| [gel-pg-protocol](https://docs.rs/gel-pg-protocol) | [Source](./gel-pg-protocol) | The Gel implementation of the PostgreSQL wire protocol. |
| [gel-protocol](https://docs.rs/gel-protocol) | [Source](./gel-protocol) | Low-level protocol implementation for Gel database client. |
| [gel-protogen](https://docs.rs/gel-protogen) | [Source](./gel-protogen) | Macros to make parsing and serializing of PostgreSQL-like protocols easier. |
| [gel-protogen-proc-macros](https://docs.rs/gel-protogen-proc-macros) | [Source](./gel-protogen-proc-macros) | Macros to make parsing and serializing of PostgreSQL-like protocols easier. |
| [gel-stream](https://docs.rs/gel-stream) | [Source](./gel-stream) | A library for streaming data between clients and servers. |
| [gel-tokio](https://docs.rs/gel-tokio) | [Source](./gel-tokio) | Gel database client implementation for tokio. |

Running Tests
=============

`cargo test --all-features` will test most features of the creates in this
repository, however some feature combinations will not be tested. See
[justfile](./justfile) for all commands to run the complete feature matrix test
suite.

Publishing
==========

To publish a crate, run `./tools/publish.sh <crate>`. The script will
automatically determine version bumps and publish the crate.

License
=======

Licensed under either of

* Apache License, Version 2.0,
  (./LICENSE-APACHE or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license (./LICENSE-MIT or http://opensource.org/licenses/MIT)

at your option.
