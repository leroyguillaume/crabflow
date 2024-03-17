#[derive(clap::Args, Clone, Debug, Default, Eq, PartialEq)]
pub struct DatabaseOptions {
    #[arg(
        long = "database-host",
        env = "DATABASE_HOST",
        help = "Database host",
        default_value = "localhost"
    )]
    pub host: Option<String>,
    #[arg(
        long = "database-name",
        env = "DATABASE_NAME",
        help = "Database name",
        default_value = "crabflow"
    )]
    pub name: Option<String>,
    #[arg(
        long = "database-password",
        env = "DATABASE_PASSWORD",
        help = "Database password"
    )]
    pub password: Option<String>,
    #[arg(long = "database-port", env = "DATABASE_PORT", help = "Database port")]
    pub port: Option<u16>,
    #[arg(
        long = "database-user",
        env = "DATABASE_USER",
        help = "Database user",
        default_value = "crabflow"
    )]
    pub user: Option<String>,
}

#[cfg(feature = "db")]
impl From<DatabaseOptions> for sqlx::postgres::PgConnectOptions {
    fn from(opts: DatabaseOptions) -> Self {
        let pg_opts = Self::new();
        let pg_opts = if let Some(host) = opts.host {
            pg_opts.host(&host)
        } else {
            pg_opts
        };
        let pg_opts = if let Some(name) = opts.name {
            pg_opts.database(&name)
        } else {
            pg_opts
        };
        let pg_opts = if let Some(password) = opts.password {
            pg_opts.password(&password)
        } else {
            pg_opts
        };
        let pg_opts = if let Some(port) = opts.port {
            pg_opts.port(port)
        } else {
            pg_opts
        };
        if let Some(user) = opts.user {
            pg_opts.username(&user)
        } else {
            pg_opts
        }
    }
}
