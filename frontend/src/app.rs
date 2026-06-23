use leptos::*;
use leptos_router::*;
use crate::components::sidebar::Sidebar;

/// Check if the user has a valid dashboard token in localStorage
fn has_auth() -> bool {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("dashboard_token").ok().flatten())
        .is_some()
}

/// Redirect to login — called from an effect, not in render path
fn redirect_to_login() {
    if let Some(loc) = web_sys::window().map(|w| w.location()) {
        let _ = loc.set_href("/login");
    }
}

/// Authenticated layout: sidebar + content via <Outlet/>
#[component]
fn AuthenticatedLayout() -> impl IntoView {
    let authed = has_auth();

    Effect::new(move |_| {
        if !authed {
            redirect_to_login();
        }
    });

    view! {
        <div class="flex min-h-screen bg-bg text-primary font-sans antialiased">
            {authed.then(|| view! {
                <Sidebar/>
            })}
            <main class="flex-1 min-w-0 overflow-y-auto">
                {authed.then(|| view! {
                    <div class="p-6 lg:p-8 xl:p-10 max-w-[1600px] mx-auto">
                        <Outlet/>
                    </div>
                })}
            </main>
        </div>
    }
}

#[component]
pub fn App() -> impl IntoView {
    view! {
        <Router>
            <Routes>
                <Route path="/login" view= Login/>
                <Route path="/change-password" view= ChangePassword/>
                <Route path="" view= AuthenticatedLayout>
                    <Route path="" view= Dashboard/>
                    <Route path="providers" view= Providers/>
                    <Route path="routes" view= RouteRules/>
                    <Route path="api-keys" view= ApiKeys/>
                    <Route path="analytics" view= Analytics/>
                    <Route path="settings" view= Settings/>
                    <Route path="/*" view= || view! {
                        <div class="flex flex-col items-center justify-center mt-20 text-center">
                            <h1 class="text-4xl font-bold text-muted font-display">"404"</h1>
                            <p class="text-secondary mt-2">"Page not found"</p>
                            <a href="/" class="mt-4 text-sm text-accent hover:text-accent-hover underline">"Back to dashboard"</a>
                        </div>
                    }/>
                </Route>
            </Routes>
        </Router>
    }
}

// Import page components
use crate::pages::dashboard::Dashboard;
use crate::pages::providers::Providers;
use crate::pages::analytics::Analytics;
use crate::pages::settings::Settings;
use crate::pages::route_rules::RouteRules;
use crate::pages::api_keys::ApiKeys;
use crate::pages::login::Login;
use crate::pages::change_password::ChangePassword;
