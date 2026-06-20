use leptos::*;
use crate::api::*;
use crate::components::skeleton::SkeletonTable;

#[component]
pub fn ApiKeys() -> impl IntoView {
    let keys = create_rw_signal(Vec::<ApiKeyDetail>::new());
    let loading = create_rw_signal(true);
    let error = create_rw_signal(String::new());
    let show_form = create_rw_signal(false);
    let form_name = create_rw_signal(String::new());
    let new_key_data = create_rw_signal(Option::<ApiKeyCreateResponse>::None);
    let saving = create_rw_signal(false);
    let delete_id = create_rw_signal(Option::<String>::None);

    let load = move || {
        spawn_local({
            let keys = keys.clone();
            let loading = loading.clone();
            let error = error.clone();
            async move {
                match fetch_api_keys().await {
                    Ok(data) => { keys.set(data); loading.set(false); }
                    Err(e) => { error.set(e); loading.set(false); }
                }
            }
        });
    };
    load();

    let create = move || {
        saving.set(true);
        error.set(String::new());
        let body_str = serde_json::json!({"key_name": form_name.get()}).to_string();
        spawn_local({
            let keys = keys.clone();
            let loading = loading.clone();
            let error = error.clone();
            let show_form = show_form.clone();
            let new_key_data = new_key_data.clone();
            let form_name = form_name.clone();
            let saving = saving.clone();
            async move {
                match create_api_key(&body_str).await {
                    Ok(resp) => {
                        new_key_data.set(Some(resp));
                        form_name.set(String::new());
                        show_form.set(false);
                        saving.set(false);
                        loading.set(true);
                        match fetch_api_keys().await {
                            Ok(data) => { keys.set(data); loading.set(false); }
                            Err(e) => { error.set(e); loading.set(false); }
                        }
                    }
                    Err(e) => { error.set(e); saving.set(false); }
                }
            }
        });
    };

    let do_delete = move |id: String| {
        spawn_local({
            let keys = keys.clone();
            let loading = loading.clone();
            let error = error.clone();
            let delete_id = delete_id.clone();
            async move {
                match delete_api_key(&id).await {
                    Ok(()) => {
                        delete_id.set(None);
                        loading.set(true);
                        match fetch_api_keys().await {
                            Ok(data) => { keys.set(data); loading.set(false); }
                            Err(e) => { error.set(e); loading.set(false); }
                        }
                    }
                    Err(e) => error.set(e),
                }
            }
        });
    };

    view! {
        <div class="animate-fade-in">
            // ─── Page Header ────────────────────────────────────────────
            <div class="flex items-center justify-between mb-6">
                <div>
                    <h1 class="text-2xl font-bold text-primary">"API Keys"</h1>
                    <p class="text-sm text-secondary mt-1">"Manage API keys for client access"</p>
                </div>
                <button
                    on:click=move|_| {
                        form_name.set(String::new());
                        new_key_data.set(None);
                        show_form.set(true);
                    }
                    class="px-4 py-2 text-sm font-medium rounded-lg text-white
                           bg-accent hover:bg-accent-hover
                           active:scale-[0.97] transition-all duration-150 flex items-center gap-2">
                    <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4"/>
                    </svg>
                    "New Key"
                </button>
            </div>

            // ─── Error Banner ───────────────────────────────────────────
            {move || (!error.get().is_empty()).then(||
                view! {
                    <p class="mb-4 p-3 rounded-lg bg-danger-bg text-danger text-sm border border-danger">
                        {error.get()}
                    </p>
                }
            )}
            {move || loading.get().then(|| view! { <SkeletonTable rows=3/> })}

            // ─── New Key Reveal Box ─────────────────────────────────────
            {move || new_key_data.get().map(|nk| {
                let full_key = nk.full_key.clone();
                view! {
                    <div class="mb-6 p-4 rounded-[14px] bg-success-bg border border-success/30 animate-scale-in">
                        <div class="flex items-center gap-2 mb-2">
                            <svg class="w-5 h-5 text-success" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                                      d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"/>
                            </svg>
                            <p class="text-sm font-semibold text-success">
                                "API Key Created — Copy it now!"
                            </p>
                        </div>
                        <p class="text-xs text-success/70 mb-2">
                            "This key will not be shown again."
                        </p>
                        <div class="flex gap-2">
                            <code class="flex-1 px-3 py-2 rounded-lg bg-[#0d1117] text-sm font-mono text-accent break-all select-all">
                                {full_key.clone()}
                            </code>
                            <button
                                on:click=move|_| {
                                    let key = full_key.clone();
                                    spawn_local(async move {
                                        let window = web_sys::window().unwrap();
                                        let _ = window.navigator().clipboard().write_text(&key);
                                    });
                                }
                                class="px-3 py-2 text-xs font-medium rounded-lg
                                       bg-surface-2 hover:bg-surface-3
                                       active:scale-[0.97] text-primary transition-all duration-150">
                                "Copy"
                            </button>
                        </div>
                    </div>
                }
            })}

            // ─── Create Key Modal ───────────────────────────────────────
            {move || show_form.get().then(|| {
                view! {
                    <div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50 animate-fade-in"
                        on:click=move|_| show_form.set(false)>
                        <div class="bg-surface border border-border-subtle rounded-[14px]
                                    w-full max-w-md mx-4 shadow-2xl animate-scale-in"
                            on:click=move|ev| ev.stop_propagation()>
                            <div class="flex items-center justify-between px-6 py-4 border-b border-surface">
                                <h2 class="text-lg font-semibold text-primary">"Create API Key"</h2>
                                <button
                                    on:click=move|_| show_form.set(false)
                                    class="text-muted hover:text-primary transition-colors">
                                    <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                                              d="M6 18L18 6M6 6l12 12"/>
                                    </svg>
                                </button>
                            </div>
                            <div class="p-6">
                                <div class="mb-4">
                                    <label class="block text-xs text-secondary mb-1.5 font-medium">
                                        "Key Name"
                                    </label>
                                    <input
                                        type="text"
                                        prop:value=form_name.get()
                                        placeholder="e.g. Production key"
                                        on:input=move|ev| form_name.set(event_target_value(&ev))
                                        class="w-full px-3 py-2 bg-surface-2 border border-surface rounded-lg
                                               text-sm text-primary placeholder-muted
                                               focus:border-accent focus:outline-none transition-colors"/>
                                </div>
                                <div class="flex gap-3 justify-end mt-6 pt-4 border-t border-surface">
                                    <button
                                        on:click=move|_| show_form.set(false)
                                        class="px-4 py-2 text-sm font-medium rounded-lg
                                               bg-transparent border border-surface text-secondary
                                               hover:text-primary hover:bg-surface-2
                                               active:scale-[0.97] transition-all duration-150">
                                        "Cancel"
                                    </button>
                                    <button
                                        on:click=move|_| create()
                                        disabled=saving.get()
                                        class="px-4 py-2 text-sm font-medium rounded-lg text-white
                                               bg-accent hover:bg-accent-hover disabled:opacity-50
                                               active:scale-[0.97] transition-all duration-150">
                                        "Create"
                                    </button>
                                </div>
                            </div>
                        </div>
                    </div>
                }
            })}

            // ─── Delete Confirm Modal ───────────────────────────────────
            {move || delete_id.get().map(|id| {
                let name = keys.with(|k| {
                    k.iter().find(|x| x.id == id)
                        .map(|x| x.key_name.clone())
                        .unwrap_or_default()
                });
                view! {
                    <div class="fixed inset-0 bg-black/60 flex items-center justify-center z-50 animate-fade-in"
                        on:click=move|_| delete_id.set(None)>
                        <div class="bg-surface border border-border-subtle rounded-[14px] p-6
                                    w-full max-w-md mx-4 shadow-2xl animate-scale-in"
                            on:click=move|ev| ev.stop_propagation()>
                            <div class="flex items-start gap-3 mb-4">
                                <div class="w-10 h-10 rounded-full bg-danger-bg flex items-center justify-center flex-shrink-0">
                                    <svg class="w-5 h-5 text-danger" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                                              d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.072 16.5c-.77.833.192 2.5 1.732 2.5z"/>
                                    </svg>
                                </div>
                                <div>
                                    <h3 class="text-base font-semibold text-primary">
                                        {format!("Revoke \"{}\"?", name)}
                                    </h3>
                                    <p class="text-sm text-secondary mt-1">
                                        "Clients using this key will immediately lose access."
                                    </p>
                                </div>
                            </div>
                            <div class="flex gap-2 justify-end">
                                <button
                                    on:click=move|_| delete_id.set(None)
                                    class="px-4 py-2 text-sm font-medium rounded-lg
                                           bg-transparent border border-surface text-secondary
                                           hover:text-primary hover:bg-surface-2
                                           active:scale-[0.97] transition-all duration-150">
                                    "Cancel"
                                </button>
                                <button
                                    on:click=move|_| do_delete(id.clone())
                                    class="px-4 py-2 text-sm font-medium rounded-lg text-white
                                           bg-danger hover:bg-red-600
                                           active:scale-[0.97] transition-all duration-150">
                                    "Revoke"
                                </button>
                            </div>
                        </div>
                    </div>
                }
            })}

            // ─── Keys Table ─────────────────────────────────────────────
            {move || (!loading.get() && !show_form.get()).then(|| {
                let ks = keys.get();
                view! {
                    <div class="bg-surface border border-border-subtle rounded-[14px] overflow-hidden animate-fade-in-up">
                        <table class="w-full">
                            <thead>
                                <tr class="bg-surface-2">
                                    <th class="text-left px-4 py-3 text-xs font-semibold text-secondary uppercase tracking-wider">
                                        "Name"
                                    </th>
                                    <th class="text-left px-4 py-3 text-xs font-semibold text-secondary uppercase tracking-wider">
                                        "Key Prefix"
                                    </th>
                                    <th class="text-left px-4 py-3 text-xs font-semibold text-secondary uppercase tracking-wider">
                                        "Created"
                                    </th>
                                    <th class="text-left px-4 py-3 text-xs font-semibold text-secondary uppercase tracking-wider">
                                        "Status"
                                    </th>
                                    <th class="text-right px-4 py-3 text-xs font-semibold text-secondary uppercase tracking-wider">
                                        "Actions"
                                    </th>
                                </tr>
                            </thead>
                            <tbody class="divide-y divide-border-subtle">
                                {ks.into_iter().map(|k| {
                                    let kid = k.id.clone();
                                    view! {
                                        <tr class="hover:bg-surface-2/50 transition-colors duration-100">
                                            <td class="px-4 py-3 text-sm font-medium text-primary">
                                                {k.key_name}
                                            </td>
                                            <td class="px-4 py-3">
                                                <code class="text-sm font-mono text-accent bg-surface-2 px-2 py-0.5 rounded">
                                                    {format!("{}...", k.key_prefix)}
                                                </code>
                                            </td>
                                            <td class="px-4 py-3 text-sm text-secondary">
                                                {k.created_at[..10].to_string()}
                                            </td>
                                            <td class="px-4 py-3">
                                                {
                                                    if k.enabled {
                                                        view! {
                                                            <span class="inline-flex items-center gap-1 text-xs text-success">
                                                                <span class="w-1.5 h-1.5 rounded-full bg-success"></span>
                                                                "Active"
                                                            </span>
                                                        }
                                                    } else {
                                                        view! {
                                                            <span class="inline-flex items-center gap-1 text-xs text-muted">
                                                                <span class="w-1.5 h-1.5 rounded-full bg-muted"></span>
                                                                "Revoked"
                                                            </span>
                                                        }
                                                    }
                                                }
                                            </td>
                                            <td class="px-4 py-3 text-right">
                                                <button
                                                    on:click=move|_| delete_id.set(Some(kid.clone()))
                                                    class="px-2.5 py-1.5 text-xs font-medium rounded-lg
                                                           text-danger border border-danger/30 hover:bg-danger-bg
                                                           active:scale-[0.97] transition-all duration-150">
                                                    "Revoke"
                                                </button>
                                            </td>
                                        </tr>
                                    }
                                }).collect::<Vec<_>>()}
                            </tbody>
                        </table>
                        {keys.with(|k| k.is_empty()).then(|| {
                            view! {
                                <div class="text-center py-12 text-muted text-sm">
                                    "No API keys yet."
                                </div>
                            }
                        })}
                    </div>
                }
            })}
        </div>
    }
}
