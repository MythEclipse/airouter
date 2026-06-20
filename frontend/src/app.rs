use leptos::*;
use leptos_router::*;
use crate::components::sidebar::Sidebar;
use crate::pages::dashboard::Dashboard;
use crate::pages::providers::Providers;
use crate::pages::analytics::Analytics;
use crate::pages::settings::Settings;

#[component]
pub fn App() -> impl IntoView {
    view! {
        <Router>
            <div class="app-layout">
                <Sidebar/>
                <main class="main-content">
                    <Routes>
                        <Route path="/" view= Dashboard/>
                        <Route path="/providers" view= Providers/>
                        <Route path="/analytics" view= Analytics/>
                        <Route path="/settings" view= Settings/>
                        <Route path="/*" view= || view! { <h1>"Not Found"</h1> }/>
                    </Routes>
                </main>
            </div>
        </Router>
    }
}
