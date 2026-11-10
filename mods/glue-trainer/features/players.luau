--!strict
-- The other players of a GTA Online session, friends: how they are, go to them or into their
-- vehicle, follow them, watch them, mark them on the map, wear their outfit, name tags; and
-- gifts (a vehicle beside them, weapons, health and armour, a bodyguard, fireworks). Listed as
-- they are each time the menu opens.
--
-- Their ped and the vehicle they drive are their game's: nothing here takes them over. What
-- the trainer makes is networked (mods run as the session's script), so everyone sees it.

local bind = require("../core/bind")
local game = require("../core/game")
local catalog = require("../core/catalog")
local spawner = require("./spawner")

local M = {}

-- The game's player slots.
local SLOTS = 32

local watching: Player? = nil

local function stopWatching()
	if watching then
		network.networkSetInSpectatorMode(false, game.me())
		watching = nil
	end
end

mod.onStop(stopWatching)

local function about(who: Player): string
	local p = player.getPlayerPed(who)
	local parts = { `{entity.getEntityHealth(p)} máu`, `{ped.getPedArmour(p)} giáp` }
	local car = ped.getVehiclePedIsIn(p, false)
	if car then
		table.insert(parts, game.label(vehicle.getDisplayNameFromVehicleModel(entity.getEntityModel(car))))
	end
	table.insert(parts, `cách {math.floor(geo.distance(game.position(), entity.getEntityCoords(p, true)))} m`)
	return table.concat(parts, " · ")
end

-- Pickups the game has for health and armour (the base game's).
local PICKUPS = { { "Máu", "PICKUP_HEALTH_STANDARD" }, { "Áo giáp", "PICKUP_ARMOUR_STANDARD" } }

-- What can be given to a player: made next to them, networked, so they see it too.
local function gifts(menu: Menu, who: Player, name: string)
	local them = function(): Ped
		return player.getPlayerPed(who)
	end
	menu:separator("Tặng quà")
	local cars = menu:submenu("Tặng một chiếc xe", `Xuất hiện cạnh {name}, máy đã nổ sẵn`)
	spawner.picker(cars, "Tặng", function(entry: catalog.VehicleEntry): string?
		local p = them()
		local car, why = spawner.spawnAt(entry.hash, entity.getOffsetFromEntityInWorldCoords(p, vector.create(3, 4, 0)), entity.getEntityHeading(p))
		return if car then `Đã tặng {entry.name} cho {name}` else why
	end)
	bind.action(menu, "Tặng toàn bộ vũ khí", { note = "Kèm đầy đạn và dù lượn", action = "Tặng" }, function(): string?
		local p = them()
		for _, w in catalog.weapons() do
			weapon.giveWeaponToPed(p, w.hash, 9999, false, false)
		end
		weapon.giveWeaponToPed(p, joaat("GADGET_PARACHUTE"), 1, false, false)
		return `Đã trang bị vũ khí cho {name}`
	end)
	for _, pick in PICKUPS do
		bind.action(menu, pick[1], { note = "Thả vật phẩm ngay dưới chân họ", action = "Thả" }, function(): string?
			local at = entity.getOffsetFromEntityInWorldCoords(them(), vector.create(0, 1.5, 0))
			local made = object.createPickupRotate(joaat(pick[2]), at, vector.zero, 512, 0, 2, true, 0)
			return if made then nil else "Game không tạo được vật phẩm"
		end)
	end
	bind.action(menu, "Gửi một vệ sĩ", { note = "Có vũ trang, đi theo bảo vệ họ", action = "Gửi" }, function(): string?
		local model = joaat("s_m_y_blackops_01")
		if not streaming.load(model) then
			return "Không tải được mô hình vệ sĩ"
		end
		local p = them()
		local guard = ped.createPed(26, model, entity.getOffsetFromEntityInWorldCoords(p, vector.create(-2, -2, 0)), entity.getEntityHeading(p), true, false)
		streaming.setModelAsNoLongerNeeded(model)
		if not guard then
			return "Game không tạo được vệ sĩ"
		end
		weapon.giveWeaponToPed(guard, joaat("WEAPON_CARBINERIFLE"), 9999, false, true)
		ped.setPedRelationshipGroupHash(guard, joaat("PLAYER"))
		ped.setPedAccuracy(guard, 80)
		ped.setPedKeepTask(guard, true)
		task.taskFollowToOffsetOfEntity(guard, p, vector.create(-1.5, -1.5, 0), 2, -1, 1, true)
		return `Đã gửi vệ sĩ cho {name}`
	end)
	bind.action(menu, "Bắn pháo hoa", { note = "Nổ trên bầu trời phía trên họ, ai cũng thấy", action = "Bắn" }, function(): string?
		local asset = "scr_indep_fireworks"
		if not streaming.load(asset, "ptfx") then
			return "Không tải được hiệu ứng pháo hoa"
		end
		local p = them()
		for i = 1, 6 do
			local at = entity.getEntityCoords(p, true) + vector.create(math.random(-15, 15), math.random(-15, 15), 30 + math.random(0, 15))
			graphics.useParticleFxAsset(asset)
			graphics.startNetworkedParticleFxNonLoopedAtCoord(if i % 2 == 0 then "scr_indep_firework_starburst" else "scr_indep_firework_trailburst", at, vector.zero, 2, false, false, false, false)
			wait(400)
		end
		return nil
	end)
end

local function playerMenu(menu: Menu, who: Player, name: string)
	local sub = menu:submenu(name, about(who))
	local them = function(): Ped
		return player.getPlayerPed(who)
	end
	sub:separator("Di chuyển")
	bind.action(sub, "Đi tới chỗ họ", { action = "Đi" }, function(): string?
		game.teleport(entity.getEntityCoords(them(), true) + vector.create(0, 0, 1), false)
		return nil
	end)
	bind.action(sub, "Lên xe của họ", { note = "Ghế còn trống", action = "Lên xe" }, function(): string?
		local car = ped.getVehiclePedIsIn(them(), false)
		if not car then
			return `{name} đang đi bộ`
		end
		for seat = 0, vehicle.getVehicleMaxNumberOfPassengers(car) - 1 do
			if vehicle.isVehicleSeatFree(car, seat, false) then
				ped.setPedIntoVehicle(game.me(), car, seat)
				return nil
			end
		end
		return "Không còn ghế trống"
	end)
	bind.action(sub, "Đánh dấu trên bản đồ", { note = "Đặt điểm đánh dấu vào vị trí của họ", action = "Đánh dấu" }, function(): string?
		world.setWaypoint(entity.getEntityCoords(them(), true))
		return nil
	end)
	sub:toggle("Theo dõi (Spectate)", watching == who, "Góc nhìn bám theo họ", bind.safe("Theo dõi", function(on)
		stopWatching()
		if on then
			network.networkSetInSpectatorMode(true, them())
			watching = who
		end
	end))
	bind.action(sub, "Bám đuôi tự động", { note = "Xe bạn sẽ chạy theo sau xe của họ bất kể đi đâu", action = "Đi theo" }, function(): string?
		local mine, theirs = game.vehicle(), ped.getVehiclePedIsIn(them(), false)
		if not mine then
			return "Hãy lên một chiếc xe trước"
		end
		if not theirs then
			return `{name} đang đi bộ`
		end
		-- Behind them (mode -1), at up to 40 m/s, driving around traffic.
		task.taskVehicleEscort(game.me(), mine, theirs, -1, 40, 786603, 8, 20, 20)
		return `Đang bám theo {name}: phanh hoặc vào Phương tiện › Tự lái › Dừng tự lái`
	end)
	gifts(sub, who, name)
	sub:separator("Tương tác")
	bind.action(sub, "Mặc trang phục của họ", { note = "Khi dùng chung mô hình nhân vật", action = "Sao chép" }, function(): string?
		local me, p = game.me(), them()
		if entity.getEntityModel(me) ~= entity.getEntityModel(p) then
			return "Bạn đang dùng mô hình nhân vật khác"
		end
		for slot = 0, 11 do
			ped.setPedComponentVariation(me, slot, ped.getPedDrawableVariation(p, slot), ped.getPedTextureVariation(p, slot), ped.getPedPaletteVariation(p, slot))
		end
		for _, slot in { 0, 1, 2, 6, 7 } do
			local prop = ped.getPedPropIndex(p, slot)
			if prop < 0 then
				ped.clearPedProp(me, slot)
			else
				ped.setPedPropIndex(me, slot, prop, ped.getPedPropTextureIndex(p, slot), true)
			end
		end
		return nil
	end)
end

-- Name tags over every other player's head, kept up as they come and go.
local function tags(menu: Menu)
	local shown: { [number]: number } = {}
	local function clear()
		for _, tag in shown do
			hud.removeMpGamerTag(tag)
		end
		shown = {}
	end
	bind.feature(menu, "players.tags", "Thẻ tên trên đầu", "Hiển thị tên phía trên đầu mỗi người chơi", {
		tick = function()
			local me = game.player()
			for slot = 0, SLOTS - 1 do
				local who = player.intToPlayerindex(slot)
				local active = who ~= me and network.networkIsPlayerActive(who)
				if active and not shown[slot] then
					local tag = hud.createFakeMpGamerTag(player.getPlayerPed(who), player.getPlayerName(who), false, false, "", 0)
					hud.setMpGamerTagVisibility(tag, 0, true, 0)
					shown[slot] = tag
				elseif not active and shown[slot] then
					hud.removeMpGamerTag(shown[slot])
					shown[slot] = nil
				end
			end
		end,
		off = clear,
	})
end

function M.build(root: Menu)
	local menu = root:submenu("Người chơi", "Những người chơi khác trong phiên GTA Online")
	local list = menu:submenu("Danh sách trong phòng", "Dịch chuyển, theo dõi, tặng đồ từng người")
	tags(menu)
	bind.feature(menu, "players.friendlyFire", "Bắn đồng đội", "Cho phép gây sát thương lên bạn bè và bang hội", {
		on = function()
			network.networkSetFriendlyFireOption(true)
			ped.setCanAttackFriendly(game.me(), true, false)
		end,
		off = function()
			network.networkSetFriendlyFireOption(false)
			ped.setCanAttackFriendly(game.me(), false, false)
		end,
	})
	list:onOpen(bind.safe("Người chơi", function()
		list:clear()
		if not network.networkIsSessionStarted() then
			list:label("Bạn chưa ở trong phiên GTA Online.")
			return
		end
		local me = game.player()
		local count = 0
		for slot = 0, SLOTS - 1 do
			local who = player.intToPlayerindex(slot)
			if who ~= me and network.networkIsPlayerActive(who) then
				playerMenu(list, who, player.getPlayerName(who))
				count += 1
			end
		end
		if count == 0 then
			list:label("Chưa có người chơi nào khác ở đây.")
		end
		if watching then
			bind.action(list, "Dừng theo dõi", { action = "Dừng" }, function(): string?
				stopWatching()
				return nil
			end)
		end
	end))
end

return M
