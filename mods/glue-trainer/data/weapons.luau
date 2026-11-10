--!strict
-- Weapons' names the game keeps only as hashes: its groups (GET_WEAPONTYPE_GROUP gives their
-- names' hashes), as the trainer names and orders them (a group not here shows by its hash,
-- last); attachments; tints.

local groups = {
	{ "GROUP_MELEE", "Cận chiến" },
	{ "GROUP_PISTOL", "Súng lục" },
	{ "GROUP_SMG", "Tiểu liên" },
	{ "GROUP_RIFLE", "Súng trường tấn công" },
	{ "GROUP_MG", "Súng máy" },
	{ "GROUP_SHOTGUN", "Shotgun" },
	{ "GROUP_SNIPER", "Súng bắn tỉa" },
	{ "GROUP_HEAVY", "Hỏa lực hạng nặng" },
	{ "GROUP_THROWN", "Vũ khí ném" },
	{ "GROUP_STUNGUN", "Súng điện" },
	{ "GROUP_PETROLCAN", "Can xăng" },
	{ "GROUP_FIREEXTINGUISHER", "Bình chữa cháy" },
	{ "GROUP_PARACHUTE", "Dù lượn" },
	{ "GROUP_DIGISCANNER", "Thiết bị phụ trợ" },
	{ "GROUP_NIGHTVISION", "Thiết bị phụ trợ" },
}

local byHash: { [number]: string } = {}
local rank: { [string]: number } = {}
for i, g in groups do
	byHash[joaat(g[1])] = g[2]
	rank[g[2]] = rank[g[2]] or i
end

-- Attachments many weapons share (COMPONENT_AT_*), as the trainer names them. A weapon takes
-- those the game says it does (DOES_WEAPON_TAKE_WEAPON_COMPONENT).
local attachments = {
	{ "COMPONENT_AT_PI_FLSH", "Đèn pin" },
	{ "COMPONENT_AT_AR_FLSH", "Đèn pin" },
	{ "COMPONENT_AT_PI_SUPP", "Ống giảm thanh" },
	{ "COMPONENT_AT_PI_SUPP_02", "Ống giảm thanh" },
	{ "COMPONENT_AT_AR_SUPP", "Ống giảm thanh" },
	{ "COMPONENT_AT_AR_SUPP_02", "Ống giảm thanh" },
	{ "COMPONENT_AT_SR_SUPP", "Ống giảm thanh" },
	{ "COMPONENT_AT_SCOPE_MACRO", "Ống ngắm" },
	{ "COMPONENT_AT_SCOPE_MACRO_02", "Ống ngắm" },
	{ "COMPONENT_AT_SCOPE_SMALL", "Ống ngắm" },
	{ "COMPONENT_AT_SCOPE_SMALL_02", "Ống ngắm" },
	{ "COMPONENT_AT_SCOPE_MEDIUM", "Ống ngắm" },
	{ "COMPONENT_AT_SCOPE_LARGE", "Ống ngắm lớn" },
	{ "COMPONENT_AT_SCOPE_MAX", "Ống ngắm cao cấp" },
	{ "COMPONENT_AT_AR_AFGRIP", "Tay cầm trước" },
}

-- A weapon's own components are named after it (COMPONENT_PISTOL_CLIP_02): these endings.
local own = {
	{ "CLIP_02", "Băng đạn mở rộng" },
	{ "CLIP_03", "Băng đạn trống" },
	{ "VARMOD_LUXE", "Hoa văn sang trọng" },
	{ "VARMOD_LOWRIDER", "Hoa văn Lowrider" },
}

return {
	attachments = attachments,
	own = own,
	-- SET_PED_WEAPON_TINT_INDEX's tints (GET_WEAPON_TINT_COUNT says how many a weapon has).
	tints = { [0] = "Mặc định", "Xanh lá", "Vàng kim", "Hồng", "Quân đội", "LSPD", "Cam", "Bạch kim" },
	groupName = function(hash: number): string
		return byHash[hash] or string.format("Nhóm %08X", hash)
	end,
	order = function(name: string): number
		return rank[name] or #groups + 1
	end,
}
