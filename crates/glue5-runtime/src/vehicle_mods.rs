//! Vehicle Modding Subsystem for GLUE 5.
//!
//! Provides non-destructive vehicle mod management:
//! - Auto-imports `.zip`, `.rar`, `.7z`, `.oiv`, and folder archives.
//! - Classifies mod type automatically:
//!   * Add-on cars: packages containing `dlc.rpf`, layered to `x64/dlcPacks/<pack>/dlc.rpf`
//!     and automatically merged into `dlcList.xml`.
//!   * Replace cars: `.yft` / `.ytd` models replacing game vehicles in
//!     `x64/levels/gta5/vehicles.rpf` via in-memory RPF7 virtual slot patching.
//!   * OpenIV `.oiv` packages and script/trainer assets (`menyooStuff/`).
//! - Manages persistent state in `<user_dir>/mods/vehicles.json`.
//! - Generates dynamic `dlcList.xml` overlay on the fly without touching game data.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VehicleModKind {
    Addon,
    Replace,
    Package,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VehicleMod {
    pub id: String,
    pub title: String,
    pub kind: VehicleModKind,
    pub pack_name: Option<String>,
    pub models: Vec<String>,
    pub replaces: Vec<String>,
    pub enabled: bool,
    pub files_count: usize,
    pub date: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VehicleDatabase {
    pub mods: Vec<VehicleMod>,
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn sanitize_id(raw: &str) -> String {
    let s: String = raw
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c.to_ascii_lowercase() } else { '_' })
        .collect();
    let trimmed = s.trim_matches('_');
    if trimmed.is_empty() { "mod".into() } else { trimmed.into() }
}

fn db_path(user_dir: &Path) -> PathBuf {
    user_dir.join("mods").join("vehicles.json")
}

pub fn load_db(user_dir: &Path) -> VehicleDatabase {
    let path = db_path(user_dir);
    if let Ok(data) = fs::read(&path)
        && let Ok(db) = serde_json::from_slice::<VehicleDatabase>(&data)
    {
        return db;
    }
    VehicleDatabase::default()
}

pub fn save_db(user_dir: &Path, db: &VehicleDatabase) -> io::Result<()> {
    let path = db_path(user_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_vec_pretty(db).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(&path, &data)?;

    // Sync directly into glue-trainer's storage (.glue5/storage/glue-trainer.json)
    let trainer_storage = user_dir.join("mods").join(".glue5").join("storage").join("glue-trainer.json");
    if let Some(parent) = trainer_storage.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut map: serde_json::Map<String, serde_json::Value> = fs::read(&trainer_storage)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    if let Ok(val) = serde_json::to_value(&db.mods) {
        map.insert("vehicles".to_string(), val);
        if let Ok(out_bytes) = serde_json::to_vec_pretty(&map) {
            let _ = fs::write(&trainer_storage, out_bytes);
        }
    }
    Ok(())
}

pub fn list_mods(user_dir: &Path) -> Vec<VehicleMod> {
    load_db(user_dir).mods
}

pub fn set_enabled(user_dir: &Path, id: &str, enabled: bool) -> Result<(), String> {
    let mut db = load_db(user_dir);
    if let Some(m) = db.mods.iter_mut().find(|m| m.id == id) {
        m.enabled = enabled;
        save_db(user_dir, &db).map_err(|e| format!("cannot save db: {e}"))?;
        Ok(())
    } else {
        Err(format!("Mod not found: {id}"))
    }
}

pub fn delete_mod(user_dir: &Path, id: &str) -> Result<(), String> {
    let mut db = load_db(user_dir);
    if let Some(pos) = db.mods.iter().position(|m| m.id == id) {
        db.mods.remove(pos);
        let _ = save_db(user_dir, &db);
        let mod_dir = user_dir.join("mods").join("vehicles").join(id);
        if mod_dir.exists() {
            let _ = fs::remove_dir_all(&mod_dir);
        }
        Ok(())
    } else {
        Err(format!("Mod not found: {id}"))
    }
}

/// Dynamic `dlcList.xml` generation:
/// Merges all enabled Add-on vehicle packs into a loose `dlcList.xml` overlay.
pub fn sync_dlclist_overlay(game_data: &Path, user_dir: &Path) -> Option<PathBuf> {
    let base_xml_path = game_data.join("common").join("data").join("dlcList.xml");
    let base_xml = if base_xml_path.is_file() {
        fs::read_to_string(&base_xml_path).ok()?
    } else {
        let alt = game_data.join("common").join("data").join("dlclist.xml");
        fs::read_to_string(&alt).ok()?
    };

    let db = load_db(user_dir);
    let mut packs_to_add = BTreeSet::new();

    for m in &db.mods {
        if m.enabled && m.kind == VehicleModKind::Addon {
            if let Some(pack) = &m.pack_name {
                let pack_clean = pack.trim();
                if !pack_clean.is_empty() {
                    packs_to_add.insert(pack_clean.to_string());
                }
            }
        }
    }

    // Also scan `<user_dir>/mods/vehicles/*/x64/dlcPacks/*` directly in case of manual mods
    let vehicles_dir = user_dir.join("mods").join("vehicles");
    if let Ok(entries) = fs::read_dir(&vehicles_dir) {
        for entry in entries.flatten() {
            let p = entry.path().join("x64").join("dlcPacks");
            if let Ok(dlc_entries) = fs::read_dir(&p) {
                for dlc in dlc_entries.flatten() {
                    if dlc.path().join("dlc.rpf").is_file() {
                        let name = dlc.file_name().to_string_lossy().into_owned();
                        // Only add if not disabled in DB
                        let is_disabled = db.mods.iter().any(|m| m.pack_name.as_deref() == Some(&name) && !m.enabled);
                        if !is_disabled {
                            packs_to_add.insert(name);
                        }
                    }
                }
            }
        }
    }

    if packs_to_add.is_empty() {
        return None;
    }

    let mut merged = base_xml.clone();
    let close_tag = "</Paths>";
    let Some(insert_pos) = merged.rfind(close_tag) else { return None };

    let mut extra = String::new();
    let lower_base = base_xml.to_ascii_lowercase();
    for pack in packs_to_add {
        let needle = format!("dlcpacks:\\{}\\<", pack.to_ascii_lowercase());
        let needle2 = format!("dlcpacks:/{}/<", pack.to_ascii_lowercase());
        let needle3 = format!("dlcpacks\\{}\\", pack.to_ascii_lowercase());
        let needle4 = format!("dlcpacks/{}/", pack.to_ascii_lowercase());
        if !lower_base.contains(&needle)
            && !lower_base.contains(&needle2)
            && !lower_base.contains(&needle3)
            && !lower_base.contains(&needle4)
        {
            extra.push_str(&format!("\t\t<Item>platform:\\dlcPacks\\{pack}\\</Item>\n"));
        }
    }

    if extra.is_empty() {
        return None;
    }

    merged.insert_str(insert_pos, &extra);

    let out_dir = user_dir.join("cache").join("game-mods").join("dlclist");
    let _ = fs::create_dir_all(&out_dir);
    let out_path = out_dir.join("dlcList.xml");
    fs::write(&out_path, merged.as_bytes()).ok()?;
    log::info!("vehicle_mods: injected DLC packs into dlcList.xml overlay at {}", out_path.display());
    Some(out_path)
}

/// Unpacks any archive (`.zip`, `.rar`, `.7z`, `.oiv`, etc.) to `dest`.
pub fn unpack_archive(archive_path: &Path, dest: &Path) -> Result<(), String> {
    let _ = fs::create_dir_all(dest);

    // Try 7-Zip CLI first (handles zip, rar, 7z, tar, etc.)
    let seven_zip_candidates = [
        PathBuf::from("C:\\Program Files\\7-Zip\\7z.exe"),
        PathBuf::from("C:\\Program Files (x86)\\7-Zip\\7z.exe"),
        PathBuf::from("7z.exe"),
        PathBuf::from("7z"),
    ];

    for candidate in &seven_zip_candidates {
        if candidate.is_file() || candidate.as_os_str() == "7z.exe" || candidate.as_os_str() == "7z" {
            let out = Command::new(candidate)
                .arg("x")
                .arg("-y")
                .arg(format!("-o{}", dest.display()))
                .arg(archive_path)
                .output();
            if let Ok(res) = out {
                if res.status.success() {
                    return Ok(());
                }
            }
        }
    }

    // Fallback: if .zip or .oiv, use pure-Rust unpack_zip
    let ext = archive_path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if ext == "zip" || ext == "oiv" {
        return crate::game_mods::unpack_zip(archive_path, dest).map_err(|e| format!("zip unpack error: {e}"));
    }

    Err(format!(
        "Cannot unpack {}: install 7-Zip (7z.exe) for RAR/7Z archive support",
        archive_path.display()
    ))
}

/// Extracts model names from an RPF7 `dlc.rpf` by locating `vehicles.meta`.
fn extract_models_from_dlc_rpf(dlc_path: &Path) -> Vec<String> {
    let mut out = BTreeSet::new();
    let Ok(data) = fs::read(dlc_path) else { return Vec::new() };
    if data.len() < 16 {
        return Vec::new();
    }
    let magic = u32::from_le_bytes(data[0..4].try_into().unwrap_or_default());
    if magic != 0x52504637 {
        return Vec::new();
    }
    let entries_cnt = u32::from_le_bytes(data[4..8].try_into().unwrap_or_default()) as usize;
    let names_len = u32::from_le_bytes(data[8..12].try_into().unwrap_or_default()) as usize;
    let header_sz = 16 + entries_cnt * 16 + names_len;
    if data.len() < header_sz {
        return Vec::new();
    }

    let entries_raw = &data[16..16 + entries_cnt * 16];
    let names_raw = &data[16 + entries_cnt * 16..header_sz];

    for i in 0..entries_cnt {
        let e_slice = &entries_raw[i * 16..(i + 1) * 16];
        let e0 = u64::from_le_bytes(e_slice[0..8].try_into().unwrap_or_default());
        let name_off = (e0 & 0xFFFF) as usize;
        let is_dir = ((e0 >> 40) & 0x7F_FFFF) == 0x7F_FFFF;
        if is_dir || name_off >= names_raw.len() {
            continue;
        }
        let end = names_raw[name_off..].iter().position(|&b| b == 0).unwrap_or(names_raw.len() - name_off);
        let name = String::from_utf8_lossy(&names_raw[name_off..name_off + end]);
        if name.eq_ignore_ascii_case("vehicles.meta") {
            let block = ((e0 >> 40) & 0x7F_FFFF) as usize;
            let length = ((e0 >> 16) & 0xFF_FFFF) as usize;
            let byte_off = block * 512;
            if byte_off + length <= data.len() {
                let chunk = &data[byte_off..byte_off + length];
                if let Some(xml) = decompress_raw_deflate(chunk) {
                    scan_model_names(&xml, &mut out);
                } else if let Ok(s) = std::str::from_utf8(chunk) {
                    scan_model_names(s, &mut out);
                }
            }
        }
    }

    if out.is_empty() {
        // Fallback: fast scan for `<modelName>` tags across the whole file
        let text = String::from_utf8_lossy(&data);
        scan_model_names(&text, &mut out);
    }

    out.into_iter().collect()
}

fn decompress_raw_deflate(chunk: &[u8]) -> Option<String> {
    use zlib_rs::{Inflate, InflateFlush, Status};
    let mut inflate = Inflate::new(false, 15);
    let mut out = vec![0u8; chunk.len() * 10 + 8192];
    match inflate.decompress(chunk, &mut out, InflateFlush::Finish) {
        Ok(Status::StreamEnd) | Ok(Status::Ok) => {
            let written = inflate.total_out() as usize;
            out.truncate(written);
            String::from_utf8(out).ok()
        }
        _ => None,
    }
}

fn scan_model_names(xml: &str, out: &mut BTreeSet<String>) {
    let mut at = 0;
    while let Some(pos) = xml[at..].find("<modelName>") {
        let start = at + pos + "<modelName>".len();
        if let Some(end_rel) = xml[start..].find("</modelName>") {
            let name = xml[start..start + end_rel].trim();
            if !name.is_empty() && name.len() < 32 {
                out.insert(name.to_ascii_lowercase());
            }
            at = start + end_rel + "</modelName>".len();
        } else {
            break;
        }
    }
}

/// Recursively discovers all files in directory `dir`.
fn list_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            list_files_recursive(&p, out);
        } else if p.is_file() {
            out.push(p);
        }
    }
}

/// Imports an archive file (.zip, .rar, .7z, .oiv) or folder into `<user_dir>/mods/vehicles/<id>`.
pub fn import_archive(archive_path: &Path, user_dir: &Path, game_data: Option<&Path>) -> Result<VehicleMod, String> {
    if !archive_path.exists() {
        return Err(format!("File does not exist: {}", archive_path.display()));
    }

    let file_stem = archive_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("vehicle_mod");
    let mod_id = sanitize_id(file_stem);

    let temp_unpack = user_dir.join("cache").join("game-mods").join("staging").join(&mod_id);
    let _ = fs::remove_dir_all(&temp_unpack);

    if archive_path.is_dir() {
        copy_dir_recursive(archive_path, &temp_unpack).map_err(|e| format!("cannot copy folder: {e}"))?;
    } else {
        unpack_archive(archive_path, &temp_unpack)?;
    }

    let mut files = Vec::new();
    list_files_recursive(&temp_unpack, &mut files);
    if files.is_empty() {
        let _ = fs::remove_dir_all(&temp_unpack);
        return Err("Archive is empty or failed to extract".into());
    }

    // Check if MenyooStuff exists
    let menyoo_dir = files.iter().find_map(|p| {
        let s = p.to_string_lossy().to_ascii_lowercase();
        if s.contains("menyoostuff") {
            let mut cur = p.as_path();
            while let Some(parent) = cur.parent() {
                if cur.file_name().map_or(false, |n| n.to_string_lossy().eq_ignore_ascii_case("menyoostuff")) {
                    return Some(cur.to_path_buf());
                }
                cur = parent;
            }
        }
        None
    });

    if let Some(src_menyoo) = menyoo_dir {
        let target_menyoo = user_dir.join("mods").join("menyooStuff");
        let _ = copy_dir_recursive(&src_menyoo, &target_menyoo);
        log::info!("vehicle_mods: copied menyooStuff to {}", target_menyoo.display());
    }

    // Check 1: Is this an Add-on car? (Contains `dlc.rpf`)
    let dlc_rpf_opt = files.iter().find(|p| p.file_name().map_or(false, |n| n.to_string_lossy().eq_ignore_ascii_case("dlc.rpf")));

    if let Some(dlc_src) = dlc_rpf_opt {
        // Parent folder of dlc.rpf is typically the pack name (e.g. `vans123/dlc.rpf`)
        let parent_name = dlc_src.parent().and_then(|p| p.file_name()).and_then(|s| s.to_str());
        let pack_name = match parent_name {
            Some(n) if !n.eq_ignore_ascii_case("content") && !n.is_empty() && n != mod_id => sanitize_id(n),
            _ => mod_id.clone(),
        };

        let models = extract_models_from_dlc_rpf(dlc_src);
        let final_models = if models.is_empty() { vec![pack_name.clone()] } else { models };

        let target_dir = user_dir
            .join("mods")
            .join("vehicles")
            .join(&mod_id)
            .join("x64")
            .join("dlcPacks")
            .join(&pack_name);
        fs::create_dir_all(&target_dir).map_err(|e| format!("cannot create target dir: {e}"))?;

        let target_dlc = target_dir.join("dlc.rpf");
        fs::copy(dlc_src, &target_dlc).map_err(|e| format!("cannot copy dlc.rpf: {e}"))?;

        let files_count = 1;
        let display_title = clean_mod_title(file_stem);

        let vehicle_mod = VehicleMod {
            id: mod_id.clone(),
            title: display_title,
            kind: VehicleModKind::Addon,
            pack_name: Some(pack_name),
            models: final_models,
            replaces: Vec::new(),
            enabled: true,
            files_count,
            date: now_secs(),
        };

        let mut db = load_db(user_dir);
        db.mods.retain(|m| m.id != mod_id);
        db.mods.push(vehicle_mod.clone());
        let _ = save_db(user_dir, &db);

        if let Some(gd) = game_data {
            sync_dlclist_overlay(gd, user_dir);
        }

        let _ = fs::remove_dir_all(&temp_unpack);
        log::info!("vehicle_mods: imported add-on car {} with models {:?}", vehicle_mod.title, vehicle_mod.models);
        return Ok(vehicle_mod);
    }

    // Check 2: Is this a Replace car? (Contains .yft / .ytd files)
    let yft_files: Vec<&PathBuf> = files
        .iter()
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map_or(false, |ext| ext.eq_ignore_ascii_case("yft"))
        })
        .collect();

    if !yft_files.is_empty() {
        let mut replaced_names = BTreeSet::new();
        for yft in &yft_files {
            if let Some(stem) = yft.file_stem().and_then(|s| s.to_str()) {
                let low = stem.to_ascii_lowercase();
                let car_name = if let Some(base) = low.strip_suffix("_hi") { base } else { &low };
                if !car_name.is_empty() {
                    replaced_names.insert(car_name.to_string());
                }
            }
        }

        let target_dir = user_dir
            .join("mods")
            .join("vehicles")
            .join(&mod_id)
            .join("x64")
            .join("levels")
            .join("gta5")
            .join("vehicles.rpf");
        fs::create_dir_all(&target_dir).map_err(|e| format!("cannot create target dir: {e}"))?;

        let mut copied_count = 0;
        for file in &files {
            let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
            if matches!(ext.as_str(), "yft" | "ytd" | "meta") {
                let name = file.file_name().unwrap();
                let dst = target_dir.join(name);
                if fs::copy(file, &dst).is_ok() {
                    copied_count += 1;
                }
            }
        }

        let display_title = clean_mod_title(file_stem);
        let replaces_vec: Vec<String> = replaced_names.into_iter().collect();

        let vehicle_mod = VehicleMod {
            id: mod_id.clone(),
            title: display_title,
            kind: VehicleModKind::Replace,
            pack_name: None,
            models: replaces_vec.clone(),
            replaces: replaces_vec,
            enabled: true,
            files_count: copied_count,
            date: now_secs(),
        };

        let mut db = load_db(user_dir);
        db.mods.retain(|m| m.id != mod_id);
        db.mods.push(vehicle_mod.clone());
        let _ = save_db(user_dir, &db);

        let _ = fs::remove_dir_all(&temp_unpack);
        log::info!("vehicle_mods: imported replace car {} replacing {:?}", vehicle_mod.title, vehicle_mod.replaces);
        return Ok(vehicle_mod);
    }

    // Check 3: Is it an OpenIV package?
    let has_assembly = files.iter().any(|p| p.file_name().map_or(false, |n| n.eq_ignore_ascii_case("assembly.xml")));
    if has_assembly {
        let dest = user_dir.join("mods").join(format!("{mod_id}.oiv"));
        let _ = copy_dir_recursive(&temp_unpack, &dest);
        let display_title = clean_mod_title(file_stem);
        let vehicle_mod = VehicleMod {
            id: mod_id.clone(),
            title: display_title,
            kind: VehicleModKind::Package,
            pack_name: None,
            models: Vec::new(),
            replaces: Vec::new(),
            enabled: true,
            files_count: files.len(),
            date: now_secs(),
        };
        let mut db = load_db(user_dir);
        db.mods.retain(|m| m.id != mod_id);
        db.mods.push(vehicle_mod.clone());
        let _ = save_db(user_dir, &db);

        let _ = fs::remove_dir_all(&temp_unpack);
        return Ok(vehicle_mod);
    }

    let _ = fs::remove_dir_all(&temp_unpack);
    Err("No compatible vehicle assets found in archive (requires dlc.rpf, .yft/.ytd models, or assembly.xml)".into())
}

fn clean_mod_title(stem: &str) -> String {
    // Strip hash prefixes e.g. "3393a1-mercedes benz vosses" -> "Mercedes Benz Vosses"
    let clean = if let Some(dash) = stem.find('-') {
        let prefix = &stem[..dash];
        if prefix.len() <= 10 && prefix.chars().all(|c| c.is_ascii_hexdigit()) {
            &stem[dash + 1..]
        } else {
            stem
        }
    } else {
        stem
    };
    clean.replace('_', " ").trim().to_string()
}

pub fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_id() {
        assert_eq!(sanitize_id("2015 Ferrari LaFerrari HQ Remake"), "2015_ferrari_laferrari_hq_remake");
        assert_eq!(sanitize_id("3393a1-mercedes benz vosses"), "3393a1-mercedes_benz_vosses");
        assert_eq!(sanitize_id(""), "mod");
    }

    #[test]
    fn test_clean_mod_title() {
        assert_eq!(clean_mod_title("3393a1-mercedes benz vosses"), "mercedes benz vosses");
        assert_eq!(clean_mod_title("f02f23-Lamborghini_Renventon_by_SCRAT"), "Lamborghini Renventon by SCRAT");
        assert_eq!(clean_mod_title("2015 Ferrari LaFerrari"), "2015 Ferrari LaFerrari");
    }

    #[test]
    fn test_dlclist_overlay_sync() {
        let tmp = std::env::temp_dir().join(format!("test_dlclist_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let game_common = tmp.join("game").join("common").join("data");
        fs::create_dir_all(&game_common).unwrap();
        let base_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<SMandatoryPacksData>
	<Paths>
  		<Item>platform:\dlcPacks\mpBeach\</Item>
	</Paths>
</SMandatoryPacksData>"#;
        fs::write(game_common.join("dlcList.xml"), base_xml).unwrap();

        let user_dir = tmp.join("user");
        let mut db = VehicleDatabase::default();
        db.mods.push(VehicleMod {
            id: "laferrari".into(),
            title: "Ferrari LaFerrari".into(),
            kind: VehicleModKind::Addon,
            pack_name: Some("vans123".into()),
            models: vec!["laferrari".into()],
            replaces: vec![],
            enabled: true,
            files_count: 1,
            date: 0,
        });
        save_db(&user_dir, &db).unwrap();

        let overlay = sync_dlclist_overlay(&tmp.join("game"), &user_dir).unwrap();
        assert!(overlay.exists());
        let content = fs::read_to_string(&overlay).unwrap();
        assert!(content.contains(r#"<Item>platform:\dlcPacks\vans123\</Item>"#));
        assert!(content.contains(r#"<Item>platform:\dlcPacks\mpBeach\</Item>"#));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_import_sample_archives() {
        let sample_dir = Path::new(r"C:\Users\Admin\Downloads\mod");
        if !sample_dir.exists() {
            return;
        }
        let tmp = std::env::temp_dir().join(format!("test_import_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);

        // 1. Add-on: Ferrari LaFerrari
        let laferrari_path = sample_dir.join("car add on").join("2015 Ferrari LaFerrari HQ Remake by Vans123 2.1.rar");
        if laferrari_path.exists() {
            let res = import_archive(&laferrari_path, &tmp, None).unwrap();
            assert_eq!(res.kind, VehicleModKind::Addon);
            assert_eq!(res.pack_name.as_deref(), Some("vans123"));
            assert!(res.models.iter().any(|m| m == "laferrari"));
            let dlc_file = tmp.join("mods").join("vehicles").join(&res.id).join("x64").join("dlcPacks").join("vans123").join("dlc.rpf");
            assert!(dlc_file.is_file(), "dlc.rpf must exist at {}", dlc_file.display());
        }

        // 2. Replace: Mercedes Benz Vosses (felon)
        let felon_path = sample_dir.join("car replace").join("3393a1-mercedes benz vosses.rar");
        if felon_path.exists() {
            let res = import_archive(&felon_path, &tmp, None).unwrap();
            assert_eq!(res.kind, VehicleModKind::Replace);
            assert!(res.replaces.iter().any(|m| m == "felon"));
            let yft_file = tmp.join("mods").join("vehicles").join(&res.id).join("x64").join("levels").join("gta5").join("vehicles.rpf").join("felon.yft");
            assert!(yft_file.is_file(), "felon.yft must exist at {}", yft_file.display());
        }

        // 3. Replace: Lamborghini Reventon (vacca)
        let vacca_path = sample_dir.join("car replace").join("f02f23-Lamborghini_Renventon_by_SCRAT.zip");
        if vacca_path.exists() {
            let res = import_archive(&vacca_path, &tmp, None).unwrap();
            assert_eq!(res.kind, VehicleModKind::Replace);
            assert!(res.replaces.iter().any(|m| m == "vacca"));
            let yft_file = tmp.join("mods").join("vehicles").join(&res.id).join("x64").join("levels").join("gta5").join("vehicles.rpf").join("vacca.yft");
            assert!(yft_file.is_file(), "vacca.yft must exist at {}", yft_file.display());
        }

        let _ = fs::remove_dir_all(&tmp);
    }
}
