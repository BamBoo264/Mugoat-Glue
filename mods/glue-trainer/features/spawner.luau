--!strict
-- Triệu hồi phương tiện trong GLUE 5: Xe từ dữ liệu gốc, xe mod bổ sung (Add-on),
-- xe mod thay thế (Replace), gọi xe theo tên mô hình, và gara cá nhân.

local bind = require("../core/bind")
local game = require("../core/game")
local catalog = require("../core/catalog")
local lists = require("../core/lists")
local customs = require("./customs")

local M = {}

type Options = { warpIn: Toggle, replace: Toggle, maxed: Toggle, god: Toggle, plate: Text }
type Saved = { name: string, look: customs.Look }

local options: Options
local last: Vehicle? = nil

-- Áp dụng âm thanh động cơ tương thích cho xe để đảm bảo không bao giờ bị mất tiếng
local function applyCompatibleVehicleAudio(car: Vehicle, modelHash: any)
	local vClass = vehicle.getVehicleClass(car)
	local sound = "CARBONIZZARE"
	if vClass == 7 then -- Super (Ferrari, Lambo, Bugatti...)
		sound = "CHEETAH"
	elseif vClass == 6 or vClass == 5 then -- Sports / Sports Classics
		sound = "CARBONIZZARE"
	elseif vClass == 4 then -- Muscle
		sound = "DOMINATOR"
	elseif vClass == 3 or vClass == 1 then -- Coupes / Sedans
		sound = "FELON"
	elseif vClass == 2 or vClass == 9 then -- SUVs / Off-road
		sound = "BALLER"
	elseif vClass == 8 then -- Motorcycles
		sound = "BATI"
	elseif vClass == 15 then -- Helicopters
		sound = "MAVERICK"
	elseif vClass == 16 then -- Planes
		sound = "LUXOR"
	end
	pcall(function()
		audio.forceUseAudioGameObject(car, sound)
	end)
end

function M.spawnAt(hash: any, pos: vector, heading: number?): (Vehicle?, string?)
	if not streaming.load(hash) then
		return nil, "Không thể tải mô hình phương tiện này"
	end
	local online = network.networkIsSessionStarted()
	local car = vehicle.createVehicle(hash, pos, heading or 0, online, false, false)
	streaming.setModelAsNoLongerNeeded(hash)
	if not car then
		return nil, if online then "Trò chơi không thể tạo xe (trong nhiệm vụ)" else "Trò chơi không thể tạo xe"
	end
	if online then
		network.setNetworkIdExistsOnAllMachines(network.vehToNet(car), true)
	end
	if not (vehicle.isThisModelAHeli(hash) or vehicle.isThisModelAPlane(hash)) then
		vehicle.setVehicleOnGroundProperly(car, 5)
	end
	vehicle.setVehicleEngineOn(car, true, true, false)
	applyCompatibleVehicleAudio(car, hash)
	return car, nil
end

function M.spawn(hash: any, name: string): (Vehicle?, string?)
	local me = game.me()
	if options.replace.value and last and entity.doesEntityExist(last) then
		local old = last :: Vehicle
		if ped.isPedInVehicle(me, old, false) then
			entity.setEntityCoordsNoOffset(me, entity.getEntityCoords(old, true), false, false, true)
		end
		entity.setEntityAsMissionEntity(old, true, true)
		vehicle.deleteVehicle(old)
	end
	local from = game.mover()
	local inside = options.warpIn.value
	local pos = entity.getEntityCoords(from, true)
	if not inside or game.vehicle() then
		pos += entity.getEntityForwardVector(from) * 8
	end
	local flying = vehicle.isThisModelAHeli(hash) or vehicle.isThisModelAPlane(hash)
	if flying and inside then
		pos += vector.create(0, 0, 100)
	end
	local car, why = M.spawnAt(hash, pos, entity.getEntityHeading(from))
	if not car then
		return nil, `{name}: {why}`
	end
	last = car
	if inside then
		ped.setPedIntoVehicle(me, car, VehicleSeat.DRIVER)
		if flying then
			vehicle.setHeliBladesFullSpeed(car)
			vehicle.setVehicleForwardSpeed(car, 40)
		end
	end
	if options.maxed.value then
		customs.maxUpgrade(car)
	end
	if options.god.value then
		entity.setEntityInvincible(car, true)
		vehicle.setVehicleCanBeVisiblyDamaged(car, false)
	end
	if options.plate.value ~= "" then
		vehicle.setVehicleNumberPlateText(car, options.plate.value)
	end
	return car, nil
end

local function garage(root: Menu)
	local menu = root:submenu("Gara của bạn", "Xe đã lưu với các phụ tùng và màu sơn")
	local function saved(): { Saved }
		return storage.get("garage") or {}
	end
	local function build()
		menu:clear()
		bind.action(menu, "Lưu xe bạn đang ngồi", { note = "Phụ tùng, màu sơn, biển số", action = "Lưu" }, function(): string?
			local car = game.vehicle()
			if not car then
				return "Bạn đang không ngồi trong phương tiện nào"
			end
			local look = customs.look(car)
			local name = game.label(vehicle.getDisplayNameFromVehicleModel(look.model))
			local list = saved()
			table.insert(list, { name = if look.plate ~= "" then `{name} ({look.plate})` else name, look = look })
			storage.set("garage", list)
			build()
			return `Đã lưu xe: {name}`
		end)
		local list = saved()
		if #list == 0 then
			menu:label("Chưa có xe nào được lưu.")
			return
		end
		menu:separator("Lấy xe")
		for _, s in list do
			bind.action(menu, s.name, { action = "Lấy xe" }, function(): string?
				local car, why = M.spawn(s.look.model, s.name)
				if car then
					customs.apply(car, s.look)
				end
				return why
			end)
		end
		local forget = menu:submenu("Xóa xe đã lưu")
		for i, s in list do
			bind.action(forget, s.name, { action = "Xóa" }, function(): string?
				local l = saved()
				table.remove(l, i)
				storage.set("garage", l)
				build()
				return `Đã xóa: {s.name}`
			end)
		end
	end
	build()
end

local function modVehicles(root: Menu)
	local menu = root:submenu("Xe Mod (Add-on & Thay thế)", "Xe bổ sung và xe thay thế đã cài đặt")
	local function build()
		menu:clear()
		local vmods = storage.get("vehicles") or {}
		local addons = {}
		local replaces = {}
		for _, m in vmods do
			if m.enabled then
				if m.kind == "addon" then
					table.insert(addons, m)
				elseif m.kind == "replace" then
					table.insert(replaces, m)
				end
			end
		end

		local customInput = menu:text("Tên mô hình", "", { placeholder = "Ví dụ: laferrari, felon, vacca", max = 32 })
		bind.action(menu, "Gọi xe theo tên mô hình", { note = "Triệu hồi theo chuỗi model", action = "Gọi xe" }, function(): string?
			local name = customInput.value:match("^%s*(.-)%s*$")
			if name == "" then
				return "Vui lòng nhập tên mô hình xe"
			end
			local car, why = M.spawn(name, name)
			return if car then `Đã gọi xe: {name}` else why
		end)

		if #addons > 0 then
			local addonMenu = menu:submenu("Xe bổ sung (Add-on)", `{#addons} bản mod xe`)
			for _, m in addons do
				local title = m.title or m.id
				if m.models and #m.models > 1 then
					local packMenu = addonMenu:submenu(title, `{#m.models} mẫu xe`)
					for _, modelName in m.models do
						bind.action(packMenu, modelName, { action = "Gọi xe" }, function(): string?
							local car, why = M.spawn(modelName, modelName)
							return if car then `Đã gọi xe: {modelName}` else why
						end)
					end
				else
					local spawnName = (m.models and m.models[1]) or m.id
					bind.action(addonMenu, `{title} ({spawnName})`, { note = spawnName, action = "Gọi xe" }, function(): string?
						local car, why = M.spawn(spawnName, title)
						return if car then `Đã gọi xe: {title}` else why
					end)
				end
			end
		end

		if #replaces > 0 then
			local repMenu = menu:submenu("Xe thay thế (Replace)", `{#replaces} bản mod xe`)
			for _, m in replaces do
				local title = m.title or m.id
				local target = (m.replaces and m.replaces[1]) or (m.models and m.models[1]) or m.id
				bind.action(repMenu, `{title} [thay {target}]`, { note = target, action = "Gọi xe" }, function(): string?
					local car, why = M.spawn(target, title)
					return if car then `Đã gọi xe: {title}` else why
				end)
			end
		end

		if #addons == 0 and #replaces == 0 then
			menu:label("Chưa có mod xe nào. Hãy nhấn nút 'Mod xe' ở màn hình chính để nhập file zip/rar.")
		end
	end

	menu:onOpen(bind.safe(menu.title, function()
		build()
	end))
	build()
end

function M.picker(menu: Menu, verb: string, pick: (entry: catalog.VehicleEntry) -> string?)
	local function fill(target: Menu, entries: { catalog.VehicleEntry }): boolean
		for _, entry in entries do
			bind.action(target, entry.name, { note = entry.model, action = verb }, function(): string?
				return pick(entry)
			end)
		end
		return #entries > 0
	end
	lists.lazy(menu, "Tìm tất cả xe", "Toàn bộ xe theo tên", function(target)
		return fill(target, catalog.vehicles())
	end)
	local listed = false
	menu:onOpen(bind.safe(menu.title, function()
		if listed then
			return
		end
		local classes = catalog.vehicleClasses()
		for _, class in classes do
			lists.lazy(menu, class.name, `{#class.vehicles} xe`, function(target)
				return fill(target, class.vehicles)
			end)
		end
		listed = #classes > 0
	end))
end

function M.build(root: Menu)
	local menu = root:submenu("Triệu hồi xe", "Toàn bộ xe gốc, xe mod bổ sung / thay thế, gọi theo tên và gara")
	local optionsMenu = menu:submenu("Tùy chọn triệu hồi", "Cách thức xe xuất hiện")
	options = {
		warpIn = (bind.feature(optionsMenu, "spawn.warpIn", "Tự động vào ghế lái", "Ngay sau khi xe xuất hiện", {})),
		replace = (bind.feature(optionsMenu, "spawn.replace", "Xóa xe cũ trước đó", "Tránh làm chật chỗ xung quanh", {})),
		maxed = (bind.feature(optionsMenu, "spawn.maxed", "Nâng cấp tối đa", "Độ máy, turbo, giáp xe", {})),
		god = (bind.feature(optionsMenu, "spawn.god", "Xe bất tử", "Không nhận sát thương hay biến dạng", {})),
		plate = optionsMenu:text("Biển số xe", "", { placeholder = "Theo mặc định trò chơi", max = 8 }),
	}
	modVehicles(menu)
	garage(menu)
	menu:separator("Danh mục xe gốc")
	M.picker(menu, "Gọi xe", function(entry: catalog.VehicleEntry): string?
		local car, why = M.spawn(entry.hash, entry.name)
		return if car then `Đã gọi xe: {entry.name}` else why
	end)
end

return M
