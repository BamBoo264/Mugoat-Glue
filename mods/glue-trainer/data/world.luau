--!strict
-- The world's settings the game names only in its own data: weather types (weather.xml), its
-- gravity levels (SET_GRAVITY_LEVEL's 0 to 3), screen filters (timecycle modifiers of the base
-- game's timecycle_mods files; an unknown one does nothing), dispatch services, the scripts
-- that guard restricted areas.

return {
	-- The game's names, and as the trainer shows them.
	weathers = {
		{ "EXTRASUNNY", "Nắng gắt" },
		{ "CLEAR", "Trời quang" },
		{ "CLOUDS", "Nhiều mây" },
		{ "SMOG", "Khói mù" },
		{ "FOGGY", "Sương mù" },
		{ "OVERCAST", "Âm u" },
		{ "RAIN", "Mưa" },
		{ "THUNDER", "Giông bão" },
		{ "CLEARING", "Tạnh dần" },
		{ "NEUTRAL", "Trung tính" },
		{ "SNOW", "Tuyết rơi" },
		{ "BLIZZARD", "Bão tuyết" },
		{ "SNOWLIGHT", "Tuyết nhẹ" },
		{ "XMAS", "Giáng sinh" },
	},
	gravity = { "Trái Đất", "Mặt Trăng", "Trọng lực thấp", "Không trọng lực" },
	-- { label, timecycle modifier }
	filters = {
		{ "Đen trắng", "CAMERA_BW" },
		{ "Camera an ninh", "CAMERA_secuirity" },
		{ "Ống ngắm bắn tỉa", "Sniper" },
		{ "Sương đỏ", "REDMIST" },
		{ "Say xỉn", "Drunk" },
		{ "Ảo giác", "DrugsMichaelAliensFight" },
		{ "Chú hề", "DrugsTrevorClownsFight" },
		{ "Góc nhìn chó Chop", "CHOP" },
		{ "Tử nạn (Wasted)", "MP_death_grade" },
		{ "Dưới nước", "underwater" },
		{ "Màu bão hòa", "rply_saturation" },
		{ "Tối viền", "rply_vignette" },
		{ "Phim 1", "NG_filmic01" },
		{ "Phim 2", "NG_filmic02" },
		{ "Phim 3", "NG_filmic03" },
		{ "Phim 4", "NG_filmic04" },
		{ "Phim 5", "NG_filmic05" },
		{ "Khán giả", "spectator1" },
		{ "Mất điện", "BlackOut" },
		{ "Choáng váng", "hud_def_blur" },
	},
	-- ENABLE_DISPATCH_SERVICE's services (police cars, helicopters, boats, roadblocks, SWAT...).
	dispatchServices = 15,
	-- The game's scripts that chase the player out of the army base, the prison and the airport.
	restrictedAreas = { "restrictedareas", "re_armybase", "re_prison", "re_lossantosintl", "am_armybase" },
}
