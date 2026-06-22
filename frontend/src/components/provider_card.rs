use std::rc::Rc;
use leptos::*;
use crate::api::*;
use crate::components::provider_icon::{ProviderIcon, category_style};

/// Renders a single provider card in the grid, with OAuth/WebCookie login support.
#[component]
pub fn ProviderCard(
    provider: ProviderDetail,
    expanded: RwSignal<Option<String>>,
    deleting: RwSignal<Option<String>>,
    model_test_results: RwSignal<std::collections::HashMap<String, TestProviderResponse>>,
    testing_model: RwSignal<Option<String>>,
    connections: Vec<OAuthConnectionItem>,
    on_edit: Box<dyn Fn(ProviderDetail) + 'static>,
    on_test: Box<dyn Fn(String, String) + 'static>,
    on_oauth_login: Box<dyn Fn(String) + 'static>,
    on_device_code: Box<dyn Fn(String) + 'static>,
    on_import_token: Box<dyn Fn(String, String) + 'static>,
) -> impl IntoView {
    let pid = provider.id.clone();
    let models = provider.models.clone();
    let model_count = models.len();
    let cat_str = provider.category.clone();
    let ptype = provider.provider_type.clone();
    let pname = provider.name.clone();
    let enabled = provider.enabled;
    let base_url = provider.base_url.clone();

    let has_conn = connections.iter().any(|c| c.provider == ptype && c.is_valid);
    let is_oauth = cat_str == "oauth";
    let is_wc = cat_str == "web-cookie";
    let supports_dev = matches!(ptype.as_str(), "github" | "kimi_coding" | "codebuddy" | "cursor" | "kilocode");

    // Convert to Rc so we can clone in nested closures
    let on_edit: Rc<dyn Fn(ProviderDetail) + 'static> = Rc::from(on_edit);
    let on_test: Rc<dyn Fn(String, String) + 'static> = Rc::from(on_test);
    let on_oauth_login: Rc<dyn Fn(String) + 'static> = Rc::from(on_oauth_login);
    let on_device_code: Rc<dyn Fn(String) + 'static> = Rc::from(on_device_code);
    let on_import_token: Rc<dyn Fn(String, String) + 'static> = Rc::from(on_import_token);

    let pid_click = pid.clone();
    let pid_status = pid.clone();

    view! {
        <div class="bg-surface border border-border-subtle rounded-[14px] p-4 transition-all duration-200 hover:border-surface hover:-translate-y-0.5 hover:shadow-lg group cursor-pointer"
            style=move || if !enabled { "opacity: 0.7" } else { "" }
            on:click=move|_| {
                let eid = expanded.get();
                if eid.as_deref() == Some(&pid_click) { expanded.set(None); }
                else { expanded.set(Some(pid_click.clone())); }
            }
        >
            // Header: icon + name
            <div class="flex items-center gap-3 mb-3">
                <ProviderIcon provider_type=ptype.clone() name=pname.clone() size=40/>
                <div class="min-w-0 flex-1">
                    <h3 class="font-semibold text-sm text-primary truncate">{pname.clone()}</h3>
                    <span class="text-xs text-muted truncate block">{ptype.clone()}</span>
                </div>
                <div class="flex items-center gap-2 shrink-0">
                    {move || if enabled {
                        view! { <span class="flex items-center gap-1 text-xs text-success"><span class="w-1.5 h-1.5 rounded-full bg-success"></span>"Active"</span> }.into_view()
                    } else if has_conn {
                        view! { <span class="flex items-center gap-1 text-xs text-accent"><span class="w-1.5 h-1.5 rounded-full bg-accent"></span>"Connected"</span> }.into_view()
                    } else if is_oauth || is_wc {
                        view! { <span class="flex items-center gap-1 text-xs text-[#db6b28]"><span class="w-1.5 h-1.5 rounded-full bg-[#db6b28] animate-pulse"></span>"Login Needed"</span> }.into_view()
                    } else {
                        view! { <span class="flex items-center gap-1 text-xs text-muted"><span class="w-1.5 h-1.5 rounded-full bg-muted"></span>"Disabled"</span> }.into_view()
                    }}
                    <svg class="w-4 h-4 text-muted transition-transform duration-200" class:rotate-180=move || expanded.get().as_deref() == Some(&pid_status) fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/></svg>
                </div>
            </div>

            // Category badge
            <div class="mb-2">{{ let (cls, _) = category_style(&cat_str); view! { <span class={cls}>{if is_oauth { "OAuth" } else if is_wc { "Web Cookie" } else if cat_str == "free" { "Free" } else if cat_str == "free-tier" { "Free Tier" } else { "API Key" }}</span> }}}</div>

            // Expanded / collapsed
            {let expanded = expanded; move || {
                let is_exp = expanded.get().as_deref() == Some(&pid);
                if is_exp {
                    // Clone all captures BEFORE the inner move closures
                    let ptype = ptype.clone();
                    let pname = pname.clone();
                    let pid = pid.clone();
                    let models = models.clone();
                    let base_url = base_url.clone();
                    let provider = provider.clone();
                    let deleting = deleting;
                    let on_edit = on_edit.clone();
                    let on_test = on_test.clone();
                    let on_oauth_login = on_oauth_login.clone();
                    let on_device_code = on_device_code.clone();
                    let on_import_token = on_import_token.clone();
                    let is_oauth = is_oauth;
                    let has_conn = has_conn;
                    let is_wc = is_wc;
                    let supports_dev = supports_dev;
                    let model_test_results = model_test_results;
                    let testing_model = testing_model;

                    view! {
                        <ExpandedContent
                            ptype pname pid models base_url provider
                            is_oauth has_conn is_wc supports_dev
                            on_edit on_test on_oauth_login on_device_code on_import_token
                            deleting model_test_results testing_model
                        />
                    }.into_view()
                } else {
                    view! {
                        <div class="text-xs text-secondary">
                            {if is_oauth && !has_conn { "Click to login".to_string() }
                            else if is_wc && !has_conn { "Click to import cookie".to_string() }
                            else { format!("{} models", model_count) }}
                        </div>
                    }.into_view()
                }
            }}
        </div>
    }
}

/// Inner expanded view — receives Rc'd callbacks so it can clone them in closures.
#[component]
fn ExpandedContent(
    ptype: String,
    pname: String,
    pid: String,
    models: Vec<String>,
    base_url: String,
    provider: ProviderDetail,
    is_oauth: bool,
    has_conn: bool,
    is_wc: bool,
    supports_dev: bool,
    on_edit: Rc<dyn Fn(ProviderDetail) + 'static>,
    on_test: Rc<dyn Fn(String, String) + 'static>,
    on_oauth_login: Rc<dyn Fn(String) + 'static>,
    on_device_code: Rc<dyn Fn(String) + 'static>,
    on_import_token: Rc<dyn Fn(String, String) + 'static>,
    deleting: RwSignal<Option<String>>,
    model_test_results: RwSignal<std::collections::HashMap<String, TestProviderResponse>>,
    testing_model: RwSignal<Option<String>>,
) -> impl IntoView {
    let enabled = provider.enabled;

    view! {
        <div class="space-y-1.5 text-xs mt-2 pt-3 border-t border-border-subtle">
            <div class="flex items-center justify-between">
                <span class="text-secondary">"Base URL"</span>
                <span class="text-primary truncate max-w-[180px] text-right font-mono">{base_url}</span>
            </div>

            // ── OAuth Login UI ──
            {if is_oauth && !has_conn && !enabled {
                let ol = on_oauth_login.clone();
                let pt = ptype.clone();
                let pt2 = pt.clone();
                let pt_txt = pt.clone();
                view! {
                    <div class="pt-3 border-t border-border-subtle space-y-2">
                        <button on:click=move|ev| { ev.stop_propagation(); ol(pt.clone()); }
                            class="w-full px-3 py-2 text-xs font-medium rounded-lg text-white bg-[#db6b28] hover:bg-[#c05d22] active:scale-[0.97] transition-all duration-150 flex items-center justify-center gap-2"
                        >
                            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 16l-4-4m0 0l4-4m-4 4h14m-5 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h7a3 3 0 013 3v1"/></svg>
                            "Login with "{pt_txt}
                        </button>
                        {supports_dev.then(|| {
                            let dc = on_device_code.clone();
                            let pt3 = pt2.clone();
                            view! {
                                <button on:click=move|ev| { ev.stop_propagation(); dc(pt3.clone()); }
                                    class="w-full px-3 py-1.5 text-xs font-medium rounded-lg border border-[#db6b28]/30 text-[#db6b28] hover:bg-[rgba(219,107,40,0.1)] active:scale-[0.97] transition-all duration-150"
                                >"Device Code Login"</button>
                            }
                        })}
                    </div>
                }.into_view()
            } else { view! { <span></span> }.into_view() }}

            // ── WebCookie Import UI ──
            {if is_wc && !has_conn && !enabled {
                let it = on_import_token.clone();
                let pt = ptype.clone();
                let input_id = format!("wc_{}", pid);
                view! {
                    <div class="pt-3 border-t border-border-subtle">
                        <label class="text-secondary block mb-1.5">"Session Cookie"</label>
                        <input type="password" id=input_id.clone()
                            placeholder="paste session cookie..."
                            class="w-full px-2 py-1.5 bg-surface-2 border border-surface rounded-lg text-xs text-primary placeholder-muted focus:border-accent focus:outline-none transition-colors mb-2"
                        />
                        <button on:click=move|ev| {
                            ev.stop_propagation();
                            let val = web_sys::window()
                                .and_then(|w| w.document())
                                .and_then(|d| d.get_element_by_id(&input_id))
                                .and_then(|el| { let input: web_sys::HtmlInputElement = wasm_bindgen::JsCast::dyn_into(el).ok()?; Some(input.value()) })
                                .unwrap_or_default();
                            if !val.is_empty() { it(pt.clone(), val); }
                        }
                            class="w-full px-3 py-1.5 text-xs font-medium rounded-lg bg-[#db6b9a] text-white hover:bg-[#c05d82] active:scale-[0.97] transition-all duration-150 flex items-center justify-center gap-2"
                        >"Import Cookie"</button>
                    </div>
                }.into_view()
            } else { view! { <span></span> }.into_view() }}

            // ── Models List ──
            {if models.is_empty() {
                view! { <div class="pt-2 text-xs text-muted">"No models configured"</div> }.into_view()
            } else {
                let models2 = models.clone();
                let ptype2 = ptype.clone();
                let pname2 = pname.clone();
                let pid2 = pid.clone();
                view! {
                    <div class="pt-2">
                        <span class="text-secondary block mb-1.5">"Models"</span>
                        <div class="flex flex-col gap-1">
                            {models2.into_iter().map(|m| {
                                let mdl = m.clone();
                                let tk = format!("{}:{}", pid2, mdl);
                                let busy = testing_model.with(|t| t.as_deref() == Some(&tk));
                                let res = model_test_results.with(|r| r.get(&tk).cloned());
                                let on_t = on_test.clone();
                                let pid_inner = pid2.clone();
                                let ptype3 = ptype2.clone();
                                let pname3 = pname2.clone();
                                view! {
                                    <div class="flex items-center gap-2 py-1.5 px-2.5 rounded-lg bg-surface-2/50 hover:bg-surface-2 transition-colors">
                                        <span class="text-xs text-primary font-mono flex-1">{m.clone()}</span>
                                        <button on:click=move|ev| { ev.stop_propagation(); on_t(pid_inner.clone(), mdl.clone()); }
                                            class="inline-flex items-center gap-1.5 px-2.5 py-1 text-xs font-medium rounded-md text-secondary hover:text-accent hover:bg-accent-bg active:scale-[0.97] transition-all duration-150 border border-transparent hover:border-accent/30"
                                        >
                                            <ProviderIcon provider_type=ptype3.clone() name=pname3.clone() size=16/>
                                            {if busy { "Test..." } else { "Test" }}
                                        </button>
                                        {res.map(|r| view! {
                                            <span class="text-xs font-mono" style=if r.ok { "color:#22C55e" } else { "color:#ef4444" }>
                                                {if r.ok { format!("{}ms", r.latency_ms) } else { "ERR".to_string() }}
                                            </span>
                                        })}
                                    </div>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    </div>
                }.into_view()
            }}

            // ── Action Buttons ──
            <div class="flex gap-2 justify-end pt-3 border-t border-border-subtle">
                <button on:click=move|ev| { ev.stop_propagation(); on_edit(provider.clone()); }
                    class="px-2.5 py-1.5 text-xs font-medium rounded-lg text-secondary border border-surface hover:text-primary hover:bg-surface-2 active:scale-[0.97] transition-all duration-150"
                >"Edit"</button>
                <button on:click=move|ev| { ev.stop_propagation(); deleting.set(Some(pid.clone())); }
                    class="px-2.5 py-1.5 text-xs font-medium rounded-lg text-danger border border-danger/30 hover:bg-danger-bg active:scale-[0.97] transition-all duration-150"
                >"Delete"</button>
            </div>
        </div>
    }
}
