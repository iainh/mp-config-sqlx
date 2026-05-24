# mp-config-sqlx

`mp-config-sqlx` configures SQLx connection pools from `mp-config`. It keeps
configuration loading separate from database access, then exposes concrete SQLx
pool types such as `PgPool`, `MySqlPool` and `SqlitePool`.

The default property prefix is `datasource`. Named datasources live under
`datasource.<name>`.

## High-level features

- Load datasource settings from any `mp-config` source.
- Configure a default datasource and any number of named datasources.
- Connect concrete SQLx pool types with `connect` and `connect_lazy`.
- Infer connection URLs from structured fields such as `host`, `port` and
  `database`.
- Pass complete SQLx connection URLs through unchanged.
- Configure common SQLx pool settings from configuration.
- Derive application-specific pool structs with `#[derive(Datasources)]`.
- Enable only the database drivers an application needs through Cargo features.

## Example

```toml
[datasource]
db-kind = "postgresql"
username = "app"
password = "secret"
host = "localhost"
port = 5432
database = "service"

[datasource.pool]
min-size = 1
max-size = 10
acquire-timeout = "5s"

[datasource.audit]
db-kind = "sqlite"
database = ":memory:"
```

```rust
use mp_config::Config;
use mp_config_sqlx::DatasourcesConfig;

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let config = Config::default_toml_sources()?;
let datasources = DatasourcesConfig::from_config(&config)?;

let app_pool = datasources.default_postgres()?.connect().await?;
let audit_pool = datasources.sqlite("audit")?.connect().await?;
# let _ = (app_pool, audit_pool);
# Ok(())
# }
```

## Configuration model

`DatasourceConfig::from_config(&config)` loads one default datasource from the
`datasource` prefix. `DatasourcesConfig::from_config(&config)` loads the same
default datasource plus named datasources under `datasource.<name>`.

Use `from_config_prefix` when an application wants a different root:

```rust
use mp_config_sqlx::DatasourcesConfig;

# fn example(config: &mp_config::Config) -> mp_config_sqlx::Result<()> {
let datasources = DatasourcesConfig::from_config_prefix(config, "app.datasource")?;
# let _ = datasources;
# Ok(())
# }
```

With that prefix, the default URL is `app.datasource.url`, and a named
`audit` datasource uses keys such as `app.datasource.audit.url`.

## Datasource properties

The following keys are supported for each datasource prefix:

- `enabled`: Whether the datasource can be connected. Defaults to `true`.
- `db-kind`: The database kind. Supported values are `postgresql`, `postgres`,
  `pgsql`, `mysql`, `mariadb`, `sqlite` and `sqlite3`.
- `url`: A complete SQLx connection URL. When present, this value is passed to
  SQLx unchanged.
- `username`: Username used when assembling a PostgreSQL or MySQL URL.
- `password`: Password used when assembling a PostgreSQL or MySQL URL.
- `host`: Host used when assembling a PostgreSQL or MySQL URL. Defaults to
  `localhost`.
- `port`: Port used when assembling a PostgreSQL or MySQL URL.
- `database`: Database name for PostgreSQL and MySQL, or a SQLite path.

If `url` is absent, `db-kind` and `database` are required. PostgreSQL and MySQL
URLs are assembled as `<scheme>://[username[:password]@]host[:port]/database`.
SQLite values are assembled as `sqlite://<database>`, except `:memory:` and
values that already start with `sqlite:` are passed through unchanged.

## Pool properties

Pool settings live under `<datasource>.pool`.

- `pool.min-size`: Minimum idle connections kept by SQLx.
- `pool.max-size`: Maximum connections opened by SQLx.
- `pool.acquire-timeout`: Maximum time to wait for a connection.
- `pool.idle-timeout`: How long an idle connection can remain in the pool.
- `pool.max-lifetime`: Maximum lifetime of a connection.
- `pool.test-before-acquire`: Whether SQLx should test connections before
  handing them out.

Duration values use the `mp-config` duration converter, for example `250ms`,
`2s`, `5m` or `1h`.

## Typed datasource selection

Select a concrete database type before connecting. The selected datasource then
exposes the same `connect`, `connect_lazy` and `pool_options` method names for
each SQLx pool type.

```rust
use mp_config_sqlx::DatasourcesConfig;

# async fn example(datasources: DatasourcesConfig) -> Result<(), Box<dyn std::error::Error>> {
let default = datasources.default_postgres()?.connect().await?;
let users = datasources.postgres("users")?.connect().await?;
let audit = datasources.sqlite("audit")?.connect().await?;
# let _ = (default, users, audit);
# Ok(())
# }
```

The concrete selectors return:

- `default_postgres` and `postgres(name)`: `PgPool`
- `default_mysql` and `mysql(name)`: `MySqlPool`
- `default_sqlite` and `sqlite(name)`: `SqlitePool`

The same selectors are available on one `DatasourceConfig`:

```rust
use mp_config_sqlx::DatasourceConfig;

# async fn example(config: &mp_config::Config) -> Result<(), Box<dyn std::error::Error>> {
let pool = DatasourceConfig::from_config(config)?
    .postgres()?
    .connect()
    .await?;
# let _ = pool;
# Ok(())
# }
```

If `db-kind` is set and the selected concrete type conflicts with it, the
selector returns `SqlxConfigError::DatabaseKindMismatch`.

## Derived pool structs

Enable the default `macros` feature and derive `Datasources` to generate an
application-specific pool container. The derive infers the datasource type from
the SQLx pool field type.

```rust
use mp_config_sqlx::Datasources;
use sqlx::{PgPool, SqlitePool};

#[derive(Datasources)]
struct AppPools {
    #[datasource(default)]
    primary: PgPool,

    audit: SqlitePool,

    #[datasource(name = "events")]
    event_store: PgPool,
}

# async fn example(config: &mp_config::Config) -> Result<(), Box<dyn std::error::Error>> {
let pools = AppPools::connect(config).await?;
# let _ = pools;
# Ok(())
# }
```

This expands to a `connect(&mp_config::Config)` associated function that loads
`DatasourcesConfig` and connects each field.

Field mapping rules:

- `#[datasource(default)]` uses the default datasource.
- A field without attributes uses the field name as the datasource name.
- `#[datasource(name = "...")]` overrides the datasource name.
- `#[datasources(prefix = "...")]` on the struct changes the root prefix.

Supported field types are `PgPool`, `MySqlPool` and `SqlitePool`, including
qualified paths such as `sqlx::PgPool`.

## Feature flags

Default features enable Tokio runtime support, PostgreSQL, MySQL, SQLite and
the derive macro.

- `runtime-tokio`: Enables SQLx Tokio runtime support.
- `postgres`: Enables PostgreSQL pool helpers.
- `mysql`: Enables MySQL pool helpers.
- `sqlite`: Enables SQLite pool helpers.
- `macros`: Re-exports `#[derive(Datasources)]`.

Disable default features when an application needs a smaller dependency set:

```toml
[dependencies]
mp-config-sqlx = {
    version = "0.1.0",
    default-features = false,
    features = ["runtime-tokio", "postgres"]
}
```

## Error handling

Most functions return `mp_config_sqlx::Result<T>`, which uses
`SqlxConfigError`.

- `Config`: A value could not be loaded or converted by `mp-config`.
- `Disabled`: The selected datasource has `enabled = false`.
- `UnknownDatasource`: A named datasource was requested but is not configured.
- `DatabaseKindMismatch`: The requested concrete SQLx type conflicts with
  `db-kind`.
- `MissingUrl`: The datasource does not have `url`, or enough structured fields
  to assemble a URL.
- `Sqlx`: SQLx rejected the URL, pool options or connection attempt.

## Relationship to mp-config

This crate does not define its own configuration source system. It relies on
`mp-config` for source ordering, profiles, environment mapping, expression
expansion and value conversion.

Typical applications load configuration with `Config::default_sources()` or
`Config::default_toml_sources()`, then pass the resulting `Config` into
`DatasourceConfig`, `DatasourcesConfig` or a derived `Datasources` pool struct.
