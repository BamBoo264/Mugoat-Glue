# Mugoat Glue cho iOS

Bản thử v0.1.0 dùng GLUE 5 và giao diện tiếng Việt. Giao diện chính gồm chọn chế độ chơi, nguồn tệp, thiết lập video và điều khiển cảm ứng, máy chủ, bạn bè, bản mod, đo hiệu năng và thông tin dự án.

Mugoat Glue kế thừa GLUE 5 theo GPL-3.0-or-later. Nguồn nền dùng commit 76e67701473a7996bb1918d83907c265fdb24194. Tên dự án phụ thuộc, thương hiệu trong trò chơi, dữ liệu dịch bên trong GTA V và nội dung do máy chủ/bản mod cung cấp theo nguồn gốc tương ứng.

## Build thử trên GitHub

1. Dùng repo riêng tư. Tạo release **nháp** và gắn đúng game.wasm của build 8b0b5899ed. SHA-256 phải là 11ca8d2c04c5e843d18ff4aea4899d72c86973c6b031df334e67c446b2ae83e0.
2. Trong Actions, chạy workflow **Build** với lựa chọn **ios**. Mugoat-Glue-ios.ipa xuất hiện trong cùng bản nháp. Giữ release ở chế độ nháp.
3. Ký IPA bằng tài khoản Apple của bạn trước khi cài. IPA cần iOS/iPadOS 16.4 trở lên. Đưa bộ tệp tương thích vào thư mục game trong tài liệu của ứng dụng. Xem hướng dẫn upstream ở [Cài đặt](docs/getting-started.md).

Mã nguồn, tệp WASM và IPA nằm ở các vị trí riêng. Git chỉ chứa mã nguồn. Workflow đọc WASM từ bản nháp và gửi IPA về đúng bản nháp đó.

