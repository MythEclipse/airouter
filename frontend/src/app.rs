use leptos::*;
use leptos_router::*;
use crate::components::sidebar::Sidebar;
use crate::pages::dashboard::Dashboard;
use crate::pages::providers::Providers;
use crate::pages::analytics::Analytics;
use crate::pages::settings::Settings;
use crate::pages::route_rules::RouteRules;
use crate::pages::api_keys::ApiKeys;
use crate::pages::login::Login;

/// Check if the user has a valid dashboard token in localStorage
fn has_auth() -> bool {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("dashboard_token").ok().flatten())
        .is_some()
}

/// The main layout for authenticated pages — renders sidebar + content via <Outlet/>
#[component]
fn AuthenticatedLayout() -> impl IntoView {
    if has_auth() {
        view! {
            <div class="flex min-h-screen bg-bg text-primary font-sans">
                <Sidebar/>
                <main class="flex-1 p-8 lg:p-10 overflow-y-auto">
                    <Outlet/>
                </main>
            </div>
        }.into_view()
    } else {
        let loc = web_sys::window().unwrap().location();
        let _ = loc.set_href("/login");
        view! { <div></div> }.into_view()
    }
}

#[component]
pub fn App() -> impl IntoView {
    view! {
        <Router>
            <Routes>
                <Route path="/login" view= Login/>
                <Route path="" view= AuthenticatedLayout>
                    <Route path="" view= Dashboard/>
                    <Route path="providers" view= Providers/>
                    <Route path="routes" view= RouteRules/>
                    <Route path="api-keys" view= ApiKeys/>
                    <Route path="analytics" view= Analytics/>
                    <Route path="settings" view= Settings/>
                    <Route path="/*" view= || view! {
                        <div class="flex flex-col items-center justify-center mt-20 text-center">
                            <h1 class="text-4xl font-bold text-muted">"404"</h1>
                            <p class="text-secondary mt-2">"Page not found"</p>
                        </div>
                    }/>
                </Route>
            </Routes>
        </Router>
    }
}
