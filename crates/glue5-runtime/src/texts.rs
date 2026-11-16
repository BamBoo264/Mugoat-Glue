//! Texts the game data lacks: its scripts are newer and name labels of
//! later DLC packs, which show as "<LABEL> missing". The ones GTA Online's
//! jobs and activities show on the way (hosting, coronas, the job lists),
//! the pause menu's sessions, and the graphics settings the menu's data
//! lacks (`overlay.rs`), are given here, in English.

use std::collections::HashMap;
use std::sync::Mutex;

use glue5_game::{Env, orig};

use crate::memory::Mem;

/// The pause menu's friends list with no friend in it (friends/game.rs):
/// its warning's text and title.
pub(crate) const NO_FRIENDS: &str = "GLUE5_NO_FRIENDS";
pub(crate) const NO_FRIENDS_TITLE: &str = "NOFRIENDS_TITLE";

/// Labels and their texts (Vietnamese fallback with full diacritics).
const TEXTS: &[(&str, &str)] = &[
    (NO_FRIENDS, "Bạn chưa có bạn bè nào. Hãy thêm bạn ở trang Bạn bè: chia sẻ mã kết bạn hoặc hỏi người chơi bạn từng chơi cùng."),
    (NO_FRIENDS_TITLE, "Chưa có bạn bè"),
    ("FACES_WARNH2", "Cảnh báo"),
    ("QM_NO_1", "Không tìm thấy phiên nào cho nhiệm vụ này để tham gia. Bạn có muốn tự tạo phòng không?"),
    ("COR_YOUR_CHAR", "Nhân vật của bạn"),
    ("UI_FLOW_OP_NEWSESH", "Tìm phiên mới"),
    ("UI_FLOW_OP_NEWSESH_M", "Tìm phiên mới"),
    ("UI_FLOW_OP_NEWSESH_D", "Rời phiên hiện tại và chơi GTA Online trong một phiên mới."),
    ("UI_FLOW_SO_PBLCSESH", "Phiên công khai"),
    ("UI_FLOW_SO_PBLCSESH_M", "Phiên công khai"),
    ("UI_FLOW_SO_PBLCSESH_D", "Vào thẳng GTA Online công khai."),
    ("UI_FLOW_SO_INVT", "Phiên chỉ theo lời mời"),
    ("UI_FLOW_SO_INVT_M", "Phiên chỉ theo lời mời"),
    ("UI_FLOW_SO_INVT_D", "Người chơi khác chỉ có thể tham gia khi bạn gửi lời mời vào phiên này."),
    ("UI_FLOW_SO_CRWSESH", "Phiên Băng đảng"),
    ("UI_FLOW_SO_CRWSESH_M", "Phiên Băng đảng"),
    (
        "UI_FLOW_SO_CRWSESH_D",
        "Vào GTA Online cùng Băng đảng của bạn và các Băng đảng khác. Chọn số Băng đảng tối đa trong phiên.",
    ),
    ("UI_FLOW_SO_CLSDCRW", "Phiên nội bộ Băng đảng"),
    ("UI_FLOW_SO_CLSDCRW_M", "Phiên nội bộ Băng đảng"),
    ("UI_FLOW_SO_CLSDCRW_D", "Vào GTA Online chỉ với các thành viên Băng đảng của bạn."),
    ("UI_FLOW_SO_CLSDFRND", "Phiên bạn bè riêng tư"),
    ("UI_FLOW_SO_CLSDFRND_M", "Phiên bạn bè riêng tư"),
    ("UI_FLOW_SO_CLSDFRND_D", "Vào GTA Online chỉ với bạn bè của bạn."),
    ("UI_FLOW_SO_SOLO", "Phiên chơi đơn"),
    ("UI_FLOW_SO_SOLO_M", "Phiên chơi đơn"),
    (
        "UI_FLOW_SO_SOLO_D",
        "Tự mình vào GTA Online. Không ai khác có thể tham gia phiên này và một số hoạt động sẽ bị giới hạn.",
    ),
    ("PM_EXIT_GAME", "Thoát trò chơi"),
    ("PM_ARENA", "Đấu trường Arena War"),
    ("PM_RC_STUNT", "Đua nhào lộn"),
    ("RC_TARGASSAULT", "Đua bắn mục tiêu"),
    ("FMMC_RSTAR_KOT", "Vua ngọn đồi"),
    ("PM_TNEWVS", "Chế độ đối kháng"),
    ("FMMC_SR", "Đua nhào lộn"),
    ("FMMC_OWR", "Đua xe bánh trần"),
    ("MATC_DPADRIGHT", "Nhấn ~INPUT_CONTEXT~ để bắt đầu."),
    ("PM_JOIN_AREN", "Tham gia Arena War"),
    // Settings › Graphics and Advanced Graphics, as the PC game names them.
    ("GFX_POP_VARIETY", "Độ đa dạng dân cư"),
    ("GFX_REFLECTION_MSAA", "Khử răng cưa phản chiếu"),
    ("GFX_PARTICLEQ", "Chất lượng hiệu ứng hạt"),
    ("GFX_GRASSQ", "Chất lượng thảm cỏ"),
    ("GFX_MB_STRENGTH", "Cường độ nhòe chuyển động"),
    ("GFX_DOF", "Hiệu ứng độ sâu trường ảnh"),
    ("PM_PANE_ADVGFX", "Đồ họa nâng cao"),
    ("GFX_LONG_SHADOWS", "Bóng đổ dài"),
    ("GFX_ULTRA_SHADOWS", "Bóng đổ độ phân giải cao"),
    ("GFX_HD_FLIGHT", "Nạp chi tiết cao khi bay"),
    ("GFX_MAX_LOD", "Mở rộng khoảng cách hiển thị"),
    ("GFX_SHADOWS_DIST", "Mở rộng khoảng cách bóng đổ"),
];

/// The engine's label hash (`rage::atStringHash`): Jenkins' one-at-a-time,
/// case-insensitive.
pub(crate) fn label_hash(label: &str) -> u32 {
    let mut h: u32 = 0;
    for b in label.bytes() {
        h = h.wrapping_add(u32::from(b.to_ascii_lowercase()));
        h = h.wrapping_add(h << 10);
        h ^= h >> 6;
    }
    h = h.wrapping_add(h << 3);
    h ^= h >> 11;
    h.wrapping_add(h << 15)
}

/// The texts in the engine's heap, by hash (made when first asked for).
static PLACED: Mutex<Option<HashMap<u32, u64>>> = Mutex::new(None);

/// Hook for `CTextFile::GetInternal(unsigned int)`: the game data's text,
/// else one of these.
pub(crate) unsafe extern "C" fn text_get_internal(env: *mut Env, hash: u32) -> u64 {
    let text = unsafe { orig::text_get_internal(env, hash) };
    if text != 0 {
        return text;
    }
    // Labels' hashes once: misses come every frame (labels the data lacks).
    static HASHES: std::sync::OnceLock<Vec<u32>> = std::sync::OnceLock::new();
    let hashes = HASHES.get_or_init(|| TEXTS.iter().map(|(label, _)| label_hash(label)).collect());
    let Some((_, ours)) = TEXTS.iter().zip(hashes).find(|(_, h)| **h == hash).map(|(t, _)| t) else { return 0 };
    let mut placed = PLACED.lock().unwrap();
    let placed = placed.get_or_insert_default();
    if let Some(&at) = placed.get(&hash) {
        return at;
    }
    let at = unsafe { orig::builtin_malloc(env, ours.len() as u64 + 1) };
    if at != 0 {
        unsafe { Mem::of(env) }.write_str(at, ours, ours.len() + 1);
        placed.insert(hash, at);
    }
    at
}

pub(crate) fn hooks(hooks: &mut glue5_game::Hooks) {
    hooks.text_get_internal = Some(text_get_internal);
}

#[cfg(test)]
mod tests {
    #[test]
    fn label_hashes() {
        // Known joaat values.
        assert_eq!(super::label_hash("a"), 0xca2e9442);
        assert_eq!(super::label_hash("ADDER"), super::label_hash("adder"));
        assert_eq!(super::label_hash("adder"), 0xb779a091);
    }
}
