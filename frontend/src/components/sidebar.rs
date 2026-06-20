use leptos::*;
use leptos_router::A;

#[component]
pub fn Sidebar() -> impl IntoView {
    let items = vec![
        ("/",            "Dashboard",  "M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6"),
        ("/providers",   "Providers",  "M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4"),
        ("/routes",      "Routes",     "M9 20l-5.447-2.724A1 1 0 013 16.382V5.618a1 1 0 011.447-.894L9 7m0 13l6-3m-6 3V7m6 10l4.553 2.276A1 1 0 0021 18.382V7.618a1 1 0 00-.553-.894L15 4m0 13V4m0 0L9 7"),
        ("/api-keys",    "API Keys",   "M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z"),
        ("/analytics",   "Analytics",  "M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"),
        ("/settings",    "Settings",   "M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"),
    ];

    view! {
        <nav class="w-60 bg-surface-alt border-r border-surface
                    flex-shrink-0 p-6 flex flex-col h-screen sticky top-0">
            <div class="mb-8">
                <h1 class="text-xl font-bold text-accent tracking-tight">
                    "AIRouter"
                </h1>
                <p class="text-xs text-muted mt-0.5">"Dashboard"</p>
            </div>
            <ul class="flex flex-col gap-1">
                {items.into_iter().map(|(href, label, icon_path)| {
                    view! {
                        <li>
                            <A href=href
                                class="flex items-center gap-3 px-3 py-2.5 text-sm rounded-lg
                                       text-secondary hover:text-primary
                                       hover:bg-surface-active transition-all duration-150"
                                active_class="bg-accent-bg text-accent hover:bg-accent-bg"
                                >
                                <svg class="w-5 h-5 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d=icon_path/>
                                </svg>
                                {label}
                            </A>
                        </li>
                    }
                }).collect::<Vec<_>>()}
            </ul>
            <div class="mt-auto pt-6 border-t border-surface">
                <div class="flex items-center gap-2 px-3 py-2 text-xs text-muted">
                    <span class="w-2 h-2 rounded-full bg-success animate-pulse-soft"></span>
                    "System Online"
                </div>
            </div>
        </nav>
    }
}
