use sea_orm::{DatabaseConnection, Statement};
use sea_orm::prelude::*;
use crate::config::settings::{Settings, ProviderConfig, RouteConfig, StrategyKind, ComboConfig, RateLimitConfig, ServerConfig};

/// Run the database schema migration (idempotent).
pub async fn run_migrations(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    let sql = include_str!("../../migrations/001_initial.sql");
    let stmts: Vec<&str> = sql
        .split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    for stmt in stmts {
        db.execute(Statement::from_string(
            sea_orm::DatabaseBackend::Postgres,
            format!("{};", stmt),
        ))
        .await?;
    }
    tracing::info!("Database migrations applied successfully");
    Ok(())
}

/// Sync default providers/routes into DB on startup.
/// Uses upsert (ON CONFLICT name) so existing custom data is untouched,
/// but seed data is always present and models/urls are updated to match code.
pub async fn seed_defaults(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    use crate::entities::{provider, route, api_key, server_config, rate_limit_config};
    use sea_orm::EntityTrait;
    use chrono::Utc;

    // ── Upsert each default provider ───────────────────────────────
    let default_providers = crate::config::settings::default_providers();
    for p in &default_providers {
        let extra = serde_json::to_value(&p.extra_headers).unwrap_or_default();

        // Check if provider with this name already exists
        let existing = provider::Entity::find()
            .filter(provider::Column::Name.eq(&p.name))
            .one(db).await?;

        if let Some(row) = existing {
            // Update — keep api_key (user-set), update models/base_url/type from seed
            let mut model: provider::ActiveModel = row.into();
            model.provider_type = sea_orm::ActiveValue::Set(p.provider_type.clone());
            model.base_url = sea_orm::ActiveValue::Set(p.base_url.clone());
            model.models = sea_orm::ActiveValue::Set(p.models.clone());
            model.extra_headers = sea_orm::ActiveValue::Set(extra);
            model.capabilities = sea_orm::ActiveValue::Set(p.capabilities.clone());
            model.updated_at = sea_orm::ActiveValue::Set(Utc::now());
            model.update(db).await?;
        } else {
            // Insert new
            provider::Entity::insert(provider::ActiveModel {
                id: sea_orm::ActiveValue::Set(uuid::Uuid::new_v4()),
                name: sea_orm::ActiveValue::Set(p.name.clone()),
                provider_type: sea_orm::ActiveValue::Set(p.provider_type.clone()),
                api_key: sea_orm::ActiveValue::Set(p.api_key.clone()),
                base_url: sea_orm::ActiveValue::Set(p.base_url.clone()),
                models: sea_orm::ActiveValue::Set(p.models.clone()),
                extra_headers: sea_orm::ActiveValue::Set(extra),
                capabilities: sea_orm::ActiveValue::Set(p.capabilities.clone()),
                enabled: sea_orm::ActiveValue::Set(true),
                created_at: sea_orm::ActiveValue::Set(Utc::now()),
                updated_at: sea_orm::ActiveValue::Set(Utc::now()),
            })
            .exec(db).await?;
        }
    }
    tracing::info!(count = %default_providers.len(), "Default providers synced");

    // ── Upsert each default route ──────────────────────────────────
    let default_routes = crate::config::settings::default_routes();
    for r in &default_routes {
        let combo = match &r.combo {
            Some(c) => serde_json::to_value(c).unwrap_or_default(),
            None => serde_json::Value::Null,
        };
        let strategy_str = match &r.strategy {
            StrategyKind::Single => "single",
            StrategyKind::Fallback => "fallback",
            StrategyKind::RoundRobin => "round-robin",
            StrategyKind::Fusion => "fusion",
        };

        let existing = route::Entity::find()
            .filter(route::Column::Model.eq(&r.model))
            .one(db).await?;

        if let Some(row) = existing {
            let mut model: route::ActiveModel = row.into();
            model.strategy = sea_orm::ActiveValue::Set(strategy_str.to_string());
            model.provider = sea_orm::ActiveValue::Set(r.provider.clone());
            model.providers = sea_orm::ActiveValue::Set(r.providers.clone());
            model.combo = sea_orm::ActiveValue::Set(combo);
            model.updated_at = sea_orm::ActiveValue::Set(Utc::now());
            model.update(db).await?;
        } else {
            route::Entity::insert(route::ActiveModel {
                id: sea_orm::ActiveValue::Set(uuid::Uuid::new_v4()),
                model: sea_orm::ActiveValue::Set(r.model.clone()),
                strategy: sea_orm::ActiveValue::Set(strategy_str.to_string()),
                provider: sea_orm::ActiveValue::Set(r.provider.clone()),
                providers: sea_orm::ActiveValue::Set(r.providers.clone()),
                combo: sea_orm::ActiveValue::Set(combo),
                enabled: sea_orm::ActiveValue::Set(true),
                created_at: sea_orm::ActiveValue::Set(Utc::now()),
                updated_at: sea_orm::ActiveValue::Set(Utc::now()),
            })
            .exec(db).await?;
        }
    }
    tracing::info!(count = %default_routes.len(), "Default routes synced");

    // ── Seed API key (only if empty) ────────────────────────────────
    let key_count = api_key::Entity::find().count(db).await.unwrap_or(0);
    if key_count == 0 {
        use crate::auth::sha2_hex;
        let key_prefix = &crate::config::settings::DEFAULT_KEY[..10.min(crate::config::settings::DEFAULT_KEY.len())];
        api_key::Entity::insert(api_key::ActiveModel {
            id: sea_orm::ActiveValue::Set(uuid::Uuid::new_v4()),
            key_name: sea_orm::ActiveValue::Set("Default key".into()),
            key_hash: sea_orm::ActiveValue::Set(sha2_hex(crate::config::settings::DEFAULT_KEY)),
            key_prefix: sea_orm::ActiveValue::Set(key_prefix.to_string()),
            enabled: sea_orm::ActiveValue::Set(true),
            created_at: sea_orm::ActiveValue::Set(Utc::now()),
        })
        .exec(db).await?;
        tracing::info!("Default API key seeded");
    }

    // ── Seed server config (only if empty) ──────────────────────────
    let sc_count = server_config::Entity::find().count(db).await.unwrap_or(0);
    if sc_count == 0 {
        server_config::Entity::insert(server_config::ActiveModel {
            id: sea_orm::ActiveValue::Set(1),
            host: sea_orm::ActiveValue::Set("0.0.0.0".into()),
            port: sea_orm::ActiveValue::Set(3000),
            default_max_tokens: sea_orm::ActiveValue::Set(None),
            updated_at: sea_orm::ActiveValue::Set(Utc::now()),
        })
        .exec(db).await?;
        tracing::info!("Server config seeded");
    }

    // ── Seed rate limit config (only if empty) ──────────────────────
    let rl_count = rate_limit_config::Entity::find().count(db).await.unwrap_or(0);
    if rl_count == 0 {
        rate_limit_config::Entity::insert(rate_limit_config::ActiveModel {
            id: sea_orm::ActiveValue::Set(1),
            enabled: sea_orm::ActiveValue::Set(true),
            requests_per_minute: sea_orm::ActiveValue::Set(60),
            burst_size: sea_orm::ActiveValue::Set(20),
            updated_at: sea_orm::ActiveValue::Set(Utc::now()),
        })
        .exec(db).await?;
        tracing::info!("Rate limit config seeded");
    }

    Ok(())
}

/// Load full configuration from database into Settings struct.
pub async fn load_config_from_db(db: &DatabaseConnection) -> Result<Settings, sea_orm::DbErr> {
    use crate::entities::{provider, route, api_key, server_config, rate_limit_config};
    use sea_orm::EntityTrait;

    // Server config
    let sc = server_config::Entity::find_by_id(1).one(db).await?
        .unwrap_or(server_config::Model {
            id: 1, host: "0.0.0.0".into(), port: 3000,
            default_max_tokens: None,
            updated_at: chrono::Utc::now(),
        });

    // Rate limit config
    let rl = rate_limit_config::Entity::find_by_id(1).one(db).await?
        .unwrap_or(rate_limit_config::Model {
            id: 1, enabled: true, requests_per_minute: 60, burst_size: 20,
            updated_at: chrono::Utc::now(),
        });

    // Providers — ALL from DB
    let db_providers = provider::Entity::find()
        .filter(provider::Column::Enabled.eq(true))
        .all(db).await?;

    let providers: Vec<ProviderConfig> = db_providers.iter().map(|p| {
        let extra: std::collections::HashMap<String, String> =
            serde_json::from_value(p.extra_headers.clone()).unwrap_or_default();
        ProviderConfig {
            name: p.name.clone(),
            provider_type: p.provider_type.clone(),
            api_key: p.api_key.clone(),
            base_url: p.base_url.clone(),
            models: p.models.clone(),
            extra_headers: extra,
            capabilities: p.capabilities.clone(),
        }
    }).collect();

    // Routes — ALL from DB
    let db_routes = route::Entity::find()
        .filter(route::Column::Enabled.eq(true))
        .all(db).await?;

    let routes: Vec<RouteConfig> = db_routes.iter().map(|r| {
        let strategy = match r.strategy.as_str() {
            "single" => StrategyKind::Single,
            "round-robin" => StrategyKind::RoundRobin,
            "fusion" => StrategyKind::Fusion,
            _ => StrategyKind::Fallback,
        };
        let combo: Option<ComboConfig> = if r.combo.is_null() {
            None
        } else {
            serde_json::from_value(r.combo.clone()).ok()
        };
        RouteConfig {
            model: r.model.clone(),
            strategy,
            provider: r.provider.clone(),
            providers: r.providers.clone(),
            combo,
        }
    }).collect();

    // API keys
    let db_keys = api_key::Entity::find()
        .filter(api_key::Column::Enabled.eq(true))
        .all(db).await?;

    let keys: Vec<String> = db_keys.iter().map(|k| k.key_prefix.clone()).collect();

    Ok(Settings {
        server: ServerConfig {
            host: sc.host,
            port: sc.port as u16,
            default_max_tokens: sc.default_max_tokens.map(|v| v as u32),
        },
        default_strategy: None,
        keys,
        providers,
        routes,
        rate_limit: RateLimitConfig {
            enabled: rl.enabled,
            requests_per_minute: rl.requests_per_minute as u64,
            burst_size: rl.burst_size as u32,
        },
    })
}
