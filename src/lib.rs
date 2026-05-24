//! Configure SQLx connection pools from [`mp_config`].
//!
//! The default property prefix is `datasource`, with named datasources under
//! `datasource.<name>`.

use mp_config::{Config, FromConfigValue};
use std::collections::BTreeMap;
use std::error::Error as StdError;
use std::fmt;
use std::time::Duration;

#[cfg(feature = "mysql")]
use sqlx::MySqlPool;
#[cfg(feature = "postgres")]
use sqlx::PgPool;
#[cfg(feature = "sqlite")]
use sqlx::SqlitePool;
#[cfg(feature = "mysql")]
use sqlx::mysql::MySqlPoolOptions;
#[cfg(feature = "postgres")]
use sqlx::postgres::PgPoolOptions;
#[cfg(feature = "sqlite")]
use sqlx::sqlite::SqlitePoolOptions;

/// Result type returned by this crate.
pub type Result<T> = std::result::Result<T, SqlxConfigError>;

/// Database driver selected for a datasource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseKind {
    /// PostgreSQL via SQLx's `postgres` driver.
    Postgres,
    /// MySQL or MariaDB via SQLx's `mysql` driver.
    MySql,
    /// SQLite via SQLx's `sqlite` driver.
    Sqlite,
}

impl DatabaseKind {
    fn scheme(self) -> &'static str {
        match self {
            Self::Postgres => "postgresql",
            Self::MySql => "mysql",
            Self::Sqlite => "sqlite",
        }
    }
}

impl fmt::Display for DatabaseKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Postgres => formatter.write_str("postgresql"),
            Self::MySql => formatter.write_str("mysql"),
            Self::Sqlite => formatter.write_str("sqlite"),
        }
    }
}

impl FromConfigValue for DatabaseKind {
    fn from_config_value(value: &str) -> std::result::Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "postgres" | "postgresql" | "pgsql" => Ok(Self::Postgres),
            "mysql" | "mariadb" => Ok(Self::MySql),
            "sqlite" | "sqlite3" => Ok(Self::Sqlite),
            other => Err(format!(
                "unsupported database kind `{other}`; expected postgresql, mysql, or sqlite"
            )),
        }
    }
}

/// SQLx pool tuning loaded from configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoolConfig {
    /// Minimum idle connections kept by the pool.
    pub min_size: Option<u32>,
    /// Maximum connections opened by the pool.
    pub max_size: Option<u32>,
    /// Maximum time to wait for a connection from the pool.
    pub acquire_timeout: Option<Duration>,
    /// How long an idle connection can remain in the pool.
    pub idle_timeout: Option<Duration>,
    /// Maximum lifetime of a connection.
    pub max_lifetime: Option<Duration>,
    /// Whether SQLx should test connections before handing them out.
    pub test_before_acquire: Option<bool>,
}

impl PoolConfig {
    /// Loads pool settings from `<prefix>.pool`.
    pub fn from_config_prefix(config: &Config, prefix: &str) -> Result<Self> {
        Ok(Self {
            min_size: optional(config, &property(prefix, "pool.min-size"))?,
            max_size: optional(config, &property(prefix, "pool.max-size"))?,
            acquire_timeout: optional(config, &property(prefix, "pool.acquire-timeout"))?,
            idle_timeout: optional(config, &property(prefix, "pool.idle-timeout"))?,
            max_lifetime: optional(config, &property(prefix, "pool.max-lifetime"))?,
            test_before_acquire: optional(config, &property(prefix, "pool.test-before-acquire"))?,
        })
    }

    #[cfg(feature = "postgres")]
    /// Applies configured values to SQLx [`PgPoolOptions`].
    pub fn apply_pg_options(&self, options: PgPoolOptions) -> PgPoolOptions {
        self.apply_options(options)
    }

    #[cfg(feature = "mysql")]
    /// Applies configured values to SQLx [`MySqlPoolOptions`].
    pub fn apply_mysql_options(&self, options: MySqlPoolOptions) -> MySqlPoolOptions {
        self.apply_options(options)
    }

    #[cfg(feature = "sqlite")]
    /// Applies configured values to SQLx [`SqlitePoolOptions`].
    pub fn apply_sqlite_options(&self, options: SqlitePoolOptions) -> SqlitePoolOptions {
        self.apply_options(options)
    }

    #[cfg(any(feature = "postgres", feature = "mysql", feature = "sqlite"))]
    fn apply_options<DB>(
        &self,
        mut options: sqlx::pool::PoolOptions<DB>,
    ) -> sqlx::pool::PoolOptions<DB>
    where
        DB: sqlx::Database,
    {
        if let Some(min_size) = self.min_size {
            options = options.min_connections(min_size);
        }
        if let Some(max_size) = self.max_size {
            options = options.max_connections(max_size);
        }
        if let Some(acquire_timeout) = self.acquire_timeout {
            options = options.acquire_timeout(acquire_timeout);
        }
        if let Some(idle_timeout) = self.idle_timeout {
            options = options.idle_timeout(idle_timeout);
        }
        if let Some(max_lifetime) = self.max_lifetime {
            options = options.max_lifetime(max_lifetime);
        }
        if let Some(test_before_acquire) = self.test_before_acquire {
            options = options.test_before_acquire(test_before_acquire);
        }
        options
    }
}

/// Configuration for one SQLx datasource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasourceConfig {
    /// Whether this datasource should be created.
    pub enabled: bool,
    /// Database kind used when a URL is assembled from parts.
    pub db_kind: Option<DatabaseKind>,
    /// Complete SQLx connection URL.
    pub url: Option<String>,
    /// Login username used when a URL is assembled from parts.
    pub username: Option<String>,
    /// Login password used when a URL is assembled from parts.
    pub password: Option<String>,
    /// Database server host used when a URL is assembled from parts.
    pub host: Option<String>,
    /// Database server port used when a URL is assembled from parts.
    pub port: Option<u16>,
    /// Database name or SQLite path used when a URL is assembled from parts.
    pub database: Option<String>,
    /// SQLx pool settings.
    pub pool: PoolConfig,
}

impl DatasourceConfig {
    /// Loads the default datasource from the `datasource` prefix.
    pub fn from_config(config: &Config) -> Result<Self> {
        Self::from_config_prefix(config, "datasource")
    }

    /// Loads one datasource from a custom prefix.
    pub fn from_config_prefix(config: &Config, prefix: &str) -> Result<Self> {
        Ok(Self {
            enabled: optional(config, &property(prefix, "enabled"))?.unwrap_or(true),
            db_kind: optional(config, &property(prefix, "db-kind"))?,
            url: optional(config, &property(prefix, "url"))?,
            username: optional(config, &property(prefix, "username"))?,
            password: optional(config, &property(prefix, "password"))?,
            host: optional(config, &property(prefix, "host"))?,
            port: optional(config, &property(prefix, "port"))?,
            database: optional(config, &property(prefix, "database"))?,
            pool: PoolConfig::from_config_prefix(config, prefix)?,
        })
    }

    /// Returns the SQLx URL, assembling one from structured fields if needed.
    pub fn sqlx_url(&self) -> Result<String> {
        if let Some(url) = &self.url {
            return Ok(url.to_owned());
        }

        let db_kind = self.db_kind.ok_or(SqlxConfigError::MissingUrl)?;
        self.sqlx_url_for(db_kind)
    }

    fn sqlx_url_for(&self, db_kind: DatabaseKind) -> Result<String> {
        if let Some(url) = &self.url {
            return Ok(url.to_owned());
        }

        let database = self
            .database
            .as_deref()
            .ok_or(SqlxConfigError::MissingUrl)?;

        if db_kind == DatabaseKind::Sqlite {
            return Ok(
                if database == ":memory:" || database.starts_with("sqlite:") {
                    database.to_owned()
                } else {
                    format!("sqlite://{database}")
                },
            );
        }

        let host = self.host.as_deref().unwrap_or("localhost");
        let mut url = format!("{}://", db_kind.scheme());
        if let Some(username) = &self.username {
            url.push_str(username);
            if let Some(password) = &self.password {
                url.push(':');
                url.push_str(password);
            }
            url.push('@');
        }
        url.push_str(host);
        if let Some(port) = self.port {
            url.push(':');
            url.push_str(&port.to_string());
        }
        url.push('/');
        url.push_str(database);
        Ok(url)
    }

    #[cfg(feature = "postgres")]
    /// Selects this datasource as a PostgreSQL datasource.
    pub fn postgres(&self) -> Result<PostgresDatasource<'_>> {
        self.ensure_kind(DatabaseKind::Postgres)?;
        Ok(PostgresDatasource { config: self })
    }

    #[cfg(feature = "mysql")]
    /// Selects this datasource as a MySQL datasource.
    pub fn mysql(&self) -> Result<MySqlDatasource<'_>> {
        self.ensure_kind(DatabaseKind::MySql)?;
        Ok(MySqlDatasource { config: self })
    }

    #[cfg(feature = "sqlite")]
    /// Selects this datasource as a SQLite datasource.
    pub fn sqlite(&self) -> Result<SqliteDatasource<'_>> {
        self.ensure_kind(DatabaseKind::Sqlite)?;
        Ok(SqliteDatasource { config: self })
    }

    #[cfg(any(feature = "postgres", feature = "mysql", feature = "sqlite"))]
    fn ensure_kind(&self, expected: DatabaseKind) -> Result<()> {
        if let Some(actual) = self.db_kind
            && actual != expected
        {
            return Err(SqlxConfigError::DatabaseKindMismatch { expected, actual });
        }
        Ok(())
    }
}

#[cfg(feature = "postgres")]
/// A datasource selected for PostgreSQL connections.
#[derive(Debug, Clone, Copy)]
pub struct PostgresDatasource<'a> {
    config: &'a DatasourceConfig,
}

#[cfg(feature = "postgres")]
impl PostgresDatasource<'_> {
    /// Connects a PostgreSQL pool.
    pub async fn connect(&self) -> Result<PgPool> {
        if !self.config.enabled {
            return Err(SqlxConfigError::Disabled);
        }

        let url = self.config.sqlx_url_for(DatabaseKind::Postgres)?;
        self.pool_options()
            .connect(&url)
            .await
            .map_err(SqlxConfigError::Sqlx)
    }

    /// Creates a lazy PostgreSQL pool without opening connections immediately.
    pub fn connect_lazy(&self) -> Result<PgPool> {
        if !self.config.enabled {
            return Err(SqlxConfigError::Disabled);
        }

        let url = self.config.sqlx_url_for(DatabaseKind::Postgres)?;
        self.pool_options()
            .connect_lazy(&url)
            .map_err(SqlxConfigError::Sqlx)
    }

    /// Builds SQLx PostgreSQL pool options.
    pub fn pool_options(&self) -> PgPoolOptions {
        self.config.pool.apply_pg_options(PgPoolOptions::new())
    }
}

#[cfg(feature = "mysql")]
/// A datasource selected for MySQL connections.
#[derive(Debug, Clone, Copy)]
pub struct MySqlDatasource<'a> {
    config: &'a DatasourceConfig,
}

#[cfg(feature = "mysql")]
impl MySqlDatasource<'_> {
    /// Connects a MySQL pool.
    pub async fn connect(&self) -> Result<MySqlPool> {
        if !self.config.enabled {
            return Err(SqlxConfigError::Disabled);
        }

        let url = self.config.sqlx_url_for(DatabaseKind::MySql)?;
        self.pool_options()
            .connect(&url)
            .await
            .map_err(SqlxConfigError::Sqlx)
    }

    /// Creates a lazy MySQL pool without opening connections immediately.
    pub fn connect_lazy(&self) -> Result<MySqlPool> {
        if !self.config.enabled {
            return Err(SqlxConfigError::Disabled);
        }

        let url = self.config.sqlx_url_for(DatabaseKind::MySql)?;
        self.pool_options()
            .connect_lazy(&url)
            .map_err(SqlxConfigError::Sqlx)
    }

    /// Builds SQLx MySQL pool options.
    pub fn pool_options(&self) -> MySqlPoolOptions {
        self.config
            .pool
            .apply_mysql_options(MySqlPoolOptions::new())
    }
}

#[cfg(feature = "sqlite")]
/// A datasource selected for SQLite connections.
#[derive(Debug, Clone, Copy)]
pub struct SqliteDatasource<'a> {
    config: &'a DatasourceConfig,
}

#[cfg(feature = "sqlite")]
impl SqliteDatasource<'_> {
    /// Connects a SQLite pool.
    pub async fn connect(&self) -> Result<SqlitePool> {
        if !self.config.enabled {
            return Err(SqlxConfigError::Disabled);
        }

        let url = self.config.sqlx_url_for(DatabaseKind::Sqlite)?;
        self.pool_options()
            .connect(&url)
            .await
            .map_err(SqlxConfigError::Sqlx)
    }

    /// Creates a lazy SQLite pool without opening connections immediately.
    pub fn connect_lazy(&self) -> Result<SqlitePool> {
        if !self.config.enabled {
            return Err(SqlxConfigError::Disabled);
        }

        let url = self.config.sqlx_url_for(DatabaseKind::Sqlite)?;
        self.pool_options()
            .connect_lazy(&url)
            .map_err(SqlxConfigError::Sqlx)
    }

    /// Builds SQLx SQLite pool options.
    pub fn pool_options(&self) -> SqlitePoolOptions {
        self.config
            .pool
            .apply_sqlite_options(SqlitePoolOptions::new())
    }
}

/// Default and named datasource configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasourcesConfig {
    /// Default datasource loaded from the configured root prefix.
    pub default: DatasourceConfig,
    /// Named datasources loaded from `<prefix>.<name>`.
    pub named: BTreeMap<String, DatasourceConfig>,
}

impl DatasourcesConfig {
    /// Loads datasources from the `datasource` prefix.
    pub fn from_config(config: &Config) -> Result<Self> {
        Self::from_config_prefix(config, "datasource")
    }

    /// Loads datasources from a custom root prefix.
    pub fn from_config_prefix(config: &Config, prefix: &str) -> Result<Self> {
        let default = DatasourceConfig::from_config_prefix(config, prefix)?;
        let mut named = BTreeMap::new();

        for name in datasource_names(config, prefix) {
            let datasource =
                DatasourceConfig::from_config_prefix(config, &property(prefix, &name))?;
            named.insert(name, datasource);
        }

        Ok(Self { default, named })
    }

    /// Returns the default datasource for `None`, otherwise a named datasource.
    pub fn get(&self, name: Option<&str>) -> Option<&DatasourceConfig> {
        match name {
            Some(name) => self.named.get(name),
            None => Some(&self.default),
        }
    }

    #[cfg(feature = "postgres")]
    /// Returns the selected PostgreSQL datasource by name.
    pub fn postgres(&self, name: impl AsRef<str>) -> Result<PostgresDatasource<'_>> {
        self.required_named(name.as_ref())?.postgres()
    }

    #[cfg(feature = "postgres")]
    /// Returns the default datasource selected for PostgreSQL.
    pub fn default_postgres(&self) -> Result<PostgresDatasource<'_>> {
        self.default.postgres()
    }

    #[cfg(feature = "mysql")]
    /// Returns the selected MySQL datasource by name.
    pub fn mysql(&self, name: impl AsRef<str>) -> Result<MySqlDatasource<'_>> {
        self.required_named(name.as_ref())?.mysql()
    }

    #[cfg(feature = "mysql")]
    /// Returns the default datasource selected for MySQL.
    pub fn default_mysql(&self) -> Result<MySqlDatasource<'_>> {
        self.default.mysql()
    }

    #[cfg(feature = "sqlite")]
    /// Returns the selected SQLite datasource by name.
    pub fn sqlite(&self, name: impl AsRef<str>) -> Result<SqliteDatasource<'_>> {
        self.required_named(name.as_ref())?.sqlite()
    }

    #[cfg(feature = "sqlite")]
    /// Returns the default datasource selected for SQLite.
    pub fn default_sqlite(&self) -> Result<SqliteDatasource<'_>> {
        self.default.sqlite()
    }

    #[cfg(any(feature = "postgres", feature = "mysql", feature = "sqlite"))]
    fn required_named(&self, name: &str) -> Result<&DatasourceConfig> {
        self.named
            .get(name)
            .ok_or_else(|| SqlxConfigError::UnknownDatasource {
                name: name.to_owned(),
            })
    }
}

/// Error returned while loading or using SQLx datasource configuration.
#[derive(Debug)]
pub enum SqlxConfigError {
    /// The `mp-config` values could not be loaded.
    Config(mp_config::ConfigError),
    /// The datasource is disabled.
    Disabled,
    /// A named datasource was requested but is not configured.
    UnknownDatasource {
        /// Missing datasource name.
        name: String,
    },
    /// A datasource was selected with a concrete database type that conflicts
    /// with its configured `db-kind`.
    DatabaseKindMismatch {
        /// Database kind requested by the caller.
        expected: DatabaseKind,
        /// Database kind loaded from configuration.
        actual: DatabaseKind,
    },
    /// The datasource needs `url` or enough fields to assemble a URL.
    MissingUrl,
    /// SQLx rejected the connection or pool options.
    Sqlx(sqlx::Error),
}

impl fmt::Display for SqlxConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(error) => write!(formatter, "failed to load SQLx config: {error}"),
            Self::Disabled => write!(formatter, "datasource is disabled"),
            Self::UnknownDatasource { name } => {
                write!(formatter, "unknown datasource `{name}`")
            }
            Self::DatabaseKindMismatch { expected, actual } => write!(
                formatter,
                "datasource db-kind is `{actual}`, but `{expected}` was requested"
            ),
            Self::MissingUrl => write!(
                formatter,
                "missing datasource URL; set url or db-kind and database"
            ),
            Self::Sqlx(error) => write!(formatter, "failed to create SQLx datasource: {error}"),
        }
    }
}

impl StdError for SqlxConfigError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Config(error) => Some(error),
            Self::Sqlx(error) => Some(error),
            Self::Disabled
            | Self::UnknownDatasource { .. }
            | Self::DatabaseKindMismatch { .. }
            | Self::MissingUrl => None,
        }
    }
}

fn optional<T>(config: &Config, name: &str) -> Result<Option<T>>
where
    T: FromConfigValue,
{
    config.get_optional(name).map_err(SqlxConfigError::Config)
}

fn property(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else {
        format!("{prefix}.{name}")
    }
}

fn datasource_names(config: &Config, prefix: &str) -> Vec<String> {
    let prefix = if prefix.is_empty() {
        String::new()
    } else {
        format!("{prefix}.")
    };
    let mut names = BTreeMap::new();

    for property_name in config.property_names() {
        let Some(rest) = property_name.strip_prefix(&prefix) else {
            continue;
        };
        let Some((candidate, _)) = rest.split_once('.') else {
            continue;
        };
        if is_default_group(candidate) {
            continue;
        }
        names.insert(candidate.to_owned(), ());
    }

    names.into_keys().collect()
}

fn is_default_group(candidate: &str) -> bool {
    matches!(
        candidate,
        "pool"
            | "enabled"
            | "db-kind"
            | "url"
            | "username"
            | "password"
            | "host"
            | "port"
            | "database"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mp_config::{Config, MapSource};

    #[test]
    fn loads_default_datasource_with_pool_config() {
        let config = Config::builder()
            .add_source(
                MapSource::new("test", 100)
                    .with("datasource.db-kind", "postgresql")
                    .with("datasource.username", "app")
                    .with("datasource.password", "secret")
                    .with("datasource.host", "db")
                    .with("datasource.port", "5433")
                    .with("datasource.database", "service")
                    .with("datasource.pool.min-size", "2")
                    .with("datasource.pool.max-size", "12")
                    .with("datasource.pool.acquire-timeout", "5s")
                    .with("datasource.pool.test-before-acquire", "false"),
            )
            .build();

        let datasource = DatasourceConfig::from_config(&config).unwrap();

        assert_eq!(datasource.db_kind, Some(DatabaseKind::Postgres));
        assert_eq!(
            datasource.sqlx_url().unwrap(),
            "postgresql://app:secret@db:5433/service"
        );
        assert_eq!(datasource.pool.min_size, Some(2));
        assert_eq!(datasource.pool.max_size, Some(12));
        assert_eq!(
            datasource.pool.acquire_timeout,
            Some(Duration::from_secs(5))
        );
        assert_eq!(datasource.pool.test_before_acquire, Some(false));
    }

    #[test]
    fn discovers_named_datasources() {
        let config = Config::builder()
            .add_source(
                MapSource::new("test", 100)
                    .with("datasource.url", "sqlite::memory:")
                    .with("datasource.users.url", "postgresql://localhost/users")
                    .with("datasource.audit.enabled", "false")
                    .with("datasource.audit.db-kind", "sqlite")
                    .with("datasource.audit.database", ":memory:"),
            )
            .build();

        let datasources = DatasourcesConfig::from_config(&config).unwrap();

        assert_eq!(datasources.default.sqlx_url().unwrap(), "sqlite::memory:");
        assert_eq!(
            datasources.named["users"].sqlx_url().unwrap(),
            "postgresql://localhost/users"
        );
        assert!(!datasources.named["audit"].enabled);
        assert_eq!(
            datasources.named.keys().collect::<Vec<_>>(),
            vec!["audit", "users"]
        );
    }

    #[test]
    fn supports_custom_prefixes() {
        let config = Config::builder()
            .add_source(
                MapSource::new("test", 100)
                    .with("app.datasource.url", "postgresql://localhost/default")
                    .with(
                        "app.datasource.inventory.url",
                        "postgresql://localhost/inventory",
                    ),
            )
            .build();

        let datasources = DatasourcesConfig::from_config_prefix(&config, "app.datasource").unwrap();

        assert_eq!(
            datasources.default.sqlx_url().unwrap(),
            "postgresql://localhost/default"
        );
        assert_eq!(
            datasources.named["inventory"].sqlx_url().unwrap(),
            "postgresql://localhost/inventory"
        );
    }

    #[test]
    fn typed_selectors_build_concrete_pool_options() {
        let config = Config::builder()
            .add_source(
                MapSource::new("test", 100)
                    .with("datasource.database", "service")
                    .with("datasource.users.url", "postgresql://localhost/users")
                    .with("datasource.audit.database", ":memory:"),
            )
            .build();
        let datasources = DatasourcesConfig::from_config(&config).unwrap();

        let default = datasources.default_postgres().unwrap();
        let users = datasources.postgres("users").unwrap();
        let audit = datasources.sqlite("audit").unwrap();

        let _: PgPoolOptions = default.pool_options();
        let _: PgPoolOptions = users.pool_options();
        let _: SqlitePoolOptions = audit.pool_options();
        assert_eq!(
            default.config.sqlx_url_for(DatabaseKind::Postgres).unwrap(),
            "postgresql://localhost/service"
        );
        assert_eq!(
            users.config.sqlx_url_for(DatabaseKind::Postgres).unwrap(),
            "postgresql://localhost/users"
        );
        assert_eq!(
            audit.config.sqlx_url_for(DatabaseKind::Sqlite).unwrap(),
            ":memory:"
        );
    }

    #[test]
    fn typed_selectors_reject_mismatched_database_kind() {
        let config = Config::builder()
            .add_source(
                MapSource::new("test", 100)
                    .with("datasource.db-kind", "sqlite")
                    .with("datasource.database", ":memory:"),
            )
            .build();
        let datasources = DatasourcesConfig::from_config(&config).unwrap();

        assert!(matches!(
            datasources.default_postgres().unwrap_err(),
            SqlxConfigError::DatabaseKindMismatch {
                expected: DatabaseKind::Postgres,
                actual: DatabaseKind::Sqlite,
            }
        ));
    }

    #[test]
    fn typed_selectors_report_unknown_datasources() {
        let datasources = DatasourcesConfig::from_config(&Config::builder().build()).unwrap();

        assert!(matches!(
            datasources.postgres("missing").unwrap_err(),
            SqlxConfigError::UnknownDatasource { name } if name == "missing"
        ));
    }

    #[test]
    fn reports_missing_url_when_datasource_cannot_be_built() {
        let config = Config::builder().build();
        let datasource = DatasourceConfig::from_config(&config).unwrap();

        assert!(matches!(
            datasource.sqlx_url().unwrap_err(),
            SqlxConfigError::MissingUrl
        ));
    }
}
