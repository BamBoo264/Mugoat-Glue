--!strict
-- You: health and protection, the police, abilities, sight.

local bind = require("../core/bind")
local game = require("../core/game")
local loop = require("../core/loop")
local prefs = require("../core/prefs")

local M = {}

-- What SET_ENTITY_PROOFS protects from, in its order.
local PROOFS = { "Đạn bắn", "Lửa", "Vụ nổ", "Va chạm", "Cận chiến", "Hơi nước", "Chết đuối" }

-- The proofs chosen, every frame while any is (missions reset them).
local function proofs(root: Menu)
	local item: Multi
	local function apply()
		local on = {}
		for _, i in item.value do
			on[i] = true
		end
		entity.setEntityProofs(game.me(), on[1] == true, on[2] == true, on[3] == true, on[4] == true, on[5] == true, on[6] == true, false, on[7] == true)
	end
	local function follow()
		prefs.set("self.proofs", item.value)
		if #item.value > 0 then
			loop.add("self.proofs", apply)
		else
			loop.remove("self.proofs")
			apply()
		end
	end
	item = root:multi("Kháng sát thương", PROOFS, prefs.get("self.proofs", {}), "Miễn nhiễm với những loại sát thương đã chọn", bind.safe("Kháng sát thương", follow))
	if #item.value > 0 then
		follow()
	end
end

-- Story mode: up again where you fell, without the hospital and its bill. GTA Online's
-- scripts respawn the player their own way: left to them.
local function respawn(root: Menu)
	local function story(): boolean
		return not network.networkIsSessionStarted()
	end
	bind.feature(root, "self.respawn", "Hồi sinh tại chỗ ngã", "Chế độ cốt truyện: không vào viện, không mất tiền", {
		on = function()
			misc.setFadeOutAfterDeath(false)
		end,
		tick = function()
			if not story() then
				return
			end
			-- The game's restart (hospital, fees) is skipped from the moment the player dies.
			misc.pauseDeathArrestRestart(true)
			misc.ignoreNextRestart(true)
			local me = game.me()
			if not entity.isEntityDead(me, false) then
				return
			end
			local at = entity.getEntityCoords(me, true)
			network.networkResurrectLocalPlayer(at, entity.getEntityHeading(me), 0, false, false, 0, 0)
			me = game.me()
			entity.setEntityHealth(me, entity.getEntityMaxHealth(me), nil)
			task.clearPedTasksImmediately(me)
			camera.doScreenFadeIn(500)
		end,
		off = function()
			misc.pauseDeathArrestRestart(false)
			misc.ignoreNextRestart(false)
			misc.setFadeOutAfterDeath(true)
		end,
	})
end

local function health(root: Menu)
	root:separator("Máu & Bảo vệ")
	bind.feature(root, "self.god", "Bất tử (God mode)", "Không gì có thể làm bạn bị thương", {
		-- Every frame: missions switch it off, and a new model is a new ped.
		tick = function()
			player.setPlayerInvincible(game.player(), true)
		end,
		off = function()
			player.setPlayerInvincible(game.player(), false)
		end,
	})
	proofs(root)
	bind.slider(root, "self.regen", "Tốc độ hồi máu", 1, 0, 10, { step = 0.5, unit = "×", note = "Tốc độ máu tự hồi phục" }, function(v)
		player.setPlayerHealthRechargeMultiplier(game.player(), v)
	end)
	-- Invincible only while falling (God mode, when on, keeps it).
	local falling = false
	bind.feature(root, "self.noFall", "Không mất máu khi ngã", "Tiếp đất từ bất kỳ độ cao nào", {
		tick = function()
			local me = game.me()
			local now = ped.isPedFalling(me) or ped.isPedInParachuteFreeFall(me)
			if now ~= falling and not bind.on("self.god") then
				player.setPlayerInvincible(game.player(), now)
			end
			falling = now
		end,
		off = function()
			if falling and not bind.on("self.god") then
				player.setPlayerInvincible(game.player(), false)
			end
			falling = false
		end,
	})
	bind.feature(root, "self.noCritical", "Không bị bắn chí mạng", "Miễn nhiễm phát bắn vào đầu", {
		tick = function()
			ped.setPedSuffersCriticalHits(game.me(), false)
		end,
		off = function()
			ped.setPedSuffersCriticalHits(game.me(), true)
		end,
	})
	bind.feature(root, "self.ragdoll", "Chống ngã", "Không bao giờ bị hất văng hay té ngã", {
		tick = function()
			ped.setPedCanRagdoll(game.me(), false)
			ped.setPedCanRagdollFromPlayerImpact(game.me(), false)
		end,
		off = function()
			ped.setPedCanRagdoll(game.me(), true)
			ped.setPedCanRagdollFromPlayerImpact(game.me(), true)
		end,
	})
	bind.command(root, "self.heal", "Hồi máu & giáp", { note = "Đầy máu và giáp ngay lập tức", action = "Hồi phục" }, function(): string?
		local me = game.me()
		entity.setEntityHealth(me, entity.getEntityMaxHealth(me), nil)
		ped.setPedArmour(me, player.getPlayerMaxArmour(game.player()))
		return "Đã hồi đầy máu & giáp"
	end)
	bind.command(root, "self.clean", "Làm sạch nhân vật", { note = "Xóa vết máu, bụi bẩn và nước", action = "Làm sạch" }, function(): string?
		local me = game.me()
		ped.clearPedBloodDamage(me)
		ped.resetPedVisibleDamage(me)
		ped.clearPedWetness(me)
		return nil
	end)
	bind.command(root, "self.die", "Tự sát", { action = "Chết" }, function(): string?
		entity.setEntityHealth(game.me(), 0, nil)
		return nil
	end)
	bind.feature(root, "self.oxygen", "Thở dưới nước vô hạn", "Lặn dưới nước bao lâu tùy thích", {
		tick = function()
			player.setPlayerUnderwaterBreathPercentRemaining(game.player(), 100)
		end,
	})
	respawn(root)
end

local function police(root: Menu)
	root:separator("Cảnh sát")
	bind.feature(root, "self.neverWanted", "Không bao giờ bị truy nã", "Xóa sao truy nã ngay khi vừa xuất hiện", {
		tick = function()
			local me = game.player()
			if player.getPlayerWantedLevel(me) > 0 then
				player.clearPlayerWantedLevel(me)
			end
		end,
	})
	local wanted = root:number("Mức truy nã", 0, { min = 0, max = 5, note = "Đặt số sao truy nã ngay" }, bind.safe("Mức truy nã", function(stars)
		local me = game.player()
		player.setPlayerWantedLevel(me, stars, false)
		player.setPlayerWantedLevelNow(me, false)
	end))
	root:onOpen(bind.safe("Bản thân", function()
		wanted.value = player.getPlayerWantedLevel(game.player())
	end))
	bind.feature(root, "self.ignored", "Bị mọi người phớt lờ", "Cảnh sát và băng đảng không tấn công bạn", {
		tick = function()
			player.setEveryoneIgnorePlayer(game.player(), true)
			player.setPoliceIgnorePlayer(game.player(), true)
		end,
		off = function()
			player.setEveryoneIgnorePlayer(game.player(), false)
			player.setPoliceIgnorePlayer(game.player(), false)
		end,
	})
end

local function abilities(root: Menu)
	root:separator("Năng lực siêu phàm")
	bind.feature(root, "self.superJump", "Siêu nhảy", "Nhảy cao vượt qua nóc nhà", {
		tick = function()
			misc.setSuperJumpThisFrame(game.player())
		end,
	})
	bind.feature(root, "self.superRun", "Siêu chạy nhanh", "Chạy nước rút nhanh như xe hơi", {
		tick = function()
			local me = game.me()
			if pad.isControlPressed(0, Control.SPRINT) and not ped.isPedInAnyVehicle(me, false) and not entity.isEntityInAir(me) then
				local ahead = entity.getEntityForwardVector(me) * 25
				entity.setEntityVelocity(me, vector.create(ahead.x, ahead.y, entity.getEntityVelocity(me).z))
			end
		end,
	})
	-- Flying while Jump is held, where the camera looks; let go and you glide down.
	local flying = false
	bind.feature(root, "self.fly", "Bay như siêu anh hùng", "Giữ Nhảy để bay theo hướng nhìn; thả ra để lượn xuống", {
		tick = function()
			local me = game.me()
			if ped.isPedInAnyVehicle(me, false) then
				flying = false
				return
			end
			if pad.isControlPressed(0, Control.JUMP) then
				flying = true
				entity.setEntityVelocity(me, geo.direction(camera.getGameplayCamRot(2)) * 35)
			elseif flying then
				if entity.isEntityInAir(me) then
					local v = entity.getEntityVelocity(me)
					entity.setEntityVelocity(me, vector.create(v.x * 0.97, v.y * 0.97, math.max(v.z, -3)))
				else
					flying = false
				end
			end
		end,
		off = function()
			flying = false
		end,
	})
	-- People and vehicles within reach pushed out of it (every other frame: the pools are
	-- walked each time).
	local frame = 0
	bind.feature(root, "self.forceField", "Trường lực đẩy lùi", "Hất văng người và xe ở gần bạn ra xa", {
		tick = function()
			frame += 1
			if frame % 2 == 1 then
				return
			end
			local me = game.me()
			local mine = game.vehicle()
			local at = entity.getEntityCoords(me, true)
			local function push(e: Entity)
				if mine and e == (mine :: Entity) then
					return
				end
				local away = entity.getEntityCoords(e, true) - at
				local out = vector.normalize(vector.create(away.x, away.y, 0)) * 25
				entity.setEntityVelocity(e, vector.create(out.x, out.y, 6))
			end
			for _, p in world.nearby("peds", me, 8) do
				if not ped.isPedInAnyVehicle(p, false) then
					push(p)
				end
			end
			for _, v in world.nearby("vehicles", me, 10) do
				push(v)
			end
		end,
	})
	-- The game takes 1 to 1.49 (more is ignored).
	bind.slider(root, "self.run", "Tốc độ chạy", 1, 1, 1.49, { step = 0.01, unit = "×" }, function(v)
		player.setRunSprintMultiplierForPlayer(game.player(), v)
	end)
	bind.slider(root, "self.swim", "Tốc độ bơi", 1, 1, 1.49, { step = 0.01, unit = "×" }, function(v)
		player.setSwimMultiplierForPlayer(game.player(), v)
	end)
	bind.feature(root, "self.stamina", "Thể lực vô hạn", "Chạy và bơi không biết mệt", {
		tick = function()
			player.restorePlayerStamina(game.player(), 1)
		end,
	})
	bind.feature(root, "self.special", "Kỹ năng đặc biệt vô hạn", "Thanh kỹ năng nhân vật chính luôn đầy", {
		tick = function()
			player.specialAbilityFillMeter(game.player(), true, 0)
		end,
	})
	bind.feature(root, "self.quiet", "Không gây tiếng động", "Không ai nghe thấy tiếng bước chân của bạn", {
		tick = function()
			player.setPlayerNoiseMultiplier(game.player(), 0)
		end,
		off = function()
			player.setPlayerNoiseMultiplier(game.player(), 1)
		end,
	})
	bind.feature(root, "self.parachute", "Luôn có dù lượn", "Tự động đeo dù trên lưng khi thiếu", {
		tick = function()
			local me = game.me()
			local chute = joaat("GADGET_PARACHUTE")
			if not weapon.hasPedGotWeapon(me, chute, 0) then
				weapon.giveWeaponToPed(me, chute, 1, false, false)
			end
		end,
	})
	bind.feature(root, "self.invisible", "Tàng hình", "Người khác không thấy bạn; bạn vẫn thấy mình", {
		tick = function()
			entity.setEntityVisible(game.me(), false, false)
			network.setEntityLocallyVisible(game.me())
		end,
		off = function()
			entity.setEntityVisible(game.me(), true, false)
		end,
	})
	bind.feature(root, "self.burn", "Bốc cháy", "Cháy rực nhưng miễn nhiễm sát thương lửa", {
		on = function()
			entity.setEntityProofs(game.me(), false, true, false, false, false, false, false, false)
			fire.startEntityFire(game.me())
		end,
		off = function()
			fire.stopEntityFire(game.me())
			entity.setEntityProofs(game.me(), false, false, false, false, false, false, false, false)
		end,
	})
end

-- The game's particle effects that work on a person: name, asset, effect.
local EFFECTS = {
	{ "Hoa chú hề", "scr_rcbarry2", "scr_clown_appears" },
	{ "Ánh sáng ngoài hành tinh", "scr_rcbarry1", "scr_alien_disintegrate" },
	{ "Dịch chuyển ngoài hành tinh", "scr_rcbarry1", "scr_alien_teleport" },
	{ "Tia lửa điện", "core", "ent_brk_sparking_wires" },
	{ "Tiền bay", "core", "ent_brk_banknotes" },
	{ "Pháo hoa", "scr_indep_fireworks", "scr_indep_firework_trailburst" },
}

local function body(root: Menu)
	root:separator("Cơ thể")
	local names = {}
	for _, e in EFFECTS do
		table.insert(names, e[1])
	end
	local effect = bind.choice(root, "self.effect", "Kiểu hiệu ứng", names, names[1], "Hiệu ứng hạt hiển thị quanh bạn")
	-- Shown again every half a second, as the effects are bursts.
	local next = 0
	bind.feature(root, "self.effects", "Bật hiệu ứng hạt", "Hạt hiệu ứng quanh người như đã chọn ở trên", {
		tick = function()
			if os.clock() < next then
				return
			end
			next = os.clock() + 0.5
			local e = EFFECTS[effect.value]
			streaming.requestNamedPtfxAsset(e[2])
			if not streaming.hasNamedPtfxAssetLoaded(e[2]) then
				return
			end
			graphics.useParticleFxAsset(e[2])
			-- In GTA Online, for everyone to see.
			if game.online() then
				graphics.startNetworkedParticleFxNonLoopedOnEntity(e[3], game.me(), vector.zero, vector.zero, 1, false, false, false)
			else
				graphics.startParticleFxNonLoopedOnEntity(e[3], game.me(), vector.zero, vector.zero, 1, false, false, false)
			end
		end,
		off = function()
			next = 0
		end,
	})
	-- Fire from the mouth, as long as it is on (and fireproof meanwhile).
	local flame: number? = nil
	bind.feature(root, "self.breatheFire", "Phun lửa", "Phun ngọn lửa từ miệng", {
		on = function()
			if not streaming.load("core", "ptfx") then
				error("không thể tải hiệu ứng ngọn lửa", 0)
			end
			local me = game.me()
			entity.setEntityProofs(me, false, true, false, false, false, false, false, false)
			graphics.useParticleFxAsset("core")
			-- Out of the mouth, forward (the head bone's axes).
			flame = graphics.startParticleFxLoopedOnEntityBone("ent_sht_flame", me, -0.02, 0.2, 0, 90, 100, 90, ped.getPedBoneIndex(me, Bone.HEAD), 1, false, false, false)
		end,
		off = function()
			if flame then
				graphics.stopParticleFxLooped(flame, false)
				flame = nil
			end
			entity.setEntityProofs(game.me(), false, false, false, false, false, false, false, false)
		end,
	})
	bind.command(root, "self.clone", "Nhân bản bản thân", { note = "Tạo một bản sao đứng cạnh bạn", action = "Nhân bản" }, function(): string?
		local copy = ped.clonePed(game.me(), game.online(), false, true)
		if copy then
			entity.setPedAsNoLongerNeeded(copy)
		end
		return nil
	end)
	bind.command(root, "self.ragdoll", "Ngã gục", { action = "Ngã" }, function(): string?
		ped.setPedToRagdoll(game.me(), 3000, 3000, 0, false, false, false)
		return nil
	end)
	-- The game's damage looks: name, pack.
	local packs = {
		{ "Bị xe tông", "BigHitByVehicle" },
		{ "Bị xe cán qua", "BigRunOverByVehicle" },
		{ "Vụ nổ lớn", "Explosion_Large" },
		{ "Bị bỏng", "Burnt_Ped_0" },
		{ "Ngã từ trên cao", "Fall" },
		{ "Bị đánh bầm dập", "Skin_Melee_0" },
		{ "Bị tra tấn", "SCR_Torture" },
		{ "Thùng rác", "SCR_Dumpster" },
	}
	local packNames = {}
	for _, pk in packs do
		table.insert(packNames, pk[1])
	end
	local pack = bind.choice(root, "self.damagePack", "Vết thương", packNames, packNames[1], "Kiểu thương tích sẽ áp lên người")
	bind.command(root, "self.hurt", "Trông như bị thương", { note = "Thêm vết máu và bầm tím (dùng Làm sạch để xóa)", action = "Áp dụng" }, function(): string?
		ped.applyPedDamagePack(game.me(), packs[pack.value][2], 0, 1)
		return nil
	end)
	bind.slider(root, "self.wet", "Độ ướt", 0, 0, 2, { step = 0.1, unit = "m", note = "Mức nước dâng lên trên người" }, function(v)
		if v > 0 then
			ped.setPedWetnessHeight(game.me(), v)
		else
			ped.clearPedWetness(game.me())
		end
	end)
	bind.slider(root, "self.sweat", "Đổ mồ hôi", 0, 0, 100, { step = 10, unit = "%" }, function(v)
		ped.setPedSweat(game.me(), v)
	end)
	-- Lines every ped's voice has (the game picks the voice's own recording).
	local lines = {
		{ "Xin chào", "GENERIC_HI" },
		{ "Tạm biệt", "GENERIC_BYE" },
		{ "Cảm ơn", "GENERIC_THANKS" },
		{ "Dạo này sao rồi", "GENERIC_HOWS_IT_GOING" },
		{ "Sao cũng được", "GENERIC_WHATEVER" },
		{ "Xúc phạm", "GENERIC_INSULT_HIGH" },
		{ "Chửi thề", "GENERIC_CURSE_HIGH" },
		{ "Kinh ngạc", "GENERIC_SHOCKED_HIGH" },
		{ "Hoảng sợ", "GENERIC_FRIGHTENED_HIGH" },
		{ "Kifflom", "KIFFLOM_GREET" },
	}
	local says = {}
	for _, l in lines do
		table.insert(says, l[1])
	end
	local line = bind.choice(root, "self.line", "Câu thoại", says, says[1], "Câu nhân vật sẽ thốt ra bằng giọng thật")
	bind.command(root, "self.say", "Nói câu thoại", { action = "Nói" }, function(): string?
		audio.playPedAmbientSpeechNative(game.me(), lines[line.value][2], "SPEECH_PARAMS_FORCE_SHOUTED", 0)
		return nil
	end)
end

local function sight(root: Menu)
	root:separator("Thị giác")
	bind.feature(root, "self.nightVision", "Nhìn ban đêm", nil, {
		on = function()
			graphics.setNightvision(true)
		end,
		off = function()
			graphics.setNightvision(false)
		end,
	})
	bind.feature(root, "self.thermal", "Tầm nhiệt", nil, {
		on = function()
			graphics.setSeethrough(true)
		end,
		off = function()
			graphics.setSeethrough(false)
		end,
	})
	bind.feature(root, "self.drunk", "Say rượu", "Đi loạng choạng và nhìn mờ như sau đêm tiệc tùng", {
		on = function()
			streaming.load("move_m@drunk@verydrunk", "clipSet")
			ped.setPedMovementClipset(game.me(), "move_m@drunk@verydrunk", 1)
			camera.shakeGameplayCam("DRUNK_SHAKE", 1)
			graphics.setTimecycleModifier("Drunk")
		end,
		off = function()
			ped.resetPedMovementClipset(game.me(), 1)
			camera.stopGameplayCamShaking(true)
			graphics.clearTimecycleModifier()
		end,
	})
end

function M.build(root: Menu)
	local me = root:submenu("Bản thân", "Máu, bảo vệ, cảnh sát, năng lực, cơ thể, thị giác")
	health(me)
	police(me)
	abilities(me)
	body(me)
	sight(me)
end

return M
