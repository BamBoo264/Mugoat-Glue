--!strict
-- Places to go, by kind: where the player stands there (x, y, z) and faces (heading). The map has
-- no list of its own; these are the well-known spots of Los Santos and Blaine County.

export type Place = { name: string, x: number, y: number, z: number, heading: number }
export type Kind = { name: string, places: { Place } }

local function at(name: string, x: number, y: number, z: number, heading: number?): Place
	return { name = name, x = x, y = y, z = z, heading = heading or 0 }
end

local kinds: { Kind } = {
	{
		name = "Nhà riêng",
		places = {
			at("Nhà Michael", -852.4, 160.0, 65.6, 90),
			at("Nhà Franklin (Strawberry)", -14.8, -1454.0, 30.5, 0),
			at("Biệt thự Franklin (Vinewood)", 7.9, 548.1, 175.5, 330),
			at("Nhà xe Trevor", 1985.7, 3812.2, 32.2, 30),
			at("Nhà Lester", 1273.9, -1719.3, 54.8, 30),
		},
	},
	{
		name = "Địa danh nổi bật",
		places = {
			at("Đỉnh núi Chiliad", 501.8, 5604.0, 797.9, 180),
			at("Nóc tháp Maze Bank", -75.0, -818.0, 326.2, 0),
			at("Nóc tòa nhà FIB", 135.5, -749.0, 258.2, 0),
			at("Biển hiệu Vinewood", 711.4, 1198.1, 348.5, 180),
			at("Đài thiên văn Galileo", -438.8, 1076.1, 352.4, 0),
			at("Trung tâm Kortz", -2243.8, 264.0, 174.6, 0),
			at("Bến tàu Del Perro", -1850.1, -1231.7, 13.0, 0),
			at("Quảng trường Legion", 195.2, -933.8, 30.7, 0),
			at("Bãi biển Vespucci", -1223.0, -1491.0, 4.4, 0),
			at("Bến Chumash", -3426.7, 967.5, 8.3, 90),
		},
	},
	{
		name = "Sân bay",
		places = {
			at("Sân bay Quốc tế Los Santos", -1336.0, -3044.0, 13.9, 240),
			at("Sân bay Sandy Shores", 1747.0, 3273.7, 41.1, 105),
			at("Sân bay McKenzie", 2121.7, 4796.3, 41.1, 25),
			at("Căn cứ quân sự Fort Zancudo", -2047.4, 3132.1, 32.8, 60),
		},
	},
	{
		name = "Cửa hàng & dịch vụ",
		places = {
			at("Los Santos Customs (Burton)", -365.4, -131.8, 37.9, 250),
			at("Cửa hàng súng Ammu-Nation", 19.6, -1112.8, 29.8, 140),
			at("Showroom xe Simeon", -56.4, -1097.5, 26.4, 30),
			at("Bệnh viện Pillbox Hill", 298.3, -584.5, 43.3, 70),
			at("Đồn cảnh sát Mission Row", 425.1, -979.6, 30.7, 90),
			at("Câu lạc bộ Vanilla Unicorn", 127.4, -1307.7, 29.2, 210),
			at("Ngân hàng Pacific Standard", 235.0, 216.4, 106.3, 290),
			at("Xưởng xe Beeker (Paleto Bay)", 110.3, 6626.4, 31.8, 225),
			at("Quán rượu Yellow Jack", 1986.6, 3054.0, 47.2, 330),
		},
	},
	{
		name = "Ngoại thành",
		places = {
			at("Phòng thí nghiệm Humane Labs", 3617.2, 3739.9, 28.7, 140),
			at("Nhà tù Bolingbroke", 1679.0, 2513.7, 45.6, 0),
			at("Trại Altruist", -1170.8, 4926.0, 224.3, 0),
			at("Thị trấn Paleto Bay", -276.0, 6226.0, 31.5, 45),
			at("Thị trấn Grapeseed", 1700.0, 4800.0, 42.0, 0),
		},
	},
}

return kinds
