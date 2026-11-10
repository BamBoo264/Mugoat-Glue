//! Connecting a coding agent (Claude Code, Codex, Cursor…) to the game's MCP
//! server (`/mcp`): each client's own steps, as its documentation gives
//! them, with a pairing of its own (`pairings.agent`, made when asked:
//! listed in the game under Mods › Studio, removable alone).

use std::cell::RefCell;
use std::collections::HashMap;

use leptos::prelude::*;
use serde_json::json;

use crate::highlight::{Lang, pieces};
use crate::net::request;
use crate::state::host;

/// Where the token goes in a snippet, until there is one.
const NO_TOKEN: &str = "<token>";

/// A snippet: what to do with it, its language, its text (`{url}` and
/// `{token}` filled in).
struct Step {
    say: &'static str,
    lang: Lang,
    text: &'static str,
}

struct Client {
    name: &'static str,
    /// What it needs, said once above its steps.
    note: &'static str,
    steps: &'static [Step],
    /// A link that installs the server in one click (`url`, `token`).
    install: Option<(&'static str, fn(&str, &str) -> String)>,
}

const CLIENTS: &[Client] = &[
    Client {
        name: "Claude Code",
        note: "Chạy lệnh sau một lần trong terminal: mọi dự án từ đó sẽ kết nối thẳng vào game.",
        steps: &[Step {
            say: "Thêm máy chủ MCP",
            lang: Lang::Bash,
            text: "claude mcp add --transport http --scope user glue5 {url} \\\n  --header \"Authorization: Bearer {token}\"",
        }],
        install: None,
    },
    Client {
        name: "Codex",
        note: "Codex CLI, ứng dụng và tiện ích IDE dùng chung tệp cấu hình này.",
        steps: &[Step {
            say: "Thêm vào ~/.codex/config.toml",
            lang: Lang::Toml,
            text: "[mcp_servers.glue5]\nurl = \"{url}\"\nhttp_headers = { \"Authorization\" = \"Bearer {token}\" }",
        }],
        install: None,
    },
    Client {
        name: "Cursor",
        note: "Nhấn một chạm để cài đặt hoặc tự thêm vào ~/.cursor/mcp.json.",
        steps: &[Step {
            say: "~/.cursor/mcp.json",
            lang: Lang::Json,
            text: "{\n  \"mcpServers\": {\n    \"glue5\": {\n      \"url\": \"{url}\",\n      \"headers\": { \"Authorization\": \"Bearer {token}\" }\n    }\n  }\n}",
        }],
        install: Some(("Thêm vào Cursor", cursor_link)),
    },
    Client {
        name: "VS Code",
        note: "Dùng cho chế độ tác nhân (Copilot agent). Nhấn một chạm hoặc chạy lệnh terminal.",
        steps: &[Step {
            say: "Hoặc trong terminal",
            lang: Lang::Bash,
            text: "code --add-mcp '{\"name\":\"glue5\",\"type\":\"http\",\"url\":\"{url}\",\"headers\":{\"Authorization\":\"Bearer {token}\"}}'",
        }],
        install: Some(("Thêm vào VS Code", vscode_link)),
    },
    Client {
        name: "Copilot CLI",
        note: "Chạy một lần trong terminal (GitHub Copilot CLI).",
        steps: &[Step {
            say: "Thêm máy chủ MCP",
            lang: Lang::Bash,
            text: "copilot mcp add --transport http glue5 {url} \\\n  --header \"Authorization: Bearer {token}\"",
        }],
        install: None,
    },
    Client {
        name: "Antigravity",
        note: "Google Antigravity IDE và CLI đều dùng chung cấu hình này.",
        steps: &[Step {
            say: "~/.gemini/config/mcp_config.json",
            lang: Lang::Json,
            text: "{\n  \"mcpServers\": {\n    \"glue5\": {\n      \"serverUrl\": \"{url}\",\n      \"headers\": { \"Authorization\": \"Bearer {token}\" }\n    }\n  }\n}",
        }],
        install: None,
    },
    Client {
        name: "Claude Desktop",
        note: "Ứng dụng kết nối từ đám mây nên dùng cầu nối mcp-remote (cần Node.js). Khởi động lại ứng dụng sau khi lưu.",
        steps: &[Step {
            say: "Settings › Developer › Edit Config (claude_desktop_config.json)",
            lang: Lang::Json,
            text: "{\n  \"mcpServers\": {\n    \"glue5\": {\n      \"command\": \"npx\",\n      \"args\": [\"-y\", \"mcp-remote\", \"{url}\", \"--transport\", \"http-only\", \"--allow-http\",\n               \"--header\", \"Authorization:${GLUE5_AUTH}\"],\n      \"env\": { \"GLUE5_AUTH\": \"Bearer {token}\" }\n    }\n  }\n}",
        }],
        install: None,
    },
    Client {
        name: "Windsurf",
        note: "Windsurf (Devin Desktop): vào Cascade › MCP servers › View raw config rồi làm mới.",
        steps: &[Step {
            say: "~/.config/devin/mcp_config.json",
            lang: Lang::Json,
            text: "{\n  \"mcpServers\": {\n    \"glue5\": {\n      \"serverUrl\": \"{url}\",\n      \"headers\": { \"Authorization\": \"Bearer {token}\" }\n    }\n  }\n}",
        }],
        install: None,
    },
    Client {
        name: "Zed",
        note: "Thêm vào settings.json của Zed hoặc Settings › AI › MCP Servers › Add Remote Server.",
        steps: &[Step {
            say: "settings.json",
            lang: Lang::Json,
            text: "{\n  \"context_servers\": {\n    \"glue5\": {\n      \"url\": \"{url}\",\n      \"headers\": { \"Authorization\": \"Bearer {token}\" }\n    }\n  }\n}",
        }],
        install: None,
    },
    Client {
        name: "opencode",
        note: "Thêm vào ~/.config/opencode/opencode.json hoặc opencode.json của dự án.",
        steps: &[Step {
            say: "opencode.json",
            lang: Lang::Json,
            text: "{\n  \"mcp\": {\n    \"glue5\": {\n      \"type\": \"remote\",\n      \"url\": \"{url}\",\n      \"oauth\": false,\n      \"headers\": { \"Authorization\": \"Bearer {token}\" }\n    }\n  }\n}",
        }],
        install: None,
    },
    Client {
        name: "Khác",
        note: "Mọi MCP client hỗ trợ Streamable HTTP (JetBrains, Cline, Goose, Kiro…): điền địa chỉ và header sau.",
        steps: &[
            Step { say: "Địa chỉ máy chủ", lang: Lang::Bash, text: "{url}" },
            Step { say: "Header xác thực", lang: Lang::Bash, text: "Authorization: Bearer {token}" },
        ],
        install: None,
    },
];

/// The server's configuration as Cursor and VS Code take it (JSON).
fn server(url: &str, token: &str) -> serde_json::Value {
    json!({ "url": url, "headers": { "Authorization": format!("Bearer {token}") } })
}

/// Cursor's install link: the configuration in base64.
fn cursor_link(url: &str, token: &str) -> String {
    let b64 = web_sys::window().and_then(|w| w.btoa(&server(url, token).to_string()).ok()).unwrap_or_default();
    format!("cursor://anysphere.cursor-deeplink/mcp/install?name=glue5&config={}", js_sys::encode_uri_component(&b64))
}

/// VS Code's install link: the configuration, named, in the address.
fn vscode_link(url: &str, token: &str) -> String {
    let mut config = server(url, token);
    config["name"] = json!("glue5");
    config["type"] = json!("http");
    format!("vscode:mcp/install?{}", js_sys::encode_uri_component(&config.to_string()))
}

thread_local! {
    /// The tokens made on this page, by client: shown again when the
    /// welcome page comes back, not made twice.
    static TOKENS: RefCell<HashMap<&'static str, String>> = RefCell::new(HashMap::new());
}

/// `text` with the address and the token in.
fn fill(text: &str, url: &str, token: Option<&str>) -> String {
    text.replace("{url}", url).replace("{token}", token.unwrap_or(NO_TOKEN))
}

/// A snippet, coloured, with its Copy button (once there is a token).
#[component]
fn Snippet(lang: Lang, text: String, ready: bool) -> impl IntoView {
    let copied = RwSignal::new(false);
    let copy = {
        let text = text.clone();
        move |_| {
            if let Some(w) = web_sys::window() {
                let _ = w.navigator().clipboard().write_text(&text);
                copied.set(true);
                set_timeout(move || copied.set(false), std::time::Duration::from_millis(1500));
            }
        }
    };
    let code = pieces(lang, &text)
        .into_iter()
        .flat_map(|(class, piece)| {
            // The token's place, marked until there is one.
            let mut out = Vec::new();
            let mut rest = piece.as_str();
            while let Some(i) = rest.find(NO_TOKEN) {
                out.push((class, rest[..i].to_string()));
                out.push(("ph", NO_TOKEN.to_string()));
                rest = &rest[i + NO_TOKEN.len()..];
            }
            out.push((class, rest.to_string()));
            out
        })
        .filter(|(_, p)| !p.is_empty())
        .map(|(class, piece)| view! { <span class=class>{piece}</span> })
        .collect_view();
    view! {
        <div class="snippet" data-lang=lang.name()>
            <pre><code>{code}</code></pre>
            <button class="btn small ghost copy" disabled=!ready on:click=copy
                title=if ready { "Sao chép" } else { "Hãy tạo mã trước" }>
                {move || if copied.get() { "Đã chép" } else { "Sao chép" }}
            </button>
        </div>
    }
}

/// The agents' section of the welcome page: a tab per client, its steps.
#[component]
pub(crate) fn Agents() -> impl IntoView {
    let chosen = RwSignal::new(0usize);
    let tokens = RwSignal::new(TOKENS.with(|t| t.borrow().clone()));
    let failed = RwSignal::new(None::<String>);
    let url = format!("http://{}/mcp", host());
    let make = move |name: &'static str| {
        failed.set(None);
        request("pairings.agent", json!({ "name": name }), move |r| match r {
            Ok(v) => {
                let token = v["token"].as_str().unwrap_or_default().to_string();
                TOKENS.with(|t| t.borrow_mut().insert(name, token.clone()));
                tokens.update(|t| {
                    t.insert(name, token);
                });
            }
            Err(e) => failed.set(Some(e)),
        });
    };
    let body = move || {
        let client = &CLIENTS[chosen.get()];
        let token = tokens.with(|t| t.get(client.name).cloned());
        let ready = token.is_some();
        let steps = client
            .steps
            .iter()
            .map(|step| {
                let text = fill(step.text, &url, token.as_deref());
                view! {
                    <li>
                        <span class="say">{step.say}</span>
                        <Snippet lang=step.lang text=text ready=ready/>
                    </li>
                }
            })
            .collect_view();
        let install = client.install.zip(token.clone()).map(|((label, link), token)| {
            view! { <a class="btn primary" href=link(&url, &token)>{label}</a> }
        });
        let name = client.name;
        view! {
            <p class="agent-note">{client.note}</p>
            <ol class="agent-steps">
                <li>
                    <span class="say">"Mã xác thực riêng"</span>
                    {match token {
                        Some(_) => view! {
                            <p class="made">"Hoàn tất: “"{name}"” đã có mã ghép nối riêng. Quản lý tại mục Bản mod › Studio trong game."</p>
                        }.into_any(),
                        None => view! {
                            <div class="make">
                                <button class="btn primary" on:click=move |_| make(name)>{format!("Tạo mã cho {name}")}</button>
                                {move || failed.get().map(|e| view! { <small class="err">{e}</small> })}
                            </div>
                        }.into_any(),
                    }}
                </li>
                {steps}
                {install.map(|a| view! { <li><span class="say">"Hoặc cài bằng một chạm"</span>{a}</li> })}
            </ol>
        }
    };
    view! {
        <section class="agents" aria-labelledby="agents-h">
            <h2 id="agents-h">"Lập trình cùng tác nhân AI"</h2>
            <p class="lead">"Tác nhân AI kết nối trực tiếp vào game qua MCP: đọc kiểu mọi hàm native, viết và kiểm tra bản mod, chạy thử, đọc nhật ký và điều khiển menu."</p>
            <div class="agent-tabs" role="tablist">
                {CLIENTS.iter().enumerate().map(|(i, c)| view! {
                    <button role="tab" class=move || if chosen.get() == i { "on" } else { "" }
                        aria-selected=move || (chosen.get() == i).to_string()
                        on:click=move |_| chosen.set(i)>
                        {c.name}
                        <Show when=move || tokens.with(|t| t.contains_key(c.name))><span class="paired" title="Đã ghép nối"></span></Show>
                    </button>
                }).collect_view()}
            </div>
            <div class="agent-body" role="tabpanel">{body}</div>
        </section>
    }
}
