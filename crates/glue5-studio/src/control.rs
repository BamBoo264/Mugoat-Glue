//! The control page (`/control`): the mods and their menus, for a phone, a
//! tablet or a screen beside the game. The list of mods (start and stop
//! them), and a mod's menus. A phone shows one at a time, a wide screen
//! both side by side. The address names the open mod (`?mod=`): the Studio
//! links to it, the browser's back goes back to the list.

use leptos::prelude::*;
use serde_json::json;
use wasm_bindgen::prelude::*;

use crate::icons;
use crate::menus::Menus;
use crate::net::request;
use crate::state::*;

/// The mod the address names.
fn mod_in_address() -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    web_sys::UrlSearchParams::new_with_str(&search).ok()?.get("mod").filter(|m| !m.is_empty())
}

/// Starts or stops mod `name`.
fn run(s: S, name: &str, on: bool) {
    request(if on { "mods.start" } else { "mods.stop" }, json!({ "name": name }), move |r| {
        if let Err(e) = r {
            s.notice.set(Some(e));
        }
    });
}

/// How many top-level menus mod `name` has.
fn menu_count(s: S, name: &str) -> usize {
    s.menus.with(|m| m.iter().filter(|m| m.parent.is_none() && m.module == name).count())
}

#[component]
pub(crate) fn Control(s: S) -> impl IntoView {
    let open = RwSignal::new(mod_in_address());
    // The browser's back and forward move between the list and a mod.
    let popped = window_event_listener(leptos::ev::popstate, move |_| open.set(mod_in_address()));
    on_cleanup(move || popped.remove());
    let show = Callback::new(move |name: Option<String>| {
        if let Some(h) = web_sys::window().and_then(|w| w.history().ok()) {
            let url = match &name {
                Some(n) => format!("/control?mod={}", js_sys::encode_uri_component(n)),
                None => "/control".to_string(),
            };
            let _ = h.push_state_with_url(&JsValue::NULL, "", Some(&url));
        }
        open.set(name);
    });
    // A notice shows a few seconds (as the Studio's).
    let notice = move || s.notice.get().map(|n| view! { <div class="toast" role="status" on:click=move |_| s.notice.set(None)>{n}</div> });
    view! {
        <div class=move || if open.get().is_some() { "ctl detail" } else { "ctl" }>
            <header class="ctl-bar">
                <div class="brand"><img src="/logo.png" alt=""/><b>"MUGOAT GLUE"</b><span>"ĐIỀU KHIỂN"</span></div>
                <div class=move || if s.connected.get() { "device" } else { "device off" } title=move || if s.connected.get() { "Đã kết nối với trò chơi" } else { "Đang kết nối lại…" }>
                    <span class="dot"></span>
                    <span class="name">{move || s.hello.with(|h| h["device"].as_str().unwrap_or("Trò chơi").to_string())}</span>
                </div>
            </header>
            <div class="ctl-body">
                <aside class="ctl-list" aria-label="Bản mod">
                    <ModList s=s open=open show=show/>
                </aside>
                <main class="ctl-detail">
                    {move || match open.get() {
                        Some(name) => view! { <ModView s=s name=name show=show/> }.into_any(),
                        None => view! { <div class="ctl-pick"><p>"Chọn một bản mod để mở menu."</p></div> }.into_any(),
                    }}
                </main>
            </div>
            {notice}
        </div>
    }
}

/// The mods: what each is, whether it runs (a switch), its menus (a row
/// that opens them).
#[component]
fn ModList(s: S, open: RwSignal<Option<String>>, show: Callback<Option<String>>) -> impl IntoView {
    view! {
        <h2 class="ctl-h">"Bản mod"</h2>
        <Show when=move || s.listed.get() && s.mods.with(Vec::is_empty)>
            <p class="ctl-empty">"Chưa có bản mod nào. Hãy viết bằng Mugoat Glue Studio trên máy tính."</p>
        </Show>
        <div class="ctl-mods">
            <For each=move || s.mods.get() key=|m| format!("{}|{}|{}|{}", m.name, m.state, m.title, m.error.clone().unwrap_or_default()) let(m)>{{
                let name = m.name.clone();
                let running = m.state == "running";
                let selected = { let n = name.clone(); move || open.get().as_deref() == Some(n.as_str()) };
                let menus = { let n = name.clone(); move || menu_count(s, &n) };
                let note = match (&m.error, m.description.is_empty()) {
                    (Some(e), _) => e.clone(),
                    (None, false) => m.description.clone(),
                    (None, true) => String::new(),
                };
                let (n_open, n_run) = (name.clone(), name.clone());
                view! {
                    <div class=move || if selected() { "ctl-mod on" } else { "ctl-mod" }>
                        <button class="ctl-mod-main" on:click=move |_| show.run(Some(n_open.clone()))
                            aria-label=format!("{}: danh sách menu", m.title)>
                            <span class=crate::sidebar::state_class(&m)></span>
                            <span class="ctl-mod-text">
                                <b>{m.title.clone()}</b>
                                {(!note.is_empty()).then(|| view! { <small class=if m.error.is_some() { "err" } else { "" }>{note.clone()}</small> })}
                            </span>
                            <span class="ctl-mod-meta">{move || match menus() {
                                0 => String::new(),
                                1 => "1 menu".into(),
                                n => format!("{n} menu"),
                            }}</span>
                            <span class="ctl-chev" aria-hidden="true">"›"</span>
                        </button>
                        <label class="switch big" title=if running { "Dừng" } else { "Bật" }>
                            <input type="checkbox" aria-label=format!("{} đang chạy", m.title) prop:checked=running
                                on:change=move |ev| run(s, &n_run, event_target_checked(&ev))/>
                            <i></i>
                        </label>
                    </div>
                }
            }}</For>
        </div>
    }
}

/// What a mod's page shows under its title.
#[derive(Clone, PartialEq)]
enum Shown {
    Missing,
    /// Stopped (by the error, if one).
    Off(Option<String>),
    NoMenu,
    Menus,
}

/// A mod's page: its state and its menus.
#[component]
fn ModView(s: S, name: String, show: Callback<Option<String>>) -> impl IntoView {
    let info = { let n = name.clone(); move || s.mods.with(|m| m.iter().find(|m| m.name == n).cloned()) };
    let menus = { let n = name.clone(); move || menu_count(s, &n) };
    let back = move |_| {
        // Back as the browser goes back, when it came from the list.
        match web_sys::window().and_then(|w| w.history().ok()) {
            Some(h) if h.length().unwrap_or(0) > 1 => {
                let _ = h.back();
            }
            _ => show.run(None),
        }
    };
    let (n_run, n_restart, n_menus) = (name.clone(), name.clone(), name.clone());
    let info2 = info.clone();
    let info3 = info.clone();
    let info4 = info.clone();
    let info5 = info.clone();
    let n_switch = name.clone();
    let shown = Memo::new(move |_| match info4() {
        None => Shown::Missing,
        Some(m) if m.state != "running" => Shown::Off(m.error),
        Some(_) if menus() == 0 => Shown::NoMenu,
        Some(_) => Shown::Menus,
    });
    view! {
        <div class="ctl-view">
            <div class="ctl-view-head">
                <button class="mback ctl-back" title="Bản mod" aria-label="Quay lại danh sách mod" on:click=back>"‹"</button>
                <div class="ctl-view-title">
                    <h1>{move || info().map(|m| m.title).unwrap_or_else(|| name.clone())}</h1>
                    <small>{move || match info2() {
                        None => "Không nằm trong thư mục mods".to_string(),
                        Some(m) if m.state == "running" => format!("Đang chạy · {:.2} ms/khung", m.cpu_ms),
                        Some(m) if m.state == "error" => "Đã dừng do lỗi".to_string(),
                        Some(_) => "Đã dừng".to_string(),
                    }}</small>
                </div>
                <Show when=move || info3().is_some_and(|m| m.state == "running")>
                    <button class="btn small ghost" title="Khởi động lại" aria-label="Khởi động lại" on:click={ let n = n_restart.clone(); move |_| {
                        request("mods.restart", json!({ "name": n }), move |r| if let Err(e) = r { s.notice.set(Some(e)) });
                    } }>
                        <span class="ico" inner_html=icons::RESTART></span>
                    </button>
                </Show>
                <label class="switch big" title="Trạng thái chạy">
                    <input type="checkbox" aria-label="Trạng thái chạy" prop:checked=move || info5().is_some_and(|m| m.state == "running")
                        on:change={ let n = n_switch.clone(); move |ev| run(s, &n, event_target_checked(&ev)) }/>
                    <i></i>
                </label>
            </div>
            // Made again only when what shows changes (not each second's figures).
            {move || match shown.get() {
                Shown::Missing => view! { <p class="ctl-empty">"Bản mod này không còn trong thư mục mods của trò chơi."</p> }.into_any(),
                Shown::Off(error) => {
                    let n = n_run.clone();
                    view! {
                        <div class="ctl-off">
                            {error.map(|e| view! { <p class="err">{e}</p> })}
                            <p>"Hãy bật mod để sử dụng menu."</p>
                            <button class="btn primary wide" on:click=move |_| run(s, &n, true)>"Bật"</button>
                        </div>
                    }
                    .into_any()
                }
                Shown::NoMenu => view! { <p class="ctl-empty">"Mod đang chạy nhưng chưa có menu nào."</p> }.into_any(),
                Shown::Menus => view! { <Menus s=s module=n_menus.clone()/> }.into_any(),
            }}
        </div>
    }
}
