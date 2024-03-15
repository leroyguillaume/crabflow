use std::fmt::{Debug, Formatter};

#[derive(clap::Args, Clone, Eq, PartialEq)]
pub struct DatabaseOptions {
    #[arg(
        long = "database-host",
        env = "DATABASE_HOST",
        help = "Database host",
        default_value = "localhost"
    )]
    pub host: String,
    #[arg(
        long = "database-name",
        env = "DATABASE_NAME",
        help = "Database name",
        default_value = "crabflow"
    )]
    pub name: String,
    #[arg(
        long = "database-password",
        env = "DATABASE_PASSWORD",
        help = "Database password"
    )]
    pub password: String,
    #[arg(
        long = "database-port",
        env = "DATABASE_PORT",
        help = "Database port",
        default_value_t = 5432
    )]
    pub port: u16,
    #[arg(
        long = "database-user",
        env = "DATABASE_USER",
        help = "Database user",
        default_value = "crabflow"
    )]
    pub user: String,
}

impl Debug for DatabaseOptions {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        f.debug_struct("DatabaseOptions")
            .field("host", &self.host)
            .field("name", &self.name)
            .field("port", &self.port)
            .field("user", &self.user)
            .finish()
    }
}

#[cfg(feature = "db")]
impl From<DatabaseOptions> for sqlx::postgres::PgConnectOptions {
    fn from(opts: DatabaseOptions) -> Self {
        Self::new()
            .database(&opts.name)
            .host(&opts.host)
            .password(&opts.password)
            .port(opts.port)
            .username(&opts.user)
    }
}
