use leptos::*;
use leptos_router::A;

#[component]
pub fn Sidebar() -> impl IntoView {
    view! {
        <nav class="sidebar">
            <div class="sidebar-header">
                <h1 class="logo">"AIRouter"</h1>
            </div>
            <ul class="nav-links">
                <li><A href="/">"Dashboard"</A></li>
                <li><A href="/providers">"Providers"</A></li>
                <li><A href="/analytics">"Analytics"</A></li>
                <li><A href="/settings">"Settings"</A></li>
            </ul>
        </nav>
    }
}
