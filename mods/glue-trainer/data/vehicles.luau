--!strict
-- What the game keeps as numbers without names: its kinds of vehicle mods (eVehicleModType),
-- window tints and plate styles. Each list is by the game's index, from 0.

return {
	-- Mod slots the game names only for some vehicles (GET_MOD_SLOT_NAME).
	modSlots = {
		[0] = "Cánh gió", "Cản trước", "Cản sau", "Ốp hông", "Ống xả", "Khung chống lật", "Lưới tản nhiệt", "Nắp ca-pô",
		"Vè trái", "Vè phải", "Mui xe", "Động cơ", "Phanh", "Hộp số", "Còi", "Hệ thống treo",
		"Giáp xe", "Nitro", "Turbo", "Loa trầm", "Khói lốp", "Nhún thủy lực", "Đèn Xenon", "Mâm xe",
		"Mâm sau", "Khung biển số", "Biển số trang trí", "Viền nội thất", "Phụ kiện trang trí", "Bảng táp-lô", "Đồng hồ", "Loa cửa",
		"Ghế ngồi", "Vô-lăng", "Cần số", "Huy hiệu", "Dàn loa", "Cốp xe", "Thủy lực", "Khối động cơ",
		"Lọc gió", "Thanh giằng", "Ốp vòm bánh", "Ăng-ten", "Ốp trang trí", "Bình xăng", "Cửa sổ", "Kính xe", "Tem xe",
	},
	-- Slots that are on or off (toggled, not chosen).
	toggles = { [17] = true, [18] = true, [19] = true, [20] = true, [21] = true, [22] = true },
	windowTints = { [0] = "Không dán", "Đen tuyền", "Tối màu", "Mờ nhẹ", "Mặc định", "Kính Limo", "Xanh lục" },
	plates = { [0] = "Chữ xanh nền trắng", "Chữ vàng nền đen", "Chữ vàng nền xanh", "Xanh trên trắng 2", "Xanh trên trắng 3", "North Yankton" },
	-- SET_VEHICLE_WHEEL_TYPE's types of the base game.
	wheelTypes = { [0] = "Thể thao", "Cơ bắp", "Lowrider", "SUV", "Địa hình", "Độ Tuner", "Xe máy", "Hạng sang" },
	-- SET_VEHICLE_DOOR_OPEN's doors.
	doors = { [0] = "Trước trái", "Trước phải", "Sau trái", "Sau phải", "Nắp ca-pô", "Cốp sau" },
	-- Driving styles (flag sets the game's scripts use), for the autopilot.
	drivingStyles = {
		{ name = "Bình thường", flags = 786603 },
		{ name = "Tránh kẹt xe", flags = 786468 },
		{ name = "Phóng nhanh", flags = 1074528293 },
		{ name = "Vượt đèn đỏ", flags = 2883621 },
	},
}
