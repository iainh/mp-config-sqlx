use mp_config::{Config, MapSource};
use mp_config_sqlx::Datasources;
use sqlx::{PgPool, SqlitePool};

#[derive(Datasources)]
#[allow(dead_code)]
struct AppPools {
    #[datasource(default)]
    primary: PgPool,
    audit: SqlitePool,
    #[datasource(name = "events")]
    event_store: PgPool,
}

#[derive(Datasources)]
#[datasources(prefix = "app.datasource")]
#[allow(dead_code)]
struct PrefixedPools {
    #[datasource(default)]
    primary: PgPool,
}

#[test]
fn derives_connect_constructor_from_pool_types() {
    let config = Config::builder()
        .add_source(
            MapSource::new("test", 100)
                .with("datasource.url", "postgresql://localhost/primary")
                .with("datasource.audit.url", "sqlite::memory:")
                .with("datasource.events.url", "postgresql://localhost/events"),
        )
        .build();

    let future = AppPools::connect(&config);
    drop(future);
}

#[test]
fn supports_custom_prefixes() {
    let config = Config::builder()
        .add_source(
            MapSource::new("test", 100).with("app.datasource.url", "postgresql://localhost/app"),
        )
        .build();

    let future = PrefixedPools::connect(&config);
    drop(future);
}
