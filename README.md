# mp-config-sqlx

`mp-config-sqlx` configures SQLx datasources from `mp-config`.

The default prefix is `datasource`. Named datasources live under
`datasource.<name>`.

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
url = "sqlite::memory:"
```

```rust
use mp_config::Config;
use mp_config_sqlx::DatasourcesConfig;

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let config = Config::default_toml_sources()?;
let datasources = DatasourcesConfig::from_config(&config)?;
let pool = datasources.default_postgres()?.connect().await?;
# let _ = pool;
# Ok(())
# }
```

Complete SQLx URLs are accepted as `url`.

Select the concrete database type before connecting. The selected datasource
then exposes the same `connect` and `connect_lazy` method names for each SQLx
pool type:

```rust
# use mp_config_sqlx::DatasourcesConfig;
# async fn example(datasources: DatasourcesConfig) -> Result<(), Box<dyn std::error::Error>> {
let default = datasources.default_postgres()?.connect().await?;
let audit = datasources.sqlite("audit")?.connect().await?;
# let _ = (default, audit);
# Ok(())
# }
```

Use `from_config_prefix(&config, "app.datasource")` if an application wants a
different property root.
