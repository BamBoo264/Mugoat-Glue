--!strict
-- Kinds of ped models, by their names' prefixes (the game's naming: a_m_y_hipster_01 is an
-- ambient young man). In the order the trainer lists them; the last takes the rest.

export type Kind = { name: string, prefixes: { string } }

local kinds: { Kind } = {
	{ name = "Nhân vật cốt truyện", prefixes = { "player_", "ig_", "cs_", "csb_" } },
	{ name = "Nhân vật Online", prefixes = { "mp_" } },
	{ name = "Người dân", prefixes = { "a_f_", "a_m_" } },
	{ name = "Băng đảng", prefixes = { "g_" } },
	{ name = "Cảnh sát & dịch vụ", prefixes = { "s_" } },
	{ name = "Nhân vật đặc biệt", prefixes = { "u_" } },
	{ name = "Động vật", prefixes = { "a_c_" } },
	{ name = "Khác", prefixes = {} },
}

-- The kind of model `name`.
local function kindOf(name: string): string
	local lower = string.lower(name)
	-- Animals first: their prefix starts as people's do.
	if string.sub(lower, 1, 4) == "a_c_" then
		return "Động vật"
	end
	for _, k in kinds do
		for _, p in k.prefixes do
			if string.sub(lower, 1, #p) == p then
				return k.name
			end
		end
	end
	return "Khác"
end

return { kinds = kinds, kindOf = kindOf }
