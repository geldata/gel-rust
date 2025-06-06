use gel_dsn::{postgres::*, SystemEnvVars};

fn main() {
    let dsn = std::env::args().nth(1).expect("No DSN provided");

    let mut params = parse_postgres_dsn_env(&dsn, SystemEnvVars).unwrap();
    #[allow(deprecated)]
    let home = std::env::home_dir().unwrap();
    eprintln!("DSN: {dsn}\n----\n{params:#?}\n");
    params
        .password
        .resolve(Some(&home), &params.hosts, &params.database, &params.user)
        .unwrap();
    eprintln!(
        "Resolved password:\n------------------\n{:#?}\n",
        params.password
    );
    params.ssl.resolve(Some(&home)).unwrap();
    eprintln!("Resolved SSL:\n-------------\n{:#?}\n", ());
}
