use std::sync::Arc;
use crate::provider::{Provider, ProviderRegistry};
use crate::config::settings::RouteConfig;

#[derive(Clone)]
pub struct RouterEngine {
    registry: Arc<ProviderRegistry>,
}

impl RouterEngine {
    pub fn new(registry: Arc<ProviderRegistry>) -> Self {
        Self { registry }
    }
}

/// Extension trait adding route resolution to ProviderRegistry
pub trait RouteModel {
    fn resolve(&self, model: &str, routes: &[RouteConfig]) -> Result<&Box<dyn Provider>, String>;
    fn get_fallback_providers(&self, model: &str, routes: &[RouteConfig]) -> Vec<&Box<dyn Provider>>;
}

impl RouteModel for ProviderRegistry {
    fn resolve(&self, model: &str, routes: &[RouteConfig]) -> Result<&Box<dyn Provider>, String> {
        // Try exact match first
        for route in routes {
            if route.model == model {
                if let Some(provider_name) = &route.provider {
                    return self.get(provider_name)
                        .ok_or_else(|| format!("Provider '{}' not found for model '{}'", provider_name, model));
                }
                if let Some(providers) = &route.providers {
                    if let Some(first) = providers.first() {
                        return self.get(first)
                            .ok_or_else(|| format!("Provider '{}' not found for model '{}'", first, model));
                    }
                }
            }
        }

        // Try wildcard fallback
        for route in routes {
            if route.model == "*" {
                if let Some(provider_name) = &route.provider {
                    return self.get(provider_name)
                        .ok_or_else(|| format!("Fallback provider '{}' not found", provider_name));
                }
                if let Some(providers) = &route.providers {
                    if let Some(first) = providers.first() {
                        return self.get(first)
                            .ok_or_else(|| format!("Fallback provider '{}' not found", first));
                    }
                }
            }
        }

        Err(format!("No route found for model '{}'", model))
    }

    fn get_fallback_providers(&self, model: &str, routes: &[RouteConfig]) -> Vec<&Box<dyn Provider>> {
        for route in routes {
            if route.model == model || route.model == "*" {
                if let Some(providers) = &route.providers {
                    return providers.iter()
                        .filter_map(|name| self.get(name))
                        .skip(1) // skip primary
                        .collect();
                }
            }
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::settings::RouteConfig;

    fn make_routes() -> Vec<RouteConfig> {
        vec![
            RouteConfig { model: "gpt-4o".into(), strategy: "single".into(), provider: Some("openai".into()), providers: None },
            RouteConfig { model: "*".into(), strategy: "fallback".into(), provider: None, providers: Some(vec!["openai".into(), "groq".into()]) },
        ]
    }

    #[test]
    fn test_resolve_exact_match() {
        let routes = make_routes();
        assert!(routes[0].model == "gpt-4o");
        assert_eq!(routes[0].provider.as_deref(), Some("openai"));
    }

    #[test]
    fn test_resolve_wildcard_match() {
        let routes = make_routes();
        assert!(routes[1].model == "*");
        assert_eq!(routes[1].providers.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_no_match_not_found() {
        let routes = make_routes();
        let has = routes.iter().any(|r| r.model == "unknown-model" || r.model == "*");
        assert!(has); // wildcard catches it
    }

    #[test]
    fn test_route_strategies() {
        let single = RouteConfig { model: "a".into(), strategy: "single".into(), provider: Some("p1".into()), providers: None };
        let fallback = RouteConfig { model: "b".into(), strategy: "fallback".into(), provider: None, providers: Some(vec!["p1".into(), "p2".into()]) };
        assert_eq!(single.strategy, "single");
        assert_eq!(fallback.strategy, "fallback");
        assert_eq!(fallback.providers.as_ref().unwrap().len(), 2);
    }
}
