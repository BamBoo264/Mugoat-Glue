//! OpenIV (`.oiv`) and loose game-file mods overlaid at runtime without
//! modifying the player's base game directory.
//!
//! Supported layouts under mod roots (`<user_dir>/mods`, `<exe_dir>/mods`,
//! `<exe_dir>/viethoa`, `<user_dir>/viethoa`, or `GLUE5_MODS_DIR`):
//! - OpenIV packages (`*.oiv` zip archives or unpacked folders containing
//!   `assembly.xml` + `content/`), honoring GTA V patch precedence
//!   (`common.rpf`/`x64*.rpf` < `update/update.rpf` < `update/update.rpf/x64/patch`
//!   < `update/update2.rpf`) and merging `.gxt2` string tables so no
//!   translated entry is dropped.
//! - Loose OpenIV folders (`x64e.rpf/...`, `update/...`, `common.rpf/...`)
//!   inside `<mods>/<mod>/...` (skipping `.off`, Luau script mods, etc.).
//!
//! Loose files replace `/game/<rel>` in [`crate::vfs::Vfs`]; files targeting
//! inside an `.rpf` archive are packed into 512-byte-aligned virtual slots past
//! the archive's real end in `Vfs` and hooked into the decrypted RPF7 table
//! when `rage::fiPackfile::ReInit` mounts the archive.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

use glue5_game::Env;
use zlib_rs::{Inflate, InflateFlush, Status};

use crate::memory::Mem;
use crate::rt;

const ALIGN: u64 = 512;
const CACHE_MAGIC_MAX_OPEN: usize = 48;
const RSC7_MAGIC: u32 = 0x3743_5352;
const GXT2_MAGIC: u32 = 0x4758_5432;
const MAX_RESOURCE_SIZE: u64 = 0x00FF_FFFF;

fn align512(v: u64) -> u64 {
    (v + (ALIGN - 1)) & !(ALIGN - 1)
}

#[derive(Clone, Debug)]
pub struct ModSlot {
    pub inner_lower: String,
    pub offset: u64,
    pub length: u64,
    pub path: PathBuf,
    pub rsc7: Option<(u32, u32)>,
}

#[derive(Debug)]
pub struct ModArchive {
    pub rel_lower: String,
    pub real_size: u64,
    pub virtual_size: u64,
    pub slots: Vec<ModSlot>,
    pub by_inner: HashMap<String, usize>,
    handles: Mutex<VecDeque<(PathBuf, Arc<File>)>>,
}

impl ModArchive {
    fn open_slot_file(&self, path: &Path) -> io::Result<Arc<File>> {
        {
            let mut cache = self.handles.lock().unwrap();
            if let Some(pos) = cache.iter().position(|(p, _)| p == path) {
                let (_, f) = cache.remove(pos).unwrap();
                cache.push_front((path.to_path_buf(), f.clone()));
                return Ok(f);
            }
        }
        let file = Arc::new(File::open(path)?);
        let mut cache = self.handles.lock().unwrap();
        if cache.len() >= CACHE_MAGIC_MAX_OPEN {
            cache.pop_back();
        }
        cache.push_front((path.to_path_buf(), file.clone()));
        Ok(file)
    }

    /// Reads bytes from `[0..real_size)` out of `base`, zero-pads gaps, and
    /// serves `[slot.offset..slot.offset+slot.length)` from the mod slot file.
    pub fn read_at(&self, base: &File, offset: u64, buf: &mut [u8]) -> io::Result<usize> {
        if offset >= self.virtual_size || buf.is_empty() {
            return Ok(0);
        }
        let limit = ((self.virtual_size - offset).min(buf.len() as u64)) as usize;
        let mut done = 0;
        while done < limit {
            let pos = offset + done as u64;
            if pos < self.real_size {
                let take = ((self.real_size - pos).min((limit - done) as u64)) as usize;
                let n = read_file_at(base, &mut buf[done..done + take], pos)?;
                done += n;
                if n < take {
                    break;
                }
                continue;
            }
            let idx = self.slots.partition_point(|s| s.offset + s.length <= pos);
            if idx < self.slots.len() && pos >= self.slots[idx].offset {
                let slot = &self.slots[idx];
                let within = pos - slot.offset;
                let take = ((slot.length - within).min((limit - done) as u64)) as usize;
                let f = self.open_slot_file(&slot.path)?;
                let n = read_file_at(&f, &mut buf[done..done + take], within)?;
                done += n;
                if n < take {
                    break;
                }
            } else {
                let next = self.slots.get(idx).map_or(self.virtual_size, |s| s.offset);
                let gap = ((next - pos).min((limit - done) as u64)) as usize;
                buf[done..done + gap].fill(0);
                done += gap;
            }
        }
        Ok(done)
    }
}

#[derive(Default, Debug)]
pub struct GameMods {
    loose: HashMap<String, PathBuf>,
    archives: HashMap<String, Arc<ModArchive>>,
}

impl GameMods {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn loose_file(&self, rel: &str) -> Option<&PathBuf> {
        self.loose.get(&rel.to_ascii_lowercase())
    }

    pub fn archive(&self, rel: &str) -> Option<&Arc<ModArchive>> {
        self.archives.get(&rel.to_ascii_lowercase())
    }

    /// Discovers, unpacks (cached under `<user_dir>/cache/game-mods`), layers,
    /// and indexes all loose and archive-slotted mods for `game_data`.
    pub fn load(game_data: Option<&Path>, user_dir: &Path) -> Self {
        let Some(game_root) = game_data else {
            return Self::default();
        };
        let mut game_index = HashMap::new();
        if let Err(e) = index_game_tree(game_root, "", &mut game_index) {
            log::warn!("game_mods: cannot index {}: {e}", game_root.display());
            return Self::default();
        }

        let cache_root = user_dir.join("cache").join("game-mods");
        let roots = discover_mod_roots(user_dir);
        let mut collected: Vec<Candidate> = Vec::new();

        // Dynamic dlcList.xml overlay for enabled Add-on vehicle mods.
        if let Some(merged_dlc) = crate::vehicle_mods::sync_dlclist_overlay(game_root, user_dir) {
            let o1 = collected.len();
            collected.push(Candidate {
                rel: "common/data/dlclist.xml".into(),
                priority: 250,
                order: o1,
                path: merged_dlc.clone(),
            });
            let o2 = collected.len();
            collected.push(Candidate {
                rel: "common/data/dlclist.xml".into(),
                priority: 250,
                order: o2,
                path: merged_dlc,
            });
        }

        for root in &roots {
            if !root.is_dir() {
                continue;
            }
            collect_from_root(root, &cache_root, user_dir, &mut collected);
        }
        if collected.is_empty() {
            return Self::default();
        }
        build_game_mods(game_root, &game_index, &cache_root, collected)
    }
}

#[derive(Clone, Debug)]
struct Candidate {
    /// Translated relative path under `data/` (forward slashes).
    rel: String,
    /// Layer priority (higher overrides lower).
    priority: u8,
    /// Monotonic discovery order for deterministic tie-breaking.
    order: usize,
    path: PathBuf,
}

fn discover_mod_roots(user_dir: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(env_dir) = std::env::var("GLUE5_MODS_DIR")
        && !env_dir.trim().is_empty()
    {
        roots.push(PathBuf::from(env_dir));
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        roots.push(dir.join("viethoa"));
        roots.push(dir.join("mods"));
    }
    roots.push(user_dir.join("viethoa"));
    roots.push(user_dir.join("mods"));
    if let Some(parent) = user_dir.parent().and_then(Path::parent) {
        let docs_mods = parent.join("Documents").join("mods");
        if docs_mods != user_dir.join("mods") {
            roots.push(docs_mods);
        }
    }
    let mut dedup = Vec::new();
    for r in roots {
        if !dedup.contains(&r) {
            dedup.push(r);
        }
    }
    dedup
}

fn collect_from_root(root: &Path, cache_root: &Path, user_dir: &Path, out: &mut Vec<Candidate>) {
    let mut oivs = Vec::new();
    let mut pkg_dirs = Vec::new();
    let mut loose = Vec::new();
    walk_mod_root(root, root, &mut oivs, &mut pkg_dirs, &mut loose);
    oivs.sort();
    pkg_dirs.sort();
    loose.sort();

    let disabled_vehicle_mods: std::collections::BTreeSet<String> = crate::vehicle_mods::load_db(user_dir)
        .mods
        .into_iter()
        .filter(|m| !m.enabled)
        .map(|m| m.id.to_ascii_lowercase())
        .collect();

    let oiv_cache = cache_root.join("oiv");
    let mut keep_cache_dirs = Vec::new();

    for oiv_path in &oivs {
        let key = package_cache_key(oiv_path);
        let dir = oiv_cache.join(&key);
        keep_cache_dirs.push(key);
        let marker = dir.join(".unpacked");
        if !marker.is_file() {
            let _ = fs::remove_dir_all(&dir);
            if let Err(e) = unpack_zip(oiv_path, &dir) {
                log::warn!("game_mods: cannot unpack {}: {e}", oiv_path.display());
                let _ = fs::remove_dir_all(&dir);
                continue;
            }
            let _ = fs::write(&marker, b"1");
        }
        collect_oiv_dir(&dir, out);
    }

    // Prune stale unpacked .oiv folders whose .oiv was removed or updated.
    if let Ok(entries) = fs::read_dir(&oiv_cache) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !keep_cache_dirs.contains(&name) {
                let _ = fs::remove_dir_all(entry.path());
            }
        }
    }

    for dir in &pkg_dirs {
        // Ignore folders nested inside another package folder.
        if pkg_dirs.iter().any(|other| other != dir && dir.starts_with(other)) {
            continue;
        }
        collect_oiv_dir(dir, out);
    }

    for path in loose {
        // Skip files owned by an unpacked package folder next to assembly.xml.
        if pkg_dirs.iter().any(|d| path.starts_with(d)) {
            continue;
        }
        let Ok(rel_os) = path.strip_prefix(root) else { continue };
        let rel_str = rel_os.to_string_lossy().replace('\\', "/");
        let parts: Vec<&str> = rel_str.split('/').collect();
        if parts.is_empty() {
            continue;
        }
        if parts.iter().any(|p| p.to_ascii_lowercase().ends_with(".off")) {
            continue;
        }
        if parts.iter().any(|p| disabled_vehicle_mods.contains(&p.to_ascii_lowercase())) {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        if matches!(ext.as_str(), "asi" | "ini" | "log" | "json" | "cfg" | "txt" | "md" | "png" | "jpg" | "luau") {
            continue;
        }
        let game_idx = parts.iter().position(|p| is_game_location(p));
        let trimmed = match game_idx {
            Some(idx) => parts[idx..].join("/"),
            None => continue,
        };
        if let Some((prio, mapped)) = translate_openiv_path(&trimmed) {
            let order = out.len();
            out.push(Candidate { rel: mapped, priority: prio.saturating_add(10), order, path });
        }
    }
}

fn walk_mod_root(root: &Path, dir: &Path, oivs: &mut Vec<PathBuf>, pkg_dirs: &mut Vec<PathBuf>, loose: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut has_assembly = false;
    let mut has_content = false;
    let mut children = Vec::new();
    for entry in rd.flatten() {
        let p = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if dir == root && name.to_ascii_lowercase().ends_with(".off") {
            continue;
        }
        if name.starts_with('.') {
            continue;
        }
        if let Ok(ft) = entry.file_type() {
            if ft.is_dir() {
                if name.eq_ignore_ascii_case("content") {
                    has_content = true;
                }
                children.push(p);
            } else if ft.is_file() {
                if name.eq_ignore_ascii_case("assembly.xml") {
                    has_assembly = true;
                } else if name.to_ascii_lowercase().ends_with(".oiv") {
                    oivs.push(p);
                } else {
                    loose.push(p);
                }
            }
        }
    }
    if dir != root && has_assembly && has_content {
        pkg_dirs.push(dir.to_path_buf());
        return;
    }
    for c in children {
        walk_mod_root(root, &c, oivs, pkg_dirs, loose);
    }
}

fn package_cache_key(path: &Path) -> String {
    let meta = fs::metadata(path).ok();
    let len = meta.as_ref().map_or(0, fs::Metadata::len);
    let mtime = meta
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    let stem: String = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("pkg")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("{stem}-{len}-{mtime}")
}

fn collect_oiv_dir(dir: &Path, out: &mut Vec<Candidate>) {
    let Ok(asm_path) = find_child_ci(dir, "assembly.xml") else { return };
    let Ok(content_dir) = find_child_ci(dir, "content") else { return };
    let Ok(xml) = fs::read_to_string(&asm_path) else { return };
    let adds = parse_oiv_adds(&xml);
    for (pc_path, source_rel) in adds {
        if let Some(src) = resolve_ci_path(&content_dir, &source_rel)
            && src.is_file()
            && let Some((priority, mapped)) = translate_openiv_path(&pc_path)
        {
            let order = out.len();
            out.push(Candidate { rel: mapped, priority, order, path: src });
        }
    }
}

fn find_child_ci(dir: &Path, target: &str) -> io::Result<PathBuf> {
    let direct = dir.join(target);
    if direct.exists() {
        return Ok(direct);
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_name().to_string_lossy().eq_ignore_ascii_case(target) {
            return Ok(entry.path());
        }
    }
    Err(io::Error::new(io::ErrorKind::NotFound, target))
}

fn resolve_ci_path(base: &Path, rel: &str) -> Option<PathBuf> {
    let mut cur = base.to_path_buf();
    for part in rel.split(['/', '\\']).filter(|s| !s.is_empty() && *s != ".") {
        if part == ".." {
            return None;
        }
        let next = cur.join(part);
        if next.exists() {
            cur = next;
        } else {
            cur = find_child_ci(&cur, part).ok()?;
        }
    }
    Some(cur)
}

/// Parses `<archive path="...">` nesting and `<add source="...">target</add>`
/// entries from OpenIV `assembly.xml`, returning `(full_pc_target, source)`.
pub(crate) fn parse_oiv_adds(xml: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let bytes = xml.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let Some(rel_lt) = xml[i..].find('<') else { break };
        i += rel_lt;
        if xml[i..].starts_with("<!--") {
            let Some(end) = xml[i..].find("-->") else { break };
            i += end + 3;
            continue;
        }
        let Some(rel_gt) = xml[i..].find('>') else { break };
        let tag = &xml[i + 1..i + rel_gt];
        i += rel_gt + 1;
        let trimmed = tag.trim();
        if trimmed.starts_with('?') || trimmed.starts_with('!') {
            continue;
        }
        if let Some(close_name) = trimmed.strip_prefix('/') {
            if close_name.trim().eq_ignore_ascii_case("archive") {
                stack.pop();
            }
            continue;
        }
        let self_closing = trimmed.ends_with('/');
        let inner = trimmed.trim_end_matches('/').trim();
        let name_end = inner.find(|c: char| c.is_ascii_whitespace()).unwrap_or(inner.len());
        let name = &inner[..name_end];
        let attrs = &inner[name_end..];
        if name.eq_ignore_ascii_case("archive") {
            if !self_closing {
                let p = xml_attr(attrs, "path").unwrap_or_default().replace('\\', "/");
                stack.push(p.trim_matches('/').to_owned());
            }
        } else if name.eq_ignore_ascii_case("add") && !self_closing {
            // Read inner text until </add>.
            let rest = &xml[i..];
            let Some(end_rel) = rest.find("</") else { continue };
            let target_raw = xml_unescape(rest[..end_rel].trim());
            if let Some(gt) = rest[end_rel..].find('>') {
                i += end_rel + gt + 1;
            }
            if let Some(source) = xml_attr(attrs, "source") {
                let target = target_raw.replace('\\', "/");
                let target = target.trim_matches('/');
                let mut prefix = String::new();
                for s in &stack {
                    if !s.is_empty() {
                        prefix.push_str(s);
                        prefix.push('/');
                    }
                }
                let full_target = format!("{prefix}{target}");
                let source = source.replace('\\', "/").trim_matches('/').to_owned();
                if is_safe_rel(&full_target) && is_safe_rel(&source) {
                    out.push((full_target, source));
                }
            }
        }
    }
    out
}

fn xml_attr(attrs: &str, wanted: &str) -> Option<String> {
    let mut s = attrs.trim_start();
    while !s.is_empty() {
        let eq = s.find('=')?;
        let key = s[..eq].trim();
        let after_eq = s[eq + 1..].trim_start();
        let quote = after_eq.chars().next()?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        let val_start = 1;
        let val_end = after_eq[val_start..].find(quote)?;
        let val = &after_eq[val_start..val_start + val_end];
        if key.eq_ignore_ascii_case(wanted) {
            return Some(xml_unescape(val));
        }
        s = after_eq[val_start + val_end + 1..].trim_start();
    }
    None
}

fn xml_unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'")
}

fn is_safe_rel(p: &str) -> bool {
    !p.is_empty() && !p.starts_with('/') && !p.contains(':') && !p.split('/').any(|c| c == "..")
}

fn is_game_location(folder: &str) -> bool {
    let f = folder.to_ascii_lowercase();
    matches!(f.as_str(), "x64" | "common" | "common.rpf" | "update" | "mods")
        || (f.starts_with("x64") && f.ends_with(".rpf") && f.len() <= 8)
}

/// Translates an OpenIV PC path (`x64b.rpf/...`, `update/update2.rpf/...`,
/// `update/update.rpf/x64/patch/...`, `common.rpf/...`) into `(priority, data_rel)`.
pub(crate) fn translate_openiv_path(raw: &str) -> Option<(u8, String)> {
    let clean = raw.replace('\\', "/");
    let mut s = clean.trim_start_matches('/').to_owned();
    let lower = s.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("mods/") {
        let _ = rest;
        s = s[5..].to_owned();
    }
    if !is_safe_rel(&s) {
        return None;
    }
    let low = s.to_ascii_lowercase();
    if let Some(rest) = low.strip_prefix("update/update2.rpf/") {
        let tail = &s["update/update2.rpf/".len()..];
        let mapped = if rest.starts_with("x64/patch/") {
            format!("x64/{}", &tail["x64/patch/".len()..])
        } else {
            tail.to_owned()
        };
        return Some((3, normalize_dlc_packs(&mapped)));
    }
    if let Some(rest) = low.strip_prefix("update/update.rpf/") {
        let tail = &s["update/update.rpf/".len()..];
        if rest.starts_with("x64/patch/") {
            return Some((2, normalize_dlc_packs(&format!("x64/{}", &tail["x64/patch/".len()..]))));
        }
        return Some((1, normalize_dlc_packs(tail)));
    }
    if low.starts_with("update/x64/dlcpacks/") {
        let tail = &s["update/x64/dlcpacks/".len()..];
        return Some((2, format!("x64/dlcPacks/{tail}")));
    }
    if low.starts_with("common.rpf/") {
        let tail = &s["common.rpf/".len()..];
        return Some((0, format!("common/{tail}")));
    }
    if let Some(slash) = low.find('/') {
        let head = &low[..slash];
        if head.starts_with("x64") && head.ends_with(".rpf") && head.len() <= 8 {
            let tail = &s[slash + 1..];
            return Some((0, normalize_dlc_packs(&format!("x64/{tail}"))));
        }
    }
    Some((1, normalize_dlc_packs(&s)))
}

fn normalize_dlc_packs(p: &str) -> String {
    let low = p.to_ascii_lowercase();
    if low.starts_with("x64/dlcpacks/") {
        format!("x64/dlcPacks/{}", &p["x64/dlcpacks/".len()..])
    } else {
        p.to_owned()
    }
}

fn build_game_mods(
    game_root: &Path,
    game_index: &HashMap<String, String>,
    cache_root: &Path,
    mut collected: Vec<Candidate>,
) -> GameMods {
    // Stable sort low -> high so later entries win; tie-break by discovery order.
    collected.sort_by_key(|c| (c.priority, c.order));

    // Also mirror `x64/data/lang/american_rel.rpf/...` onto `x64/data/lang/american.rpf/...`
    // because some builds open `american.rpf` instead of `american_REL.rpf`.
    let orig_len = collected.len();
    for i in 0..orig_len {
        let c = &collected[i];
        let low = c.rel.to_ascii_lowercase();
        if let Some(rest) = low.strip_prefix("x64/data/lang/american_rel.rpf/")
            && game_index.contains_key("x64/data/lang/american.rpf")
        {
            let tail = &c.rel["x64/data/lang/american_rel.rpf/".len()..];
            let _ = rest;
            collected.push(Candidate {
                rel: format!("x64/data/lang/american.rpf/{tail}"),
                priority: c.priority,
                order: c.order,
                path: c.path.clone(),
            });
        }
    }

    let mut loose: HashMap<String, PathBuf> = HashMap::new();
    // archive_lower -> (inner_lower -> Vec<Candidate>)
    let mut inside_layers: HashMap<String, BTreeMap<String, Vec<Candidate>>> = HashMap::new();

    for c in collected {
        let low = c.rel.to_ascii_lowercase();
        if let Some((archive_rel, inner_rel)) = split_archive_path(&low, game_index) {
            // Skip nested archives inside archives (e.g. `.rpf/.../.rpf/...`).
            if inner_rel.contains(".rpf/") {
                continue;
            }
            inside_layers.entry(archive_rel).or_default().entry(inner_rel).or_default().push(c);
        } else {
            // Loose file replacement (overrides existing game file or adds a new loose file).
            loose.insert(low, c.path);
        }
    }

    let merged_dir = cache_root.join("merged-gxt2");
    let mut archives: HashMap<String, Arc<ModArchive>> = HashMap::new();
    for (archive_lower, files_map) in inside_layers {
        let Some(real_rel) = game_index.get(&archive_lower) else { continue };
        let real_path = game_root.join(real_rel);
        let Ok(real_size) = fs::metadata(&real_path).map(|m| m.len()) else { continue };

        let mut cursor = align512(real_size);
        let mut slots = Vec::with_capacity(files_map.len());
        let mut by_inner = HashMap::with_capacity(files_map.len());

        for (inner_lower, layers) in files_map {
            let final_path = if inner_lower.ends_with(".gxt2") && layers.len() > 1 {
                merge_gxt2_layers(&merged_dir, &archive_lower, &inner_lower, &layers)
                    .unwrap_or_else(|| layers.last().unwrap().path.clone())
            } else {
                layers.last().unwrap().path.clone()
            };

            let Ok(meta) = fs::metadata(&final_path) else { continue };
            let length = meta.len();
            let rsc7 = read_rsc7_header(&final_path);
            if rsc7.is_some() && length >= MAX_RESOURCE_SIZE {
                log::warn!("game_mods: {inner_lower} exceeds 16 MiB RSC7 limit, skipped");
                continue;
            }
            let idx = slots.len();
            slots.push(ModSlot { inner_lower: inner_lower.clone(), offset: cursor, length, path: final_path, rsc7 });
            by_inner.insert(inner_lower, idx);
            cursor = align512(cursor + length);
        }

        if slots.is_empty() {
            continue;
        }
        log::info!(
            "game_mods: {} -> {} mod slot(s) (virtual size {:.2} MiB)",
            archive_lower,
            slots.len(),
            cursor as f64 / (1024.0 * 1024.0)
        );
        archives.insert(
            archive_lower.clone(),
            Arc::new(ModArchive {
                rel_lower: archive_lower,
                real_size,
                virtual_size: cursor,
                slots,
                by_inner,
                handles: Mutex::new(VecDeque::new()),
            }),
        );
    }

    if !loose.is_empty() || !archives.is_empty() {
        log::info!("game_mods: active ({} loose file(s), {} archive(s))", loose.len(), archives.len());
    }
    GameMods { loose, archives }
}

fn split_archive_path(low: &str, game_index: &HashMap<String, String>) -> Option<(String, String)> {
    let mut start = 0;
    while let Some(pos) = low[start..].find(".rpf/") {
        let end = start + pos + 4;
        let prefix = &low[..end];
        if game_index.contains_key(prefix) {
            return Some((prefix.to_owned(), low[end + 1..].to_owned()));
        }
        start = end + 1;
    }
    None
}

fn read_rsc7_header(path: &Path) -> Option<(u32, u32)> {
    let mut f = File::open(path).ok()?;
    let mut hdr = [0u8; 16];
    if f.read_exact(&mut hdr).is_err() {
        return None;
    }
    let magic = u32::from_le_bytes(hdr[0..4].try_into().ok()?);
    if magic != RSC7_MAGIC {
        return None;
    }
    let sys = u32::from_le_bytes(hdr[8..12].try_into().ok()?);
    let gfx = u32::from_le_bytes(hdr[12..16].try_into().ok()?);
    Some((sys, gfx))
}

fn index_game_tree(root: &Path, rel: &str, out: &mut HashMap<String, String>) -> io::Result<()> {
    let dir = if rel.is_empty() { root.to_path_buf() } else { root.join(rel) };
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = if rel.is_empty() { name } else { format!("{rel}/{name}") };
        out.insert(path.to_ascii_lowercase(), path.clone());
        let ft = entry.file_type()?;
        if ft.is_dir() || (ft.is_symlink() && fs::metadata(entry.path()).is_ok_and(|m| m.is_dir())) {
            index_game_tree(root, &path, out)?;
        }
    }
    Ok(())
}

/// Parses a `GXT2` string table into `hash -> raw C string bytes`.
pub(crate) fn parse_gxt2(data: &[u8]) -> Option<Vec<(u32, Vec<u8>)>> {
    if data.len() < 8 || u32::from_le_bytes(data[0..4].try_into().ok()?) != GXT2_MAGIC {
        return None;
    }
    let count = u32::from_le_bytes(data[4..8].try_into().ok()?) as usize;
    let table_end = 8usize.checked_add(count.checked_mul(8)?)?;
    if data.len() < table_end {
        return None;
    }
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let base = 8 + i * 8;
        let hash = u32::from_le_bytes(data[base..base + 4].try_into().ok()?);
        let off = u32::from_le_bytes(data[base + 4..base + 8].try_into().ok()?) as usize;
        if off >= data.len() {
            continue;
        }
        let end = data[off..].iter().position(|&b| b == 0).map_or(data.len(), |n| off + n);
        out.push((hash, data[off..end].to_vec()));
    }
    Some(out)
}

/// Serializes sorted `(hash, utf8_bytes)` pairs back into `GXT2` format.
pub(crate) fn encode_gxt2(entries: &BTreeMap<u32, Vec<u8>>) -> Vec<u8> {
    let count = entries.len();
    let header_len = 8 + count * 8 + 8;
    let payload_len: usize = entries.values().map(|v| v.len() + 1).sum();
    let total_len = header_len + payload_len;

    let mut out = Vec::with_capacity(total_len);
    out.extend_from_slice(&GXT2_MAGIC.to_le_bytes());
    out.extend_from_slice(&(count as u32).to_le_bytes());
    let mut off = header_len;
    for (&hash, val) in entries {
        out.extend_from_slice(&hash.to_le_bytes());
        out.extend_from_slice(&(off as u32).to_le_bytes());
        off += val.len() + 1;
    }
    out.extend_from_slice(&GXT2_MAGIC.to_le_bytes());
    out.extend_from_slice(&(total_len as u32).to_le_bytes());
    for val in entries.values() {
        out.extend_from_slice(val);
        out.push(0);
    }
    out
}

fn merge_gxt2_layers(merged_dir: &Path, archive_lower: &str, inner_lower: &str, layers: &[Candidate]) -> Option<PathBuf> {
    let mut combined: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    for layer in layers {
        let bytes = fs::read(&layer.path).ok()?;
        let entries = parse_gxt2(&bytes)?;
        for (hash, val) in entries {
            match combined.get(&hash) {
                // Keep higher-priority translation unless it is plain ASCII while we already have UTF-8 non-ASCII.
                Some(prev) if !has_non_ascii(&val) && has_non_ascii(prev) => {}
                _ => {
                    combined.insert(hash, val);
                }
            }
        }
    }
    let out_path = merged_dir.join(archive_lower.replace('/', "_")).join(inner_lower);
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).ok()?;
    }
    let encoded = encode_gxt2(&combined);
    fs::write(&out_path, encoded).ok()?;
    Some(out_path)
}

fn has_non_ascii(bytes: &[u8]) -> bool {
    bytes.iter().any(|&b| b >= 0x80)
}

/// Minimal pure-Rust ZIP / `.oiv` unpacker using `zlib-rs` (handles Store=0
/// and Deflate=8; sizes come authoritatively from the Central Directory).
pub(crate) fn unpack_zip(zip_path: &Path, dest: &Path) -> io::Result<()> {
    let data = fs::read(zip_path)?;
    if data.len() < 22 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "file too short for zip"));
    }
    let min_eocd = data.len().saturating_sub(65558);
    let eocd = (min_eocd..=data.len() - 22)
        .rev()
        .find(|&i| &data[i..i + 4] == b"PK\x05\x06")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing zip EOCD"))?;

    let cd_entries = u16::from_le_bytes(data[eocd + 10..eocd + 12].try_into().unwrap()) as usize;
    let cd_offset = u32::from_le_bytes(data[eocd + 16..eocd + 20].try_into().unwrap()) as usize;
    if cd_offset >= data.len() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "zip central directory out of bounds"));
    }

    fs::create_dir_all(dest)?;
    let mut at = cd_offset;
    let mut inflate = Inflate::new(false, 15);
    for _ in 0..cd_entries {
        if at + 46 > data.len() || &data[at..at + 4] != b"PK\x01\x02" {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "corrupt zip central directory entry"));
        }
        let flags = u16::from_le_bytes(data[at + 8..at + 10].try_into().unwrap());
        let method = u16::from_le_bytes(data[at + 10..at + 12].try_into().unwrap());
        let packed_len = u32::from_le_bytes(data[at + 20..at + 24].try_into().unwrap()) as usize;
        let unpacked_len = u32::from_le_bytes(data[at + 24..at + 28].try_into().unwrap()) as usize;
        let name_len = u16::from_le_bytes(data[at + 28..at + 30].try_into().unwrap()) as usize;
        let extra_len = u16::from_le_bytes(data[at + 30..at + 32].try_into().unwrap()) as usize;
        let comment_len = u16::from_le_bytes(data[at + 32..at + 34].try_into().unwrap()) as usize;
        let local_hdr = u32::from_le_bytes(data[at + 42..at + 46].try_into().unwrap()) as usize;

        if at + 46 + name_len > data.len() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "zip entry name truncated"));
        }
        let name_bytes = &data[at + 46..at + 46 + name_len];
        let name = decode_zip_name(name_bytes, flags & 0x0800 != 0).replace('\\', "/");
        at += 46 + name_len + extra_len + comment_len;

        if name.ends_with('/') {
            continue;
        }
        if !is_safe_rel(&name) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, format!("unsafe zip entry path: {name}")));
        }
        if flags & 1 != 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, format!("encrypted zip entry: {name}")));
        }
        if local_hdr + 30 > data.len() || &data[local_hdr..local_hdr + 4] != b"PK\x03\x04" {
            return Err(io::Error::new(io::ErrorKind::InvalidData, format!("bad local header: {name}")));
        }
        let l_name = u16::from_le_bytes(data[local_hdr + 26..local_hdr + 28].try_into().unwrap()) as usize;
        let l_extra = u16::from_le_bytes(data[local_hdr + 28..local_hdr + 30].try_into().unwrap()) as usize;
        let payload_start = local_hdr + 30 + l_name + l_extra;
        let payload_end = payload_start.checked_add(packed_len).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, format!("zip payload overflow: {name}"))
        })?;
        if payload_end > data.len() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, format!("zip payload truncated: {name}")));
        }
        let compressed = &data[payload_start..payload_end];

        let out_path = dest.join(&name);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        match method {
            0 => fs::write(&out_path, compressed)?,
            8 => {
                inflate.reset(false);
                let mut out = vec![0u8; unpacked_len];
                match inflate.decompress(compressed, &mut out, InflateFlush::Finish) {
                    Ok(Status::StreamEnd) if inflate.total_out() as usize == unpacked_len => {
                        fs::write(&out_path, &out)?;
                    }
                    Ok(_) => {
                        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("incomplete deflate: {name}")));
                    }
                    Err(e) => {
                        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("deflate error in {name}: {e:?}")));
                    }
                }
            }
            m => return Err(io::Error::new(io::ErrorKind::Unsupported, format!("zip method {m} in {name}"))),
        }
    }
    Ok(())
}

fn decode_zip_name(bytes: &[u8], utf8: bool) -> String {
    if utf8 {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_owned();
    }
    bytes.iter().map(|&b| b as char).collect()
}

/// Normalizes an engine packfile path (`/game/x64/...` or `platform:/...`) to
/// a relative `data/` path (`x64/...` or `common/...`).
fn normalize_archive_name(engine_path: &str) -> Option<String> {
    let mut s = engine_path.trim().replace('\\', "/");
    if let Some(rest) = s.strip_prefix("/game/") {
        s = rest.to_owned();
    }
    let low = s.to_ascii_lowercase();
    let normalized = if let Some(rest) = low.strip_prefix("platform:") {
        format!("x64/{}", rest.trim_start_matches('/'))
    } else if let Some(rest) = low.strip_prefix("common:") {
        format!("common/{}", rest.trim_start_matches('/'))
    } else if low.contains(':') {
        // Sub-archive inside another archive (e.g. "dlcMPBeach:/...").
        return None;
    } else {
        low.trim_start_matches('/').to_owned()
    };
    Some(normalized)
}

/// Patches a mounted RPF7 packfile's decrypted in-memory entry table so any
/// modded files point to their 512-byte-aligned slots in [`ModArchive`].
pub(crate) fn apply_to_packfile(env: *mut Env, packfile: u64, name_ptr: u64) {
    if packfile == 0 || name_ptr == 0 {
        return;
    }
    let m = unsafe { Mem::of(env) };
    let engine_name = m.str(name_ptr);
    let Some(rel) = normalize_archive_name(&engine_name) else { return };
    let Some(archive) = rt().vfs.mod_archive(&rel).cloned() else { return };

    let entries = m.u64(packfile + 32);
    let names = m.u64(packfile + 16);
    let shift = m.u8(packfile + 184) as usize;
    if entries == 0 || names == 0 {
        return;
    }
    let mut path = String::with_capacity(256);
    let patched = patch_rpf_tree(m, entries, names, shift, 0, &mut path, 0, &archive);
    if patched > 0 {
        log::info!("game_mods: {} patched {patched}/{} entries", archive.rel_lower, archive.slots.len());
    }
}

fn patch_rpf_tree(
    m: Mem,
    entries: u64,
    names: u64,
    shift: usize,
    index: u32,
    path: &mut String,
    depth: u32,
    archive: &ModArchive,
) -> usize {
    if depth > 32 {
        return 0;
    }
    let entry_addr = entries + u64::from(index) * 16;
    let e0 = m.u64(entry_addr);
    let e1 = m.u64(entry_addr + 8);
    let name_off = (e0 & 0xFFFF) as u64;
    let name = m.str(names + (name_off << shift));

    let saved_len = path.len();
    if index != 0 {
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(&name);
    }

    let block_offset = (e0 >> 40) & 0x7F_FFFF;
    let mut patched = 0;
    if block_offset == 0x7F_FFFF {
        let first = (e1 & 0xFFFF_FFFF) as u32;
        let count = (e1 >> 32) as u32;
        if count <= 200_000 {
            for i in 0..count {
                patched += patch_rpf_tree(m, entries, names, shift, first + i, path, depth + 1, archive);
            }
        }
    } else if let Some(&slot_idx) = archive.by_inner.get(&path.to_ascii_lowercase()) {
        let slot = &archive.slots[slot_idx];
        let block = slot.offset / ALIGN;
        if block < 0x7F_FFFF {
            let (new_e0, new_e1) = match slot.rsc7 {
                Some((sys, gfx)) => (
                    name_off | ((slot.length & MAX_RESOURCE_SIZE) << 16) | (block << 40) | (1u64 << 63),
                    u64::from(sys) | (u64::from(gfx) << 32),
                ),
                // Plain uncompressed, unencrypted data (high 32 bits of e1 = 0 disables AES).
                None => (name_off | (block << 40), slot.length & 0xFFFF_FFFF),
            };
            m.set_u64(entry_addr, new_e0);
            m.set_u64(entry_addr + 8, new_e1);
            patched += 1;
        }
    }

    path.truncate(saved_len);
    patched
}

#[cfg(unix)]
fn read_file_at(f: &File, buf: &mut [u8], off: u64) -> io::Result<usize> {
    std::os::unix::fs::FileExt::read_at(f, buf, off)
}

#[cfg(windows)]
fn read_file_at(f: &File, buf: &mut [u8], off: u64) -> io::Result<usize> {
    std::os::windows::fs::FileExt::seek_read(f, buf, off)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openiv_paths_and_layer_priorities() {
        assert_eq!(translate_openiv_path("common.rpf\\data\\scaleform\\gfxfontlib.gfx"), Some((0, "common/data/scaleform/gfxfontlib.gfx".into())));
        assert_eq!(translate_openiv_path("x64b.rpf/data/lang/american_rel.rpf/global.gxt2"), Some((0, "x64/data/lang/american_rel.rpf/global.gxt2".into())));
        assert_eq!(translate_openiv_path("update\\update.rpf\\common\\data\\ui\\fontmap.xml"), Some((1, "common/data/ui/fontmap.xml".into())));
        assert_eq!(translate_openiv_path("update/update.rpf/x64/patch/data/cdimages/scaleform_generic.rpf/a.gfx"), Some((2, "x64/data/cdimages/scaleform_generic.rpf/a.gfx".into())));
        assert_eq!(translate_openiv_path("update/update2.rpf/x64/data/lang/american_rel.rpf/global.gxt2"), Some((3, "x64/data/lang/american_rel.rpf/global.gxt2".into())));
        assert_eq!(translate_openiv_path("update/x64/dlcpacks/mpcar/dlc.rpf"), Some((2, "x64/dlcPacks/mpcar/dlc.rpf".into())));
    }

    #[test]
    fn parses_nested_oiv_assembly_xml() {
        let xml = r#"
        <package target="Five">
          <content>
            <archive path="update\update2.rpf" createIfNotExist="True">
              <archive path="x64\data\lang\american_rel.rpf">
                <add source="file\update\update2.rpf\x64\data\lang\american_rel.rpf\global.gxt2">global.gxt2</add>
              </archive>
            </archive>
            <archive path="common.rpf">
              <add source="file\common.rpf\data\scaleform\gfxfontlib.gfx">data\scaleform\gfxfontlib.gfx</add>
            </archive>
          </content>
        </package>"#;
        let adds = parse_oiv_adds(xml);
        assert_eq!(adds.len(), 2);
        assert_eq!(adds[0].0, "update/update2.rpf/x64/data/lang/american_rel.rpf/global.gxt2");
        assert_eq!(adds[0].1, "file/update/update2.rpf/x64/data/lang/american_rel.rpf/global.gxt2");
        assert_eq!(adds[1].0, "common.rpf/data/scaleform/gfxfontlib.gfx");
    }

    #[test]
    fn merge_gxt2_preserves_missing_keys_and_prefers_vietnamese() {
        let dir = std::env::temp_dir().join(format!("glue5-gxt2-merge-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let mut base = BTreeMap::new();
        base.insert(1u32, "Only in base".as_bytes().to_vec());
        base.insert(2u32, "Cài đặt".as_bytes().to_vec());
        base.insert(3u32, "Old ASCII".as_bytes().to_vec());
        let p0 = dir.join("base.gxt2");
        fs::write(&p0, encode_gxt2(&base)).unwrap();

        let mut upd = BTreeMap::new();
        upd.insert(2u32, "Settings".as_bytes().to_vec()); // lower ASCII must not clobber UTF-8 Vietnamese
        upd.insert(3u32, "Bản đồ".as_bytes().to_vec());
        let p1 = dir.join("upd.gxt2");
        fs::write(&p1, encode_gxt2(&upd)).unwrap();

        let layers = [
            Candidate { rel: "a".into(), priority: 0, order: 0, path: p0 },
            Candidate { rel: "a".into(), priority: 3, order: 1, path: p1 },
        ];
        let merged = merge_gxt2_layers(&dir.join("out"), "x64/data/lang/american_rel.rpf", "global.gxt2", &layers).unwrap();
        let parsed: BTreeMap<u32, Vec<u8>> = parse_gxt2(&fs::read(merged).unwrap()).unwrap().into_iter().collect();
        assert_eq!(std::str::from_utf8(&parsed[&1]).unwrap(), "Only in base");
        assert_eq!(std::str::from_utf8(&parsed[&2]).unwrap(), "Cài đặt");
        assert_eq!(std::str::from_utf8(&parsed[&3]).unwrap(), "Bản đồ");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn zip_unpack_and_mod_archive_pread_stitch_cleanly() {
        let dir = std::env::temp_dir().join(format!("glue5-zip-archive-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // Build a minimal zip containing one stored file and one deflated file.
        let zip_path = dir.join("test.oiv");
        let payload_plain = b"hello".as_slice();
        let payload_def_src = "Cài đặt đồ họa GTA V tiếng Việt có dấu!".repeat(10);
        let mut def = zlib_rs::Deflate::new(6, false, 15);
        let mut compressed = vec![0u8; payload_def_src.len() + 64];
        def.compress(payload_def_src.as_bytes(), &mut compressed, zlib_rs::DeflateFlush::Finish).unwrap();
        compressed.truncate(def.total_out() as usize);

        let mut z = Vec::new();
        let add_local = |z: &mut Vec<u8>, name: &[u8], method: u16, packed: &[u8], uncomp: u32| -> u32 {
            let off = z.len() as u32;
            z.extend_from_slice(b"PK\x03\x04");
            z.extend_from_slice(&20u16.to_le_bytes());
            z.extend_from_slice(&0x0800u16.to_le_bytes());
            z.extend_from_slice(&method.to_le_bytes());
            z.extend_from_slice(&0u32.to_le_bytes());
            z.extend_from_slice(&0u32.to_le_bytes());
            z.extend_from_slice(&(packed.len() as u32).to_le_bytes());
            z.extend_from_slice(&uncomp.to_le_bytes());
            z.extend_from_slice(&(name.len() as u16).to_le_bytes());
            z.extend_from_slice(&0u16.to_le_bytes());
            z.extend_from_slice(name);
            z.extend_from_slice(packed);
            off
        };
        let off1 = add_local(&mut z, b"content/a.txt", 0, payload_plain, payload_plain.len() as u32);
        let off2 = add_local(&mut z, b"content/b.txt", 8, &compressed, payload_def_src.len() as u32);
        let cd_off = z.len() as u32;
        let add_cd = |z: &mut Vec<u8>, name: &[u8], method: u16, packed_len: u32, uncomp: u32, local_off: u32| {
            z.extend_from_slice(b"PK\x01\x02");
            z.extend_from_slice(&20u16.to_le_bytes());
            z.extend_from_slice(&20u16.to_le_bytes());
            z.extend_from_slice(&0x0800u16.to_le_bytes());
            z.extend_from_slice(&method.to_le_bytes());
            z.extend_from_slice(&0u32.to_le_bytes());
            z.extend_from_slice(&0u32.to_le_bytes());
            z.extend_from_slice(&packed_len.to_le_bytes());
            z.extend_from_slice(&uncomp.to_le_bytes());
            z.extend_from_slice(&(name.len() as u16).to_le_bytes());
            z.extend_from_slice(&0u32.to_le_bytes());
            z.extend_from_slice(&0u32.to_le_bytes());
            z.extend_from_slice(&0u32.to_le_bytes());
            z.extend_from_slice(&local_off.to_le_bytes());
            z.extend_from_slice(name);
        };
        add_cd(&mut z, b"content/a.txt", 0, payload_plain.len() as u32, payload_plain.len() as u32, off1);
        add_cd(&mut z, b"content/b.txt", 8, compressed.len() as u32, payload_def_src.len() as u32, off2);
        let cd_size = z.len() as u32 - cd_off;
        z.extend_from_slice(b"PK\x05\x06");
        z.extend_from_slice(&0u32.to_le_bytes());
        z.extend_from_slice(&2u16.to_le_bytes());
        z.extend_from_slice(&2u16.to_le_bytes());
        z.extend_from_slice(&cd_size.to_le_bytes());
        z.extend_from_slice(&cd_off.to_le_bytes());
        z.extend_from_slice(&0u16.to_le_bytes());
        fs::write(&zip_path, &z).unwrap();

        let out = dir.join("unpacked");
        unpack_zip(&zip_path, &out).unwrap();
        assert_eq!(fs::read(out.join("content/a.txt")).unwrap(), payload_plain);
        assert_eq!(fs::read_to_string(out.join("content/b.txt")).unwrap(), payload_def_src);

        // Verify stitched pread over base file + 512-byte gap + slot file.
        let base_path = dir.join("base.rpf");
        fs::write(&base_path, vec![0xAAu8; 300]).unwrap();
        let archive = ModArchive {
            rel_lower: "x64/test.rpf".into(),
            real_size: 300,
            virtual_size: 1024,
            slots: vec![ModSlot {
                inner_lower: "a.txt".into(),
                offset: 512,
                length: 5,
                path: out.join("content/a.txt"),
                rsc7: None,
            }],
            by_inner: [("a.txt".into(), 0)].into_iter().collect(),
            handles: Mutex::new(VecDeque::new()),
        };
        let base_file = File::open(&base_path).unwrap();
        let mut full = vec![0u8; 600];
        let n = archive.read_at(&base_file, 0, &mut full).unwrap();
        assert_eq!(n, 600);
        assert!(full[..300].iter().all(|&b| b == 0xAA));
        assert!(full[300..512].iter().all(|&b| b == 0));
        assert_eq!(&full[512..517], b"hello");
        assert!(full[517..600].iter().all(|&b| b == 0));

        let _ = fs::remove_dir_all(&dir);
    }
}
