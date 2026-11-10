--!strict
-- The trainer's own settings: a key for any feature, notifications, units, whether it keeps
-- what was on. All of it is the mod's settings.json, which can be edited by hand too.

local bind = require("../core/bind")
local game = require("../core/game")
local keys = require("../core/keys")
local prefs = require("../core/prefs")

local M = {}

-- Không có phím tắt.
local NONE = "Không"

-- A feature and a key for it, and the keys given so far. Keys reach the trainer while no page of
-- the app is open.
local function bindings(menu: Menu)
	local sub = menu:submenu("Phím tắt", "Gán phím tắt cho bất kỳ tính năng hay thao tác nào")
	local filled = false
	sub:onOpen(bind.safe("Phím tắt", function()
		if filled then
			return
		end
		filled = true
		local actions = keys.actions()
		local labels = {}
		for _, a in actions do
			table.insert(labels, a.label)
		end
		local names = { NONE }
		for _, k in input.keys() do
			table.insert(names, k)
		end
		-- No key is kept as "" (a missing one would be the default again).
		local function keyOf(i: number): string
			local k = prefs.data.keys[actions[i].name] or ""
			return if table.find(names, k) then k else NONE
		end
		local given: Label
		local function list()
			local lines = {}
			for _, a in actions do
				local k = prefs.data.keys[a.name]
				if k and k ~= "" then
					table.insert(lines, `{k}: {a.label}`)
				end
			end
			given:set(if #lines > 0 then table.concat(lines, " · ") else "Chưa gán phím tắt nào.")
		end
		local feature = sub:choice("Tính năng", labels, 1, "Chọn mục muốn gán phím")
		local key = sub:choice("Phím", names, keyOf(1), "Phím kích hoạt; chọn Không để gỡ", function(_, k)
			prefs.data.keys[actions[feature.value].name] = if k == NONE then "" else k
			prefs.save()
			list()
		end)
		feature.onChange = function(i)
			key.value = table.find(names, keyOf(i)) or 1
		end
		sub:separator("Phím đã gán")
		given = sub:label("")
		list()
	end))
end

function M.build(root: Menu)
	local menu = root:submenu("Cài đặt", "Phím tắt, thông báo, đơn vị đo")
	bindings(menu)
	menu:toggle("Thông báo", prefs.data.notify, "Hiển thị thông báo thao tác vừa thực hiện", function(on)
		prefs.data.notify = on
		prefs.save()
	end)
	menu:toggle("Ghi nhớ trạng thái", prefs.data.remember, "Tự khôi phục các nút bật và giá trị ở lần mở sau", function(on)
		prefs.data.remember = on
		prefs.save()
	end)
	menu:toggle("Bay khi dịch chuyển xa", prefs.data.flyTeleports, "Dịch chuyển xa sẽ bay trên bầu trời giống chuyển nhân vật", function(on)
		prefs.data.flyTeleports = on
		prefs.save()
	end)
	menu:choice("Đơn vị tốc độ", { "km/h", "mph" }, prefs.data.speedUnit, function(_, unit)
		prefs.data.speedUnit = unit
		prefs.save()
	end)
	menu:button("Khôi phục mặc định", { note = "Đặt lại phím tắt, tùy chọn, mục yêu thích từ lần chạy sau", action = "Đặt lại" }, function()
		prefs.reset()
		game.notify("Đã đặt lại cài đặt: hãy khởi động lại trainer để áp dụng toàn bộ")
	end)
	local about = json.decode(mod.read("mod.json") or "{}")
	menu:label(`Glue Trainer {about.version or ""} đi kèm Mugoat Glue. Mã nguồn nằm trong thư mục mods/glue-trainer: mở bằng Studio để xem hoặc chỉnh sửa theo ý bạn.`)
end

return M
