--!strict
-- How you look and move: any ped model of the game, every piece of clothing it has, saved
-- outfits, animations, scenarios, walks and moods.

local bind = require("../core/bind")
local game = require("../core/game")
local catalog = require("../core/catalog")
local lists = require("../core/lists")
local data = require("../data/looks")

local M = {}

type Outfit = { name: string, model: number, components: { { number } }, props: { { number } } }

local function model(root: Menu)
	local start = entity.getEntityModel(game.me())
	local function become(hash: number): string?
		if not streaming.load(hash) then
			return "Không tải được mô hình nhân vật này"
		end
		player.setPlayerModel(game.player(), hash)
		streaming.setModelAsNoLongerNeeded(hash)
		ped.setPedDefaultComponentVariation(game.me())
		return nil
	end
	local models = root:submenu("Nhân vật (Model)", "Mọi mô hình nhân vật trong game")
	bind.command(models, "looks.ownModel", "Quay về nhân vật gốc", { note = "Nhân vật ban đầu của bạn", action = "Đổi" }, function(): string?
		return become(start)
	end)
	local function fill(menu: Menu, names: { string }): boolean
		for _, name in names do
			bind.action(menu, name, { action = "Biến hình" }, function(): string?
				return become(joaat(name))
			end)
		end
		return #names > 0
	end
	lists.lazy(models, "Tìm trong tất cả", "Toàn bộ mô hình theo tên", function(menu)
		return fill(menu, catalog.peds())
	end)
	models:separator("Theo phân loại")
	-- The kinds come from the game's models: added under the heading once they are read.
	local listed = false
	models:onOpen(bind.safe("Nhân vật", function()
		if listed then
			return
		end
		local groups = catalog.pedGroups()
		for _, group in groups do
			lists.lazy(models, group.name, `{#group.models} mô hình`, function(menu)
				return fill(menu, group.models)
			end)
		end
		listed = #groups > 0
	end))
end

-- Every clothing slot and prop of the ped you are now, with as many choices as it has.
local function wardrobe(root: Menu)
	local menu = root:submenu("Tủ đồ", "Từng món trang phục nhân vật hiện tại hỗ trợ")
	local function build()
		menu:clear()
		local me = game.me()
		bind.action(menu, "Phối đồ ngẫu nhiên", { action = "Mặc" }, function(): string?
			ped.setPedRandomComponentVariation(game.me(), 0)
			build()
			return nil
		end)
		bind.action(menu, "Trang phục mặc định", { action = "Mặc" }, function(): string?
			ped.setPedDefaultComponentVariation(game.me())
			ped.clearAllPedProps(game.me())
			build()
			return nil
		end)
		menu:separator("Quần áo")
		for slot = 0, 11 do
			local count = ped.getNumberOfPedDrawableVariations(me, slot)
			if count > 1 then
				local name = data.components[slot]
				local drawable = ped.getPedDrawableVariation(me, slot)
				local textures = math.max(ped.getNumberOfPedTextureVariations(me, slot, drawable), 1)
				local texture: Number
				menu:number(name, drawable, { min = 0, max = count - 1, note = `{count} kiểu` }, bind.safe(name, function(d)
					ped.setPedComponentVariation(game.me(), slot, d, 0, 0)
					texture.max = math.max(ped.getNumberOfPedTextureVariations(game.me(), slot, d), 1) - 1
					texture.value = 0
				end))
				texture = menu:number(`{name}: màu sắc`, ped.getPedTextureVariation(me, slot), { min = 0, max = textures - 1 }, bind.safe(name, function(t)
					ped.setPedComponentVariation(game.me(), slot, ped.getPedDrawableVariation(game.me(), slot), t, 0)
				end))
			end
		end
		menu:separator("Phụ kiện đeo")
		for slot, name in data.props do
			local count = ped.getNumberOfPedPropDrawableVariations(me, slot)
			if count > 0 then
				-- -1: none.
				menu:number(name, ped.getPedPropIndex(me, slot), { min = -1, max = count - 1, note = "-1: không đeo" }, bind.safe(name, function(p)
					if p < 0 then
						ped.clearPedProp(game.me(), slot)
					else
						ped.setPedPropIndex(game.me(), slot, p, 0, true)
					end
				end))
			end
		end
	end
	menu:onOpen(bind.safe("Tủ đồ", build))
end

-- The models with a face to make: GTA Online's characters.
local FREEMODE = { [joaat("mp_m_freemode_01")] = true, [joaat("mp_f_freemode_01")] = true }

-- Head overlays (SET_PED_HEAD_OVERLAY's 0-10), and the colour kind of those that take one
-- (1 hair, 2 make-up).
type Overlay = { name: string, id: number, kind: number? }
local OVERLAYS: { Overlay } = {
	{ name = "Tì vết da", id = 0 },
	{ name = "Râu & ria", id = 1, kind = 1 },
	{ name = "Lông mày", id = 2, kind = 1 },
	{ name = "Nếp nhăn tuổi tác", id = 3 },
	{ name = "Trang điểm", id = 4 },
	{ name = "Phấn má", id = 5, kind = 2 },
	{ name = "Sắc tố da", id = 6 },
	{ name = "Cháy nắng", id = 7 },
	{ name = "Son môi", id = 8, kind = 2 },
	{ name = "Tàn nhang", id = 9 },
	{ name = "Lông ngực", id = 10, kind = 1 },
}

-- An Online character's face: parents, hair colour, eyes, and each overlay.
local function face(root: Menu)
	local menu = root:submenu("Khuôn mặt", "Nhân vật GTA Online: cha mẹ, màu tóc, mắt, râu, trang điểm")
	-- The parents' heads and the mixes, as last set here (the game does not tell them back).
	local blend = { mother = 21, father = 0, shape = 0.5, skin = 0.5 }
	local function apply()
		ped.setPedHeadBlendData(game.me(), blend.mother, blend.father, 0, blend.mother, blend.father, 0, blend.shape, blend.skin, 0, false)
	end
	local hair = { colour = 0, highlight = 0 }
	local function build()
		menu:clear()
		local me = game.me()
		if not FREEMODE[entity.getEntityModel(me)] then
			menu:label("Chỉ nhân vật GTA Online mới tùy chỉnh được khuôn mặt: hãy chọn trong mục Nhân vật (mp_m_freemode_01, mp_f_freemode_01).")
			return
		end
		menu:separator("Cha mẹ")
		local heads = math.max(ped.getPedHeadBlendNumHeads(0), 1)
		menu:number("Mẹ", blend.mother, { min = 0, max = heads - 1 }, bind.safe("Mẹ", function(v)
			blend.mother = v
			apply()
		end))
		menu:number("Cha", blend.father, { min = 0, max = heads - 1 }, bind.safe("Cha", function(v)
			blend.father = v
			apply()
		end))
		menu:slider("Nét giống", blend.shape, 0, 1, { step = 0.05, note = "0: giống mẹ, 1: giống cha" }, bind.safe("Nét giống", function(v)
			blend.shape = v
			apply()
		end))
		menu:slider("Màu da", blend.skin, 0, 1, { step = 0.05, note = "0: da mẹ, 1: da cha" }, bind.safe("Màu da", function(v)
			blend.skin = v
			apply()
		end))
		menu:separator("Tóc và mắt")
		local tints = math.max(ped.getNumPedHairTints(), 1)
		menu:number("Màu tóc", hair.colour, { min = 0, max = tints - 1 }, bind.safe("Màu tóc", function(v)
			hair.colour = v
			ped.setPedHairTint(game.me(), hair.colour, hair.highlight)
		end))
		menu:number("Highlight tóc", hair.highlight, { min = 0, max = tints - 1 }, bind.safe("Highlight tóc", function(v)
			hair.highlight = v
			ped.setPedHairTint(game.me(), hair.colour, hair.highlight)
		end))
		menu:number("Màu mắt", ped.getHeadBlendEyeColor(me), { min = 0, max = 31 }, bind.safe("Màu mắt", function(v)
			ped.setHeadBlendEyeColor(game.me(), v)
		end))
		menu:separator("Chi tiết khuôn mặt")
		for _, o in OVERLAYS do
			local name, id, kind = o.name, o.id, o.kind
			local count = ped.getPedHeadOverlayNum(id)
			if count > 0 then
				-- 255: none (shown as -1).
				local now = ped.getPedHeadOverlay(me, id)
				local opacity = 1
				local index = if now == 255 then -1 else now
				local function set()
					ped.setPedHeadOverlay(game.me(), id, if index < 0 then 255 else index, opacity)
				end
				menu:number(name, index, { min = -1, max = count - 1, note = "-1: không có" }, bind.safe(name, function(v)
					index = v
					set()
				end))
				menu:slider(`{name}: độ đậm`, opacity, 0, 1, { step = 0.1 }, bind.safe(name, function(v)
					opacity = v
					set()
				end))
				if kind then
					menu:number(`{name}: màu sắc`, 0, { min = 0, max = 63 }, bind.safe(name, function(v)
						ped.setPedHeadOverlayTint(game.me(), id, kind, v, v)
					end))
				end
			end
		end
	end
	menu:onOpen(bind.safe("Khuôn mặt", build))
end

-- What you wear now, to put on again.
local function current(name: string): Outfit
	local me = game.me()
	local o: Outfit = { name = name, model = entity.getEntityModel(me), components = {}, props = {} }
	for slot = 0, 11 do
		o.components[slot + 1] = { ped.getPedDrawableVariation(me, slot), ped.getPedTextureVariation(me, slot) }
	end
	for slot in data.props do
		table.insert(o.props, { slot, ped.getPedPropIndex(me, slot), ped.getPedPropTextureIndex(me, slot) })
	end
	return o
end

local function wear(o: Outfit): string?
	local me = game.me()
	if entity.getEntityModel(me) ~= o.model then
		return `{o.name} là trang phục của nhân vật khác`
	end
	for i, c in o.components do
		ped.setPedComponentVariation(me, i - 1, c[1], c[2], 0)
	end
	for _, p in o.props do
		if p[2] < 0 then
			ped.clearPedProp(me, p[1])
		else
			ped.setPedPropIndex(me, p[1], p[2], p[3], true)
		end
	end
	return nil
end

local function outfits(root: Menu)
	local menu = root:submenu("Trang phục đã lưu", "Lưu đồ đang mặc để mặc lại sau")
	local function saved(): { Outfit }
		return storage.get("outfits") or {}
	end
	local function build()
		menu:clear()
		menu:text("Lưu với tên", "", { placeholder = "Nhập tên rồi Enter", max = 40 }, bind.safe("Lưu với tên", function(name)
			if name == "" then
				return
			end
			local list = saved()
			table.insert(list, current(name))
			storage.set("outfits", list)
			build()
			game.notify(`Đã lưu: {name}`)
		end))
		local list = saved()
		if #list == 0 then
			menu:label("Chưa có trang phục nào được lưu.")
			return
		end
		menu:separator("Mặc đồ")
		for _, o in list do
			bind.action(menu, o.name, { action = "Mặc" }, function(): string?
				return wear(o)
			end)
		end
		local forget = menu:submenu("Xóa trang phục")
		for i, o in list do
			bind.action(forget, o.name, { action = "Xóa" }, function(): string?
				local l = saved()
				table.remove(l, i)
				storage.set("outfits", l)
				build()
				return `Đã xóa: {o.name}`
			end)
		end
	end
	build()
end

local function moves(root: Menu)
	local stop = function(): string?
		task.clearPedTasks(game.me())
		return nil
	end
	local anims = root:submenu("Hành động", { search = true })
	bind.command(anims, "looks.stop", "Dừng hành động", { note = "Quay lại trạng thái bình thường", action = "Dừng" }, stop)
	local loopAnim = anims:toggle("Lặp lại", true, "Lặp liên tục cho đến khi dừng")
	local upper = anims:toggle("Chỉ nửa thân trên", false, "Vừa đi bộ vừa thực hiện động tác")
	anims:separator()
	for _, a in data.animations do
		local label, dict, clip = a[1], a[2], a[3]
		bind.action(anims, label, { action = "Phát" }, function(): string?
			if not streaming.load(dict, "anim") then
				return `{label} không có trong game này`
			end
			-- Flags: 1 loop, 16 upper body, 32 the player keeps control.
			local flags = (if loopAnim.value then 1 else 0) + (if upper.value then 48 else 0)
			task.taskPlayAnim(game.me(), dict, clip, 8, -8, -1, flags, 0, false, 0, false)
			return nil
		end)
	end

	local scenarios = root:submenu("Tình huống", { note = "Hoạt động thường ngày của NPC", search = true })
	bind.action(scenarios, "Dừng", { action = "Dừng" }, stop)
	scenarios:separator()
	for _, s in data.scenarios do
		bind.action(scenarios, s[1], { action = "Làm" }, function(): string?
			task.taskStartScenarioInPlace(game.me(), s[2], 0, true)
			return nil
		end)
	end

	local walks = root:submenu("Dáng đi", { note = "Cách bạn bước đi", search = true })
	bind.action(walks, "Mặc định của bạn", { action = "Chọn" }, function(): string?
		ped.resetPedMovementClipset(game.me(), 1)
		return nil
	end)
	walks:separator()
	for _, w in data.walks do
		bind.action(walks, w[1], { action = "Chọn" }, function(): string?
			if not streaming.load(w[2], "clipSet") then
				return `{w[1]} không có trong game này`
			end
			ped.setPedMovementClipset(game.me(), w[2], 1)
			return nil
		end)
	end

	local moods = {}
	for _, m in data.moods do
		table.insert(moods, m[1])
	end
	root:choice("Biểu cảm", moods, 1, "Nét mặt của bạn", bind.safe("Biểu cảm", function(i: number)
		if i == 1 then
			ped.clearFacialIdleAnimOverride(game.me())
		else
			ped.setFacialIdleAnimOverride(game.me(), data.moods[i][2], "")
		end
	end))
end

function M.build(root: Menu)
	local menu = root:submenu("Ngoại hình", "Nhân vật, khuôn mặt, quần áo, trang phục, hành động, dáng đi")
	model(menu)
	face(menu)
	wardrobe(menu)
	outfits(menu)
	menu:separator("Diễn hoạt")
	moves(menu)
end

return M
