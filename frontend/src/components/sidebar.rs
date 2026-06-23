use leptos::*;
use leptos_router::A;

#[component]
pub fn Sidebar() -> impl IntoView {
    let nav_items = vec![
        ("/",          "Dashboard",  "M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6"),
        ("/providers", "Providers",  "M19 21V5a2 2 0 00-2-2H7a2 2 0 00-2 2v16m14 0h2m-2 0h-5m-9 0H3m2 0h5M9 7h1m-1 4h1m4-4h1m-1 4h1m-5 10v-5a1 1 0 011-1h2a1 1 0 011 1v5m-4 0h4"),
        ("/routes",    "Routes",     "M9 20l-5.447-2.724A1 1 0 013 16.382V5.618a1 1 0 011.447-.894L9 7m0 13l6-3m-6 3V7m6 10l4.553 2.276A1 1 0 0021 18.382V7.618a1 1 0 00-.553-.894L15 4m0 13V4m0 0L9 7"),
        ("/api-keys",  "API Keys",   "M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z"),
        ("/analytics", "Analytics",  "M2.25 18L9 11.25l4.306 4.307a11.95 11.95 0 015.814-5.519l2.74-1.22m0 0l-5.94-2.28m5.94 2.28l-2.28 5.941"),
        ("/settings",  "Settings",   "M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"),
    ];

    view! {
        <nav class="w-56 bg-surface border-r border-border-subtle
                    flex-shrink-0 flex flex-col h-screen sticky top-0 z-30">
            // Brand header
            <div class="px-5 pt-5 pb-4 border-b border-border-subtle">
                <h1 class="text-base font-bold text-primary font-display tracking-tight">"AIRouter"</h1>
                <p class="text-[11px] text-muted mt-0.5 tracking-wide">"AI Gateway"</p>
            </div>

            // Navigation
            <div class="flex-1 flex flex-col gap-0.5 px-2 py-3">
                <span class="px-3 py-1.5 text-[10px] font-semibold text-muted uppercase tracking-widest">"Menu"</span>
                {nav_items.into_iter().map(|(href, label, icon_path)| {
                    view! {
                        <A href=href
                            class="flex items-center gap-2.5 px-3 py-2 text-sm rounded-lg \
                                   text-secondary hover:text-primary hover:bg-surface-2 \
                                   transition-all duration-150"
                            active_class="bg-accent-bg text-accent hover:bg-accent-bg font-medium"
                        >
                            <svg class="w-4.5 h-4.5 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                                <path stroke-linecap="round" stroke-linejoin="round" d=icon_path />
                            </svg>
                            {label}
                        </A>
                    }
                }).collect::<Vec<_>>()}
            </div>

            // Footer
            <div class="px-4 py-4 border-t border-border-subtle flex flex-col gap-3">
                <button on:click=move|_| {
                    if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
                        let _ = storage.remove_item("dashboard_token");
                        let _ = storage.remove_item("ai_token");
                    }
                    if let Some(loc) = web_sys::window().map(|w| w.location()) {
                        let _ = loc.set_href("/login");
                    }
                }
                    class="flex items-center gap-2 text-xs text-muted hover:text-danger transition-colors px-2 py-1.5 rounded-lg hover:bg-danger-bg/50 w-full"
                >
                    <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1"/>
                    </svg>
                    "Logout"
                </button>
                <div class="flex items-center gap-2 text-[11px] text-muted/60">
                    <span class="w-1.5 h-1.5 rounded-full bg-success shadow-[0_0_6px_rgba(45,212,191,0.5)]"></span>
                    "System Online"
                </div>
            </div>
        </nav>
    }
}
